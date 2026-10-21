//! De dónde salen los píxeles del lienzo con capas (specs/hito-3-capas-vista.md, unidad 3.3): una
//! [`Image`] o la composición de las capas visibles del documento ([`DocSource`]), la imagen que
//! se guarda ([`flatten`]) y lo que se ve en un píxel ([`visible_picked`], [`overlay_pixels`]).
//!
//! La composición vive en `px-raster` (`compose_rect`, `composite_pixel`); aquí solo se pide por
//! rectángulos (las tiras sucias de las baldosas) o por píxel (el cuentagotas y la superposición
//! del trazo). Con **una** capa visible, opaca y Normal (todo lo de antes de las capas) se lee la
//! celda tal cual ([`identity_cell`]): mismos píxeles y sin copiar nada.

use std::borrow::Cow;

use egui::Color32;
use px_core::{BlendMode, Document, Image, ImageId, Mode, Pixel, PixelEdit, Rgba};
use px_raster::{ComposeError, Rect, compose, compose_rect, composite_pixel};

use crate::canvas::CanvasError;
use crate::pick::{Picked, picked_at};
use crate::texture::{pixel_rgba, try_tile_pixels, try_tile_pixels_with_entry};
use crate::tiles::{TileAlloc, TileRect};

/// Origen de los píxeles que el lienzo sube a las baldosas.
pub trait PixelSource {
    fn width(&self) -> u32;
    fn height(&self) -> u32;
    /// El índice transparente de la paleta, si el origen es indexado y lo tiene.
    fn transparent_index(&self) -> Option<u8>;
    /// Los píxeles de `rect` (recortado al lienzo), por filas y como `Color32` premultiplicado.
    /// Con `entry`, la entrada de la paleta vale ese color (vista previa del editor de color).
    fn region(
        &self,
        rect: TileRect,
        entry: Option<(u8, Rgba)>,
        alloc: &mut dyn TileAlloc,
    ) -> Result<Vec<Color32>, CanvasError>;
}

impl PixelSource for Image {
    fn width(&self) -> u32 {
        Image::width(self)
    }

    fn height(&self) -> u32 {
        Image::height(self)
    }

    fn transparent_index(&self) -> Option<u8> {
        self.palette()
            .and_then(|palette| palette.transparent_index())
    }

    fn region(
        &self,
        rect: TileRect,
        entry: Option<(u8, Rgba)>,
        alloc: &mut dyn TileAlloc,
    ) -> Result<Vec<Color32>, CanvasError> {
        match entry {
            None => try_tile_pixels(self, rect, alloc),
            Some((index, color)) => try_tile_pixels_with_entry(self, rect, index, color, alloc),
        }
    }
}

impl<T: PixelSource + ?Sized> PixelSource for &T {
    fn width(&self) -> u32 {
        (**self).width()
    }

    fn height(&self) -> u32 {
        (**self).height()
    }

    fn transparent_index(&self) -> Option<u8> {
        (**self).transparent_index()
    }

    fn region(
        &self,
        rect: TileRect,
        entry: Option<(u8, Rgba)>,
        alloc: &mut dyn TileAlloc,
    ) -> Result<Vec<Color32>, CanvasError> {
        (**self).region(rect, entry, alloc)
    }
}

/// La celda tal cual se ve: el documento tiene una sola capa, visible, de opacidad 255, Normal y
/// con imagen. Es el camino de todo lo anterior a las capas: no compone nada.
pub fn identity_cell(doc: &Document) -> Option<&Image> {
    match doc.layers() {
        [layer]
            if layer.visible() && layer.opacity() == 255 && layer.blend() == BlendMode::Normal =>
        {
            doc.image(layer.cel())
        }
        _ => None,
    }
}

/// La composición de las capas visibles del documento, por rectángulos.
pub struct DocSource<'a>(pub &'a Document);

impl PixelSource for DocSource<'_> {
    fn width(&self) -> u32 {
        self.0.width()
    }

    fn height(&self) -> u32 {
        self.0.height()
    }

    fn transparent_index(&self) -> Option<u8> {
        self.0
            .palette()
            .and_then(|palette| palette.transparent_index())
    }

    fn region(
        &self,
        rect: TileRect,
        entry: Option<(u8, Rgba)>,
        alloc: &mut dyn TileAlloc,
    ) -> Result<Vec<Color32>, CanvasError> {
        if let Some(cell) = identity_cell(self.0) {
            return cell.region(rect, entry, alloc);
        }
        let Some(clipped) = rect.intersect(&TileRect {
            x: 0,
            y: 0,
            width: self.0.width(),
            height: self.0.height(),
        }) else {
            return Ok(Vec::new());
        };
        let composed = compose_rect(
            self.0,
            Rect {
                x: clipped.x,
                y: clipped.y,
                width: clipped.width,
                height: clipped.height,
            },
        )
        // `EmptyRect` y `RectOutside` no pasan (el rectángulo viene recortado y con área);
        // `NoTransparency` es un documento sin nada que mostrar; el resto, falta de memoria.
        .map_err(|_| CanvasError::OutOfMemory)?;
        composed.region(
            TileRect {
                x: 0,
                y: 0,
                width: clipped.width,
                height: clipped.height,
            },
            entry,
            alloc,
        )
    }
}

/// La imagen que se guarda: la celda tal cual si es la identidad (el PNG es idéntico al de antes
/// de las capas y no se copia nada) y, si no, la composición de las capas visibles.
pub fn flatten(doc: &Document) -> Result<Cow<'_, Image>, ComposeError> {
    match identity_cell(doc) {
        Some(cell) => Ok(Cow::Borrowed(cell)),
        None => compose(doc).map(Cow::Owned),
    }
}

/// El valor (del modo del documento) que se ve en `(x, y)`, con el píxel de la celda `over.0`
/// sustituido por `over.1` si se pide. Es el mismo recorrido que `compose_rect`, de abajo arriba
/// y con `composite_pixel`; `None` fuera del lienzo o si no hay nada visible que dar (gris sin
/// ninguna capa visible con imagen).
fn visible_value(doc: &Document, x: u32, y: u32, over: Option<(ImageId, Pixel)>) -> Option<Pixel> {
    if x >= doc.width() || y >= doc.height() {
        return None;
    }
    let mode = doc.mode();
    let transparent = doc
        .palette()
        .and_then(|palette| palette.transparent_index());
    let mut back = Rgba::TRANSPARENT;
    let mut index = transparent.unwrap_or(0);
    let mut gray = None;
    for layer in doc.layers() {
        if !layer.visible() {
            continue;
        }
        let value = match over {
            Some((cel, pixel)) if cel == layer.cel() => pixel,
            _ => match doc.image(layer.cel()) {
                Some(cell) => cell.get_pixel(x, y).ok()?,
                None => continue,
            },
        };
        match (mode, value) {
            (Mode::Rgba, Pixel::Rgba(src)) => {
                back = composite_pixel(back, src, layer.opacity(), layer.blend());
            }
            (Mode::GrayAlpha, Pixel::GrayAlpha { gray: g, alpha }) => {
                back = composite_pixel(
                    back,
                    Rgba::new(g, g, g, alpha),
                    layer.opacity(),
                    layer.blend(),
                );
            }
            (Mode::Indexed, Pixel::Indexed(i)) => {
                if Some(i) != transparent {
                    index = i;
                }
            }
            (Mode::Gray, Pixel::Gray(g)) => gray = Some(g),
            _ => return None,
        }
    }
    match mode {
        Mode::Rgba => Some(Pixel::Rgba(back)),
        Mode::GrayAlpha => Some(Pixel::GrayAlpha {
            gray: back.r,
            alpha: back.a,
        }),
        Mode::Indexed => Some(Pixel::Indexed(index)),
        Mode::Gray => gray.map(Pixel::Gray),
    }
}

/// Lo que elige el cuentagotas en `(x, y)` mirando lo que se ve: la composición del píxel (en
/// indexado, la entrada de la capa visible más alta que no es la transparente). Con una sola
/// capa visible y opaca es lo que hay en la celda, alfa 0 incluido. `None` fuera del lienzo.
pub fn visible_picked(doc: &Document, x: u32, y: u32) -> Option<Picked> {
    if let Some(cell) = identity_cell(doc) {
        return picked_at(cell, x, y);
    }
    Some(match visible_value(doc, x, y, None)? {
        Pixel::Indexed(index) => Picked::Entry(index),
        Pixel::Rgba(color) => Picked::Color(color),
        Pixel::Gray(gray) => Picked::Color(Rgba::opaque(gray, gray, gray)),
        Pixel::GrayAlpha { gray, alpha } => Picked::Color(Rgba::new(gray, gray, gray, alpha)),
    })
}

/// Los colores de la superposición de un trazo pendiente: por cada edición dentro del lienzo, el
/// color que se verá en ese píxel con el valor de la celda activa `active` sustituido por el del
/// trazo (el borrador, por el transparente). Es la composición de después de aplicar el trazo,
/// así que al soltar no cambia ningún píxel. Con una sola capa visible y opaca es el color
/// del propio trazo, como antes de las capas.
pub fn overlay_pixels(
    doc: &Document,
    active: ImageId,
    edits: impl IntoIterator<Item = PixelEdit>,
) -> Vec<(u32, u32, Rgba)> {
    let identity = identity_cell(doc).is_some();
    let reference = doc.canvas_image();
    edits
        .into_iter()
        .filter_map(|edit| {
            let pixel = if identity {
                (edit.x < doc.width() && edit.y < doc.height()).then_some(edit.pixel)?
            } else {
                visible_value(doc, edit.x, edit.y, Some((active, edit.pixel)))?
            };
            Some((edit.x, edit.y, pixel_rgba(reference, pixel)?))
        })
        .collect()
}

/// Los colores de la superposición de un trazo que crece, calculados una sola vez: cada fotograma
/// la app pide los colores de **todo** el trazo pendiente, y con varias capas cada uno cuesta una
/// pasada por las capas; el caché solo calcula lo añadido. Se vacía cuando cambia el documento
/// (`state_id`) o la capa activa, o cuando la lista no continúa a la anterior (la línea que cambia
/// de extremo).
#[derive(Debug, Default)]
pub struct OverlayCache {
    key: Option<(u64, ImageId)>,
    edits: Vec<PixelEdit>,
    colors: Vec<(u32, u32, Rgba)>,
    computed: usize,
}

impl OverlayCache {
    /// Los colores de `edits` (como [`overlay_pixels`]), reutilizando los de la llamada anterior si
    /// `edits` la continúa.
    pub fn colors(
        &mut self,
        doc: &Document,
        active: ImageId,
        edits: impl IntoIterator<Item = PixelEdit>,
    ) -> &[(u32, u32, Rgba)] {
        let edits: Vec<PixelEdit> = edits.into_iter().collect();
        let key = Some((doc.state_id(), active));
        let known = if key == self.key && edits.starts_with(&self.edits) {
            self.edits.len()
        } else {
            self.colors.clear();
            0
        };
        self.computed = edits.len() - known;
        self.colors
            .extend(overlay_pixels(doc, active, edits[known..].iter().copied()));
        self.edits = edits;
        self.key = key;
        &self.colors
    }

    /// Cuántas ediciones calculó la última llamada (las demás venían del caché).
    pub fn last_computed(&self) -> usize {
        self.computed
    }
}
