//! Color editor inside the palette panel: the sliders (specs/hito-2-editar-color.md), and the tail
//! under the grid: the color bar, the hexadecimal field and the icon row (specs/ui-palette-handle.md).
//! The logic lives in [`crate::color_edit`] and [`crate::color_bar`] and the geometry in
//! [`crate::edit_layout`]; here they are only translated to egui.
//!
//! Everything falls on whole physical pixels: the measures are `u` (`n` physical pixels) from `top_px`,
//! which is a multiple of `n`, and the positions inside a track come from `slider_x`.

use egui::{
    Align2, Color32, Event, FontFamily, FontId, Frame, Id, Key, Label, Margin, PointerButton, Pos2,
    Rect, RichText, Role, Sense, Stroke, StrokeKind, TextEdit, Ui, WidgetInfo, vec2,
};
use px_core::{Mode, Rgba};

use crate::color_bar::{BarInk, bar_ink, bar_label};
use crate::color_edit::{
    ColorEditor, Commit, EditTarget, HexOutcome, Resolved, control_value, controls, hex_notice,
    hex_text, slider_value_at, slider_x, stale_notice, value_text,
};
use crate::edit_layout::{
    BAR_H_U, BAR_X_U, BAR_Y_U, BUTTON_TEXT_X_U, BUTTON_TEXT_Y_U, FIELD_H_U, FIELD_PAD_U, FIELD_W_U,
    FIELD_X_U, FIELD_Y_U, LABEL_X_U, PaletteButton, ROW_H_U, SEPARATOR_Y_U, THUMB_H_U, THUMB_W_U,
    TRACK_H_U, TRACK_W_U, TRACK_X_U, VALUE_X_U, button_rect_u, control_row_u, palette_icon,
    slider_hit,
};
use crate::icon_button::{look, paint_icon, paint_text};
use crate::icons::IconSet;
use crate::palette_grid::{TEXT_WIDTH_U, block_parts, panel_width_u, text_width_u};
use crate::palette_ops::{blocked_reason, palette_button_tooltip};
use crate::status::{bar_text, columns};
use crate::text::{font_size_points, printable, ui_unit};
use crate::theme::{ACCENT, SURFACE_0, SURFACE_3, TEXT, TEXT_DIM, WARNING};

/// Where the editor is: scale, corner of the panel and `top_px` from its top edge (negative
/// if the panel scrolled). What is painted and the clicks are limited to `panel_rect`.
#[derive(Clone, Copy, Debug)]
pub(crate) struct Geometry {
    pub ppp: f32,
    pub n: u32,
    pub panel_min: Pos2,
    pub panel_rect: Rect,
    pub top_px: i64,
    pub mode: Mode,
    /// Columns of swatches of the column: the width of the bar.
    pub cols: i64,
}

impl Geometry {
    fn n_px(&self) -> i64 {
        i64::from(self.n)
    }

    /// Physical pixels to points.
    fn pt(&self, px: i64) -> f32 {
        px as f32 / self.ppp
    }

    /// Corner of what is painted (the editor or the tail), in points.
    fn origin(&self) -> Pos2 {
        self.panel_min + vec2(0.0, self.pt(self.top_px))
    }

    /// Rectangle in `u` from the corner of the editor or the tail, in points.
    fn rect_u(&self, x: i64, y: i64, width: i64, height: i64) -> Rect {
        let n = self.n_px();
        Rect::from_min_size(
            self.origin() + vec2(self.pt(x * n), self.pt(y * n)),
            vec2(self.pt(width * n), self.pt(height * n)),
        )
    }

    /// An egui point, in physical pixels from the corner of the editor or the tail.
    pub(crate) fn rel_px(&self, pos: Pos2) -> (i64, i64) {
        (
            ((pos.x - self.panel_min.x) * self.ppp).round() as i64,
            ((pos.y - self.panel_min.y) * self.ppp).round() as i64 - self.top_px,
        )
    }

    /// Value of `control` under the `x` (relative to the editor) of a point.
    fn value_at(&self, x_px: i64, max: u32) -> u32 {
        slider_value_at(x_px - TRACK_X_U * self.n_px(), TRACK_W_U * self.n_px(), max)
    }
}

pub(crate) fn is_escape(event: &Event) -> bool {
    matches!(
        event,
        Event::Key { key: Key::Escape, pressed: true, modifiers, .. } if modifiers.is_none()
    )
}

/// Identifier of the widget of a piece of the editor (the square or one of the two bars), which
/// takes the keyboard focus (specs/ui-color-block.md, decision 18).
pub fn piece_id(_piece: crate::color_block::Piece) -> Id {
    todo!("C4: piece_id")
}

/// How many times the texture of the saturation and brightness square has been uploaded to `ctx`
/// (a counter for the tests, like `Canvas::preview_uploads`).
pub fn sv_uploads(_ctx: &egui::Context) -> u32 {
    todo!("C4: sv_uploads")
}

/// Identifier of the hexadecimal field (the same every frame).
pub fn hex_field_id() -> Id {
    Id::unique("pixoko-campo-hex")
}

fn is_enter(event: &Event) -> bool {
    matches!(
        event,
        Event::Key {
            key: Key::Enter,
            pressed: true,
            ..
        }
    )
}

/// Mouse and keyboard of the sliders, in the order of the events of the frame. Returns the
/// color to apply if a drag that changed the color was released. `primary_down` is what egui
/// believes about the primary button (safety net if a release was lost).
pub(crate) fn interact(
    events: &[Event],
    geo: &Geometry,
    editor: &mut ColorEditor,
    enabled: bool,
    primary_down: bool,
) -> Option<Commit> {
    let mut commit = None;
    for event in events {
        match event {
            Event::PointerButton {
                pos,
                button: PointerButton::Primary,
                pressed: true,
                ..
            } if enabled && !editor.dragging() && geo.panel_rect.contains(*pos) => {
                let (x, y) = geo.rel_px(*pos);
                if let Some(control) = slider_hit(geo.mode, x, y, geo.n) {
                    editor.begin(control, geo.value_at(x, control.max()));
                }
            }
            Event::PointerMoved(pos) => {
                if let Some(control) = editor.dragging_control() {
                    editor.drag(geo.value_at(geo.rel_px(*pos).0, control.max()));
                }
            }
            Event::PointerButton {
                pos,
                button: PointerButton::Primary,
                pressed: false,
                ..
            } => {
                if let Some(control) = editor.dragging_control() {
                    editor.drag(geo.value_at(geo.rel_px(*pos).0, control.max()));
                    commit = editor.release().or(commit);
                }
            }
            Event::WindowFocused(false) => editor.cancel(),
            event if is_escape(event) => editor.cancel(),
            _ => {}
        }
    }
    if editor.dragging() && !primary_down {
        commit = editor.release().or(commit);
    }
    commit
}

/// Which palette buttons to paint and how (specs/hito-2-paleta-operaciones.md).
#[derive(Clone, Copy, Debug)]
pub(crate) struct ButtonsView {
    /// There is a pending removal: `Confirmar` and `Cancel` (text) replace the icon row.
    pub removing: bool,
    /// Entries of the palette.
    pub len: usize,
    /// The pending removal has a destination.
    pub has_destination: bool,
    /// The chosen entry is the transparent one: the set or remove icon looks sunken and its
    /// tooltip says `Remove transparent`.
    pub transparent_active: bool,
    /// The textures of the icons.
    pub icons: IconSet,
}

/// What the tail asked for in a frame.
#[derive(Debug, Default)]
pub(crate) struct Output {
    pub commit: Option<Commit>,
    /// 5 s notice for the status bar (an invalid hexadecimal).
    pub notice: Option<String>,
}

/// Paints the editor: the separator and the sliders. It has no header, no field and no notices:
/// the bar, the field and the warnings are in the tail under the grid ([`show_tail`]).
pub(crate) fn show(ui: &mut Ui, geo: &Geometry, editor: &ColorEditor) {
    let Some((rgba, hsl)) = editor.shown() else {
        return;
    };
    let ctx = ui.ctx().clone();
    let mode = geo.mode;
    let n = geo.n_px();
    let cols = columns((TEXT_WIDTH_U * n) as f32, geo.ppp);
    let font = FontId::new(font_size_points(geo.ppp), FontFamily::Proportional);
    let painter = ui.painter_at(geo.panel_rect);
    let text = |x: i64, y: i64, content: &str, color| {
        let shown = bar_text(&ctx, content, false, cols);
        painter.text(
            geo.rect_u(x, y, 0, ROW_H_U).min,
            Align2::LEFT_TOP,
            shown,
            font.clone(),
            color,
        );
    };

    painter.rect_filled(
        geo.rect_u(0, SEPARATOR_Y_U, panel_width_u(geo.cols), 1),
        0.0,
        SURFACE_3,
    );

    for &control in controls(mode) {
        let Some(row) = control_row_u(mode, control) else {
            continue;
        };
        let value = control_value(rgba, hsl, control);
        text(LABEL_X_U, row, &control.label().to_string(), TEXT);
        let track = geo.rect_u(
            TRACK_X_U,
            row + (ROW_H_U - TRACK_H_U) / 2,
            TRACK_W_U,
            TRACK_H_U,
        );
        painter.rect_filled(track, 0.0, SURFACE_3);
        let at = slider_x(value, TRACK_W_U * n, control.max());
        let filled = Rect::from_min_size(track.min, vec2(geo.pt(at), track.height()));
        painter.rect_filled(filled, 0.0, TEXT_DIM);
        let thumb = Rect::from_min_size(
            geo.rect_u(TRACK_X_U, row + (ROW_H_U - THUMB_H_U) / 2, 0, THUMB_H_U)
                .min
                + vec2(geo.pt(at - n), 0.0),
            vec2(geo.pt(THUMB_W_U * n), geo.pt(THUMB_H_U * n)),
        );
        painter.rect_filled(thumb, 0.0, ACCENT);
        text(VALUE_X_U, row, &value_text(value), TEXT);
    }
}

fn to_color32(color: Rgba) -> Color32 {
    Color32::from_rgba_unmultiplied(color.r, color.g, color.b, color.a)
}

/// Paints the color bar and the hexadecimal field under the grid (`geo.top_px` is the top edge of
/// the tail) and attends the field. `transparent`: the target is the transparent entry of the
/// palette. `stored`: the stored color of a target, which is what the bar shows.
#[allow(clippy::too_many_arguments)]
pub(crate) fn show_tail(
    ui: &mut Ui,
    geo: &Geometry,
    editor: &mut ColorEditor,
    target: EditTarget,
    transparent: bool,
    events: &[Event],
    stored: &dyn Fn(EditTarget) -> Option<Rgba>,
) -> Output {
    let mut output = Output::default();
    let Some((rgba, _)) = editor.shown() else {
        return output;
    };
    let ctx = ui.ctx().clone();
    let mode = geo.mode;
    let n = geo.n_px();
    let u = ui_unit(geo.ppp);
    let font = FontId::new(font_size_points(geo.ppp), FontFamily::Proportional);
    let painter = ui.painter_at(geo.panel_rect);

    // The color bar: the stored color of what is edited and its text.
    let bar_columns = columns((text_width_u(geo.cols) * n) as f32, geo.ppp);
    let bar = geo.rect_u(BAR_X_U, BAR_Y_U, text_width_u(geo.cols), BAR_H_U);
    let color = stored(target).unwrap_or(rgba);
    let ink: BarInk = bar_ink(color);
    for (part, tone) in block_parts(text_width_u(geo.cols) * n, BAR_H_U * n, 4 * n, color) {
        let rect = Rect::from_min_size(
            bar.min + vec2(geo.pt(part.x), geo.pt(part.y)),
            vec2(geo.pt(part.width), geo.pt(part.height)),
        );
        painter.rect_filled(rect, 0.0, to_color32(tone));
    }
    let label = bar_text(&ctx, &bar_label(target, transparent), false, bar_columns);
    let text_origin = geo
        .rect_u(BAR_X_U + 3, BAR_Y_U + (BAR_H_U - ROW_H_U) / 2, 0, ROW_H_U)
        .min;
    if ink.boxed {
        // With alpha the text goes in an opaque box over the checker.
        let width = label.chars().count() as i64 * 7 + 2;
        painter.rect_filled(
            geo.rect_u(BAR_X_U + 2, BAR_Y_U + 1, width, BAR_H_U - 2),
            0.0,
            ink.behind,
        );
    }
    painter.text(text_origin, Align2::LEFT_TOP, label, font.clone(), ink.text);

    // Hexadecimal field: background and border are ours; the `TextEdit` goes inside without a frame,
    // because the margins of egui are whole points and do not fall on whole physical pixels at 1.5×.
    let id = hex_field_id();
    let focused = ui.memory(|memory| memory.has_focus(id));
    let invalid = editor.hex_error().is_some();
    let (edge, edge_u) = if invalid {
        (WARNING, 2.0)
    } else if focused {
        (ACCENT, 1.0)
    } else {
        (SURFACE_3, 1.0)
    };
    painter.rect(
        geo.rect_u(FIELD_X_U, FIELD_Y_U, FIELD_W_U, FIELD_H_U),
        0.0,
        SURFACE_0,
        Stroke::new(u * edge_u, edge),
        StrokeKind::Inside,
    );
    let inner = geo.rect_u(
        FIELD_X_U + FIELD_PAD_U,
        FIELD_Y_U + (FIELD_H_U - ROW_H_U) / 2,
        FIELD_W_U - 2 * FIELD_PAD_U,
        ROW_H_U,
    );
    let mut content = editor.hex_display().unwrap_or_else(|| hex_text(rgba));
    let before = content.clone();
    let response = ui.put(
        inner,
        TextEdit::singleline(&mut content)
            .id(id)
            .font(font.clone())
            .text_color(TEXT)
            .frame(Frame::NONE)
            .margin(Margin::ZERO)
            .desired_width(inner.width())
            .clip_text(true),
    );
    if response.changed() && content != before {
        editor.type_hex(target, rgba, content);
    }
    if response.lost_focus() {
        if events.iter().any(is_escape) {
            editor.escape_hex();
        } else {
            let enter = events.iter().any(is_enter);
            match editor.confirm_hex_checked(mode, !enter, stored) {
                Resolved::Done(outcome) => match outcome {
                    HexOutcome::Commit(commit) => output.commit = Some(commit),
                    HexOutcome::Invalid(error) => {
                        if enter {
                            response.request_focus();
                        }
                        output.notice = Some(hex_notice(&error).to_string());
                    }
                    HexOutcome::Nothing | HexOutcome::Unchanged => {}
                },
                Resolved::Stale(reason) => output.notice = Some(stale_notice(reason).to_string()),
            }
        }
    }
    output
}

/// Paints the palette icon row (or `Confirm`/`Cancel` while a removal is pending) with `geo.top_px`
/// at the top edge of the row, and registers each button's tooltip and accessible label.
pub(crate) fn show_icon_row(ui: &mut Ui, geo: &Geometry, view: ButtonsView) {
    let ctx = ui.ctx().clone();
    let mode = geo.mode;
    let u = ui_unit(geo.ppp);
    let font = FontId::new(font_size_points(geo.ppp), FontFamily::Proportional);
    let painter = ui.painter_at(geo.panel_rect);
    let pointer = ui.input(|input| input.pointer.hover_pos());
    let buttons: &[PaletteButton] = if view.removing {
        &[PaletteButton::Confirm, PaletteButton::Cancel]
    } else {
        &[
            PaletteButton::Remove,
            PaletteButton::RotateLeft,
            PaletteButton::RotateRight,
            PaletteButton::Transparent,
        ]
    };
    for &button in buttons {
        let Some((x, y, width, height)) = button_rect_u(mode, button) else {
            continue;
        };
        let rect = geo.rect_u(x, y, width, height);
        let enabled = blocked_reason(button, view.len, view.has_destination).is_none();
        let hovered = pointer.is_some_and(|p| geo.panel_rect.contains(p) && rect.contains(p));
        let sunken = button == PaletteButton::Transparent && view.transparent_active;
        let look = look(sunken, enabled, hovered);
        match palette_icon(button) {
            Some(icon) => paint_icon(&painter, rect, u, view.icons.get(icon), &look),
            None => paint_text(
                &painter,
                rect,
                u,
                geo.rect_u(x + BUTTON_TEXT_X_U, y + BUTTON_TEXT_Y_U, 0, ROW_H_U)
                    .min,
                palette_button_tooltip(button, false),
                &font,
                &look,
            ),
        }
        // Tooltip and accessible text over the visible part of the button (the panel may be
        // scrolled and clip it).
        let visible = rect.intersect(geo.panel_rect);
        if visible.is_positive() {
            let label = palette_button_tooltip(button, view.transparent_active);
            let response = ui.interact(
                visible,
                ui.unique_id().with(("boton-paleta", button as u8)),
                Sense::click(),
            );
            response.widget_info(|| WidgetInfo::labeled(Role::Button, enabled, label));
            response.on_hover_ui(|ui| {
                ui.add(
                    Label::new(
                        RichText::new(printable(&ctx, label))
                            .font(font.clone())
                            .color(TEXT),
                    )
                    .extend(),
                );
            });
        }
    }
}
