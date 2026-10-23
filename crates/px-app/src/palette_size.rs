//! `Imagen → Tamaño de la paleta…` and the dialog of the transparent entry that is lost
//! (specs/ui-palette-handle.md, conditions 6 and 7): the forms and how what was accepted is read.
//! No egui: the form types are `px-ui`'s and it is tested without a window.

use px_ui::actions::Locks;
use px_ui::dialog::{DialogKind, FieldId, FieldKind, FieldSpec, FormSpec, Modal, Rows, Values};
use px_ui::help_texts_palette_es::{
    PALETTE_SIZE_ACCEPT, PALETTE_SIZE_CANCEL, PALETTE_SIZE_LABEL, PALETTE_SIZE_TITLE,
    TRANSPARENT_LOSS_TEXT, TRANSPARENT_LOSS_TITLE,
};

/// The numeric field of the size: from 1 to 256 colors.
pub const FIELD_LENGTH: FieldId = "colores";
/// The text zone of the dialog of the transparent entry.
pub const FIELD_LOSS_TEXT: FieldId = "texto";

/// The size form, with the current length in the field.
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

/// Opens the dialog with the current length; `false` if some lock prevents it.
pub fn open_palette_size(modal: &mut Modal, current: usize, locks: Locks) -> bool {
    modal.open(DialogKind::PaletteSize, palette_size_form(current), locks)
}

/// The length that comes out of what was accepted.
pub fn length_from_values(values: &Values) -> Option<u16> {
    values
        .number(FIELD_LENGTH)
        .and_then(|length| u16::try_from(length).ok())
        .filter(|length| (1..=256).contains(length))
}

/// The form of the transparent entry that is lost with several layers: the text and `Aceptar`
/// and `Cancelar`.
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

/// Opens the dialog of the transparent entry; `false` if some lock prevents it.
pub fn open_transparent_loss(modal: &mut Modal, locks: Locks) -> bool {
    modal.open(DialogKind::TransparentLoss, transparent_loss_form(), locks)
}
