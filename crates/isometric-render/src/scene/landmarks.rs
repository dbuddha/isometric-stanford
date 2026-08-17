//! Original parameterized geometry for the prototype's three hero landmarks.

use isometric_core::{ObjectId, WorldPoint};
use isometric_style::StylePack;
use isometric_world::{Polygon, SemanticClass, WorldObject};

use super::{
    MAIN_QUAD_OBJECT_ID, Viewport, append_polygon_fill, push_triangle, scene_rings, scene_vertex,
};
use crate::{RasterVertex, RenderError, Triangle};

const PASS_LANDMARK: u8 = 6;
const PASS_DETAIL: u8 = 7;

/// Replaces an ordinary extrusion when the source object names a hero landmark.
pub(super) fn append_hero_landmark(
    output: &mut Vec<Triangle>,
    polygon: &Polygon,
    object: &WorldObject,
    style: &StylePack,
    view: Viewport,
    ordinal: &mut u32,
) -> Result<bool, RenderError> {
    if object.id().get() == MAIN_QUAD_OBJECT_ID {
        append_polygon_shell(
            output,
            polygon,
            0,
            style.landmarks.main_quad_wall_height_mm,
            object.id(),
            style,
            view,
            ordinal,
        )?;
        append_main_quad_arcades(output, polygon, object, style, view, ordinal)?;
        return Ok(true);
    }
    match object.name() {
        Some("Hoover Tower") => {
            append_hoover(output, polygon, object.id(), style, view, ordinal)?;
            Ok(true)
        }
        Some("Memorial Church") => {
            append_church(output, polygon, object.id(), style, view, ordinal)?;
            Ok(true)
        }
        _ => Ok(false),
    }
}

/// Adds repeated arcade apertures to the canonical Main Quad courtyard walls.
pub(super) fn append_main_quad_arcades(
    output: &mut Vec<Triangle>,
    polygon: &Polygon,
    object: &WorldObject,
    style: &StylePack,
    view: Viewport,
    ordinal: &mut u32,
) -> Result<(), RenderError> {
    if object.id().get() != MAIN_QUAD_OBJECT_ID {
        return Ok(());
    }
    let [spacing, width, shoulder_height, apex_height] = style.landmarks.arcade_mm;
    for ring in polygon.rings().iter().skip(1) {
        let points = ring.points();
        let Some((min_x, max_x, min_y, max_y)) = ring_bounds(points) else {
            continue;
        };
        if (max_x - min_x).max(max_y - min_y) < 30_000 {
            continue;
        }
        let center = WorldPoint::new(min_x.midpoint(max_x), min_y.midpoint(max_y), 0);
        for edge in points.windows(2) {
            let dx = edge[1].x_mm - edge[0].x_mm;
            let dy = edge[1].y_mm - edge[0].y_mm;
            let edge_length = dx.abs().max(dy.abs());
            let count = edge_length / spacing;
            if count == 0 {
                continue;
            }
            for index in 0..count {
                let denominator = count + 1;
                let numerator = index + 1;
                let middle = WorldPoint::new(
                    edge[0].x_mm + dx * numerator / denominator,
                    edge[0].y_mm + dy * numerator / denominator,
                    0,
                );
                let offset_x = (center.x_mm - middle.x_mm).signum() * 250;
                let offset_y = (center.y_mm - middle.y_mm).signum() * 250;
                let half = width / 2;
                let along_x = dx * half / edge_length;
                let along_y = dy * half / edge_length;
                let left = WorldPoint::new(
                    middle.x_mm - along_x + offset_x,
                    middle.y_mm - along_y + offset_y,
                    0,
                );
                let right = WorldPoint::new(
                    middle.x_mm + along_x + offset_x,
                    middle.y_mm + along_y + offset_y,
                    0,
                );
                let vertices = [
                    landmark_vertex(left, 1_000, style, view)?,
                    landmark_vertex(right, 1_000, style, view)?,
                    landmark_vertex(right, shoulder_height, style, view)?,
                    landmark_vertex(left, shoulder_height, style, view)?,
                    landmark_vertex(
                        WorldPoint::new(middle.x_mm + offset_x, middle.y_mm + offset_y, 0),
                        apex_height,
                        style,
                        view,
                    )?,
                ];
                push_triangle(
                    output,
                    [vertices[0], vertices[1], vertices[2]],
                    landmark_opening_color(style, style.ordinary.outline),
                    object.id(),
                    PASS_DETAIL,
                    ordinal,
                );
                push_triangle(
                    output,
                    [vertices[0], vertices[2], vertices[3]],
                    landmark_opening_color(style, style.ordinary.outline),
                    object.id(),
                    PASS_DETAIL,
                    ordinal,
                );
                push_triangle(
                    output,
                    [vertices[3], vertices[2], vertices[4]],
                    landmark_opening_color(style, style.ordinary.outline),
                    object.id(),
                    PASS_DETAIL,
                    ordinal,
                );
            }
        }
    }
    Ok(())
}

fn append_hoover(
    output: &mut Vec<Triangle>,
    polygon: &Polygon,
    object_id: ObjectId,
    style: &StylePack,
    view: Viewport,
    ordinal: &mut u32,
) -> Result<(), RenderError> {
    let center = polygon_center(polygon);
    let [base_top, shaft_top, ..] = style.landmarks.hoover_heights_mm;
    let [shaft_width, ..] = style.landmarks.hoover_widths_mm;

    append_polygon_shell(
        output, polygon, 0, base_top, object_id, style, view, ordinal,
    )?;
    append_prism(
        output,
        center,
        shaft_width,
        shaft_width,
        base_top,
        shaft_top,
        style.ordinary.building,
        object_id,
        style,
        view,
        ordinal,
    )?;

    append_hoover_windows(
        output,
        center,
        shaft_width,
        base_top,
        shaft_top,
        object_id,
        style,
        view,
        ordinal,
    )?;

    for band_bottom in [27_000, 44_000, 59_500] {
        append_prism(
            output,
            center,
            shaft_width + 1_200,
            shaft_width + 1_200,
            band_bottom,
            band_bottom + 1_200,
            [style.ordinary.outline; 3],
            object_id,
            style,
            view,
            ordinal,
        )?;
    }

    append_hoover_crown(output, center, object_id, style, view, ordinal)
}

fn append_hoover_crown(
    output: &mut Vec<Triangle>,
    center: WorldPoint,
    object_id: ObjectId,
    style: &StylePack,
    view: Viewport,
    ordinal: &mut u32,
) -> Result<(), RenderError> {
    let [_, shaft_top, crown_top, ..] = style.landmarks.hoover_heights_mm;
    let [_, crown_width, _] = style.landmarks.hoover_widths_mm;
    append_prism(
        output,
        center,
        crown_width,
        crown_width,
        shaft_top,
        crown_top,
        style.ordinary.building,
        object_id,
        style,
        view,
        ordinal,
    )?;
    append_window_belt(
        output,
        center,
        crown_width / 2,
        crown_width / 2,
        shaft_top + 2_000,
        crown_top - 1_500,
        4_200,
        1_400,
        object_id,
        style,
        view,
        ordinal,
    )?;
    append_hoover_cap(output, center, object_id, style, view, ordinal)
}

fn append_hoover_cap(
    output: &mut Vec<Triangle>,
    center: WorldPoint,
    object_id: ObjectId,
    style: &StylePack,
    view: Viewport,
    ordinal: &mut u32,
) -> Result<(), RenderError> {
    let [_, _, crown_top, lantern_top, total_top] = style.landmarks.hoover_heights_mm;
    let [_, _, lantern_width] = style.landmarks.hoover_widths_mm;
    append_prism(
        output,
        center,
        lantern_width,
        lantern_width,
        crown_top,
        lantern_top,
        style.ordinary.building,
        object_id,
        style,
        view,
        ordinal,
    )?;
    append_window_belt(
        output,
        center,
        lantern_width / 2,
        lantern_width / 2,
        crown_top + 1_500,
        lantern_top - 1_500,
        3_600,
        1_200,
        object_id,
        style,
        view,
        ordinal,
    )?;
    append_prism(
        output,
        center,
        lantern_width + 1_000,
        lantern_width + 1_000,
        crown_top + 3_000,
        crown_top + 6_500,
        [style.ordinary.outline; 3],
        object_id,
        style,
        view,
        ordinal,
    )?;
    append_prism(
        output,
        center,
        10_000,
        10_000,
        lantern_top,
        total_top - 1_500,
        style.ordinary.building,
        object_id,
        style,
        view,
        ordinal,
    )?;
    append_pyramid(
        output,
        center,
        10_000,
        total_top - 1_500,
        total_top,
        object_id,
        style,
        view,
        ordinal,
    )
}

#[allow(clippy::too_many_arguments)]
fn append_hoover_windows(
    output: &mut Vec<Triangle>,
    center: WorldPoint,
    shaft_width: i64,
    bottom_mm: i64,
    top_mm: i64,
    object_id: ObjectId,
    style: &StylePack,
    view: Viewport,
    ordinal: &mut u32,
) -> Result<(), RenderError> {
    let half = shaft_width / 2;
    for level in [16_000, 24_000, 33_000, 41_000, 50_000, 57_000] {
        if level + 2_200 >= top_mm || level < bottom_mm {
            continue;
        }
        for along in [-3_000, 3_000] {
            append_face_panels(
                output,
                center,
                half,
                half,
                along,
                1_500,
                level,
                level + 2_200,
                landmark_opening_color(style, style.ordinary.shadow),
                object_id,
                style,
                view,
                ordinal,
            )?;
        }
    }
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn append_window_belt(
    output: &mut Vec<Triangle>,
    center: WorldPoint,
    half_width: i64,
    half_length: i64,
    bottom_mm: i64,
    top_mm: i64,
    spacing_mm: i64,
    panel_width_mm: i64,
    object_id: ObjectId,
    style: &StylePack,
    view: Viewport,
    ordinal: &mut u32,
) -> Result<(), RenderError> {
    let count = ((half_width * 2 - spacing_mm) / spacing_mm).max(1);
    for index in 0..count {
        let along = -((count - 1) * spacing_mm) / 2 + index * spacing_mm;
        append_face_panels(
            output,
            center,
            half_width,
            half_length,
            along,
            panel_width_mm,
            bottom_mm,
            top_mm,
            landmark_opening_color(style, style.ordinary.shadow),
            object_id,
            style,
            view,
            ordinal,
        )?;
    }
    Ok(())
}

fn append_church(
    output: &mut Vec<Triangle>,
    polygon: &Polygon,
    object_id: ObjectId,
    style: &StylePack,
    view: Viewport,
    ordinal: &mut u32,
) -> Result<(), RenderError> {
    let center = polygon_center(polygon);
    let [wall_height, roof_rise, half_width, half_length] = style.landmarks.church_mm;
    append_polygon_shell(
        output,
        polygon,
        0,
        wall_height,
        object_id,
        style,
        view,
        ordinal,
    )?;

    let corners = [
        oriented_point(center, -half_width, -half_length, wall_height, style),
        oriented_point(center, half_width, -half_length, wall_height, style),
        oriented_point(center, half_width, half_length, wall_height, style),
        oriented_point(center, -half_width, half_length, wall_height, style),
    ];
    let ridge_start = oriented_point(center, 0, -half_length, wall_height + roof_rise, style);
    let ridge_end = oriented_point(center, 0, half_length, wall_height + roof_rise, style);
    let projected = corners
        .into_iter()
        .map(|point| landmark_vertex(point, 0, style, view))
        .collect::<Result<Vec<_>, _>>()?;
    let ridge_start = landmark_vertex(ridge_start, 0, style, view)?;
    let ridge_end = landmark_vertex(ridge_end, 0, style, view)?;
    for (vertices, color) in [
        (
            [projected[0], projected[3], ridge_end],
            style.ordinary.building[0],
        ),
        (
            [projected[0], ridge_end, ridge_start],
            style.ordinary.building[0],
        ),
        (
            [projected[1], ridge_start, ridge_end],
            style.ordinary.building[2],
        ),
        (
            [projected[1], ridge_end, projected[2]],
            style.ordinary.building[2],
        ),
        (
            [projected[0], projected[1], ridge_start],
            style.ordinary.building[1],
        ),
        (
            [projected[3], ridge_end, projected[2]],
            style.ordinary.building[1],
        ),
    ] {
        push_triangle(output, vertices, color, object_id, PASS_LANDMARK, ordinal);
    }
    append_church_openings(
        output,
        center,
        half_width,
        half_length,
        wall_height,
        object_id,
        style,
        view,
        ordinal,
    )?;
    append_rose_window(
        output,
        center,
        wall_height,
        roof_rise,
        half_length,
        object_id,
        style,
        view,
        ordinal,
    )
}

#[allow(clippy::too_many_arguments)]
fn append_church_openings(
    output: &mut Vec<Triangle>,
    center: WorldPoint,
    half_width: i64,
    half_length: i64,
    wall_height: i64,
    object_id: ObjectId,
    style: &StylePack,
    view: Viewport,
    ordinal: &mut u32,
) -> Result<(), RenderError> {
    for along in [-18_000, -9_000, 0, 9_000, 18_000] {
        append_u_face_panel(
            output,
            center,
            half_width + 300,
            along,
            2_400,
            5_000,
            wall_height - 2_500,
            landmark_opening_color(style, style.ordinary.outline),
            object_id,
            style,
            view,
            ordinal,
        )?;
        append_u_face_panel(
            output,
            center,
            -half_width - 300,
            along,
            2_400,
            5_000,
            wall_height - 2_500,
            landmark_opening_color(style, style.ordinary.outline),
            object_id,
            style,
            view,
            ordinal,
        )?;
    }
    append_v_face_panel(
        output,
        center,
        0,
        -half_length - 300,
        6_000,
        1_000,
        10_000,
        if style.ordinary.candidate_c_details() {
            style.ordinary.door
        } else {
            style.ordinary.outline
        },
        object_id,
        style,
        view,
        ordinal,
    )
}

#[allow(clippy::too_many_arguments)]
fn append_rose_window(
    output: &mut Vec<Triangle>,
    center: WorldPoint,
    wall_height: i64,
    roof_rise: i64,
    half_length: i64,
    object_id: ObjectId,
    style: &StylePack,
    view: Viewport,
    ordinal: &mut u32,
) -> Result<(), RenderError> {
    let v = -half_length - 300;
    let center_z = wall_height + roof_rise / 2;
    let points = [
        oriented_point(center, -2_500, v, center_z, style),
        oriented_point(center, 0, v, center_z - 2_500, style),
        oriented_point(center, 2_500, v, center_z, style),
        oriented_point(center, 0, v, center_z + 2_500, style),
    ];
    let projected = points
        .into_iter()
        .map(|point| landmark_vertex(point, 0, style, view))
        .collect::<Result<Vec<_>, _>>()?;
    push_triangle(
        output,
        [projected[0], projected[1], projected[2]],
        landmark_opening_color(style, style.ordinary.outline),
        object_id,
        PASS_DETAIL,
        ordinal,
    );
    push_triangle(
        output,
        [projected[0], projected[2], projected[3]],
        landmark_opening_color(style, style.ordinary.outline),
        object_id,
        PASS_DETAIL,
        ordinal,
    );
    Ok(())
}

fn landmark_opening_color(style: &StylePack, fallback: u8) -> u8 {
    if style.ordinary.candidate_c_details() {
        style.ordinary.windows[0]
    } else {
        fallback
    }
}

#[allow(clippy::too_many_arguments)]
fn append_face_panels(
    output: &mut Vec<Triangle>,
    center: WorldPoint,
    half_width: i64,
    half_length: i64,
    along_mm: i64,
    panel_width_mm: i64,
    bottom_mm: i64,
    top_mm: i64,
    color: u8,
    object_id: ObjectId,
    style: &StylePack,
    view: Viewport,
    ordinal: &mut u32,
) -> Result<(), RenderError> {
    for face_u in [half_width + 300, -half_width - 300] {
        append_u_face_panel(
            output,
            center,
            face_u,
            along_mm,
            panel_width_mm,
            bottom_mm,
            top_mm,
            color,
            object_id,
            style,
            view,
            ordinal,
        )?;
    }
    for face_v in [half_length + 300, -half_length - 300] {
        append_v_face_panel(
            output,
            center,
            along_mm,
            face_v,
            panel_width_mm,
            bottom_mm,
            top_mm,
            color,
            object_id,
            style,
            view,
            ordinal,
        )?;
    }
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn append_u_face_panel(
    output: &mut Vec<Triangle>,
    center: WorldPoint,
    face_u_mm: i64,
    center_v_mm: i64,
    width_mm: i64,
    bottom_mm: i64,
    top_mm: i64,
    color: u8,
    object_id: ObjectId,
    style: &StylePack,
    view: Viewport,
    ordinal: &mut u32,
) -> Result<(), RenderError> {
    append_panel(
        output,
        [
            oriented_point(
                center,
                face_u_mm,
                center_v_mm - width_mm / 2,
                bottom_mm,
                style,
            ),
            oriented_point(
                center,
                face_u_mm,
                center_v_mm + width_mm / 2,
                bottom_mm,
                style,
            ),
            oriented_point(center, face_u_mm, center_v_mm + width_mm / 2, top_mm, style),
            oriented_point(center, face_u_mm, center_v_mm - width_mm / 2, top_mm, style),
        ],
        color,
        object_id,
        style,
        view,
        ordinal,
    )
}

#[allow(clippy::too_many_arguments)]
fn append_v_face_panel(
    output: &mut Vec<Triangle>,
    center: WorldPoint,
    center_u_mm: i64,
    face_v_mm: i64,
    width_mm: i64,
    bottom_mm: i64,
    top_mm: i64,
    color: u8,
    object_id: ObjectId,
    style: &StylePack,
    view: Viewport,
    ordinal: &mut u32,
) -> Result<(), RenderError> {
    append_panel(
        output,
        [
            oriented_point(
                center,
                center_u_mm - width_mm / 2,
                face_v_mm,
                bottom_mm,
                style,
            ),
            oriented_point(
                center,
                center_u_mm + width_mm / 2,
                face_v_mm,
                bottom_mm,
                style,
            ),
            oriented_point(center, center_u_mm + width_mm / 2, face_v_mm, top_mm, style),
            oriented_point(center, center_u_mm - width_mm / 2, face_v_mm, top_mm, style),
        ],
        color,
        object_id,
        style,
        view,
        ordinal,
    )
}

#[allow(clippy::too_many_arguments)]
fn append_panel(
    output: &mut Vec<Triangle>,
    points: [WorldPoint; 4],
    color: u8,
    object_id: ObjectId,
    style: &StylePack,
    view: Viewport,
    ordinal: &mut u32,
) -> Result<(), RenderError> {
    let vertices = points
        .into_iter()
        .map(|point| landmark_vertex(point, 0, style, view))
        .collect::<Result<Vec<_>, _>>()?;
    push_triangle(
        output,
        [vertices[0], vertices[1], vertices[2]],
        color,
        object_id,
        PASS_DETAIL,
        ordinal,
    );
    push_triangle(
        output,
        [vertices[0], vertices[2], vertices[3]],
        color,
        object_id,
        PASS_DETAIL,
        ordinal,
    );
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn append_polygon_shell(
    output: &mut Vec<Triangle>,
    polygon: &Polygon,
    bottom_mm: i64,
    top_mm: i64,
    object_id: ObjectId,
    style: &StylePack,
    view: Viewport,
    ordinal: &mut u32,
) -> Result<(), RenderError> {
    for ring in polygon.rings() {
        append_ring_walls(
            output,
            ring.points(),
            bottom_mm,
            top_mm,
            style.ordinary.building,
            object_id,
            style,
            view,
            ordinal,
        )?;
    }
    let roof = scene_rings(polygon, top_mm, SemanticClass::Building, style, view)?;
    append_polygon_fill(
        output,
        &roof,
        style.ordinary.building[0],
        object_id,
        PASS_LANDMARK,
        ordinal,
    )
}

#[allow(clippy::too_many_arguments)]
fn append_prism(
    output: &mut Vec<Triangle>,
    center: WorldPoint,
    width: i64,
    length: i64,
    bottom_mm: i64,
    top_mm: i64,
    colors: [u8; 3],
    object_id: ObjectId,
    style: &StylePack,
    view: Viewport,
    ordinal: &mut u32,
) -> Result<(), RenderError> {
    let ring = oriented_ring(center, width / 2, length / 2, style);
    append_ring_walls(
        output, &ring, bottom_mm, top_mm, colors, object_id, style, view, ordinal,
    )?;
    let roof = ring
        .iter()
        .map(|point| landmark_vertex(*point, top_mm, style, view))
        .collect::<Result<Vec<_>, _>>()?;
    append_polygon_fill(
        output,
        &[roof],
        colors[0],
        object_id,
        PASS_LANDMARK,
        ordinal,
    )
}

#[allow(clippy::too_many_arguments)]
fn append_ring_walls(
    output: &mut Vec<Triangle>,
    ring: &[WorldPoint],
    bottom_mm: i64,
    top_mm: i64,
    colors: [u8; 3],
    object_id: ObjectId,
    style: &StylePack,
    view: Viewport,
    ordinal: &mut u32,
) -> Result<(), RenderError> {
    for edge in ring.windows(2) {
        let vertices = [
            landmark_vertex(edge[0], bottom_mm, style, view)?,
            landmark_vertex(edge[1], bottom_mm, style, view)?,
            landmark_vertex(edge[1], top_mm, style, view)?,
            landmark_vertex(edge[0], top_mm, style, view)?,
        ];
        let color = if edge[1].x_mm - edge[0].x_mm >= edge[1].y_mm - edge[0].y_mm {
            colors[1]
        } else {
            colors[2]
        };
        push_triangle(
            output,
            [vertices[0], vertices[1], vertices[2]],
            color,
            object_id,
            PASS_LANDMARK,
            ordinal,
        );
        push_triangle(
            output,
            [vertices[0], vertices[2], vertices[3]],
            color,
            object_id,
            PASS_LANDMARK,
            ordinal,
        );
    }
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn append_pyramid(
    output: &mut Vec<Triangle>,
    center: WorldPoint,
    width: i64,
    bottom_mm: i64,
    top_mm: i64,
    object_id: ObjectId,
    style: &StylePack,
    view: Viewport,
    ordinal: &mut u32,
) -> Result<(), RenderError> {
    let ring = oriented_ring(center, width / 2, width / 2, style);
    let apex = landmark_vertex(center, top_mm, style, view)?;
    for edge in ring.windows(2) {
        push_triangle(
            output,
            [
                landmark_vertex(edge[0], bottom_mm, style, view)?,
                landmark_vertex(edge[1], bottom_mm, style, view)?,
                apex,
            ],
            style.ordinary.building[0],
            object_id,
            PASS_LANDMARK,
            ordinal,
        );
    }
    Ok(())
}

fn landmark_vertex(
    point: WorldPoint,
    z_mm: i64,
    style: &StylePack,
    view: Viewport,
) -> Result<RasterVertex, RenderError> {
    scene_vertex(point, z_mm, SemanticClass::Building, style, view)
}

fn oriented_ring(
    center: WorldPoint,
    half_width: i64,
    half_length: i64,
    style: &StylePack,
) -> Vec<WorldPoint> {
    [
        (-half_width, -half_length),
        (half_width, -half_length),
        (half_width, half_length),
        (-half_width, half_length),
        (-half_width, -half_length),
    ]
    .into_iter()
    .map(|(u, v)| oriented_point(center, u, v, 0, style))
    .collect()
}

fn oriented_point(
    center: WorldPoint,
    u_mm: i64,
    v_mm: i64,
    z_mm: i64,
    style: &StylePack,
) -> WorldPoint {
    WorldPoint::new(
        center.x_mm
            + (style.landmarks.campus_u[0] * u_mm + style.landmarks.campus_v[0] * v_mm) / 1_000,
        center.y_mm
            + (style.landmarks.campus_u[1] * u_mm + style.landmarks.campus_v[1] * v_mm) / 1_000,
        center.z_mm + z_mm,
    )
}

fn polygon_center(polygon: &Polygon) -> WorldPoint {
    let (min_x, max_x, min_y, max_y) =
        ring_bounds(polygon.rings()[0].points()).expect("validated ring has points");
    WorldPoint::new(min_x.midpoint(max_x), min_y.midpoint(max_y), 0)
}

fn ring_bounds(points: &[WorldPoint]) -> Option<(i64, i64, i64, i64)> {
    Some((
        points.iter().map(|point| point.x_mm).min()?,
        points.iter().map(|point| point.x_mm).max()?,
        points.iter().map(|point| point.y_mm).min()?,
        points.iter().map(|point| point.y_mm).max()?,
    ))
}

#[cfg(test)]
mod tests {
    use isometric_world::Geometry;

    use super::*;

    const OSM: &[u8] = include_bytes!("../../../../fixtures/sources/osm-2026-07-15-hero.json");
    const OVERTURE: &[u8] =
        include_bytes!("../../../../fixtures/sources/overture-buildings.geojson");

    #[test]
    fn all_three_landmark_grammars_emit_distinct_details() {
        let compiled = isometric_world::compile_hero(OSM, OVERTURE).expect("world compiles");
        let style = StylePack::stanford_v1();
        let view = Viewport::from_world(&compiled.world, &style).expect("viewport");

        for (name, minimum_triangles) in [("Hoover Tower", 180), ("Memorial Church", 70)] {
            let object = compiled
                .world
                .objects()
                .iter()
                .find(|object| object.name() == Some(name))
                .expect("named landmark");
            let Geometry::Polygon(polygon) = object.geometry() else {
                panic!("landmark must be polygonal");
            };
            let mut output = Vec::new();
            let mut ordinal = 0;
            assert!(
                append_hero_landmark(&mut output, polygon, object, &style, view, &mut ordinal,)
                    .expect("landmark renders")
            );
            assert!(output.len() >= minimum_triangles);
            let detail_color = if name == "Hoover Tower" {
                style.ordinary.shadow
            } else {
                style.ordinary.outline
            };
            assert!(
                output
                    .iter()
                    .filter(|triangle| triangle.palette_index == detail_color)
                    .count()
                    >= 20
            );
        }

        let quad = compiled
            .world
            .objects()
            .iter()
            .find(|object| object.id().get() == MAIN_QUAD_OBJECT_ID)
            .expect("main quad object");
        let Geometry::Polygon(polygon) = quad.geometry() else {
            panic!("quad must be polygonal");
        };
        let mut output = Vec::new();
        let mut ordinal = 0;
        assert!(
            append_hero_landmark(&mut output, polygon, quad, &style, view, &mut ordinal,)
                .expect("quad renders")
        );
        assert!(
            output
                .iter()
                .filter(|triangle| triangle.palette_index == style.ordinary.outline)
                .count()
                > 100
        );
    }
}
