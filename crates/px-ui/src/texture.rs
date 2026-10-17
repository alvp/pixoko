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
            let color = match image
                .get_pixel(x, y)
                .expect("las coordenadas del bucle están dentro de la imagen")
            {
                Pixel::Rgba(color) => color,
                Pixel::Indexed(index) => image
                    .palette()
                    .and_then(|palette| palette.resolved(index).ok())
                    .expect("un píxel indexado está en la paleta de su imagen"),
                Pixel::Gray(g) => Rgba::opaque(g, g, g),
                Pixel::GrayAlpha { gray, alpha } => Rgba::new(gray, gray, gray, alpha),
            };
            bytes.extend_from_slice(&[color.r, color.g, color.b, color.a]);
        }
    }
    bytes
}
