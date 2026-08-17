//! Deterministic visual-review artifact assembly.

use std::{collections::BTreeSet, fmt::Write as _, fs, path::Path};

use isometric_core::ObjectId;
use isometric_publish::encode_lossless_webp;
use isometric_render::{
    IndexedImage, MAIN_QUAD_OBJECT_ID, RenderLayout, TileRequest, render_layout,
    render_selected_region, render_tile, required_tile_guard, stable_hash,
};
use isometric_style::StylePack;
use isometric_world::{SemanticClass, World, WorldObject};
use serde_json::json;

const PARKING_SCENE_OBJECT_ID: u64 = 195_279_117_591_313_501;

#[derive(Clone, Copy)]
struct CandidateSpec {
    id: &'static str,
    label: &'static str,
    deviations: &'static str,
}

const CANDIDATE_A: CandidateSpec = CandidateSpec {
    id: "stanford_v1.candidate_a",
    label: "Candidate A",
    deviations: "# Candidate A known deviations\n\n- The world remains vector-only with 387,096 ppm unknown coverage; NAIP and LiDAR evidence are not compiled.\n- Ordinary buildings still use flat roofs and do not yet have facade window cadence.\n- Vegetation coverage follows mapped vector polygons and lacks LiDAR-refined individual canopy.\n- The original palette contains 16 colors, substantially less tonal variation than the live Isometric NYC reference.\n- A same-viewport observation found much lower edge density and a dominant ground field; Candidate A is recognizably Stanford but remains more diagrammatic and sparse than the target analogue.\n- No reference screenshots, generated final pixels, or manually painted saved tiles are included.\n- Candidate A requires explicit owner acceptance or rejection before Candidate B starts.\n",
};

const CANDIDATE_B: CandidateSpec = CandidateSpec {
    id: "stanford_v1.candidate_b",
    label: "Candidate B",
    deviations: "# Candidate B known deviations\n\n- The world remains vector-only with 387,096 ppm unknown coverage; NAIP and LiDAR evidence are not compiled.\n- Convex ordinary buildings receive procedural hip roofs; complex and courtyard footprints retain flat roofs.\n- Facade openings use stable grammar rather than surveyed architectural detail.\n- Vegetation remains bounded by mapped vector polygons and lacks LiDAR-refined species or individual canopy evidence.\n- Parking markings are world-anchored visual grammar, not surveyed stall geometry.\n- No reference screenshots, generated final pixels, or manually painted saved tiles are included.\n- Candidate B requires review before it can become the approved style.\n",
};

struct Scene {
    id: &'static str,
    title: &'static str,
    image: IndexedImage,
    crop: Crop,
}

#[derive(Clone, Copy)]
struct Crop {
    x: u32,
    y: u32,
    width: u32,
    height: u32,
}

/// Writes the complete original Candidate A review pack.
pub(super) fn write_candidate_a(
    world: &World,
    base_style: &StylePack,
    world_sha256: &str,
    style_sha256: &str,
    output: &Path,
) -> Result<String, String> {
    write_candidate(
        world,
        base_style,
        world_sha256,
        style_sha256,
        output,
        CANDIDATE_A,
    )
}

/// Writes the bounded Candidate B review pack.
pub(super) fn write_candidate_b(
    world: &World,
    style: &StylePack,
    world_sha256: &str,
    style_sha256: &str,
    output: &Path,
) -> Result<String, String> {
    write_candidate(
        world,
        style,
        world_sha256,
        style_sha256,
        output,
        CANDIDATE_B,
    )
}

fn write_candidate(
    world: &World,
    base_style: &StylePack,
    world_sha256: &str,
    style_sha256: &str,
    output: &Path,
    spec: CandidateSpec,
) -> Result<String, String> {
    if output.exists() {
        return Err(format!(
            "candidate output already exists: {}",
            output.display()
        ));
    }
    let staging = staging_path(output)?;
    if staging.exists() {
        return Err(format!(
            "candidate staging already exists: {}",
            staging.display()
        ));
    }
    if let Some(parent) = output.parent() {
        fs::create_dir_all(parent).map_err(|error| error.to_string())?;
    }
    fs::create_dir(&staging).map_err(|error| error.to_string())?;
    let result = write_staged(
        world,
        base_style,
        world_sha256,
        style_sha256,
        &staging,
        spec,
    )
    .and_then(|summary| {
        fs::rename(&staging, output).map_err(|error| error.to_string())?;
        Ok(format!("{summary} at {}", output.display()))
    });
    if result.is_err() && staging.exists() {
        fs::remove_dir_all(&staging).map_err(|error| error.to_string())?;
    }
    result
}

#[allow(clippy::too_many_lines)]
fn write_staged(
    world: &World,
    base_style: &StylePack,
    world_sha256: &str,
    style_sha256: &str,
    output: &Path,
    spec: CandidateSpec,
) -> Result<String, String> {
    let mut style = base_style.clone();
    style.world_mm_per_half_step = 250;
    style.validate().map_err(|error| error.to_string())?;
    let layout = render_layout(world, &style).map_err(|error| error.to_string())?;
    let master = render_tiled_master(world, &style, layout)?;
    let hoover = named(world, "Hoover Tower")?;
    let church = named(world, "Memorial Church")?;
    let quad = by_raw_id(world, MAIN_QUAD_OBJECT_ID)?;
    let parking = by_raw_id(world, PARKING_SCENE_OBJECT_ID)?;
    let canopy = largest_class(world, SemanticClass::Vegetation)?;

    let hoover_center = image_anchor(layout, hoover, &style)?;
    let church_center = image_anchor(layout, church, &style)?;
    let quad_center = image_anchor(layout, quad, &style)?;
    let scenes = vec![
        scene(
            "hoover-tower",
            "Hoover Tower silhouette",
            &master,
            (hoover_center.0, hoover_center.1 - 80),
            512,
            512,
        )?,
        scene(
            "church-main-quad",
            "Memorial Church and Main Quad",
            &master,
            (
                church_center.0.midpoint(quad_center.0),
                church_center.1.midpoint(quad_center.1),
            ),
            1_600,
            1_000,
        )?,
        scene(
            "roads-empty-parking",
            "Roads and empty parking",
            &master,
            image_anchor(layout, parking, &style)?,
            768,
            512,
        )?,
        scene(
            "canopy-buildings",
            "Dense canopy and ordinary buildings",
            &master,
            image_anchor(layout, canopy, &style)?,
            768,
            512,
        )?,
    ];

    fs::create_dir(output.join("scenes")).map_err(|error| error.to_string())?;
    fs::create_dir(output.join("masks")).map_err(|error| error.to_string())?;
    for scene in &scenes {
        write_webp(
            &output.join("scenes").join(format!("{}.webp", scene.id)),
            &scene.image,
            &style,
        )?;
    }
    write_landmark_mask(
        output,
        world,
        &style,
        layout,
        hoover.id(),
        &scenes[0].crop,
        "hoover-tower",
    )?;
    write_landmark_mask(
        output,
        world,
        &style,
        layout,
        church.id(),
        &scenes[1].crop,
        "memorial-church",
    )?;
    write_landmark_mask(
        output,
        world,
        &style,
        layout,
        quad.id(),
        &scenes[1].crop,
        "main-quad",
    )?;

    let contact = contact_sheet(&scenes, style.ordinary.terrain[0], style.palette.len())?;
    write_webp(&output.join("contact-sheet.webp"), &contact, &style)?;
    let metrics = scenes
        .iter()
        .map(|scene| scene_metrics(scene, &style))
        .collect::<Vec<_>>();
    let report = json!({
        "schema": "isometric-style-candidate/v1",
        "candidate": spec.id,
        "status": "owner-review-required",
        "style_id": style.id,
        "world_sha256": world_sha256,
        "style_sha256": style_sha256,
        "render": {
            "world_mm_per_half_step": style.world_mm_per_half_step,
            "elevation_mm_per_pixel": style.elevation_mm_per_pixel,
            "subpixels_per_pixel": style.subpixels_per_pixel,
        },
        "master": {
            "width": master.width(),
            "height": master.height(),
            "indexed_hash": format!("{:016x}", stable_hash(master.pixels())),
        },
        "palette_colors": style.palette.len(),
        "contact_sheet": {
            "file": "contact-sheet.webp",
            "indexed_hash": format!("{:016x}", stable_hash(contact.pixels())),
        },
        "scenes": metrics,
        "reference_images_redistributed": false,
        "manual_tile_painting": false,
        "generated_final_pixels": false,
    });
    let mut report_bytes = serde_json::to_vec_pretty(&report).map_err(|error| error.to_string())?;
    report_bytes.push(b'\n');
    fs::write(output.join("metrics.json"), report_bytes).map_err(|error| error.to_string())?;
    fs::write(output.join("index.html"), contact_html(&scenes, spec))
        .map_err(|error| error.to_string())?;
    fs::write(output.join("known-deviations.md"), spec.deviations)
        .map_err(|error| error.to_string())?;

    Ok(format!(
        "wrote {} with {} scenes and contact sheet: {:016x}",
        spec.label,
        scenes.len(),
        stable_hash(contact.pixels())
    ))
}

fn scene(
    id: &'static str,
    title: &'static str,
    master: &IndexedImage,
    center: (i64, i64),
    width: u32,
    height: u32,
) -> Result<Scene, String> {
    let crop = centered_crop(master, center, width, height);
    let image = master
        .crop(crop.x, crop.y, crop.width, crop.height)
        .map_err(|error| error.to_string())?;
    Ok(Scene {
        id,
        title,
        image,
        crop,
    })
}

fn render_tiled_master(
    world: &World,
    style: &StylePack,
    layout: RenderLayout,
) -> Result<IndexedImage, String> {
    let tile_size = 512_u32;
    let guard = required_tile_guard(style).map_err(|error| error.to_string())?;
    let capacity = usize::try_from(layout.width())
        .ok()
        .and_then(|width| width.checked_mul(usize::try_from(layout.height()).ok()?))
        .ok_or_else(|| "candidate master capacity overflow".to_owned())?;
    let mut pixels = vec![style.ordinary.terrain[0]; capacity];
    for row in 0..layout.height().div_ceil(tile_size) {
        for column in 0..layout.width().div_ceil(tile_size) {
            let tile = render_tile(
                world,
                style,
                layout,
                TileRequest {
                    column,
                    row,
                    tile_size,
                    guard,
                },
            )
            .map_err(|error| error.to_string())?;
            blit(
                &mut pixels,
                layout.width(),
                &tile.image,
                column * tile_size,
                row * tile_size,
            );
        }
    }
    IndexedImage::from_pixels(layout.width(), layout.height(), pixels, style.palette.len())
        .map_err(|error| error.to_string())
}

fn centered_crop(master: &IndexedImage, center: (i64, i64), width: u32, height: u32) -> Crop {
    let width = width.min(master.width());
    let height = height.min(master.height());
    let maximum_x = i64::from(master.width() - width);
    let maximum_y = i64::from(master.height() - height);
    let x = (center.0 - i64::from(width) / 2).clamp(0, maximum_x);
    let y = (center.1 - i64::from(height) / 2).clamp(0, maximum_y);
    Crop {
        x: u32::try_from(x).expect("clamped crop x"),
        y: u32::try_from(y).expect("clamped crop y"),
        width,
        height,
    }
}

fn image_anchor(
    layout: RenderLayout,
    object: &WorldObject,
    style: &StylePack,
) -> Result<(i64, i64), String> {
    layout
        .image_coordinates(object.anchor(), style)
        .map_err(|error| error.to_string())
}

fn named<'a>(world: &'a World, name: &str) -> Result<&'a WorldObject, String> {
    world
        .objects()
        .iter()
        .find(|object| object.name() == Some(name))
        .ok_or_else(|| format!("candidate world lacks {name}"))
}

fn by_raw_id(world: &World, raw_id: u64) -> Result<&WorldObject, String> {
    world
        .objects()
        .iter()
        .find(|object| object.id().get() == raw_id)
        .ok_or_else(|| format!("candidate world lacks object {raw_id}"))
}

fn largest_class(world: &World, class: SemanticClass) -> Result<&WorldObject, String> {
    world
        .objects()
        .iter()
        .filter(|object| object.class() == class)
        .max_by_key(|object| (object.radius_mm(), object.id()))
        .ok_or_else(|| format!("candidate world lacks {class:?}"))
}

fn write_landmark_mask(
    output: &Path,
    world: &World,
    style: &StylePack,
    layout: RenderLayout,
    object_id: ObjectId,
    crop: &Crop,
    name: &str,
) -> Result<(), String> {
    let selected = render_selected_region(
        world,
        style,
        layout,
        &[object_id],
        crop.x,
        crop.y,
        crop.width,
        crop.height,
    )
    .map_err(|error| error.to_string())?;
    let excluded = [
        style.ordinary.terrain[0],
        style.ordinary.terrain[1],
        style.ordinary.shadow,
    ];
    let pixels = selected
        .pixels()
        .iter()
        .map(|index| {
            if excluded.contains(index) {
                style.ordinary.terrain[0]
            } else {
                style.ordinary.outline
            }
        })
        .collect();
    let mask = IndexedImage::from_pixels(
        selected.width(),
        selected.height(),
        pixels,
        style.palette.len(),
    )
    .map_err(|error| error.to_string())?;
    write_webp(
        &output.join("masks").join(format!("{name}.webp")),
        &mask,
        style,
    )
}

fn scene_metrics(scene: &Scene, style: &StylePack) -> serde_json::Value {
    let pixels = scene.image.pixels();
    let mut colors = BTreeSet::new();
    colors.extend(pixels.iter().copied());
    let width = usize::try_from(scene.image.width()).expect("bounded width");
    let height = usize::try_from(scene.image.height()).expect("bounded height");
    let mut transitions = 0_u64;
    for y in 0..height {
        for x in 0..width {
            let index = y * width + x;
            if x + 1 < width && pixels[index] != pixels[index + 1] {
                transitions += 1;
            }
            if y + 1 < height && pixels[index] != pixels[index + width] {
                transitions += 1;
            }
        }
    }
    let comparisons = (width - 1) * height + width * (height - 1);
    let foreground = pixels
        .iter()
        .filter(|index| !style.ordinary.terrain.contains(index))
        .count();
    json!({
        "id": scene.id,
        "title": scene.title,
        "file": format!("scenes/{}.webp", scene.id),
        "width": scene.image.width(),
        "height": scene.image.height(),
        "crop": { "x": scene.crop.x, "y": scene.crop.y },
        "used_palette_colors": colors.len(),
        "edge_transition_ppm": transitions * 1_000_000 / u64::try_from(comparisons).expect("comparisons"),
        "foreground_coverage_ppm": foreground * 1_000_000 / pixels.len(),
        "indexed_hash": format!("{:016x}", stable_hash(pixels)),
    })
}

fn contact_sheet(
    scenes: &[Scene],
    background: u8,
    palette_len: usize,
) -> Result<IndexedImage, String> {
    let gap = 24_u32;
    let top_width = scenes[0].image.width() + gap + scenes[1].image.width();
    let bottom_width = scenes[2].image.width() + gap + scenes[3].image.width();
    let width = top_width.max(bottom_width);
    let top_height = scenes[0].image.height().max(scenes[1].image.height());
    let bottom_height = scenes[2].image.height().max(scenes[3].image.height());
    let height = top_height + gap + bottom_height;
    let capacity = usize::try_from(width)
        .ok()
        .and_then(|width| width.checked_mul(usize::try_from(height).ok()?))
        .ok_or_else(|| "contact sheet capacity overflow".to_owned())?;
    let mut pixels = vec![background; capacity];
    blit(&mut pixels, width, &scenes[0].image, 0, 0);
    blit(
        &mut pixels,
        width,
        &scenes[1].image,
        scenes[0].image.width() + gap,
        0,
    );
    blit(&mut pixels, width, &scenes[2].image, 0, top_height + gap);
    blit(
        &mut pixels,
        width,
        &scenes[3].image,
        scenes[2].image.width() + gap,
        top_height + gap,
    );
    IndexedImage::from_pixels(width, height, pixels, palette_len).map_err(|error| error.to_string())
}

fn blit(target: &mut [u8], target_width: u32, source: &IndexedImage, x: u32, y: u32) {
    let target_width = usize::try_from(target_width).expect("bounded target width");
    let source_width = usize::try_from(source.width()).expect("bounded source width");
    for row in 0..usize::try_from(source.height()).expect("bounded source height") {
        let target_start = (usize::try_from(y).expect("bounded y") + row) * target_width
            + usize::try_from(x).expect("bounded x");
        let source_start = row * source_width;
        target[target_start..target_start + source_width]
            .copy_from_slice(&source.pixels()[source_start..source_start + source_width]);
    }
}

fn write_webp(path: &Path, image: &IndexedImage, style: &StylePack) -> Result<(), String> {
    let bytes = encode_lossless_webp(image, &style.palette).map_err(|error| error.to_string())?;
    fs::write(path, bytes).map_err(|error| error.to_string())
}

fn contact_html(scenes: &[Scene], spec: CandidateSpec) -> String {
    let mut figures = String::new();
    for scene in scenes {
        write!(
                &mut figures,
                "<figure><img src=\"scenes/{}.webp\" width=\"{}\" height=\"{}\" alt=\"{}\"><figcaption>{}</figcaption></figure>",
                scene.id,
                scene.image.width(),
                scene.image.height(),
                scene.title,
                scene.title,
            )
            .expect("writing to String cannot fail");
    }
    format!(
        "<!doctype html><html lang=\"en\"><meta charset=\"utf-8\"><meta name=\"viewport\" content=\"width=device-width\"><title>Stanford {}</title><style>body{{margin:0;background:#363230;color:#f5eed3;font:16px system-ui}}main{{padding:24px}}h1{{font-weight:500}}.grid{{display:grid;grid-template-columns:repeat(2,minmax(0,1fr));gap:24px}}figure{{margin:0}}img{{display:block;width:100%;height:auto;image-rendering:pixelated;background:#efe1be}}figcaption{{padding:8px 0 16px}}@media(max-width:720px){{.grid{{grid-template-columns:1fr}}}}</style><main><h1>Isometric Stanford, {}</h1><p>Original deterministic procedural output. Reference imagery is not redistributed.</p><div class=\"grid\">{figures}</div></main></html>",
        spec.label, spec.label
    )
}

fn staging_path(output: &Path) -> Result<std::path::PathBuf, String> {
    let name = output
        .file_name()
        .and_then(|value| value.to_str())
        .ok_or_else(|| "invalid candidate output path".to_owned())?;
    Ok(output.with_file_name(format!("{name}.partial")))
}
