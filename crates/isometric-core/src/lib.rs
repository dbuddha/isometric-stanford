//! Validated, dependency-free value types shared by the pipeline.

use core::fmt;

/// A stable identifier derived from source identity, never collection order.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct ObjectId(u64);

impl ObjectId {
    /// Creates an identifier. Zero is reserved for background and is rejected.
    ///
    /// # Errors
    ///
    /// Returns [`CoreError::ReservedObjectId`] when `value` is zero.
    pub const fn new(value: u64) -> Result<Self, CoreError> {
        if value == 0 {
            Err(CoreError::ReservedObjectId)
        } else {
            Ok(Self(value))
        }
    }

    /// Returns the underlying stable value.
    #[must_use]
    pub const fn get(self) -> u64 {
        self.0
    }

    /// Returns a deterministic variation bucket without process-random hashing.
    #[must_use]
    pub fn variation(self, buckets: u8) -> u8 {
        if buckets == 0 {
            return 0;
        }
        let mut value = self.0;
        value ^= value >> 33;
        value = value.wrapping_mul(0xff51_afd7_ed55_8ccd);
        value ^= value >> 33;
        let reduced = value % u64::from(buckets);
        reduced.to_le_bytes()[0]
    }
}

/// A world-space point in integer millimeters.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct WorldPoint {
    /// Easting relative to the world origin.
    pub x_mm: i64,
    /// Northing relative to the world origin.
    pub y_mm: i64,
    /// Elevation relative to the world datum.
    pub z_mm: i64,
}

impl WorldPoint {
    /// Creates an integer world point.
    #[must_use]
    pub const fn new(x_mm: i64, y_mm: i64, z_mm: i64) -> Self {
        Self { x_mm, y_mm, z_mm }
    }
}

/// A projected point in fixed subpixels.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ScreenPoint {
    /// Horizontal coordinate in fixed subpixels.
    pub x_subpx: i64,
    /// Vertical coordinate in fixed subpixels.
    pub y_subpx: i64,
}

/// An index into an approved style palette.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PaletteIndex(u8);

impl PaletteIndex {
    /// Creates an index if it is present in the declared palette length.
    ///
    /// # Errors
    ///
    /// Returns [`CoreError::PaletteIndexOutOfRange`] for an empty palette or an
    /// index outside `palette_len`.
    pub const fn new(value: u8, palette_len: u8) -> Result<Self, CoreError> {
        if palette_len == 0 || value >= palette_len {
            Err(CoreError::PaletteIndexOutOfRange)
        } else {
            Ok(Self(value))
        }
    }

    /// Returns the raw palette index.
    #[must_use]
    pub const fn get(self) -> u8 {
        self.0
    }
}

/// Validation failures at the portable value boundary.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CoreError {
    /// Object ID zero is reserved for the background.
    ReservedObjectId,
    /// A palette index is outside the declared palette.
    PaletteIndexOutOfRange,
}

impl fmt::Display for CoreError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ReservedObjectId => formatter.write_str("object ID zero is reserved"),
            Self::PaletteIndexOutOfRange => formatter.write_str("palette index is out of range"),
        }
    }
}

impl std::error::Error for CoreError {}

#[cfg(test)]
mod tests {
    use super::{CoreError, ObjectId, PaletteIndex};

    #[test]
    fn rejects_reserved_object_id() {
        assert_eq!(ObjectId::new(0), Err(CoreError::ReservedObjectId));
    }

    #[test]
    fn stable_variation_is_repeatable() {
        let id = ObjectId::new(42).expect("42 is valid");
        assert_eq!(id.variation(7), id.variation(7));
        assert_eq!(id.variation(0), 0);
    }

    #[test]
    fn palette_index_must_exist() {
        assert!(PaletteIndex::new(3, 4).is_ok());
        assert_eq!(
            PaletteIndex::new(4, 4),
            Err(CoreError::PaletteIndexOutOfRange)
        );
    }
}
