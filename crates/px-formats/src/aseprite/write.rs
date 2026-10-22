//! Escritor de `.aseprite` (Spec: Header, Frames, Chunk Types): un fotograma con el perfil sRGB,
//! la paleta (indexado), las capas y las celdas con contenido.

use miniz_oxide::deflate::compress_to_vec_zlib;
use px_core::{Document, Image, Mode};

use super::tables::*;
use super::{AsepriteError, Unsupported};

/// Nivel de compresión zlib de las celdas.
const ZLIB_LEVEL: u8 = 6;

/// Spec: Header (`Frame duration`, `Speed`). Milisegundos por fotograma.
const FRAME_MS: u16 = 100;

fn put_u16(out: &mut Vec<u8>, value: u16) {
    out.extend_from_slice(&value.to_le_bytes());
}

fn put_u32(out: &mut Vec<u8>, value: u32) {
    out.extend_from_slice(&value.to_le_bytes());
}

/// Spec: References, `STRING`.
fn put_string(out: &mut Vec<u8>, text: &str) {
    put_u16(out, text.len() as u16);
    out.extend_from_slice(text.as_bytes());
}

/// Spec: Frames. Un chunk: tamaño (incluye los 6 bytes de cabecera), tipo y datos.
fn chunk(kind: u16, data: &[u8]) -> Result<Vec<u8>, AsepriteError> {
    let size = u32::try_from(data.len() + CHUNK_HEADER_SIZE).map_err(|_| too_large())?;
    let mut out = Vec::with_capacity(data.len() + CHUNK_HEADER_SIZE);
    put_u32(&mut out, size);
    put_u16(&mut out, kind);
    out.extend_from_slice(data);
    Ok(out)
}

fn too_large() -> AsepriteError {
    AsepriteError::TooLarge {
        limit: u64::from(u32::MAX),
    }
}

/// Spec: Header, Frames y Chunk Types. Escribe `doc` como un `.aseprite` de un fotograma.
///
/// Solo RGBA e indexado; un indexado necesita índice transparente (el formato siempre tiene uno).
pub fn write_aseprite(doc: &Document) -> Result<Vec<u8>, AsepriteError> {
    let mode = doc.mode();
    let depth = match mode {
        Mode::Rgba => DEPTH_RGBA,
        Mode::Indexed => DEPTH_INDEXED,
        _ => return Err(AsepriteError::Unsupported(Unsupported::Grayscale)),
    };
    let limit = u64::from(u16::MAX);
    if u64::from(doc.width()) > limit || u64::from(doc.height()) > limit {
        return Err(AsepriteError::TooLarge { limit });
    }
    let (palette_len, transparent) = match doc.palette() {
        Some(palette) => (
            palette.len(),
            palette
                .transparent_index()
                .ok_or(AsepriteError::NoTransparentIndex)?,
        ),
        None => (0, 0),
    };

    let mut chunks: Vec<Vec<u8>> = Vec::new();
    // Spec: Color Profile Chunk. Tipo 1 (sRGB), sin gamma fija.
    let mut profile = Vec::new();
    put_u16(&mut profile, PROFILE_SRGB);
    put_u16(&mut profile, 0);
    put_u32(&mut profile, 0);
    profile.extend_from_slice(&[0; 8]);
    chunks.push(chunk(CHUNK_COLOR_PROFILE, &profile)?);

    // Spec: Palette Chunk (0x2019). Todas las entradas, con alfa y sin nombre.
    if let Some(palette) = doc.palette() {
        let mut data = Vec::new();
        put_u32(&mut data, palette_len as u32);
        put_u32(&mut data, 0);
        put_u32(&mut data, palette_len as u32 - 1);
        data.extend_from_slice(&[0; 8]);
        for color in palette.colors() {
            put_u16(&mut data, 0);
            data.extend_from_slice(&[color.r, color.g, color.b, color.a]);
        }
        chunks.push(chunk(CHUNK_PALETTE, &data)?);
    }

    // Spec: Layer Chunk (0x2004), NOTE.2. Las capas van por orden de posición, de abajo arriba.
    for layer in doc.layers() {
        let code = blend_to_code(layer.blend()).ok_or_else(|| {
            AsepriteError::Unsupported(Unsupported::BlendMode {
                code: u16::MAX,
                layer: layer.name().to_string(),
            })
        })?;
        let mut flags = 0;
        if layer.visible() {
            flags |= LAYER_VISIBLE;
        }
        if !layer.locked() {
            flags |= LAYER_EDITABLE;
        }
        let mut data = Vec::new();
        put_u16(&mut data, flags);
        put_u16(&mut data, LAYER_TYPE_NORMAL);
        put_u16(&mut data, 0); // nivel hijo
        put_u16(&mut data, 0); // ancho por defecto (ignorado)
        put_u16(&mut data, 0); // alto por defecto (ignorado)
        put_u16(&mut data, code);
        data.push(layer.opacity());
        data.extend_from_slice(&[0; 3]);
        put_string(&mut data, layer.name());
        chunks.push(chunk(CHUNK_LAYER, &data)?);
    }

    // Spec: Cel Chunk (0x2005), tipo 2. Solo el rectángulo con píxeles no transparentes.
    for (index, layer) in doc.layers().iter().enumerate() {
        let Some(image) = doc.image(layer.cel()) else {
            continue;
        };
        let Some((x, y, w, h)) = visible_rect(image, mode, transparent) else {
            continue;
        };
        // La posición es un `SHORT`: un origen más allá de 32767 se adelanta y la celda se ensancha.
        let (x_start, y_start) = (x.min(i16::MAX as u32), y.min(i16::MAX as u32));
        let (w, h) = (x + w - x_start, y + h - y_start);
        let bpp = mode.bytes_per_pixel();
        let mut raw = Vec::new();
        raw.try_reserve_exact(w as usize * h as usize * bpp)
            .map_err(|_| too_large())?;
        for row in y_start..y_start + h {
            let line = image
                .row(row)
                .ok_or(AsepriteError::Malformed("fila de la celda"))?;
            raw.extend_from_slice(&line[x_start as usize * bpp..(x_start + w) as usize * bpp]);
        }
        let mut data = Vec::new();
        put_u16(&mut data, index as u16);
        data.extend_from_slice(&(x_start as i16).to_le_bytes());
        data.extend_from_slice(&(y_start as i16).to_le_bytes());
        data.push(255);
        put_u16(&mut data, CEL_COMPRESSED);
        data.extend_from_slice(&0i16.to_le_bytes());
        data.extend_from_slice(&[0; 5]);
        put_u16(&mut data, w as u16);
        put_u16(&mut data, h as u16);
        data.extend_from_slice(&compress_to_vec_zlib(&raw, ZLIB_LEVEL));
        chunks.push(chunk(CHUNK_CEL, &data)?);
    }

    let chunk_bytes: usize = chunks.iter().map(Vec::len).sum();
    let total = HEADER_SIZE + FRAME_HEADER_SIZE + chunk_bytes;
    let file_size = u32::try_from(total).map_err(|_| too_large())?;
    let frame_size = u32::try_from(FRAME_HEADER_SIZE + chunk_bytes).map_err(|_| too_large())?;
    let count = chunks.len() as u32;

    let mut out = Vec::new();
    out.try_reserve_exact(total).map_err(|_| too_large())?;
    // Spec: Header.
    put_u32(&mut out, file_size);
    put_u16(&mut out, MAGIC_FILE);
    put_u16(&mut out, 1);
    put_u16(&mut out, doc.width() as u16);
    put_u16(&mut out, doc.height() as u16);
    put_u16(&mut out, depth);
    put_u32(&mut out, HEADER_LAYER_OPACITY);
    put_u16(&mut out, FRAME_MS);
    put_u32(&mut out, 0);
    put_u32(&mut out, 0);
    out.push(transparent);
    out.extend_from_slice(&[0; 3]);
    put_u16(&mut out, palette_len as u16);
    out.extend_from_slice(&[1, 1]); // razón de píxel 1:1
    out.extend_from_slice(&0i16.to_le_bytes()); // cuadrícula: x, y, ancho y alto
    out.extend_from_slice(&0i16.to_le_bytes());
    put_u16(&mut out, 16);
    put_u16(&mut out, 16);
    out.resize(HEADER_SIZE, 0);
    // Spec: Frames.
    put_u32(&mut out, frame_size);
    put_u16(&mut out, MAGIC_FRAME);
    put_u16(&mut out, u16::try_from(count).unwrap_or(u16::MAX));
    put_u16(&mut out, FRAME_MS);
    out.extend_from_slice(&[0; 2]);
    put_u32(&mut out, count);
    for chunk in chunks {
        out.extend_from_slice(&chunk);
    }
    Ok(out)
}

/// El rectángulo mínimo `(x, y, width, height)` que contiene todos los píxeles no transparentes, o
/// `None` si no hay ninguno (RGBA: alfa 0; indexado: el índice transparente).
fn visible_rect(image: &Image, mode: Mode, transparent: u8) -> Option<(u32, u32, u32, u32)> {
    let bpp = mode.bytes_per_pixel();
    let is_clear = |pixel: &[u8]| match mode {
        Mode::Indexed => pixel[0] == transparent,
        _ => pixel[3] == 0,
    };
    let (mut x0, mut y0, mut x1, mut y1) = (u32::MAX, u32::MAX, 0u32, 0u32);
    let mut any = false;
    for y in 0..image.height() {
        let row = image.row(y)?;
        for (x, pixel) in row.chunks_exact(bpp).enumerate() {
            if !is_clear(pixel) {
                any = true;
                let x = x as u32;
                x0 = x0.min(x);
                x1 = x1.max(x + 1);
                y0 = y0.min(y);
                y1 = y1.max(y + 1);
            }
        }
    }
    any.then(|| (x0, y0, x1 - x0, y1 - y0))
}
