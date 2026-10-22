//! Los 3 textos de ayuda de combinar y aplanar capas (specs/hito-3-combinar.md, decisión D11 y
//! condición (d)), en un fichero aparte para revisarlos: palabras propias, solo datos, preparados
//! para traducir en el hito 8. Límites que comprueba una prueba: nombre <= 20 caracteres,
//! descripción <= 8 líneas de 42 columnas y todo dentro de `INTERFACE_CHARSET`.
//!
//! Las tres acciones no tienen tecla por defecto y el texto lo dice. «Exportar PNG aplanado…» es
//! una acción de archivo y no está en esta tabla.

use crate::help_texts::HelpEntry;

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

/// Las 3 entradas, en el orden de la spec (van detrás de las 8 de `help_texts_layers_es`).
pub(crate) const HELP_MERGE_ES: [HelpEntry; 3] = [
    entry(
        "layer.merge-down",
        "Combinar abajo",
        "Une la capa activa con la de debajo, que conserva su nombre. La opacidad y la mezcla de las dos se funden en los píxeles y la capa queda con opacidad 255 y mezcla Normal. Ninguna puede estar oculta ni la de abajo bloqueada. Se puede deshacer. No tiene atajo por defecto: puedes asignarle uno.",
    ),
    entry(
        "layer.merge-visible",
        "Combinar visibles",
        "Une todas las capas visibles en la visible más baja; las ocultas se quedan como están. Lo que se ve no cambia. Se puede deshacer. No tiene atajo por defecto: puedes asignarle uno.",
    ),
    entry(
        "layer.flatten",
        "Aplanar",
        "Deja una sola capa con lo que se ve: une las visibles y quita las ocultas (si tienen píxeles avisa: repite la orden en 5 s). Se puede deshacer. No tiene atajo por defecto: puedes asignarle uno.",
    ),
];
