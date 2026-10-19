//! Conversión de una `Image` a bytes RGBA para subirla como textura
//! (specs/hito-2-ventana.md, «De `Image` a textura»).

use egui::Color32;
use px_core::{Image, Mode, Palette, Pixel, Rgba};

use crate::canvas::CanvasError;
use crate::tiles::{TileAlloc, TileRect};

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

/// Los píxeles del rectángulo `rect` (recortado a la imagen) como `Color32` premultiplicado,
/// por filas: lo mismo que recortar `ColorImage::from_rgba_unmultiplied(to_rgba8(image))`, sin el
/// `Vec` intermedio de `to_rgba8` ni clonar la imagen. Fuera de la imagen, vacío.
pub fn tile_pixels(image: &Image, rect: TileRect) -> Vec<Color32> {
    let mut out = Vec::new();
    fill_tile(image, rect, None, &mut out);
    out
}

/// Como [`tile_pixels`], con la entrada `index` de la paleta valiendo `color` (vista previa de
/// la edición de un color; los bytes de [`to_rgba8_with_entry`]).
pub fn tile_pixels_with_entry(
    image: &Image,
    rect: TileRect,
    index: u8,
    color: Rgba,
) -> Vec<Color32> {
    let mut out = Vec::new();
    fill_tile(image, rect, Some((index, color)), &mut out);
    out
}

/// Como [`tile_pixels`], pidiendo el búfer a `alloc` (`Err(OutOfMemory)` si no hay).
pub fn try_tile_pixels(
    image: &Image,
    rect: TileRect,
    alloc: &mut dyn TileAlloc,
) -> Result<Vec<Color32>, CanvasError> {
    try_fill(image, rect, None, alloc)
}

/// Como [`tile_pixels_with_entry`], pidiendo el búfer a `alloc`.
pub fn try_tile_pixels_with_entry(
    image: &Image,
    rect: TileRect,
    index: u8,
    color: Rgba,
    alloc: &mut dyn TileAlloc,
) -> Result<Vec<Color32>, CanvasError> {
    try_fill(image, rect, Some((index, color)), alloc)
}

fn try_fill(
    image: &Image,
    rect: TileRect,
    entry: Option<(u8, Rgba)>,
    alloc: &mut dyn TileAlloc,
) -> Result<Vec<Color32>, CanvasError> {
    let Some(clipped) = clip_to_image(image, rect) else {
        return Ok(Vec::new());
    };
    let len = clipped.width as usize * clipped.height as usize;
    let mut out = alloc.pixels(len).ok_or(CanvasError::OutOfMemory)?;
    fill_tile(image, clipped, entry, &mut out);
    Ok(out)
}

fn clip_to_image(image: &Image, rect: TileRect) -> Option<TileRect> {
    rect.intersect(&TileRect {
        x: 0,
        y: 0,
        width: image.width(),
        height: image.height(),
    })
}

/// Añade a `out` los píxeles del recorte de `rect`; el búfer ya viene con la capacidad (o crece).
fn fill_tile(image: &Image, rect: TileRect, entry: Option<(u8, Rgba)>, out: &mut Vec<Color32>) {
    let Some(rect) = clip_to_image(image, rect) else {
        return;
    };
    let (x0, w) = (rect.x as usize, rect.width as usize);
    out.reserve_exact(w * rect.height as usize);
    let table = (image.mode() == Mode::Indexed).then(|| {
        let replace = entry.filter(|(index, _)| {
            image
                .palette()
                .is_some_and(|palette| usize::from(*index) < palette.len())
        });
        resolved_table(image.palette(), replace)
            .map(|c| Color32::from_rgba_unmultiplied(c[0], c[1], c[2], c[3]))
    });
    for y in rect.y..rect.y + rect.height {
        let Some(row) = image.row(y) else {
            break;
        };
        match image.mode() {
            Mode::Rgba => {
                for px in row[x0 * 4..(x0 + w) * 4].as_chunks::<4>().0 {
                    out.push(Color32::from_rgba_unmultiplied(px[0], px[1], px[2], px[3]));
                }
            }
            Mode::Gray => {
                for &g in &row[x0..x0 + w] {
                    out.push(Color32::from_rgba_unmultiplied(g, g, g, 255));
                }
            }
            Mode::GrayAlpha => {
                for px in row[x0 * 2..(x0 + w) * 2].as_chunks::<2>().0 {
                    out.push(Color32::from_rgba_unmultiplied(px[0], px[0], px[0], px[1]));
                }
            }
            Mode::Indexed => {
                if let Some(table) = &table {
                    for &index in &row[x0..x0 + w] {
                        out.push(table[usize::from(index)]);
                    }
                }
            }
        }
    }
}
