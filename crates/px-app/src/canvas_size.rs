//! `Imagen → Tamaño del lienzo…` (specs/hito-4-imagen.md): the form and how what was accepted is
//! read. No egui: the form types are `px-ui`'s and it is tested without a window.

use px_raster::{Anchor, ImageOp};
use px_ui::actions::Locks;
use px_ui::canvas::Canvas;
use px_ui::dialog::{DialogKind, FieldId, FieldKind, FieldSpec, FormSpec, Modal, Values};
use px_ui::image_ops_es::{
    CANVAS_ACCEPT, CANVAS_ANCHOR, CANVAS_ANCHORS, CANVAS_CANCEL, CANVAS_HEIGHT, CANVAS_TITLE,
    CANVAS_WIDTH,
};

pub const FIELD_WIDTH: FieldId = "ancho";
pub const FIELD_HEIGHT: FieldId = "alto";
pub const FIELD_ANCHOR: FieldId = "ancla";

/// The anchors in the order of the options of the form (row by row from the top left).
const ANCHORS: [Anchor; 9] = [
    Anchor::TopLeft,
    Anchor::Top,
    Anchor::TopRight,
    Anchor::Left,
    Anchor::Center,
    Anchor::Right,
    Anchor::BottomLeft,
    Anchor::Bottom,
    Anchor::BottomRight,
];

const CENTER: usize = 4;

fn max_side() -> u32 {
    u32::try_from(Canvas::MAX_SIDE).unwrap_or(16384)
}

/// The form, with the current size in the fields and the center anchor.
pub fn canvas_size_form(width: u32, height: u32) -> FormSpec {
    let number = |id, label, current: u32| FieldSpec {
        id,
        label,
        kind: FieldKind::Number {
            min: 1,
            max: max_side(),
            default: current.clamp(1, max_side()),
        },
        shown_when: None,
    };
    FormSpec {
        title: CANVAS_TITLE,
        accept: CANVAS_ACCEPT,
        cancel: Some(CANVAS_CANCEL),
        fields: vec![
            number(FIELD_WIDTH, CANVAS_WIDTH, width),
            number(FIELD_HEIGHT, CANVAS_HEIGHT, height),
            FieldSpec {
                id: FIELD_ANCHOR,
                label: CANVAS_ANCHOR,
                kind: FieldKind::Choice {
                    options: &CANVAS_ANCHORS,
                    default: CENTER,
                },
                shown_when: None,
            },
        ],
        check: None,
    }
}

/// Opens the dialog with the current size; `false` if some lock prevents it.
pub fn open_canvas_size(modal: &mut Modal, width: u32, height: u32, locks: Locks) -> bool {
    modal.open(
        DialogKind::CanvasSize,
        canvas_size_form(width, height),
        locks,
    )
}

/// The operation that comes out of what was accepted.
pub fn op_from_values(values: &Values) -> Option<ImageOp> {
    let width = values.number(FIELD_WIDTH)?;
    let height = values.number(FIELD_HEIGHT)?;
    let anchor = *ANCHORS.get(values.choice(FIELD_ANCHOR)?)?;
    ((1..=max_side()).contains(&width) && (1..=max_side()).contains(&height)).then_some(
        ImageOp::Canvas {
            width,
            height,
            anchor,
        },
    )
}
