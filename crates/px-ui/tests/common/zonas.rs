//! Puntos y rectángulos de las zonas de la ventana sin escribir de qué lado está cada una
//! (specs/interfaz-caja-derecha.md, «Cómo no romper 50 pruebas con el próximo cambio de lado»).
//!
//! `window_layout` es la única fuente del reparto; aquí solo se deduce, **comparando `x`** con la del
//! lienzo, dónde queda una zona cuando un banco monta **solo** esa pieza (el panel o la caja, sin
//! las demás): pegada al borde de su lado de la ventana.

#![allow(dead_code)]

use px_ui::chrome::{WindowLayout, window_layout};
use px_ui::view::ScreenRect;

/// Punto `(x_u, y_u)` de una zona, en píxeles físicos, medido desde su esquina.
pub fn en_zona(zona: ScreenRect, x_u: i64, y_u: i64, n: i64) -> (i64, i64) {
    (zona.x + x_u * n, zona.y + y_u * n)
}

/// La zona pegada al borde de su lado de una ventana de `ventana_px` en la que solo se monta ella.
fn pegada_al_borde(zona: ScreenRect, layout: &WindowLayout, ventana_px: (i64, i64)) -> ScreenRect {
    let x = if zona.x > layout.canvas.x {
        ventana_px.0 - zona.width
    } else {
        0
    };
    ScreenRect { x, ..zona }
}

/// El panel de paletas en un banco que no monta la caja de herramientas.
pub fn panel_solo(ventana_px: (i64, i64), n: u32) -> ScreenRect {
    let layout = window_layout(ventana_px, n);
    pegada_al_borde(layout.panel, &layout, ventana_px)
}

/// La caja de herramientas en un banco que no monta el panel.
pub fn caja_sola(ventana_px: (i64, i64), n: u32) -> ScreenRect {
    let layout = window_layout(ventana_px, n);
    pegada_al_borde(layout.toolbar, &layout, ventana_px)
}

/// Monta un panel de paletas (`panel`) y, en lo que queda, el resto del banco (`resto`: el lienzo)
/// con el panel **en una ranura de su ancho pegada al borde izquierdo** del espacio libre de `ui`,
/// sea cual sea el lado en que lo ponga la ventana. Los bancos que prueban el interior del panel
/// (muestras, deslizadores, botones, rueda) miden sus posiciones desde el borde izquierdo de la
/// ventana y no tienen por qué saber de qué lado va el panel real: eso lo prueba `window_stack.rs`
/// y la propiedad del ancho en `palette_panel.rs`.
pub fn panel_a_la_izquierda<A, B>(
    ui: &mut egui::Ui,
    panel: impl FnOnce(&mut egui::Ui) -> A,
    resto: impl FnOnce(&mut egui::Ui) -> B,
) -> (A, B) {
    let u = px_ui::text::ui_unit(ui.ctx().pixels_per_point());
    let libre = ui.available_rect_before_wrap();
    // El corte se redondea como egui redondea el borde de un panel real (si no, marca como
    // «no alineado» el borde del `Ui` y lo pinta aparte, fuera del recorte del panel).
    use egui::emath::GuiRounding as _;
    let corte = (libre.min.x + px_ui::palette_grid::PANEL_WIDTH_U as f32 * u).round_ui();
    let ranura = egui::Rect::from_min_max(libre.min, egui::pos2(corte, libre.max.y));
    let sobra = egui::Rect::from_min_max(egui::pos2(corte, libre.min.y), libre.max);
    let mut izquierda = ui.new_child(egui::UiBuilder::new().max_rect(ranura));
    // El `Ui` de la ranura se suelta dentro del fotograma y egui, en depuración, pintaría una
    // marca «Unaligned» con el borde del panel (233 u no es un múltiplo de 1/32 pt a todo `ppp`);
    // el `Ui` raíz de la app se suelta fuera y no la deja en lo pintado.
    izquierda.style_mut().debug.show_unaligned = false;
    let a = panel(&mut izquierda);
    let mut derecha = ui.new_child(egui::UiBuilder::new().max_rect(sobra));
    let b = resto(&mut derecha);
    (a, b)
}
