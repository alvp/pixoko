//! Fondo elegible bajo la transparencia del lienzo, sin tipos de egui en la interfaz pública
//! (specs/hito-2-damero-lienzo.md, decisión 12). Es una preferencia de la sesión: no es del
//! documento y no se guarda en disco todavía.

use px_core::Rgba;

use crate::canvas::BACKGROUND;

/// Qué se ve bajo los píxeles transparentes, dentro de la imagen.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum CanvasBackground {
    /// El damero de dos grises (el de las muestras transparentes del panel de paletas).
    #[default]
    Checker,
    Black,
    /// El gris liso de siempre, el de [`BACKGROUND`].
    Gray,
    White,
}

/// Cómo se rellena ese fondo.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Fill {
    Checker,
    Solid(Rgba),
}

pub const BLACK: Rgba = Rgba::opaque(0, 0, 0);
pub const WHITE: Rgba = Rgba::opaque(255, 255, 255);

pub fn background_fill(background: CanvasBackground) -> Fill {
    match background {
        CanvasBackground::Checker => Fill::Checker,
        CanvasBackground::Black => Fill::Solid(BLACK),
        CanvasBackground::Gray => {
            Fill::Solid(Rgba::opaque(BACKGROUND.r(), BACKGROUND.g(), BACKGROUND.b()))
        }
        CanvasBackground::White => Fill::Solid(WHITE),
    }
}
