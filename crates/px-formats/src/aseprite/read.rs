//! Lector de `.aseprite` (Spec: Header, Frames, Chunk Types). El fichero no es de fiar: todo
//! desplazamiento y toda longitud se validan, y ningún tope depende de una longitud declarada
//! antes de comprobarse (specs/hito-3-aseprite.md, «Límites del lector»).

use miniz_oxide::inflate::decompress_to_vec_zlib_with_limit;
use px_core::{BlendMode, Document, Image, LayerSpec, Mode, Palette, Rgba};

use super::tables::*;
use super::{
    AsepriteError, AsepriteLimits, AsepriteRead, AsepriteReport, Ignored, IgnoredCounts,
    Unsupported,
};

/// Spec: Frames. Máximo de chunks del primer fotograma que se recorren. Un chunk mide al menos 6
/// bytes y recorrerlo no reserva nada: el tope es una política, no una defensa de memoria.
pub(super) const MAX_CHUNKS: u32 = 65_536;

/// Spec: Cel Chunk / NOTE.3. Una secuencia DEFLATE no expande más de 1032 veces: una celda que
/// declara más bytes de los que sus datos pueden dar se rechaza antes de descomprimir.
const MAX_DEFLATE_RATIO: u64 = 1032;

/// Lector de trozos del búfer: toda lectura comprueba límites y devuelve `Truncated`.
struct Cursor<'a> {
    data: &'a [u8],
    pos: usize,
}

impl<'a> Cursor<'a> {
    fn new(data: &'a [u8]) -> Cursor<'a> {
        Cursor { data, pos: 0 }
    }

    fn remaining(&self) -> usize {
        self.data.len() - self.pos
    }

    fn take(&mut self, n: usize) -> Result<&'a [u8], AsepriteError> {
        let end = self
            .pos
            .checked_add(n)
            .filter(|&end| end <= self.data.len())
            .ok_or(AsepriteError::Truncated)?;
        let slice = &self.data[self.pos..end];
        self.pos = end;
        Ok(slice)
    }

    fn rest(&mut self) -> &'a [u8] {
        let slice = &self.data[self.pos..];
        self.pos = self.data.len();
        slice
    }

    fn u8(&mut self) -> Result<u8, AsepriteError> {
        Ok(self.take(1)?[0])
    }

    fn u16(&mut self) -> Result<u16, AsepriteError> {
        let b = self.take(2)?;
        Ok(u16::from_le_bytes([b[0], b[1]]))
    }

    fn i16(&mut self) -> Result<i16, AsepriteError> {
        let b = self.take(2)?;
        Ok(i16::from_le_bytes([b[0], b[1]]))
    }

    fn u32(&mut self) -> Result<u32, AsepriteError> {
        let b = self.take(4)?;
        Ok(u32::from_le_bytes([b[0], b[1], b[2], b[3]]))
    }

    /// Spec: References, `STRING`: `WORD` de longitud y los bytes. `max` es el tope de la longitud.
    fn string(&mut self, max: usize, too_long: AsepriteError) -> Result<&'a [u8], AsepriteError> {
        let len = usize::from(self.u16()?);
        if len > max {
            return Err(too_long);
        }
        self.take(len)
    }
}

/// Spec: Header. Lo que la cabecera de 128 bytes dice del lienzo.
struct Header {
    frames: u16,
    width: u32,
    height: u32,
    mode: Mode,
    flags: u32,
    transparent: u8,
    colors: u16,
}

/// Spec: Header. Valida y lee la cabecera.
fn parse_header(bytes: &[u8]) -> Result<Header, AsepriteError> {
    if bytes.len() < 6 {
        return Err(AsepriteError::Truncated);
    }
    let mut cursor = Cursor::new(bytes);
    let file_size = cursor.u32()?;
    if cursor.u16()? != MAGIC_FILE {
        return Err(AsepriteError::NotAseprite);
    }
    if bytes.len() < HEADER_SIZE {
        return Err(AsepriteError::Truncated);
    }
    match usize::try_from(file_size) {
        Ok(size) if size == bytes.len() => {}
        Ok(size) if size > bytes.len() => return Err(AsepriteError::Truncated),
        _ => return Err(AsepriteError::Malformed("tamaño del fichero")),
    }
    let frames = cursor.u16()?;
    let width = u32::from(cursor.u16()?);
    let height = u32::from(cursor.u16()?);
    let depth = cursor.u16()?;
    let flags = cursor.u32()?;
    if frames == 0 {
        return Err(AsepriteError::Malformed("número de fotogramas"));
    }
    if width == 0 || height == 0 {
        return Err(AsepriteError::Malformed("tamaño del lienzo"));
    }
    let mode = match depth {
        DEPTH_RGBA => Mode::Rgba,
        DEPTH_INDEXED => Mode::Indexed,
        DEPTH_GRAY => return Err(AsepriteError::Unsupported(Unsupported::Grayscale)),
        _ => return Err(AsepriteError::Malformed("profundidad de color")),
    };
    // Spec: Header. Índice transparente en el desplazamiento 28 y número de colores en el 32.
    let transparent = bytes[28];
    let colors = u16::from_le_bytes([bytes[32], bytes[33]]);
    Ok(Header {
        frames,
        width,
        height,
        mode,
        flags,
        transparent,
        colors,
    })
}

/// Spec: Header. Ancho y alto del lienzo, sin leer nada más del fichero.
pub fn aseprite_dimensions(bytes: &[u8]) -> Result<(u32, u32), AsepriteError> {
    let header = parse_header(bytes)?;
    Ok((header.width, header.height))
}

/// Un chunk del primer fotograma: su tipo y sus datos (sin la cabecera de 6 bytes).
struct Chunk<'a> {
    kind: u16,
    data: &'a [u8],
}

/// Una capa a medio leer.
struct LayerBuild {
    name: String,
    visible: bool,
    locked: bool,
    background: bool,
    blend: BlendMode,
    opacity: u8,
    has_cel: bool,
    /// Píxeles de la celda del tamaño del lienzo, si tiene contenido.
    cell: Option<Vec<u8>>,
}

/// Spec: Header, Frames y Chunk Types. Lee las capas, las celdas y la paleta del primer
/// fotograma.
pub fn read_aseprite(bytes: &[u8], limits: &AsepriteLimits) -> Result<AsepriteRead, AsepriteError> {
    let header = parse_header(bytes)?;
    let pixels = u64::from(header.width) * u64::from(header.height);
    if pixels > limits.max_pixels {
        return Err(AsepriteError::TooLarge {
            limit: limits.max_pixels,
        });
    }
    let cel_bytes =
        Image::byte_len(header.width, header.height, header.mode).map_err(AsepriteError::Image)?;

    let chunks = frame_chunks(bytes, &header)?;
    let mut ignored = IgnoredCounts::default();
    let mut lossy = header.frames > 1;

    let palette = match header.mode {
        Mode::Indexed => Some(build_palette(&chunks, &header)?),
        _ => {
            if chunks.iter().any(|c| is_palette(c.kind)) {
                ignored.add(Ignored::RgbaPalette);
            }
            None
        }
    };
    let transparent = palette
        .as_ref()
        .and_then(|p: &Palette| p.transparent_index());
    // Spec: Palette Chunk. Valor con el que se rellena lo que una celda no cubre.
    let fill = match header.mode {
        Mode::Indexed => transparent.unwrap_or(0),
        _ => 0,
    };

    let mut layers: Vec<LayerBuild> = Vec::new();
    let mut materialized = 0usize;
    for chunk in &chunks {
        match chunk.kind {
            CHUNK_LAYER => {
                if layers.len() >= Document::MAX_LAYERS {
                    return Err(AsepriteError::TooManyLayers);
                }
                layers.push(parse_layer(chunk.data, &header, layers.len())?);
            }
            CHUNK_CEL => {
                let ctx = CelContext {
                    header: &header,
                    limits,
                    cel_bytes,
                    transparent,
                    fill,
                };
                let outcome = parse_cel(chunk.data, &ctx, &mut layers, &mut materialized)?;
                if outcome.background_hit {
                    ignored.add(Ignored::BackgroundTransparent);
                    lossy = true;
                }
            }
            CHUNK_COLOR_PROFILE => {
                if profile_changes_colors(chunk.data)? {
                    ignored.add(Ignored::ColorProfile);
                    lossy = true;
                }
            }
            CHUNK_OLD_PALETTE | CHUNK_OLD_PALETTE_6BIT | CHUNK_PALETTE => {}
            kind => {
                ignored.add(ignored_kind(kind));
                lossy = true;
            }
        }
    }
    if layers.is_empty() {
        return Err(AsepriteError::Malformed("el fichero no tiene capas"));
    }

    let mut specs = Vec::new();
    specs
        .try_reserve_exact(layers.len())
        .map_err(|_| AsepriteError::TooManyLayers)?;
    for layer in layers {
        let image = match layer.cell {
            None => None,
            Some(data) => Some(
                match &palette {
                    Some(palette) => Image::from_bytes_indexed(
                        header.width,
                        header.height,
                        palette.clone(),
                        data,
                    ),
                    None => Image::from_bytes(header.width, header.height, Mode::Rgba, data),
                }
                .map_err(AsepriteError::Image)?,
            ),
        };
        specs.push(LayerSpec {
            name: layer.name,
            visible: layer.visible,
            opacity: layer.opacity,
            blend: layer.blend,
            locked: layer.locked,
            image,
        });
    }
    let document = Document::from_layers(header.width, header.height, header.mode, palette, specs)
        .map_err(AsepriteError::Document)?;
    Ok(AsepriteRead {
        document,
        report: AsepriteReport {
            frames_in_file: header.frames,
            ignored: ignored.into_vec(),
            lossy,
        },
    })
}

/// Spec: Frames. Los chunks del primer fotograma, sin interpretarlos.
fn frame_chunks<'a>(bytes: &'a [u8], header: &Header) -> Result<Vec<Chunk<'a>>, AsepriteError> {
    let mut cursor = Cursor::new(&bytes[HEADER_SIZE..]);
    let frame_size = usize::try_from(cursor.u32()?).map_err(|_| AsepriteError::Truncated)?;
    if cursor.u16()? != MAGIC_FRAME {
        return Err(AsepriteError::Malformed("magia del fotograma"));
    }
    let old = cursor.u16()?;
    cursor.take(2 + 2)?; // duración (ignorada) y 2 bytes reservados
    let new = cursor.u32()?;
    let count = if new != 0 { new } else { u32::from(old) };
    if frame_size < FRAME_HEADER_SIZE {
        return Err(AsepriteError::Malformed("tamaño del fotograma"));
    }
    let available = bytes.len() - HEADER_SIZE;
    if frame_size > available {
        return Err(AsepriteError::Truncated);
    }
    if header.frames == 1 && frame_size != available {
        return Err(AsepriteError::Malformed("bytes tras el fotograma"));
    }
    if count > MAX_CHUNKS {
        return Err(AsepriteError::TooManyChunks);
    }
    let mut body = Cursor::new(&bytes[HEADER_SIZE + FRAME_HEADER_SIZE..HEADER_SIZE + frame_size]);
    let mut chunks = Vec::new();
    for _ in 0..count {
        let size = usize::try_from(body.u32()?).map_err(|_| AsepriteError::Truncated)?;
        let kind = body.u16()?;
        if size < CHUNK_HEADER_SIZE {
            return Err(AsepriteError::Malformed("tamaño de chunk"));
        }
        let data = body.take(size - CHUNK_HEADER_SIZE)?;
        chunks.push(Chunk { kind, data });
    }
    if body.remaining() != 0 {
        return Err(AsepriteError::Malformed("bytes sobrantes en el fotograma"));
    }
    Ok(chunks)
}

fn is_palette(kind: u16) -> bool {
    matches!(
        kind,
        CHUNK_PALETTE | CHUNK_OLD_PALETTE | CHUNK_OLD_PALETTE_6BIT
    )
}

/// Qué se informa de un chunk que no se interpreta.
fn ignored_kind(kind: u16) -> Ignored {
    match kind {
        CHUNK_TAGS => Ignored::Tags,
        CHUNK_USER_DATA => Ignored::UserData,
        CHUNK_SLICE => Ignored::Slices,
        CHUNK_EXTERNAL_FILES => Ignored::ExternalFiles,
        CHUNK_CEL_EXTRA => Ignored::CelExtra,
        CHUNK_MASK => Ignored::Mask,
        CHUNK_PATH => Ignored::Path,
        CHUNK_TILESET => Ignored::Tileset,
        _ => Ignored::Unknown,
    }
}

/// Spec: Color Profile Chunk. `true` si el perfil cambia lo que se ve (ICC o gamma fija); sRGB y
/// «sin perfil» no cambian nada.
fn profile_changes_colors(data: &[u8]) -> Result<bool, AsepriteError> {
    let mut cursor = Cursor::new(data);
    let kind = cursor.u16()?;
    let flags = cursor.u16()?;
    cursor.u32()?; // gamma fija
    Ok(kind > PROFILE_SRGB || flags & PROFILE_FIXED_GAMMA != 0)
}

/// Spec: Old palette chunk (0x0004), Palette Chunk (0x2019). La paleta de un indexado: gana la
/// nueva si la hay; si no, la antigua; la de 6 bits sola no se admite.
fn build_palette(chunks: &[Chunk<'_>], header: &Header) -> Result<Palette, AsepriteError> {
    let new: Vec<&Chunk<'_>> = chunks.iter().filter(|c| c.kind == CHUNK_PALETTE).collect();
    let old: Vec<&Chunk<'_>> = chunks
        .iter()
        .filter(|c| c.kind == CHUNK_OLD_PALETTE)
        .collect();
    let mut colors: Vec<Rgba> = Vec::new();
    if !new.is_empty() {
        for chunk in new {
            read_new_palette(chunk.data, &mut colors)?;
        }
    } else if !old.is_empty() {
        // Spec: Header, «Number of colors» (0 es 256).
        let len = if header.colors == 0 {
            256
        } else {
            usize::from(header.colors).min(256)
        };
        colors.resize(len, Rgba::opaque(0, 0, 0));
        for chunk in old {
            read_old_palette(chunk.data, &mut colors)?;
        }
    } else if chunks.iter().any(|c| c.kind == CHUNK_OLD_PALETTE_6BIT) {
        return Err(AsepriteError::Unsupported(Unsupported::Palette6Bit));
    } else {
        return Err(AsepriteError::NoPalette);
    }
    let len = colors.len();
    let mut palette =
        Palette::from_colors(colors).map_err(|_| AsepriteError::Malformed("paleta"))?;
    if usize::from(header.transparent) < len {
        palette
            .set_transparent_index(Some(header.transparent))
            .map_err(|_| AsepriteError::Malformed("índice transparente"))?;
    }
    Ok(palette)
}

/// Spec: Palette Chunk (0x2019). Tamaño nuevo, rango a cambiar y una entrada (con alfa y nombre
/// opcional) por índice. Lo que ningún rango cubre queda negro opaco (decisión propia).
fn read_new_palette(data: &[u8], colors: &mut Vec<Rgba>) -> Result<(), AsepriteError> {
    let mut cursor = Cursor::new(data);
    let size = cursor.u32()?;
    let first = cursor.u32()?;
    let last = cursor.u32()?;
    cursor.take(8)?;
    if size == 0 || size > 256 {
        return Err(AsepriteError::Malformed("tamaño de la paleta"));
    }
    if first > last || last >= size {
        return Err(AsepriteError::Malformed("rango de la paleta"));
    }
    colors.resize(size as usize, Rgba::opaque(0, 0, 0));
    for index in first..=last {
        let flags = cursor.u16()?;
        let rgba = cursor.take(4)?;
        if flags & PALETTE_ENTRY_NAME != 0 {
            cursor.string(usize::from(u16::MAX), AsepriteError::Truncated)?;
        }
        colors[index as usize] = Rgba::new(rgba[0], rgba[1], rgba[2], rgba[3]);
    }
    Ok(())
}

/// Spec: Old palette chunk (0x0004). Paquetes de «saltar» y «cantidad» (0 es 256) colores RGB.
fn read_old_palette(data: &[u8], colors: &mut [Rgba]) -> Result<(), AsepriteError> {
    let mut cursor = Cursor::new(data);
    let packets = cursor.u16()?;
    let mut index = 0usize;
    for _ in 0..packets {
        index += usize::from(cursor.u8()?);
        let count = match cursor.u8()? {
            0 => 256,
            n => usize::from(n),
        };
        for _ in 0..count {
            let rgb = cursor.take(3)?;
            let slot = colors.get_mut(index).ok_or(AsepriteError::Malformed(
                "la paleta antigua se sale de su longitud",
            ))?;
            *slot = Rgba::opaque(rgb[0], rgb[1], rgb[2]);
            index += 1;
        }
    }
    Ok(())
}

/// Spec: Layer Chunk (0x2004), NOTE.1 y NOTE.6.
fn parse_layer(data: &[u8], header: &Header, index: usize) -> Result<LayerBuild, AsepriteError> {
    let mut cursor = Cursor::new(data);
    let flags = cursor.u16()?;
    let layer_type = cursor.u16()?;
    let level = cursor.u16()?;
    cursor.take(2 + 2)?; // ancho y alto por defecto (ignorados)
    let blend_code = cursor.u16()?;
    let opacity = cursor.u8()?;
    cursor.take(3)?;
    let name_bytes = cursor.string(Document::MAX_LAYER_NAME_BYTES, AsepriteError::LayerName)?;
    let name = String::from_utf8(name_bytes.to_vec())
        .map_err(|_| AsepriteError::Malformed("nombre de capa que no es UTF-8"))?;
    if flags & LAYER_REFERENCE != 0 {
        return Err(AsepriteError::Unsupported(Unsupported::ReferenceLayer {
            layer: name,
        }));
    }
    match layer_type {
        LAYER_TYPE_NORMAL => {}
        LAYER_TYPE_GROUP => {
            return Err(AsepriteError::Unsupported(Unsupported::Group {
                layer: name,
            }));
        }
        LAYER_TYPE_TILEMAP => {
            return Err(AsepriteError::Unsupported(Unsupported::TilemapLayer {
                layer: name,
            }));
        }
        _ => return Err(AsepriteError::Malformed("tipo de capa")),
    }
    if level != 0 {
        return Err(AsepriteError::Malformed("nivel de capa sin grupo"));
    }
    if header.flags & HEADER_LAYER_UUID != 0 {
        cursor.take(16)?;
    }
    let blend = blend_from_code(blend_code).ok_or_else(|| {
        AsepriteError::Unsupported(Unsupported::BlendMode {
            code: blend_code,
            layer: name.clone(),
        })
    })?;
    // Spec: NOTE.6. La opacidad solo vale si la cabecera lo dice.
    let opacity = if header.flags & HEADER_LAYER_OPACITY != 0 {
        opacity
    } else {
        255
    };
    if header.mode == Mode::Indexed && (opacity != 255 || blend != BlendMode::Normal) {
        return Err(AsepriteError::IndexedLayer { layer: index });
    }
    Ok(LayerBuild {
        name,
        visible: flags & LAYER_VISIBLE != 0,
        locked: flags & LAYER_EDITABLE == 0,
        background: flags & LAYER_BACKGROUND != 0,
        blend,
        opacity,
        has_cel: false,
        cell: None,
    })
}

struct CelContext<'a> {
    header: &'a Header,
    limits: &'a AsepriteLimits,
    cel_bytes: usize,
    transparent: Option<u8>,
    fill: u8,
}

struct CelOutcome {
    background_hit: bool,
}

/// Spec: Cel Chunk (0x2005), NOTE.2 y NOTE.3. Coloca la celda en una celda del tamaño del lienzo,
/// recortando lo que sale; una celda sin contenido visible queda vacía.
fn parse_cel(
    data: &[u8],
    ctx: &CelContext<'_>,
    layers: &mut [LayerBuild],
    materialized: &mut usize,
) -> Result<CelOutcome, AsepriteError> {
    let mut cursor = Cursor::new(data);
    let index = usize::from(cursor.u16()?);
    let x = i64::from(cursor.i16()?);
    let y = i64::from(cursor.i16()?);
    let opacity = cursor.u8()?;
    let kind = cursor.u16()?;
    let z_index = cursor.i16()?;
    cursor.take(5)?;
    let layer = layers
        .get_mut(index)
        .ok_or(AsepriteError::Malformed("índice de capa de una celda"))?;
    if layer.has_cel {
        return Err(AsepriteError::Malformed("celda repetida en una capa"));
    }
    layer.has_cel = true;
    let name = || layer.name.clone();
    if opacity != 255 {
        return Err(AsepriteError::Unsupported(Unsupported::CelOpacity {
            layer: layer.name.clone(),
        }));
    }
    if z_index != 0 {
        return Err(AsepriteError::Unsupported(Unsupported::CelZIndex {
            layer: layer.name.clone(),
        }));
    }
    match kind {
        CEL_RAW | CEL_COMPRESSED => {}
        CEL_LINKED => {
            return Err(AsepriteError::Unsupported(Unsupported::LinkedCel {
                layer: name(),
            }));
        }
        CEL_TILEMAP => {
            return Err(AsepriteError::Unsupported(Unsupported::TilemapCel {
                layer: name(),
            }));
        }
        _ => return Err(AsepriteError::Malformed("tipo de celda")),
    }
    let width = i64::from(cursor.u16()?);
    let height = i64::from(cursor.u16()?);
    if width == 0 || height == 0 {
        return Ok(CelOutcome {
            background_hit: false,
        });
    }
    let bpp = ctx.header.mode.bytes_per_pixel();
    let expected = (width as u64) * (height as u64) * (bpp as u64);
    let payload = cursor.rest();
    let bad = AsepriteError::CelData { layer: index };
    if expected > Image::MAX_BYTES as u64 {
        return Err(bad);
    }
    let expected = expected as usize;
    let decoded;
    let pixels: &[u8] = if kind == CEL_RAW {
        if payload.len() != expected {
            return Err(bad);
        }
        payload
    } else {
        if expected as u64 > MAX_DEFLATE_RATIO * payload.len() as u64 {
            return Err(bad);
        }
        decoded = decompress_to_vec_zlib_with_limit(payload, expected).map_err(|_| bad.clone())?;
        if decoded.len() != expected {
            return Err(bad);
        }
        &decoded
    };

    // Intersección de la celda con el lienzo.
    let (canvas_w, canvas_h) = (i64::from(ctx.header.width), i64::from(ctx.header.height));
    let (x0, y0) = (x.max(0), y.max(0));
    let (x1, y1) = ((x + width).min(canvas_w), (y + height).min(canvas_h));
    if x1 <= x0 || y1 <= y0 {
        return Ok(CelOutcome {
            background_hit: false,
        });
    }
    let row_bytes = (x1 - x0) as usize * bpp;
    let src_at = |row: i64| ((row - y) as usize * width as usize + (x0 - x) as usize) * bpp;
    let is_transparent = |pixel: &[u8]| match ctx.header.mode {
        Mode::Indexed => ctx.transparent == Some(pixel[0]),
        _ => pixel[3] == 0,
    };
    let mut visible = false;
    let mut background_hit = false;
    let watch_background =
        layer.background && ctx.header.mode == Mode::Indexed && ctx.transparent.is_some();
    for row in y0..y1 {
        let at = src_at(row);
        for pixel in pixels[at..at + row_bytes].chunks_exact(bpp) {
            if is_transparent(pixel) {
                background_hit |= watch_background;
            } else {
                visible = true;
            }
        }
    }
    if !visible {
        return Ok(CelOutcome { background_hit });
    }
    // Spec: Cel Chunk. La celda del modelo es del tamaño del lienzo (D04): se reserva solo ahora,
    // con el tope de celdas comprobado antes.
    let needed = materialized.saturating_add(ctx.cel_bytes);
    if needed > ctx.limits.max_cells_bytes {
        return Err(AsepriteError::TooLarge {
            limit: ctx.limits.max_cells_bytes as u64,
        });
    }
    let mut cell: Vec<u8> = Vec::new();
    cell.try_reserve_exact(ctx.cel_bytes)
        .map_err(|_| AsepriteError::TooLarge {
            limit: ctx.limits.max_cells_bytes as u64,
        })?;
    cell.resize(ctx.cel_bytes, ctx.fill);
    let canvas_row = ctx.header.width as usize * bpp;
    for row in y0..y1 {
        let at = src_at(row);
        let to = row as usize * canvas_row + x0 as usize * bpp;
        cell[to..to + row_bytes].copy_from_slice(&pixels[at..at + row_bytes]);
    }
    *materialized = needed;
    layer.cell = Some(cell);
    Ok(CelOutcome { background_hit })
}
