//! `Imagen → Tamaño de la paleta…` y el diálogo de la transparente que se pierde
//! (specs/ui-palette-handle.md, condiciones 6 y 7): los formularios y cómo se lee lo aceptado. Sin
//! egui: los tipos del formulario son de `px-ui` y se prueba sin ventana.

use px_ui::actions::Locks;
use px_ui::dialog::{DialogKind, FieldId, FieldKind, FieldSpec, FormSpec, Modal, Rows, Values};
use px_ui::help_texts_palette_es::{
    PALETTE_SIZE_ACCEPT, PALETTE_SIZE_CANCEL, PALETTE_SIZE_LABEL, PALETTE_SIZE_TITLE,
    TRANSPARENT_LOSS_TEXT, TRANSPARENT_LOSS_TITLE,
};

/// El campo numérico del tamaño: de 1 a 256 colores.
pub const FIELD_LENGTH: FieldId = "colores";
/// La zona de texto del diálogo de la transparente.
pub const FIELD_LOSS_TEXT: FieldId = "texto";

/// El formulario del tamaño, con la longitud actual en el campo.
pub fn palette_size_form(current: usize) -> FormSpec {
    let current = u32::try_from(current).unwrap_or(256).clamp(1, 256);
    FormSpec {
        title: PALETTE_SIZE_TITLE,
        accept: PALETTE_SIZE_ACCEPT,
        cancel: Some(PALETTE_SIZE_CANCEL),
        fields: vec![FieldSpec {
            id: FIELD_LENGTH,
            label: PALETTE_SIZE_LABEL,
            kind: FieldKind::Number {
                min: 1,
                max: 256,
                default: current,
            },
            shown_when: None,
        }],
        check: None,
    }
}

/// Abre el diálogo con la longitud actual; `false` si algún bloqueo lo impide.
pub fn open_palette_size(modal: &mut Modal, current: usize, locks: Locks) -> bool {
    modal.open(DialogKind::PaletteSize, palette_size_form(current), locks)
}

/// La longitud que sale de lo aceptado.
pub fn length_from_values(values: &Values) -> Option<u16> {
    values
        .number(FIELD_LENGTH)
        .and_then(|length| u16::try_from(length).ok())
        .filter(|length| (1..=256).contains(length))
}

/// El formulario de la transparente que se pierde con varias capas: el texto y `Aceptar` y
/// `Cancelar`.
pub fn transparent_loss_form() -> FormSpec {
    FormSpec {
        title: TRANSPARENT_LOSS_TITLE,
        accept: PALETTE_SIZE_ACCEPT,
        cancel: Some(PALETTE_SIZE_CANCEL),
        fields: vec![FieldSpec {
            id: FIELD_LOSS_TEXT,
            label: "",
            kind: FieldKind::ReadOnly {
                text: TRANSPARENT_LOSS_TEXT.to_string(),
                rows: Rows::Fixed(4),
            },
            shown_when: None,
        }],
        check: None,
    }
}

/// Abre el diálogo de la transparente; `false` si algún bloqueo lo impide.
pub fn open_transparent_loss(modal: &mut Modal, locks: Locks) -> bool {
    modal.open(DialogKind::TransparentLoss, transparent_loss_form(), locks)
}
