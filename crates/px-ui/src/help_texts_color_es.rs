//! The help text of the color editor (specs/ui-composition.md, decision 11), in a file of its own
//! to be reviewed: own words, data only, ready to translate in milestone 8. Limits checked by a
//! test: name <= 20 characters, description <= 8 lines of 42 columns and everything inside
//! `INTERFACE_CHARSET`.

use crate::help_texts::HelpEntry;

/// The single entry (it goes after the 3 of `help_texts_merge_es`).
pub(crate) const HELP_COLOR_ES: [HelpEntry; 1] = [HelpEntry {
    key: "color.editor",
    name: "Editor de color",
    description: "Abre o cierra el editor del color elegido. Se abre con p o con el clic derecho en el indicador; se cierra con Esc, con p, con otro clic derecho en ese rectángulo o al cambiar de documento. El clic izquierdo en el indicador es el cuentagotas. Elegir herramienta o muestra no lo cierra.",
    mouse_left: None,
    mouse_right: None,
    options: &[],
}];
