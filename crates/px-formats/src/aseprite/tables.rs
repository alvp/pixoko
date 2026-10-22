//! Constantes del formato `.aseprite`. Cada una cita el apartado de la especificación pública
//! (`ase-file-specs`) que la define (specs/hito-3-aseprite.md, «campo → apartado»).

use px_core::BlendMode;

/// Spec: Header. Tamaño de la cabecera del fichero.
pub(super) const HEADER_SIZE: usize = 128;
/// Spec: Header. Magia del fichero (`WORD` en el desplazamiento 4).
pub(super) const MAGIC_FILE: u16 = 0xA5E0;
/// Spec: Header. Bandera 1: la opacidad de capa es válida (NOTE.6).
pub(super) const HEADER_LAYER_OPACITY: u32 = 1;
/// Spec: Header. Bandera 4: las capas llevan UUID.
pub(super) const HEADER_LAYER_UUID: u32 = 4;
/// Spec: Header. Profundidades de color en bits por píxel.
pub(super) const DEPTH_RGBA: u16 = 32;
pub(super) const DEPTH_GRAY: u16 = 16;
pub(super) const DEPTH_INDEXED: u16 = 8;

/// Spec: Frames. Tamaño de la cabecera de fotograma y su magia.
pub(super) const FRAME_HEADER_SIZE: usize = 16;
pub(super) const MAGIC_FRAME: u16 = 0xF1FA;
/// Spec: Frames. Tamaño mínimo de un chunk (incluye `DWORD` de tamaño y `WORD` de tipo).
pub(super) const CHUNK_HEADER_SIZE: usize = 6;

/// Spec: Chunk Types. Tipos de chunk.
pub(super) const CHUNK_OLD_PALETTE: u16 = 0x0004;
pub(super) const CHUNK_OLD_PALETTE_6BIT: u16 = 0x0011;
pub(super) const CHUNK_LAYER: u16 = 0x2004;
pub(super) const CHUNK_CEL: u16 = 0x2005;
pub(super) const CHUNK_CEL_EXTRA: u16 = 0x2006;
pub(super) const CHUNK_COLOR_PROFILE: u16 = 0x2007;
pub(super) const CHUNK_EXTERNAL_FILES: u16 = 0x2008;
pub(super) const CHUNK_MASK: u16 = 0x2016;
pub(super) const CHUNK_PATH: u16 = 0x2017;
pub(super) const CHUNK_TAGS: u16 = 0x2018;
pub(super) const CHUNK_PALETTE: u16 = 0x2019;
pub(super) const CHUNK_USER_DATA: u16 = 0x2020;
pub(super) const CHUNK_SLICE: u16 = 0x2022;
pub(super) const CHUNK_TILESET: u16 = 0x2023;

/// Spec: Layer Chunk. Banderas de capa.
pub(super) const LAYER_VISIBLE: u16 = 1;
pub(super) const LAYER_EDITABLE: u16 = 2;
pub(super) const LAYER_BACKGROUND: u16 = 8;
pub(super) const LAYER_REFERENCE: u16 = 64;
/// Spec: Layer Chunk. Tipos de capa.
pub(super) const LAYER_TYPE_NORMAL: u16 = 0;
pub(super) const LAYER_TYPE_GROUP: u16 = 1;
pub(super) const LAYER_TYPE_TILEMAP: u16 = 2;

/// Spec: Cel Chunk. Tipos de celda.
pub(super) const CEL_RAW: u16 = 0;
pub(super) const CEL_LINKED: u16 = 1;
pub(super) const CEL_COMPRESSED: u16 = 2;
pub(super) const CEL_TILEMAP: u16 = 3;

/// Spec: Color Profile Chunk. Tipos de perfil y bandera de gamma fija.
pub(super) const PROFILE_SRGB: u16 = 1;
pub(super) const PROFILE_FIXED_GAMMA: u16 = 1;

/// Spec: Palette Chunk. Bandera de entrada con nombre.
pub(super) const PALETTE_ENTRY_NAME: u16 = 1;

/// Spec: Layer Chunk, «Blend mode». Tabla de códigos de los ocho modos que Pixoko tiene.
pub(super) fn blend_from_code(code: u16) -> Option<BlendMode> {
    Some(match code {
        0 => BlendMode::Normal,
        1 => BlendMode::Multiply,
        2 => BlendMode::Screen,
        3 => BlendMode::Overlay,
        4 => BlendMode::Darken,
        5 => BlendMode::Lighten,
        10 => BlendMode::Difference,
        16 => BlendMode::Add,
        _ => return None,
    })
}

/// Spec: Layer Chunk, «Blend mode». El código de cada modo; `None` si el modo es posterior a esta
/// tabla.
pub(super) fn blend_to_code(mode: BlendMode) -> Option<u16> {
    Some(match mode {
        BlendMode::Normal => 0,
        BlendMode::Multiply => 1,
        BlendMode::Screen => 2,
        BlendMode::Overlay => 3,
        BlendMode::Darken => 4,
        BlendMode::Lighten => 5,
        BlendMode::Difference => 10,
        BlendMode::Add => 16,
        _ => return None,
    })
}
