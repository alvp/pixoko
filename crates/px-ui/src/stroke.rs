//! Trazo del lápiz: posiciones del ratón unidas con líneas (specs/hito-2-lapiz-deshacer.md,
//! sección 1). No usa tipos de egui: se prueba sin ventana.

use std::collections::HashMap;

use px_core::{Command, Document, DocumentError, ImageId, Pixel, PixelEdit};
use px_raster::Line;

/// Píxeles que un trazo del lápiz dejará en la imagen, en el orden de su primera visita.
///
/// Solo guarda coordenadas dentro del lienzo; la última posición se guarda sin recortar.
#[derive(Clone, Debug)]
pub struct Stroke {
    width: u32,
    height: u32,
    last: Option<(i32, i32)>,
    pixels: Vec<PixelEdit>,
    /// Posición de cada coordenada en `pixels`.
    index: HashMap<(u32, u32), usize>,
}

impl Stroke {
    pub fn new(width: u32, height: u32) -> Stroke {
        Stroke {
            width,
            height,
            last: None,
            pixels: Vec::new(),
            index: HashMap::new(),
        }
    }

    /// Primera llamada: pinta `pos`. Después: la línea de la posición anterior a `pos`, con el
    /// `pixel` de esta llamada en todo el segmento (también en el extremo ya pintado). Una
    /// posición igual a la anterior no hace nada, aunque `pixel` sea otro.
    ///
    /// Si una coordenada se repite, conserva el lugar de su primera visita y el último `pixel`.
    pub fn move_to(&mut self, pos: (i32, i32), pixel: Pixel) {
        if self.last == Some(pos) {
            return;
        }
        let from = self.last.unwrap_or(pos);
        self.last = Some(pos);
        for (x, y) in Line::new(from, pos).clip_to(self.width, self.height) {
            // Tras recortar, `x` e `y` están dentro del lienzo: la conversión no falla.
            if let (Ok(x), Ok(y)) = (u32::try_from(x), u32::try_from(y)) {
                self.write(x, y, pixel);
            }
        }
    }

    /// Stamps the mask `offsets` (relative to the center) at `center`, clipped to the canvas, with
    /// `pixel`. A coordinate that repeats keeps its first place and takes the last `pixel`.
    pub fn stamp(&mut self, offsets: &[(i32, i32)], center: (i32, i32), pixel: Pixel) {
        self.last = Some(center);
        for &(dx, dy) in offsets {
            let (x, y) = (
                i64::from(center.0) + i64::from(dx),
                i64::from(center.1) + i64::from(dy),
            );
            if let (Ok(x), Ok(y)) = (u32::try_from(x), u32::try_from(y))
                && x < self.width
                && y < self.height
            {
                self.write(x, y, pixel);
            }
        }
    }

    /// Adds pixels that are already inside the canvas (the union of the figures left as traces).
    pub fn add(&mut self, pixels: impl IntoIterator<Item = (u32, u32)>, pixel: Pixel) {
        for (x, y) in pixels {
            self.write(x, y, pixel);
        }
    }

    fn write(&mut self, x: u32, y: u32, pixel: Pixel) {
        match self.index.get(&(x, y)) {
            Some(&at) => self.pixels[at].pixel = pixel,
            None => {
                self.index.insert((x, y), self.pixels.len());
                self.pixels.push(PixelEdit { x, y, pixel });
            }
        }
    }

    /// Píxeles pendientes, sin repetidos, en el orden de primera visita.
    pub fn pixels(&self) -> &[PixelEdit] {
        &self.pixels
    }

    /// Los píxeles pendientes, sin copiarlos.
    pub fn into_pixels(self) -> Vec<PixelEdit> {
        self.pixels
    }

    /// The size of the canvas the stroke is clipped to.
    pub fn size(&self) -> (u32, u32) {
        (self.width, self.height)
    }

    pub fn is_empty(&self) -> bool {
        self.pixels.is_empty()
    }

    /// Última posición recibida, sin recortar al lienzo.
    pub fn last(&self) -> Option<(i32, i32)> {
        self.last
    }

    /// Aplica el trazo como un solo `SetPixels`, es decir, un solo paso de deshacer. Vacío:
    /// `Ok(false)` sin llamar a `apply` (un `SetPixels` vacío sería `EmptyCommand`). Si no, lo
    /// que devuelva `Document::apply`.
    pub fn commit(self, doc: &mut Document, image: ImageId) -> Result<bool, DocumentError> {
        if self.pixels.is_empty() {
            return Ok(false);
        }
        doc.apply(Command::SetPixels {
            image,
            pixels: self.pixels,
        })
    }
}
