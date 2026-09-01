//! Small, bounded deterministic image kernels used by the reference study.

use crate::{StylizeError, palette::Rgb};

const SMOOTH_NEIGHBORS: [(isize, isize, u32); 8] = [
    (-1, 0, 4),
    (1, 0, 4),
    (0, -1, 4),
    (0, 1, 4),
    (-1, -1, 2),
    (1, -1, 2),
    (-1, 1, 2),
    (1, 1, 2),
];

/// Tightly packed RGB image.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RgbImage {
    /// Pixel width.
    pub width: usize,
    /// Pixel height.
    pub height: usize,
    /// Row-major RGB pixels.
    pub pixels: Vec<Rgb>,
}

impl RgbImage {
    pub(crate) fn new(width: usize, height: usize, pixels: Vec<Rgb>) -> Result<Self, StylizeError> {
        if width == 0
            || height == 0
            || width.checked_mul(height) != Some(pixels.len())
            || width > 4_096
            || height > 4_096
        {
            return Err(StylizeError::Invalid(
                "RGB image dimensions are invalid".into(),
            ));
        }
        Ok(Self {
            width,
            height,
            pixels,
        })
    }

    #[inline]
    pub(crate) const fn index(&self, x: usize, y: usize) -> usize {
        y * self.width + x
    }
}

pub(crate) fn luminance(pixel: Rgb) -> u8 {
    let value = 54 * u32::from(pixel[0]) + 183 * u32::from(pixel[1]) + 19 * u32::from(pixel[2]);
    u8::try_from(value >> 8).expect("weighted u8 luminance fits")
}

pub(crate) fn reduce_rgb_2x(image: &RgbImage) -> Result<RgbImage, StylizeError> {
    if !image.width.is_multiple_of(2) || !image.height.is_multiple_of(2) {
        return Err(StylizeError::Invalid(
            "logical pixel reduction requires even dimensions".into(),
        ));
    }
    let width = image.width / 2;
    let height = image.height / 2;
    let mut pixels = Vec::with_capacity(width * height);
    for y in 0..height {
        for x in 0..width {
            let source_x = x * 2;
            let source_y = y * 2;
            let samples = [
                image.pixels[image.index(source_x, source_y)],
                image.pixels[image.index(source_x + 1, source_y)],
                image.pixels[image.index(source_x, source_y + 1)],
                image.pixels[image.index(source_x + 1, source_y + 1)],
            ];
            let mut reduced = [0_u8; 3];
            for channel in 0..3 {
                let sum = samples
                    .iter()
                    .map(|pixel| u16::from(pixel[channel]))
                    .sum::<u16>();
                reduced[channel] = u8::try_from((sum + 2) / 4).expect("u8 average fits");
            }
            pixels.push(reduced);
        }
    }
    RgbImage::new(width, height, pixels)
}

pub(crate) fn reduce_depth_2x(
    width: usize,
    height: usize,
    depth: &[u32],
) -> Result<Vec<u32>, StylizeError> {
    if width.checked_mul(height) != Some(depth.len())
        || !width.is_multiple_of(2)
        || !height.is_multiple_of(2)
    {
        return Err(StylizeError::Invalid(
            "depth reduction dimensions are invalid".into(),
        ));
    }
    let output_width = width / 2;
    let mut output = Vec::with_capacity(output_width * (height / 2));
    for y in (0..height).step_by(2) {
        for x in (0..width).step_by(2) {
            let samples = [
                depth[y * width + x],
                depth[y * width + x + 1],
                depth[(y + 1) * width + x],
                depth[(y + 1) * width + x + 1],
            ];
            let mut valid = samples
                .into_iter()
                .filter(|value| *value != 0)
                .collect::<Vec<_>>();
            valid.sort_unstable();
            output.push(match valid.as_slice() {
                [] => 0,
                values => values[values.len() / 2],
            });
        }
    }
    Ok(output)
}

pub(crate) fn edge_aware_smooth(
    image: &RgbImage,
    normals: Option<&RgbImage>,
    depth: Option<&[u32]>,
    iterations: u8,
) -> Result<RgbImage, StylizeError> {
    if normals.is_some_and(|normal| normal.width != image.width || normal.height != image.height)
        || depth.is_some_and(|values| values.len() != image.pixels.len())
    {
        return Err(StylizeError::Invalid(
            "smoothing guidance is not registered to color".into(),
        ));
    }
    let mut current = image.clone();
    for _ in 0..iterations {
        let luma = current
            .pixels
            .iter()
            .copied()
            .map(luminance)
            .collect::<Vec<_>>();
        let mut output = vec![[0_u8; 3]; current.pixels.len()];
        for y in 0..current.height {
            for x in 0..current.width {
                let index = current.index(x, y);
                let mut sums = [0_u32; 3];
                let mut denominator = 8_u32;
                for (sum, channel) in sums.iter_mut().zip(current.pixels[index]) {
                    *sum = u32::from(channel) * denominator;
                }
                for (dx, dy, weight) in SMOOTH_NEIGHBORS {
                    let Some(nx) = x.checked_add_signed(dx) else {
                        continue;
                    };
                    let Some(ny) = y.checked_add_signed(dy) else {
                        continue;
                    };
                    if nx >= current.width || ny >= current.height {
                        continue;
                    }
                    let neighbor = current.index(nx, ny);
                    if luma[index].abs_diff(luma[neighbor]) >= 28 {
                        continue;
                    }
                    if let Some(normal) = normals {
                        let difference = (0..3)
                            .map(|channel| {
                                normal.pixels[index][channel]
                                    .abs_diff(normal.pixels[neighbor][channel])
                            })
                            .map(u16::from)
                            .sum::<u16>();
                        if difference >= 70 {
                            continue;
                        }
                    }
                    if let Some(values) = depth {
                        let first = values[index];
                        let second = values[neighbor];
                        if first == 0 || second == 0 || first.abs_diff(second) > 2_000 {
                            continue;
                        }
                    }
                    denominator += weight;
                    for (sum, channel) in sums.iter_mut().zip(current.pixels[neighbor]) {
                        *sum += u32::from(channel) * weight;
                    }
                }
                for (channel, sum) in sums.into_iter().enumerate() {
                    output[index][channel] = u8::try_from((sum + denominator / 2) / denominator)
                        .expect("weighted u8 average fits");
                }
            }
        }
        current.pixels = output;
    }
    Ok(current)
}

pub(crate) fn quantize(image: &RgbImage, palette: &[Rgb]) -> Result<RgbImage, StylizeError> {
    if palette.is_empty() || palette.len() > 128 {
        return Err(StylizeError::Invalid(
            "candidate palette must contain 1 to 128 colors".into(),
        ));
    }
    let pixels = image
        .pixels
        .iter()
        .copied()
        .map(|pixel| nearest(pixel, palette))
        .collect();
    RgbImage::new(image.width, image.height, pixels)
}

fn nearest(pixel: Rgb, palette: &[Rgb]) -> Rgb {
    palette
        .iter()
        .copied()
        .min_by_key(|candidate| {
            let red = i32::from(pixel[0]) - i32::from(candidate[0]);
            let green = i32::from(pixel[1]) - i32::from(candidate[1]);
            let blue = i32::from(pixel[2]) - i32::from(candidate[2]);
            2 * red * red + 3 * green * green + blue * blue
        })
        .expect("validated non-empty palette")
}

pub(crate) fn box_blur(values: &[u8], width: usize, height: usize, radius: usize) -> Vec<u8> {
    let mut horizontal = vec![0_u8; values.len()];
    let mut prefix = vec![0_u32; width + 1];
    for y in 0..height {
        prefix.fill(0);
        for x in 0..width {
            prefix[x + 1] = prefix[x] + u32::from(values[y * width + x]);
        }
        for x in 0..width {
            let left = x.saturating_sub(radius);
            let right = (x + radius + 1).min(width);
            let count = u32::try_from(right - left).expect("bounded row span");
            horizontal[y * width + x] =
                u8::try_from((prefix[right] - prefix[left] + count / 2) / count)
                    .expect("u8 average fits");
        }
    }
    let mut output = vec![0_u8; values.len()];
    prefix.resize(height + 1, 0);
    for x in 0..width {
        prefix.fill(0);
        for y in 0..height {
            prefix[y + 1] = prefix[y] + u32::from(horizontal[y * width + x]);
        }
        for y in 0..height {
            let top = y.saturating_sub(radius);
            let bottom = (y + radius + 1).min(height);
            let count = u32::try_from(bottom - top).expect("bounded column span");
            output[y * width + x] =
                u8::try_from((prefix[bottom] - prefix[top] + count / 2) / count)
                    .expect("u8 average fits");
        }
    }
    output
}

pub(crate) fn relight(image: &RgbImage, normals: &RgbImage) -> Result<RgbImage, StylizeError> {
    if image.width != normals.width || image.height != normals.height {
        return Err(StylizeError::Invalid(
            "relighting guidance is not registered to color".into(),
        ));
    }
    let luma = image
        .pixels
        .iter()
        .copied()
        .map(luminance)
        .collect::<Vec<_>>();
    let illumination = box_blur(&luma, image.width, image.height, 18);
    let mut pixels = Vec::with_capacity(image.pixels.len());
    for (index, pixel) in image.pixels.iter().copied().enumerate() {
        let normal = normals.pixels[index];
        let nx = i32::from(normal[0]) - 128;
        let ny = i32::from(normal[1]) - 128;
        let nz = i32::from(normal[2]) - 128;
        let synthetic = (220 + (-nx + ny + 2 * nz) / 10).clamp(185, 270);
        let factor = (256 + (112 - i32::from(illumination[index])) / 10 + (synthetic - 225) / 5)
            .clamp(226, 286);
        let mut output = [0_u8; 3];
        for channel in 0..3 {
            output[channel] =
                u8::try_from(((i32::from(pixel[channel]) * factor + 128) / 256).clamp(0, 255))
                    .expect("clamped relit channel fits");
        }
        pixels.push(output);
    }
    RgbImage::new(image.width, image.height, pixels)
}

pub(crate) fn structural_edges(
    normals: &RgbImage,
    depth: &[u32],
) -> Result<Vec<bool>, StylizeError> {
    if depth.len() != normals.pixels.len() {
        return Err(StylizeError::Invalid(
            "structural edge guidance is not registered".into(),
        ));
    }
    let mut output = vec![false; depth.len()];
    for y in 0..normals.height {
        for x in 0..normals.width {
            let index = normals.index(x, y);
            for (nx, ny) in [(x.saturating_sub(1), y), (x, y.saturating_sub(1))] {
                if nx == x && ny == y {
                    continue;
                }
                let neighbor = normals.index(nx, ny);
                let normal_difference = (0..3)
                    .map(|channel| {
                        normals.pixels[index][channel].abs_diff(normals.pixels[neighbor][channel])
                    })
                    .map(u16::from)
                    .sum::<u16>();
                let depth_difference = if depth[index] == 0 || depth[neighbor] == 0 {
                    u32::MAX
                } else {
                    depth[index].abs_diff(depth[neighbor])
                };
                if normal_difference > 190 || depth_difference > 2_000 {
                    output[index] = true;
                    output[neighbor] = true;
                }
            }
        }
    }
    Ok(output)
}

fn dilate(mask: &[bool], width: usize, height: usize) -> Vec<bool> {
    let mut output = vec![false; mask.len()];
    for y in 0..height {
        for x in 0..width {
            let mut value = false;
            for dy in -1_isize..=1 {
                for dx in -1_isize..=1 {
                    let Some(nx) = x.checked_add_signed(dx) else {
                        continue;
                    };
                    let Some(ny) = y.checked_add_signed(dy) else {
                        continue;
                    };
                    if nx < width && ny < height {
                        value |= mask[ny * width + nx];
                    }
                }
            }
            output[y * width + x] = value;
        }
    }
    output
}

fn erode(mask: &[bool], width: usize, height: usize) -> Vec<bool> {
    let mut output = vec![false; mask.len()];
    for y in 0..height {
        for x in 0..width {
            let mut value = true;
            for dy in -1_isize..=1 {
                for dx in -1_isize..=1 {
                    let Some(nx) = x.checked_add_signed(dx) else {
                        value = false;
                        continue;
                    };
                    let Some(ny) = y.checked_add_signed(dy) else {
                        value = false;
                        continue;
                    };
                    value &= nx < width && ny < height && mask[ny * width + nx];
                }
            }
            output[y * width + x] = value;
        }
    }
    output
}

fn remove_small_components(mask: &mut [bool], width: usize, height: usize, minimum: usize) {
    let mut visited = vec![false; mask.len()];
    let mut queue = Vec::new();
    for start in 0..mask.len() {
        if !mask[start] || visited[start] {
            continue;
        }
        queue.clear();
        queue.push(start);
        visited[start] = true;
        let mut cursor = 0;
        while cursor < queue.len() {
            let index = queue[cursor];
            cursor += 1;
            let x = index % width;
            let y = index / width;
            for (dx, dy) in [(-1_isize, 0_isize), (1, 0), (0, -1), (0, 1)] {
                let Some(nx) = x.checked_add_signed(dx) else {
                    continue;
                };
                let Some(ny) = y.checked_add_signed(dy) else {
                    continue;
                };
                if nx >= width || ny >= height {
                    continue;
                }
                let neighbor = ny * width + nx;
                if mask[neighbor] && !visited[neighbor] {
                    visited[neighbor] = true;
                    queue.push(neighbor);
                }
            }
        }
        if queue.len() < minimum {
            for index in &queue {
                mask[*index] = false;
            }
        }
    }
}

pub(crate) fn canopy_mask(image: &RgbImage, normals: &RgbImage) -> Result<Vec<bool>, StylizeError> {
    if image.width != normals.width || image.height != normals.height {
        return Err(StylizeError::Invalid(
            "canopy guidance is not registered to color".into(),
        ));
    }
    let normal_channels = (0..3)
        .map(|channel| {
            normals
                .pixels
                .iter()
                .map(|pixel| pixel[channel])
                .collect::<Vec<_>>()
        })
        .collect::<Vec<_>>();
    let normal_means = normal_channels
        .iter()
        .map(|channel| box_blur(channel, image.width, image.height, 2))
        .collect::<Vec<_>>();
    let mut mask = vec![false; image.pixels.len()];
    for (index, pixel) in image.pixels.iter().copied().enumerate() {
        let red = i32::from(pixel[0]);
        let green = i32::from(pixel[1]);
        let blue = i32::from(pixel[2]);
        let luma = i32::from(luminance(pixel));
        let roughness = (0..3)
            .map(|channel| normals.pixels[index][channel].abs_diff(normal_means[channel][index]))
            .map(i32::from)
            .sum::<i32>();
        let green_evidence = 2 * green - red - blue > 5 && 100 * green > 98 * blue && red < 175;
        let red_exclusion = 100 * red > 125 * green && 100 * red > 130 * blue;
        mask[index] = green_evidence || (roughness > 23 && luma < 105 && !red_exclusion);
    }
    mask = erode(
        &dilate(&mask, image.width, image.height),
        image.width,
        image.height,
    );
    remove_small_components(&mut mask, image.width, image.height, 64);
    Ok(mask)
}

pub(crate) fn replace_canopy(
    baseline: &RgbImage,
    source: &RgbImage,
    normals: &RgbImage,
    mask: &[bool],
    palette: &[Rgb],
) -> Result<RgbImage, StylizeError> {
    if mask.len() != baseline.pixels.len()
        || baseline.width != source.width
        || baseline.height != source.height
        || baseline.width != normals.width
        || baseline.height != normals.height
        || palette.len() < 2
    {
        return Err(StylizeError::Invalid(
            "canopy replacement inputs are not registered".into(),
        ));
    }
    let luma = source
        .pixels
        .iter()
        .copied()
        .map(luminance)
        .collect::<Vec<_>>();
    let crown_luma = box_blur(&luma, source.width, source.height, 4);
    let mut pixels = baseline.pixels.clone();
    for index in 0..pixels.len() {
        if !mask[index] {
            continue;
        }
        let normal = normals.pixels[index];
        let nx = i32::from(normal[0]) - 128;
        let ny = i32::from(normal[1]) - 128;
        let nz = i32::from(normal[2]) - 128;
        let light = i32::from(crown_luma[index]) + (-nx + ny + 2 * nz) / 24;
        let shade =
            usize::try_from(((light - 18) / 20).clamp(0, 6)).expect("clamped canopy shade fits");
        pixels[index] = palette[shade.min(palette.len() - 1)];
    }
    for y in 0..baseline.height {
        for x in 0..baseline.width {
            let index = baseline.index(x, y);
            if !mask[index] {
                continue;
            }
            let boundary = [(-1_isize, 0_isize), (1, 0), (0, -1), (0, 1)]
                .into_iter()
                .any(|(dx, dy)| {
                    let Some(nx) = x.checked_add_signed(dx) else {
                        return true;
                    };
                    let Some(ny) = y.checked_add_signed(dy) else {
                        return true;
                    };
                    nx >= baseline.width || ny >= baseline.height || !mask[ny * baseline.width + nx]
                });
            if boundary {
                pixels[index] = palette[0];
            }
        }
    }
    RgbImage::new(baseline.width, baseline.height, pixels)
}

pub(crate) fn apply_structural_outlines(
    image: &RgbImage,
    structural: &[bool],
    canopy: &[bool],
    outline: Rgb,
) -> Result<RgbImage, StylizeError> {
    if structural.len() != image.pixels.len() || canopy.len() != image.pixels.len() {
        return Err(StylizeError::Invalid(
            "structural outline inputs are not registered".into(),
        ));
    }
    let mut pixels = image.pixels.clone();
    for index in 0..pixels.len() {
        if structural[index] && !mask_interior(canopy, index, image.width, image.height) {
            pixels[index] = outline;
        }
    }
    RgbImage::new(image.width, image.height, pixels)
}

fn mask_interior(mask: &[bool], index: usize, width: usize, height: usize) -> bool {
    let x = index % width;
    let y = index / width;
    x > 0
        && y > 0
        && x + 1 < width
        && y + 1 < height
        && mask[index - 1]
        && mask[index + 1]
        && mask[index - width]
        && mask[index + width]
}

pub(crate) fn mask_image(
    mask: &[bool],
    width: usize,
    height: usize,
) -> Result<RgbImage, StylizeError> {
    let pixels = mask
        .iter()
        .map(|value| {
            if *value {
                [0xff, 0xff, 0xff]
            } else {
                [0, 0, 0]
            }
        })
        .collect();
    RgbImage::new(width, height, pixels)
}

pub(crate) fn output_edges(image: &RgbImage) -> Vec<bool> {
    let luma = image
        .pixels
        .iter()
        .copied()
        .map(luminance)
        .collect::<Vec<_>>();
    let mut output = vec![false; luma.len()];
    for y in 0..image.height {
        for x in 0..image.width {
            let index = image.index(x, y);
            if x > 0 && luma[index].abs_diff(luma[index - 1]) > 18 {
                output[index] = true;
                output[index - 1] = true;
            }
            if y > 0 && luma[index].abs_diff(luma[index - image.width]) > 18 {
                output[index] = true;
                output[index - image.width] = true;
            }
        }
    }
    output
}

#[cfg(test)]
mod tests {
    use super::*;

    fn image(width: usize, height: usize, value: Rgb) -> RgbImage {
        RgbImage::new(width, height, vec![value; width * height]).expect("fixture")
    }

    #[test]
    fn two_by_two_reduction_is_exact() {
        let source = RgbImage::new(2, 2, vec![[0, 0, 0], [4, 8, 12], [8, 12, 16], [12, 16, 20]])
            .expect("fixture");
        assert_eq!(reduce_rgb_2x(&source).expect("reduce").pixels, [[6, 9, 12]]);
    }

    #[test]
    fn smoothing_does_not_cross_a_hard_color_boundary() {
        let mut source = image(6, 4, [20, 20, 20]);
        for y in 0..4 {
            for x in 3..6 {
                source.pixels[y * 6 + x] = [220, 220, 220];
            }
        }
        assert_eq!(
            edge_aware_smooth(&source, None, None, 3).expect("smooth"),
            source
        );
    }

    #[test]
    fn canopy_replacement_cannot_capture_a_small_car_like_component() {
        let mut color = image(16, 16, [100, 95, 90]);
        let mut normals = image(16, 16, [128, 220, 215]);
        for y in 7..9 {
            for x in 6..10 {
                let index = y * 16 + x;
                color.pixels[index] = [20, 48, 22];
                normals.pixels[index] = [40, 100, 220];
            }
        }
        assert!(
            canopy_mask(&color, &normals)
                .expect("mask")
                .into_iter()
                .all(|value| !value)
        );
    }
}
