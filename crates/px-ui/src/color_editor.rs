//! Color editor inside the palette panel: sliders, hexadecimal field and notices
//! (specs/hito-2-editar-color.md). The logic lives in [`crate::color_edit`] and the geometry in
//! [`crate::edit_layout`]; here they are only translated to egui.
//!
//! Everything falls on whole physical pixels: the measures are `u` (`n` physical pixels) from `top_px`,
//! which is a multiple of `n`, and the positions inside a track come from `slider_x`.

use egui::{
    Align2, Event, FontFamily, FontId, Frame, Id, Key, Label, Margin, PointerButton, Pos2, Rect,
    RichText, Role, Sense, Stroke, StrokeKind, TextEdit, Ui, WidgetInfo, vec2,
};
use px_core::{Mode, Rgba};

use crate::color_edit::{
    ColorEditor, Commit, EditTarget, HexOutcome, Resolved, control_value, controls, hex_notice,
    hex_text, slider_value_at, slider_x, stale_notice, value_text,
};
use crate::edit_layout::{
    BUTTON_TEXT_X_U, BUTTON_TEXT_Y_U, FIELD_H_U, FIELD_PAD_U, FIELD_W_U, FIELD_X_U, HEADER_X_U,
    HEADER_Y_U, LABEL_X_U, PaletteButton, ROW_H_U, SEPARATOR_Y_U, THUMB_H_U, THUMB_W_U, TRACK_H_U,
    TRACK_W_U, TRACK_X_U, VALUE_X_U, button_rect_u, control_row_u, field_y_u, notice_y_u,
    palette_icon, slider_hit,
};
use crate::icon_button::{look, paint_icon, paint_text};
use crate::icons::IconSet;
use crate::palette_grid::{PANEL_WIDTH_U, TEXT_WIDTH_U};
use crate::palette_ops::{blocked_reason, palette_button_tooltip};
use crate::status::{bar_text, columns};
use crate::text::{font_size_points, printable, ui_unit};
use crate::theme::{ACCENT, NOTICE_PREFIX, SURFACE_0, SURFACE_3, TEXT, TEXT_DIM, WARNING};

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
}

impl Geometry {
    fn n_px(&self) -> i64 {
        i64::from(self.n)
    }

    /// Physical pixels to points.
    fn pt(&self, px: i64) -> f32 {
        px as f32 / self.ppp
    }

    /// Corner of the editor, in points.
    fn origin(&self) -> Pos2 {
        self.panel_min + vec2(0.0, self.pt(self.top_px))
    }

    /// Rectangle in `u` from the corner of the editor, in points.
    fn rect_u(&self, x: i64, y: i64, width: i64, height: i64) -> Rect {
        let n = self.n_px();
        Rect::from_min_size(
            self.origin() + vec2(self.pt(x * n), self.pt(y * n)),
            vec2(self.pt(width * n), self.pt(height * n)),
        )
    }

    /// An egui point, in physical pixels from the corner of the editor.
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

/// What the editor paints besides the sliders and the field.
#[derive(Clone, Debug, Default)]
pub(crate) struct Extras {
    /// The two lines of a pending removal, instead of the notice of the editor.
    pub prompt: Option<[String; 2]>,
}

/// What the editor asked for in a frame.
#[derive(Debug, Default)]
pub(crate) struct Output {
    pub commit: Option<Commit>,
    /// 5 s notice for the status bar.
    pub notice: Option<String>,
}

/// Paints the editor and attends the hexadecimal field. `transparent`: the target is the
/// transparent entry of the palette.
#[allow(clippy::too_many_arguments)]
pub(crate) fn show(
    ui: &mut Ui,
    geo: &Geometry,
    editor: &mut ColorEditor,
    target: EditTarget,
    transparent: bool,
    events: &[Event],
    stored: &dyn Fn(EditTarget) -> Option<Rgba>,
    extras: &Extras,
) -> Output {
    let mut output = Output::default();
    let Some((rgba, hsl)) = editor.shown() else {
        return output;
    };
    let ctx = ui.ctx().clone();
    let mode = geo.mode;
    let n = geo.n_px();
    let u = ui_unit(geo.ppp);
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
        geo.rect_u(0, SEPARATOR_Y_U, PANEL_WIDTH_U, 1),
        0.0,
        SURFACE_3,
    );
    text(HEADER_X_U, HEADER_Y_U, &target.header(mode), TEXT);

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

    // Hexadecimal field: background and border are ours; the `TextEdit` goes inside without a frame,
    // because the margins of egui are whole points and do not fall on whole physical pixels at 1.5×.
    let id = hex_field_id();
    let field_y = field_y_u(mode);
    let focused = ui.memory(|memory| memory.has_focus(id));
    painter.rect(
        geo.rect_u(FIELD_X_U, field_y, FIELD_W_U, FIELD_H_U),
        0.0,
        SURFACE_0,
        Stroke::new(u, if focused { ACCENT } else { SURFACE_3 }),
        StrokeKind::Inside,
    );
    let inner = geo.rect_u(
        FIELD_X_U + FIELD_PAD_U,
        field_y + (FIELD_H_U - ROW_H_U) / 2,
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
                    HexOutcome::Invalid(_) if enter => response.request_focus(),
                    HexOutcome::Invalid(error) => {
                        output.notice = Some(hex_notice(&error).to_string());
                    }
                    HexOutcome::Nothing | HexOutcome::Unchanged => {}
                },
                Resolved::Stale(reason) => output.notice = Some(stale_notice(reason).to_string()),
            }
        }
    }

    // Two-line notice, or the lines of the pending removal.
    let notice = notice_y_u(mode);
    if let Some([first, second]) = &extras.prompt {
        text(HEADER_X_U, notice, first, TEXT);
        let warning = second.starts_with(NOTICE_PREFIX);
        let color = if warning { WARNING } else { TEXT_DIM };
        text(HEADER_X_U, notice + ROW_H_U, second, color);
    } else {
        let [first, second] = editor.notice(transparent);
        text(HEADER_X_U, notice, &first, WARNING);
        text(HEADER_X_U, notice + ROW_H_U, &second, WARNING);
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
            PaletteButton::Add,
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
