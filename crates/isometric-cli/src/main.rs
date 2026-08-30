//! User-facing orchestration commands.

use std::{
    collections::{BTreeMap, BTreeSet},
    env, fs,
    path::{Path, PathBuf},
    process::ExitCode,
};

use isometric_publish::{DziOptions, InputDigests, sha256_hex, validate_dzi};
use isometric_render::{render_reference, render_world, stable_hash};
use isometric_style::StylePack;
use isometric_validate::{validate_style, validate_world};
use isometric_world::{SemanticClass, World};

mod candidate;

const USAGE: &str = "Usage:
  isometric-stanford source sync [cache-directory]
  isometric-stanford reference inspect [bundle-directory]
  isometric-stanford reference encode-png [raw] [output] [width] [height] [gray8|rgba8]
  isometric-stanford reference crop-png [raw] [output] [source-width] [source-height] [x] [y] [width] [height] [gray8|rgba8]
  isometric-stanford mask inspect [artifact-directory]
  isometric-stanford perceive run [output-directory]
  isometric-stanford world compile [output-directory]
  isometric-stanford world inspect [world.json]
  isometric-stanford render region [output.ppm]
  isometric-stanford render fixture [output.ppm]
  isometric-stanford render slice
  isometric-stanford validate semantic|render
  isometric-stanford validate release [artifact-directory]
  isometric-stanford publish dzi [output-directory] [base|candidate-c]
  isometric-stanford style candidate-a [output-directory]
  isometric-stanford style candidate-b [output-directory]
  isometric-stanford style candidate-c [output-directory]

Implemented commands:
  render region writes the compiled deterministic hero-world PPM.
  render fixture writes the original synthetic regression PPM.
  validate semantic, validate render, and validate release are executable.
  source sync verifies approved artifacts in a content-addressed cache.
  reference inspect validates a registered multipass reference bundle and its
  complete layer hash chain without decoding source imagery into final art.
  mask inspect streams and validates an immutable registered semantic mask,
  its ontology summaries, transient policy, instances, and complete hash chain.
  perceive run compiles locked NAIP and streamed LiDAR into a transient-safe
  semantic evidence artifact; source pixels and point records are not retained.
  world compile verifies the complete source lock, compiles the locked vectors,
  and writes a canonical world plus manifest. world inspect validates an artifact.
  publish dzi writes a staged, lossless WebP DZI and indexed canonical pyramid.
  The optional style selector is explicit so Candidate C can be inspected without
  implying approval of the locked base style.
  style candidate-a writes four native review scenes, masks, metrics, and a contact sheet.
  style candidate-b writes the bounded second procedural style iteration.
  style candidate-c writes the final bounded procedural style iteration.
  Other commands fail closed until their tracked task is implemented.";

fn main() -> ExitCode {
    let arguments = env::args().skip(1).collect::<Vec<_>>();
    match run(&arguments) {
        Ok(message) => {
            println!("{message}");
            ExitCode::SUCCESS
        }
        Err(message) => {
            eprintln!("error: {message}\n\n{USAGE}");
            ExitCode::FAILURE
        }
    }
}

fn run(arguments: &[String]) -> Result<String, String> {
    match arguments {
        [group, rest @ ..] if group == "reference" => run_reference(rest),
        [group, command] if group == "source" && command == "sync" => {
            sync_sources(Path::new("artifacts/source-cache"))
        }
        [group, command, cache] if group == "source" && command == "sync" => {
            sync_sources(&PathBuf::from(cache))
        }
        [group, command] if command == "inspect" && group == "mask" => {
            inspect_artifact(group, None)
        }
        [group, command, input] if command == "inspect" && group == "mask" => {
            inspect_artifact(group, Some(input))
        }
        [group, command] if group == "perceive" && command == "run" => {
            compile_perception(Path::new("artifacts/perception"))
        }
        [group, command, output] if group == "perceive" && command == "run" => {
            compile_perception(Path::new(output))
        }
        [group, command] if group == "world" && command == "compile" => {
            compile_world(Path::new("artifacts/world"))
        }
        [group, command, output] if group == "world" && command == "compile" => {
            compile_world(Path::new(output))
        }
        [group, command] if group == "world" && command == "inspect" => {
            inspect_world(Path::new("artifacts/world/hero.json"))
        }
        [group, command, input] if group == "world" && command == "inspect" => {
            inspect_world(Path::new(input))
        }
        [group, command] if group == "validate" && command == "semantic" => {
            validate_world(&World::reference_fixture()).map_err(|error| error.to_string())?;
            Ok("semantic fixture passed".into())
        }
        [group, command] if group == "validate" && command == "render" => {
            let world = World::reference_fixture();
            let style = StylePack::stanford_v1();
            validate_world(&world).map_err(|error| error.to_string())?;
            validate_style(&style).map_err(|error| error.to_string())?;
            let image =
                render_reference(&world, &style, 128, 128).map_err(|error| error.to_string())?;
            Ok(format!(
                "reference render passed: {:016x}",
                stable_hash(image.pixels())
            ))
        }
        [group, command] if group == "validate" && command == "release" => {
            validate_release(Path::new("artifacts/dzi/hero"))
        }
        [group, command, input] if group == "validate" && command == "release" => {
            validate_release(Path::new(input))
        }
        [group, command] if group == "render" && command == "region" => {
            render_region("artifacts/render/hero.ppm")
        }
        [group, command, output] if group == "render" && command == "region" => {
            render_region(output)
        }
        [group, command] if group == "render" && command == "fixture" => {
            render_fixture("artifacts/reference.ppm")
        }
        [group, command, output] if group == "render" && command == "fixture" => {
            render_fixture(output)
        }
        [group, command] if group == "publish" && command == "dzi" => {
            publish_dzi_artifact(Path::new("artifacts/dzi/hero"))
        }
        [group, command, output] if group == "publish" && command == "dzi" => {
            publish_dzi_artifact(Path::new(output))
        }
        [group, command, output, style] if group == "publish" && command == "dzi" => {
            publish_dzi_artifact_with_style(Path::new(output), style)
        }
        [group, command] if group == "style" => write_style(command, None),
        [group, command, output] if group == "style" => write_style(command, Some(output)),
        [] => Ok(USAGE.into()),
        [single] if single == "--help" || single == "-h" => Ok(USAGE.into()),
        [group, command] => Err(format!(
            "{group} {command} is specified but not implemented yet"
        )),
        _ => Err("unrecognized command".into()),
    }
}

fn run_reference(arguments: &[String]) -> Result<String, String> {
    match arguments {
        [command] if command == "inspect" => inspect_artifact("reference", None),
        [command, input] if command == "inspect" => inspect_artifact("reference", Some(input)),
        [command, raw, output, width, height, color] if command == "encode-png" => {
            encode_reference_png(raw, output, width, height, color)
        }
        [
            command,
            raw,
            output,
            source_width,
            source_height,
            x,
            y,
            width,
            height,
            color,
        ] if command == "crop-png" => crop_reference_png(
            raw,
            output,
            source_width,
            source_height,
            x,
            y,
            width,
            height,
            color,
        ),
        [command, ..] => Err(format!(
            "reference {command} is specified but not implemented yet"
        )),
        [] => Err("reference command is missing".into()),
    }
}

fn encode_reference_png(
    raw: &str,
    output: &str,
    width: &str,
    height: &str,
    color: &str,
) -> Result<String, String> {
    let width = width
        .parse::<u32>()
        .map_err(|_| "reference PNG width is not a u32".to_string())?;
    let height = height
        .parse::<u32>()
        .map_err(|_| "reference PNG height is not a u32".to_string())?;
    let color_type = match color {
        "gray8" => isometric_reference::PngColorType::Grayscale,
        "rgba8" => isometric_reference::PngColorType::Rgba,
        _ => return Err("reference PNG color type must be gray8 or rgba8".into()),
    };
    let bytes = isometric_reference::encode_raw_png(
        Path::new(raw),
        Path::new(output),
        width,
        height,
        color_type,
    )
    .map_err(|error| error.to_string())?;
    Ok(format!("encoded bounded reference PNG: {bytes} bytes"))
}

#[allow(clippy::too_many_arguments)]
fn crop_reference_png(
    raw: &str,
    output: &str,
    source_width: &str,
    source_height: &str,
    x: &str,
    y: &str,
    width: &str,
    height: &str,
    color: &str,
) -> Result<String, String> {
    let parse = |name: &str, value: &str| {
        value
            .parse::<u32>()
            .map_err(|_| format!("reference PNG {name} is not a u32"))
    };
    let color_type = match color {
        "gray8" => isometric_reference::PngColorType::Grayscale,
        "rgba8" => isometric_reference::PngColorType::Rgba,
        _ => return Err("reference PNG color type must be gray8 or rgba8".into()),
    };
    let bytes = isometric_reference::encode_raw_png_crop(
        Path::new(raw),
        Path::new(output),
        parse("source width", source_width)?,
        parse("source height", source_height)?,
        parse("crop x", x)?,
        parse("crop y", y)?,
        parse("crop width", width)?,
        parse("crop height", height)?,
        color_type,
    )
    .map_err(|error| error.to_string())?;
    Ok(format!("encoded bounded reference PNG crop: {bytes} bytes"))
}

fn inspect_artifact(group: &str, input: Option<&String>) -> Result<String, String> {
    match group {
        "reference" => inspect_reference(Path::new(
            input.map_or("artifacts/reference/hoover", String::as_str),
        )),
        "mask" => inspect_mask(Path::new(
            input.map_or("artifacts/masks/hoover", String::as_str),
        )),
        _ => Err(format!("unknown inspectable artifact {group}")),
    }
}

fn write_style(command: &str, output: Option<&String>) -> Result<String, String> {
    let default = format!("artifacts/style/{command}");
    let output = Path::new(output.map_or(default.as_str(), String::as_str));
    match command {
        "candidate-a" => write_style_candidate(output),
        "candidate-b" => write_style_candidate_b(output),
        "candidate-c" => write_style_candidate_c(output),
        _ => Err(format!(
            "style {command} is specified but not implemented yet"
        )),
    }
}

fn sync_sources(cache: &Path) -> Result<String, String> {
    let artifacts = isometric_source::sync(Path::new("source.lock.json"), cache)
        .map_err(|error| error.to_string())?;
    Ok(sync_report(cache, &artifacts))
}

fn inspect_reference(root: &Path) -> Result<String, String> {
    let manifest_path = root.join(isometric_reference::MANIFEST_FILENAME);
    let manifest =
        isometric_reference::read_manifest(&manifest_path).map_err(|error| error.to_string())?;
    let report =
        isometric_reference::validate_bundle(root, &manifest).map_err(|error| error.to_string())?;
    Ok(format!(
        "reference bundle {} passed: {} layers, {} bytes, manifest {}",
        manifest.bundle_id,
        report.layer_sha256.len(),
        report.total_layer_bytes,
        report.manifest_sha256
    ))
}

fn inspect_mask(root: &Path) -> Result<String, String> {
    let manifest_path = root.join(isometric_mask::MANIFEST_FILENAME);
    let manifest =
        isometric_mask::read_manifest(&manifest_path).map_err(|error| error.to_string())?;
    let report =
        isometric_mask::validate_artifact(root, &manifest).map_err(|error| error.to_string())?;
    Ok(format!(
        "mask artifact {} ({}) passed: {} pixels, {} instances, {} unknown, {} transient, manifest {}",
        manifest.artifact_id,
        manifest.role.as_str(),
        report.pixel_count,
        report.instance_count,
        report.unknown_pixels,
        report.transient_pixels,
        report.manifest_sha256
    ))
}

fn sync_report(cache: &Path, artifacts: &[isometric_source::SyncedArtifact]) -> String {
    let downloaded = artifacts.iter().filter(|artifact| !artifact.reused).count();
    let details = artifacts
        .iter()
        .map(|artifact| {
            if artifact.reused {
                format!("source {}: verified cache hit", artifact.id)
            } else {
                format!(
                    "source {}: downloaded in {} attempt(s)",
                    artifact.id, artifact.attempts
                )
            }
        })
        .collect::<Vec<_>>()
        .join("\n");
    format!(
        "verified {} sources in {} ({downloaded} downloaded)\n{details}",
        artifacts.len(),
        cache.display()
    )
}

fn compile_perception(output: &Path) -> Result<String, String> {
    const PERCEPTION_SOURCES: [&str; 5] = [
        "naip-2024-hero",
        "usgs-lidar-07509800",
        "usgs-lidar-07509825",
        "usgs-lidar-07759800",
        "usgs-lidar-07759825",
    ];
    const VECTOR_SOURCES: [&str; 2] = ["osm-2026-07-15-hero", "overture-2026-06-17-buildings"];
    let selected = PERCEPTION_SOURCES
        .iter()
        .chain(VECTOR_SOURCES.iter())
        .copied()
        .collect::<Vec<_>>();
    let artifacts = isometric_source::sync_selected(
        Path::new("source.lock.json"),
        Path::new("artifacts/source-cache"),
        &selected,
    )
    .map_err(|error| error.to_string())?;
    let path_for = |id: &str| {
        artifacts
            .iter()
            .find(|artifact| artifact.id == id)
            .map(|artifact| artifact.path.clone())
            .ok_or_else(|| format!("verified source cache lacks {id}"))
    };
    let osm = fs::read(path_for(VECTOR_SOURCES[0])?).map_err(|error| error.to_string())?;
    let overture = fs::read(path_for(VECTOR_SOURCES[1])?).map_err(|error| error.to_string())?;
    let vector_world = isometric_world::compile_hero(&osm, &overture)
        .map_err(|error| error.to_string())?
        .world;
    let eligible_cells = vector_world
        .objects()
        .iter()
        .filter(|object| object.class() == SemanticClass::Unknown)
        .map(|object| {
            let anchor = object.anchor();
            let column = (anchor.x_mm - isometric_perception::GRID_MIN_X_MM)
                .div_euclid(isometric_perception::CELL_SIZE_MM);
            let row = (anchor.y_mm - isometric_perception::GRID_MIN_Y_MM)
                .div_euclid(isometric_perception::CELL_SIZE_MM);
            isometric_perception::CellIndex::new(
                u16::try_from(column).map_err(|_| "unknown cell column is negative")?,
                u16::try_from(row).map_err(|_| "unknown cell row is negative")?,
            )
            .map_err(|error| error.to_string())
        })
        .collect::<Result<BTreeSet<_>, _>>()?;
    let source_lock: serde_json::Value =
        serde_json::from_slice(&fs::read("source.lock.json").map_err(|error| error.to_string())?)
            .map_err(|error| error.to_string())?;
    let source_hashes = source_lock
        .get("sources")
        .and_then(serde_json::Value::as_array)
        .ok_or_else(|| "source lock lacks sources".to_string())?
        .iter()
        .filter_map(|source| {
            let id = source.get("id")?.as_str()?;
            PERCEPTION_SOURCES.contains(&id).then(|| {
                source
                    .get("sha256")
                    .and_then(serde_json::Value::as_str)
                    .map(|digest| (id.to_owned(), digest.to_owned()))
            })?
        })
        .collect::<BTreeMap<_, _>>();
    let lidar = PERCEPTION_SOURCES[1..]
        .iter()
        .map(|id| {
            Ok(isometric_perception::LidarInput {
                source_id: (*id).to_owned(),
                path: path_for(id)?,
            })
        })
        .collect::<Result<Vec<_>, String>>()?;
    let compiled = isometric_perception::compile(&isometric_perception::CompileInput {
        naip_path: path_for(PERCEPTION_SOURCES[0])?,
        lidar,
        source_sha256: source_hashes,
        eligible_cells,
    })
    .map_err(|error| error.to_string())?;
    fs::create_dir_all(output).map_err(|error| error.to_string())?;
    atomic_write(
        &output.join("hero-evidence.json"),
        compiled.artifact_json.as_bytes(),
    )?;
    Ok(format!(
        "compiled {} transient-safe evidence cells from {} NAIP samples and {} LiDAR points at {}: {}",
        compiled.artifact.evidence_cell_count,
        compiled.artifact.naip_sample_count,
        compiled.artifact.lidar_sample_count,
        output.display(),
        compiled.artifact_sha256
    ))
}

fn compile_world(output: &Path) -> Result<String, String> {
    let artifacts = isometric_source::sync_selected(
        Path::new("source.lock.json"),
        Path::new("artifacts/source-cache"),
        &["osm-2026-07-15-hero", "overture-2026-06-17-buildings"],
    )
    .map_err(|error| error.to_string())?;
    let path_for = |id: &str| {
        artifacts
            .iter()
            .find(|artifact| artifact.id == id)
            .map(|artifact| artifact.path.as_path())
            .ok_or_else(|| format!("verified source cache lacks {id}"))
    };
    let osm = fs::read(path_for("osm-2026-07-15-hero")?).map_err(|error| error.to_string())?;
    let overture =
        fs::read(path_for("overture-2026-06-17-buildings")?).map_err(|error| error.to_string())?;
    let evidence = load_locked_perception()?;
    let compiled = isometric_world::compile_hero_with_evidence(&osm, &overture, &evidence)
        .map_err(|error| error.to_string())?;
    fs::create_dir_all(output).map_err(|error| error.to_string())?;
    atomic_write(&output.join("hero.json"), compiled.world_json.as_bytes())?;
    atomic_write(
        &output.join("world.manifest.json"),
        compiled.manifest_json.as_bytes(),
    )?;
    Ok(format!(
        "compiled {} objects into {} partitions at {} (unknown {} ppm; rejected {} source geometries)",
        compiled.report.object_count,
        compiled.world.partitions().len(),
        output.display(),
        compiled.report.unknown_fraction_ppm,
        compiled.report.rejected_geometry_count
    ))
}

fn load_locked_perception() -> Result<Vec<u8>, String> {
    let lock: serde_json::Value = serde_json::from_slice(
        &fs::read("perception.lock.json").map_err(|error| error.to_string())?,
    )
    .map_err(|error| error.to_string())?;
    if lock.get("status").and_then(serde_json::Value::as_str) != Some("compiled-prototype") {
        return Err("perception lock is not compiled for the prototype".into());
    }
    let artifact = lock
        .get("artifacts")
        .and_then(serde_json::Value::as_array)
        .and_then(|artifacts| artifacts.first())
        .ok_or_else(|| "perception lock lacks its frozen artifact".to_string())?;
    let path = artifact
        .get("path")
        .and_then(serde_json::Value::as_str)
        .ok_or_else(|| "perception artifact path is invalid".to_string())?;
    let expected = artifact
        .get("sha256")
        .and_then(serde_json::Value::as_str)
        .ok_or_else(|| "perception artifact hash is invalid".to_string())?;
    let bytes = fs::read(path).map_err(|error| error.to_string())?;
    if sha256_hex(&bytes) != expected {
        return Err("perception artifact does not match its lock".into());
    }
    Ok(bytes)
}

fn inspect_world(input: &Path) -> Result<String, String> {
    let json = fs::read_to_string(input).map_err(|error| error.to_string())?;
    let world = World::from_artifact_json(&json).map_err(|error| error.to_string())?;
    let named = world
        .objects()
        .iter()
        .filter(|object| object.name().is_some())
        .count();
    Ok(format!(
        "validated {} objects ({} named) across {} partitions from {} sources",
        world.objects().len(),
        named,
        world.partitions().len(),
        world.sources().len()
    ))
}

fn atomic_write(path: &Path, bytes: &[u8]) -> Result<(), String> {
    let extension = path
        .extension()
        .and_then(|value| value.to_str())
        .unwrap_or("artifact");
    let temporary = path.with_extension(format!("{extension}.partial"));
    fs::write(&temporary, bytes).map_err(|error| error.to_string())?;
    fs::rename(&temporary, path).map_err(|error| error.to_string())
}

fn render_region(output: &str) -> Result<String, String> {
    let json = fs::read_to_string("artifacts/world/hero.json").map_err(|error| {
        format!("read artifacts/world/hero.json after `world compile`: {error}")
    })?;
    let world = World::from_artifact_json(&json).map_err(|error| error.to_string())?;
    let style = StylePack::stanford_v1();
    let image = render_world(&world, &style).map_err(|error| error.to_string())?;
    write_ppm(output, &image, &style)
}

fn render_fixture(output: &str) -> Result<String, String> {
    let world = World::reference_fixture();
    let style = StylePack::stanford_v1();
    let image = render_reference(&world, &style, 256, 256).map_err(|error| error.to_string())?;
    write_ppm(output, &image, &style)
}

fn publish_dzi_artifact(output: &Path) -> Result<String, String> {
    publish_dzi_artifact_with_style(output, "base")
}

fn publication_style(selection: &str) -> Result<(StylePack, &'static Path), String> {
    match selection {
        "base" => Ok((
            StylePack::stanford_v1(),
            Path::new("styles/stanford_v1/style.toml"),
        )),
        "candidate-c" => Ok((
            StylePack::stanford_v1_candidate_c(),
            Path::new("styles/stanford_v1/candidate_c.toml"),
        )),
        _ => Err(format!(
            "unknown DZI style {selection:?}; expected base or candidate-c"
        )),
    }
}

fn style_by_id(style_id: &str) -> Result<StylePack, String> {
    match style_id {
        "stanford_v1.landmarks.1" => Ok(StylePack::stanford_v1()),
        "stanford_v1.candidate_c.1" => Ok(StylePack::stanford_v1_candidate_c()),
        _ => Err(format!("release artifact names unknown style {style_id:?}")),
    }
}

fn publish_dzi_artifact_with_style(output: &Path, style_selection: &str) -> Result<String, String> {
    let world_bytes = fs::read("artifacts/world/hero.json")
        .map_err(|error| format!("read artifacts/world/hero.json after world compile: {error}"))?;
    let world_json = std::str::from_utf8(&world_bytes).map_err(|error| error.to_string())?;
    let world = World::from_artifact_json(world_json).map_err(|error| error.to_string())?;
    let (style, style_path) = publication_style(style_selection)?;
    let style_bytes = fs::read(style_path).map_err(|error| error.to_string())?;
    let inputs = InputDigests::new(sha256_hex(&world_bytes), sha256_hex(&style_bytes))
        .map_err(|error| error.to_string())?;
    let report =
        isometric_publish::publish_dzi(&world, &style, &inputs, output, DziOptions::prototype())
            .map_err(|error| error.to_string())?;
    Ok(format!(
        "published {} x {} {} DZI with {} tiles and {} encoded bytes at {}: {}",
        report.width,
        report.height,
        style.id,
        report.tile_count,
        report.encoded_bytes,
        output.display(),
        report.tile_set_sha256
    ))
}

fn validate_release(input: &Path) -> Result<String, String> {
    let manifest_path = input.join("release.json");
    let metadata = fs::metadata(&manifest_path).map_err(|error| error.to_string())?;
    if metadata.len() > 8 * 1_024 * 1_024 {
        return Err("release manifest exceeds the 8 MiB CLI read limit".into());
    }
    let manifest_bytes = fs::read(&manifest_path).map_err(|error| error.to_string())?;
    let manifest: serde_json::Value =
        serde_json::from_slice(&manifest_bytes).map_err(|error| error.to_string())?;
    let style_id = manifest
        .get("style_id")
        .and_then(serde_json::Value::as_str)
        .ok_or_else(|| "release artifact does not identify its style".to_string())?;
    let style = style_by_id(style_id)?;
    let report = validate_dzi(input, &style).map_err(|error| error.to_string())?;
    Ok(format!(
        "validated {} x {} {} DZI with {} tiles and {} encoded bytes at {}: {}",
        report.width,
        report.height,
        style.id,
        report.tile_count,
        report.encoded_bytes,
        input.display(),
        report.tile_set_sha256
    ))
}

fn write_style_candidate(output: &Path) -> Result<String, String> {
    let world_bytes = fs::read("artifacts/world/hero.json")
        .map_err(|error| format!("read artifacts/world/hero.json after world compile: {error}"))?;
    let world_json = std::str::from_utf8(&world_bytes).map_err(|error| error.to_string())?;
    let world = World::from_artifact_json(world_json).map_err(|error| error.to_string())?;
    let style_bytes =
        fs::read("styles/stanford_v1/style.toml").map_err(|error| error.to_string())?;
    candidate::write_candidate_a(
        &world,
        &StylePack::stanford_v1(),
        &sha256_hex(&world_bytes),
        &sha256_hex(&style_bytes),
        output,
    )
}

fn write_style_candidate_b(output: &Path) -> Result<String, String> {
    let world_bytes = fs::read("artifacts/world/hero.json")
        .map_err(|error| format!("read artifacts/world/hero.json after world compile: {error}"))?;
    let world_json = std::str::from_utf8(&world_bytes).map_err(|error| error.to_string())?;
    let world = World::from_artifact_json(world_json).map_err(|error| error.to_string())?;
    let style_bytes =
        fs::read("styles/stanford_v1/candidate_b.toml").map_err(|error| error.to_string())?;
    candidate::write_candidate_b(
        &world,
        &StylePack::stanford_v1_candidate_b(),
        &sha256_hex(&world_bytes),
        &sha256_hex(&style_bytes),
        output,
    )
}

fn write_style_candidate_c(output: &Path) -> Result<String, String> {
    let world_bytes = fs::read("artifacts/world/hero.json")
        .map_err(|error| format!("read artifacts/world/hero.json after world compile: {error}"))?;
    let world_json = std::str::from_utf8(&world_bytes).map_err(|error| error.to_string())?;
    let world = World::from_artifact_json(world_json).map_err(|error| error.to_string())?;
    let style_bytes =
        fs::read("styles/stanford_v1/candidate_c.toml").map_err(|error| error.to_string())?;
    candidate::write_candidate_c(
        &world,
        &StylePack::stanford_v1_candidate_c(),
        &sha256_hex(&world_bytes),
        &sha256_hex(&style_bytes),
        output,
    )
}

fn write_ppm(
    output: &str,
    image: &isometric_render::IndexedImage,
    style: &StylePack,
) -> Result<String, String> {
    let bytes = image
        .to_ppm(&style.palette)
        .map_err(|error| error.to_string())?;
    if let Some(parent) = std::path::Path::new(output).parent() {
        fs::create_dir_all(parent).map_err(|error| error.to_string())?;
    }
    fs::write(output, bytes).map_err(|error| error.to_string())?;
    Ok(format!(
        "wrote {output} ({} x {}): {:016x}",
        image.width(),
        image.height(),
        stable_hash(image.pixels())
    ))
}

#[cfg(test)]
mod tests {
    use super::{
        inspect_mask, inspect_reference, publication_style, run, style_by_id, sync_report,
    };
    use isometric_mask::{
        ArtifactDescriptor, ArtifactRole, EvidenceFlags, MaskPixel, ProducerIdentity,
        ReferenceRegistration, SemanticClass, write_artifact,
    };
    use isometric_source::SyncedArtifact;
    use std::path::{Path, PathBuf};

    #[test]
    fn semantic_validation_command_runs() {
        let arguments = vec!["validate".into(), "semantic".into()];
        let result = run(&arguments);
        assert_eq!(
            result.expect("command must pass"),
            "semantic fixture passed"
        );
    }

    #[test]
    fn unimplemented_contract_fails_closed() {
        let arguments = vec!["render".into(), "slice".into()];
        let result = run(&arguments);
        assert!(result.expect_err("must fail").contains("not implemented"));
    }

    #[test]
    fn reference_inspection_fails_closed_without_a_manifest() {
        let missing = std::env::temp_dir().join(format!(
            "isometric-reference-missing-{}",
            std::process::id()
        ));
        assert!(inspect_reference(&missing).is_err());
    }

    #[test]
    fn mask_inspection_validates_an_immutable_synthetic_artifact() {
        let root = std::env::temp_dir().join(format!(
            "isometric-cli-mask-{}-{}",
            std::process::id(),
            std::thread::current().name().unwrap_or("unnamed")
        ));
        let _ = std::fs::remove_dir_all(&root);
        let descriptor = ArtifactDescriptor {
            artifact_id: "cli-synthetic-mask".into(),
            role: ArtifactRole::Evidence,
            reference: ReferenceRegistration {
                bundle_id: "cli-synthetic-reference".into(),
                manifest_sha256: "1".repeat(64),
                region_id: "cli-synthetic-region".into(),
                width_px: 2,
                height_px: 2,
                grid_sha256: "2".repeat(64),
            },
            producer: ProducerIdentity {
                name: "isometric-cli-test".into(),
                version: "fixture-v1".into(),
            },
        };
        let pixels = vec![
            MaskPixel {
                class: SemanticClass::BuildingRoof,
                confidence: 250,
                instance_id: 1,
                evidence: EvidenceFlags::GEOMETRY,
            },
            MaskPixel {
                class: SemanticClass::Road,
                confidence: 245,
                instance_id: 2,
                evidence: EvidenceFlags::GEOGRAPHIC_PRIOR,
            },
            MaskPixel {
                class: SemanticClass::Car,
                confidence: 230,
                instance_id: 3,
                evidence: EvidenceFlags::DETECTOR,
            },
            MaskPixel {
                class: SemanticClass::Unknown,
                confidence: 0,
                instance_id: 0,
                evidence: EvidenceFlags::NONE,
            },
        ];
        write_artifact(&root, descriptor, pixels).expect("write CLI mask fixture");

        let report = inspect_mask(&root).expect("inspect CLI mask fixture");
        assert!(report.contains("cli-synthetic-mask (evidence) passed"));
        assert!(report.contains("4 pixels, 3 instances, 1 unknown, 1 transient"));
    }

    #[test]
    fn mask_inspection_fails_closed_without_a_manifest() {
        let missing =
            std::env::temp_dir().join(format!("isometric-mask-missing-{}", std::process::id()));
        assert!(inspect_mask(&missing).is_err());
    }

    #[test]
    fn publication_style_selection_is_explicit_and_fail_closed() {
        assert_eq!(
            publication_style("base").expect("base").0.id,
            "stanford_v1.landmarks.1"
        );
        assert_eq!(
            publication_style("candidate-c").expect("Candidate C").0.id,
            "stanford_v1.candidate_c.1"
        );
        assert!(publication_style("candidate-b").is_err());
        assert_eq!(
            style_by_id("stanford_v1.candidate_c.1")
                .expect("known style")
                .id,
            "stanford_v1.candidate_c.1"
        );
        assert!(style_by_id("stanford_v1.unknown").is_err());
    }

    #[test]
    fn source_report_names_attempts_without_urls() {
        let report = sync_report(
            Path::new("artifacts/source-cache"),
            &[
                SyncedArtifact {
                    id: "naip".into(),
                    path: PathBuf::from("cache/naip"),
                    reused: false,
                    attempts: 2,
                },
                SyncedArtifact {
                    id: "lidar".into(),
                    path: PathBuf::from("cache/lidar"),
                    reused: true,
                    attempts: 0,
                },
            ],
        );

        assert!(report.contains("source naip: downloaded in 2 attempt(s)"));
        assert!(report.contains("source lidar: verified cache hit"));
        assert!(!report.contains("https://"));
    }
}
