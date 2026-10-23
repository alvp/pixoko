//! The texts of the color block (specs/ui-color-block.md), in a file of their own to be reviewed:
//! own words, data only, ready to translate in milestone 8. The help texts `color.bar`,
//! `color.editor` and `color.swap` are in `help_texts_color_es.rs`; the review copy of all of them
//! is `specs/ui-color-block-textos.md`.

/// Tooltip of the bars: the first line, by role, and the second one, the same for both.
pub const FOREGROUND: &str = "Primer plano";
pub const BACKGROUND: &str = "Fondo";
pub const BAR_TIP: &str = "clic: cuentagotas; clic derecho: editar este color";

/// What a bar says when what it paints is entirely transparent.
pub const TRANSPARENT: &str = "transparente";

/// The three options of the selector of the sliders.
pub const MODE_RGB: &str = "RGB";
pub const MODE_HSL: &str = "HSL";
pub const MODE_HSV: &str = "HSV";

/// Tooltip of the button that swaps the foreground and the background.
pub const SWAP_TIP: &str = "Intercambiar primer plano y fondo";

/// Entry `Editar → Intercambiar colores`.
pub const MENU_SWAP: &str = "Intercambiar colores";
