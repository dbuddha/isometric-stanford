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

/// Deterministic ordinary-scene grammar owned by the style pack.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct OrdinaryGrammar {
    /// Terrain fill and sparse alternate index.
    pub terrain: [u8; 2],
    /// Light and dark canopy indexes.
    pub canopy: [u8; 2],
    /// Light and dark athletic-surface indexes.
    pub athletic: [u8; 2],
    /// Roof, light wall, and dark wall indexes.
    pub building: [u8; 3],
    /// Hard cast-shadow palette index.
    pub shadow: u8,
    /// One-logical-pixel outline palette index.
    pub outline: u8,
    /// Shadow displacement east in world millimeters.
    pub shadow_x_mm: i64,
    /// Shadow displacement north in world millimeters.
    pub shadow_y_mm: i64,
    /// World-anchored tree placement grid spacing.
    pub tree_spacing_mm: i64,
    /// Nominal faceted crown radius.
    pub tree_radius_mm: i64,
    /// Nominal faceted crown height.
    pub tree_height_mm: i64,
    /// Sparse terrain dither period in projected logical pixels.
    pub terrain_dither_period: u8,
    /// Athletic-surface dither period in projected logical pixels.
    pub athletic_dither_period: u8,
}

/// Original parameterized grammar for the three prototype landmarks.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct LandmarkGrammar {
    /// Fixed-point campus east axis in thousandths.
    pub campus_u: [i64; 2],
    /// Fixed-point campus north axis in thousandths.
    pub campus_v: [i64; 2],
    /// Hoover base, shaft, crown, lantern, and total heights in millimeters.
    pub hoover_heights_mm: [i64; 5],
    /// Hoover shaft, crown, and lantern widths in millimeters.
    pub hoover_widths_mm: [i64; 3],
    /// Church wall height, roof rise, half-width, and half-length in millimeters.
    pub church_mm: [i64; 4],
    /// Reviewed Main Quad wall height in millimeters.
    pub main_quad_wall_height_mm: i64,
    /// Arcade opening spacing, width, shoulder height, and apex height.
    pub arcade_mm: [i64; 4],
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
    /// Ordinary Stanford scene grammar.
    pub ordinary: OrdinaryGrammar,
    /// Prototype landmark grammar.
    pub landmarks: LandmarkGrammar,
}

impl StylePack {
    /// Returns the original Stanford v1 bootstrap palette and camera contract.
    #[must_use]
    pub fn stanford_v1() -> Self {
        Self {
            id: "stanford_v1.landmarks.1",
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
            ordinary: OrdinaryGrammar {
                terrain: [1, 0],
                canopy: [2, 3],
                athletic: [12, 13],
                building: [5, 7, 6],
                shadow: 10,
                outline: 11,
                shadow_x_mm: 12_000,
                shadow_y_mm: -4_000,
                tree_spacing_mm: 16_000,
                tree_radius_mm: 6_500,
                tree_height_mm: 12_000,
                terrain_dither_period: 16,
                athletic_dither_period: 8,
            },
            landmarks: LandmarkGrammar {
                campus_u: [970, -242],
                campus_v: [242, 970],
                hoover_heights_mm: [10_000, 62_000, 70_000, 82_000, 87_000],
                hoover_widths_mm: [16_000, 26_000, 18_000],
                church_mm: [17_000, 14_000, 18_000, 29_000],
                main_quad_wall_height_mm: 12_000,
                arcade_mm: [6_000, 2_800, 4_500, 6_500],
            },
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
        let indexes = [
            self.ordinary.terrain[0],
            self.ordinary.terrain[1],
            self.ordinary.canopy[0],
            self.ordinary.canopy[1],
            self.ordinary.athletic[0],
            self.ordinary.athletic[1],
            self.ordinary.building[0],
            self.ordinary.building[1],
            self.ordinary.building[2],
            self.ordinary.shadow,
            self.ordinary.outline,
        ];
        if indexes
            .into_iter()
            .any(|index| usize::from(index) >= self.palette.len())
        {
            return Err(StyleError::PaletteIndex);
        }
        if self.ordinary.tree_spacing_mm <= 0
            || self.ordinary.tree_radius_mm <= 0
            || self.ordinary.tree_height_mm <= 0
            || self.ordinary.terrain_dither_period == 0
            || self.ordinary.athletic_dither_period == 0
        {
            return Err(StyleError::InvalidGrammar);
        }
        let landmark_values = self
            .landmarks
            .hoover_heights_mm
            .into_iter()
            .chain(self.landmarks.hoover_widths_mm)
            .chain(self.landmarks.church_mm)
            .chain(self.landmarks.arcade_mm);
        if landmark_values.into_iter().any(|value| value <= 0)
            || self.landmarks.campus_u == [0, 0]
            || self.landmarks.campus_v == [0, 0]
            || self.landmarks.main_quad_wall_height_mm <= 0
            || !self
                .landmarks
                .hoover_heights_mm
                .windows(2)
                .all(|pair| pair[0] < pair[1])
        {
            return Err(StyleError::InvalidLandmarkGrammar);
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
    /// A grammar palette index is not present in the palette.
    PaletteIndex,
    /// An ordinary-scene grammar measurement or period is invalid.
    InvalidGrammar,
    /// Landmark axes, dimensions, or ordered heights are invalid.
    InvalidLandmarkGrammar,
}

impl fmt::Display for StyleError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::PaletteSize => formatter.write_str("palette must contain 1 to 128 colors"),
            Self::NonPositiveScale => formatter.write_str("style scales must be positive"),
            Self::PaletteIndex => formatter.write_str("style grammar palette index is invalid"),
            Self::InvalidGrammar => formatter.write_str("style grammar values must be positive"),
            Self::InvalidLandmarkGrammar => {
                formatter.write_str("landmark grammar axes and dimensions are invalid")
            }
        }
    }
}

impl std::error::Error for StyleError {}

#[cfg(test)]
mod tests {
    use super::{StyleError, StylePack};

    #[test]
    fn bootstrap_style_is_valid() {
        StylePack::stanford_v1()
            .validate()
            .expect("style must validate");
    }

    #[test]
    fn grammar_indexes_and_measurements_are_validated() {
        let mut style = StylePack::stanford_v1();
        style.ordinary.outline = 127;
        assert_eq!(style.validate(), Err(StyleError::PaletteIndex));

        let mut style = StylePack::stanford_v1();
        style.ordinary.tree_spacing_mm = 0;
        assert_eq!(style.validate(), Err(StyleError::InvalidGrammar));

        let mut style = StylePack::stanford_v1();
        style.landmarks.hoover_heights_mm[2] = style.landmarks.hoover_heights_mm[1];
        assert_eq!(style.validate(), Err(StyleError::InvalidLandmarkGrammar));
    }
}
