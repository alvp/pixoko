//! The window of options painted with egui (specs/ui-single-column.md, decisions 8, 10 and 11): an
//! `egui::Area` of exactly the rectangle of the window, as the layers panel is, so outside it
//! everything reaches the canvas and the column. It does not touch the document: it returns
//! [`OptionsEvent`]. The geometry, the state and the texts are in `options` (without egui).

use egui::{
    Align2, Area, Context, Event, FontFamily, FontId, Id, MouseWheelUnit, Order, Pos2, Rect, Sense,
    Stroke, StrokeKind, Ui, vec2,
};

use crate::actions::Platform;
use crate::icon_button::{look, paint_text};
use crate::keymap::Keymap;
use crate::options::{
    BODY_MAX_ROWS, BODY_Y_U, CLOSE_X_U, HELP_LINE_H_U, KEY_Y_U, NO_OPTIONS, OptionsWindow, ROW_H_U,
    SCROLL_ROWS_PER_NOTCH, SCROLLBAR_W_U, SCROLLBAR_X_U, TEXT_X_U, TITLE_BAR_H_U, TITLE_TEXT_Y_U,
    WINDOW_W_U, body_rows, brief_help, help_y_u, key_line, options_max_scroll, title,
    window_height_u, wrap_help,
};
use crate::text::{font_size_points, printable, text_scale, ui_unit};
use crate::theme::{SURFACE_0, SURFACE_1, SURFACE_3, TEXT, TEXT_DIM};
use crate::tool::Tool;

/// What the window asks of whoever shows it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum OptionsEvent {
    /// The `x` of the title bar.
    Close,
}

/// What the window needs to be painted.
pub struct OptionsView<'a> {
    /// The tool whose options it shows: the active one.
    pub tool: Tool,
    /// Rows of options of the tool ([`crate::options::options_row_count`]; a test passes any number).
    pub rows: usize,
    pub keymap: &'a Keymap,
    pub platform: Platform,
    /// `false` with a modal dialog (or a conversion or a save in wait) open: the window is painted
    /// but responds to nothing.
    pub enabled: bool,
}

/// Paints the window (if it is open) and returns what happened in it in this frame. `window_px` is
/// the size of the window of the app in physical pixels and `wanted_cols` the columns of swatches that
/// were chosen (the first position depends on the width of the canvas).
pub fn show_options_window(
    ctx: &Context,
    view: &OptionsView,
    state: &mut OptionsWindow,
    window_px: (i64, i64),
    wanted_cols: i64,
) -> Vec<OptionsEvent> {
    let mut events = Vec::new();
    if !state.open {
        return events;
    }
    let ppp = ctx.pixels_per_point();
    let n = text_scale(ppp);
    let u = ui_unit(ppp);
    let rect_px = state.rect(view.rows, window_px, n, wanted_cols);
    let origin = Pos2::new(rect_px.x as f32 / ppp, rect_px.y as f32 / ppp);
    let size = vec2(WINDOW_W_U as f32 * u, window_height_u(view.rows) as f32 * u);
    // The scroll never passes the end of the body.
    state.scroll_by(view.tool, 0, view.rows);
    Area::new(Id::unique("pixoko-options-window"))
        .order(Order::Middle)
        .fixed_pos(origin)
        .interactable(view.enabled)
        .constrain(false)
        .fade_in(false)
        .show(ctx, |ui| {
            ui.allocate_exact_size(size, Sense::hover());
            draw_window(ui, ctx, view, state, origin, u, &mut events);
        });
    events
}

fn draw_window(
    ui: &mut Ui,
    ctx: &Context,
    view: &OptionsView,
    state: &mut OptionsWindow,
    origin: Pos2,
    u: f32,
    events: &mut Vec<OptionsEvent>,
) {
    let ppp = ctx.pixels_per_point();
    let n = text_scale(ppp);
    let font = FontId::new(font_size_points(ppp), FontFamily::Proportional);
    let painter = ui.painter().clone();
    let base = Id::unique("pixoko-options-window-widgets");
    let rect = |x: i64, y: i64, w: i64, h: i64| {
        Rect::from_min_size(
            origin + vec2(x as f32 * u, y as f32 * u),
            vec2(w as f32 * u, h as f32 * u),
        )
    };
    let tool = view.tool;
    let rows = view.rows;
    let text = |pos: Pos2, s: &str, color| {
        painter.text(
            pos,
            Align2::LEFT_TOP,
            printable(ctx, s),
            font.clone(),
            color,
        );
    };

    painter.rect(
        rect(0, 0, WINDOW_W_U, window_height_u(rows)),
        0.0,
        SURFACE_1,
        Stroke::new(u, SURFACE_3),
        StrokeKind::Inside,
    );

    // Title bar: the name of the tool; dragging it moves the window.
    let title_bar = rect(1, 1, CLOSE_X_U - 1, TITLE_BAR_H_U);
    text(
        origin + vec2(TEXT_X_U as f32 * u, TITLE_TEXT_Y_U as f32 * u),
        title(tool),
        TEXT,
    );
    let sense = |sense: Sense| if view.enabled { sense } else { Sense::hover() };
    let drag = ui.interact(title_bar, base.with("title"), sense(Sense::drag()));
    if drag.drag_started() {
        state.drag_from = state.pos_u;
    }
    if drag.dragged()
        && let Some(from) = state.drag_from
        && let (Some(press), Some(now)) =
            ctx.input(|i| (i.pointer.press_origin(), i.pointer.interact_pos()))
    {
        let dx = ((now.x - press.x) * ppp).round() as i64;
        let dy = ((now.y - press.y) * ppp).round() as i64;
        state.drag_total(from, dx, dy, n);
    }
    if drag.drag_stopped() {
        state.drag_from = None;
    }
    // The `x`.
    let close = rect(CLOSE_X_U, 1, TITLE_BAR_H_U, TITLE_BAR_H_U);
    let close_response = ui.interact(close, base.with("close"), sense(Sense::click()));
    paint_text(
        &painter,
        close,
        u,
        close.min + vec2(5.0 * u, 1.0 * u),
        "x",
        &font,
        &look(false, view.enabled, close_response.hovered()),
    );
    if view.enabled && close_response.clicked() {
        events.push(OptionsEvent::Close);
    }

    // The line of the shortcut.
    text(
        origin + vec2(TEXT_X_U as f32 * u, KEY_Y_U as f32 * u),
        &key_line(tool, view.keymap, view.platform),
        TEXT_DIM,
    );

    // The body, clipped to its rectangle: `sin opciones` or, with more rows than fit, the rows from the
    // scroll of the tool.
    let shown = body_rows(rows);
    let body = rect(1, BODY_Y_U, WINDOW_W_U - 2, ROW_H_U * shown as i64);
    if rows == 0 {
        painter
            .with_clip_rect(body.intersect(painter.clip_rect()))
            .text(
                origin + vec2(TEXT_X_U as f32 * u, BODY_Y_U as f32 * u),
                Align2::LEFT_TOP,
                NO_OPTIONS,
                font.clone(),
                TEXT_DIM,
            );
    }
    if rows > BODY_MAX_ROWS {
        let track = rect(
            SCROLLBAR_X_U,
            BODY_Y_U,
            SCROLLBAR_W_U,
            ROW_H_U * BODY_MAX_ROWS as i64,
        );
        painter.rect_filled(track, 0.0, SURFACE_0);
        let max = options_max_scroll(rows);
        let thumb_h = (track.height() * BODY_MAX_ROWS as f32 / rows as f32).max(8.0 * u);
        let free = track.height() - thumb_h;
        let top = track.min.y + free * state.scroll_of(tool) as f32 / max as f32;
        painter.rect_filled(
            Rect::from_min_size(Pos2::new(track.min.x, top), vec2(track.width(), thumb_h)),
            0.0,
            SURFACE_3,
        );
    }
    if view.enabled {
        let row_h = ROW_H_U as f32 * u;
        let mut delta_rows = 0i64;
        ctx.input(|input| {
            let hover = input.pointer.hover_pos();
            for event in &input.events {
                if let Event::MouseWheel { unit, delta, .. } = event
                    && hover.is_some_and(|p| body.contains(p))
                {
                    let moved = match unit {
                        MouseWheelUnit::Line | MouseWheelUnit::Page => {
                            -delta.y * SCROLL_ROWS_PER_NOTCH as f32
                        }
                        MouseWheelUnit::Point => -delta.y / row_h,
                    };
                    delta_rows += moved.round() as i64;
                }
            }
        });
        if delta_rows != 0 {
            state.scroll_by(tool, delta_rows, rows);
        }
    }

    // The brief help under the body.
    let y = help_y_u(rows);
    for (i, line) in wrap_help(&brief_help(tool))
        .iter()
        .take(crate::options::HELP_LINES)
        .enumerate()
    {
        text(
            origin
                + vec2(
                    TEXT_X_U as f32 * u,
                    (y + HELP_LINE_H_U * i as i64) as f32 * u,
                ),
            line,
            TEXT_DIM,
        );
    }
}
