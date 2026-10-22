//! px-formats: lectura y escritura de formatos de imagen.

mod aseprite;
mod aux_chunks;
mod png;

pub use aseprite::{
    AsepriteError, AsepriteLimits, AsepriteRead, AsepriteReport, FileFormat, Ignored, Unsupported,
    aseprite_dimensions, detect_format, read_aseprite, write_aseprite,
};
pub use aux_chunks::{AuxChunk, AuxReport};
pub use png::{Conversion, PngError, PngRead, png_dimensions, read_png, write_png};
