//! Fixed fixture-only root-texture checks, never a desktop/screenshot interface.
use super::Result;
use std::{sync::mpsc, time::Duration};

const STRIDE: usize = 256;
const SAMPLES: [Sample; 18] = [
    Sample::blend(
        "nested blue overlap",
        190,
        270,
        [0, 0, 255],
        0.25,
        [12, 17, 22],
    ),
    Sample::blend("nested red", 110, 190, [255, 0, 0], 0.25, [12, 17, 22]),
    Sample::blend(
        "outer white badge",
        410,
        190,
        [255, 255, 255],
        0.5,
        [12, 17, 22],
    ),
    Sample::exact("inner clip outside", 190, 410, [12, 17, 22]),
    Sample::exact("PNG top left", 670, 240, [32, 160, 128]),
    Sample::exact("PNG top right", 820, 240, [230, 160, 64]),
    Sample::exact("PNG bottom left", 670, 290, [40, 64, 128]),
    Sample::blend(
        "PNG bottom right alpha",
        820,
        290,
        [230, 64, 96],
        128.0 / 255.0,
        [31, 42, 51],
    ),
    Sample::exact("low left inside", 1240, 181, [255; 3]),
    Sample::exact("high left outside", 1400, 181, [31, 42, 51]),
    Sample::exact("low right outside", 1340, 200, [31, 42, 51]),
    Sample::exact("high right inside", 1500, 200, [255; 3]),
    Sample::exact("low top inside", 1250, 180, [255; 3]),
    Sample::exact("high top outside", 1410, 180, [31, 42, 51]),
    Sample::exact("low bottom outside", 1250, 280, [31, 42, 51]),
    Sample::exact("high bottom inside", 1410, 280, [255; 3]),
    Sample::exact("empty clip", 1560, 200, [31, 42, 51]),
    Sample::exact("canvas control", 20, 500, [12, 17, 22]),
];
const SIZE: usize = STRIDE * SAMPLES.len();

struct Sample {
    name: &'static str,
    x: u32,
    y: u32,
    foreground: [u8; 3],
    alpha: f64,
    background: [u8; 3],
    tolerance: u8,
}
impl Sample {
    const fn exact(name: &'static str, x: u32, y: u32, color: [u8; 3]) -> Self {
        Self {
            name,
            x,
            y,
            foreground: color,
            alpha: 1.0,
            background: color,
            tolerance: 1,
        }
    }
    const fn blend(
        name: &'static str,
        x: u32,
        y: u32,
        foreground: [u8; 3],
        alpha: f64,
        background: [u8; 3],
    ) -> Self {
        Self {
            name,
            x,
            y,
            foreground,
            alpha,
            background,
            tolerance: 2,
        }
    }
    fn expected(&self) -> [u8; 4] {
        let mut result = [255; 4];
        for (i, out) in result[..3].iter_mut().enumerate() {
            let linear = decode(self.foreground[i]) * self.alpha
                + decode(self.background[i]) * (1.0 - self.alpha);
            let encoded = if linear <= 0.0031308 {
                linear * 12.92
            } else {
                1.055 * linear.powf(1.0 / 2.4) - 0.055
            };
            *out = (encoded * 255.0).round() as u8;
        }
        result
    }
    fn matches(&self, actual: &[u8]) -> bool {
        let expected = self.expected();
        actual.len() == 4
            && actual[3] == 255
            && actual[..3]
                .iter()
                .zip(expected)
                .all(|(&a, e)| a.abs_diff(e) <= self.tolerance)
    }
}
fn decode(value: u8) -> f64 {
    let encoded = f64::from(value) / 255.0;
    if encoded <= 0.04045 {
        encoded / 12.92
    } else {
        ((encoded + 0.055) / 1.055).powf(2.4)
    }
}

pub struct Readback(wgpu::Buffer);
impl Readback {
    pub fn encode(
        device: &wgpu::Device,
        encoder: &mut wgpu::CommandEncoder,
        source: &wgpu::Texture,
    ) -> Result<Self> {
        if source.size()
            != (wgpu::Extent3d {
                width: 1920,
                height: 1080,
                depth_or_array_layers: 1,
            })
            || source.format() != wgpu::TextureFormat::Rgba8UnormSrgb
        {
            return Err("Pixel test requires the fixed 1920x1080 RGBA8 sRGB root".into());
        }
        let buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Fixed 18-pixel readback"),
            size: SIZE as u64,
            usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
            mapped_at_creation: false,
        });
        for (i, sample) in SAMPLES.iter().enumerate() {
            encoder.copy_texture_to_buffer(
                wgpu::TexelCopyTextureInfo {
                    texture: source,
                    mip_level: 0,
                    origin: wgpu::Origin3d {
                        x: sample.x,
                        y: sample.y,
                        z: 0,
                    },
                    aspect: wgpu::TextureAspect::All,
                },
                wgpu::TexelCopyBufferInfo {
                    buffer: &buffer,
                    layout: wgpu::TexelCopyBufferLayout {
                        offset: (i * STRIDE) as u64,
                        bytes_per_row: Some(STRIDE as u32),
                        rows_per_image: Some(1),
                    },
                },
                wgpu::Extent3d {
                    width: 1,
                    height: 1,
                    depth_or_array_layers: 1,
                },
            );
        }
        Ok(Self(buffer))
    }
    pub fn verify(self, device: &wgpu::Device, submission: wgpu::SubmissionIndex) -> Result<()> {
        let (tx, rx) = mpsc::sync_channel(1);
        self.0
            .slice(..)
            .map_async(wgpu::MapMode::Read, move |result| {
                let _ = tx.try_send(result);
            });
        let result = (|| {
            device.poll(wgpu::PollType::Wait {
                submission_index: Some(submission),
                timeout: Some(Duration::from_secs(2)),
            })?;
            rx.try_recv()
                .map_err(|_| "Pixel mapping callback was not completed")??;
            let data = self.0.slice(..).get_mapped_range()?;
            validate(&data)?;
            Ok(())
        })();
        self.0.unmap();
        result
    }
}
fn validate(data: &[u8]) -> Result<()> {
    if data.len() != SIZE {
        return Err("Unexpected pixel readback size".into());
    }
    for (i, sample) in SAMPLES.iter().enumerate() {
        let actual = &data[i * STRIDE..i * STRIDE + 4];
        if !sample.matches(actual) {
            return Err(format!("GPU pixel mismatch: {} at ({},{}): actual={actual:?}, expected={:?}, RGB tolerance={}, alpha tolerance=0", sample.name, sample.x, sample.y, sample.expected(), sample.tolerance).into());
        }
        eprintln!(
            "GPU sample {} at ({},{}): actual={actual:?}, expected={:?}, RGB tolerance={}, alpha tolerance=0",
            sample.name,
            sample.x,
            sample.y,
            sample.expected(),
            sample.tolerance
        );
    }
    eprintln!(
        "GPU pixel test passed: samples={}, mapped bytes={SIZE}, root=1920x1080 RGBA8 sRGB",
        SAMPLES.len()
    );
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    fn reference() -> Vec<u8> {
        let mut data = vec![0xA5; SIZE];
        for (i, sample) in SAMPLES.iter().enumerate() {
            data[i * STRIDE..i * STRIDE + 4].copy_from_slice(&sample.expected());
        }
        data
    }
    #[test]
    fn fixed_copy_bounds_and_padding() {
        assert_eq!(SIZE, 4608);
        assert_eq!(STRIDE as u32, wgpu::COPY_BYTES_PER_ROW_ALIGNMENT);
        for sample in &SAMPLES {
            assert!(sample.x < 1920 && sample.y < 1080);
        }
        assert!(validate(&reference()).is_ok());
        assert!(validate(&reference()[..SIZE - 1]).is_err());
        assert!(validate(&vec![0; SIZE]).is_err());
    }
    #[test]
    fn fractional_coverage_and_alpha_cannot_pass_by_tolerance() {
        for i in 8..17 {
            let mut data = reference();
            let wrong = if SAMPLES[i].foreground == [255; 3] {
                [31, 42, 51, 255]
            } else {
                [255; 4]
            };
            data[i * STRIDE..i * STRIDE + 4].copy_from_slice(&wrong);
            assert!(validate(&data).is_err());
        }
        let mut data = reference();
        data[3] = 254;
        assert!(validate(&data).is_err());
    }
    #[test]
    fn independent_linear_blend_reference_and_error_bounds() {
        let white = Sample::blend("control", 0, 0, [255; 3], 0.5, [0; 3]);
        assert_eq!(white.expected(), [188, 188, 188, 255]);
        assert!(!white.matches(&[128, 128, 128, 255]));
        assert!(white.matches(&[190, 186, 188, 255]));
        assert!(!white.matches(&[191, 188, 188, 255]));
        // Losing either nested opacity stage must be distinguishable.
        let wrong = Sample::blend("wrong", 0, 0, [0, 0, 255], 0.5, [12, 17, 22]);
        assert!(!SAMPLES[0].matches(&wrong.expected()));
    }
}
