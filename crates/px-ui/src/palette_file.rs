//! Palette files (specs/hito-4-paleta.md, unit 4.6): JASC-PAL (`.pal`), the plain-text palette of
//! GIMP (`.gpl`) and one `RRGGBB` per line (`.hex`). Pure text in and out, no dependencies.

use px_core::{Palette, Rgba};

/// A palette file format.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PaletteFormat {
    Pal,
    Gpl,
    Hex,
}

impl PaletteFormat {
    /// The format of a file extension (any case), if it is one of the three.
    pub fn from_extension(extension: &str) -> Option<PaletteFormat> {
        match extension.to_ascii_lowercase().as_str() {
            "pal" => Some(PaletteFormat::Pal),
            "gpl" => Some(PaletteFormat::Gpl),
            "hex" => Some(PaletteFormat::Hex),
            _ => None,
        }
    }
}

/// Why a palette file was rejected.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PaletteFileError {
    /// No colors at all.
    Empty,
    /// A JASC or GIMP header that is not what it should be.
    Header,
    /// The line (from 1) is not a color.
    Line(usize),
    /// A JASC file with fewer colors than it announces.
    Count,
}

/// The colors of a file (at most 256) and how many more it had.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LoadedPalette {
    pub colors: Vec<Rgba>,
    pub ignored: usize,
}

fn finish(mut colors: Vec<Rgba>) -> Result<LoadedPalette, PaletteFileError> {
    if colors.is_empty() {
        return Err(PaletteFileError::Empty);
    }
    let ignored = colors.len().saturating_sub(Palette::MAX_COLORS);
    colors.truncate(Palette::MAX_COLORS);
    Ok(LoadedPalette { colors, ignored })
}

fn channels(text: &str, line: usize) -> Result<Rgba, PaletteFileError> {
    let mut parts = text.split_whitespace();
    let mut next = || {
        parts
            .next()
            .and_then(|part| part.parse::<u8>().ok())
            .ok_or(PaletteFileError::Line(line))
    };
    Ok(Rgba::opaque(next()?, next()?, next()?))
}

/// Reads a palette; the format is detected from the content.
pub fn parse_palette(text: &str) -> Result<LoadedPalette, PaletteFileError> {
    let text = text.strip_prefix('\u{feff}').unwrap_or(text);
    let mut lines = text.lines().map(str::trim).enumerate();
    let first = text.lines().map(str::trim).find(|line| !line.is_empty());
    let mut colors = Vec::new();
    match first {
        Some("JASC-PAL") => {
            lines.next();
            let version = lines.next().map(|(_, line)| line);
            let count = lines
                .next()
                .and_then(|(_, line)| line.parse::<usize>().ok());
            let (Some("0100"), Some(count)) = (version, count) else {
                return Err(PaletteFileError::Header);
            };
            for (index, line) in lines.filter(|(_, line)| !line.is_empty()).take(count) {
                colors.push(channels(line, index + 1)?);
            }
            if colors.len() < count {
                return Err(PaletteFileError::Count);
            }
        }
        Some("GIMP Palette") => {
            for (index, line) in lines.skip(1) {
                let skip = line.is_empty()
                    || line.starts_with('#')
                    || line.starts_with("Name:")
                    || line.starts_with("Columns:");
                if !skip {
                    colors.push(channels(line, index + 1)?);
                }
            }
        }
        _ => {
            for (index, line) in lines {
                let line = line.trim_start_matches('#');
                if line.is_empty() || text_is_comment(line) {
                    continue;
                }
                let color = (line.len() == 6)
                    .then(|| Rgba::from_hex(line).ok())
                    .flatten()
                    .ok_or(PaletteFileError::Line(index + 1))?;
                colors.push(Rgba::opaque(color.r, color.g, color.b));
            }
        }
    }
    finish(colors)
}

/// A `.hex` comment line: `#` followed by a space or text that is not six hex digits.
fn text_is_comment(line: &str) -> bool {
    line.starts_with(' ') || (line.len() != 6 && !line.chars().all(|c| c.is_ascii_hexdigit()))
}

/// Writes the palette (alpha is not saved) in `format`.
pub fn write_palette(colors: &[Rgba], format: PaletteFormat) -> String {
    let mut out = String::new();
    match format {
        PaletteFormat::Pal => {
            out.push_str(&format!("JASC-PAL\r\n0100\r\n{}\r\n", colors.len()));
            for c in colors {
                out.push_str(&format!("{} {} {}\r\n", c.r, c.g, c.b));
            }
        }
        PaletteFormat::Gpl => {
            out.push_str("GIMP Palette\nName: Pixoko\nColumns: 16\n#\n");
            for c in colors {
                out.push_str(&format!("{:3} {:3} {:3} Untitled\n", c.r, c.g, c.b));
            }
        }
        PaletteFormat::Hex => {
            for c in colors {
                out.push_str(&format!("{:02X}{:02X}{:02X}\n", c.r, c.g, c.b));
            }
        }
    }
    out
}
