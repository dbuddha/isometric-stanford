//! Conversion from canonical semantic polygons to ordinary campus art faces.

use std::collections::BTreeSet;

use isometric_core::{ObjectId, WorldPoint};
use isometric_style::StylePack;
use isometric_world::{Geometry, Polygon, SemanticClass, World, WorldObject};

use crate::{IndexedImage, RasterSurface, RasterVertex, RenderError, Triangle, project};

mod landmarks;

const VIEW_MARGIN_PIXELS: i64 = 32;
const PASS_GROUND: u8 = 1;
const PASS_WALL: u8 = 2;
const PASS_ROOF: u8 = 3;
const PASS_CROWN: u8 = 4;
const PASS_SHADOW: u8 = 5;
const PASS_ORDINARY_DETAIL: u8 = 8;
const MAX_FACADE_QUADS_PER_OBJECT: usize = 512;
const MAX_HIP_ROOF_PLANES: usize = 32;

/// Stable content ID of the canonical Main Quad courtyard object.
pub const MAIN_QUAD_OBJECT_ID: u64 = 7_375_667_649_447_908_307;

/// Stable full-scene coordinate system used by independently rendered tiles.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RenderLayout {
    offset_x_subpx: i64,
    offset_y_subpx: i64,
    width: u32,
    height: u32,
}

impl RenderLayout {
    /// Returns the complete artwork width in logical pixels.
    #[must_use]
    pub const fn width(self) -> u32 {
        self.width
    }

    /// Returns the complete artwork height in logical pixels.
    #[must_use]
    pub const fn height(self) -> u32 {
        self.height
    }

    /// Projects a world point into this layout's logical pixel coordinates.
    ///
    /// # Errors
    ///
    /// Returns an error when projection or offset arithmetic overflows.
    pub fn image_coordinates(
        self,
        point: WorldPoint,
        style: &StylePack,
    ) -> Result<(i64, i64), RenderError> {
        let projected = project(point, style)?;
        let x = projected
            .x_subpx
            .checked_add(self.offset_x_subpx)
            .ok_or(RenderError::ArithmeticOverflow)?;
        let y = projected
            .y_subpx
            .checked_add(self.offset_y_subpx)
            .ok_or(RenderError::ArithmeticOverflow)?;
        Ok((
            x.div_euclid(style.subpixels_per_pixel),
            y.div_euclid(style.subpixels_per_pixel),
        ))
    }
}

/// One canonical level-zero tile request.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct TileRequest {
    /// Zero-based tile column.
    pub column: u32,
    /// Zero-based tile row.
    pub row: u32,
    /// Saved tile edge length in logical pixels.
    pub tile_size: u32,
    /// Unsaved context rendered around every tile edge.
    pub guard: u32,
}

/// Bounded-memory evidence emitted with one canonical tile.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct TileRenderStats {
    /// Objects conservatively selected for this guarded tile.
    pub candidate_objects: usize,
    /// Main and shadow primitives submitted to the rasterizer.
    pub primitives: usize,
    /// Guarded surface width.
    pub surface_width: u32,
    /// Guarded surface height.
    pub surface_height: u32,
    /// Conservative peak bytes owned by palette and depth pixel buffers.
    pub peak_pixel_buffer_bytes: usize,
}

/// One saved canonical tile and its bounded-memory evidence.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TileRender {
    /// Cropped palette-indexed tile with no guard pixels.
    pub image: IndexedImage,
    /// Render-time selection and allocation evidence.
    pub stats: TileRenderStats,
}

/// Renders ordinary semantic geometry into a tightly bounded indexed image.
///
/// This stage covers ordinary polygonal surfaces, faceted vegetation, hard
/// shadows, one-pixel outlines, and world-anchored material dithering. Detailed
/// roofs and landmark-specific grammar remain later procedural passes.
///
/// # Errors
///
/// Returns an error for invalid style, empty geometry, projection or capacity
/// overflow, noncanonical polygons, or raster failures.
pub fn render_world(world: &World, style: &StylePack) -> Result<IndexedImage, RenderError> {
    style.validate().map_err(|_| RenderError::InvalidStyle)?;
    let layout = render_layout(world, style)?;
    let view = Viewport::from_layout(layout);
    let (image, _) = render_objects(world.objects().iter(), style, view)?;
    Ok(image)
}

/// Renders selected canonical objects in one guarded region of the stable layout.
///
/// # Errors
///
/// Returns an error when the selection or region is invalid or rendering fails.
#[allow(clippy::too_many_arguments)]
pub fn render_selected_region(
    world: &World,
    style: &StylePack,
    layout: RenderLayout,
    object_ids: &[ObjectId],
    x: u32,
    y: u32,
    width: u32,
    height: u32,
) -> Result<IndexedImage, RenderError> {
    if object_ids.is_empty()
        || width == 0
        || height == 0
        || width > 2_048
        || height > 2_048
        || x.checked_add(width)
            .is_none_or(|right| right > layout.width)
        || y.checked_add(height)
            .is_none_or(|bottom| bottom > layout.height)
    {
        return Err(RenderError::InvalidObjectSelection);
    }
    let selected = object_ids.iter().copied().collect::<BTreeSet<_>>();
    if selected.len() != object_ids.len()
        || !selected
            .iter()
            .all(|id| world.objects().iter().any(|object| object.id() == *id))
    {
        return Err(RenderError::InvalidObjectSelection);
    }
    let guard = required_tile_guard(style)?;
    let surface_width = width
        .checked_add(guard * 2)
        .ok_or(RenderError::ArithmeticOverflow)?;
    let surface_height = height
        .checked_add(guard * 2)
        .ok_or(RenderError::ArithmeticOverflow)?;
    let view = Viewport::for_region(
        layout,
        i64::from(x) - i64::from(guard),
        i64::from(y) - i64::from(guard),
        surface_width,
        surface_height,
        style,
    )?;
    let objects = world
        .objects()
        .iter()
        .filter(|object| selected.contains(&object.id()));
    let (guarded, _) = render_objects(objects, style, view)?;
    crop_image(&guarded, guard, guard, width, height)
}

/// Derives the stable full-scene layout without allocating a framebuffer.
///
/// # Errors
///
/// Returns an error for invalid style, empty geometry, or projection overflow.
pub fn render_layout(world: &World, style: &StylePack) -> Result<RenderLayout, RenderError> {
    style.validate().map_err(|_| RenderError::InvalidStyle)?;
    Viewport::from_world(world, style).map(Viewport::into_layout)
}

/// Returns the minimum guard needed by the current shadow, crown, and outline grammar.
///
/// # Errors
///
/// Returns an error when style projection arithmetic overflows.
pub fn required_tile_guard(style: &StylePack) -> Result<u32, RenderError> {
    style.validate().map_err(|_| RenderError::InvalidStyle)?;
    let radius = style.ordinary.tree_radius_mm + 1_000;
    let height = style.ordinary.tree_height_mm;
    let effects = [
        WorldPoint::new(style.ordinary.shadow_x_mm, style.ordinary.shadow_y_mm, 0),
        WorldPoint::new(radius, -radius, 0),
        WorldPoint::new(-radius, radius, 0),
        WorldPoint::new(radius, radius, height),
        WorldPoint::new(-radius, -radius, height),
    ];
    let maximum = effects
        .into_iter()
        .map(|point| project(point, style))
        .collect::<Result<Vec<_>, _>>()?
        .into_iter()
        .flat_map(|point| [point.x_subpx.unsigned_abs(), point.y_subpx.unsigned_abs()])
        .max()
        .ok_or(RenderError::InvalidStyle)?;
    let scale =
        u64::try_from(style.subpixels_per_pixel).map_err(|_| RenderError::ArithmeticOverflow)?;
    u32::try_from(maximum.div_ceil(scale) + 2).map_err(|_| RenderError::InvalidDimensions)
}

/// Renders one guarded, cropped canonical tile without allocating the full scene.
///
/// # Errors
///
/// Returns an error for invalid tile coordinates or guard size, projection
/// overflow, noncanonical geometry, or raster failure.
pub fn render_tile(
    world: &World,
    style: &StylePack,
    layout: RenderLayout,
    request: TileRequest,
) -> Result<TileRender, RenderError> {
    style.validate().map_err(|_| RenderError::InvalidStyle)?;
    let required_guard = required_tile_guard(style)?;
    if request.tile_size == 0
        || request.tile_size > 2_048
        || request.guard < required_guard
        || request.guard > 512
    {
        return Err(RenderError::InvalidTileRequest);
    }
    let saved_x = request
        .column
        .checked_mul(request.tile_size)
        .ok_or(RenderError::ArithmeticOverflow)?;
    let saved_y = request
        .row
        .checked_mul(request.tile_size)
        .ok_or(RenderError::ArithmeticOverflow)?;
    if saved_x >= layout.width || saved_y >= layout.height {
        return Err(RenderError::InvalidTileRequest);
    }
    let saved_width = request.tile_size.min(layout.width - saved_x);
    let saved_height = request.tile_size.min(layout.height - saved_y);
    let surface_width = saved_width
        .checked_add(request.guard * 2)
        .ok_or(RenderError::ArithmeticOverflow)?;
    let surface_height = saved_height
        .checked_add(request.guard * 2)
        .ok_or(RenderError::ArithmeticOverflow)?;
    let origin_x = i64::from(saved_x) - i64::from(request.guard);
    let origin_y = i64::from(saved_y) - i64::from(request.guard);
    let view = Viewport::for_region(
        layout,
        origin_x,
        origin_y,
        surface_width,
        surface_height,
        style,
    )?;
    let bounds = PixelBounds {
        min_x: origin_x,
        min_y: origin_y,
        max_x: origin_x + i64::from(surface_width),
        max_y: origin_y + i64::from(surface_height),
    };
    let mut candidates = Vec::new();
    for object in world.objects() {
        if object_intersects(object, style, layout, bounds)? {
            candidates.push(object);
        }
    }
    let candidate_objects = candidates.len();
    let (guarded, primitives) = render_objects(candidates.into_iter(), style, view)?;
    let image = crop_image(
        &guarded,
        request.guard,
        request.guard,
        saved_width,
        saved_height,
    )?;
    let guarded_pixels = usize::try_from(surface_width)
        .ok()
        .and_then(|width| width.checked_mul(usize::try_from(surface_height).ok()?))
        .ok_or(RenderError::CapacityOverflow)?;
    Ok(TileRender {
        image,
        stats: TileRenderStats {
            candidate_objects,
            primitives,
            surface_width,
            surface_height,
            peak_pixel_buffer_bytes: guarded_pixels
                .checked_mul(6)
                .ok_or(RenderError::CapacityOverflow)?,
        },
    })
}

fn render_objects<'a>(
    objects: impl IntoIterator<Item = &'a WorldObject>,
    style: &StylePack,
    view: Viewport,
) -> Result<(IndexedImage, usize), RenderError> {
    let mut triangles = Vec::new();
    let mut shadows = Vec::new();
    for object in objects {
        append_object(&mut triangles, &mut shadows, object, style, view)?;
    }
    let primitive_count = triangles.len() + shadows.len();
    let palette_len = u8::try_from(style.palette.len()).map_err(|_| RenderError::PaletteIndex)?;
    let mut surface = RasterSurface::new(view.width, view.height, 0, palette_len)?;
    surface.rasterize(&triangles)?;
    let mut image = surface.into_image();
    let mut shadow_surface = RasterSurface::new(view.width, view.height, 15, palette_len)?;
    shadow_surface.rasterize(&shadows)?;
    composite_shadows(&mut image, shadow_surface.image(), style);
    drop(shadow_surface);
    apply_world_patterns(&mut image, view, style)?;
    apply_outlines(&mut image, style);
    Ok((image, primitive_count))
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
            let bounds = object_render_bounds(object, style)?;
            min_x = min_x.min(bounds.min_x);
            min_y = min_y.min(bounds.min_y);
            max_x = max_x.max(bounds.max_x);
            max_y = max_y.max(bounds.max_y);
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

    const fn from_layout(layout: RenderLayout) -> Self {
        Self {
            offset_x_subpx: layout.offset_x_subpx,
            offset_y_subpx: layout.offset_y_subpx,
            width: layout.width,
            height: layout.height,
        }
    }

    const fn into_layout(self) -> RenderLayout {
        RenderLayout {
            offset_x_subpx: self.offset_x_subpx,
            offset_y_subpx: self.offset_y_subpx,
            width: self.width,
            height: self.height,
        }
    }

    fn for_region(
        layout: RenderLayout,
        origin_x: i64,
        origin_y: i64,
        width: u32,
        height: u32,
        style: &StylePack,
    ) -> Result<Self, RenderError> {
        let scale = style.subpixels_per_pixel;
        Ok(Self {
            offset_x_subpx: layout
                .offset_x_subpx
                .checked_sub(
                    origin_x
                        .checked_mul(scale)
                        .ok_or(RenderError::ArithmeticOverflow)?,
                )
                .ok_or(RenderError::ArithmeticOverflow)?,
            offset_y_subpx: layout
                .offset_y_subpx
                .checked_sub(
                    origin_y
                        .checked_mul(scale)
                        .ok_or(RenderError::ArithmeticOverflow)?,
                )
                .ok_or(RenderError::ArithmeticOverflow)?,
            width,
            height,
        })
    }
}

#[derive(Clone, Copy)]
struct PixelBounds {
    min_x: i64,
    min_y: i64,
    max_x: i64,
    max_y: i64,
}

#[derive(Clone, Copy)]
struct ProjectedBounds {
    min_x: i64,
    min_y: i64,
    max_x: i64,
    max_y: i64,
}

fn object_render_bounds(
    object: &WorldObject,
    style: &StylePack,
) -> Result<ProjectedBounds, RenderError> {
    let source = object.bounds();
    let mut min_x = source.min_x_mm;
    let mut min_y = source.min_y_mm;
    let mut max_x = source.max_x_mm;
    let mut max_y = source.max_y_mm;
    let mut max_z = source.max_z_mm;

    if object.class() == SemanticClass::Vegetation {
        let radius = style.ordinary.tree_radius_mm + 1_000;
        min_x = min_x
            .checked_sub(radius)
            .ok_or(RenderError::ArithmeticOverflow)?;
        min_y = min_y
            .checked_sub(radius)
            .ok_or(RenderError::ArithmeticOverflow)?;
        max_x = max_x
            .checked_add(radius)
            .ok_or(RenderError::ArithmeticOverflow)?;
        max_y = max_y
            .checked_add(radius)
            .ok_or(RenderError::ArithmeticOverflow)?;
        max_z = max_z.max(
            source
                .min_z_mm
                .checked_add(style.ordinary.tree_height_mm)
                .ok_or(RenderError::ArithmeticOverflow)?,
        );
    }

    max_z = ordinary_roof_max_z(object, style, max_z)?;

    let landmark_extent = match object.name() {
        Some("Hoover Tower" | "Memorial Church") => 40_000,
        _ => 0,
    };
    if landmark_extent > 0 {
        let anchor = object.anchor();
        min_x = min_x.min(
            anchor
                .x_mm
                .checked_sub(landmark_extent)
                .ok_or(RenderError::ArithmeticOverflow)?,
        );
        min_y = min_y.min(
            anchor
                .y_mm
                .checked_sub(landmark_extent)
                .ok_or(RenderError::ArithmeticOverflow)?,
        );
        max_x = max_x.max(
            anchor
                .x_mm
                .checked_add(landmark_extent)
                .ok_or(RenderError::ArithmeticOverflow)?,
        );
        max_y = max_y.max(
            anchor
                .y_mm
                .checked_add(landmark_extent)
                .ok_or(RenderError::ArithmeticOverflow)?,
        );
        let landmark_height = if object.name() == Some("Hoover Tower") {
            style.landmarks.hoover_heights_mm[4]
        } else {
            style.landmarks.church_mm[0] + style.landmarks.church_mm[1]
        };
        max_z = max_z.max(
            source
                .min_z_mm
                .checked_add(landmark_height)
                .ok_or(RenderError::ArithmeticOverflow)?,
        );
    }

    if matches!(
        object.class(),
        SemanticClass::Building | SemanticClass::Vegetation
    ) {
        min_x = min_x.min(
            min_x
                .checked_add(style.ordinary.shadow_x_mm)
                .ok_or(RenderError::ArithmeticOverflow)?,
        );
        min_y = min_y.min(
            min_y
                .checked_add(style.ordinary.shadow_y_mm)
                .ok_or(RenderError::ArithmeticOverflow)?,
        );
        max_x = max_x.max(
            max_x
                .checked_add(style.ordinary.shadow_x_mm)
                .ok_or(RenderError::ArithmeticOverflow)?,
        );
        max_y = max_y.max(
            max_y
                .checked_add(style.ordinary.shadow_y_mm)
                .ok_or(RenderError::ArithmeticOverflow)?,
        );
    }

    projected_box(min_x, min_y, source.min_z_mm, max_x, max_y, max_z, style)
}

fn ordinary_roof_max_z(
    object: &WorldObject,
    style: &StylePack,
    source_max_z: i64,
) -> Result<i64, RenderError> {
    if object.class() != SemanticClass::Building || !style.ordinary.roof_details {
        return Ok(source_max_z);
    }
    source_max_z
        .checked_add(style.ordinary.roof_rise_mm)
        .ok_or(RenderError::ArithmeticOverflow)
}

#[allow(clippy::too_many_arguments)]
fn projected_box(
    min_x: i64,
    min_y: i64,
    min_z: i64,
    max_x: i64,
    max_y: i64,
    max_z: i64,
    style: &StylePack,
) -> Result<ProjectedBounds, RenderError> {
    let mut bounds = ProjectedBounds {
        min_x: i64::MAX,
        min_y: i64::MAX,
        max_x: i64::MIN,
        max_y: i64::MIN,
    };
    for x in [min_x, max_x] {
        for y in [min_y, max_y] {
            for z in [min_z, max_z] {
                let point = project(WorldPoint::new(x, y, z), style)?;
                bounds.min_x = bounds.min_x.min(point.x_subpx);
                bounds.min_y = bounds.min_y.min(point.y_subpx);
                bounds.max_x = bounds.max_x.max(point.x_subpx);
                bounds.max_y = bounds.max_y.max(point.y_subpx);
            }
        }
    }
    Ok(bounds)
}

fn object_intersects(
    object: &WorldObject,
    style: &StylePack,
    layout: RenderLayout,
    tile: PixelBounds,
) -> Result<bool, RenderError> {
    let object = object_render_bounds(object, style)?;
    let scale = style.subpixels_per_pixel;
    let tile_min_x = tile
        .min_x
        .checked_mul(scale)
        .ok_or(RenderError::ArithmeticOverflow)?;
    let tile_min_y = tile
        .min_y
        .checked_mul(scale)
        .ok_or(RenderError::ArithmeticOverflow)?;
    let tile_max_x = tile
        .max_x
        .checked_mul(scale)
        .ok_or(RenderError::ArithmeticOverflow)?;
    let tile_max_y = tile
        .max_y
        .checked_mul(scale)
        .ok_or(RenderError::ArithmeticOverflow)?;
    let min_x = object
        .min_x
        .checked_add(layout.offset_x_subpx)
        .ok_or(RenderError::ArithmeticOverflow)?;
    let min_y = object
        .min_y
        .checked_add(layout.offset_y_subpx)
        .ok_or(RenderError::ArithmeticOverflow)?;
    let max_x = object
        .max_x
        .checked_add(layout.offset_x_subpx)
        .ok_or(RenderError::ArithmeticOverflow)?;
    let max_y = object
        .max_y
        .checked_add(layout.offset_y_subpx)
        .ok_or(RenderError::ArithmeticOverflow)?;
    Ok(max_x >= tile_min_x && min_x <= tile_max_x && max_y >= tile_min_y && min_y <= tile_max_y)
}

fn crop_image(
    source: &IndexedImage,
    x: u32,
    y: u32,
    width: u32,
    height: u32,
) -> Result<IndexedImage, RenderError> {
    if x.checked_add(width)
        .is_none_or(|right| right > source.width())
        || y.checked_add(height)
            .is_none_or(|bottom| bottom > source.height())
    {
        return Err(RenderError::InvalidTileRequest);
    }
    let mut output = IndexedImage::new(width, height, 0)?;
    let source_width =
        usize::try_from(source.width()).map_err(|_| RenderError::InvalidDimensions)?;
    let output_width = usize::try_from(width).map_err(|_| RenderError::InvalidDimensions)?;
    let x = usize::try_from(x).map_err(|_| RenderError::InvalidDimensions)?;
    let y = usize::try_from(y).map_err(|_| RenderError::InvalidDimensions)?;
    for row in 0..usize::try_from(height).map_err(|_| RenderError::InvalidDimensions)? {
        let source_start = (y + row) * source_width + x;
        let target_start = row * output_width;
        output.pixels_mut()[target_start..target_start + output_width]
            .copy_from_slice(&source.pixels()[source_start..source_start + output_width]);
    }
    Ok(output)
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
    shadows: &mut Vec<Triangle>,
    object: &WorldObject,
    style: &StylePack,
    view: Viewport,
) -> Result<(), RenderError> {
    let polygons: &[Polygon] = match object.geometry() {
        Geometry::Polygon(polygon) => std::slice::from_ref(polygon),
        Geometry::MultiPolygon(polygons) => polygons,
    };
    let mut ordinal = 0_u32;
    let mut shadow_ordinal = 0_u32;
    for polygon in polygons {
        let base_rings = scene_rings(polygon, 0, object.class(), style, view)?;
        append_polygon_fill(
            output,
            &base_rings,
            ground_color(object.class(), style),
            object.id(),
            PASS_GROUND,
            &mut ordinal,
        )?;
        if object.class() == SemanticClass::Vegetation {
            append_tree_grove(
                output,
                shadows,
                polygon,
                object,
                style,
                view,
                &mut ordinal,
                &mut shadow_ordinal,
            )?;
        }
        if object.class() != SemanticClass::Building || object.height_mm() == 0 {
            continue;
        }
        let shadow_rings = shifted_scene_rings(
            polygon,
            style.ordinary.shadow_x_mm,
            style.ordinary.shadow_y_mm,
            style,
            view,
        )?;
        append_polygon_fill(
            shadows,
            &shadow_rings,
            style.ordinary.shadow,
            object.id(),
            PASS_SHADOW,
            &mut shadow_ordinal,
        )?;
        if landmarks::append_hero_landmark(output, polygon, object, style, view, &mut ordinal)? {
            continue;
        }
        append_walls(output, polygon, object, style, view, &mut ordinal)?;
        if style.ordinary.facade_details {
            append_facade_details(output, polygon, object, style, view, &mut ordinal)?;
        }
        if !style.ordinary.roof_details
            || !append_hip_roof(output, polygon, object, style, view, &mut ordinal)?
        {
            let roof_rings = scene_rings(
                polygon,
                i64::from(object.height_mm()),
                SemanticClass::Building,
                style,
                view,
            )?;
            append_polygon_fill(
                output,
                &roof_rings,
                style.ordinary.building[0],
                object.id(),
                PASS_ROOF,
                &mut ordinal,
            )?;
        }
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
                style.ordinary.building[1]
            } else {
                style.ordinary.building[2]
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

fn append_facade_details(
    output: &mut Vec<Triangle>,
    polygon: &Polygon,
    object: &WorldObject,
    style: &StylePack,
    view: Viewport,
    ordinal: &mut u32,
) -> Result<(), RenderError> {
    let height = i64::from(object.height_mm());
    if height < 4_500 {
        return Ok(());
    }
    let floor_count = ((height - 2_000) / style.ordinary.facade_floor_spacing_mm).clamp(1, 8);
    let mut detail_count = 0_usize;
    'facades: for ring in polygon.rings() {
        for edge in ring.points().windows(2) {
            let dx = edge[1].x_mm - edge[0].x_mm;
            let dy = edge[1].y_mm - edge[0].y_mm;
            let edge_length = dx.abs().max(dy.abs());
            if edge_length < style.ordinary.window_mm[0] * 2 {
                continue;
            }
            let bay_count = (edge_length / style.ordinary.facade_bay_spacing_mm).clamp(1, 16);
            let half_width = (style.ordinary.window_mm[0] / 2)
                .min(edge_length / (bay_count + 1) / 3)
                .max(250);
            let window_color = style.ordinary.windows[usize::from(dx < dy)];
            for floor in 0..floor_count {
                let bottom = 1_500 + floor * style.ordinary.facade_floor_spacing_mm;
                let top = (bottom + style.ordinary.window_mm[1]).min(height - 750);
                if top <= bottom {
                    continue;
                }
                for bay in 0..bay_count {
                    if detail_count == MAX_FACADE_QUADS_PER_OBJECT {
                        break 'facades;
                    }
                    let center = point_between(edge[0], edge[1], bay + 1, bay_count + 1)?;
                    let along_x = dx
                        .checked_mul(half_width)
                        .ok_or(RenderError::ArithmeticOverflow)?
                        / edge_length;
                    let along_y = dy
                        .checked_mul(half_width)
                        .ok_or(RenderError::ArithmeticOverflow)?
                        / edge_length;
                    append_facade_quad(
                        output,
                        WorldPoint::new(center.x_mm - along_x, center.y_mm - along_y, center.z_mm),
                        WorldPoint::new(center.x_mm + along_x, center.y_mm + along_y, center.z_mm),
                        bottom + 50,
                        top + 50,
                        window_color,
                        object.id(),
                        style,
                        view,
                        ordinal,
                    )?;
                    detail_count += 1;
                }
            }
        }
    }

    if let Some(edge) = polygon.rings()[0].points().windows(2).max_by_key(|edge| {
        (edge[1].x_mm - edge[0].x_mm)
            .abs()
            .max((edge[1].y_mm - edge[0].y_mm).abs())
    }) {
        let dx = edge[1].x_mm - edge[0].x_mm;
        let dy = edge[1].y_mm - edge[0].y_mm;
        let edge_length = dx.abs().max(dy.abs());
        if edge_length >= style.ordinary.door_mm[0] * 2 {
            let center = point_between(edge[0], edge[1], 1, 2)?;
            let half_width = style.ordinary.door_mm[0] / 2;
            let along_x = dx
                .checked_mul(half_width)
                .ok_or(RenderError::ArithmeticOverflow)?
                / edge_length;
            let along_y = dy
                .checked_mul(half_width)
                .ok_or(RenderError::ArithmeticOverflow)?
                / edge_length;
            append_facade_quad(
                output,
                WorldPoint::new(center.x_mm - along_x, center.y_mm - along_y, center.z_mm),
                WorldPoint::new(center.x_mm + along_x, center.y_mm + along_y, center.z_mm),
                100,
                style.ordinary.door_mm[1].min(height - 500),
                style.ordinary.door,
                object.id(),
                style,
                view,
                ordinal,
            )?;
        }
    }
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn append_facade_quad(
    output: &mut Vec<Triangle>,
    left: WorldPoint,
    right: WorldPoint,
    bottom: i64,
    top: i64,
    color: u8,
    object_id: ObjectId,
    style: &StylePack,
    view: Viewport,
    ordinal: &mut u32,
) -> Result<(), RenderError> {
    let vertices = [
        scene_vertex(left, bottom, SemanticClass::Building, style, view)?,
        scene_vertex(right, bottom, SemanticClass::Building, style, view)?,
        scene_vertex(right, top, SemanticClass::Building, style, view)?,
        scene_vertex(left, top, SemanticClass::Building, style, view)?,
    ];
    push_triangle(
        output,
        [vertices[0], vertices[1], vertices[2]],
        color,
        object_id,
        PASS_ORDINARY_DETAIL,
        ordinal,
    );
    push_triangle(
        output,
        [vertices[0], vertices[2], vertices[3]],
        color,
        object_id,
        PASS_ORDINARY_DETAIL,
        ordinal,
    );
    Ok(())
}

fn append_hip_roof(
    output: &mut Vec<Triangle>,
    polygon: &Polygon,
    object: &WorldObject,
    style: &StylePack,
    view: Viewport,
    ordinal: &mut u32,
) -> Result<bool, RenderError> {
    if polygon.rings().len() != 1 || !ring_is_convex(polygon.rings()[0].points()) {
        return Ok(false);
    }
    let ring = polygon.rings()[0].points();
    let vertices = &ring[..ring.len() - 1];
    if vertices.len() > MAX_HIP_ROOF_PLANES {
        return Ok(false);
    }
    let count = i64::try_from(vertices.len()).map_err(|_| RenderError::ArithmeticOverflow)?;
    let center = WorldPoint::new(
        vertices
            .iter()
            .try_fold(0_i64, |sum, point| sum.checked_add(point.x_mm))
            .ok_or(RenderError::ArithmeticOverflow)?
            / count,
        vertices
            .iter()
            .try_fold(0_i64, |sum, point| sum.checked_add(point.y_mm))
            .ok_or(RenderError::ArithmeticOverflow)?
            / count,
        vertices
            .iter()
            .try_fold(0_i64, |sum, point| sum.checked_add(point.z_mm))
            .ok_or(RenderError::ArithmeticOverflow)?
            / count,
    );
    let wall_height = i64::from(object.height_mm());
    let roof_rise = (wall_height / 3).clamp(1_500, style.ordinary.roof_rise_mm);
    let apex = scene_vertex(
        center,
        wall_height
            .checked_add(roof_rise)
            .ok_or(RenderError::ArithmeticOverflow)?,
        SemanticClass::Building,
        style,
        view,
    )?;
    for (index, edge) in ring.windows(2).enumerate() {
        push_triangle(
            output,
            [
                scene_vertex(edge[0], wall_height, SemanticClass::Building, style, view)?,
                scene_vertex(edge[1], wall_height, SemanticClass::Building, style, view)?,
                apex,
            ],
            style.ordinary.roof_faces[(index + usize::from(object.id().variation(2))) % 2],
            object.id(),
            PASS_ROOF,
            ordinal,
        );
    }
    Ok(true)
}

fn ring_is_convex(points: &[WorldPoint]) -> bool {
    let vertices = &points[..points.len().saturating_sub(1)];
    if vertices.len() < 3 {
        return false;
    }
    let mut sign = 0_i8;
    for index in 0..vertices.len() {
        let a = vertices[index];
        let b = vertices[(index + 1) % vertices.len()];
        let c = vertices[(index + 2) % vertices.len()];
        let cross = i128::from(b.x_mm - a.x_mm) * i128::from(c.y_mm - b.y_mm)
            - i128::from(b.y_mm - a.y_mm) * i128::from(c.x_mm - b.x_mm);
        if cross == 0 {
            continue;
        }
        let current = if cross > 0 { 1 } else { -1 };
        if sign != 0 && sign != current {
            return false;
        }
        sign = current;
    }
    sign != 0
}

fn point_between(
    start: WorldPoint,
    end: WorldPoint,
    numerator: i64,
    denominator: i64,
) -> Result<WorldPoint, RenderError> {
    fn component(
        start: i64,
        end: i64,
        numerator: i64,
        denominator: i64,
    ) -> Result<i64, RenderError> {
        let value = i128::from(end)
            .checked_sub(i128::from(start))
            .ok_or(RenderError::ArithmeticOverflow)?
            .checked_mul(i128::from(numerator))
            .and_then(|delta| i128::from(start).checked_add(delta / i128::from(denominator)))
            .ok_or(RenderError::ArithmeticOverflow)?;
        i64::try_from(value).map_err(|_| RenderError::ArithmeticOverflow)
    }
    Ok(WorldPoint::new(
        component(start.x_mm, end.x_mm, numerator, denominator)?,
        component(start.y_mm, end.y_mm, numerator, denominator)?,
        component(start.z_mm, end.z_mm, numerator, denominator)?,
    ))
}

#[allow(clippy::too_many_arguments)]
fn append_tree_grove(
    output: &mut Vec<Triangle>,
    shadows: &mut Vec<Triangle>,
    polygon: &Polygon,
    object: &WorldObject,
    style: &StylePack,
    view: Viewport,
    ordinal: &mut u32,
    shadow_ordinal: &mut u32,
) -> Result<(), RenderError> {
    for center in tree_centers(polygon, object.id(), style.ordinary.tree_spacing_mm) {
        append_tree_crown(output, center, object, style, view, ordinal)?;
        let shadow_ring = crown_ring(
            WorldPoint::new(
                center.x_mm + style.ordinary.shadow_x_mm,
                center.y_mm + style.ordinary.shadow_y_mm,
                0,
            ),
            style.ordinary.tree_radius_mm,
            0,
        )
        .into_iter()
        .map(|point| scene_vertex(point, 0, SemanticClass::Terrain, style, view))
        .collect::<Result<Vec<_>, _>>()?;
        append_polygon_fill(
            shadows,
            &[shadow_ring],
            style.ordinary.shadow,
            object.id(),
            PASS_SHADOW,
            shadow_ordinal,
        )?;
    }
    Ok(())
}

fn append_tree_crown(
    output: &mut Vec<Triangle>,
    center: WorldPoint,
    object: &WorldObject,
    style: &StylePack,
    view: Viewport,
    ordinal: &mut u32,
) -> Result<(), RenderError> {
    let height = i64::from(object.height_mm()).max(style.ordinary.tree_height_mm);
    let radius = style.ordinary.tree_radius_mm + i64::from(object.id().variation(3)) * 500;
    let lower = crown_ring(center, radius, height / 3);
    let upper = crown_ring(center, radius * 2 / 3, height * 5 / 6);
    let apex = scene_vertex(center, height, SemanticClass::Vegetation, style, view)?;
    for index in 0..8 {
        let next = index + 1;
        let low_left = scene_vertex(lower[index], 0, SemanticClass::Vegetation, style, view)?;
        let low_right = scene_vertex(lower[next], 0, SemanticClass::Vegetation, style, view)?;
        let high_left = scene_vertex(upper[index], 0, SemanticClass::Vegetation, style, view)?;
        let high_right = scene_vertex(upper[next], 0, SemanticClass::Vegetation, style, view)?;
        let color = style.ordinary.canopy
            [(index + usize::from(object.id().variation(4))) % style.ordinary.canopy.len()];
        push_triangle(
            output,
            [low_left, low_right, high_right],
            color,
            object.id(),
            PASS_CROWN,
            ordinal,
        );
        push_triangle(
            output,
            [low_left, high_right, high_left],
            color,
            object.id(),
            PASS_CROWN,
            ordinal,
        );
        push_triangle(
            output,
            [high_left, high_right, apex],
            style.ordinary.canopy[index % style.ordinary.canopy.len()],
            object.id(),
            PASS_CROWN,
            ordinal,
        );
    }
    Ok(())
}

fn crown_ring(center: WorldPoint, radius: i64, z_mm: i64) -> Vec<WorldPoint> {
    const DIRECTIONS: [(i64, i64); 9] = [
        (1_000, 0),
        (707, 707),
        (0, 1_000),
        (-707, 707),
        (-1_000, 0),
        (-707, -707),
        (0, -1_000),
        (707, -707),
        (1_000, 0),
    ];
    DIRECTIONS
        .into_iter()
        .map(|(x, y)| {
            WorldPoint::new(
                center.x_mm + x * radius / 1_000,
                center.y_mm + y * radius / 1_000,
                center.z_mm + z_mm,
            )
        })
        .collect()
}

fn tree_centers(polygon: &Polygon, object_id: ObjectId, spacing: i64) -> Vec<WorldPoint> {
    let outer = &polygon.rings()[0];
    let min_x = outer
        .points()
        .iter()
        .map(|point| point.x_mm)
        .min()
        .unwrap_or(0);
    let max_x = outer
        .points()
        .iter()
        .map(|point| point.x_mm)
        .max()
        .unwrap_or(0);
    let min_y = outer
        .points()
        .iter()
        .map(|point| point.y_mm)
        .min()
        .unwrap_or(0);
    let max_y = outer
        .points()
        .iter()
        .map(|point| point.y_mm)
        .max()
        .unwrap_or(0);
    let mut centers = Vec::new();
    let mut x = min_x.div_euclid(spacing) * spacing;
    while x <= max_x {
        let mut y = min_y.div_euclid(spacing) * spacing;
        while y <= max_y {
            let seed =
                mix64(object_id.get() ^ x.cast_unsigned().rotate_left(17) ^ y.cast_unsigned());
            let jitter_span = spacing / 5;
            let jitter_x = i64::try_from(seed % u64::try_from(jitter_span * 2 + 1).unwrap_or(1))
                .unwrap_or(0)
                - jitter_span;
            let jitter_y = i64::try_from(
                seed.rotate_left(29) % u64::try_from(jitter_span * 2 + 1).unwrap_or(1),
            )
            .unwrap_or(0)
                - jitter_span;
            let candidate = WorldPoint::new(x + jitter_x, y + jitter_y, 0);
            if point_in_polygon(candidate, polygon) {
                centers.push(candidate);
            }
            y = y.saturating_add(spacing);
        }
        x = x.saturating_add(spacing);
    }
    centers
}

fn point_in_polygon(point: WorldPoint, polygon: &Polygon) -> bool {
    let mut rings = polygon.rings().iter();
    let Some(outer) = rings.next() else {
        return false;
    };
    point_in_ring(point, outer.points()) && rings.all(|ring| !point_in_ring(point, ring.points()))
}

fn point_in_ring(point: WorldPoint, ring: &[WorldPoint]) -> bool {
    let mut inside = false;
    for edge in ring.windows(2) {
        let (a, b) = (edge[0], edge[1]);
        if (a.y_mm > point.y_mm) == (b.y_mm > point.y_mm) {
            continue;
        }
        let left = i128::from(point.x_mm - a.x_mm) * i128::from(b.y_mm - a.y_mm);
        let right = i128::from(b.x_mm - a.x_mm) * i128::from(point.y_mm - a.y_mm);
        if (b.y_mm > a.y_mm && left < right) || (b.y_mm < a.y_mm && left > right) {
            inside = !inside;
        }
    }
    inside
}

const fn mix64(mut value: u64) -> u64 {
    value ^= value >> 30;
    value = value.wrapping_mul(0xbf58_476d_1ce4_e5b9);
    value ^= value >> 27;
    value = value.wrapping_mul(0x94d0_49bb_1331_11eb);
    value ^ (value >> 31)
}

fn shifted_scene_rings(
    polygon: &Polygon,
    x_mm: i64,
    y_mm: i64,
    style: &StylePack,
    view: Viewport,
) -> Result<Vec<Vec<RasterVertex>>, RenderError> {
    polygon
        .rings()
        .iter()
        .map(|ring| {
            ring.points()
                .iter()
                .map(|point| {
                    scene_vertex(
                        WorldPoint::new(point.x_mm + x_mm, point.y_mm + y_mm, point.z_mm),
                        0,
                        SemanticClass::Terrain,
                        style,
                        view,
                    )
                })
                .collect()
        })
        .collect()
}

fn composite_shadows(image: &mut IndexedImage, shadows: &IndexedImage, style: &StylePack) {
    for (target, &shadow) in image.pixels_mut().iter_mut().zip(shadows.pixels()) {
        if shadow == style.ordinary.shadow && shadow_eligible(*target, style) {
            *target = style.ordinary.shadow;
        }
    }
}

fn shadow_eligible(index: u8, style: &StylePack) -> bool {
    style.ordinary.terrain.contains(&index)
        || style.ordinary.athletic.contains(&index)
        || style.ordinary.parking.contains(&index)
        || index == style.ordinary.road
        || index == style.ordinary.path
        || matches!(index, 4 | 14)
}

fn apply_world_patterns(
    image: &mut IndexedImage,
    view: Viewport,
    style: &StylePack,
) -> Result<(), RenderError> {
    let width = usize::try_from(image.width()).map_err(|_| RenderError::InvalidDimensions)?;
    let scale = style.subpixels_per_pixel;
    for (index, pixel) in image.pixels_mut().iter_mut().enumerate() {
        let x = i64::try_from(index % width).map_err(|_| RenderError::ArithmeticOverflow)?;
        let y = i64::try_from(index / width).map_err(|_| RenderError::ArithmeticOverflow)?;
        let absolute_x = x * scale - view.offset_x_subpx;
        let absolute_y = y * scale - view.offset_y_subpx;
        let pattern =
            mix64(absolute_x.cast_unsigned().rotate_left(13) ^ absolute_y.cast_unsigned());
        if *pixel == style.ordinary.terrain[0]
            && pattern.is_multiple_of(u64::from(style.ordinary.terrain_dither_period))
        {
            *pixel = style.ordinary.terrain[1];
        } else if *pixel == style.ordinary.athletic[0]
            && pattern.is_multiple_of(u64::from(style.ordinary.athletic_dither_period))
        {
            *pixel = style.ordinary.athletic[1];
        } else if *pixel == style.ordinary.parking[0]
            && (absolute_x.div_euclid(scale) + absolute_y.div_euclid(scale))
                .rem_euclid(i64::from(style.ordinary.parking_line_period))
                == 0
        {
            *pixel = style.ordinary.parking[1];
        }
    }
    Ok(())
}

fn apply_outlines(image: &mut IndexedImage, style: &StylePack) {
    let source = image.pixels().to_vec();
    let width = usize::try_from(image.width()).expect("bounded width fits usize");
    let height = usize::try_from(image.height()).expect("bounded height fits usize");
    for y in 1..height.saturating_sub(1) {
        for x in 1..width.saturating_sub(1) {
            let index = y * width + x;
            let family = outline_family(source[index], style);
            if family == 0 {
                continue;
            }
            let neighbours = [index - 1, index + 1, index - width, index + width];
            if neighbours
                .into_iter()
                .any(|other| outline_family(source[other], style) != family)
            {
                image.pixels_mut()[index] = style.ordinary.outline;
            }
        }
    }
}

fn outline_family(index: u8, style: &StylePack) -> u8 {
    if style.ordinary.building.contains(&index)
        || (style.ordinary.roof_details && style.ordinary.roof_faces.contains(&index))
        || (style.ordinary.facade_details
            && (style.ordinary.windows.contains(&index) || index == style.ordinary.door))
    {
        1
    } else if style.ordinary.canopy.contains(&index) {
        2
    } else {
        0
    }
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

fn ground_color(class: SemanticClass, style: &StylePack) -> u8 {
    match class {
        SemanticClass::Terrain => style.ordinary.terrain[0],
        SemanticClass::Water => 4,
        SemanticClass::Road => style.ordinary.road,
        SemanticClass::Parking => style.ordinary.parking[0],
        SemanticClass::Path => style.ordinary.path,
        SemanticClass::AthleticSurface => style.ordinary.athletic[0],
        SemanticClass::Building => style.ordinary.building[1],
        SemanticClass::Vegetation => style.ordinary.canopy[0],
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
        assert_eq!((first.width(), first.height()), (1_954, 880));
        assert_eq!(crate::stable_hash(first.pixels()), 0xa9ed_798e_f548_8603);
        assert!(first.pixels().contains(&5));
        assert!(first.pixels().contains(&9));
        assert!(first.pixels().contains(&style.ordinary.shadow));
        assert!(first.pixels().contains(&style.ordinary.outline));
        assert!(first.pixels().contains(&style.ordinary.canopy[1]));
    }

    #[test]
    fn guarded_tiles_reassemble_to_the_exact_full_scene() {
        let compiled = isometric_world::compile_hero(OSM, OVERTURE).expect("world compiles");
        for style in [
            StylePack::stanford_v1(),
            StylePack::stanford_v1_candidate_b(),
        ] {
            let layout = render_layout(&compiled.world, &style).expect("layout");
            let full = render_world(&compiled.world, &style).expect("full render");
            let guard = required_tile_guard(&style).expect("guard");
            let tile_size = 512;
            let columns = layout.width().div_ceil(tile_size);
            let rows = layout.height().div_ceil(tile_size);
            let mut assembled =
                IndexedImage::new(layout.width(), layout.height(), 0).expect("image");
            let mut saw_spatial_filter = false;

            for row in 0..rows {
                for column in 0..columns {
                    let request = TileRequest {
                        column,
                        row,
                        tile_size,
                        guard,
                    };
                    let tile = render_tile(&compiled.world, &style, layout, request).expect("tile");
                    saw_spatial_filter |=
                        tile.stats.candidate_objects < compiled.world.objects().len();
                    assert!(tile.stats.surface_width <= tile_size + guard * 2);
                    assert!(tile.stats.surface_height <= tile_size + guard * 2);
                    assert_eq!(
                        tile.stats.peak_pixel_buffer_bytes,
                        usize::try_from(tile.stats.surface_width).expect("width")
                            * usize::try_from(tile.stats.surface_height).expect("height")
                            * 6
                    );
                    copy_tile(
                        &mut assembled,
                        &tile.image,
                        column * tile_size,
                        row * tile_size,
                    );
                }
            }

            assert!(saw_spatial_filter);
            assert_eq!(assembled, full);
        }
    }

    #[test]
    fn candidate_b_adds_deterministic_ordinary_detail() {
        let compiled = isometric_world::compile_hero(OSM, OVERTURE).expect("world compiles");
        let style = StylePack::stanford_v1_candidate_b();
        let first = render_world(&compiled.world, &style).expect("Candidate B renders");
        let second = render_world(&compiled.world, &style).expect("Candidate B rerenders");
        assert_eq!(first, second);
        assert!(
            style
                .ordinary
                .windows
                .iter()
                .any(|color| first.pixels().contains(color))
        );
        assert!(first.pixels().contains(&style.ordinary.door));
        assert!(
            style
                .ordinary
                .roof_faces
                .iter()
                .all(|color| first.pixels().contains(color))
        );
        assert!(first.pixels().contains(&style.ordinary.parking[1]));
        assert!(
            style
                .ordinary
                .canopy
                .iter()
                .all(|color| first.pixels().contains(color))
        );
    }

    #[test]
    fn guarded_tile_is_deterministic_and_rejects_unsafe_requests() {
        let compiled = isometric_world::compile_hero(OSM, OVERTURE).expect("world compiles");
        let style = StylePack::stanford_v1();
        let layout = render_layout(&compiled.world, &style).expect("layout");
        let guard = required_tile_guard(&style).expect("guard");
        let request = TileRequest {
            column: 1,
            row: 0,
            tile_size: 512,
            guard,
        };
        let first = render_tile(&compiled.world, &style, layout, request).expect("first");
        let second = render_tile(&compiled.world, &style, layout, request).expect("second");
        assert_eq!(first, second);

        let too_small = TileRequest {
            guard: guard - 1,
            ..request
        };
        assert_eq!(
            render_tile(&compiled.world, &style, layout, too_small),
            Err(RenderError::InvalidTileRequest)
        );
        let outside = TileRequest {
            column: layout.width().div_ceil(512),
            ..request
        };
        assert_eq!(
            render_tile(&compiled.world, &style, layout, outside),
            Err(RenderError::InvalidTileRequest)
        );
    }

    #[test]
    fn selected_region_is_deterministic_and_rejects_invalid_ids() {
        let compiled = isometric_world::compile_hero(OSM, OVERTURE).expect("world compiles");
        let style = StylePack::stanford_v1();
        let layout = render_layout(&compiled.world, &style).expect("layout");
        let object = &compiled.world.objects()[0];
        let (center_x, center_y) = layout
            .image_coordinates(object.anchor(), &style)
            .expect("image coordinates");
        let x = u32::try_from((center_x - 64).clamp(0, i64::from(layout.width() - 128)))
            .expect("bounded x");
        let y = u32::try_from((center_y - 64).clamp(0, i64::from(layout.height() - 128)))
            .expect("bounded y");
        let first = render_selected_region(
            &compiled.world,
            &style,
            layout,
            &[object.id()],
            x,
            y,
            128,
            128,
        )
        .expect("selected region");
        let second = render_selected_region(
            &compiled.world,
            &style,
            layout,
            &[object.id()],
            x,
            y,
            128,
            128,
        )
        .expect("selected region repeat");
        assert_eq!(first, second);

        let unknown = ObjectId::new(u64::MAX).expect("nonzero id");
        assert_eq!(
            render_selected_region(&compiled.world, &style, layout, &[unknown], x, y, 128, 128,),
            Err(RenderError::InvalidObjectSelection)
        );
        assert_eq!(
            render_selected_region(
                &compiled.world,
                &style,
                layout,
                &[object.id(), object.id()],
                x,
                y,
                128,
                128,
            ),
            Err(RenderError::InvalidObjectSelection)
        );
    }

    #[test]
    fn eight_k_scale_stays_bounded_to_one_guarded_tile() {
        let compiled = isometric_world::compile_hero(OSM, OVERTURE).expect("world compiles");
        let mut style = StylePack::stanford_v1();
        style.world_mm_per_half_step = 250;
        style.elevation_mm_per_pixel = 250;
        let layout = render_layout(&compiled.world, &style).expect("layout");
        assert!(layout.width() >= 7_500);
        assert!(layout.height() >= 3_200);
        let guard = required_tile_guard(&style).expect("guard");
        let tile = render_tile(
            &compiled.world,
            &style,
            layout,
            TileRequest {
                column: layout.width().div_ceil(512) / 2,
                row: layout.height().div_ceil(512) / 2,
                tile_size: 512,
                guard,
            },
        )
        .expect("high-resolution tile");
        assert_eq!((tile.image.width(), tile.image.height()), (512, 512));
        assert!(tile.stats.candidate_objects < compiled.world.objects().len());
        assert!(tile.stats.peak_pixel_buffer_bytes < 4 * 1_024 * 1_024);
    }

    #[test]
    #[ignore = "release-only full prototype throughput evidence"]
    fn eight_k_tile_set_is_deterministic_and_bounded() {
        let compiled = isometric_world::compile_hero(OSM, OVERTURE).expect("world compiles");
        let mut style = StylePack::stanford_v1();
        style.world_mm_per_half_step = 250;
        style.elevation_mm_per_pixel = 250;
        let layout = render_layout(&compiled.world, &style).expect("layout");
        let guard = required_tile_guard(&style).expect("guard");
        let columns = layout.width().div_ceil(512);
        let rows = layout.height().div_ceil(512);
        let mut hash = 0xcbf2_9ce4_8422_2325_u64;
        let mut max_pixel_bytes = 0;
        let mut max_candidates = 0;
        let mut primitives = 0_usize;
        for row in 0..rows {
            for column in 0..columns {
                let tile = render_tile(
                    &compiled.world,
                    &style,
                    layout,
                    TileRequest {
                        column,
                        row,
                        tile_size: 512,
                        guard,
                    },
                )
                .expect("tile");
                for byte in tile.image.pixels() {
                    hash ^= u64::from(*byte);
                    hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
                }
                max_pixel_bytes = max_pixel_bytes.max(tile.stats.peak_pixel_buffer_bytes);
                max_candidates = max_candidates.max(tile.stats.candidate_objects);
                primitives += tile.stats.primitives;
            }
        }
        eprintln!(
            "{}x{} pixels, {} tiles, guard {}, max {} pixel bytes, max {} objects, {} primitives, {hash:016x}",
            layout.width(),
            layout.height(),
            columns * rows,
            guard,
            max_pixel_bytes,
            max_candidates,
            primitives,
        );
        assert!(max_pixel_bytes < 4 * 1_024 * 1_024);
        assert!(max_candidates < compiled.world.objects().len());
        assert_eq!(hash, 0xbf06_04f6_8bc3_8d2c);
    }

    fn copy_tile(target: &mut IndexedImage, tile: &IndexedImage, x: u32, y: u32) {
        let target_width = usize::try_from(target.width()).expect("target width");
        let tile_width = usize::try_from(tile.width()).expect("tile width");
        let x = usize::try_from(x).expect("x");
        let y = usize::try_from(y).expect("y");
        for row in 0..usize::try_from(tile.height()).expect("tile height") {
            let source_start = row * tile_width;
            let target_start = (y + row) * target_width + x;
            target.pixels_mut()[target_start..target_start + tile_width]
                .copy_from_slice(&tile.pixels()[source_start..source_start + tile_width]);
        }
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

    #[test]
    fn tree_placement_is_stable_and_inside_source_polygons() {
        let compiled = isometric_world::compile_hero(OSM, OVERTURE).expect("world compiles");
        let style = StylePack::stanford_v1();
        let mut count = 0;
        for object in compiled
            .world
            .objects()
            .iter()
            .filter(|object| object.class() == SemanticClass::Vegetation)
        {
            let Geometry::Polygon(polygon) = object.geometry() else {
                continue;
            };
            let first = tree_centers(polygon, object.id(), style.ordinary.tree_spacing_mm);
            let second = tree_centers(polygon, object.id(), style.ordinary.tree_spacing_mm);
            assert_eq!(first, second);
            assert!(first.iter().all(|point| point_in_polygon(*point, polygon)));
            count += first.len();
        }
        assert!(count > 100);
    }

    #[test]
    fn material_pattern_is_anchored_to_absolute_projection() {
        let style = StylePack::stanford_v1();
        let mut first = IndexedImage::new(64, 1, style.ordinary.terrain[0]).expect("image");
        let mut shifted = first.clone();
        let base_view = Viewport {
            offset_x_subpx: 0,
            offset_y_subpx: 0,
            width: 64,
            height: 1,
        };
        let shifted_view = Viewport {
            offset_x_subpx: -style.subpixels_per_pixel,
            ..base_view
        };
        apply_world_patterns(&mut first, base_view, &style).expect("pattern");
        apply_world_patterns(&mut shifted, shifted_view, &style).expect("shifted pattern");
        assert_eq!(&first.pixels()[1..], &shifted.pixels()[..63]);
        assert!(first.pixels().contains(&style.ordinary.terrain[1]));
    }

    #[test]
    fn outlines_are_one_pixel_and_preserve_building_interior() {
        let style = StylePack::stanford_v1();
        let mut image = IndexedImage::new(5, 5, 0).expect("image");
        for y in 1..4 {
            for x in 1..4 {
                image.pixels_mut()[y * 5 + x] = style.ordinary.building[0];
            }
        }
        apply_outlines(&mut image, &style);
        assert_eq!(image.pixels()[2 * 5 + 2], style.ordinary.building[0]);
        assert_eq!(image.pixels()[7], style.ordinary.outline);
        assert_eq!(image.pixels()[2 * 5 + 1], style.ordinary.outline);
    }
}
