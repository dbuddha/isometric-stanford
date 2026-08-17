//! Deterministic fixed-point reference renderer and production raster core.
//!
//! The original diamond-and-column grammar remains a small regression fixture.
//! Production scene passes target the bounded fixed-point triangle and integer
//! depth surface exposed by this crate.

use core::fmt;
use isometric_core::{ScreenPoint, WorldPoint};
use isometric_style::{Rgb8, StylePack};
use isometric_world::{SemanticClass, World};

mod raster;
mod scene;

pub use raster::{RasterSurface, RasterVertex, Triangle};
pub use scene::render_world;

/// Indexed-palette image with one byte per pixel.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct IndexedImage {
    width: u32,
    height: u32,
    pixels: Vec<u8>,
}

impl IndexedImage {
    /// Creates an image after validating its bounded dimensions.
    ///
    /// # Errors
    ///
    /// Returns [`RenderError::InvalidDimensions`] for zero or oversized
    /// dimensions and [`RenderError::CapacityOverflow`] when allocation size
    /// arithmetic overflows.
    pub fn new(width: u32, height: u32, background: u8) -> Result<Self, RenderError> {
        if width == 0 || height == 0 || width > 16_384 || height > 16_384 {
            return Err(RenderError::InvalidDimensions);
        }
        let length = usize::try_from(width)
            .ok()
            .and_then(|value| value.checked_mul(usize::try_from(height).ok()?))
            .ok_or(RenderError::CapacityOverflow)?;
        Ok(Self {
            width,
            height,
            pixels: vec![background; length],
        })
    }

    /// Returns the image width.
    #[must_use]
    pub const fn width(&self) -> u32 {
        self.width
    }

    /// Returns the image height.
    #[must_use]
    pub const fn height(&self) -> u32 {
        self.height
    }

    /// Returns the palette indexes in row-major order.
    #[must_use]
    pub fn pixels(&self) -> &[u8] {
        &self.pixels
    }

    fn set(&mut self, x: i64, y: i64, color: u8) {
        let Ok(x) = u32::try_from(x) else { return };
        let Ok(y) = u32::try_from(y) else { return };
        if x >= self.width || y >= self.height {
            return;
        }
        let index = usize::try_from(y).expect("bounded y fits usize")
            * usize::try_from(self.width).expect("bounded width fits usize")
            + usize::try_from(x).expect("bounded x fits usize");
        self.pixels[index] = color;
    }

    /// Serializes a lossless PPM preview while preserving palette membership.
    ///
    /// # Errors
    ///
    /// Returns [`RenderError::PaletteIndex`] if an indexed pixel is not present
    /// in `palette`.
    pub fn to_ppm(&self, palette: &[Rgb8]) -> Result<Vec<u8>, RenderError> {
        let mut bytes = format!("P6\n{} {}\n255\n", self.width, self.height).into_bytes();
        bytes.reserve(self.pixels.len() * 3);
        for &index in &self.pixels {
            let color = palette
                .get(index as usize)
                .ok_or(RenderError::PaletteIndex)?;
            bytes.extend_from_slice(&[color.red, color.green, color.blue]);
        }
        Ok(bytes)
    }
}

/// Projects integer millimeters into fixed subpixels with a 2:1 isometric camera.
///
/// # Errors
///
/// Returns [`RenderError::InvalidStyle`] for an invalid style or
/// [`RenderError::ArithmeticOverflow`] when checked projection math overflows.
pub fn project(point: WorldPoint, style: &StylePack) -> Result<ScreenPoint, RenderError> {
    style.validate().map_err(|_| RenderError::InvalidStyle)?;
    let scale = style.subpixels_per_pixel;
    let half_step = style.world_mm_per_half_step;
    let x = point
        .x_mm
        .checked_sub(point.y_mm)
        .ok_or(RenderError::ArithmeticOverflow)?;
    let xy = point
        .x_mm
        .checked_add(point.y_mm)
        .ok_or(RenderError::ArithmeticOverflow)?;
    let projected_x = x
        .checked_mul(scale)
        .ok_or(RenderError::ArithmeticOverflow)?
        / half_step;
    let projected_y = xy
        .checked_mul(scale)
        .ok_or(RenderError::ArithmeticOverflow)?
        / half_step
        / 2;
    let elevation = point
        .z_mm
        .checked_mul(scale)
        .ok_or(RenderError::ArithmeticOverflow)?
        / style.elevation_mm_per_pixel;
    Ok(ScreenPoint {
        x_subpx: projected_x,
        y_subpx: projected_y
            .checked_sub(elevation)
            .ok_or(RenderError::ArithmeticOverflow)?,
    })
}

/// Renders the current reference grammar into an indexed image.
///
/// # Errors
///
/// Returns a [`RenderError`] when style, dimensions, allocation, or projection
/// validation fails.
pub fn render_reference(
    world: &World,
    style: &StylePack,
    width: u32,
    height: u32,
) -> Result<IndexedImage, RenderError> {
    style.validate().map_err(|_| RenderError::InvalidStyle)?;
    let mut image = IndexedImage::new(width, height, 0)?;
    let origin_x = i64::from(width) / 2;
    let origin_y = i64::from(height) / 2;
    for object in world.objects() {
        let point = project(object.anchor(), style)?;
        let center_x = origin_x + point.x_subpx / style.subpixels_per_pixel;
        let center_y = origin_y + point.y_subpx / style.subpixels_per_pixel;
        let radius = i64::from(object.radius_mm() / 1_000).clamp(2, 48);
        let color = semantic_color(object.class(), object.id().variation(2));
        draw_diamond(&mut image, center_x, center_y, radius, color);
        if object.height_mm() > 0 {
            let top = i64::from(object.height_mm() / 1_000).clamp(1, 64);
            draw_column(&mut image, center_x, center_y, radius / 2, top, color);
        }
    }
    Ok(image)
}

fn draw_diamond(image: &mut IndexedImage, center_x: i64, center_y: i64, radius: i64, color: u8) {
    for y in -radius..=radius {
        let half_width = radius - y.abs();
        for x in -half_width..=half_width {
            image.set(center_x + x, center_y + y / 2, color);
        }
    }
}

fn draw_column(
    image: &mut IndexedImage,
    center_x: i64,
    center_y: i64,
    half_width: i64,
    height: i64,
    color: u8,
) {
    let shadow = color.saturating_add(1).min(15);
    for y in 0..height {
        for x in -half_width..=half_width {
            image.set(
                center_x + x,
                center_y - y,
                if x < 0 { shadow } else { color },
            );
        }
    }
}

const fn semantic_color(class: SemanticClass, variation: u8) -> u8 {
    match class {
        SemanticClass::Terrain => 1 + variation,
        SemanticClass::Water => 4,
        SemanticClass::Road | SemanticClass::Parking => 9,
        SemanticClass::Path => 8,
        SemanticClass::AthleticSurface => 12,
        SemanticClass::Building => 5,
        SemanticClass::Vegetation => 2 + variation,
        SemanticClass::Unknown => 15,
    }
}

/// Stable 64-bit FNV-1a used for regression fixtures, not cryptography.
#[must_use]
pub fn stable_hash(bytes: &[u8]) -> u64 {
    let mut hash = 0xcbf2_9ce4_8422_2325_u64;
    for byte in bytes {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
    }
    hash
}

/// Reference renderer errors.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RenderError {
    /// An image dimension is zero or above the bounded maximum.
    InvalidDimensions,
    /// Pixel capacity overflowed the host address space.
    CapacityOverflow,
    /// Fixed-point projection overflowed.
    ArithmeticOverflow,
    /// Style validation failed.
    InvalidStyle,
    /// A pixel references a palette entry that does not exist.
    PaletteIndex,
    /// A triangle has zero signed area.
    DegenerateTriangle,
    /// A primitive key is zero, duplicated, or otherwise noncanonical.
    InvalidPrimitiveKey,
    /// A vertex exceeds the bounded raster coordinate contract.
    InvalidVertex,
    /// A surface was submitted more than once.
    SurfaceAlreadyRasterized,
    /// No renderable geometry was available for viewport construction.
    EmptyWorld,
    /// Polygon decomposition could not produce valid triangles.
    Triangulation,
}

impl fmt::Display for RenderError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::InvalidDimensions => "image dimensions are outside the accepted bounds",
            Self::CapacityOverflow => "image capacity overflow",
            Self::ArithmeticOverflow => "fixed-point projection overflow",
            Self::InvalidStyle => "style pack is invalid",
            Self::PaletteIndex => "rendered palette index does not exist",
            Self::DegenerateTriangle => "triangle has zero signed area",
            Self::InvalidPrimitiveKey => "primitive keys must be nonzero and unique",
            Self::InvalidVertex => "raster vertex exceeds the accepted coordinate range",
            Self::SurfaceAlreadyRasterized => "raster surface accepts one canonical batch",
            Self::EmptyWorld => "world contains no renderable geometry",
            Self::Triangulation => "polygon could not be decomposed deterministically",
        })
    }
}

impl std::error::Error for RenderError {}

#[cfg(test)]
mod tests {
    use super::{project, render_reference, stable_hash};
    use isometric_core::WorldPoint;
    use isometric_style::StylePack;
    use isometric_world::World;

    #[test]
    fn projection_has_two_to_one_axes() {
        let style = StylePack::stanford_v1();
        let east = project(WorldPoint::new(1_000, 0, 0), &style).expect("project");
        let north = project(WorldPoint::new(0, 1_000, 0), &style).expect("project");
        assert_eq!(east.x_subpx, 256);
        assert_eq!(north.x_subpx, -256);
        assert_eq!(east.y_subpx, 128);
        assert_eq!(north.y_subpx, 128);
    }

    #[test]
    fn reference_render_is_byte_deterministic() {
        let world = World::reference_fixture();
        let style = StylePack::stanford_v1();
        let first = render_reference(&world, &style, 128, 128).expect("first render");
        let second = render_reference(&world, &style, 128, 128).expect("second render");
        assert_eq!(first, second);
        assert_eq!(stable_hash(first.pixels()), 0x57c2_9b24_5605_1ff4);
    }

    #[test]
    fn every_pixel_is_in_the_palette() {
        let style = StylePack::stanford_v1();
        let image = render_reference(&World::reference_fixture(), &style, 128, 128)
            .expect("render must succeed");
        assert!(
            image
                .pixels()
                .iter()
                .all(|index| usize::from(*index) < style.palette.len())
        );
    }
}
