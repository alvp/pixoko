//! Borrador y cuentagotas como lógica pura (specs/hito-2-cuentagotas-borrador.md): el valor que
//! escribe el borrador en cada modo, el color que elige el cuentagotas y el estado de su
//! arrastre. Sin tipos de egui: se prueba sin ventana.

use std::fmt;

use px_core::{Image, Mode, Pixel, Rgba};

/// Por qué el borrador no puede con una imagen.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EraseError {
    /// Indexada sin índice transparente.
    NoTransparentIndex,
    /// Gray: no tiene alfa.
    NoTransparency,
}

impl fmt::Display for EraseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            EraseError::NoTransparentIndex => "esta imagen no tiene índice transparente",
            EraseError::NoTransparency => "el modo gris no tiene transparencia",
        })
    }
}

/// Entrada que usa el borrador en una imagen indexada: el índice transparente si está fijado
/// y, si no, la primera entrada de la paleta con alfa guardado 0 (el `tRNS` de un PNG o lo que
/// se edita con el editor de color).
pub(crate) fn eraser_entry(image: &Image) -> Option<(u8, bool)> {
    let palette = image.palette()?;
    if let Some(index) = palette.transparent_index() {
        return Some((index, false));
    }
    let first = palette.colors().iter().position(|color| color.a == 0)?;
    Some((u8::try_from(first).ok()?, true))
}

/// Valor que escribe el borrador: en indexada, `Indexed(t)` con el índice transparente `t` o,
/// si no está fijado, la primera entrada de alfa 0; en RGBA el transparente canónico
/// (`(0, 0, 0, 0)`) y en GrayAlpha `gris 0, alfa 0`. Se calcula en cada llamada.
pub fn eraser_pixel(image: &Image) -> Result<Pixel, EraseError> {
    match image.mode() {
        Mode::Rgba => Ok(Pixel::Rgba(Rgba::TRANSPARENT)),
        Mode::GrayAlpha => Ok(Pixel::GrayAlpha { gray: 0, alpha: 0 }),
        Mode::Indexed => eraser_entry(image)
            .map(|(index, _)| Pixel::Indexed(index))
            .ok_or(EraseError::NoTransparentIndex),
        Mode::Gray => Err(EraseError::NoTransparency),
    }
}

/// Lo que acompaña a `borrador` en el título y la barra de estado: `transparente`,
/// `transparente (entrada N)` si usa una entrada de alfa 0 sin índice transparente, o el
/// motivo por el que no puede.
pub fn eraser_label(image: &Image) -> String {
    match eraser_pixel(image) {
        Ok(_) => match eraser_entry(image) {
            Some((index, true)) => format!("transparente (entrada {index})"),
            _ => "transparente".to_string(),
        },
        Err(EraseError::NoTransparentIndex) => "sin índice transparente".to_string(),
        Err(EraseError::NoTransparency) => "sin transparencia".to_string(),
    }
}

/// El borrador cambiaría ese píxel: falso si ya es transparente (alfa 0 una vez resuelto, aunque
/// su índice no sea el que escribe el borrador), si el borrador no puede con la imagen y si
/// `(x, y)` cae fuera.
pub fn is_erasable(image: &Image, x: u32, y: u32) -> bool {
    if eraser_pixel(image).is_err() {
        return false;
    }
    match image.get_pixel(x, y) {
        Ok(Pixel::Rgba(color)) => color.a != 0,
        Ok(Pixel::GrayAlpha { alpha, .. }) => alpha != 0,
        Ok(Pixel::Indexed(index)) => image
            .palette()
            .and_then(|palette| palette.resolved(index).ok())
            .is_some_and(|color| color.a != 0),
        _ => false,
    }
}

/// Lo que elige el cuentagotas.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Picked {
    /// Indexada: la entrada de la paleta del píxel.
    Entry(u8),
    /// RGBA, Gray y GrayAlpha: el valor exacto del píxel, alfa incluido (`r = g = b` en los
    /// grises).
    Color(Rgba),
}

/// Lo que elige el cuentagotas en `(x, y)`; `None` fuera de la imagen.
pub fn picked_at(image: &Image, x: u32, y: u32) -> Option<Picked> {
    Some(match image.get_pixel(x, y).ok()? {
        Pixel::Indexed(index) => Picked::Entry(index),
        Pixel::Rgba(color) => Picked::Color(color),
        Pixel::Gray(gray) => Picked::Color(Rgba::opaque(gray, gray, gray)),
        Pixel::GrayAlpha { gray, alpha } => Picked::Color(Rgba::new(gray, gray, gray, alpha)),
    })
}

/// Aviso de un color elegido con alfa 0: se copia tal cual, con su valor exacto.
pub fn pick_notice(picked: &Picked) -> Option<String> {
    match picked {
        Picked::Color(color) if color.a == 0 => {
            Some(format!("píxel transparente copiado tal cual ({color})"))
        }
        _ => None,
    }
}

/// Arrastre del cuentagotas: recuerda el último píxel visitado para no repetir la elección.
#[derive(Clone, Debug, Default)]
pub struct Dropper {
    last: Option<(u32, u32)>,
}

impl Dropper {
    pub fn new() -> Dropper {
        Dropper::default()
    }

    /// Visita `pos`: `Some` solo si cae dentro de la imagen y es otro píxel que el último
    /// visitado. Fuera de la imagen no hace nada (ni olvida el último).
    pub fn visit(&mut self, image: &Image, pos: (i32, i32)) -> Option<Picked> {
        let x = u32::try_from(pos.0).ok()?;
        let y = u32::try_from(pos.1).ok()?;
        if x >= image.width() || y >= image.height() || self.last == Some((x, y)) {
            return None;
        }
        self.last = Some((x, y));
        picked_at(image, x, y)
    }
}
