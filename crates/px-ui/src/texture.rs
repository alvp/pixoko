//! Conversión de una `Image` a bytes RGBA para subirla como textura
//! (specs/hito-2-ventana.md, «De `Image` a textura»).

use px_core::{Image, Pixel, Rgba};

/// Bytes RGBA de la imagen, `ancho · alto · 4`, fila a fila (la de arriba primero) y con alfa
/// directo (no premultiplicado).
///
/// - RGBA: tal cual.
/// - Indexado: el color efectivo de la paleta ([`px_core::Palette::resolved`]): el índice
///   transparente da `a = 0` y conserva su RGB.
/// - Gris: `(g, g, g, 255)`. Gris con alfa: `(g, g, g, a)`.
///
/// Reserva `4 · ancho · alto` bytes: quien la llame comprueba antes que el tamaño cabe en una
/// textura. Usa `get_pixel` hasta que `Image` dé acceso por bytes (unidad 12 del hito 2).
pub fn to_rgba8(image: &Image) -> Vec<u8> {
    let (width, height) = (image.width(), image.height());
    let mut bytes = Vec::with_capacity(width as usize * height as usize * 4);
    for y in 0..height {
        for x in 0..width {
            let pixel = image
                .get_pixel(x, y)
                .expect("las coordenadas del bucle están dentro de la imagen");
            let color =
                pixel_rgba(image, pixel).expect("un píxel indexado está en la paleta de su imagen");
            bytes.extend_from_slice(&[color.r, color.g, color.b, color.a]);
        }
    }
    bytes
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
