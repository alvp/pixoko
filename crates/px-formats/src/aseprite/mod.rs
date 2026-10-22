//! Formato `.aseprite`, etapa 1: capas, celdas y paleta de un fotograma
//! (specs/hito-3-aseprite.md). Todo sale de la especificación pública del formato
//! (`ase-file-specs`); cada estructura y función cita su apartado.

mod read;
mod tables;
mod write;

use std::collections::BTreeMap;
use std::error::Error;
use std::fmt;

use px_core::{Document, DocumentError, ImageError};

pub use read::{aseprite_dimensions, read_aseprite};
pub use write::write_aseprite;

/// Formato de un fichero de imagen, por su contenido (no por su extensión).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FileFormat {
    Png,
    Aseprite,
}

/// Firma de 8 bytes de un PNG.
const PNG_SIGNATURE: [u8; 8] = [0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A];

/// Formato de `bytes` por su contenido: firma PNG, o la magia de la cabecera de `.aseprite`
/// (Spec: Header). `None` si no es ninguno.
pub fn detect_format(bytes: &[u8]) -> Option<FileFormat> {
    if bytes.starts_with(&PNG_SIGNATURE) {
        return Some(FileFormat::Png);
    }
    let magic = bytes.get(4..6)?;
    (u16::from_le_bytes([magic[0], magic[1]]) == tables::MAGIC_FILE).then_some(FileFormat::Aseprite)
}

/// Topes de la lectura. Se aplican antes de reservar nada.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct AsepriteLimits {
    /// Píxeles máximos del lienzo.
    pub max_pixels: u64,
    /// Bytes máximos de celdas con píxeles (el tope del núcleo por defecto).
    pub max_cells_bytes: usize,
}

impl Default for AsepriteLimits {
    fn default() -> AsepriteLimits {
        AsepriteLimits {
            max_pixels: 1 << 26,
            max_cells_bytes: Document::DEFAULT_CELLS_LIMIT,
        }
    }
}

/// Qué se ignoró al leer. Todo lo de esta lista salvo `RgbaPalette` y un perfil sRGB cambia lo que
/// el usuario tiene o ve (`AsepriteReport::lossy`).
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
#[non_exhaustive]
pub enum Ignored {
    /// Spec: Tags Chunk.
    Tags,
    /// Spec: User Data Chunk.
    UserData,
    /// Spec: Slice Chunk.
    Slices,
    /// Spec: External Files Chunk.
    ExternalFiles,
    /// Spec: Cel Extra Chunk.
    CelExtra,
    /// Spec: Mask Chunk.
    Mask,
    /// Spec: Path Chunk.
    Path,
    /// Spec: Tileset Chunk.
    Tileset,
    /// Spec: Color Profile Chunk (ICC o gamma fija).
    ColorProfile,
    /// Spec: Palette Chunk en un fichero RGBA (el modelo RGBA no tiene paleta). No es `lossy`.
    RgbaPalette,
    /// Spec: Layer Chunk (bandera de fondo) en un indexado con píxeles del índice transparente.
    BackgroundTransparent,
    /// Un tipo de chunk que esta etapa no conoce.
    Unknown,
}

/// Informe de una lectura.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AsepriteReport {
    /// Fotogramas que declara el fichero (solo se lee el primero).
    pub frames_in_file: u16,
    /// Qué se ignoró y cuántas veces, ordenado por tipo.
    pub ignored: Vec<(Ignored, u32)>,
    /// Se descartó algo que el usuario tiene o ve: `Ctrl+S` no debe pisar el original.
    pub lossy: bool,
}

/// Un documento leído de un `.aseprite` y su informe.
#[derive(Clone, Debug, PartialEq)]
pub struct AsepriteRead {
    pub document: Document,
    pub report: AsepriteReport,
}

/// Lo que esta etapa no admite. Cada variante nombra qué es.
#[derive(Clone, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum Unsupported {
    /// Spec: Layer Chunk, tipo 1.
    Group { layer: String },
    /// Spec: Layer Chunk, tipo 2.
    TilemapLayer { layer: String },
    /// Spec: Layer Chunk, bandera 64.
    ReferenceLayer { layer: String },
    /// Spec: Cel Chunk, tipo 1.
    LinkedCel { layer: String },
    /// Spec: Cel Chunk, tipo 3.
    TilemapCel { layer: String },
    /// Spec: Header, profundidad 16 (grises).
    Grayscale,
    /// Spec: Layer Chunk, «Blend mode».
    BlendMode { code: u16, layer: String },
    /// Spec: Cel Chunk, «Z-Index» (NOTE.5).
    CelZIndex { layer: String },
    /// Spec: Cel Chunk, «Opacity level».
    CelOpacity { layer: String },
    /// Spec: Old palette chunk (0x0011), componentes de 0 a 63.
    Palette6Bit,
}

impl fmt::Display for Unsupported {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Unsupported::Group { layer } => {
                write!(
                    f,
                    "capa de grupo «{layer}»: aún no se abren los grupos (hito 6)"
                )
            }
            Unsupported::TilemapLayer { layer } => write!(
                f,
                "capa de mapa de tiles «{layer}»: aún no se abren los mapas de tiles (hito 6)"
            ),
            Unsupported::ReferenceLayer { layer } => write!(
                f,
                "capa de referencia «{layer}»: aún no se abren las capas de referencia (hito 6)"
            ),
            Unsupported::LinkedCel { layer } => {
                write!(f, "celda enlazada en la capa «{layer}»: aún no se admite")
            }
            Unsupported::TilemapCel { layer } => write!(
                f,
                "celda de mapa de tiles en la capa «{layer}»: aún no se admite"
            ),
            Unsupported::Grayscale => write!(
                f,
                "el modo gris aún no se admite en .aseprite: convierte a RGBA o indexado"
            ),
            Unsupported::BlendMode { code, layer } => write!(
                f,
                "modo de mezcla {code} en la capa «{layer}»: aún no se admite (hito 6)"
            ),
            Unsupported::CelZIndex { layer } => write!(
                f,
                "orden de celda (z-index) distinto de 0 en la capa «{layer}»: aún no se admite"
            ),
            Unsupported::CelOpacity { layer } => write!(
                f,
                "opacidad de celda distinta de 255 en la capa «{layer}»: aún no se admite (hito 6)"
            ),
            Unsupported::Palette6Bit => write!(
                f,
                "paleta antigua de 6 bits por componente: la especificación no dice cómo escalarla"
            ),
        }
    }
}

/// Error al leer o escribir un `.aseprite`.
#[derive(Clone, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum AsepriteError {
    /// La magia de la cabecera no es la de `.aseprite`.
    NotAseprite,
    /// El fichero acaba antes de lo que declara.
    Truncated,
    /// Un campo incoherente (el texto nombra cuál).
    Malformed(&'static str),
    Unsupported(Unsupported),
    /// La capa `layer` de un indexado tiene opacidad o mezcla (el modelo no las admite en indexado).
    IndexedLayer {
        layer: usize,
    },
    /// Indexado sin ningún chunk de paleta.
    NoPalette,
    /// Escribir un indexado sin índice transparente.
    NoTransparentIndex,
    /// Pasa de un tope; `limit` es el tope.
    TooLarge {
        limit: u64,
    },
    TooManyChunks,
    TooManyLayers,
    LayerName,
    /// Los datos de la celda de la capa `layer` no cuadran con su tamaño o están corruptos.
    CelData {
        layer: usize,
    },
    Document(DocumentError),
    Image(ImageError),
}

impl fmt::Display for AsepriteError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            AsepriteError::NotAseprite => write!(f, "no es un fichero .aseprite"),
            AsepriteError::Truncated => write!(f, "el fichero .aseprite está incompleto"),
            AsepriteError::Malformed(what) => write!(f, "fichero .aseprite mal formado: {what}"),
            AsepriteError::Unsupported(what) => {
                write!(f, "{what}; el fichero no se ha modificado")
            }
            AsepriteError::IndexedLayer { layer } => write!(
                f,
                "la capa {layer} de la imagen indexada tiene opacidad o mezcla, que el indexado no admite; el fichero no se ha modificado"
            ),
            AsepriteError::NoPalette => write!(f, "la imagen indexada no trae paleta"),
            AsepriteError::NoTransparentIndex => write!(
                f,
                "esta imagen indexada no tiene entrada transparente: fíjala con Fijar transparente o convierte a RGBA"
            ),
            AsepriteError::TooLarge { limit } => {
                write!(f, "supera el máximo permitido ({limit})")
            }
            AsepriteError::TooManyChunks => write!(f, "demasiados chunks en el fotograma"),
            AsepriteError::TooManyLayers => write!(f, "demasiadas capas"),
            AsepriteError::LayerName => write!(f, "el nombre de una capa es demasiado largo"),
            AsepriteError::CelData { layer } => {
                write!(f, "los datos de la celda de la capa {layer} no son válidos")
            }
            AsepriteError::Document(source) => write!(f, "{source}"),
            AsepriteError::Image(source) => write!(f, "{source}"),
        }
    }
}

impl Error for AsepriteError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            AsepriteError::Document(source) => Some(source),
            AsepriteError::Image(source) => Some(source),
            _ => None,
        }
    }
}

/// Cuenta de lo ignorado, por tipo.
#[derive(Default)]
pub(super) struct IgnoredCounts(BTreeMap<Ignored, u32>);

impl IgnoredCounts {
    pub(super) fn add(&mut self, kind: Ignored) {
        let count = self.0.entry(kind).or_insert(0);
        *count = count.saturating_add(1);
    }

    pub(super) fn into_vec(self) -> Vec<(Ignored, u32)> {
        self.0.into_iter().collect()
    }
}
