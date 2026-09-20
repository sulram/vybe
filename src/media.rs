//! MEDIA — the core's only file format: PNG, in (`frames`) and out (headless
//! render). This is IO, so unlike everything expressive it returns `Result`
//! (Principle 3) — and its errors say what to do next.

use std::fs::File;
use std::io::{BufReader, BufWriter};
use std::path::Path;

/// Tightly packed RGBA8, straight alpha, rows top to bottom.
pub(crate) struct Pixels {
    pub width: u32,
    pub height: u32,
    pub rgba: Vec<u8>,
}

/// Decodes a PNG of any colour type into RGBA8.
pub(crate) fn load_png(path: &Path) -> Result<Pixels, String> {
    let file = File::open(path).map_err(|e| format!("{}: {e}", path.display()))?;
    let mut decoder = png::Decoder::new(BufReader::new(file));
    // Palettes expand, 16 bits narrow: every PNG arrives as 8-bit channels.
    decoder.set_transformations(png::Transformations::normalize_to_color8());
    let mut reader = decoder
        .read_info()
        .map_err(|e| format!("{}: {e}", path.display()))?;
    let size = reader
        .output_buffer_size()
        .ok_or_else(|| format!("{}: image too large", path.display()))?;
    let mut buf = vec![0; size];
    let info = reader
        .next_frame(&mut buf)
        .map_err(|e| format!("{}: {e}", path.display()))?;
    buf.truncate(info.buffer_size());

    let rgba = match info.color_type {
        png::ColorType::Rgba => buf,
        png::ColorType::Rgb => buf
            .chunks_exact(3)
            .flat_map(|p| [p[0], p[1], p[2], 255])
            .collect(),
        png::ColorType::Grayscale => buf.iter().flat_map(|&g| [g, g, g, 255]).collect(),
        png::ColorType::GrayscaleAlpha => buf
            .chunks_exact(2)
            .flat_map(|p| [p[0], p[0], p[0], p[1]])
            .collect(),
        png::ColorType::Indexed => {
            return Err(format!("{}: palette did not expand", path.display()));
        }
    };
    Ok(Pixels {
        width: info.width,
        height: info.height,
        rgba,
    })
}

/// Encodes RGBA8 (straight alpha, sRGB) as a PNG.
pub(crate) fn save_png(path: &Path, pixels: &Pixels) -> Result<(), String> {
    let file = File::create(path).map_err(|e| format!("{}: {e}", path.display()))?;
    let mut encoder = png::Encoder::new(BufWriter::new(file), pixels.width, pixels.height);
    encoder.set_color(png::ColorType::Rgba);
    encoder.set_depth(png::BitDepth::Eight);
    let mut writer = encoder
        .write_header()
        .map_err(|e| format!("{}: {e}", path.display()))?;
    writer
        .write_image_data(&pixels.rgba)
        .map_err(|e| format!("{}: {e}", path.display()))
}
