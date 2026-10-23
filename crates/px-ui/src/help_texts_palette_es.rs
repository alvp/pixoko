//! Los textos de la unidad C3 (specs/ui-palette-handle.md, condición 12), en un fichero aparte para
//! revisarlos: palabras propias, solo datos, preparados para traducir en el hito 8. Son la ayuda
//! del asa, de la barra de color y de las tres acciones de `Imagen → …` de la paleta, las
//! etiquetas del menú, los avisos, los textos de la barra de estado y los diálogos de tamaño y de
//! la transparente. Todo dentro de `INTERFACE_CHARSET`; la ayuda con nombre <= 20 caracteres y
//! descripción <= 8 líneas de 42 columnas (lo comprueba una prueba).

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

/// Las 5 entradas (van detrás de la del editor de color): el asa y la barra, que no son acciones
/// (se llega con `F1` sobre ellas), y las tres acciones del menú `Imagen`.
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

// Menú `Imagen`.
pub(crate) const MENU_ADD_ENTRY: &str = "Añadir entrada";
pub(crate) const MENU_REMOVE_ENTRY: &str = "Quitar entrada";
pub(crate) const MENU_PALETTE_SIZE: &str = "Tamaño de la paleta…";

// Barra de estado.
pub(crate) const HANDLE_HINT: &str = "asa de la paleta: arrastra para añadir o quitar colores";
pub(crate) const WARN_LOSES_TRANSPARENT: &str = "aviso: se pierde la transparente";
pub(crate) const WARN_PIXELS_OPAQUE: &str = "aviso: sus píxeles quedan opacos";
pub(crate) const WARN_DESTINATION_TRANSPARENT: &str = "aviso: destino transparente";
/// Las formas cortas, para cuando el texto entero no cabe en las 66 columnas.
pub(crate) const WARN_PIXELS_OPAQUE_SHORT: &str = "aviso: píxeles opacos";
pub(crate) const WARN_DESTINATION_TRANSPARENT_SHORT: &str = "aviso: dest. transp.";
/// Aviso de 5 s al cancelarse un recorte pendiente porque el documento cambió.
pub const RESIZE_CANCELLED: &str = "recorte cancelado: el documento cambió";

/// El diálogo del tamaño de la paleta: título, botón, rótulo del campo.
pub const PALETTE_SIZE_TITLE: &str = "Tamaño de la paleta";
pub const PALETTE_SIZE_ACCEPT: &str = "Aceptar";
pub const PALETTE_SIZE_CANCEL: &str = "Cancelar";
pub const PALETTE_SIZE_LABEL: &str = "Colores";

/// El diálogo de la transparente que se pierde con varias capas.
pub const TRANSPARENT_LOSS_TITLE: &str = "Quitar la transparente";
pub const TRANSPARENT_LOSS_TEXT: &str = "la entrada transparente se elimina; con varias capas no podrás añadir capas ni pintar en celdas vacías hasta fijar otra";
