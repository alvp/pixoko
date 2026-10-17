//! Color activo del lápiz: nueve ranuras (teclas 1 a 9) que dan un `Pixel` del modo de la
//! imagen (specs/hito-2-lapiz-deshacer.md, sección 4), y la elección de color del panel de
//! paletas, que en imágenes indexadas llega a las 256 entradas (specs/hito-2-panel-paletas.md).

use px_core::{Image, Mode, Pixel, Rgba};

use crate::texture::pixel_rgba;

/// Número de ranuras: `1..=SLOTS`.
pub const SLOTS: u8 = 9;

/// Colores de las ranuras 1 a 8 en RGBA. La 9 es transparente.
const COLORS: [Rgba; 8] = [
    Rgba::opaque(0, 0, 0),
    Rgba::opaque(255, 255, 255),
    Rgba::opaque(255, 0, 0),
    Rgba::opaque(0, 255, 0),
    Rgba::opaque(0, 0, 255),
    Rgba::opaque(255, 255, 0),
    Rgba::opaque(255, 0, 255),
    Rgba::opaque(0, 255, 255),
];

/// `Pixel` de la ranura `slot` para esa imagen: siempre del modo de la imagen y, si es
/// indexada, con el índice dentro de su paleta. `None` si `slot` no está en `1..=SLOTS` o,
/// en indexado, si la paleta tiene menos entradas que `slot`.
pub fn pixel_for_slot(image: &Image, slot: u8) -> Option<Pixel> {
    if !(1..=SLOTS).contains(&slot) {
        return None;
    }
    let n = usize::from(slot - 1);
    let gray = if slot == SLOTS { 255 } else { (slot - 1) * 32 };
    match image.mode() {
        Mode::Rgba => Some(Pixel::Rgba(
            COLORS.get(n).copied().unwrap_or(Rgba::TRANSPARENT),
        )),
        Mode::Gray => Some(Pixel::Gray(gray)),
        Mode::GrayAlpha if slot == SLOTS => Some(Pixel::GrayAlpha { gray: 0, alpha: 0 }),
        Mode::GrayAlpha => Some(Pixel::GrayAlpha { gray, alpha: 255 }),
        Mode::Indexed => {
            let palette = image.palette()?;
            (n < palette.len()).then_some(Pixel::Indexed(slot - 1))
        }
    }
}

/// Texto del color de la ranura para el título: `color 3 #ff0000`, `color 2 índice 1`,
/// `color 4 gris 96`, `color 9 transparente`. `None` si `pixel_for_slot` da `None`.
pub fn slot_label(image: &Image, slot: u8) -> Option<String> {
    let what = match pixel_for_slot(image, slot)? {
        Pixel::Rgba(color) if color.a == 0 => "transparente".to_string(),
        Pixel::Rgba(color) => color.to_string(),
        Pixel::Indexed(index) => format!("índice {index}"),
        Pixel::Gray(gray) => format!("gris {gray}"),
        Pixel::GrayAlpha { alpha: 0, .. } => "transparente".to_string(),
        Pixel::GrayAlpha { gray, .. } => format!("gris {gray}"),
    };
    Some(format!("color {slot} {what}"))
}

/// Muestras de un modo sin paleta: las nueve ranuras.
const SLOT_CELLS: usize = SLOTS as usize;

/// Color activo: una ranura (1 a 9) o una entrada de la paleta. En imágenes indexadas se usa
/// siempre `Entry`; en las demás, `Slot` (forma normal que dan `choice_for_cell`, `key_choice` y
/// `initial_choice`). `Slot(n)` sobre una indexada sigue valiendo, como `pixel_for_slot`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ColorChoice {
    Slot(u8),
    Entry(u8),
}

impl From<u8> for ColorChoice {
    fn from(slot: u8) -> ColorChoice {
        ColorChoice::Slot(slot)
    }
}

/// Número de entradas de la paleta si la imagen es indexada.
fn palette_len(image: &Image) -> Option<usize> {
    (image.mode() == Mode::Indexed)
        .then(|| image.palette().map(|palette| palette.len()))
        .flatten()
}

/// `Pixel` de la elección para esa imagen: `Slot` como `pixel_for_slot`; `Entry(i)` es
/// `Indexed(i)` si la imagen es indexada e `i` está en su paleta, y `None` si no.
pub fn pixel_for_choice(image: &Image, choice: ColorChoice) -> Option<Pixel> {
    match choice {
        ColorChoice::Slot(slot) => pixel_for_slot(image, slot),
        ColorChoice::Entry(index) => {
            (usize::from(index) < palette_len(image)?).then_some(Pixel::Indexed(index))
        }
    }
}

/// Elección al abrir una imagen: la primera entrada si es indexada y, si no, la ranura 1.
pub fn initial_choice(image: &Image) -> ColorChoice {
    if image.mode() == Mode::Indexed {
        ColorChoice::Entry(0)
    } else {
        ColorChoice::Slot(1)
    }
}

/// Número de muestras del panel: las entradas de la paleta, o 9 ranuras en los demás modos.
pub fn cell_count(image: &Image) -> usize {
    palette_len(image).unwrap_or(SLOT_CELLS)
}

/// Elección de la muestra `cell` (desde 0): `Entry(cell)` en indexadas y `Slot(cell + 1)` en
/// las demás. `None` si no existe esa muestra.
pub fn choice_for_cell(image: &Image, cell: usize) -> Option<ColorChoice> {
    if cell >= cell_count(image) {
        return None;
    }
    if image.mode() == Mode::Indexed {
        u8::try_from(cell).ok().map(ColorChoice::Entry)
    } else {
        u8::try_from(cell + 1).ok().map(ColorChoice::Slot)
    }
}

/// Muestra que corresponde a la elección, o `None` si no vale para la imagen.
pub fn cell_of_choice(image: &Image, choice: ColorChoice) -> Option<usize> {
    pixel_for_choice(image, choice)?;
    Some(match choice {
        ColorChoice::Slot(slot) => usize::from(slot) - 1,
        ColorChoice::Entry(index) => usize::from(index),
    })
}

/// Elección de la tecla `n` (1 a 9): la muestra `n − 1`. `None` si no existe esa muestra.
pub fn key_choice(image: &Image, n: u8) -> Option<ColorChoice> {
    choice_for_cell(image, usize::from(n.checked_sub(1)?))
}

/// Color con el que se ve la muestra (el resuelto: el índice transparente tiene alfa 0).
pub fn cell_color(image: &Image, cell: usize) -> Option<Rgba> {
    let pixel = pixel_for_choice(image, choice_for_cell(image, cell)?)?;
    pixel_rgba(image, pixel)
}

/// La muestra es transparente: en indexadas, el índice transparente de la paleta; en RGBA y
/// gris con alfa, alfa 0; en gris, nunca.
pub fn is_transparent_cell(image: &Image, cell: usize) -> bool {
    if image.mode() == Mode::Indexed {
        let transparent = image
            .palette()
            .and_then(|palette| palette.transparent_index());
        return transparent.is_some_and(|index| usize::from(index) == cell);
    }
    cell_color(image, cell).is_some_and(|color| color.a == 0)
}

/// Línea 1 del panel: `índice 12 #ff8800` (color resuelto, así que la entrada transparente
/// da `#rrggbb00`), `color 3 #ff0000`, `color 4 gris 96` o `color 9 gris 0 alfa 0`.
pub fn cell_label(image: &Image, cell: usize) -> Option<String> {
    let choice = choice_for_cell(image, cell)?;
    let pixel = pixel_for_choice(image, choice)?;
    let color = pixel_rgba(image, pixel)?;
    Some(match (choice, pixel) {
        (ColorChoice::Entry(index), _) => format!("índice {index} {color}"),
        (ColorChoice::Slot(slot), Pixel::Rgba(color)) => format!("color {slot} {color}"),
        (ColorChoice::Slot(slot), Pixel::Gray(gray)) => format!("color {slot} gris {gray}"),
        (ColorChoice::Slot(slot), Pixel::GrayAlpha { gray, alpha: 255 }) => {
            format!("color {slot} gris {gray}")
        }
        (ColorChoice::Slot(slot), Pixel::GrayAlpha { gray, alpha }) => {
            format!("color {slot} gris {gray} alfa {alpha}")
        }
        (ColorChoice::Slot(slot), Pixel::Indexed(_)) => format!("color {slot} {color}"),
    })
}

/// Línea 2 del panel: `cursor`, `activa` y `transparente` (las que apliquen, en ese orden,
/// unidas con ` · `). Puede ser vacía.
pub fn info_flags(hover: bool, active: bool, transparent: bool) -> String {
    let flags = [
        (hover, "cursor"),
        (active, "activa"),
        (transparent, "transparente"),
    ];
    let shown: Vec<&str> = flags
        .iter()
        .filter(|(on, _)| *on)
        .map(|(_, name)| *name)
        .collect();
    shown.join(" · ")
}

/// Texto del color activo para la barra de estado: `Slot` como `slot_label`; `Entry(i)` es
/// `índice 12 #ff8800`, y la entrada transparente `índice 2 #00ff0000 transparente`.
pub fn choice_label(image: &Image, choice: ColorChoice) -> Option<String> {
    match choice {
        ColorChoice::Slot(slot) => slot_label(image, slot),
        ColorChoice::Entry(index) => {
            let cell = cell_of_choice(image, choice)?;
            let label = cell_label(image, cell)?;
            Some(if is_transparent_cell(image, usize::from(index)) {
                format!("{label} transparente")
            } else {
                label
            })
        }
    }
}
