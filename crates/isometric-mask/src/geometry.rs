//! Bounded deterministic computer-vision kernels for registered supertiles.

use std::{
    cmp::Reverse,
    collections::{BinaryHeap, VecDeque},
    error::Error,
    fmt::{Display, Formatter},
};

const MAX_DIMENSION: u32 = 4_096;
const MAX_RADIUS: u8 = 32;
const EDGE_OFF: u8 = 0;
const EDGE_ON: u8 = 255;
const UNREACHED_DISTANCE: u32 = u32::MAX / 4;

/// One row-major bounded raster.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Raster<T> {
    width: u32,
    height: u32,
    pixels: Vec<T>,
}

impl<T> Raster<T> {
    /// Construct a raster after checking dimensions and exact pixel count.
    ///
    /// # Errors
    ///
    /// Returns an error for zero or excessive dimensions, arithmetic overflow,
    /// or a pixel vector that does not match the grid.
    pub fn new(width: u32, height: u32, pixels: Vec<T>) -> Result<Self, KernelError> {
        let expected = checked_pixel_count(width, height)?;
        if pixels.len() != expected {
            return Err(KernelError::Invalid(format!(
                "raster contains {} pixels; expected {expected}",
                pixels.len()
            )));
        }
        Ok(Self {
            width,
            height,
            pixels,
        })
    }

    /// Raster width.
    #[must_use]
    pub const fn width(&self) -> u32 {
        self.width
    }

    /// Raster height.
    #[must_use]
    pub const fn height(&self) -> u32 {
        self.height
    }

    /// Immutable row-major pixels.
    #[must_use]
    pub fn pixels(&self) -> &[T] {
        &self.pixels
    }

    /// Consume the raster and return its row-major storage.
    #[must_use]
    pub fn into_pixels(self) -> Vec<T> {
        self.pixels
    }

    fn index(&self, x: u32, y: u32) -> usize {
        usize::try_from(u64::from(y) * u64::from(self.width) + u64::from(x))
            .expect("validated raster index fits usize")
    }

    fn same_grid<U>(&self, other: &Raster<U>) -> Result<(), KernelError> {
        if self.width != other.width || self.height != other.height {
            return Err(KernelError::Invalid(
                "registered kernel rasters do not share one grid".into(),
            ));
        }
        Ok(())
    }
}

impl<T: Clone> Raster<T> {
    /// Allocate a uniformly filled bounded raster.
    ///
    /// # Errors
    ///
    /// Returns an error when dimensions violate the raster ceiling.
    pub fn filled(width: u32, height: u32, value: T) -> Result<Self, KernelError> {
        let pixels = checked_pixel_count(width, height)?;
        Self::new(width, height, vec![value; pixels])
    }
}

/// Four quantized gradient-normal directions used by nonmaximum suppression.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u8)]
pub enum GradientDirection {
    /// Gradient normal follows the positive or negative x axis.
    Horizontal = 0,
    /// Gradient normal follows the rising diagonal.
    DiagonalUp = 1,
    /// Gradient normal follows the positive or negative y axis.
    Vertical = 2,
    /// Gradient normal follows the falling diagonal.
    DiagonalDown = 3,
}

/// Integer depth-gradient magnitude and quantized direction.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct GradientField {
    magnitude: Raster<u32>,
    direction: Raster<GradientDirection>,
}

impl GradientField {
    /// Gradient magnitudes using the L1 norm.
    #[must_use]
    pub const fn magnitude(&self) -> &Raster<u32> {
        &self.magnitude
    }

    /// Quantized gradient-normal directions.
    #[must_use]
    pub const fn direction(&self) -> &Raster<GradientDirection> {
        &self.direction
    }
}

/// Stable connected-component evidence.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Component {
    /// Positive label written to the component raster.
    pub label: u32,
    /// Exact component area in pixels.
    pub area: u32,
    /// Inclusive minimum x coordinate.
    pub min_x: u32,
    /// Inclusive minimum y coordinate.
    pub min_y: u32,
    /// Inclusive maximum x coordinate.
    pub max_x: u32,
    /// Inclusive maximum y coordinate.
    pub max_y: u32,
}

/// Neighborhood used by connected components.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Connectivity {
    /// Orthogonal neighbors only.
    Four,
    /// Orthogonal and diagonal neighbors.
    Eight,
}

/// Quantized line orientation.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum LineOrientation {
    /// Left to right.
    Horizontal,
    /// Top to bottom.
    Vertical,
    /// Top-left to bottom-right.
    DiagonalDown,
    /// Bottom-left to top-right.
    DiagonalUp,
}

/// One maximal deterministic line run.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LineSegment {
    /// Quantized orientation.
    pub orientation: LineOrientation,
    /// Inclusive start x coordinate.
    pub start_x: u32,
    /// Inclusive start y coordinate.
    pub start_y: u32,
    /// Inclusive end x coordinate.
    pub end_x: u32,
    /// Inclusive end y coordinate.
    pub end_y: u32,
    /// Number of contributing pixels.
    pub length: u32,
}

/// Fail-closed geometry-kernel error.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum KernelError {
    /// Input, threshold, or arithmetic violates a bounded kernel contract.
    Invalid(String),
}

impl Display for KernelError {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Invalid(message) => formatter.write_str(message),
        }
    }
}

impl Error for KernelError {}

/// Calculate integer Scharr depth gradients on a registered covered grid.
///
/// A pixel is zero when its complete 3 by 3 neighborhood is not covered. The
/// caller supplies the maximum legal depth so multiplication cannot overflow
/// the stored 32-bit magnitude.
///
/// # Errors
///
/// Returns an error for misregistered rasters, invalid coverage values, zero
/// depth ceiling, samples above the ceiling, or unsafe arithmetic.
pub fn scharr_depth(
    depth: &Raster<u32>,
    coverage: &Raster<u8>,
    max_depth_mm: u32,
) -> Result<GradientField, KernelError> {
    depth.same_grid(coverage)?;
    validate_coverage(coverage)?;
    if max_depth_mm == 0 || u64::from(max_depth_mm) * 32 > u64::from(u32::MAX) {
        return Err(KernelError::Invalid(
            "Scharr depth ceiling is zero or cannot fit its L1 response".into(),
        ));
    }
    if depth
        .pixels
        .iter()
        .zip(&coverage.pixels)
        .any(|(sample, valid)| *valid == EDGE_ON && (*sample == 0 || *sample > max_depth_mm))
    {
        return Err(KernelError::Invalid(
            "covered depth sample is zero or exceeds its declared ceiling".into(),
        ));
    }

    let mut magnitude = vec![0_u32; depth.pixels.len()];
    let mut direction = vec![GradientDirection::Horizontal; depth.pixels.len()];
    if depth.width < 3 || depth.height < 3 {
        return Ok(GradientField {
            magnitude: Raster::new(depth.width, depth.height, magnitude)?,
            direction: Raster::new(depth.width, depth.height, direction)?,
        });
    }

    for y in 1..depth.height - 1 {
        for x in 1..depth.width - 1 {
            if !neighborhood_is_covered(coverage, x, y) {
                continue;
            }
            let top_left = i64::from(depth.pixels[depth.index(x - 1, y - 1)]);
            let top = i64::from(depth.pixels[depth.index(x, y - 1)]);
            let top_right = i64::from(depth.pixels[depth.index(x + 1, y - 1)]);
            let left = i64::from(depth.pixels[depth.index(x - 1, y)]);
            let right = i64::from(depth.pixels[depth.index(x + 1, y)]);
            let bottom_left = i64::from(depth.pixels[depth.index(x - 1, y + 1)]);
            let bottom = i64::from(depth.pixels[depth.index(x, y + 1)]);
            let bottom_right = i64::from(depth.pixels[depth.index(x + 1, y + 1)]);
            let dx =
                3 * (top_right - top_left) + 10 * (right - left) + 3 * (bottom_right - bottom_left);
            let dy =
                3 * (bottom_left - top_left) + 10 * (bottom - top) + 3 * (bottom_right - top_right);
            let response = dx
                .unsigned_abs()
                .checked_add(dy.unsigned_abs())
                .ok_or_else(|| KernelError::Invalid("Scharr response overflowed".into()))?;
            let index = depth.index(x, y);
            magnitude[index] = u32::try_from(response)
                .map_err(|_| KernelError::Invalid("Scharr response exceeds u32".into()))?;
            direction[index] = quantize_direction(dx, dy);
        }
    }

    Ok(GradientField {
        magnitude: Raster::new(depth.width, depth.height, magnitude)?,
        direction: Raster::new(depth.width, depth.height, direction)?,
    })
}

/// Measure the greatest local encoded-normal difference at each covered pixel.
///
/// The squared RGB-vector distance is monotonic with angular difference for
/// normalized encoded normals and avoids nondeterministic trigonometry.
///
/// # Errors
///
/// Returns an error for misregistered inputs or invalid coverage values.
pub fn normal_discontinuity(
    normals: &Raster<[u8; 3]>,
    coverage: &Raster<u8>,
) -> Result<Raster<u32>, KernelError> {
    normals.same_grid(coverage)?;
    validate_coverage(coverage)?;
    let mut output = vec![0_u32; normals.pixels.len()];
    for y in 0..normals.height {
        for x in 0..normals.width {
            let index = normals.index(x, y);
            if coverage.pixels[index] != EDGE_ON {
                continue;
            }
            let center = normals.pixels[index];
            let mut maximum = 0_u32;
            for (neighbor_x, neighbor_y) in neighbors(normals.width, normals.height, x, y, true) {
                let neighbor_index = normals.index(neighbor_x, neighbor_y);
                if coverage.pixels[neighbor_index] != EDGE_ON {
                    continue;
                }
                let neighbor = normals.pixels[neighbor_index];
                let distance = center
                    .into_iter()
                    .zip(neighbor)
                    .try_fold(0_u32, |sum, (first, second)| {
                        let difference = i32::from(first) - i32::from(second);
                        sum.checked_add(difference.unsigned_abs().pow(2))
                    })
                    .ok_or_else(|| KernelError::Invalid("normal difference overflowed".into()))?;
                maximum = maximum.max(distance);
            }
            output[index] = maximum;
        }
    }
    Raster::new(normals.width, normals.height, output)
}

/// Apply deterministic Canny-style nonmaximum suppression and hysteresis.
///
/// Strong seeds and weak-neighbor traversal are both row-major. The output is
/// binary 0 or 255.
///
/// # Errors
///
/// Returns an error when thresholds are zero or reversed, gradient grids are
/// misregistered, or the raster cannot use bounded 32-bit queue identities.
pub fn hysteresis_edges(
    gradients: &GradientField,
    low_threshold: u32,
    high_threshold: u32,
) -> Result<Raster<u8>, KernelError> {
    gradients.magnitude.same_grid(&gradients.direction)?;
    if low_threshold == 0 || high_threshold < low_threshold {
        return Err(KernelError::Invalid(
            "hysteresis thresholds are zero or reversed".into(),
        ));
    }
    let width = gradients.magnitude.width;
    let height = gradients.magnitude.height;
    let mut states = vec![0_u8; gradients.magnitude.pixels.len()];
    let mut queue = VecDeque::<u32>::new();

    for y in 0..height {
        for x in 0..width {
            let index = gradients.magnitude.index(x, y);
            let value = gradients.magnitude.pixels[index];
            if value < low_threshold || !is_nonmaximum(gradients, x, y) {
                continue;
            }
            if value >= high_threshold {
                states[index] = 2;
                queue.push_back(u32::try_from(index).map_err(|_| {
                    KernelError::Invalid("hysteresis queue identity overflowed".into())
                })?);
            } else {
                states[index] = 1;
            }
        }
    }

    while let Some(index) = queue.pop_front() {
        let x = index % width;
        let y = index / width;
        for (neighbor_x, neighbor_y) in neighbors(width, height, x, y, true) {
            let neighbor = gradients.magnitude.index(neighbor_x, neighbor_y);
            if states[neighbor] == 1 {
                states[neighbor] = 2;
                queue.push_back(u32::try_from(neighbor).map_err(|_| {
                    KernelError::Invalid("hysteresis queue identity overflowed".into())
                })?);
            }
        }
    }
    for state in &mut states {
        *state = if *state == 2 { EDGE_ON } else { EDGE_OFF };
    }
    Raster::new(width, height, states)
}

/// Dilate a binary raster with a square radius.
///
/// # Errors
///
/// Returns an error for nonbinary pixels or a radius above 32.
pub fn morphology_dilate(input: &Raster<u8>, radius: u8) -> Result<Raster<u8>, KernelError> {
    binary_filter(input, radius, true)
}

/// Erode a binary raster with a square radius and zero exterior.
///
/// # Errors
///
/// Returns an error for nonbinary pixels or a radius above 32.
pub fn morphology_erode(input: &Raster<u8>, radius: u8) -> Result<Raster<u8>, KernelError> {
    binary_filter(input, radius, false)
}

/// Apply erosion followed by dilation.
///
/// # Errors
///
/// Returns an error for any invalid binary-kernel input.
pub fn open_binary(input: &Raster<u8>, radius: u8) -> Result<Raster<u8>, KernelError> {
    morphology_dilate(&morphology_erode(input, radius)?, radius)
}

/// Apply dilation followed by erosion.
///
/// # Errors
///
/// Returns an error for any invalid binary-kernel input.
pub fn close_binary(input: &Raster<u8>, radius: u8) -> Result<Raster<u8>, KernelError> {
    morphology_erode(&morphology_dilate(input, radius)?, radius)
}

/// Label binary connected components in stable row-major discovery order.
///
/// # Errors
///
/// Returns an error for nonbinary input or label arithmetic overflow.
pub fn connected_components(
    input: &Raster<u8>,
    connectivity: Connectivity,
) -> Result<(Raster<u32>, Vec<Component>), KernelError> {
    validate_binary(input)?;
    let mut labels = vec![0_u32; input.pixels.len()];
    let mut components = Vec::new();
    let mut queue = VecDeque::<u32>::new();

    for y in 0..input.height {
        for x in 0..input.width {
            let start = input.index(x, y);
            if input.pixels[start] != EDGE_ON || labels[start] != 0 {
                continue;
            }
            let label = u32::try_from(components.len() + 1)
                .map_err(|_| KernelError::Invalid("component label overflowed".into()))?;
            labels[start] = label;
            queue.push_back(
                u32::try_from(start).map_err(|_| {
                    KernelError::Invalid("component queue identity overflowed".into())
                })?,
            );
            let mut component = Component {
                label,
                area: 0,
                min_x: x,
                min_y: y,
                max_x: x,
                max_y: y,
            };
            while let Some(index) = queue.pop_front() {
                let pixel_x = index % input.width;
                let pixel_y = index / input.width;
                component.area = component
                    .area
                    .checked_add(1)
                    .ok_or_else(|| KernelError::Invalid("component area overflowed".into()))?;
                component.min_x = component.min_x.min(pixel_x);
                component.min_y = component.min_y.min(pixel_y);
                component.max_x = component.max_x.max(pixel_x);
                component.max_y = component.max_y.max(pixel_y);
                for (neighbor_x, neighbor_y) in neighbors(
                    input.width,
                    input.height,
                    pixel_x,
                    pixel_y,
                    connectivity == Connectivity::Eight,
                ) {
                    let neighbor = input.index(neighbor_x, neighbor_y);
                    if input.pixels[neighbor] == EDGE_ON && labels[neighbor] == 0 {
                        labels[neighbor] = label;
                        queue.push_back(u32::try_from(neighbor).map_err(|_| {
                            KernelError::Invalid("component queue identity overflowed".into())
                        })?);
                    }
                }
            }
            components.push(component);
        }
    }
    Ok((Raster::new(input.width, input.height, labels)?, components))
}

/// Compute a deterministic 3-4 chamfer distance from nonzero source pixels.
///
/// Orthogonal steps cost 3 and diagonal steps cost 4. Pixels in a raster with
/// no sources retain the stable unreached sentinel.
///
/// # Errors
///
/// Returns an error for nonbinary input.
pub fn chamfer_distance(input: &Raster<u8>) -> Result<Raster<u32>, KernelError> {
    validate_binary(input)?;
    let mut distance = input
        .pixels
        .iter()
        .map(|pixel| {
            if *pixel == EDGE_ON {
                0
            } else {
                UNREACHED_DISTANCE
            }
        })
        .collect::<Vec<_>>();
    for y in 0..input.height {
        for x in 0..input.width {
            let index = input.index(x, y);
            relax_distance(input, &mut distance, (x, y), index, (-1, 0), 3);
            relax_distance(input, &mut distance, (x, y), index, (0, -1), 3);
            relax_distance(input, &mut distance, (x, y), index, (-1, -1), 4);
            relax_distance(input, &mut distance, (x, y), index, (1, -1), 4);
        }
    }
    for y in (0..input.height).rev() {
        for x in (0..input.width).rev() {
            let index = input.index(x, y);
            relax_distance(input, &mut distance, (x, y), index, (1, 0), 3);
            relax_distance(input, &mut distance, (x, y), index, (0, 1), 3);
            relax_distance(input, &mut distance, (x, y), index, (1, 1), 4);
            relax_distance(input, &mut distance, (x, y), index, (-1, 1), 4);
        }
    }
    Raster::new(input.width, input.height, distance)
}

/// Partition a covered topography from positive marker labels.
///
/// Flood priority is `(maximum path cost, pixel index, label)`. Equal-cost
/// conflicts resolve to the lower positive label independent of marker input
/// discovery order.
///
/// # Errors
///
/// Returns an error for misregistered rasters, invalid coverage, uncovered
/// markers, or an absence of markers.
pub fn watershed(
    topography: &Raster<u32>,
    markers: &Raster<u32>,
    coverage: &Raster<u8>,
) -> Result<Raster<u32>, KernelError> {
    topography.same_grid(markers)?;
    topography.same_grid(coverage)?;
    validate_coverage(coverage)?;
    let mut labels = markers.pixels.clone();
    let mut cost = vec![u32::MAX; labels.len()];
    let mut queue = BinaryHeap::<Reverse<(u32, u32, u32)>>::new();

    for (index, label) in markers.pixels.iter().copied().enumerate() {
        if label == 0 {
            continue;
        }
        if coverage.pixels[index] != EDGE_ON {
            return Err(KernelError::Invalid(
                "watershed marker lies outside source coverage".into(),
            ));
        }
        cost[index] = topography.pixels[index];
        let pixel = u32::try_from(index)
            .map_err(|_| KernelError::Invalid("watershed identity overflowed".into()))?;
        queue.push(Reverse((cost[index], pixel, label)));
    }
    if queue.is_empty() {
        return Err(KernelError::Invalid(
            "watershed requires at least one positive marker".into(),
        ));
    }

    while let Some(Reverse((path_cost, index, label))) = queue.pop() {
        let index_usize = usize::try_from(index)
            .map_err(|_| KernelError::Invalid("watershed identity does not fit memory".into()))?;
        if cost[index_usize] != path_cost || labels[index_usize] != label {
            continue;
        }
        let x = index % topography.width;
        let y = index / topography.width;
        for (neighbor_x, neighbor_y) in neighbors(topography.width, topography.height, x, y, true) {
            let neighbor = topography.index(neighbor_x, neighbor_y);
            if coverage.pixels[neighbor] != EDGE_ON || markers.pixels[neighbor] != 0 {
                continue;
            }
            let candidate_cost = path_cost.max(topography.pixels[neighbor]);
            if candidate_cost < cost[neighbor]
                || (candidate_cost == cost[neighbor]
                    && (labels[neighbor] == 0 || label < labels[neighbor]))
            {
                cost[neighbor] = candidate_cost;
                labels[neighbor] = label;
                let pixel = u32::try_from(neighbor)
                    .map_err(|_| KernelError::Invalid("watershed identity overflowed".into()))?;
                queue.push(Reverse((candidate_cost, pixel, label)));
            }
        }
    }
    Raster::new(topography.width, topography.height, labels)
}

/// Extract maximal horizontal, vertical, and 45-degree binary line evidence.
///
/// # Errors
///
/// Returns an error for nonbinary input or a zero minimum length.
pub fn extract_line_evidence(
    input: &Raster<u8>,
    minimum_length: u16,
) -> Result<Vec<LineSegment>, KernelError> {
    validate_binary(input)?;
    if minimum_length == 0 {
        return Err(KernelError::Invalid(
            "line minimum length must be positive".into(),
        ));
    }
    let mut output = Vec::new();
    for (orientation, step_x, step_y) in [
        (LineOrientation::Horizontal, 1, 0),
        (LineOrientation::Vertical, 0, 1),
        (LineOrientation::DiagonalDown, 1, 1),
        (LineOrientation::DiagonalUp, 1, -1),
    ] {
        for y in 0..input.height {
            for x in 0..input.width {
                if input.pixels[input.index(x, y)] != EDGE_ON
                    || sample_binary(input, i64::from(x) - step_x, i64::from(y) - step_y)
                {
                    continue;
                }
                let mut end_x = i64::from(x);
                let mut end_y = i64::from(y);
                let mut length = 0_u32;
                while sample_binary(input, end_x, end_y) {
                    length = length
                        .checked_add(1)
                        .ok_or_else(|| KernelError::Invalid("line length overflowed".into()))?;
                    end_x += step_x;
                    end_y += step_y;
                }
                if length >= u32::from(minimum_length) {
                    let endpoint_x = u32::try_from(end_x - step_x)
                        .map_err(|_| KernelError::Invalid("line x endpoint overflowed".into()))?;
                    let endpoint_y = u32::try_from(end_y - step_y)
                        .map_err(|_| KernelError::Invalid("line y endpoint overflowed".into()))?;
                    output.push(LineSegment {
                        orientation,
                        start_x: x,
                        start_y: y,
                        end_x: endpoint_x,
                        end_y: endpoint_y,
                        length,
                    });
                }
            }
        }
    }
    Ok(output)
}

fn checked_pixel_count(width: u32, height: u32) -> Result<usize, KernelError> {
    if width == 0 || height == 0 || width > MAX_DIMENSION || height > MAX_DIMENSION {
        return Err(KernelError::Invalid(
            "kernel raster dimensions are zero or exceed 4,096".into(),
        ));
    }
    let pixels = u64::from(width)
        .checked_mul(u64::from(height))
        .ok_or_else(|| KernelError::Invalid("kernel raster size overflowed".into()))?;
    if pixels > u64::from(u32::MAX) {
        return Err(KernelError::Invalid(
            "kernel raster exceeds 32-bit stable pixel identity".into(),
        ));
    }
    usize::try_from(pixels)
        .map_err(|_| KernelError::Invalid("kernel raster does not fit address space".into()))
}

fn validate_coverage(coverage: &Raster<u8>) -> Result<(), KernelError> {
    if coverage
        .pixels
        .iter()
        .any(|pixel| !matches!(*pixel, EDGE_OFF | EDGE_ON))
    {
        return Err(KernelError::Invalid(
            "coverage raster must contain only 0 or 255".into(),
        ));
    }
    Ok(())
}

fn validate_binary(input: &Raster<u8>) -> Result<(), KernelError> {
    if input
        .pixels
        .iter()
        .any(|pixel| !matches!(*pixel, EDGE_OFF | EDGE_ON))
    {
        return Err(KernelError::Invalid(
            "binary kernel input must contain only 0 or 255".into(),
        ));
    }
    Ok(())
}

fn neighborhood_is_covered(coverage: &Raster<u8>, x: u32, y: u32) -> bool {
    (y - 1..=y + 1).all(|neighbor_y| {
        (x - 1..=x + 1)
            .all(|neighbor_x| coverage.pixels[coverage.index(neighbor_x, neighbor_y)] == EDGE_ON)
    })
}

fn quantize_direction(dx: i64, dy: i64) -> GradientDirection {
    let absolute_x = dx.unsigned_abs();
    let absolute_y = dy.unsigned_abs();
    if absolute_y.saturating_mul(2) < absolute_x {
        GradientDirection::Horizontal
    } else if absolute_x.saturating_mul(2) < absolute_y {
        GradientDirection::Vertical
    } else if dx.signum() == dy.signum() {
        GradientDirection::DiagonalDown
    } else {
        GradientDirection::DiagonalUp
    }
}

fn is_nonmaximum(gradients: &GradientField, x: u32, y: u32) -> bool {
    let index = gradients.magnitude.index(x, y);
    let value = gradients.magnitude.pixels[index];
    let (first, second) = match gradients.direction.pixels[index] {
        GradientDirection::Horizontal => ((-1, 0), (1, 0)),
        GradientDirection::DiagonalUp => ((-1, 1), (1, -1)),
        GradientDirection::Vertical => ((0, -1), (0, 1)),
        GradientDirection::DiagonalDown => ((-1, -1), (1, 1)),
    };
    value >= sample_magnitude(gradients, x, y, first.0, first.1)
        && value >= sample_magnitude(gradients, x, y, second.0, second.1)
}

fn sample_magnitude(
    gradients: &GradientField,
    x: u32,
    y: u32,
    offset_x: i64,
    offset_y: i64,
) -> u32 {
    let candidate_x = i64::from(x) + offset_x;
    let candidate_y = i64::from(y) + offset_y;
    if candidate_x < 0
        || candidate_y < 0
        || candidate_x >= i64::from(gradients.magnitude.width)
        || candidate_y >= i64::from(gradients.magnitude.height)
    {
        return 0;
    }
    gradients.magnitude.pixels[gradients.magnitude.index(
        u32::try_from(candidate_x).expect("bounded x"),
        u32::try_from(candidate_y).expect("bounded y"),
    )]
}

fn binary_filter(input: &Raster<u8>, radius: u8, dilate: bool) -> Result<Raster<u8>, KernelError> {
    validate_binary(input)?;
    if radius > MAX_RADIUS {
        return Err(KernelError::Invalid(
            "morphology radius exceeds the 32-pixel bound".into(),
        ));
    }
    if radius == 0 {
        return Ok(input.clone());
    }
    let radius = i64::from(radius);
    let diameter = u32::try_from(2 * radius + 1).expect("bounded morphology diameter");
    let mut horizontal = vec![EDGE_OFF; input.pixels.len()];
    for y in 0..input.height {
        let mut count = 0_u32;
        for offset in -radius..=radius {
            count += u32::from(sample_binary(input, offset, i64::from(y)));
        }
        for x in 0..input.width {
            horizontal[input.index(x, y)] = binary_filter_value(count, diameter, dilate);
            count -= u32::from(sample_binary(input, i64::from(x) - radius, i64::from(y)));
            count += u32::from(sample_binary(
                input,
                i64::from(x) + radius + 1,
                i64::from(y),
            ));
        }
    }
    let horizontal = Raster::new(input.width, input.height, horizontal)?;
    let mut output = vec![EDGE_OFF; input.pixels.len()];
    for x in 0..input.width {
        let mut count = 0_u32;
        for offset in -radius..=radius {
            count += u32::from(sample_binary(&horizontal, i64::from(x), offset));
        }
        for y in 0..input.height {
            output[input.index(x, y)] = binary_filter_value(count, diameter, dilate);
            count -= u32::from(sample_binary(
                &horizontal,
                i64::from(x),
                i64::from(y) - radius,
            ));
            count += u32::from(sample_binary(
                &horizontal,
                i64::from(x),
                i64::from(y) + radius + 1,
            ));
        }
    }
    Raster::new(input.width, input.height, output)
}

const fn binary_filter_value(count: u32, diameter: u32, dilate: bool) -> u8 {
    if (dilate && count > 0) || (!dilate && count == diameter) {
        EDGE_ON
    } else {
        EDGE_OFF
    }
}

fn sample_binary(input: &Raster<u8>, x: i64, y: i64) -> bool {
    if x < 0 || y < 0 || x >= i64::from(input.width) || y >= i64::from(input.height) {
        return false;
    }
    input.pixels[input.index(
        u32::try_from(x).expect("bounded binary x"),
        u32::try_from(y).expect("bounded binary y"),
    )] == EDGE_ON
}

fn relax_distance(
    input: &Raster<u8>,
    distance: &mut [u32],
    point: (u32, u32),
    index: usize,
    offset: (i64, i64),
    weight: u32,
) {
    let neighbor_x = i64::from(point.0) + offset.0;
    let neighbor_y = i64::from(point.1) + offset.1;
    if neighbor_x < 0
        || neighbor_y < 0
        || neighbor_x >= i64::from(input.width)
        || neighbor_y >= i64::from(input.height)
    {
        return;
    }
    let neighbor = input.index(
        u32::try_from(neighbor_x).expect("bounded distance x"),
        u32::try_from(neighbor_y).expect("bounded distance y"),
    );
    distance[index] = distance[index].min(distance[neighbor].saturating_add(weight));
}

fn neighbors(
    width: u32,
    height: u32,
    x: u32,
    y: u32,
    diagonals: bool,
) -> impl Iterator<Item = (u32, u32)> {
    const OFFSETS: [(i64, i64); 8] = [
        (-1, -1),
        (0, -1),
        (1, -1),
        (-1, 0),
        (1, 0),
        (-1, 1),
        (0, 1),
        (1, 1),
    ];
    OFFSETS.into_iter().filter_map(move |(offset_x, offset_y)| {
        if !diagonals && offset_x != 0 && offset_y != 0 {
            return None;
        }
        let neighbor_x = i64::from(x) + offset_x;
        let neighbor_y = i64::from(y) + offset_y;
        (neighbor_x >= 0
            && neighbor_y >= 0
            && neighbor_x < i64::from(width)
            && neighbor_y < i64::from(height))
        .then(|| {
            (
                u32::try_from(neighbor_x).expect("bounded neighbor x"),
                u32::try_from(neighbor_y).expect("bounded neighbor y"),
            )
        })
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn raster(width: u32, height: u32, pixels: &[u8]) -> Raster<u8> {
        Raster::new(width, height, pixels.to_vec()).expect("valid fixture raster")
    }

    #[test]
    fn raster_bounds_and_registration_fail_closed() {
        assert!(Raster::<u8>::new(0, 1, Vec::new()).is_err());
        assert!(Raster::<u8>::new(MAX_DIMENSION + 1, 1, Vec::new()).is_err());
        assert!(Raster::new(2, 2, vec![0_u8; 3]).is_err());
        let first = Raster::filled(2, 2, 0_u8).expect("first");
        let second = Raster::filled(3, 2, 0_u8).expect("second");
        assert!(first.same_grid(&second).is_err());
    }

    #[test]
    fn malformed_kernel_inputs_fail_closed() {
        let invalid_binary = raster(2, 2, &[0, 1, 255, 0]);
        assert!(morphology_dilate(&invalid_binary, 1).is_err());
        assert!(connected_components(&invalid_binary, Connectivity::Four).is_err());
        assert!(chamfer_distance(&invalid_binary).is_err());

        let depth = Raster::filled(2, 2, 1_000_u32).expect("depth");
        let invalid_coverage = raster(2, 2, &[255, 255, 7, 255]);
        assert!(scharr_depth(&depth, &invalid_coverage, 1_000).is_err());

        let topography = Raster::filled(2, 2, 1_u32).expect("topography");
        let markers = Raster::filled(2, 2, 0_u32).expect("markers");
        let coverage = Raster::filled(2, 2, EDGE_ON).expect("coverage");
        assert!(watershed(&topography, &markers, &coverage).is_err());
    }

    #[test]
    fn scharr_depth_detects_a_step_and_rejects_overflow_contracts() {
        let depth = Raster::new(
            5,
            5,
            (0..25)
                .map(|index| if index % 5 < 2 { 1_000 } else { 2_000 })
                .collect(),
        )
        .expect("depth");
        let coverage = Raster::filled(5, 5, EDGE_ON).expect("coverage");
        let first = scharr_depth(&depth, &coverage, 2_000).expect("first gradient");
        let second = scharr_depth(&depth, &coverage, 2_000).expect("second gradient");
        assert_eq!(first, second);
        assert_eq!(first.magnitude.pixels[first.magnitude.index(2, 2)], 16_000);
        assert_eq!(
            first.direction.pixels[first.direction.index(2, 2)],
            GradientDirection::Horizontal
        );
        assert!(scharr_depth(&depth, &coverage, u32::MAX).is_err());
    }

    #[test]
    fn uncovered_depth_neighborhood_is_suppressed() {
        let depth = Raster::filled(5, 5, 1_000_u32).expect("depth");
        let mut coverage = vec![EDGE_ON; 25];
        coverage[6] = EDGE_OFF;
        let coverage = Raster::new(5, 5, coverage).expect("coverage");
        let gradient = scharr_depth(&depth, &coverage, 1_000).expect("gradient");
        assert_eq!(gradient.magnitude.pixels[gradient.magnitude.index(2, 2)], 0);
    }

    #[test]
    fn normal_difference_uses_covered_neighbors_only() {
        let normals = Raster::new(
            3,
            1,
            vec![[128, 128, 255], [128, 128, 255], [128, 255, 128]],
        )
        .expect("normals");
        let coverage = Raster::filled(3, 1, EDGE_ON).expect("coverage");
        let difference = normal_discontinuity(&normals, &coverage).expect("difference");
        assert_eq!(difference.pixels, vec![0, 32_258, 32_258]);
    }

    #[test]
    fn hysteresis_connects_weak_edges_in_stable_order() {
        let magnitude = Raster::new(5, 1, vec![0, 20, 10, 10, 0]).expect("magnitude");
        let direction = Raster::filled(5, 1, GradientDirection::Vertical).expect("direction");
        let edges = hysteresis_edges(
            &GradientField {
                magnitude,
                direction,
            },
            10,
            20,
        )
        .expect("edges");
        assert_eq!(edges.pixels, vec![0, 255, 255, 255, 0]);
    }

    #[test]
    fn morphology_matches_square_zero_border_contract() {
        let input = raster(
            5,
            5,
            &[
                0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 255, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
            ],
        );
        let dilated = morphology_dilate(&input, 1).expect("dilate");
        assert_eq!(
            dilated
                .pixels
                .iter()
                .fold(0, |count, pixel| count + usize::from(*pixel == 255)),
            9
        );
        assert_eq!(morphology_erode(&dilated, 1).expect("erode"), input);
        assert!(morphology_dilate(&input, MAX_RADIUS + 1).is_err());
    }

    #[test]
    fn components_are_labeled_in_row_major_order() {
        let input = raster(4, 3, &[255, 0, 0, 255, 255, 0, 0, 0, 0, 0, 255, 255]);
        let (labels, components) = connected_components(&input, Connectivity::Four).expect("cc");
        assert_eq!(components.len(), 3);
        assert_eq!(components[0].area, 2);
        assert_eq!(components[1].label, 2);
        assert_eq!(labels.pixels, vec![1, 0, 0, 2, 1, 0, 0, 0, 0, 0, 3, 3]);
    }

    #[test]
    fn chamfer_distance_has_exact_three_four_metric() {
        let input = raster(3, 3, &[0, 0, 0, 0, 255, 0, 0, 0, 0]);
        let distance = chamfer_distance(&input).expect("distance");
        assert_eq!(distance.pixels, vec![4, 3, 4, 3, 0, 3, 4, 3, 4]);
    }

    #[test]
    fn watershed_resolves_equal_cost_conflicts_to_lower_label() {
        let topography = Raster::filled(5, 1, 10_u32).expect("topography");
        let markers = Raster::new(5, 1, vec![9, 0, 0, 0, 2]).expect("markers");
        let coverage = Raster::filled(5, 1, EDGE_ON).expect("coverage");
        let labels = watershed(&topography, &markers, &coverage).expect("watershed");
        assert_eq!(labels.pixels, vec![9, 2, 2, 2, 2]);

        let permuted_markers = Raster::new(5, 1, vec![2, 0, 0, 0, 9]).expect("markers");
        let permuted = watershed(&topography, &permuted_markers, &coverage).expect("watershed");
        assert_eq!(permuted.pixels, vec![2, 2, 2, 2, 9]);
    }

    #[test]
    fn line_runs_are_maximal_stable_and_scale_gated() {
        let input = raster(
            5,
            5,
            &[
                255, 0, 0, 0, 255, 0, 255, 0, 255, 0, 0, 0, 255, 0, 0, 0, 255, 0, 255, 0, 255, 0,
                0, 0, 255,
            ],
        );
        let lines = extract_line_evidence(&input, 5).expect("lines");
        assert_eq!(lines.len(), 2);
        assert_eq!(lines[0].orientation, LineOrientation::DiagonalDown);
        assert_eq!(lines[1].orientation, LineOrientation::DiagonalUp);
        assert!(extract_line_evidence(&input, 0).is_err());
    }

    #[test]
    fn accepted_kernels_match_the_pinned_opencv_oracle() {
        let oracle: serde_json::Value = serde_json::from_str(include_str!(
            "../../../fixtures/masks/geometry/opencv-oracle.json"
        ))
        .expect("parse OpenCV oracle");
        assert_eq!(oracle["schema"], "isometric-opencv-geometry-oracle/v1");
        assert_eq!(oracle["opencv_version"], "5.0.0");

        let scharr = &oracle["scharr"];
        let width = json_u32(scharr, "width");
        let height = json_u32(scharr, "height");
        let depth = Raster::new(width, height, json_u32_pixels(scharr, "depth")).expect("depth");
        let coverage = Raster::filled(width, height, EDGE_ON).expect("coverage");
        let gradients = scharr_depth(&depth, &coverage, 2_000).expect("Scharr");
        assert_eq!(
            gradients.magnitude.pixels,
            json_u32_pixels(scharr, "l1_magnitude")
        );

        let morphology = &oracle["morphology"];
        let width = json_u32(morphology, "width");
        let height = json_u32(morphology, "height");
        let input =
            Raster::new(width, height, json_u8_pixels(morphology, "input")).expect("binary input");
        let radius = u8::try_from(json_u32(morphology, "radius")).expect("radius fits");
        assert_eq!(
            morphology_dilate(&input, radius).expect("dilate").pixels,
            json_u8_pixels(morphology, "dilate")
        );
        assert_eq!(
            morphology_erode(&input, radius).expect("erode").pixels,
            json_u8_pixels(morphology, "erode")
        );
        assert_eq!(
            open_binary(&input, radius).expect("open").pixels,
            json_u8_pixels(morphology, "open")
        );
        assert_eq!(
            close_binary(&input, radius).expect("close").pixels,
            json_u8_pixels(morphology, "close")
        );
        let (labels, components) =
            connected_components(&input, Connectivity::Eight).expect("components");
        assert_eq!(
            components.len(),
            usize::try_from(json_u32(&oracle["components"], "foreground_components"))
                .expect("component count fits")
        );
        assert_eq!(
            labels.pixels,
            json_u32_pixels(&oracle["components"], "labels")
        );
    }

    #[test]
    fn guarded_local_kernels_match_monolithic_core() {
        let mut pixels = vec![0_u8; 9 * 7];
        for y in 2..5 {
            for x in 2..7 {
                pixels[y * 9 + x] = EDGE_ON;
            }
        }
        let full = Raster::new(9, 7, pixels).expect("full");
        let full_result = close_binary(&full, 1).expect("full result");
        let guarded_crop = crop_raster(&full, 2, 0, 7, 7);
        let crop_result = close_binary(&guarded_crop, 1).expect("crop result");
        assert_eq!(
            crop_raster(&full_result, 4, 2, 3, 3),
            crop_raster(&crop_result, 2, 2, 3, 3)
        );

        let depth_pixels = (0..7)
            .flat_map(|_| (0..9).map(|x| if x < 5 { 1_000_u32 } else { 2_000_u32 }))
            .collect();
        let full_depth = Raster::new(9, 7, depth_pixels).expect("full depth");
        let full_coverage = Raster::filled(9, 7, EDGE_ON).expect("full coverage");
        let full_gradient =
            scharr_depth(&full_depth, &full_coverage, 2_000).expect("full gradient");
        let guarded_depth = crop_raster(&full_depth, 2, 0, 7, 7);
        let guarded_coverage = crop_raster(&full_coverage, 2, 0, 7, 7);
        let guarded_gradient =
            scharr_depth(&guarded_depth, &guarded_coverage, 2_000).expect("guarded gradient");
        assert_eq!(
            crop_raster(full_gradient.magnitude(), 4, 2, 3, 3),
            crop_raster(guarded_gradient.magnitude(), 2, 2, 3, 3)
        );
    }

    fn crop_raster<T: Copy>(
        input: &Raster<T>,
        x: u32,
        y: u32,
        width: u32,
        height: u32,
    ) -> Raster<T> {
        let mut output = Vec::new();
        for row in y..y + height {
            for column in x..x + width {
                output.push(input.pixels[input.index(column, row)]);
            }
        }
        Raster::new(width, height, output).expect("valid crop")
    }

    fn json_u32(value: &serde_json::Value, field: &str) -> u32 {
        u32::try_from(value[field].as_u64().expect("oracle integer")).expect("oracle u32")
    }

    fn json_u32_pixels(value: &serde_json::Value, field: &str) -> Vec<u32> {
        value[field]
            .as_array()
            .expect("oracle pixel array")
            .iter()
            .map(|pixel| {
                u32::try_from(pixel.as_u64().expect("oracle pixel integer"))
                    .expect("oracle pixel u32")
            })
            .collect()
    }

    fn json_u8_pixels(value: &serde_json::Value, field: &str) -> Vec<u8> {
        json_u32_pixels(value, field)
            .into_iter()
            .map(|pixel| u8::try_from(pixel).expect("oracle pixel u8"))
            .collect()
    }
}
