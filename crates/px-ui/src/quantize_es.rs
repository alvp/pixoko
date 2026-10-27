//! The texts of the color quantizer (specs/hito-4-cuantizador.md): menu entry, dialog, notices
//! and the help entry. Own words, data only, ready to translate in milestone 8. Limits checked by
//! a test: every text <= 42 characters, the help name <= 20 and its description <= 8 lines of 42
//! columns, everything inside `INTERFACE_CHARSET`. The review copy is
//! `specs/hito-4-cuantizador-textos.md`.

use crate::help_texts::HelpEntry;

pub const MENU_QUANTIZE: &str = "Reducir colores…";

pub const QUANTIZE_TITLE: &str = "Reducir colores";
pub const QUANTIZE_ACCEPT: &str = "Aceptar";
pub const QUANTIZE_CANCEL: &str = "Cancelar";
/// Labels of a numeric field fit 12 characters.
pub const QUANTIZE_COLORS: &str = "Colores";
pub const QUANTIZE_PALETTE: &str = "Paleta";
/// The palettes offered when the session has no saved one.
pub const QUANTIZE_PALETTES_ONE: [&str; 1] = ["Calculada de la imagen"];
/// The palettes offered when the session has a saved one.
pub const QUANTIZE_PALETTES_TWO: [&str; 2] = ["Calculada de la imagen", "Guardada de la sesión"];
pub const QUANTIZE_DITHER: &str = "Tramado";
/// In the order of `px_raster::Dither`.
pub const QUANTIZE_DITHERS: [&str; 3] = ["Ninguno", "Difusión de error", "Ordenado"];
pub const QUANTIZE_THRESHOLD: &str = "Alfa mínimo";
/// Notice when the document cannot be reduced because of its mode.
pub const QUANTIZE_ONLY_COLOR: &str = "Reducir colores: solo RGBA o gris con alfa";
/// The notice of a reduction that takes long (the status bar, two frames before it starts).
pub const QUANTIZE_WORKING: &str = "Reduciendo colores…";
/// The label of the button of `Convertir modo…` that opens this dialog.
pub const QUANTIZE_BUTTON: &str = "Reducir colores…";
/// The label of the group of the button in `Convertir modo…`.
pub const QUANTIZE_BUTTON_LABEL: &str = "Colores";

pub(crate) const HELP_QUANTIZE_ES: [HelpEntry; 1] = [HelpEntry {
    key: "image.quantize",
    name: "Reducir colores",
    description: "Pasa toda la imagen (RGBA o gris con alfa, con todas sus capas) a indexado con los colores que elijas. La paleta se calcula o se toma la guardada de la sesión; el tramado reparte el error y el alfa mínimo decide qué píxeles quedan transparentes. Un paso de deshacer. Sin atajo por defecto: puedes asignarle uno.",
    mouse_left: None,
    mouse_right: None,
    options: &[],
}];
