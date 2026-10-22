//! px-core: tipos básicos del documento.

mod convert;
mod document;
mod hsl;
mod image;
mod palette;
mod palette_edit;
mod rgba;

pub use convert::{
    Allowed, CelsAnalysis, CelsConversion, Conversion, ConversionReport, ConvertError,
    LUMA_WEIGHTS, analyze_cels, convert_cels, convert_image, luma, target_byte_len,
};
pub use document::{
    BlendMode, Command, Document, DocumentError, ImageId, Layer, LayerId, LayerSpec, MergePlan,
    PixelEdit, Span,
};
pub use hsl::Hsl;
pub use image::{Image, ImageError, Mode, Pixel};
pub use palette::{Palette, PaletteError};
pub use palette_edit::PaletteEditError;
pub use rgba::{ParseHexError, Rgba};
