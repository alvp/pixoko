//! px-core: tipos básicos del documento.

mod palette;
mod rgba;

pub use palette::{Palette, PaletteError};
pub use rgba::{ParseHexError, Rgba};
