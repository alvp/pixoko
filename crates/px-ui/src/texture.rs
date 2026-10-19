//! Conversión de una `Image` a bytes RGBA para subirla como textura
//! (specs/hito-2-ventana.md, «De `Image` a textura»).

use px_core::{Image, Mode, Palette, Pixel, Rgba};

/// Bytes RGBA de la imagen, `ancho · alto · 4`, fila a fila (la de arriba primero) y con alfa
/// directo (no premultiplicado).
///
/// - RGBA: tal cual.
/// - Indexado: el color efectivo de la paleta ([`px_core::Palette::resolved`]): el índice
///   transparente da `a = 0` y conserva su RGB.
/// - Gris: `(g, g, g, 255)`. Gris con alfa: `(g, g, g, a)`.
///
/// Reserva `4 · ancho · alto` bytes: quien la llame comprueba antes que el tamaño cabe en una
/// textura. Recorre `Image::bytes`; `pixel_rgba` es la misma conversión para un solo píxel.
pub fn to_rgba8(image: &Image) -> Vec<u8> {
    let (width, height) = (image.width(), image.height());
    let mut bytes = Vec::with_capacity(width as usize * height as usize * 4);
    match image.mode() {
        Mode::Rgba => bytes.extend_from_slice(image.bytes()),
        Mode::Gray => {
            for &gray in image.bytes() {
                bytes.extend_from_slice(&[gray, gray, gray, 255]);
            }
        }
        Mode::GrayAlpha => {
            for pixel in image.bytes().as_chunks::<2>().0 {
                bytes.extend_from_slice(&[pixel[0], pixel[0], pixel[0], pixel[1]]);
            }
        }
        Mode::Indexed => {
            let table = resolved_table(image.palette(), None);
            for &index in image.bytes() {
                bytes.extend_from_slice(&table[usize::from(index)]);
            }
        }
    }
    bytes
}

/// Color resuelto de cada índice como bytes RGBA (los que no están en la paleta, a ceros),
/// con `replace` en lugar de la entrada que dice: el índice transparente sigue con alfa 0 y
/// conserva el RGB del color nuevo.
fn resolved_table(palette: Option<&Palette>, replace: Option<(u8, Rgba)>) -> [[u8; 4]; 256] {
    let mut table = [[0u8; 4]; 256];
    let Some(palette) = palette else {
        return table;
    };
    for (slot, index) in table.iter_mut().zip(0..=u8::MAX) {
        let color = match replace {
            Some((target, color)) if target == index => {
                if palette.transparent_index() == Some(index) {
                    Some(Rgba { a: 0, ..color })
                } else {
                    Some(color)
                }
            }
            _ => palette.resolved(index).ok(),
        };
        if let Some(color) = color {
            *slot = [color.r, color.g, color.b, color.a];
        }
    }
    table
}

/// Color con el que se ve `pixel` en `image`: el de la paleta para un índice (`None` si no está
/// en ella o la imagen no es indexada), gris opaco o con alfa para los grises, y el propio
/// color en RGBA. Es la conversión de [`to_rgba8`], suelta, para pintar la superposición del
/// lápiz.
pub fn pixel_rgba(image: &Image, pixel: Pixel) -> Option<Rgba> {
    match pixel {
        Pixel::Rgba(color) => Some(color),
        Pixel::Indexed(index) => image
            .palette()
            .and_then(|palette| palette.resolved(index).ok()),
        Pixel::Gray(g) => Some(Rgba::opaque(g, g, g)),
        Pixel::GrayAlpha { gray, alpha } => Some(Rgba::new(gray, gray, gray, alpha)),
    }
}

/// Como [`to_rgba8`], pero con la entrada `index` de la paleta valiendo `color`, sin clonar la
/// imagen (vista previa de la edición de un color). El índice transparente sigue con alfa 0 y
/// conserva el RGB de `color`. Si la imagen no es indexada o `index` no está en la paleta, es
/// `to_rgba8(image)`. Sustituye por **índice**, no por color: las entradas duplicadas no se
/// tocan.
pub fn to_rgba8_with_entry(image: &Image, index: u8, color: Rgba) -> Vec<u8> {
    let Some(palette) = image
        .palette()
        .filter(|palette| image.mode() == Mode::Indexed && usize::from(index) < palette.len())
    else {
        return to_rgba8(image);
    };
    // Tabla índice → color resuelto: una consulta por píxel en vez de una a la paleta.
    let table = resolved_table(Some(palette), Some((index, color)));
    let (width, height) = (image.width(), image.height());
    let mut bytes = Vec::with_capacity(width as usize * height as usize * 4);
    for &index in image.bytes() {
        bytes.extend_from_slice(&table[usize::from(index)]);
    }
    bytes
}
