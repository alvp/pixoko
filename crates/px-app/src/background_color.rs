//! `Ver → Fondo del lienzo → Color…` (specs/hito-2-fondo-color.md): el formulario de un campo
//! hexadecimal que solo acepta colores opacos, el color con el que nace y cómo se lee lo aceptado.
//! Sin egui: los tipos del formulario son de `px-ui` y se prueba sin ventana.

use px_core::{Mode, Rgba};
use px_ui::actions::Locks;
use px_ui::background::CanvasBackground;
use px_ui::dialog::{DialogKind, FieldId, FieldKind, FieldSpec, FormSpec, Modal, Values};

/// El único campo: el hexadecimal.
pub const FIELD_COLOR: FieldId = "color";
/// Por qué se rechaza un color con alfa menor que `ff` (el fondo va *debajo* de la transparencia).
pub const OPAQUE_REASON: &str = "el fondo debe ser opaco (alfa ff)";
/// Con el damero (que no tiene un color) el diálogo nace en gris medio, como el de nueva imagen.
const DEFAULT_COLOR: Rgba = Rgba::opaque(0x80, 0x80, 0x80);

/// El color con el que nace el diálogo: el del fondo liso actual o, con el damero, `#808080`.
pub fn initial_color(current: CanvasBackground) -> Rgba {
    current.solid_color().unwrap_or(DEFAULT_COLOR)
}

/// Se evalúa solo con el campo válido: el alfa no puede rechazarse en el análisis sin cambiar el
/// motor de la 18 (D1).
fn opaque_check(values: &Values) -> Option<String> {
    values
        .color(FIELD_COLOR)
        .filter(|color| color.a != 255)
        .map(|_| OPAQUE_REASON.to_string())
}

/// El formulario, con `initial` en el campo.
pub fn background_color_form(initial: Rgba) -> FormSpec {
    FormSpec {
        title: "Fondo del lienzo",
        accept: "Aplicar",
        cancel: Some("Cancelar"),
        fields: vec![FieldSpec {
            id: FIELD_COLOR,
            label: "Color",
            kind: FieldKind::Hex {
                mode: Mode::Rgba,
                default: initial,
            },
            shown_when: None,
        }],
        check: Some(opaque_check),
    }
}

/// Abre el diálogo con el color del fondo actual; `false` si algún bloqueo lo impide.
pub fn open_background_color(modal: &mut Modal, current: CanvasBackground, locks: Locks) -> bool {
    modal.open(
        DialogKind::BackgroundColor,
        background_color_form(initial_color(current)),
        locks,
    )
}

/// El fondo que sale de lo aceptado: `Custom` con el color opaco, o `None` si falta o no es opaco.
pub fn background_from_values(values: &Values) -> Option<CanvasBackground> {
    values.color(FIELD_COLOR).and_then(CanvasBackground::custom)
}

#[cfg(test)]
mod tests;
