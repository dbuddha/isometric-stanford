//! Procedural style values that are independent from source imagery.

use core::fmt;

/// An RGB palette entry.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Rgb8 {
    /// Red channel.
    pub red: u8,
    /// Green channel.
    pub green: u8,
    /// Blue channel.
    pub blue: u8,
}

/// The immutable subset of a style pack needed by the bootstrap renderer.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct StylePack {
    /// Stable, versioned style identifier.
    pub id: &'static str,
    /// Fixed subpixels per screen pixel.
    pub subpixels_per_pixel: i64,
    /// World millimeters represented by one horizontal isometric half-step.
    pub world_mm_per_half_step: i64,
    /// Vertical exaggeration denominator.
    pub elevation_mm_per_pixel: i64,
    /// Complete indexed palette.
    pub palette: Vec<Rgb8>,
}

impl StylePack {
    /// Returns the original Stanford v1 bootstrap palette and camera contract.
    #[must_use]
    pub fn stanford_v1() -> Self {
        Self {
            id: "stanford_v1.bootstrap.1",
            subpixels_per_pixel: 256,
            world_mm_per_half_step: 1_000,
            elevation_mm_per_pixel: 1_000,
            palette: vec![
                rgb(239, 225, 190),
                rgb(185, 184, 139),
                rgb(111, 132, 88),
                rgb(70, 96, 66),
                rgb(81, 133, 151),
                rgb(171, 92, 69),
                rgb(121, 60, 49),
                rgb(231, 202, 157),
                rgb(202, 184, 150),
                rgb(128, 126, 115),
                rgb(84, 83, 78),
                rgb(54, 50, 48),
                rgb(211, 183, 98),
                rgb(157, 147, 91),
                rgb(245, 238, 211),
                rgb(35, 37, 37),
            ],
        }
    }

    /// Validates hard style invariants.
    ///
    /// # Errors
    ///
    /// Returns a [`StyleError`] when the palette or projection scales violate
    /// the hard style contract.
    pub fn validate(&self) -> Result<(), StyleError> {
        if self.palette.is_empty() || self.palette.len() > 128 {
            return Err(StyleError::PaletteSize);
        }
        if self.subpixels_per_pixel <= 0
            || self.world_mm_per_half_step <= 0
            || self.elevation_mm_per_pixel <= 0
        {
            return Err(StyleError::NonPositiveScale);
        }
        Ok(())
    }
}

const fn rgb(red: u8, green: u8, blue: u8) -> Rgb8 {
    Rgb8 { red, green, blue }
}

/// Style validation error.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum StyleError {
    /// The palette is empty or exceeds the 128-color contract.
    PaletteSize,
    /// A projection or logical-pixel scale is not positive.
    NonPositiveScale,
}

impl fmt::Display for StyleError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::PaletteSize => formatter.write_str("palette must contain 1 to 128 colors"),
            Self::NonPositiveScale => formatter.write_str("style scales must be positive"),
        }
    }
}

impl std::error::Error for StyleError {}

#[cfg(test)]
mod tests {
    use super::StylePack;

    #[test]
    fn bootstrap_style_is_valid() {
        StylePack::stanford_v1()
            .validate()
            .expect("style must validate");
    }
}
