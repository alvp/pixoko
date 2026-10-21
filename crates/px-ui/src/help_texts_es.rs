//! Los textos de la ayuda y de las ventanas de atajos, en español y con palabras propias
//! (specs/interfaz-ayuda.md, decisión 1 y visto bueno 4 y 5). **Solo datos:** se editan sin tocar
//! código. Límites que comprueban las pruebas: nombre ≤ 20 caracteres, descripción ≤ 8 líneas de 42
//! columnas, todo dentro de `INTERFACE_CHARSET`.
//!
//! PENDIENTE DE REVISIÓN DEL USUARIO (visto bueno 5): los 22 textos de `HELP_ES` no están cerrados
//! hasta que los confirme.

use crate::help_texts::{HelpEntry, UiStrings};

const fn entry(
    key: &'static str,
    name: &'static str,
    description: &'static str,
    mouse_left: Option<&'static str>,
    mouse_right: Option<&'static str>,
) -> HelpEntry {
    HelpEntry {
        key,
        name,
        description,
        mouse_left,
        mouse_right,
        options: &[],
    }
}

/// Las 22 entradas, en el orden de la spec.
pub(crate) const HELP_ES: [HelpEntry; 22] = [
    entry(
        "tool.pencil",
        "Lápiz",
        "Dibuja a mano alzada, un píxel por cada punto que recorre el cursor. Cada trazo es un solo paso de deshacer.",
        Some("en el lienzo, pinta con el primer plano"),
        Some("en el lienzo, pinta con el fondo"),
    ),
    entry(
        "tool.line",
        "Línea",
        "Traza una recta desde donde pulsas hasta donde sueltas, con vista previa mientras arrastras. Esc la cancela.",
        Some("en el lienzo, recta con el primer plano"),
        Some("en el lienzo, recta con el fondo"),
    ),
    entry(
        "tool.fill",
        "Relleno",
        "Pinta la zona de píxeles iguales que rodea al que pulsas. Un clic es un solo paso de deshacer.",
        Some("rellena con el primer plano"),
        Some("rellena con el fondo"),
    ),
    entry(
        "tool.dropper",
        "Cuentagotas",
        "Copia el color del píxel pulsado. En una imagen indexada elige su entrada de la paleta.",
        Some("toma el color como primer plano"),
        Some("toma el color como fondo"),
    ),
    entry(
        "tool.eraser",
        "Borrador",
        "Borra al arrastrar y deja los píxeles transparentes. Si la imagen no admite transparencia, avisa.",
        Some("en el lienzo, borra"),
        Some("en el lienzo, borra igual"),
    ),
    entry(
        "view.zoom-in",
        "Acercar",
        "Sube un paso el zoom, hasta ×64.",
        Some("en el botón, acerca"),
        Some("en el botón, aleja"),
    ),
    entry(
        "view.zoom-out",
        "Alejar",
        "Baja un paso el zoom, hasta 1/64.",
        Some("en el botón, aleja"),
        Some("en el botón, acerca"),
    ),
    entry(
        "view.zoom-fit",
        "Ajustar",
        "Elige el zoom con el que la imagen entera cabe en el lienzo y la centra.",
        None,
        None,
    ),
    entry(
        "view.zoom-actual",
        "1:1",
        "Pone el zoom en ×1: un píxel de la imagen ocupa un píxel de pantalla.",
        None,
        None,
    ),
    entry(
        "view.center",
        "Centrar en cursor",
        "Lleva al centro del lienzo el punto de la imagen que está bajo el cursor, sin cambiar el zoom. Con el cursor fuera del lienzo no hace nada.",
        None,
        None,
    ),
    entry(
        "edit.undo",
        "Deshacer",
        "Anula el último paso de la historia: un trazo, un relleno, un cambio de la paleta…",
        None,
        None,
    ),
    entry(
        "edit.redo",
        "Rehacer",
        "Vuelve a aplicar el último paso que deshiciste.",
        None,
        None,
    ),
    entry(
        "edit.clear-page",
        "Borrar página",
        "Pinta toda la imagen con el color de fondo, en un solo paso que se puede deshacer.",
        None,
        None,
    ),
    entry(
        "color.next",
        "Color siguiente",
        "Elige el color que sigue en la paleta (en una imagen indexada, la entrada). Tras el último vuelve al primero.",
        None,
        None,
    ),
    entry(
        "color.prev",
        "Color anterior",
        "Elige el color anterior de la paleta. Antes del primero salta al último.",
        None,
        None,
    ),
    entry(
        "color.bg-next",
        "Fondo siguiente",
        "Igual que Color siguiente, pero para el color de fondo.",
        None,
        None,
    ),
    entry(
        "color.bg-prev",
        "Fondo anterior",
        "Igual que Color anterior, pero para el color de fondo.",
        None,
        None,
    ),
    entry(
        "file.new",
        "Nueva imagen",
        "Abre el diálogo para crear una imagen vacía: tamaño, modo y fondo.",
        None,
        None,
    ),
    entry(
        "file.open",
        "Abrir",
        "Abre un PNG del disco. También puedes soltar el fichero sobre la ventana.",
        None,
        None,
    ),
    entry("file.save", "Guardar", "Guarda la imagen como PNG.", None, None),
    entry(
        "file.save-as",
        "Guardar como",
        "Guarda la imagen como PNG con otro nombre o en otra carpeta.",
        None,
        None,
    ),
    entry(
        "file.quit",
        "Salir",
        "Cierra Pixoko. Con cambios sin guardar avisa: repite la orden en 5 s para salir.",
        None,
        None,
    ),
];

/// El texto fijo de la interfaz de ayuda y de atajos.
pub(crate) const UI_ES: UiStrings = UiStrings {
    primary: "Principal",
    secondary: "Secundario",
    no_shortcut: "sin atajo",
    close: "Cerrar",
    accept: "Aceptar",
    cancel: "Cancelar",
    remove: "Quitar",
    reassign: "Reasignar",
    keep: "Dejarlo",
    conflict_label: "Conflicto",
    reset_label: "Esta acción",
    reset: "Restablecer",
    reset_all: "Restablecer todo",
    reset_all_text: "¿Volver a los atajos de fábrica? Se pierden los que has cambiado.",
    editor_title: "Editar atajo",
    list_title: "Atajos de teclado",
    reset_title: "Restablecer atajos",
    action_prefix: "Acción: ",
    fixed_prefix: "Fijos: ",
    also_prefix: "También: ",
    fixed_single_prefix: "Atajo (fijo): ",
    left_click: "Clic izquierdo: ",
    right_click: "Clic derecho: ",
    options_prefix: "Opciones: ",
    capture_prompt: "pulsa el nuevo atajo (Esc cancela)",
    armed: "pulsa una tecla…",
    same_action: "ya es otro atajo de esta acción",
    resolve_conflict: "resuelve el conflicto",
    conflict_mark: "conflicto",
    already_belongs: "ya es de",
    not_a_key: "esa tecla no vale",
    no_command: "Ctrl y Cmd no se pueden asignar",
    no_alt_letter: "Alt+letra no vale",
    no_space: "Espacio es del lienzo",
    f1_reserved: "F1 es de la ayuda",
};
