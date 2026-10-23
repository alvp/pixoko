//! The texts of unit C3 (specs/ui-palette-handle.md, condition 12), in a file of their own to be
//! reviewed: own words, data only, ready to translate in milestone 8. They are the help of the
//! handle, of the color bar and of the three `Imagen → …` palette actions, the menu labels,
//! the notices, the texts of the status bar and the dialogs of the size and of the transparent
//! entry. Everything inside `INTERFACE_CHARSET`; the help with name <= 20 characters and
//! description <= 8 lines of 42 columns (a test checks it).

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

/// The 5 entries (after the one of the color editor): the handle and the bar, which are not actions
/// (`F1` over them reaches them), and the three actions of the `Imagen` menu.
pub(crate) const HELP_PALETTE_ES: [HelpEntry; 5] = [
    entry(
        "palette.handle",
        "Asa de la paleta",
        "Arrástrala sobre la rejilla: la celda bajo el puntero es la nueva longitud (de 1 a 256). Estirar añade colores negros; encoger los quita al soltar. Esc cancela. Con píxeles que usan los colores que sobran, elige un destino y confirma. Imagen, Tamaño de la paleta… es la alternativa sin ratón.",
    ),
    entry(
        "color.bar",
        "Barra de color",
        "Muestra el color que se edita y su número. Debajo, escribe un color en hexadecimal (3, 4, 6 u 8 dígitos) y pulsa Intro. Clic derecho en el indicador cambia entre primer plano y fondo.",
    ),
    entry(
        "palette.add_entry",
        "Añadir entrada",
        "Copia el color elegido al final de la paleta y lo elige. Solo en imágenes indexadas. No tiene atajo por defecto: puedes asignarle uno.",
    ),
    entry(
        "palette.remove_entry",
        "Quitar entrada",
        "Quita la entrada elegida; si algún píxel la usa, elige un destino y confirma. Solo en imágenes indexadas. No tiene atajo por defecto: puedes asignarle uno.",
    ),
    entry(
        "palette.size",
        "Tamaño de la paleta",
        "Fija la longitud de la paleta, de 1 a 256 colores, con lo mismo que soltar el asa: estirar añade colores negros y encoger los quita, con destino y confirmación si algún píxel los usa. Solo en imágenes indexadas. No tiene atajo por defecto: puedes asignarle uno.",
    ),
];

// `Imagen` menu.
pub(crate) const MENU_ADD_ENTRY: &str = "Añadir entrada";
pub(crate) const MENU_REMOVE_ENTRY: &str = "Quitar entrada";
pub(crate) const MENU_PALETTE_SIZE: &str = "Tamaño de la paleta…";

// Status bar.
pub(crate) const HANDLE_HINT: &str = "asa de la paleta: arrastra para añadir o quitar colores";
pub(crate) const MAX_ENTRIES_HINT: &str = "máximo 256 entradas en una imagen indexada";
pub(crate) const WARN_LOSES_TRANSPARENT: &str = "aviso: se pierde la transparente";
pub(crate) const WARN_PIXELS_OPAQUE: &str = "aviso: sus píxeles quedan opacos";
pub(crate) const WARN_DESTINATION_TRANSPARENT: &str = "aviso: destino transparente";
/// The short forms, for when the whole text does not fit in 66 columns.
pub(crate) const WARN_PIXELS_OPAQUE_SHORT: &str = "aviso: píxeles opacos";
pub(crate) const WARN_DESTINATION_TRANSPARENT_SHORT: &str = "aviso: dest. transp.";
/// 5 s notice when a pending cut is cancelled because the document changed.
pub const RESIZE_CANCELLED: &str = "recorte cancelado: el documento cambió";

/// The palette size dialog: title, button, label of the field.
pub const PALETTE_SIZE_TITLE: &str = "Tamaño de la paleta";
pub const PALETTE_SIZE_ACCEPT: &str = "Aceptar";
pub const PALETTE_SIZE_CANCEL: &str = "Cancelar";
pub const PALETTE_SIZE_LABEL: &str = "Colores";

/// The dialog of the transparent entry that is lost with several layers.
pub const TRANSPARENT_LOSS_TITLE: &str = "Quitar la transparente";
pub const TRANSPARENT_LOSS_TEXT: &str = "la entrada transparente se elimina; con varias capas no podrás añadir capas ni pintar en celdas vacías hasta fijar otra";
