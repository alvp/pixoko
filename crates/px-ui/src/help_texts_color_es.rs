//! The help text of the color editor and of the swap button (specs/ui-composition.md, decision 11;
//! specs/ui-color-block.md), in a file to be reviewed: own words, data only, ready to translate in
//! milestone 8. Limits checked by a test: name <= 20 characters, description <= 8 lines of 42
//! columns and everything inside `INTERFACE_CHARSET`. The review copy is
//! `specs/ui-color-block-textos.md`.

use crate::help_texts::HelpEntry;

/// The two entries (they go after the 3 of `help_texts_merge_es`): the editor, which is also the
/// help of the square, the bars and the sliders, and the action of swapping the two colors.
pub(crate) const HELP_COLOR_ES: [HelpEntry; 2] = [
    HelpEntry {
        key: "color.editor",
        name: "Editor de color",
        description: "Se abre con p o con el clic derecho en una barra de color; se cierra con Esc, con p, con otro clic derecho en esa barra o al cambiar de documento. Arrastra en el cuadrado (saturación a la derecha, brillo arriba) o en las barras de tono y alfa. Con el foco, las flechas mueven 1 y con Mayús 10.",
        mouse_left: None,
        mouse_right: None,
        options: &[],
    },
    HelpEntry {
        key: "color.swap",
        name: "Intercambiar colores",
        description: "Cambia el primer plano por el fondo y el fondo por el primer plano. No es un paso de deshacer. Está en Editar y en el botón junto a las barras de color. No tiene atajo por defecto: puedes asignarle uno.",
        mouse_left: None,
        mouse_right: None,
        options: &[],
    },
];
