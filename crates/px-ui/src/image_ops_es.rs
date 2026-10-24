//! The texts of the image operations (specs/hito-4-imagen.md): menu entries, the dialog of the
//! canvas size and the six help entries. Own words, data only, ready to translate in milestone 8.
//! The pixel font has no degree sign, so the turns are named by direction. Limits checked by a
//! test: name <= 20 characters, description <= 8 lines of 42 columns, everything inside
//! `INTERFACE_CHARSET`. The review copy is `specs/hito-4-imagen-textos.md`.

use crate::help_texts::HelpEntry;

pub const MENU_FLIP_H: &str = "Voltear horizontal";
pub const MENU_FLIP_V: &str = "Voltear vertical";
pub const MENU_TURN_RIGHT: &str = "Girar a la derecha";
pub const MENU_TURN_LEFT: &str = "Girar a la izquierda";
pub const MENU_TURN_HALF: &str = "Girar media vuelta";
pub const MENU_CANVAS_SIZE: &str = "Tamaño del lienzo…";

pub const CANVAS_TITLE: &str = "Tamaño del lienzo";
pub const CANVAS_ACCEPT: &str = "Aceptar";
pub const CANVAS_CANCEL: &str = "Cancelar";
pub const CANVAS_WIDTH: &str = "Ancho";
pub const CANVAS_HEIGHT: &str = "Alto";
pub const CANVAS_ANCHOR: &str = "Ancla";
/// The nine anchors, in the order of `px_raster::Anchor` (row by row, from the top left).
pub const CANVAS_ANCHORS: [&str; 9] = [
    "Arriba a la izquierda",
    "Arriba",
    "Arriba a la derecha",
    "Izquierda",
    "Centro",
    "Derecha",
    "Abajo a la izquierda",
    "Abajo",
    "Abajo a la derecha",
];
/// Notice when the operation does not change anything.
pub const IMAGE_OP_NOTHING: &str = "Sin cambios: la imagen queda igual";

pub(crate) const HELP_IMAGE_ES: [HelpEntry; 6] = [
    entry(
        "image.flip-h",
        "Voltear horizontal",
        "Da la vuelta a toda la imagen de izquierda a derecha, con todas las capas. Es un paso de deshacer. No tiene atajo por defecto: puedes asignarle uno.",
    ),
    entry(
        "image.flip-v",
        "Voltear vertical",
        "Da la vuelta a toda la imagen de arriba abajo, con todas las capas. Es un paso de deshacer. No tiene atajo por defecto: puedes asignarle uno.",
    ),
    entry(
        "image.turn-right",
        "Girar a la derecha",
        "Gira toda la imagen un cuarto de vuelta a la derecha, con todas las capas; el ancho y el alto se intercambian. Es un paso de deshacer. No tiene atajo por defecto: puedes asignarle uno.",
    ),
    entry(
        "image.turn-left",
        "Girar a la izquierda",
        "Gira toda la imagen un cuarto de vuelta a la izquierda, con todas las capas; el ancho y el alto se intercambian. Es un paso de deshacer. No tiene atajo por defecto: puedes asignarle uno.",
    ),
    entry(
        "image.turn-half",
        "Girar media vuelta",
        "Gira toda la imagen media vuelta, con todas las capas. Es un paso de deshacer. No tiene atajo por defecto: puedes asignarle uno.",
    ),
    entry(
        "image.canvas-size",
        "Tamaño del lienzo",
        "Cambia el ancho y el alto sin escalar la imagen. El ancla dice dónde queda lo que ya hay; lo que sobra se pierde y lo nuevo se rellena (transparente si el modo lo permite; si no, con el color de fondo). Un paso de deshacer.",
    ),
];

const fn entry(key: &'static str, name: &'static str, description: &'static str) -> HelpEntry {
    HelpEntry {
        key,
        name,
        description,
        mouse_left: None,
        mouse_right: None,
        options: &[],
    }
}
