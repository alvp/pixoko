//! The fixed Spanish texts of the fill modes (specs/hito-4-relleno.md, unit 4.5): the row of the
//! options window. Only data: they are edited without touching code. The limits checked by the
//! tests are the same as for the other options rows: everything inside `INTERFACE_CHARSET`.

use crate::tool::FillMode;

pub fn mode_text(mode: FillMode) -> &'static str {
    match mode {
        FillMode::Solid => "Modo: Sólido",
        FillMode::Pattern => "Modo: Patrón",
        FillMode::UntilBackground => "Modo: Hasta el fondo",
    }
}
