//! Semantic and style validation entry points.

use core::fmt;
use isometric_style::StylePack;
use isometric_world::{SemanticClass, World};

/// Validates bootstrap semantic contracts.
///
/// # Errors
///
/// Returns a [`ValidationError`] for an empty, non-canonical, or unresolved
/// world.
pub fn validate_world(world: &World) -> Result<(), ValidationError> {
    if world.objects().is_empty() {
        return Err(ValidationError::EmptyWorld);
    }
    let mut previous = None;
    for object in world.objects() {
        if previous.is_some_and(|id| id >= object.id) {
            return Err(ValidationError::ObjectOrder);
        }
        if object.class == SemanticClass::Unknown {
            return Err(ValidationError::UnknownObject);
        }
        previous = Some(object.id);
    }
    Ok(())
}

/// Validates style policy, including the hard palette ceiling.
///
/// # Errors
///
/// Returns [`ValidationError::InvalidStyle`] when the style violates a hard
/// contract.
pub fn validate_style(style: &StylePack) -> Result<(), ValidationError> {
    style.validate().map_err(|_| ValidationError::InvalidStyle)
}

/// A fail-closed validation error.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ValidationError {
    /// A world contains no permanent objects.
    EmptyWorld,
    /// Stable object ordering is not strictly increasing.
    ObjectOrder,
    /// An unresolved object reached a qualification input.
    UnknownObject,
    /// The style pack violates a hard contract.
    InvalidStyle,
}

impl fmt::Display for ValidationError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::EmptyWorld => "world is empty",
            Self::ObjectOrder => "world objects are not in stable ID order",
            Self::UnknownObject => "world contains unresolved objects",
            Self::InvalidStyle => "style pack violates its contract",
        })
    }
}

impl std::error::Error for ValidationError {}

#[cfg(test)]
mod tests {
    use super::{validate_style, validate_world};
    use isometric_style::StylePack;
    use isometric_world::World;

    #[test]
    fn reference_inputs_pass() {
        validate_world(&World::reference_fixture()).expect("world must pass");
        validate_style(&StylePack::stanford_v1()).expect("style must pass");
    }
}
