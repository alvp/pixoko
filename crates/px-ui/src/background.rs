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
    /// Un color elegido por el usuario (`Color…`): siempre opaco (véase [`CanvasBackground::custom`]).
    Custom(Rgba),
}

impl CanvasBackground {
    /// El fondo de un color propio; `None` si el color no es opaco (el fondo va *debajo* de la
    /// transparencia).
    pub fn custom(color: Rgba) -> Option<CanvasBackground> {
        (color.a == 255).then_some(CanvasBackground::Custom(color))
    }

    /// El color del fondo si es liso (las tres sólidas y `Custom`); `None` con el damero.
    pub fn solid_color(self) -> Option<Rgba> {
        match background_fill(self) {
            Fill::Solid(color) => Some(color),
            Fill::Checker => None,
        }
    }
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
        CanvasBackground::Custom(c) => Fill::Solid(Rgba::opaque(c.r, c.g, c.b)),
    }
}

/// El valor de un color propio como se ve en el menú (`#rrggbb`); `None` con los demás fondos.
pub fn background_value_text(background: CanvasBackground) -> Option<String> {
    match background {
        CanvasBackground::Custom(c) => Some(Rgba::opaque(c.r, c.g, c.b).to_string()),
        _ => None,
    }
}
