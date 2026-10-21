//! Botón cuadrado con relieve para los iconos (specs/interfaz-iconos.md, decisión 3).
//!
//! 24 u = 1 u de relieve + 3 u de margen + 16 u de icono + 3 + 1. El aspecto sale de tres banderas
//! (`hundido`, `habilitado`, `ratón encima`) en [`look`], sin egui, y [`paint_icon`] y
//! [`paint_text`] lo pintan. Todo cae en píxeles físicos enteros: las medidas son `u` y el
//! icono se pinta a `16 · n` píxeles, con el UV completo (la textura se muestrea con vecino más
//! cercano).

use egui::{Align2, Color32, FontId, Painter, Rect, TextureId, pos2, vec2};

use crate::theme::{
    ACCENT, BEVEL_DARK, BEVEL_LIGHT, SURFACE_0, SURFACE_2, SURFACE_3, TEXT, TEXT_DISABLED,
};

/// Lado del botón, relieve, margen entre el relieve y el icono, icono, hueco entre botones y paso,
/// en `u`.
pub const BUTTON_U: i64 = 24;
pub const BEVEL_U: i64 = 1;
pub const PAD_U: i64 = 3;
pub const ICON_U: i64 = 16;
pub const GAP_U: i64 = 1;
pub const PITCH_U: i64 = BUTTON_U + GAP_U;

/// El aspecto de un botón en un estado.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Look {
    /// Relleno de la cara.
    pub face: Color32,
    /// Relieve arriba y a la izquierda.
    pub top_left: Color32,
    /// Relieve abajo y a la derecha.
    pub bottom_right: Color32,
    /// Tinta del icono (o del texto).
    pub tint: Color32,
    /// Desplazamiento del icono, en `u`, hacia abajo y a la derecha: 1 si está hundido.
    pub offset_u: i64,
}

/// Aspecto de un botón. Prioridad de la cara: hundido > ratón encima (solo si está habilitado) >
/// normal. Tinta: desactivado > hundido (`ACCENT`) > normal. Hundido invierte el relieve y baja
/// el icono 1 u, así que no depende solo del color.
pub fn look(sunken: bool, enabled: bool, hovered: bool) -> Look {
    let face = if sunken {
        SURFACE_0
    } else if enabled && hovered {
        SURFACE_3
    } else {
        SURFACE_2
    };
    let (top_left, bottom_right) = if sunken {
        (BEVEL_DARK, BEVEL_LIGHT)
    } else {
        (BEVEL_LIGHT, BEVEL_DARK)
    };
    let tint = if !enabled {
        TEXT_DISABLED
    } else if sunken {
        ACCENT
    } else {
        TEXT
    };
    Look {
        face,
        top_left,
        bottom_right,
        tint,
        offset_u: i64::from(sunken),
    }
}

/// Pinta la cara y el relieve en `rect` (el botón entero); `u` es el tamaño de un píxel de
/// interfaz en puntos. El orden fija las esquinas: el relieve de abajo y de la derecha se pinta
/// encima del de arriba y de la izquierda.
fn paint_frame(painter: &Painter, rect: Rect, u: f32, look: &Look) {
    let bevel = BEVEL_U as f32 * u;
    painter.rect_filled(rect, 0.0, look.face);
    painter.rect_filled(
        Rect::from_min_size(rect.min, vec2(rect.width(), bevel)),
        0.0,
        look.top_left,
    );
    painter.rect_filled(
        Rect::from_min_size(rect.min, vec2(bevel, rect.height())),
        0.0,
        look.top_left,
    );
    painter.rect_filled(
        Rect::from_min_size(
            pos2(rect.min.x, rect.max.y - bevel),
            vec2(rect.width(), bevel),
        ),
        0.0,
        look.bottom_right,
    );
    painter.rect_filled(
        Rect::from_min_size(
            pos2(rect.max.x - bevel, rect.min.y),
            vec2(bevel, rect.height()),
        ),
        0.0,
        look.bottom_right,
    );
}

/// Pinta un botón de icono: cara, relieve y, si hay textura, el icono de `16 u` a `4 u` del borde
/// (más el desplazamiento del aspecto), tintado, con el UV completo.
pub fn paint_icon(painter: &Painter, rect: Rect, u: f32, texture: Option<TextureId>, look: &Look) {
    paint_frame(painter, rect, u, look);
    if let Some(texture) = texture {
        let offset = (BEVEL_U + PAD_U + look.offset_u) as f32 * u;
        let icon = Rect::from_min_size(
            rect.min + vec2(offset, offset),
            vec2(ICON_U as f32 * u, ICON_U as f32 * u),
        );
        painter.image(
            texture,
            icon,
            Rect::from_min_max(pos2(0.0, 0.0), pos2(1.0, 1.0)),
            look.tint,
        );
    }
}

/// Pinta un botón de texto (`Confirmar` y `Cancelar`): cara, relieve y el rótulo a `text_pos`,
/// con la tinta del aspecto.
pub fn paint_text(
    painter: &Painter,
    rect: Rect,
    u: f32,
    text_pos: egui::Pos2,
    label: &str,
    font: &FontId,
    look: &Look,
) {
    paint_frame(painter, rect, u, look);
    painter.text(text_pos, Align2::LEFT_TOP, label, font.clone(), look.tint);
}
