//! Lectura y escritura de PNG sobre bytes en memoria (specs/hito-1-png.md).

use std::error::Error;
use std::fmt;
use std::io::Cursor;

use png::{BitDepth, ColorType, Decoder, Encoder, Limits, Transformations};
use px_core::{Image, Mode, Palette, Pixel, Rgba};

use crate::aux_chunks::{AuxReport, AuxScan};

/// Memoria que se concede al crate `png` para sus propios búferes (filas y chunks).
const CRATE_LIMIT_BYTES: usize = 64 << 20;

/// Lo que se convirtió al leer un PNG que `Image` no representa tal cual.
///
/// Una variante aparece como mucho una vez, en el orden en que se declaran aquí.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum Conversion {
    /// Gris de 1, 2 o 4 bits llevado a 8.
    DepthExpanded { from_bits: u8 },
    /// Muestras de 16 bits reducidas a 8.
    Depth16Reduced,
    /// El fichero era RGB (tipo 2); la imagen es `Rgba` con alfa.
    RgbToRgba,
    /// tRNS de gris o RGB (color clave) convertido en alfa por píxel.
    ColorKeyToAlpha,
}

/// Resultado de leer un PNG: la imagen y las conversiones que hizo falta aplicar.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PngRead {
    pub image: Image,
    pub report: Vec<Conversion>,
    /// Chunks auxiliares que se ignoran (no cambian ningún valor de píxel ni el informe).
    pub aux: AuxReport,
}

impl PngRead {
    /// `true` si la imagen tiene exactamente los valores del fichero (sin conversiones).
    pub fn is_exact(&self) -> bool {
        self.report.is_empty()
    }
}

/// Error al leer o escribir un PNG.
#[derive(Clone, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum PngError {
    /// El crate `png` rechaza el fichero. El texto es el del crate y no es estable.
    Decode(String),
    /// Las dimensiones, o el búfer de muestras del fichero, superan `Image::MAX_BYTES`.
    TooLarge { width: u32, height: u32 },
    /// Indexado sin PLTE o con más entradas que `2^profundidad`.
    InvalidPalette,
    /// Un píxel apunta fuera de PLTE.
    PaletteIndexOutOfRange { index: u8, len: usize },
    /// tRNS con longitud incorrecta, clave fuera de rango o más entradas que PLTE.
    InvalidTrns,
    /// Hay bytes después del chunk IEND.
    TrailingData,
    /// (Escritura) paleta sin entradas.
    EmptyPalette,
    /// El crate `png` falló al escribir.
    Encode(String),
}

impl fmt::Display for PngError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            PngError::Decode(msg) => write!(f, "PNG inválido: {msg}"),
            PngError::TooLarge { width, height } => {
                write!(f, "un PNG de {width} × {height} supera el tamaño máximo")
            }
            PngError::InvalidPalette => write!(f, "paleta PLTE ausente o inválida"),
            PngError::PaletteIndexOutOfRange { index, len } => {
                write!(f, "índice {index} fuera de una paleta de {len} colores")
            }
            PngError::InvalidTrns => write!(f, "chunk tRNS inválido"),
            PngError::TrailingData => write!(f, "hay datos después del chunk IEND"),
            PngError::EmptyPalette => write!(f, "no se puede escribir una paleta vacía"),
            PngError::Encode(msg) => write!(f, "no se pudo escribir el PNG: {msg}"),
        }
    }
}

impl Error for PngError {}

fn decode_error(e: png::DecodingError) -> PngError {
    PngError::Decode(e.to_string())
}

fn encode_error(e: png::EncodingError) -> PngError {
    PngError::Encode(e.to_string())
}

/// Cómo se interpretan las muestras del fichero (ya validadas las paletas y los colores clave).
enum Plan {
    Indexed(Palette),
    Gray { key: Option<u16> },
    GrayAlpha,
    Rgb { key: Option<[u16; 3]> },
    Rgba,
}

impl Plan {
    fn mode(&self) -> Mode {
        match self {
            Plan::Indexed(_) => Mode::Indexed,
            Plan::Gray { key: None } => Mode::Gray,
            Plan::Gray { key: Some(_) } | Plan::GrayAlpha => Mode::GrayAlpha,
            Plan::Rgb { .. } | Plan::Rgba => Mode::Rgba,
        }
    }

    fn channels(&self) -> usize {
        match self {
            Plan::Indexed(_) | Plan::Gray { .. } => 1,
            Plan::GrayAlpha => 2,
            Plan::Rgb { .. } => 3,
            Plan::Rgba => 4,
        }
    }
}

/// Firma de 8 bytes de todo PNG.
const SIGNATURE: [u8; 8] = [0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A];

/// Mayor dimensión que admite el formato (2³¹ − 1).
const MAX_DIMENSION: u32 = i32::MAX as u32;

/// Lee solo las dimensiones `(ancho, alto)` de la cabecera (specs/hito-2-abrir-guardar.md).
///
/// Mira la firma y el primer chunk, que debe ser `IHDR` de 13 bytes. No decodifica, no reserva
/// memoria y **no verifica el CRC ni nada posterior al IHDR**: de eso se encarga `read_png`.
/// Una dimensión 0 o mayor que 2³¹ − 1 es un error.
pub fn png_dimensions(bytes: &[u8]) -> Result<(u32, u32), PngError> {
    let bad = |msg: &str| PngError::Decode(msg.to_string());
    // Firma (8) + longitud (4) + tipo (4) + ancho (4) + alto (4).
    let header = bytes
        .get(..24)
        .ok_or_else(|| bad("cabecera PNG incompleta"))?;
    if header[..8] != SIGNATURE {
        return Err(bad("firma PNG incorrecta"));
    }
    if header[8..12] != 13u32.to_be_bytes() || &header[12..16] != b"IHDR" {
        return Err(bad("el primer chunk no es un IHDR de 13 bytes"));
    }
    let be = |at: usize| {
        u32::from_be_bytes([header[at], header[at + 1], header[at + 2], header[at + 3]])
    };
    let (width, height) = (be(16), be(20));
    if !(1..=MAX_DIMENSION).contains(&width) || !(1..=MAX_DIMENSION).contains(&height) {
        return Err(bad("dimensiones fuera de rango"));
    }
    Ok((width, height))
}

/// Lee un PNG. Ver `specs/hito-1-png.md` para la correspondencia con los modos de `Image`.
pub fn read_png(bytes: &[u8]) -> Result<PngRead, PngError> {
    let mut decoder = Decoder::new_with_limits(
        Cursor::new(bytes),
        Limits {
            bytes: CRATE_LIMIT_BYTES,
        },
    );
    // Muestras crudas: las conversiones son las documentadas, no las del crate.
    decoder.set_transformations(Transformations::IDENTITY);
    // Por defecto el crate no verifica el Adler-32.
    decoder.ignore_checksums(false);
    decoder.set_ignore_text_chunk(true);
    decoder.set_ignore_iccp_chunk(true);
    let mut reader = decoder.read_info().map_err(decode_error)?;

    let info = reader.info();
    let (width, height) = (info.width, info.height);
    let depth = info.bit_depth as u8;
    // tRNS se lee del fichero crudo: el crate descarta en silencio el que es demasiado
    // corto y recorta el de gris y RGB a un byte por muestra por debajo de 16 bits.
    let (trns, aux) = scan_chunks(bytes)?;
    let plan = plan(info.color_type, depth, info.palette.as_deref(), trns)?;

    // Tamaño de la imagen y del búfer de muestras, antes de reservar nada.
    let too_large = || PngError::TooLarge { width, height };
    Image::byte_len(width, height, plan.mode()).map_err(|_| too_large())?;
    let raw_len = reader
        .output_buffer_size()
        .filter(|&len| len <= Image::MAX_BYTES)
        .ok_or_else(too_large)?;
    let row_len = reader.output_line_size(width).ok_or_else(too_large)?;
    let mut raw = Vec::new();
    raw.try_reserve_exact(raw_len).map_err(|_| too_large())?;
    raw.resize(raw_len, 0);

    reader.next_frame(&mut raw).map_err(decode_error)?;
    reader.finish().map_err(decode_error)?;

    let report = report(&plan, depth);
    let image = build_image(width, height, depth, &plan, &raw, row_len)?;
    Ok(PngRead { image, report, aux })
}

/// Valida PLTE y tRNS contra el tipo de color y decide cómo leer las muestras.
fn plan(
    color: ColorType,
    depth: u8,
    palette: Option<&[u8]>,
    trns: Option<&[u8]>,
) -> Result<Plan, PngError> {
    // Mayor valor de muestra que cabe en la profundidad.
    let max_sample = u16::try_from((1u32 << depth) - 1).unwrap_or(u16::MAX);
    let sample = |bytes: &[u8]| u16::from_be_bytes([bytes[0], bytes[1]]);
    match color {
        ColorType::Indexed => {
            let plte = palette.ok_or(PngError::InvalidPalette)?;
            let entries = plte.len() / 3;
            if entries == 0 || entries > (1usize << depth) {
                return Err(PngError::InvalidPalette);
            }
            let alphas = trns.unwrap_or(&[]);
            if alphas.len() > entries {
                return Err(PngError::InvalidTrns);
            }
            let colors = plte.as_chunks::<3>().0.iter().enumerate().map(|(i, rgb)| {
                Rgba::new(
                    rgb[0],
                    rgb[1],
                    rgb[2],
                    alphas.get(i).copied().unwrap_or(255),
                )
            });
            let palette = Palette::from_colors(colors).map_err(|_| PngError::InvalidPalette)?;
            Ok(Plan::Indexed(palette))
        }
        ColorType::Grayscale => {
            let key = match trns {
                None => None,
                Some(t) if t.len() == 2 && sample(t) <= max_sample => Some(sample(t)),
                Some(_) => return Err(PngError::InvalidTrns),
            };
            Ok(Plan::Gray { key })
        }
        ColorType::Rgb => {
            let key = match trns {
                None => None,
                Some(t) if t.len() == 6 => {
                    let key = [sample(&t[0..2]), sample(&t[2..4]), sample(&t[4..6])];
                    if key.iter().any(|&v| v > max_sample) {
                        return Err(PngError::InvalidTrns);
                    }
                    Some(key)
                }
                Some(_) => return Err(PngError::InvalidTrns),
            };
            Ok(Plan::Rgb { key })
        }
        // PNG prohíbe tRNS en estos tipos: se ignora.
        ColorType::GrayscaleAlpha => Ok(Plan::GrayAlpha),
        ColorType::Rgba => Ok(Plan::Rgba),
    }
}

/// Conversiones aplicadas, en el orden de [`Conversion`].
fn report(plan: &Plan, depth: u8) -> Vec<Conversion> {
    let mut report = Vec::new();
    if matches!(plan, Plan::Gray { .. }) && depth < 8 {
        report.push(Conversion::DepthExpanded { from_bits: depth });
    }
    if depth == 16 && !matches!(plan, Plan::Indexed(_)) {
        report.push(Conversion::Depth16Reduced);
    }
    if matches!(plan, Plan::Rgb { .. }) {
        report.push(Conversion::RgbToRgba);
    }
    if matches!(
        plan,
        Plan::Gray { key: Some(_) } | Plan::Rgb { key: Some(_) }
    ) {
        report.push(Conversion::ColorKeyToAlpha);
    }
    report
}

/// Recorre los chunks hasta IEND y devuelve los datos del tRNS previo al IDAT, si lo hay, y el
/// informe de los auxiliares ignorados.
///
/// El IEND debe estar completo y acabar en el último byte. El crate no detecta ni los datos
/// sobrantes ni un IEND cortado.
fn scan_chunks(bytes: &[u8]) -> Result<(Option<&[u8]>, AuxReport), PngError> {
    let truncated = || PngError::Decode("PNG truncado".to_string());
    let mut trns = None;
    let mut seen_idat = false;
    let mut aux = AuxScan::default();
    // La firma ya la validó el crate.
    let mut pos = 8usize;
    loop {
        let header = pos
            .checked_add(8)
            .and_then(|end| bytes.get(pos..end))
            .ok_or_else(truncated)?;
        let length = usize::try_from(u32::from_be_bytes([
            header[0], header[1], header[2], header[3],
        ]))
        .map_err(|_| truncated())?;
        let end = pos
            .checked_add(12)
            .and_then(|p| p.checked_add(length))
            .filter(|&end| end <= bytes.len())
            .ok_or_else(truncated)?;
        aux.chunk(&header[4..8], &bytes[pos + 8..end - 4]);
        match &header[4..8] {
            b"IDAT" => seen_idat = true,
            b"tRNS" if !seen_idat && trns.is_none() => trns = Some(&bytes[pos + 8..end - 4]),
            b"IEND" => {
                return if end == bytes.len() {
                    Ok((trns, aux.finish()))
                } else {
                    Err(PngError::TrailingData)
                };
            }
            _ => {}
        }
        pos = end;
    }
}

/// Muestra número `index` de una fila (en muestras, no en bytes).
fn sample(row: &[u8], depth: u8, index: usize) -> u16 {
    match depth {
        8 => u16::from(row[index]),
        16 => u16::from_be_bytes([row[2 * index], row[2 * index + 1]]),
        n => {
            let bit = index * usize::from(n);
            let shift = 8 - usize::from(n) - (bit % 8);
            u16::from((row[bit / 8] >> shift) & ((1u8 << n) - 1))
        }
    }
}

/// Lleva una muestra de `depth` bits a 8 bits (comportamiento 3 de la spec).
fn scale(value: u16, depth: u8) -> u8 {
    let value = u32::from(value);
    let scaled = match depth {
        8 => value,
        16 => (value + 128) / 257,
        n => value * 255 / ((1 << n) - 1),
    };
    u8::try_from(scaled).unwrap_or(u8::MAX)
}

fn build_image(
    width: u32,
    height: u32,
    depth: u8,
    plan: &Plan,
    raw: &[u8],
    row_len: usize,
) -> Result<Image, PngError> {
    let image_error = |e: px_core::ImageError| PngError::Decode(e.to_string());
    let mut image = match plan {
        Plan::Indexed(palette) => Image::new_indexed(width, height, palette.clone()),
        other => Image::new(width, height, other.mode()),
    }
    .map_err(image_error)?;
    let channels = plan.channels();
    if row_len == 0 || raw.len() / row_len != usize::try_from(height).unwrap_or(usize::MAX) {
        return Err(PngError::Decode("tamaño de imagen inesperado".to_string()));
    }

    for (y, row) in (0..height).zip(raw.chunks_exact(row_len)) {
        for x in 0..width {
            let first = usize::try_from(x).unwrap_or(usize::MAX) * channels;
            let s = |c: usize| sample(row, depth, first + c);
            let pixel = match plan {
                Plan::Indexed(palette) => {
                    let index = u8::try_from(s(0)).unwrap_or(u8::MAX);
                    if usize::from(index) >= palette.len() {
                        return Err(PngError::PaletteIndexOutOfRange {
                            index,
                            len: palette.len(),
                        });
                    }
                    Pixel::Indexed(index)
                }
                Plan::Gray { key: None } => Pixel::Gray(scale(s(0), depth)),
                Plan::Gray { key: Some(key) } => Pixel::GrayAlpha {
                    gray: scale(s(0), depth),
                    alpha: if s(0) == *key { 0 } else { 255 },
                },
                Plan::GrayAlpha => Pixel::GrayAlpha {
                    gray: scale(s(0), depth),
                    alpha: scale(s(1), depth),
                },
                Plan::Rgb { key } => {
                    let rgb = [s(0), s(1), s(2)];
                    Pixel::Rgba(Rgba::new(
                        scale(rgb[0], depth),
                        scale(rgb[1], depth),
                        scale(rgb[2], depth),
                        if *key == Some(rgb) { 0 } else { 255 },
                    ))
                }
                Plan::Rgba => Pixel::Rgba(Rgba::new(
                    scale(s(0), depth),
                    scale(s(1), depth),
                    scale(s(2), depth),
                    scale(s(3), depth),
                )),
            };
            image.set_pixel(x, y, pixel).map_err(image_error)?;
        }
    }
    Ok(image)
}

/// Escribe un PNG a 8 bits, sin entrelazar, con el tipo de color que corresponde al modo.
pub fn write_png(image: &Image) -> Result<Vec<u8>, PngError> {
    let (width, height) = (image.width(), image.height());
    let mut out = Vec::new();
    let mut encoder = Encoder::new(&mut out, width, height);
    encoder.set_depth(BitDepth::Eight);
    encoder.set_color(match image.mode() {
        Mode::Indexed => ColorType::Indexed,
        Mode::Gray => ColorType::Grayscale,
        Mode::GrayAlpha => ColorType::GrayscaleAlpha,
        Mode::Rgba => ColorType::Rgba,
    });
    if let Some(palette) = image.palette() {
        if palette.is_empty() {
            return Err(PngError::EmptyPalette);
        }
        let mut plte = Vec::with_capacity(palette.len() * 3);
        let mut alphas = Vec::with_capacity(palette.len());
        for (i, color) in palette.colors().iter().enumerate() {
            plte.extend_from_slice(&[color.r, color.g, color.b]);
            let index = u8::try_from(i).map_err(|_| PngError::Encode("paleta larga".into()))?;
            let resolved = palette
                .resolved(index)
                .map_err(|e| PngError::Encode(e.to_string()))?;
            alphas.push(resolved.a);
        }
        encoder.set_palette(plte);
        // tRNS llega hasta la última entrada con alfa distinto de 255; si no hay, se omite.
        if let Some(last) = alphas.iter().rposition(|&a| a != 255) {
            alphas.truncate(last + 1);
            encoder.set_trns(alphas);
        }
    }

    let len = Image::byte_len(width, height, image.mode())
        .map_err(|e| PngError::Encode(e.to_string()))?;
    let mut data = Vec::new();
    data.try_reserve_exact(len)
        .map_err(|_| PngError::Encode("sin memoria para los píxeles".to_string()))?;
    for y in 0..height {
        for x in 0..width {
            let pixel = image
                .get_pixel(x, y)
                .map_err(|e| PngError::Encode(e.to_string()))?;
            match pixel {
                Pixel::Rgba(c) => data.extend_from_slice(&[c.r, c.g, c.b, c.a]),
                Pixel::Indexed(i) | Pixel::Gray(i) => data.push(i),
                Pixel::GrayAlpha { gray, alpha } => data.extend_from_slice(&[gray, alpha]),
            }
        }
    }

    let mut writer = encoder.write_header().map_err(encode_error)?;
    writer.write_image_data(&data).map_err(encode_error)?;
    writer.finish().map_err(encode_error)?;
    Ok(out)
}
