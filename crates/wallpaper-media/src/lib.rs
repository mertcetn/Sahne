//! Bounded native image data for fixed fixtures; no filesystem, network or GPU access.
use std::{collections::BTreeMap, fmt, io::Cursor};
use wallpaper_scene::{MAX_DECODED_IMAGE_BYTES, MAX_IMAGE_DIMENSION, ValidatedScene};

pub const MAX_ENCODED_IMAGE_BYTES: usize = 16 * 1024 * 1024;
pub const MAX_PNG_CHUNKS: usize = 4096;
pub const DECODER_INTERNAL_BYTES: usize = 8 * 1024 * 1024;

/// Structured, bounded errors. Upstream messages and input bytes are never retained.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Error {
    BudgetExceeded,
    InvalidDimensions,
    InvalidPixels,
    InvalidPng,
    UnsupportedPng,
    Checksum,
    Decode,
    AssetMismatch,
}
impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Image {self:?}")
    }
}
impl std::error::Error for Error {}
pub type Result<T> = std::result::Result<T, Error>;

/// Immutable, tightly packed, straight-alpha sRGB RGBA8 data.
/// Construction checks dimensions and exact byte length; identity is scene-local.
#[derive(Debug, PartialEq, Eq)]
pub struct RgbaImage {
    width: u32,
    height: u32,
    pixels: Vec<u8>,
}
impl RgbaImage {
    pub fn from_rgba8(width: u32, height: u32, pixels: Vec<u8>) -> Result<Self> {
        if pixels.len() != pixel_bytes(width, height)? {
            return Err(Error::InvalidPixels);
        }
        Ok(Self {
            width,
            height,
            pixels,
        })
    }
    pub fn width(&self) -> u32 {
        self.width
    }
    pub fn height(&self) -> u32 {
        self.height
    }
    pub fn pixels(&self) -> &[u8] {
        &self.pixels
    }
}

fn pixel_bytes(width: u32, height: u32) -> Result<usize> {
    if width == 0 || height == 0 || width > MAX_IMAGE_DIMENSION || height > MAX_IMAGE_DIMENSION {
        return Err(Error::InvalidDimensions);
    }
    usize::try_from(u64::from(width) * u64::from(height) * 4).map_err(|_| Error::BudgetExceeded)
}

/// Deliberately narrow PNG contract: IHDR, optional sRGB, consecutive IDAT, IEND.
/// Reject unsupported metadata/animation rather than silently changing semantics.
fn inspect_png(bytes: &[u8], remaining_bytes: u64) -> Result<(u32, u32, usize)> {
    if bytes.len() > MAX_ENCODED_IMAGE_BYTES {
        return Err(Error::BudgetExceeded);
    }
    if !bytes.starts_with(b"\x89PNG\r\n\x1a\n") {
        return Err(Error::InvalidPng);
    }
    let mut offset = 8usize;
    let mut dimensions = None;
    let mut srgb = false;
    let mut idat = false;
    for chunk_index in 0..MAX_PNG_CHUNKS {
        let header = bytes.get(offset..offset + 8).ok_or(Error::InvalidPng)?;
        let length = u32::from_be_bytes(header[..4].try_into().unwrap()) as usize;
        let end = offset
            .checked_add(12)
            .and_then(|v| v.checked_add(length))
            .ok_or(Error::InvalidPng)?;
        let chunk = bytes.get(offset..end).ok_or(Error::InvalidPng)?;
        let data = &chunk[8..8 + length];
        let crc = u32::from_be_bytes(chunk[8 + length..].try_into().unwrap());
        if crc32fast::hash(&chunk[4..8 + length]) != crc {
            return Err(Error::Checksum);
        }
        let kind = &header[4..8];
        if chunk_index == 0 && kind != b"IHDR" {
            return Err(Error::InvalidPng);
        }
        match kind {
            b"IHDR" if chunk_index == 0 && length == 13 => {
                let width = u32::from_be_bytes(data[..4].try_into().unwrap());
                let height = u32::from_be_bytes(data[4..8].try_into().unwrap());
                let size = pixel_bytes(width, height)?;
                if size as u64 > remaining_bytes.min(MAX_DECODED_IMAGE_BYTES) {
                    return Err(Error::BudgetExceeded);
                }
                if data[8..] != [8, 6, 0, 0, 0] {
                    return Err(Error::UnsupportedPng);
                }
                dimensions = Some((width, height, size));
            }
            b"sRGB" if !srgb && !idat && length == 1 && data[0] <= 3 => srgb = true,
            b"IDAT" => idat = true,
            b"IEND" if length == 0 && idat && end == bytes.len() => {
                return dimensions.ok_or(Error::InvalidPng);
            }
            b"IHDR" | b"sRGB" | b"IEND" => return Err(Error::InvalidPng),
            _ => return Err(Error::UnsupportedPng),
        }
        offset = end;
    }
    Err(Error::BudgetExceeded)
}

/// Decode only the documented subset, checking output budget before allocation.
/// The decoder's internal limit is best effort; OS isolation remains necessary
/// before accepting untrusted input in a production helper.
pub fn decode_png(bytes: &[u8], remaining_bytes: u64) -> Result<RgbaImage> {
    let (width, height, size) = inspect_png(bytes, remaining_bytes)?;
    let mut decoder = png::Decoder::new(Cursor::new(bytes));
    decoder.set_limits(png::Limits {
        bytes: DECODER_INTERNAL_BYTES,
    });
    decoder.ignore_checksums(false);
    let mut reader = decoder.read_info().map_err(|_| Error::Decode)?;
    if reader.info().width != width
        || reader.info().height != height
        || reader.output_buffer_size() != Some(size)
        || reader.output_color_type() != (png::ColorType::Rgba, png::BitDepth::Eight)
    {
        return Err(Error::Decode);
    }
    let mut pixels = Vec::new();
    pixels
        .try_reserve_exact(size)
        .map_err(|_| Error::BudgetExceeded)?;
    pixels.resize(size, 0);
    let frame = reader.next_frame(&mut pixels).map_err(|_| Error::Decode)?;
    if frame.width != width
        || frame.height != height
        || frame.buffer_size() != size
        || frame.color_type != png::ColorType::Rgba
        || frame.bit_depth != png::BitDepth::Eight
    {
        return Err(Error::Decode);
    }
    reader.finish().map_err(|_| Error::Decode)?;
    RgbaImage::from_rgba8(width, height, pixels)
}

/// Recheck an independently decoded scene against exact immutable pixel assets,
/// including unused declarations. No extra IDs or dimension substitutions.
pub fn validate_assets(scene: &ValidatedScene, images: &BTreeMap<u32, RgbaImage>) -> Result<()> {
    validate_resource_table(&scene.scene().resources, images)
}
/// Match immutable pixels against independently validated version-2 declarations.
pub fn validate_hierarchy_assets(
    scene: &wallpaper_scene::hierarchy::ValidatedHierarchy,
    images: &BTreeMap<u32, RgbaImage>,
) -> Result<()> {
    validate_resource_table(&scene.scene().resources, images)
}
fn validate_resource_table(
    resources: &[wallpaper_scene::ImageResource],
    images: &BTreeMap<u32, RgbaImage>,
) -> Result<()> {
    if images.len() != resources.len() {
        return Err(Error::AssetMismatch);
    }
    let mut total = 0u64;
    for resource in resources {
        let image = images.get(&resource.id).ok_or(Error::AssetMismatch)?;
        if image.width != resource.width || image.height != resource.height {
            return Err(Error::AssetMismatch);
        }
        total = total
            .checked_add(image.pixels.len() as u64)
            .ok_or(Error::BudgetExceeded)?;
        if total > MAX_DECODED_IMAGE_BYTES {
            return Err(Error::BudgetExceeded);
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests;
