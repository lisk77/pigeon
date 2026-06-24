use std::{
    collections::HashMap,
    sync::{Arc, Weak},
};

use image::{DynamicImage, ImageReader, Limits, RgbImage, RgbaImage, imageops};
use zbus::zvariant::{Array, OwnedValue, Structure, Value};

const MAX_IMAGE_DIMENSION: u32 = 4096;
const MAX_IMAGE_BYTES: usize = 64 * 1024 * 1024;

#[derive(Clone)]
pub struct Image(RgbaImage);

impl Image {
    pub fn dimensions(&self) -> (u32, u32) {
        self.0.dimensions()
    }

    pub fn inner(&self) -> &RgbaImage {
        &self.0
    }
}

#[derive(Hash, Eq, PartialEq)]
enum ImageKey {
    Path { path: String, size: u32 },
    AppIcon { icon: String, size: u32 },
}

#[derive(Default)]
pub struct ImageCache {
    entries: HashMap<ImageKey, Weak<Image>>,
}

impl ImageCache {
    pub fn thumbnail(
        &mut self,
        hints: &mut HashMap<String, OwnedValue>,
        app_icon: &str,
        thumbnail_size: u32,
    ) -> Option<Arc<Image>> {
        self.purge_dead();
        if thumbnail_size == 0 {
            discard_image_hints(hints);
            return None;
        }

        if let Some(raw) = take_raw_image_hint(hints) {
            let image = decode_raw_image(&raw);
            drop(raw);
            crate::memory::trim_free_heap_pages();
            let image = image?;
            let thumbnail = Arc::new(Image(imageops::thumbnail(
                image.inner(),
                thumbnail_size,
                thumbnail_size,
            )));
            return Some(thumbnail);
        }

        let path = take_path_hint(hints);
        let key = match path {
            Some(path) => ImageKey::Path {
                path,
                size: thumbnail_size,
            },
            None if !app_icon.is_empty() => ImageKey::AppIcon {
                icon: app_icon.to_owned(),
                size: thumbnail_size,
            },
            None => return None,
        };

        if let Some(image) = self.entries.get(&key).and_then(Weak::upgrade) {
            return Some(image);
        }

        let source = match &key {
            ImageKey::Path { path, .. } => decode_image_path(path),
            ImageKey::AppIcon { icon, .. } => decode_app_icon(icon),
        }?;
        let image = Arc::new(Image(imageops::thumbnail(
            source.inner(),
            thumbnail_size,
            thumbnail_size,
        )));
        self.entries.insert(key, Arc::downgrade(&image));
        Some(image)
    }

    pub fn purge_dead(&mut self) {
        self.entries.retain(|_, image| image.strong_count() > 0);
    }
}

fn take_raw_image_hint(hints: &mut HashMap<String, OwnedValue>) -> Option<OwnedValue> {
    hints
        .remove("image-data")
        .or_else(|| hints.remove("image_data"))
        .or_else(|| hints.remove("icon_data"))
}

fn take_path_hint(hints: &mut HashMap<String, OwnedValue>) -> Option<String> {
    hints
        .remove("image-path")
        .or_else(|| hints.remove("image_path"))
        .and_then(|value| String::try_from(value).ok())
}

fn discard_image_hints(hints: &mut HashMap<String, OwnedValue>) {
    let _ = take_raw_image_hint(hints);
    let _ = take_path_hint(hints);
}

fn decode_raw_image(value: &OwnedValue) -> Option<Image> {
    let structure = <&Structure>::try_from(value).ok()?;
    let fields = structure.fields();
    if fields.len() != 7 {
        return None;
    }

    let width = number::<i32>(&fields[0])?;
    let height = number::<i32>(&fields[1])?;
    let rowstride = number::<i32>(&fields[2])?;
    let has_alpha = boolean(&fields[3])?;
    let bits_per_sample = number::<i32>(&fields[4])?;
    let channels = number::<i32>(&fields[5])?;
    let bytes = byte_array(&fields[6])?;

    let width = u32::try_from(width).ok()?;
    let height = u32::try_from(height).ok()?;
    let rowstride = usize::try_from(rowstride).ok()?;
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
    let height = usize::try_from(height).ok()?;
    let packed_len = packed_row_len.checked_mul(height)?;
    let required_len = rowstride.checked_mul(height)?;
    if packed_len > MAX_IMAGE_BYTES
        || required_len > MAX_IMAGE_BYTES
        || rowstride < packed_row_len
        || bytes.len() < required_len
    {
        return None;
    }

    let pixels = if rowstride == packed_row_len {
        let mut bytes = bytes;
        bytes.truncate(packed_len);
        bytes
    } else {
        let mut compact = Vec::with_capacity(packed_len);
        for row in bytes.chunks_exact(rowstride).take(height) {
            compact.extend_from_slice(&row[..packed_row_len]);
        }
        compact
    };
    let width = u32::try_from(packed_row_len / channels).ok()?;
    let height = u32::try_from(height).ok()?;
    match (has_alpha, channels) {
        (true, 4) => RgbaImage::from_raw(width, height, pixels).map(Image),
        (false, 3) => RgbImage::from_raw(width, height, pixels)
            .map(DynamicImage::ImageRgb8)
            .map(|image| Image(image.to_rgba8())),
        _ => None,
    }
}

fn number<T>(value: &Value<'_>) -> Option<T>
where
    for<'a> T: TryFrom<&'a Value<'a>, Error = zbus::zvariant::Error>,
{
    T::try_from(value).ok()
}

fn boolean(value: &Value<'_>) -> Option<bool> {
    bool::try_from(value).ok()
}

fn byte_array(value: &Value<'_>) -> Option<Vec<u8>> {
    let array = <&Array>::try_from(value).ok()?;
    if array.len() > MAX_IMAGE_BYTES {
        return None;
    }
    array
        .inner()
        .iter()
        .map(|value| u8::try_from(value).ok())
        .collect()
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
    limits.max_alloc = Some(u64::try_from(MAX_IMAGE_BYTES).ok()?);
    reader.limits(limits);
    reader.decode().ok().map(|image| Image(image.to_rgba8()))
}
