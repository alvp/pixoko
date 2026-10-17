//! Color activo del lápiz: nueve ranuras (teclas 1 a 9) que dan un `Pixel` del modo de la
//! imagen (specs/hito-2-lapiz-deshacer.md, sección 4). Hasta el panel de paletas (unidad 11).

use px_core::{Image, Mode, Pixel, Rgba};

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
