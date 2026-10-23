//! Puntos y rectángulos de las zonas de la ventana sin escribir de qué lado está cada una
//! (specs/interfaz-caja-derecha.md, «Cómo no romper 50 pruebas con el próximo cambio de lado»).
//!
//! Dónde queda una zona cuando un banco monta **solo** esa pieza (el panel o la caja, sin las
//! demás, con las 16 columnas de hoy): pegada al borde derecho de la ventana. El reparto de la
//! columna única de verdad lo prueba `window_stack.rs` con `chrome::window_layout`.

#![allow(dead_code)]

use px_ui::chrome::{COLUMN_DEFAULT_COLS, MENU_H_U, column_width_u, head_u};
use px_ui::status::bar_height_px;
use px_ui::view::ScreenRect;

/// Punto `(x_u, y_u)` de una zona, en píxeles físicos, medido desde su esquina.
pub fn in_zone(zone: ScreenRect, x_u: i64, y_u: i64, n: i64) -> (i64, i64) {
    (zone.x + x_u * n, zone.y + y_u * n)
}

/// El alto de la franja central de una ventana de `window_px` (entre el menú y la barra de estado).
fn band_of(window_px: (i64, i64), n: u32) -> i64 {
    (window_px.1 - MENU_H_U * i64::from(n) - i64::from(bar_height_px(n))).max(0)
}

/// El panel de paletas en un banco que no monta la caja de herramientas: de 16 columnas (233 u),
/// pegado al borde derecho y de todo el alto de la franja central.
pub fn panel_only(window_px: (i64, i64), n: u32) -> ScreenRect {
    let k = i64::from(n);
    let width = column_width_u(COLUMN_DEFAULT_COLS) * k;
    ScreenRect {
        x: window_px.0 - width,
        y: MENU_H_U * k,
        width,
        height: band_of(window_px, n),
    }
}

/// La caja de herramientas en un banco que no monta el panel: de 16 columnas (233 u), pegada al
/// borde derecho, de `head` de alto (o menos, si la franja es más baja).
pub fn toolbox_single(window_px: (i64, i64), n: u32) -> ScreenRect {
    let k = i64::from(n);
    let width = column_width_u(COLUMN_DEFAULT_COLS) * k;
    ScreenRect {
        x: window_px.0 - width,
        y: MENU_H_U * k,
        width,
        height: (head_u(5, COLUMN_DEFAULT_COLS) * k).min(band_of(window_px, n)),
    }
}

/// Monta un panel de paletas (`panel`) y, en lo que queda, el resto del banco (`rest_2`: el lienzo)
/// con el panel **en una ranura de su ancho pegada al borde izquierdo** del espacio libre de `ui`,
/// sea cual sea el lado en que lo ponga la ventana. Los bancos que prueban el interior del panel
/// (muestras, deslizadores, botones, rueda) miden sus posiciones desde el borde izquierdo de la
/// ventana y no tienen por qué saber de qué lado va el panel real: eso lo prueba `window_stack.rs`
/// y la propiedad del ancho en `palette_panel.rs`.
pub fn panel_on_the_left<A, B>(
    ui: &mut egui::Ui,
    panel: impl FnOnce(&mut egui::Ui) -> A,
    rest_2: impl FnOnce(&mut egui::Ui) -> B,
) -> (A, B) {
    let u = px_ui::text::ui_unit(ui.ctx().pixels_per_point());
    let free = ui.available_rect_before_wrap();
    // El corte se redondea como egui redondea el borde de un panel real (si no, marca como
    // «no alineado» el borde del `Ui` y lo pinta aparte, fuera del recorte del panel).
    use egui::emath::GuiRounding as _;
    let cut = (free.min.x + px_ui::palette_grid::PANEL_WIDTH_U as f32 * u).round_ui();
    let slot = egui::Rect::from_min_max(free.min, egui::pos2(cut, free.max.y));
    let surplus = egui::Rect::from_min_max(egui::pos2(cut, free.min.y), free.max);
    let mut left = ui.new_child(egui::UiBuilder::new().max_rect(slot));
    // El `Ui` de la ranura se suelta dentro del fotograma y egui, en depuración, pintaría una
    // marca «Unaligned» con el borde del panel (233 u no es un múltiplo de 1/32 pt a todo `ppp`);
    // el `Ui` raíz de la app se suelta fuera y no la deja en lo pintado.
    left.style_mut().debug.show_unaligned = false;
    let a = panel(&mut left);
    let mut right = ui.new_child(egui::UiBuilder::new().max_rect(surplus));
    let b = rest_2(&mut right);
    (a, b)
}
