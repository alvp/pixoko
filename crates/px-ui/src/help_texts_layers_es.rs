//! Los 8 textos de ayuda de las acciones de capa y del panel (specs/hito-3-capas-interfaz.md,
//! decisión 8 y condiciones 3 y 4), en un fichero aparte para revisarlos: palabras propias, solo
//! datos, preparados para traducir en el hito 8. Límites que comprueba una prueba: nombre <= 20
//! caracteres, descripción <= 8 líneas de 42 columnas y todo dentro de `INTERFACE_CHARSET`.
//!
//! Las cuatro teclas por defecto dicen cómo teclearlas en un teclado español; las otras cuatro
//! acciones no tienen tecla y lo dicen.

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

/// Las 8 entradas, en el orden de la spec (van detrás de las 22 de `help_texts_es`).
pub(crate) const HELP_LAYERS_ES: [HelpEntry; 8] = [
    entry(
        "layer.panel",
        "Panel de capas",
        "Muestra u oculta el panel flotante de capas. Arrastra su barra de título para moverlo: solo tapa lo que hay bajo su rectángulo y se cierra también con la x. Atajo por defecto: ' (el apóstrofo, una tecla normal en un teclado español, sin Mayús).",
    ),
    entry(
        "layer.new",
        "Nueva capa",
        "Añade una capa vacía justo encima de la activa y la elige. Es un paso de deshacer. En una imagen en gris avisa y ofrece pasar a gris con alfa: repite la orden en 5 s. Atajo por defecto: ; (en un teclado español, Mayús+coma).",
    ),
    entry(
        "layer.duplicate",
        "Duplicar capa",
        "Copia la capa activa justo encima, con sus píxeles, su opacidad, su mezcla y su bloqueo, y elige la copia. Es un paso de deshacer. No tiene atajo por defecto: puedes asignarle uno.",
    ),
    entry(
        "layer.remove",
        "Quitar capa",
        "Quita la capa activa con sus píxeles. Se puede deshacer; la última capa no se puede quitar. No tiene atajo por defecto: puedes asignarle uno.",
    ),
    entry(
        "layer.up",
        "Subir capa",
        "Sube un puesto la capa activa en la pila. No cambia cuál es la activa. Atajo por defecto: ) (en un teclado español, Mayús+9).",
    ),
    entry(
        "layer.down",
        "Bajar capa",
        "Baja un puesto la capa activa en la pila. No cambia cuál es la activa. Atajo por defecto: ( (en un teclado español, Mayús+8).",
    ),
    entry(
        "layer.rename",
        "Renombrar capa",
        "Abre el panel y lleva el cursor al nombre de la capa activa. Intro lo cambia y Esc lo deja como estaba. También con doble clic en la fila. No tiene atajo por defecto.",
    ),
    entry(
        "layer.properties",
        "Propiedades capa",
        "Abre el panel y lleva el foco a las opciones de la capa activa: nombre, opacidad y mezcla. El ojo de cada fila oculta la capa. El candado protege sus píxeles de las herramientas de dibujo; los cambios de paleta y de modo afectan a todas las capas, también a las bloqueadas.",
    ),
];
