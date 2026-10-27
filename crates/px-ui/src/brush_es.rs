//! The fixed Spanish texts of the brush (specs/hito-4-pincel.md, unit 4.3): the rows of the options
//! window and the four help entries. Only data: they are edited without touching code. The
//! limits checked by the tests are the same as for the other help files: name <= 20 characters,
//! description <= 8 lines of 42 columns and everything inside `INTERFACE_CHARSET`.

use px_raster::{BrushShape, Spacing};

use crate::help_texts::HelpEntry;

/// The name of a shape, as the options window shows it.
pub fn shape_name(shape: BrushShape) -> &'static str {
    match shape {
        BrushShape::Round => "Redondo",
        BrushShape::Square => "Cuadrado",
        BrushShape::Diamond => "Rombo",
        BrushShape::Ring => "Anillo",
        BrushShape::Horizontal => "Horizontal",
        BrushShape::Vertical => "Vertical",
        BrushShape::Slash => "Diagonal sube",
        BrushShape::Backslash => "Diagonal baja",
        BrushShape::Plus => "Cruz",
        BrushShape::Cross => "Aspa",
    }
}

pub fn mode_text(curve_tool: bool, on: bool) -> &'static str {
    match (curve_tool, on) {
        (false, false) => "Modo: Continuo",
        (false, true) => "Modo: Punteado",
        (true, false) => "Modo: Recta",
        (true, true) => "Modo: Curva",
    }
}

pub fn shape_text(shape: BrushShape) -> String {
    format!("Forma: {}", shape_name(shape))
}

pub fn size_text(size: u32) -> String {
    format!("Tamaño: {size}")
}

pub fn spacing_text(spacing: Spacing) -> &'static str {
    match spacing {
        Spacing::Continuous => "Espaciado: Continuo",
        Spacing::EveryN(_) => "Espaciado: Cada N",
        Spacing::TotalN(_) => "Espaciado: N en total",
    }
}

pub fn spacing_n_text(n: u32) -> String {
    format!("N: {n}")
}

const fn entry(
    key: &'static str,
    name: &'static str,
    description: &'static str,
    mouse_left: Option<&'static str>,
    mouse_right: Option<&'static str>,
) -> HelpEntry {
    HelpEntry {
        key,
        name,
        description,
        mouse_left,
        mouse_right,
        options: &[],
    }
}

/// The four entries, after those of the quantizer.
pub(crate) const HELP_BRUSH_ES: [HelpEntry; 4] = [
    entry(
        "tool.dotted",
        "Lápiz punteado",
        "Pone el pincel solo donde el cursor tiene una muestra, sin unirlas: si mueves rápido quedan huecos. Es el lápiz en modo Punteado. Con Mayús el trazo sale recto. Cada trazo es un solo paso de deshacer.",
        Some("en el lienzo, pinta con el primer plano"),
        Some("en el lienzo, pinta con el fondo"),
    ),
    entry(
        "tool.curve",
        "Curva",
        "Es la línea en modo Curva. Arrastra para fijar los dos extremos y suelta; luego arrastra otra vez para curvar el arco y suelta. Esc descarta el segmento pendiente. Con Ctrl quedan las posiciones intermedias.",
        Some("en el lienzo, curva con el primer plano"),
        Some("en el lienzo, curva con el fondo"),
    ),
    entry(
        "brush.bigger",
        "Pincel más grande",
        "Sube un píxel el tamaño del pincel, hasta 100. Vale con el lápiz y con la línea. Ya no acerca el zoom: para eso están la tecla mayor que, la rueda y Inicio.",
        None,
        None,
    ),
    entry(
        "brush.smaller",
        "Pincel más pequeño",
        "Baja un píxel el tamaño del pincel, hasta 1. Vale con el lápiz y con la línea. Ya no aleja el zoom: para eso están la tecla menor que, la rueda y Inicio.",
        None,
        None,
    ),
];
