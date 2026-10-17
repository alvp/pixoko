//! Panel de paletas: rejilla de muestras para ver y elegir el color activo
//! (specs/hito-2-panel-paletas.md). No modifica el documento.
//!
//! La geometría y las constantes viven en [`crate::palette_grid`]; aquí solo se traducen a
//! puntos de egui (`u` = `ui_unit(ppp)` puntos = `n` píxeles físicos) y se leen los eventos.

use egui::{
    Align2, Color32, Event, FontFamily, FontId, Frame, MouseWheelUnit, Panel, PointerButton, Pos2,
    Rect, Sense, Ui, vec2,
};
use px_core::{Image, Rgba};

use crate::color::{
    ColorChoice, cell_color, cell_count, cell_label, cell_of_choice, choice_for_cell, info_flags,
    is_transparent_cell,
};
use crate::palette_grid::{
    GRID_X_U, GRID_Y_U, Grid, LINE_H_U, LINE1_Y_U, LINE2_Y_U, PANEL_WIDTH_U, PITCH_U,
    RIGHT_LINE_X_U, RING_ACTIVE_U, RING_HOVER_U, SEPARATOR_Y_U, Scroll, TEXT_WIDTH_U, TEXT_X_U,
    TRACK_W_U, TRACK_X_U, ring_rects, swatch_parts, thumb, total_rows, visible_rows,
};
use crate::status::{bar_text, columns};
use crate::text::{font_size_points, text_scale, ui_unit};
use crate::theme::{RING_ACTIVE, RING_HOVER, SURFACE_1, SURFACE_2, SURFACE_3, TEXT, TEXT_DIM};
use crate::view::ScreenRect;

/// Filas que desplaza una «línea» de la rueda.
const WHEEL_LINE_ROWS: f32 = 3.0;

/// Datos de un fotograma: escala y origen de la rejilla en el panel.
struct Layout {
    ppp: f32,
    n: u32,
    /// El panel, en puntos.
    rect: Rect,
    /// Esquina superior izquierda del panel, en puntos.
    min: Pos2,
    /// Alto del panel en píxeles físicos.
    height_px: i64,
}

impl Layout {
    /// Un punto de egui sobre el panel, en píxeles físicos desde el origen de la rejilla.
    fn grid_px(&self, p: Pos2) -> (i64, i64) {
        let u = ui_unit(self.ppp);
        let origin = self.min + vec2(GRID_X_U as f32 * u, GRID_Y_U as f32 * u);
        (
            ((p.x - origin.x) * self.ppp).round() as i64,
            ((p.y - origin.y) * self.ppp).round() as i64,
        )
    }

    /// Un rectángulo en píxeles físicos desde el origen de la rejilla, en puntos de egui.
    fn grid_rect(&self, r: ScreenRect) -> Rect {
        let u = ui_unit(self.ppp);
        let origin = self.min + vec2(GRID_X_U as f32 * u, GRID_Y_U as f32 * u);
        Rect::from_min_size(
            origin + vec2(r.x as f32 / self.ppp, r.y as f32 / self.ppp),
            vec2(r.width as f32 / self.ppp, r.height as f32 / self.ppp),
        )
    }

    /// Un rectángulo en `u` desde la esquina del panel, en puntos de egui.
    fn u_rect(&self, x: i64, y: i64, width: i64, height: i64) -> Rect {
        let u = ui_unit(self.ppp);
        Rect::from_min_size(
            self.min + vec2(x as f32 * u, y as f32 * u),
            vec2(width as f32 * u, height as f32 * u),
        )
    }
}

/// Pinta el panel a la izquierda de `ui` y devuelve la elección que pidió un clic (botón
/// izquierdo pulsado sobre una muestra). `enabled` falso (gesto en curso): se ve y se
/// desplaza, pero no elige. Hay que llamarla **después** de la barra de estado y **antes** del
/// lienzo, que se queda con el rectángulo que sobra.
pub fn show(
    ui: &mut Ui,
    image: &Image,
    choice: ColorChoice,
    scroll: &mut Scroll,
    enabled: bool,
) -> Option<ColorChoice> {
    let ctx = ui.ctx().clone();
    let ppp = ctx.pixels_per_point();
    let u = ui_unit(ppp);
    let width = PANEL_WIDTH_U as f32 * u;
    Panel::left("paleta")
        .exact_size(width)
        .resizable(false)
        .show_separator_line(false)
        .frame(Frame::new().fill(SURFACE_1))
        .show(ui, |ui| {
            let (rect, _) = ui.allocate_exact_size(ui.available_size(), Sense::hover());
            let layout = Layout {
                ppp,
                n: text_scale(ppp),
                rect,
                min: rect.min,
                height_px: (rect.height() * ppp).round() as i64,
            };
            let count = cell_count(image);
            let total = total_rows(count);
            let visible = visible_rows(layout.height_px, layout.n);
            scroll.clamp(total, visible);

            let pointer = ui
                .input(|i| i.pointer.hover_pos())
                .filter(|p| rect.contains(*p));
            let events = ui.input(|i| i.events.clone());
            let mut picked = None;
            for event in &events {
                match event {
                    Event::MouseWheel {
                        unit,
                        delta,
                        modifiers,
                        ..
                    } if !modifiers.command && delta.y != 0.0 && pointer.is_some() => {
                        let pitch_px = (PITCH_U * i64::from(layout.n)) as f32;
                        let down = -delta.y;
                        let pixels = match unit {
                            MouseWheelUnit::Point => down * ppp,
                            MouseWheelUnit::Line => down * WHEEL_LINE_ROWS * pitch_px,
                            MouseWheelUnit::Page => down * visible as f32 * pitch_px,
                        };
                        scroll.by(pixels.round() as i64, layout.n, total, visible);
                    }
                    Event::PointerButton {
                        pos,
                        button: PointerButton::Primary,
                        pressed: true,
                        ..
                    } if enabled && rect.contains(*pos) => {
                        let grid = Grid::new(layout.n, count, visible, scroll.first_row);
                        let (x, y) = layout.grid_px(*pos);
                        if let Some(cell) = grid.index_at(x, y) {
                            picked = choice_for_cell(image, cell).or(picked);
                        }
                    }
                    _ => {}
                }
            }

            let grid = Grid::new(layout.n, count, visible, scroll.first_row);
            let hover = pointer.and_then(|p| {
                let (x, y) = layout.grid_px(p);
                grid.index_at(x, y)
            });
            paint(
                ui, &layout, image, choice, &grid, hover, scroll, total, visible,
            );
            picked
        })
        .inner
}

#[allow(clippy::too_many_arguments)]
fn paint(
    ui: &Ui,
    layout: &Layout,
    image: &Image,
    choice: ColorChoice,
    grid: &Grid,
    hover: Option<usize>,
    scroll: &Scroll,
    total: usize,
    visible: usize,
) {
    let ctx = ui.ctx();
    let (n, ppp) = (layout.n, layout.ppp);
    let n_px = i64::from(n);
    let painter = ui.painter_at(layout.rect);
    let active = cell_of_choice(image, choice);

    // Trazos: bajo el texto y a la derecha.
    painter.rect_filled(
        layout.u_rect(0, SEPARATOR_Y_U, PANEL_WIDTH_U, 1),
        0.0,
        SURFACE_3,
    );
    let line = layout.u_rect(RIGHT_LINE_X_U, 0, 1, 0);
    painter.rect_filled(
        Rect::from_min_size(line.min, vec2(line.width(), layout.rect.height())),
        0.0,
        SURFACE_3,
    );

    // Información: la muestra bajo el cursor o, si no hay, la elegida.
    let shown = hover.or(active);
    let line1 = shown
        .and_then(|cell| cell_label(image, cell))
        .unwrap_or_default();
    let line2 = shown
        .map(|cell| {
            info_flags(
                hover.is_some(),
                Some(cell) == active,
                is_transparent_cell(image, cell),
            )
        })
        .unwrap_or_default();
    let cols = columns((TEXT_WIDTH_U * n_px) as f32, ppp);
    let font = FontId::new(font_size_points(ppp), FontFamily::Proportional);
    for (y, text, color) in [(LINE1_Y_U, line1, TEXT), (LINE2_Y_U, line2, TEXT_DIM)] {
        let text = bar_text(ctx, &text, false, cols);
        let top_left = layout.u_rect(TEXT_X_U, y, 0, LINE_H_U).min;
        painter.text(top_left, Align2::LEFT_TOP, text, font.clone(), color);
    }

    // Muestras visibles.
    for index in 0..cell_count(image) {
        let Some(cell) = grid.cell_rect(index) else {
            continue;
        };
        let color = cell_color(image, index).unwrap_or(Rgba::TRANSPARENT);
        for (part, tone) in swatch_parts(n, color) {
            let part = ScreenRect {
                x: cell.x + part.x,
                y: cell.y + part.y,
                ..part
            };
            painter.rect_filled(layout.grid_rect(part), 0.0, to_color32(tone));
        }
    }

    // Anillos: el del cursor (si no es la elegida) y el de la elegida.
    let rings = [
        (
            hover.filter(|cell| Some(*cell) != active),
            RING_HOVER_U,
            RING_HOVER,
        ),
        (active, RING_ACTIVE_U, RING_ACTIVE),
    ];
    for (cell, thickness, color) in rings {
        let Some(rect) = cell.and_then(|cell| grid.cell_rect(cell)) else {
            continue;
        };
        for ring in ring_rects(rect, thickness * n_px) {
            painter.rect_filled(layout.grid_rect(ring), 0.0, color);
        }
    }

    // Pista e indicador, solo si hace falta desplazar.
    if let Some((y, height)) = thumb(scroll.first_row, visible, total, n) {
        let track_h = visible as i64 * PITCH_U;
        painter.rect_filled(
            layout.u_rect(TRACK_X_U, GRID_Y_U, TRACK_W_U, track_h),
            0.0,
            SURFACE_2,
        );
        let top = layout.u_rect(TRACK_X_U, GRID_Y_U, TRACK_W_U, 0).min;
        let bar = Rect::from_min_size(
            top + vec2(0.0, y as f32 / ppp),
            vec2(TRACK_W_U as f32 * ui_unit(ppp), height as f32 / ppp),
        );
        painter.rect_filled(bar, 0.0, TEXT_DIM);
    }
}

fn to_color32(color: Rgba) -> Color32 {
    Color32::from_rgba_unmultiplied(color.r, color.g, color.b, color.a)
}
