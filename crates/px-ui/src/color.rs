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

/// Cuál de los dos colores: el de primer plano (botón izquierdo) o el de fondo (botón derecho)
/// (specs/interfaz-dos-colores.md).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Role {
    #[default]
    Foreground,
    Background,
}

/// Fondo por defecto de una imagen: en indexadas, el índice transparente si está fijado, si no la
/// primera entrada de alfa guardado 0 (la regla del borrador) y, si no hay, la entrada 0; en RGBA y
/// gris con alfa, la ranura 9 (transparente); en gris, la ranura 9 (blanco).
pub fn default_background(image: &Image) -> ColorChoice {
    match image.mode() {
        Mode::Indexed => {
            ColorChoice::Entry(crate::pick::eraser_entry(image).map_or(0, |(index, _)| index))
        }
        Mode::Rgba | Mode::GrayAlpha | Mode::Gray => ColorChoice::Slot(SLOTS),
    }
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
    cell_color_with(image, cell, &SlotColors::defaults(image.mode()))
}

/// La muestra es transparente: en indexadas, el índice transparente de la paleta; en RGBA y
/// gris con alfa, alfa 0; en gris, nunca.
pub fn is_transparent_cell(image: &Image, cell: usize) -> bool {
    is_transparent_cell_with(image, cell, &SlotColors::defaults(image.mode()))
}

/// Línea 1 del panel: `índice 12 #ff8800` (color resuelto, así que la entrada transparente
/// da `#rrggbb00`), `color 3 #ff0000`, `color 4 gris 96` o `color 9 gris 0 alfa 0`.
pub fn cell_label(image: &Image, cell: usize) -> Option<String> {
    cell_label_with(image, cell, &SlotColors::defaults(image.mode()))
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

/// Como [`info_flags`] con la bandera `rango` (la muestra está en el rango de rotación y no es
/// la activa), entre `activa` y `transparente`.
pub fn info_flags_range(hover: bool, active: bool, in_range: bool, transparent: bool) -> String {
    let flags = [
        (hover, "cursor"),
        (active, "activa"),
        (in_range && !active, "rango"),
        (transparent, "transparente"),
    ];
    let shown: Vec<&str> = flags
        .iter()
        .filter(|(on, _)| *on)
        .map(|(_, name)| *name)
        .collect();
    shown.join(" · ")
}

/// Como [`info_flags_range`] con la bandera `alfa 0` (entrada indexada con alfa guardado 0 que no
/// es el índice transparente), al final. `transparente` y `alfa 0` son excluyentes: si se piden
/// las dos, gana `transparente` (specs/hito-2-fijar-transparente.md).
pub fn info_flags_alpha0(
    hover: bool,
    active: bool,
    in_range: bool,
    transparent: bool,
    alpha0: bool,
) -> String {
    let flags = [
        (hover, "cursor"),
        (active, "activa"),
        (in_range && !active, "rango"),
        (transparent, "transparente"),
        (alpha0 && !transparent, "alfa 0"),
    ];
    let shown: Vec<&str> = flags
        .iter()
        .filter(|(on, _)| *on)
        .map(|(_, name)| *name)
        .collect();
    shown.join(" · ")
}

/// Como [`info_flags_alpha0`] con los dos colores: `frente` y `fondo` en lugar de `activa`. Orden:
/// `cursor`, `frente`, `fondo`, `rango` (en el rango y no es el primer plano), `transparente` y
/// `alfa 0` (excluyentes). Si no cabe en `columns` caracteres se omite `cursor`.
pub fn info_flags_roles(flags: RoleFlags, columns: usize) -> String {
    let join = |with_cursor: bool| {
        let list = [
            (with_cursor && flags.hover, "cursor"),
            (flags.foreground, "frente"),
            (flags.background, "fondo"),
            (flags.in_range && !flags.foreground, "rango"),
            (flags.transparent, "transparente"),
            (flags.alpha0 && !flags.transparent, "alfa 0"),
        ];
        let shown: Vec<&str> = list
            .iter()
            .filter(|(on, _)| *on)
            .map(|(_, name)| *name)
            .collect();
        shown.join(" · ")
    };
    let full = join(true);
    if full.chars().count() > columns {
        join(false)
    } else {
        full
    }
}

/// Lo que dice la línea 2 de una muestra.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct RoleFlags {
    pub hover: bool,
    pub foreground: bool,
    pub background: bool,
    pub in_range: bool,
    pub transparent: bool,
    pub alpha0: bool,
}

/// Deja la elección válida para la imagen: en indexadas `Entry(min(i, len − 1))` (`Slot(n)` pasa
/// a `Entry(n − 1)`); en los demás modos, igual si vale y si no la inicial.
pub fn clamp_choice(image: &Image, choice: ColorChoice) -> ColorChoice {
    match palette_len(image) {
        Some(len) => {
            let index = match choice {
                ColorChoice::Entry(i) => usize::from(i),
                ColorChoice::Slot(n) => usize::from(n).saturating_sub(1),
            };
            let last = len.saturating_sub(1);
            ColorChoice::Entry(u8::try_from(index.min(last)).unwrap_or(u8::MAX))
        }
        None if pixel_for_choice(image, choice).is_some() => choice,
        None => initial_choice(image),
    }
}

/// Texto del color activo para la barra de estado: `Slot` como `slot_label`; `Entry(i)` es
/// `índice 12 #ff8800`, y la entrada transparente `índice 2 #00ff0000 transparente`.
pub fn choice_label(image: &Image, choice: ColorChoice) -> Option<String> {
    choice_label_with(image, choice, &SlotColors::defaults(image.mode()))
}

/// Colores de las nueve ranuras de un modo sin paleta (specs/hito-2-editar-color.md). Los
/// valores por defecto son los de `pixel_for_slot`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SlotColors {
    colors: [Rgba; SLOT_CELLS],
}

impl SlotColors {
    pub fn defaults(mode: Mode) -> SlotColors {
        let mut colors = [Rgba::TRANSPARENT; SLOT_CELLS];
        for slot in 1..=SLOTS {
            let gray = if slot == SLOTS { 255 } else { (slot - 1) * 32 };
            colors[usize::from(slot - 1)] = match mode {
                Mode::Rgba | Mode::Indexed => COLORS
                    .get(usize::from(slot - 1))
                    .copied()
                    .unwrap_or(Rgba::TRANSPARENT),
                Mode::Gray => Rgba::opaque(gray, gray, gray),
                Mode::GrayAlpha if slot == SLOTS => Rgba::TRANSPARENT,
                Mode::GrayAlpha => Rgba::opaque(gray, gray, gray),
            };
        }
        SlotColors { colors }
    }

    /// Color de la ranura `slot` (1 a 9).
    pub fn get(&self, slot: u8) -> Option<Rgba> {
        let index = usize::from(slot.checked_sub(1)?);
        self.colors.get(index).copied()
    }

    /// Cambia la ranura `slot` (1 a 9); `false` si no existe.
    pub fn set(&mut self, slot: u8, color: Rgba) -> bool {
        let Some(index) = slot.checked_sub(1) else {
            return false;
        };
        match self.colors.get_mut(usize::from(index)) {
            Some(entry) => {
                *entry = color;
                true
            }
            None => false,
        }
    }
}

/// Una tabla de ranuras por modo; indexadas comparte la de RGBA (no se usa).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SlotBank {
    rgba: SlotColors,
    gray: SlotColors,
    gray_alpha: SlotColors,
}

impl SlotBank {
    pub fn new() -> SlotBank {
        SlotBank {
            rgba: SlotColors::defaults(Mode::Rgba),
            gray: SlotColors::defaults(Mode::Gray),
            gray_alpha: SlotColors::defaults(Mode::GrayAlpha),
        }
    }

    pub fn get(&self, mode: Mode) -> &SlotColors {
        match mode {
            Mode::Rgba | Mode::Indexed => &self.rgba,
            Mode::Gray => &self.gray,
            Mode::GrayAlpha => &self.gray_alpha,
        }
    }

    pub fn get_mut(&mut self, mode: Mode) -> &mut SlotColors {
        match mode {
            Mode::Rgba | Mode::Indexed => &mut self.rgba,
            Mode::Gray => &mut self.gray,
            Mode::GrayAlpha => &mut self.gray_alpha,
        }
    }
}

impl Default for SlotBank {
    fn default() -> SlotBank {
        SlotBank::new()
    }
}

/// Como [`pixel_for_choice`], pero con los colores de ranura de `slots`. En Gray el `Pixel` es
/// `Gray(color.r)` y en GrayAlpha `GrayAlpha { gray: color.r, alpha: color.a }`: el editor
/// garantiza `r = g = b` en esos modos.
pub fn pixel_for_choice_with(
    image: &Image,
    choice: ColorChoice,
    slots: &SlotColors,
) -> Option<Pixel> {
    let ColorChoice::Slot(slot) = choice else {
        return pixel_for_choice(image, choice);
    };
    let color = slots.get(slot)?;
    match image.mode() {
        Mode::Rgba => Some(Pixel::Rgba(color)),
        Mode::Gray => Some(Pixel::Gray(color.r)),
        Mode::GrayAlpha => Some(Pixel::GrayAlpha {
            gray: color.r,
            alpha: color.a,
        }),
        Mode::Indexed => pixel_for_slot(image, slot),
    }
}

/// Como [`cell_color`], con los colores de ranura de `slots`.
pub fn cell_color_with(image: &Image, cell: usize, slots: &SlotColors) -> Option<Rgba> {
    let pixel = pixel_for_choice_with(image, choice_for_cell(image, cell)?, slots)?;
    pixel_rgba(image, pixel)
}

/// Como [`is_transparent_cell`], con los colores de ranura de `slots`.
pub fn is_transparent_cell_with(image: &Image, cell: usize, slots: &SlotColors) -> bool {
    if image.mode() == Mode::Indexed {
        let transparent = image
            .palette()
            .and_then(|palette| palette.transparent_index());
        return transparent.is_some_and(|index| usize::from(index) == cell);
    }
    cell_color_with(image, cell, slots).is_some_and(|color| color.a == 0)
}

/// La muestra es una entrada indexada con alfa guardado 0 que no es el índice transparente (el
/// caso del `tRNS` de un PNG). En los demás modos, falso: `is_transparent_cell_with` ya dice
/// `transparente` para el alfa 0.
pub fn is_alpha0_cell_with(image: &Image, cell: usize, _slots: &SlotColors) -> bool {
    if image.mode() != Mode::Indexed {
        return false;
    }
    let Some(palette) = image.palette() else {
        return false;
    };
    let Ok(index) = u8::try_from(cell) else {
        return false;
    };
    palette.transparent_index() != Some(index) && palette.get(index).is_ok_and(|color| color.a == 0)
}

/// Como [`cell_label`], con los colores de ranura de `slots`.
pub fn cell_label_with(image: &Image, cell: usize, slots: &SlotColors) -> Option<String> {
    let choice = choice_for_cell(image, cell)?;
    let pixel = pixel_for_choice_with(image, choice, slots)?;
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

/// Como [`choice_label`], con los colores de ranura de `slots`.
pub fn choice_label_with(image: &Image, choice: ColorChoice, slots: &SlotColors) -> Option<String> {
    match choice {
        ColorChoice::Slot(slot) => {
            let what = match pixel_for_choice_with(image, choice, slots)? {
                Pixel::Rgba(color) if color.a == 0 => "transparente".to_string(),
                Pixel::Rgba(color) => color.to_string(),
                Pixel::Indexed(index) => format!("índice {index}"),
                Pixel::Gray(gray) => format!("gris {gray}"),
                Pixel::GrayAlpha { alpha: 0, .. } => "transparente".to_string(),
                Pixel::GrayAlpha { gray, .. } => format!("gris {gray}"),
            };
            Some(format!("color {slot} {what}"))
        }
        ColorChoice::Entry(index) => {
            let cell = cell_of_choice(image, choice)?;
            let label = cell_label_with(image, cell, slots)?;
            Some(
                if is_transparent_cell_with(image, usize::from(index), slots) {
                    format!("{label} transparente")
                } else {
                    label
                },
            )
        }
    }
}
