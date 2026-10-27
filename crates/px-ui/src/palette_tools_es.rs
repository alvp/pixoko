//! The texts of the palette operations of unit 4.6 (specs/hito-4-paleta.md): menu labels, help
//! and notices. Own words, data only, ready to translate in milestone 8. Everything inside
//! `INTERFACE_CHARSET`; help with name <= 20 characters and description <= 8 lines of 42 columns
//! (a test checks it); notices <= 43 columns.

use crate::help_texts::HelpEntry;
use crate::palette_tools::PaletteTool;

pub const MENU_TITLE: &str = "Paleta";

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

/// The 11 entries, in the order of `PaletteTool::ALL`.
pub(crate) const HELP_TOOLS_ES: [HelpEntry; 11] = [
    entry(
        "palette.copy",
        "Copiar entradas",
        "Guarda los colores de las entradas marcadas (Mayús+clic marca un rango; Ctrl+clic suma o quita una entrada) para pegarlos después. No cambia el documento. Solo en imágenes indexadas. No tiene atajo por defecto: puedes asignarle uno.",
    ),
    entry(
        "palette.paste",
        "Pegar entradas",
        "Escribe los colores copiados a partir de la entrada elegida. Si no caben, la paleta crece hasta 256 colores. Los píxeles no cambian de índice. Es un paso de deshacer. Solo en imágenes indexadas. No tiene atajo por defecto: puedes asignarle uno.",
    ),
    entry(
        "palette.swap",
        "Cambiar de sitio",
        "Cambia de sitio las dos entradas marcadas y los píxeles las siguen: la imagen se ve igual. Hacen falta exactamente dos marcadas. Es un paso de deshacer. Solo en imágenes indexadas. No tiene atajo por defecto: puedes asignarle uno.",
    ),
    entry(
        "palette.gradient",
        "Degradado",
        "Rellena las entradas entre la primera y la última de las marcadas con una mezcla gradual de los colores de los extremos. Los píxeles no cambian de índice. Es un paso de deshacer. Solo en imágenes indexadas. No tiene atajo por defecto: puedes asignarle uno.",
    ),
    entry(
        "palette.delete",
        "Borrar entradas",
        "Quita las entradas marcadas. Si algún píxel las usa, eliges adónde pasan con una muestra y confirmas. Es un paso de deshacer. Solo en imágenes indexadas. No tiene atajo por defecto: puedes asignarle uno.",
    ),
    entry(
        "palette.default",
        "Paleta por defecto",
        "Pone los colores de la paleta por defecto en las entradas que hay: 16 colores básicos, un cubo de 216 colores y 24 grises. La longitud no cambia y los píxeles conservan su índice. Solo en imágenes indexadas. No tiene atajo por defecto: puedes asignarle uno.",
    ),
    entry(
        "palette.load",
        "Cargar paleta",
        "Lee un fichero .pal, .gpl o .hex y pone sus colores desde la primera entrada. La paleta crece si hace falta pero nunca encoge, y los píxeles no cambian. Es un paso de deshacer. Solo en imágenes indexadas. No tiene atajo por defecto: puedes asignarle uno.",
    ),
    entry(
        "palette.load-at",
        "Cargar en la activa",
        "Como Cargar paleta, pero los colores empiezan en la entrada elegida y sustituyen a las que hay desde ahí. Solo en imágenes indexadas. No tiene atajo por defecto: puedes asignarle uno.",
    ),
    entry(
        "palette.save",
        "Guardar paleta",
        "Guarda la paleta entera en un fichero .pal, .gpl o .hex, según la extensión. No guarda la transparencia ni cambia el documento. Solo en imágenes indexadas. No tiene atajo por defecto: puedes asignarle uno.",
    ),
    entry(
        "palette.replace-color",
        "Cambiar color",
        "Cambia en todas las capas el color de primer plano por el de fondo, píxel a píxel. Es un paso de deshacer. Funciona en todos los modos. No tiene atajo por defecto: puedes asignarle uno.",
    ),
    entry(
        "palette.swap-colors",
        "Canjear en imagen",
        "Intercambia en todas las capas el color de primer plano y el de fondo: los píxeles de uno pasan al otro y al revés. Es un paso de deshacer. Funciona en todos los modos. No tiene atajo por defecto: puedes asignarle uno.",
    ),
];

/// The label of a tool in the `Paleta` menu.
pub const fn menu_label(tool: PaletteTool) -> &'static str {
    match tool {
        PaletteTool::Copy => "Copiar entradas",
        PaletteTool::Paste => "Pegar entradas",
        PaletteTool::Swap => "Intercambiar entradas",
        PaletteTool::Gradient => "Degradado",
        PaletteTool::Delete => "Borrar entradas",
        PaletteTool::Default => "Paleta por defecto",
        PaletteTool::Load => "Cargar paleta…",
        PaletteTool::LoadAt => "Cargar en la activa…",
        PaletteTool::Save => "Guardar paleta…",
        PaletteTool::ReplaceColor => "Cambiar color en la imagen",
        PaletteTool::SwapColors => "Intercambiar en la imagen",
    }
}

pub const NOTICE_COPIED: &str = "entradas copiadas";
pub const NOTICE_NOTHING_COPIED: &str = "no hay entradas copiadas";
pub const NOTICE_NEED_TWO: &str = "marca exactamente dos entradas";
pub const NOTICE_NEED_RANGE: &str = "marca dos entradas con 2 de distancia";
pub const NOTICE_DELETE_ALL: &str = "no se pueden borrar todas las entradas";
pub const NOTICE_SAME_COLOR: &str = "el primer plano y el fondo son iguales";
pub const NOTICE_NO_PIXELS: &str = "ningún píxel tiene ese color";
pub const NOTICE_NOT_INDEXED: &str = "la imagen no tiene paleta";
pub const NOTICE_SAVED: &str = "paleta guardada";
pub const NOTICE_CUT_PASTE: &str = "sobran colores: se recortó a 256";
pub const NOTICE_IGNORED_PREFIX: &str = "se ignoran colores de más: ";

/// Why a palette file could not be read.
pub fn file_error_text(error: crate::palette_file::PaletteFileError) -> String {
    use crate::palette_file::PaletteFileError;
    match error {
        PaletteFileError::Empty => "el fichero no tiene colores".to_string(),
        PaletteFileError::Header => "cabecera de paleta no válida".to_string(),
        PaletteFileError::Line(line) => format!("línea {line} no válida"),
        PaletteFileError::Count => "faltan colores en el fichero".to_string(),
    }
}

/// Notice after loading: how many colors were written and, if any, how many were left out.
pub fn loaded_notice(written: usize, ignored: usize) -> String {
    if ignored == 0 {
        format!("paleta cargada: {written} colores")
    } else {
        format!("cargados {written}; {ignored} de más ignorados")
    }
}

/// Notice after deleting several entries.
pub fn deleted_notice(count: usize, into: usize) -> String {
    format!("{count} quitadas; píxeles pasan a la {into}")
}
