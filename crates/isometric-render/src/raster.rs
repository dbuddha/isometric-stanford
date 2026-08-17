//! Bounded fixed-point triangle rasterization with an integer depth buffer.

use std::collections::BTreeSet;

use crate::{IndexedImage, RenderError};

const SUBPIXELS_PER_PIXEL: i64 = 256;
const SAMPLE_OFFSET: i64 = SUBPIXELS_PER_PIXEL / 2;
const MAX_RASTER_DIMENSION: u32 = 4_096;
const MAX_VERTEX_SUBPIXELS: u64 = 1_000_000_000_000;

/// One fixed-point screen vertex and its integer depth value.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RasterVertex {
    /// Horizontal coordinate in 1/256 pixel subpixels.
    pub x_subpx: i64,
    /// Vertical coordinate in 1/256 pixel subpixels.
    pub y_subpx: i64,
    /// Integer depth. Larger values are closer to the camera.
    pub depth: i32,
}

impl RasterVertex {
    /// Constructs a fixed-point raster vertex.
    #[must_use]
    pub const fn new(x_subpx: i64, y_subpx: i64, depth: i32) -> Self {
        Self {
            x_subpx,
            y_subpx,
            depth,
        }
    }
}

/// One indexed triangle with a stable source-derived primitive key.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Triangle {
    /// Triangle vertices.
    pub vertices: [RasterVertex; 3],
    /// Output palette index.
    pub palette_index: u8,
    /// Nonzero stable key used to canonicalize equal-depth ownership.
    pub stable_key: u64,
}

/// A bounded raster target using five bytes per active pixel.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RasterSurface {
    image: IndexedImage,
    depth: Vec<i32>,
    palette_len: u8,
    rasterized: bool,
}

impl RasterSurface {
    /// Allocates one palette byte and one 32-bit depth value per pixel.
    ///
    /// # Errors
    ///
    /// Returns an error for invalid dimensions, palette bounds, or capacity
    /// overflow. Dimensions are capped at 4,096 because the production caller
    /// renders bounded guarded tiles rather than an estate-sized framebuffer.
    pub fn new(
        width: u32,
        height: u32,
        background: u8,
        palette_len: u8,
    ) -> Result<Self, RenderError> {
        if width == 0
            || height == 0
            || width > MAX_RASTER_DIMENSION
            || height > MAX_RASTER_DIMENSION
        {
            return Err(RenderError::InvalidDimensions);
        }
        if palette_len == 0 || palette_len > 128 || background >= palette_len {
            return Err(RenderError::PaletteIndex);
        }
        let length = usize::try_from(width)
            .ok()
            .and_then(|value| value.checked_mul(usize::try_from(height).ok()?))
            .ok_or(RenderError::CapacityOverflow)?;
        Ok(Self {
            image: IndexedImage::new(width, height, background)?,
            depth: vec![i32::MIN; length],
            palette_len,
            rasterized: false,
        })
    }

    /// Rasterizes a batch in stable-key order.
    ///
    /// Equal-depth fragments retain the lowest stable primitive key. This
    /// provides deterministic ownership without an additional owner buffer.
    ///
    /// # Errors
    ///
    /// Returns an error before drawing when any primitive key, palette index,
    /// vertex, or triangle area is invalid.
    pub fn rasterize(&mut self, triangles: &[Triangle]) -> Result<(), RenderError> {
        if self.rasterized {
            return Err(RenderError::SurfaceAlreadyRasterized);
        }
        validate_batch(triangles, self.palette_len)?;
        let mut ordered = triangles.iter().collect::<Vec<_>>();
        ordered.sort_by_key(|triangle| triangle.stable_key);
        for triangle in ordered {
            self.draw_triangle(*triangle)?;
        }
        self.rasterized = true;
        Ok(())
    }

    /// Returns the palette-indexed output.
    #[must_use]
    pub const fn image(&self) -> &IndexedImage {
        &self.image
    }

    /// Consumes the surface and returns its palette-indexed output.
    #[must_use]
    pub fn into_image(self) -> IndexedImage {
        self.image
    }

    /// Returns the exact owned pixel-buffer byte count, excluding vector headers.
    #[must_use]
    pub fn pixel_buffer_bytes(&self) -> usize {
        self.image.pixels.len() + self.depth.len() * size_of::<i32>()
    }

    fn draw_triangle(&mut self, triangle: Triangle) -> Result<(), RenderError> {
        let mut vertices = triangle.vertices;
        let mut area = edge(vertices[0], vertices[1], vertices[2]);
        if area == 0 {
            return Err(RenderError::DegenerateTriangle);
        }
        if area < 0 {
            vertices.swap(1, 2);
            area = area.checked_neg().ok_or(RenderError::ArithmeticOverflow)?;
        }

        let Some(bounds) = clipped_pixel_bounds(vertices, self.image.width, self.image.height)?
        else {
            return Ok(());
        };
        for y in bounds.min_y..=bounds.max_y {
            for x in bounds.min_x..=bounds.max_x {
                let sample = RasterVertex::new(
                    i64::from(x)
                        .checked_mul(SUBPIXELS_PER_PIXEL)
                        .and_then(|value| value.checked_add(SAMPLE_OFFSET))
                        .ok_or(RenderError::ArithmeticOverflow)?,
                    i64::from(y)
                        .checked_mul(SUBPIXELS_PER_PIXEL)
                        .and_then(|value| value.checked_add(SAMPLE_OFFSET))
                        .ok_or(RenderError::ArithmeticOverflow)?,
                    0,
                );
                let weights = [
                    edge(vertices[1], vertices[2], sample),
                    edge(vertices[2], vertices[0], sample),
                    edge(vertices[0], vertices[1], sample),
                ];
                if !inside(weights, vertices) {
                    continue;
                }
                let depth = interpolate_depth(vertices, weights, area)?;
                let index = usize::try_from(y).expect("bounded y fits usize")
                    * usize::try_from(self.image.width).expect("bounded width fits usize")
                    + usize::try_from(x).expect("bounded x fits usize");
                if depth > self.depth[index] {
                    self.depth[index] = depth;
                    self.image.pixels[index] = triangle.palette_index;
                }
            }
        }
        Ok(())
    }
}

#[derive(Clone, Copy)]
struct PixelBounds {
    min_x: u32,
    min_y: u32,
    max_x: u32,
    max_y: u32,
}

fn validate_batch(triangles: &[Triangle], palette_len: u8) -> Result<(), RenderError> {
    let mut keys = BTreeSet::new();
    for triangle in triangles {
        if triangle.stable_key == 0 || !keys.insert(triangle.stable_key) {
            return Err(RenderError::InvalidPrimitiveKey);
        }
        if triangle.palette_index >= palette_len {
            return Err(RenderError::PaletteIndex);
        }
        if triangle.vertices.iter().any(|vertex| {
            vertex.x_subpx.unsigned_abs() > MAX_VERTEX_SUBPIXELS
                || vertex.y_subpx.unsigned_abs() > MAX_VERTEX_SUBPIXELS
                || vertex.depth == i32::MIN
        }) {
            return Err(RenderError::InvalidVertex);
        }
        if edge(
            triangle.vertices[0],
            triangle.vertices[1],
            triangle.vertices[2],
        ) == 0
        {
            return Err(RenderError::DegenerateTriangle);
        }
    }
    Ok(())
}

fn clipped_pixel_bounds(
    vertices: [RasterVertex; 3],
    width: u32,
    height: u32,
) -> Result<Option<PixelBounds>, RenderError> {
    let min_x = vertices
        .iter()
        .map(|vertex| vertex.x_subpx)
        .min()
        .ok_or(RenderError::DegenerateTriangle)?;
    let max_x = vertices
        .iter()
        .map(|vertex| vertex.x_subpx)
        .max()
        .ok_or(RenderError::DegenerateTriangle)?;
    let min_y = vertices
        .iter()
        .map(|vertex| vertex.y_subpx)
        .min()
        .ok_or(RenderError::DegenerateTriangle)?;
    let max_y = vertices
        .iter()
        .map(|vertex| vertex.y_subpx)
        .max()
        .ok_or(RenderError::DegenerateTriangle)?;
    let last_x = i64::from(width - 1);
    let last_y = i64::from(height - 1);
    let pixel_min_x = floor_div(
        min_x
            .checked_sub(SAMPLE_OFFSET)
            .ok_or(RenderError::ArithmeticOverflow)?,
        SUBPIXELS_PER_PIXEL,
    )
    .clamp(0, last_x);
    let pixel_max_x = floor_div(
        max_x
            .checked_sub(SAMPLE_OFFSET)
            .ok_or(RenderError::ArithmeticOverflow)?,
        SUBPIXELS_PER_PIXEL,
    )
    .clamp(0, last_x);
    let pixel_min_y = floor_div(
        min_y
            .checked_sub(SAMPLE_OFFSET)
            .ok_or(RenderError::ArithmeticOverflow)?,
        SUBPIXELS_PER_PIXEL,
    )
    .clamp(0, last_y);
    let pixel_max_y = floor_div(
        max_y
            .checked_sub(SAMPLE_OFFSET)
            .ok_or(RenderError::ArithmeticOverflow)?,
        SUBPIXELS_PER_PIXEL,
    )
    .clamp(0, last_y);
    if max_x < SAMPLE_OFFSET
        || max_y < SAMPLE_OFFSET
        || min_x > last_x * SUBPIXELS_PER_PIXEL + SAMPLE_OFFSET
        || min_y > last_y * SUBPIXELS_PER_PIXEL + SAMPLE_OFFSET
        || pixel_min_x > pixel_max_x
        || pixel_min_y > pixel_max_y
    {
        return Ok(None);
    }
    Ok(Some(PixelBounds {
        min_x: u32::try_from(pixel_min_x).map_err(|_| RenderError::ArithmeticOverflow)?,
        min_y: u32::try_from(pixel_min_y).map_err(|_| RenderError::ArithmeticOverflow)?,
        max_x: u32::try_from(pixel_max_x).map_err(|_| RenderError::ArithmeticOverflow)?,
        max_y: u32::try_from(pixel_max_y).map_err(|_| RenderError::ArithmeticOverflow)?,
    }))
}

const fn edge(start: RasterVertex, end: RasterVertex, point: RasterVertex) -> i128 {
    (end.x_subpx as i128 - start.x_subpx as i128) * (point.y_subpx as i128 - start.y_subpx as i128)
        - (end.y_subpx as i128 - start.y_subpx as i128)
            * (point.x_subpx as i128 - start.x_subpx as i128)
}

fn inside(weights: [i128; 3], vertices: [RasterVertex; 3]) -> bool {
    weights.iter().enumerate().all(|(index, weight)| {
        *weight > 0
            || (*weight == 0
                && inclusive_edge(vertices[(index + 1) % 3], vertices[(index + 2) % 3]))
    })
}

const fn inclusive_edge(start: RasterVertex, end: RasterVertex) -> bool {
    start.y_subpx < end.y_subpx || (start.y_subpx == end.y_subpx && start.x_subpx < end.x_subpx)
}

fn interpolate_depth(
    vertices: [RasterVertex; 3],
    weights: [i128; 3],
    area: i128,
) -> Result<i32, RenderError> {
    let numerator = weights
        .iter()
        .zip(vertices)
        .try_fold(0_i128, |sum, (weight, vertex)| {
            sum.checked_add(
                weight
                    .checked_mul(i128::from(vertex.depth))
                    .ok_or(RenderError::ArithmeticOverflow)?,
            )
            .ok_or(RenderError::ArithmeticOverflow)
        })?;
    i32::try_from(numerator / area).map_err(|_| RenderError::ArithmeticOverflow)
}

const fn floor_div(value: i64, divisor: i64) -> i64 {
    let quotient = value / divisor;
    let remainder = value % divisor;
    if remainder < 0 {
        quotient - 1
    } else {
        quotient
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn vertex(x: i64, y: i64, depth: i32) -> RasterVertex {
        RasterVertex::new(x * SUBPIXELS_PER_PIXEL, y * SUBPIXELS_PER_PIXEL, depth)
    }

    fn triangle(vertices: [RasterVertex; 3], color: u8, key: u64) -> Triangle {
        Triangle {
            vertices,
            palette_index: color,
            stable_key: key,
        }
    }

    #[test]
    fn adjacent_triangles_fill_without_shared_edge_cracks() {
        let first = triangle([vertex(0, 0, 1), vertex(4, 0, 1), vertex(4, 4, 1)], 1, 1);
        let second = triangle([vertex(0, 0, 1), vertex(4, 4, 1), vertex(0, 4, 1)], 2, 2);
        let mut surface = RasterSurface::new(4, 4, 0, 3).expect("surface");
        surface.rasterize(&[second, first]).expect("rasterize");
        assert!(surface.image().pixels().iter().all(|pixel| *pixel != 0));
        assert_eq!(surface.pixel_buffer_bytes(), 4 * 4 * 5);
        assert_eq!(
            crate::stable_hash(surface.image().pixels()),
            0x3c59_1e65_e0cf_2fbd
        );
    }

    #[test]
    fn stable_key_resolves_equal_depth_independent_of_input_order() {
        let vertices = [vertex(0, 0, 5), vertex(4, 0, 5), vertex(0, 4, 5)];
        let low_key = triangle(vertices, 1, 10);
        let high_key = triangle(vertices, 2, 20);
        let mut first = RasterSurface::new(4, 4, 0, 3).expect("surface");
        let mut second = RasterSurface::new(4, 4, 0, 3).expect("surface");
        first.rasterize(&[high_key, low_key]).expect("rasterize");
        second.rasterize(&[low_key, high_key]).expect("rasterize");
        assert_eq!(first, second);
        assert!(first.image().pixels().contains(&1));
        assert!(!first.image().pixels().contains(&2));
    }

    #[test]
    fn closer_depth_wins_and_offscreen_geometry_is_clipped() {
        let far = triangle(
            [vertex(-10, -10, 1), vertex(20, -10, 1), vertex(-10, 20, 1)],
            1,
            1,
        );
        let near = triangle(
            [vertex(-10, -10, 2), vertex(20, -10, 2), vertex(-10, 20, 2)],
            2,
            2,
        );
        let mut surface = RasterSurface::new(4, 4, 0, 3).expect("surface");
        surface.rasterize(&[near, far]).expect("rasterize");
        assert!(surface.image().pixels().contains(&2));
        assert!(!surface.image().pixels().contains(&1));
    }

    #[test]
    fn invalid_batches_fail_before_mutating_output() {
        let mut surface = RasterSurface::new(4, 4, 0, 2).expect("surface");
        let original = surface.clone();
        let valid = triangle([vertex(0, 0, 1), vertex(4, 0, 1), vertex(0, 4, 1)], 1, 1);
        let duplicate = triangle([vertex(0, 0, 2), vertex(4, 0, 2), vertex(0, 4, 2)], 1, 1);
        assert_eq!(
            surface.rasterize(&[valid, duplicate]),
            Err(RenderError::InvalidPrimitiveKey)
        );
        assert_eq!(surface, original);

        let degenerate = triangle([vertex(0, 0, 1), vertex(1, 1, 1), vertex(2, 2, 1)], 1, 2);
        assert_eq!(
            surface.rasterize(&[degenerate]),
            Err(RenderError::DegenerateTriangle)
        );
        assert_eq!(surface, original);
    }

    #[test]
    fn surface_accepts_exactly_one_canonical_batch() {
        let valid = triangle([vertex(0, 0, 1), vertex(4, 0, 1), vertex(0, 4, 1)], 1, 1);
        let mut surface = RasterSurface::new(4, 4, 0, 2).expect("surface");
        surface.rasterize(&[valid]).expect("first batch");
        assert_eq!(
            surface.rasterize(&[]),
            Err(RenderError::SurfaceAlreadyRasterized)
        );
    }

    #[test]
    fn winding_does_not_change_coverage() {
        let clockwise = triangle([vertex(0, 0, 1), vertex(0, 4, 1), vertex(4, 0, 1)], 1, 1);
        let counter_clockwise = triangle([vertex(0, 0, 1), vertex(4, 0, 1), vertex(0, 4, 1)], 1, 1);
        let mut first = RasterSurface::new(4, 4, 0, 2).expect("surface");
        let mut second = RasterSurface::new(4, 4, 0, 2).expect("surface");
        first.rasterize(&[clockwise]).expect("rasterize");
        second.rasterize(&[counter_clockwise]).expect("rasterize");
        assert_eq!(first, second);
    }

    #[test]
    fn surface_rejects_unbounded_allocation() {
        assert_eq!(
            RasterSurface::new(MAX_RASTER_DIMENSION + 1, 1, 0, 1),
            Err(RenderError::InvalidDimensions)
        );
    }
}
