use std::collections::HashMap;

use image::{DynamicImage, ImageReader, Limits, RgbImage, RgbaImage, imageops};
use zbus::zvariant::OwnedValue;

const MAX_IMAGE_DIMENSION: u32 = 4096;
const MAX_IMAGE_BYTES: u64 = 64 * 1024 * 1024;

#[derive(Clone)]
pub struct Image(RgbaImage);

impl Image {
    pub fn dimensions(&self) -> (u32, u32) {
        self.inner().dimensions()
    }

    pub fn inner(&self) -> &RgbaImage {
        &self.0
    }
}

pub fn decode_notification_image(
    hints: &HashMap<String, OwnedValue>,
    app_icon: &str,
    thumbnail_size: u32,
) -> Option<Image> {
    if thumbnail_size == 0 {
        return None;
    }

    hints
        .get("image-data")
        .or_else(|| hints.get("image_data"))
        .or_else(|| hints.get("icon_data"))
        .and_then(decode_raw_image)
        .or_else(|| {
            hints
                .get("image-path")
                .or_else(|| hints.get("image_path"))
                .and_then(decode_image_path_hint)
        })
        .or_else(|| decode_app_icon(app_icon))
        .map(|image| {
            Image(imageops::thumbnail(
                &image.0,
                thumbnail_size,
                thumbnail_size,
            ))
        })
}

fn decode_raw_image(value: &OwnedValue) -> Option<Image> {
    let (width, height, rowstride, has_alpha, bits_per_sample, channels, data): (
        i32,
        i32,
        i32,
        bool,
        i32,
        i32,
        Vec<u8>,
    ) = value.try_clone().ok()?.try_into().ok()?;

    let width = u32::try_from(width).ok()?;
    let height = u32::try_from(height).ok()?;
    let rowstride = usize::try_from(rowstride).ok()?;
    let bits_per_sample = u32::try_from(bits_per_sample).ok()?;
    let channels = usize::try_from(channels).ok()?;

    if width == 0
        || height == 0
        || width > MAX_IMAGE_DIMENSION
        || height > MAX_IMAGE_DIMENSION
        || bits_per_sample != 8
        || !matches!((has_alpha, channels), (false, 3) | (true, 4))
    {
        return None;
    }

    let packed_row_len = usize::try_from(width).ok()?.checked_mul(channels)?;
    let height_usize = usize::try_from(height).ok()?;
    let packed_len = packed_row_len.checked_mul(height_usize)?;
    let required_len = rowstride.checked_mul(height_usize)?;

    if packed_len > usize::try_from(MAX_IMAGE_BYTES).ok()?
        || rowstride < packed_row_len
        || data.len() < required_len
    {
        return None;
    }

    let mut pixels = Vec::with_capacity(packed_len);
    for row in data.chunks_exact(rowstride).take(height_usize) {
        pixels.extend_from_slice(&row[..packed_row_len]);
    }

    match (has_alpha, channels) {
        (false, 3) => RgbImage::from_raw(width, height, pixels)
            .map(DynamicImage::ImageRgb8)
            .map(|image| Image(image.to_rgba8())),
        (true, 4) => RgbaImage::from_raw(width, height, pixels)
            .map(DynamicImage::ImageRgba8)
            .map(|image| Image(image.to_rgba8())),
        _ => None,
    }
}

fn decode_image_path_hint(value: &OwnedValue) -> Option<Image> {
    let path = <&str>::try_from(value).ok()?;
    decode_image_path(path)
}

fn decode_app_icon(app_icon: &str) -> Option<Image> {
    if app_icon.starts_with('/') || app_icon.starts_with("file://") {
        decode_image_path(app_icon)
    } else {
        None
    }
}

fn decode_image_path(path: &str) -> Option<Image> {
    let path = match path.strip_prefix("file://") {
        Some(path) if path.starts_with('/') => path,
        Some(_) => return None,
        None => path,
    };

    let mut reader = ImageReader::open(path).ok()?.with_guessed_format().ok()?;
    let mut limits = Limits::default();
    limits.max_image_width = Some(MAX_IMAGE_DIMENSION);
    limits.max_image_height = Some(MAX_IMAGE_DIMENSION);
    limits.max_alloc = Some(MAX_IMAGE_BYTES);
    reader.limits(limits);
    reader.decode().ok().map(|image| Image(image.to_rgba8()))
}
