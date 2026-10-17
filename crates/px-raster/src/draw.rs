//! La línea como comando de `Document`.

use px_core::{Command, Document, DocumentError, ImageId, Pixel, PixelEdit};

use crate::Line;

/// `SetPixels` con los píxeles del segmento de `a` a `b` que caen dentro de la imagen, en el
/// orden de la línea; `None` si no cae ninguno.
///
/// Solo lee el tamaño de la imagen. No comprueba que `pixel` sea del modo de la imagen: eso lo
/// valida `Document::apply`. Reserva el `Vec` una vez, con `try_reserve_exact`.
pub fn line_command(
    doc: &Document,
    image: ImageId,
    a: (i32, i32),
    b: (i32, i32),
    pixel: Pixel,
) -> Result<Option<Command>, DocumentError> {
    let target = doc.image(image).ok_or(DocumentError::UnknownImage(image))?;
    let line = Line::new(a, b).clip_to(target.width(), target.height());
    if line.is_empty() {
        return Ok(None);
    }
    let len = usize::try_from(line.len()).map_err(|_| DocumentError::OutOfMemory)?;
    let mut pixels = Vec::new();
    pixels
        .try_reserve_exact(len)
        .map_err(|_| DocumentError::OutOfMemory)?;
    for (x, y) in line {
        // Tras recortar, `x` e `y` están en `0..width` y `0..height`: la conversión no falla.
        if let (Ok(x), Ok(y)) = (u32::try_from(x), u32::try_from(y)) {
            pixels.push(PixelEdit { x, y, pixel });
        }
    }
    Ok(Some(Command::SetPixels { image, pixels }))
}

/// Dibuja la línea en `doc` como un solo paso de deshacer. Devuelve lo que devuelve
/// `Document::apply`; si no cae ningún píxel dentro de la imagen, `Ok(false)` sin llamar a
/// `apply` (un `SetPixels` vacío sería `EmptyCommand`).
pub fn draw_line(
    doc: &mut Document,
    image: ImageId,
    a: (i32, i32),
    b: (i32, i32),
    pixel: Pixel,
) -> Result<bool, DocumentError> {
    match line_command(doc, image, a, b, pixel)? {
        Some(command) => doc.apply(command),
        None => Ok(false),
    }
}
