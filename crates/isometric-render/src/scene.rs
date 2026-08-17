//! Conversion from canonical semantic polygons to ordinary campus art faces.

use std::collections::BTreeSet;

use isometric_core::{ObjectId, WorldPoint};
use isometric_style::StylePack;
use isometric_world::{Geometry, Polygon, SemanticClass, World, WorldObject};

use crate::{IndexedImage, RasterSurface, RasterVertex, RenderError, Triangle, project};

const VIEW_MARGIN_PIXELS: i64 = 32;
const PASS_GROUND: u8 = 1;
const PASS_WALL: u8 = 2;
const PASS_ROOF: u8 = 3;

/// Renders ordinary semantic geometry into a tightly bounded indexed image.
///
/// This stage covers polygonal ground, hardscape, flat building roofs, and
/// directional building walls. Vegetation crowns, detailed roofs, shadows,
/// outlines, dithering, and landmarks remain later procedural passes.
///
/// # Errors
///
/// Returns an error for invalid style, empty geometry, projection or capacity
/// overflow, noncanonical polygons, or raster failures.
pub fn render_world(world: &World, style: &StylePack) -> Result<IndexedImage, RenderError> {
    style.validate().map_err(|_| RenderError::InvalidStyle)?;
    let view = Viewport::from_world(world, style)?;
    let mut triangles = Vec::new();
    for object in world.objects() {
        append_object(&mut triangles, object, style, view)?;
    }
    let palette_len = u8::try_from(style.palette.len()).map_err(|_| RenderError::PaletteIndex)?;
    let mut surface = RasterSurface::new(view.width, view.height, 0, palette_len)?;
    surface.rasterize(&triangles)?;
    Ok(surface.into_image())
}

#[derive(Clone, Copy)]
struct Viewport {
    offset_x_subpx: i64,
    offset_y_subpx: i64,
    width: u32,
    height: u32,
}

impl Viewport {
    fn from_world(world: &World, style: &StylePack) -> Result<Self, RenderError> {
        let mut min_x = i64::MAX;
        let mut min_y = i64::MAX;
        let mut max_x = i64::MIN;
        let mut max_y = i64::MIN;
        for object in world.objects() {
            for point in geometry_points(object.geometry()) {
                for z_offset in [0_i64, i64::from(object.height_mm())] {
                    let projected = project(
                        WorldPoint::new(
                            point.x_mm,
                            point.y_mm,
                            point
                                .z_mm
                                .checked_add(z_offset)
                                .ok_or(RenderError::ArithmeticOverflow)?,
                        ),
                        style,
                    )?;
                    min_x = min_x.min(projected.x_subpx);
                    min_y = min_y.min(projected.y_subpx);
                    max_x = max_x.max(projected.x_subpx);
                    max_y = max_y.max(projected.y_subpx);
                }
            }
        }
        if min_x == i64::MAX {
            return Err(RenderError::EmptyWorld);
        }
        let margin = VIEW_MARGIN_PIXELS
            .checked_mul(style.subpixels_per_pixel)
            .ok_or(RenderError::ArithmeticOverflow)?;
        let span_x = max_x
            .checked_sub(min_x)
            .and_then(|value| value.checked_add(margin * 2))
            .ok_or(RenderError::ArithmeticOverflow)?;
        let span_y = max_y
            .checked_sub(min_y)
            .and_then(|value| value.checked_add(margin * 2))
            .ok_or(RenderError::ArithmeticOverflow)?;
        let width = pixel_span(span_x, style.subpixels_per_pixel)?;
        let height = pixel_span(span_y, style.subpixels_per_pixel)?;
        Ok(Self {
            offset_x_subpx: margin
                .checked_sub(min_x)
                .ok_or(RenderError::ArithmeticOverflow)?,
            offset_y_subpx: margin
                .checked_sub(min_y)
                .ok_or(RenderError::ArithmeticOverflow)?,
            width,
            height,
        })
    }
}

fn pixel_span(span_subpx: i64, subpixels_per_pixel: i64) -> Result<u32, RenderError> {
    let rounded = span_subpx
        .checked_add(subpixels_per_pixel - 1)
        .ok_or(RenderError::ArithmeticOverflow)?
        / subpixels_per_pixel;
    u32::try_from(rounded).map_err(|_| RenderError::InvalidDimensions)
}

fn append_object(
    output: &mut Vec<Triangle>,
    object: &WorldObject,
    style: &StylePack,
    view: Viewport,
) -> Result<(), RenderError> {
    let polygons: &[Polygon] = match object.geometry() {
        Geometry::Polygon(polygon) => std::slice::from_ref(polygon),
        Geometry::MultiPolygon(polygons) => polygons,
    };
    let mut ordinal = 0_u32;
    for polygon in polygons {
        let base_rings = scene_rings(polygon, 0, object.class(), style, view)?;
        append_polygon_fill(
            output,
            &base_rings,
            ground_color(object.class()),
            object.id(),
            PASS_GROUND,
            &mut ordinal,
        )?;
        if object.class() != SemanticClass::Building || object.height_mm() == 0 {
            continue;
        }
        append_walls(output, polygon, object, style, view, &mut ordinal)?;
        let roof_rings = scene_rings(
            polygon,
            i64::from(object.height_mm()),
            SemanticClass::Building,
            style,
            view,
        )?;
        append_polygon_fill(output, &roof_rings, 5, object.id(), PASS_ROOF, &mut ordinal)?;
    }
    Ok(())
}

fn append_walls(
    output: &mut Vec<Triangle>,
    polygon: &Polygon,
    object: &WorldObject,
    style: &StylePack,
    view: Viewport,
    ordinal: &mut u32,
) -> Result<(), RenderError> {
    for ring in polygon.rings() {
        for edge in ring.points().windows(2) {
            let bottom_left = scene_vertex(edge[0], 0, SemanticClass::Building, style, view)?;
            let bottom_right = scene_vertex(edge[1], 0, SemanticClass::Building, style, view)?;
            let top_left = scene_vertex(
                edge[0],
                i64::from(object.height_mm()),
                SemanticClass::Building,
                style,
                view,
            )?;
            let top_right = scene_vertex(
                edge[1],
                i64::from(object.height_mm()),
                SemanticClass::Building,
                style,
                view,
            )?;
            let color = if edge[1].x_mm - edge[0].x_mm >= edge[1].y_mm - edge[0].y_mm {
                7
            } else {
                6
            };
            push_triangle(
                output,
                [bottom_left, bottom_right, top_right],
                color,
                object.id(),
                PASS_WALL,
                ordinal,
            );
            push_triangle(
                output,
                [bottom_left, top_right, top_left],
                color,
                object.id(),
                PASS_WALL,
                ordinal,
            );
        }
    }
    Ok(())
}

fn scene_rings(
    polygon: &Polygon,
    z_offset_mm: i64,
    class: SemanticClass,
    style: &StylePack,
    view: Viewport,
) -> Result<Vec<Vec<RasterVertex>>, RenderError> {
    polygon
        .rings()
        .iter()
        .map(|ring| {
            ring.points()
                .iter()
                .map(|point| scene_vertex(*point, z_offset_mm, class, style, view))
                .collect()
        })
        .collect()
}

fn scene_vertex(
    point: WorldPoint,
    z_offset_mm: i64,
    class: SemanticClass,
    style: &StylePack,
    view: Viewport,
) -> Result<RasterVertex, RenderError> {
    let z_mm = point
        .z_mm
        .checked_add(z_offset_mm)
        .ok_or(RenderError::ArithmeticOverflow)?;
    let projected = project(WorldPoint::new(point.x_mm, point.y_mm, z_mm), style)?;
    let raw_depth = point
        .x_mm
        .checked_add(point.y_mm)
        .and_then(|value| value.checked_add(z_mm))
        .and_then(|value| value.checked_mul(16))
        .and_then(|value| value.checked_add(i64::from(depth_layer(class))))
        .ok_or(RenderError::ArithmeticOverflow)?;
    Ok(RasterVertex::new(
        projected
            .x_subpx
            .checked_add(view.offset_x_subpx)
            .ok_or(RenderError::ArithmeticOverflow)?,
        projected
            .y_subpx
            .checked_add(view.offset_y_subpx)
            .ok_or(RenderError::ArithmeticOverflow)?,
        i32::try_from(raw_depth).map_err(|_| RenderError::ArithmeticOverflow)?,
    ))
}

fn append_polygon_fill(
    output: &mut Vec<Triangle>,
    rings: &[Vec<RasterVertex>],
    palette_index: u8,
    object_id: ObjectId,
    pass: u8,
    ordinal: &mut u32,
) -> Result<(), RenderError> {
    let y_levels = rings
        .iter()
        .flat_map(|ring| ring.iter().map(|point| point.y_subpx))
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect::<Vec<_>>();
    for strip in y_levels.windows(2) {
        let (low_y, high_y) = (strip[0], strip[1]);
        if low_y == high_y {
            continue;
        }
        let mut crossings = rings
            .iter()
            .flat_map(|ring| ring.windows(2))
            .enumerate()
            .filter_map(|(index, edge)| {
                let edge_low = edge[0].y_subpx.min(edge[1].y_subpx);
                let edge_high = edge[0].y_subpx.max(edge[1].y_subpx);
                (edge_low <= low_y && edge_high >= high_y && edge_low != edge_high)
                    .then_some((index, edge))
            })
            .map(|(index, edge)| {
                Ok((
                    interpolate_at_y(edge[0], edge[1], low_y)?,
                    interpolate_at_y(edge[0], edge[1], high_y)?,
                    index,
                ))
            })
            .collect::<Result<Vec<_>, RenderError>>()?;
        crossings.sort_by_key(|(low, high, index)| {
            (i128::from(low.x_subpx) + i128::from(high.x_subpx), *index)
        });
        if crossings.len() % 2 != 0 {
            return Err(RenderError::Triangulation);
        }
        for pair in crossings.chunks_exact(2) {
            let (left_low, left_high) = (pair[0].0, pair[0].1);
            let (right_low, right_high) = (pair[1].0, pair[1].1);
            push_triangle(
                output,
                [left_low, right_low, right_high],
                palette_index,
                object_id,
                pass,
                ordinal,
            );
            push_triangle(
                output,
                [left_low, right_high, left_high],
                palette_index,
                object_id,
                pass,
                ordinal,
            );
        }
    }
    Ok(())
}

fn interpolate_at_y(
    start: RasterVertex,
    end: RasterVertex,
    y_subpx: i64,
) -> Result<RasterVertex, RenderError> {
    let delta_y = end
        .y_subpx
        .checked_sub(start.y_subpx)
        .ok_or(RenderError::ArithmeticOverflow)?;
    if delta_y == 0 {
        return Err(RenderError::Triangulation);
    }
    Ok(RasterVertex::new(
        interpolate(start.x_subpx, end.x_subpx, y_subpx - start.y_subpx, delta_y)?,
        y_subpx,
        interpolate_i32(start.depth, end.depth, y_subpx - start.y_subpx, delta_y)?,
    ))
}

fn interpolate(start: i64, end: i64, numerator: i64, denominator: i64) -> Result<i64, RenderError> {
    let delta = i128::from(end - start)
        .checked_mul(i128::from(numerator))
        .ok_or(RenderError::ArithmeticOverflow)?
        / i128::from(denominator);
    i64::try_from(i128::from(start) + delta).map_err(|_| RenderError::ArithmeticOverflow)
}

fn interpolate_i32(
    start: i32,
    end: i32,
    numerator: i64,
    denominator: i64,
) -> Result<i32, RenderError> {
    let delta = i128::from(end - start)
        .checked_mul(i128::from(numerator))
        .ok_or(RenderError::ArithmeticOverflow)?
        / i128::from(denominator);
    i32::try_from(i128::from(start) + delta).map_err(|_| RenderError::ArithmeticOverflow)
}

fn push_triangle(
    output: &mut Vec<Triangle>,
    vertices: [RasterVertex; 3],
    palette_index: u8,
    object_id: ObjectId,
    pass: u8,
    ordinal: &mut u32,
) {
    if triangle_area(vertices) == 0 {
        return;
    }
    output.push(Triangle {
        vertices,
        palette_index,
        stable_key: primitive_key(object_id, pass, *ordinal),
    });
    *ordinal = ordinal.wrapping_add(1);
}

const fn triangle_area(vertices: [RasterVertex; 3]) -> i128 {
    (vertices[1].x_subpx as i128 - vertices[0].x_subpx as i128)
        * (vertices[2].y_subpx as i128 - vertices[0].y_subpx as i128)
        - (vertices[1].y_subpx as i128 - vertices[0].y_subpx as i128)
            * (vertices[2].x_subpx as i128 - vertices[0].x_subpx as i128)
}

const fn primitive_key(object_id: ObjectId, pass: u8, ordinal: u32) -> u64 {
    let mut value = object_id.get() ^ (pass as u64).wrapping_mul(0x9e37_79b9_7f4a_7c15);
    value ^= (ordinal as u64).wrapping_mul(0xbf58_476d_1ce4_e5b9);
    value ^= value >> 30;
    value = value.wrapping_mul(0xbf58_476d_1ce4_e5b9);
    value ^= value >> 27;
    value = value.wrapping_mul(0x94d0_49bb_1331_11eb);
    value ^= value >> 31;
    if value == 0 { 1 } else { value }
}

fn geometry_points(geometry: &Geometry) -> impl Iterator<Item = &WorldPoint> {
    let polygons: &[Polygon] = match geometry {
        Geometry::Polygon(polygon) => std::slice::from_ref(polygon),
        Geometry::MultiPolygon(polygons) => polygons,
    };
    polygons
        .iter()
        .flat_map(Polygon::rings)
        .flat_map(isometric_world::Ring::points)
}

const fn ground_color(class: SemanticClass) -> u8 {
    match class {
        SemanticClass::Terrain => 1,
        SemanticClass::Water => 4,
        SemanticClass::Road | SemanticClass::Parking => 9,
        SemanticClass::Path => 8,
        SemanticClass::AthleticSurface => 12,
        SemanticClass::Building => 7,
        SemanticClass::Vegetation => 2,
        SemanticClass::Unknown => 0,
    }
}

const fn depth_layer(class: SemanticClass) -> u8 {
    match class {
        SemanticClass::Unknown => 0,
        SemanticClass::Terrain => 1,
        SemanticClass::Water => 2,
        SemanticClass::Vegetation => 3,
        SemanticClass::Parking | SemanticClass::AthleticSurface => 4,
        SemanticClass::Road => 5,
        SemanticClass::Path => 6,
        SemanticClass::Building => 7,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const OSM: &[u8] = include_bytes!("../../../fixtures/sources/osm-2026-07-15-hero.json");
    const OVERTURE: &[u8] = include_bytes!("../../../fixtures/sources/overture-buildings.geojson");

    #[test]
    fn hero_world_produces_deterministic_non_abstract_art() {
        let compiled = isometric_world::compile_hero(OSM, OVERTURE).expect("world compiles");
        let style = StylePack::stanford_v1();
        let first = render_world(&compiled.world, &style).expect("hero renders");
        let second = render_world(&compiled.world, &style).expect("hero rerenders");
        assert_eq!(first, second);
        assert!(first.width() > 1_000);
        assert!(first.height() > 500);
        assert_eq!((first.width(), first.height()), (1_950, 873));
        assert_eq!(crate::stable_hash(first.pixels()), 0x2613_2f98_95f0_cd70);
        assert!(first.pixels().contains(&5));
        assert!(first.pixels().contains(&9));
    }

    #[test]
    fn scanline_decomposition_preserves_a_hole() {
        let outer = vec![
            RasterVertex::new(0, 0, 1),
            RasterVertex::new(1_024, 0, 1),
            RasterVertex::new(1_024, 1_024, 1),
            RasterVertex::new(0, 1_024, 1),
            RasterVertex::new(0, 0, 1),
        ];
        let hole = vec![
            RasterVertex::new(256, 256, 1),
            RasterVertex::new(768, 256, 1),
            RasterVertex::new(768, 768, 1),
            RasterVertex::new(256, 768, 1),
            RasterVertex::new(256, 256, 1),
        ];
        let mut triangles = Vec::new();
        let mut ordinal = 0;
        append_polygon_fill(
            &mut triangles,
            &[outer, hole],
            1,
            ObjectId::new(1).expect("id"),
            PASS_GROUND,
            &mut ordinal,
        )
        .expect("decompose");
        let mut surface = RasterSurface::new(4, 4, 0, 2).expect("surface");
        surface.rasterize(&triangles).expect("rasterize");
        assert_eq!(surface.image().pixels()[5], 0);
        assert_eq!(surface.image().pixels()[0], 1);
    }
}
