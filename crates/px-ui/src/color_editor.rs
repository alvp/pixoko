//! Color editor inside the palette panel: the square, the bars and the sliders
//! (specs/hito-2-editar-color.md, specs/ui-color-block.md), and the tail under the grid: the two
//! color bars with the swap button, the hexadecimal field and the icon row
//! (specs/ui-palette-handle.md). The logic lives in [`crate::color_edit`], [`crate::color_bar`]
//! and [`crate::color_block`] and the geometry in [`crate::edit_layout`]; here they are only
//! translated to egui.
//!
//! Everything falls on whole physical pixels: the measures are `u` (`n` physical pixels) from `top_px`,
//! which is a multiple of `n`, and the positions inside a track come from `slider_x`.

use egui::{
    Align2, Color32, ColorImage, Context, Event, EventFilter, FontFamily, FontId, Frame, Id, Key,
    Label, Margin, Mesh, PointerButton, Pos2, Rect, RichText, Role, Sense, Shape, Stroke,
    StrokeKind, TextEdit, TextureHandle, TextureId, TextureOptions, Ui, WidgetInfo, vec2,
};
use px_core::{Image, Mode, Rgba};

use crate::color::{
    ColorChoice, Role as ColorRole, SlotColors, cell_color_with, cell_of_choice,
    is_transparent_cell_with,
};
use crate::color_bar::{BarInk, bar_ink, bar_label};
use crate::color_block::{
    Arrow, BarHit, MarkTone, Piece, SELECTOR_X_U, SQUARE_SIDE_U, alpha_mesh, bar_cell_rect,
    bar_fill_rect, bar_hit, bar_key, bar_mark, bar_value_at, hue_mesh, piece_rect, selector_rect,
    square_key, square_mark, square_value_at, sv_texel, swap_rect,
};
use crate::color_block_es::{
    BACKGROUND, BAR_TIP, FOREGROUND, MODE_HSL, MODE_HSV, MODE_RGB, SWAP_TIP,
};
use crate::color_edit::{
    ColorEditor, Commit, Control, EditTarget, HexOutcome, Resolved, SliderMode, control_value,
    control_value_hsv, hex_notice, hex_text, slider_value_at, slider_x, stale_notice, value_text,
    visible_controls,
};
use crate::edit_layout::{
    BUTTON_TEXT_X_U, BUTTON_TEXT_Y_U, FIELD_H_U, FIELD_PAD_U, FIELD_W_U, FIELD_X_U, FIELD_Y_U,
    LABEL_X_U, PaletteButton, ROW_H_U, SEPARATOR_Y_U, THUMB_H_U, THUMB_W_U, TRACK_H_U, TRACK_W_U,
    TRACK_X_U, VALUE_X_U, button_rect_u, control_row_u, palette_icon, slider_hit,
};
use crate::icon_button::{look, paint_icon, paint_text};
use crate::icons::{Icon, IconSet};
use crate::palette_grid::{
    CHECK_U, TEXT_WIDTH_U, block_parts, panel_width_u, ring_rects, text_width_u,
};
use crate::palette_ops::{blocked_reason, palette_button_tooltip};
use crate::status::{bar_text, columns};
use crate::text::{font_size_points, printable, ui_unit};
use crate::theme::{ACCENT, SURFACE_0, SURFACE_3, TEXT, TEXT_DIM, WARNING};
use crate::view::ScreenRect;

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
    /// Which model the numeric sliders show.
    pub slider_mode: SliderMode,
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

    /// A rectangle in physical pixels from the corner of the editor or the tail, in points.
    fn rect_px(&self, r: ScreenRect) -> Rect {
        Rect::from_min_size(
            self.origin() + vec2(self.pt(r.x), self.pt(r.y)),
            vec2(self.pt(r.width), self.pt(r.height)),
        )
    }

    /// An egui point, in physical pixels from the corner of the editor or the tail.
    pub(crate) fn rel_px(&self, pos: Pos2) -> (i64, i64) {
        (
            ((pos.x - self.panel_min.x) * self.ppp).round() as i64,
            ((pos.y - self.panel_min.y) * self.ppp).round() as i64 - self.top_px,
        )
    }

    /// Value of a slider under the `x` (relative to the editor) of a point.
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

/// Identifier of the widget of a piece of the editor (the square or one of the two bars), which
/// takes the keyboard focus (specs/ui-color-block.md, decision 18).
pub fn piece_id(piece: Piece) -> Id {
    Id::unique(("pixoko-pieza-color", piece as u8))
}

fn sv_uploads_id() -> Id {
    Id::unique("pixoko-sv-uploads")
}

fn sv_texture_id() -> Id {
    Id::unique("pixoko-sv-texture")
}

/// How many times the texture of the saturation and brightness square has been uploaded to `ctx`
/// (a counter for the tests, like `Canvas::preview_uploads`).
pub fn sv_uploads(ctx: &Context) -> u32 {
    ctx.data(|d| d.get_temp(sv_uploads_id())).unwrap_or(0)
}

/// What the drag in progress was started by: a slider, a piece (square or bar) or an arrow key.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Source {
    Slider(Control),
    Piece(Piece),
    Key(Key),
}

fn source_id() -> Id {
    Id::unique("pixoko-color-drag-source")
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

const PIECES: [Piece; 3] = [Piece::Square, Piece::HueBar, Piece::AlphaBar];

fn piece_at(x_px: i64, y_px: i64, n: u32) -> Option<Piece> {
    PIECES.into_iter().find(|piece| {
        let r = piece_rect(*piece, n);
        x_px >= r.x && x_px < r.x + r.width && y_px >= r.y && y_px < r.y + r.height
    })
}

fn selector_at(x_px: i64, y_px: i64, n: u32) -> Option<usize> {
    (0..SELECTOR_X_U.len()).find(|i| {
        let r = selector_rect(*i, n);
        x_px >= r.x && x_px < r.x + r.width && y_px >= r.y && y_px < r.y + r.height
    })
}

const SLIDER_MODES: [SliderMode; 3] = [SliderMode::Rgb, SliderMode::Hsl, SliderMode::Hsv];

/// The piece that has the keyboard focus, if any.
fn focused_piece(ctx: &Context) -> Option<Piece> {
    PIECES
        .into_iter()
        .find(|piece| ctx.memory(|m| m.has_focus(piece_id(*piece))))
}

fn arrow_of(key: Key) -> Option<Arrow> {
    match key {
        Key::ArrowLeft => Some(Arrow::Left),
        Key::ArrowRight => Some(Arrow::Right),
        Key::ArrowUp => Some(Arrow::Up),
        Key::ArrowDown => Some(Arrow::Down),
        _ => None,
    }
}

/// Mouse and keyboard of the square, the bars, the selector and the sliders, in the order of the
/// events of the frame. Returns the color to apply if a drag that changed the color was released.
/// A drag is one step, whatever started it: the mouse (press, move, release) or an arrow key
/// (press, repetitions, release of the key). `Escape` and the loss of the window focus cancel it.
pub(crate) fn interact(
    ctx: &Context,
    events: &[Event],
    geo: &Geometry,
    editor: &mut ColorEditor,
    enabled: bool,
    slider_mode: &mut SliderMode,
) -> Option<Commit> {
    let mut commit = None;
    let mut source: Option<Source> = ctx.data(|d| d.get_temp(source_id()));
    let chromatic = matches!(geo.mode, Mode::Rgba | Mode::Indexed);
    let n = geo.n;
    for event in events {
        if source.is_some() && !editor.dragging() {
            source = None;
        }
        match event {
            Event::PointerButton {
                pos,
                button: PointerButton::Primary,
                pressed: true,
                ..
            } if enabled && !editor.dragging() && geo.panel_rect.contains(*pos) => {
                let (x, y) = geo.rel_px(*pos);
                if chromatic && let Some(index) = selector_at(x, y, n) {
                    *slider_mode = SLIDER_MODES[index];
                } else if chromatic && let Some(piece) = piece_at(x, y, n) {
                    let began = match piece {
                        Piece::Square => {
                            let (s, v) = square_value_at(x, y, n);
                            editor.begin_square(s, v)
                        }
                        Piece::HueBar => editor.begin(Control::HueV, bar_value_at(piece, y, n)),
                        Piece::AlphaBar => editor.begin(Control::Alpha, bar_value_at(piece, y, n)),
                    };
                    if began {
                        source = Some(Source::Piece(piece));
                    }
                } else if let Some(control) = slider_hit(geo.mode, *slider_mode, x, y, n)
                    && editor.begin(control, geo.value_at(x, control.max()))
                {
                    source = Some(Source::Slider(control));
                }
            }
            Event::PointerMoved(pos) => {
                if let Some(source) = source {
                    drag_to(editor, geo, source, geo.rel_px(*pos));
                }
            }
            Event::PointerButton {
                pos,
                button: PointerButton::Primary,
                pressed: false,
                ..
            } => {
                if let Some(Source::Slider(_) | Source::Piece(_)) = source {
                    drag_to(editor, geo, source.expect("checked"), geo.rel_px(*pos));
                    commit = editor.release().or(commit);
                    source = None;
                }
            }
            // The repetitions of the key that started the drag are the same step, even though the
            // panel is disabled while a drag lasts.
            Event::Key {
                key,
                pressed: true,
                modifiers,
                ..
            } if source == Some(Source::Key(*key)) => {
                let arrow = arrow_of(*key).expect("a key source is an arrow");
                key_step(editor, held_piece(ctx), arrow, modifiers.shift, false);
            }
            Event::Key {
                key,
                pressed: true,
                modifiers,
                ..
            } if enabled
                && source.is_none()
                && !editor.dragging()
                && arrow_of(*key).is_some()
                && focused_piece(ctx).is_some() =>
            {
                let arrow = arrow_of(*key).expect("checked");
                let piece = focused_piece(ctx).expect("checked");
                if key_step(editor, Some(piece), arrow, modifiers.shift, true) {
                    source = Some(Source::Key(*key));
                    ctx.data_mut(|d| d.insert_temp(key_piece_id(), piece));
                }
            }
            Event::Key {
                key,
                pressed: false,
                ..
            } => {
                if source == Some(Source::Key(*key)) {
                    commit = editor.release().or(commit);
                    source = None;
                }
            }
            Event::WindowFocused(false) => {
                editor.cancel();
                source = None;
            }
            event if is_escape(event) => {
                editor.cancel();
                source = None;
            }
            _ => {}
        }
    }
    // Safety nets: a release that egui never told about.
    if editor.dragging() {
        let lost = match source {
            Some(Source::Key(key)) => !ctx.input(|i| i.key_down(key)),
            _ => !ctx.input(|i| i.pointer.primary_down()),
        };
        if lost {
            commit = editor.release().or(commit);
            source = None;
        }
    }
    if !editor.dragging() {
        source = None;
    }
    ctx.data_mut(|d| {
        if let Some(source) = source {
            d.insert_temp(source_id(), source);
        } else {
            d.remove::<Source>(source_id());
        }
    });
    commit
}

fn key_piece_id() -> Id {
    Id::unique("pixoko-color-key-piece")
}

/// The piece whose arrow key is held down.
fn held_piece(ctx: &Context) -> Option<Piece> {
    ctx.data(|d| d.get_temp(key_piece_id()))
}

/// Moves the drag of `piece` one arrow step from the value the editor shows (`begin` starts it).
/// `false` if there was nothing to do.
fn key_step(
    editor: &mut ColorEditor,
    piece: Option<Piece>,
    arrow: Arrow,
    shift: bool,
    begin: bool,
) -> bool {
    let (Some(piece), Some(hsv), Some((rgba, _))) = (piece, editor.shown_hsv(), editor.shown())
    else {
        return false;
    };
    match piece {
        Piece::Square => {
            let (s, v) = (
                (100.0 * hsv.s()).round() as u32,
                (100.0 * hsv.v()).round() as u32,
            );
            let (s, v) = square_key(s, v, arrow, shift);
            let (s, v) = (s as f32 / 100.0, v as f32 / 100.0);
            if begin {
                editor.begin_square(s, v)
            } else {
                editor.drag_square(s, v);
                true
            }
        }
        Piece::HueBar => {
            let value = bar_key(piece, (hsv.h().round() as u32) % 360, arrow, shift);
            if begin {
                editor.begin(Control::HueV, value)
            } else {
                editor.drag(value);
                true
            }
        }
        Piece::AlphaBar => {
            let value = bar_key(piece, u32::from(rgba.a), arrow, shift);
            if begin {
                editor.begin(Control::Alpha, value)
            } else {
                editor.drag(value);
                true
            }
        }
    }
}

/// Moves the drag that `source` started to the point `(x, y)` (physical pixels from the editor).
fn drag_to(editor: &mut ColorEditor, geo: &Geometry, source: Source, (x, y): (i64, i64)) {
    match source {
        Source::Slider(control) => editor.drag(geo.value_at(x, control.max())),
        Source::Piece(Piece::Square) => {
            let (s, v) = square_value_at(x, y, geo.n);
            editor.drag_square(s, v);
        }
        Source::Piece(piece) => editor.drag(bar_value_at(piece, y, geo.n)),
        Source::Key(_) => {}
    }
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

fn to_color32(color: Rgba) -> Color32 {
    Color32::from_rgba_unmultiplied(color.r, color.g, color.b, color.a)
}

/// The texture of the square for whole degrees of hue `hue`. It is generated only when the hue
/// changes (and the first time): arrastrar inside the square never regenerates it.
fn square_texture(ctx: &Context, hue: u32) -> TextureId {
    let stored: Option<(u32, TextureHandle)> = ctx.data(|d| d.get_temp(sv_texture_id()));
    if let Some((stored_hue, handle)) = &stored
        && *stored_hue == hue
    {
        return handle.id();
    }
    let side = SQUARE_SIDE_U as usize;
    let mut pixels = Vec::with_capacity(side * side);
    for y in 0..side as u32 {
        for x in 0..side as u32 {
            pixels.push(to_color32(sv_texel(hue, x, y)));
        }
    }
    let image = ColorImage::new([side, side], pixels);
    let handle = match stored {
        Some((_, mut handle)) => {
            handle.set(image, TextureOptions::NEAREST);
            handle
        }
        None => ctx.load_texture("pixoko-sv-square", image, TextureOptions::NEAREST),
    };
    let id = handle.id();
    ctx.data_mut(|d| {
        d.insert_temp(sv_texture_id(), (hue, handle));
        let uploads: u32 = d.get_temp(sv_uploads_id()).unwrap_or(0);
        d.insert_temp(sv_uploads_id(), uploads + 1);
    });
    id
}

/// Paints the editor: the separator, the square and the two bars, the selector and the sliders.
/// It has no header, no field and no notices: the bars, the field and the warnings are in the tail
/// under the grid ([`show_bars`], [`show_tail`]).
pub(crate) fn show(ui: &mut Ui, geo: &Geometry, editor: &ColorEditor) {
    let (Some((rgba, hsl)), Some(hsv)) = (editor.shown(), editor.shown_hsv()) else {
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

    if matches!(mode, Mode::Rgba | Mode::Indexed) {
        show_pieces(ui, geo, &painter, rgba, hsv);
        show_selector(&painter, geo, &font);
    }

    for &control in visible_controls(mode, geo.slider_mode) {
        let Some(row) = control_row_u(mode, control) else {
            continue;
        };
        let value = match control {
            Control::HueV | Control::SaturationV | Control::Value => {
                control_value_hsv(hsv, control)
            }
            _ => control_value(rgba, hsl, control),
        };
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

/// The square, the hue bar and the alpha bar with their marks and, for the one that has the
/// keyboard focus, a ring of 2 u of `ACCENT` outside it.
fn show_pieces(
    ui: &mut Ui,
    geo: &Geometry,
    painter: &egui::Painter,
    rgba: Rgba,
    hsv: crate::color_hsv::Hsv,
) {
    let ctx = ui.ctx().clone();
    let n = geo.n;
    let k = geo.n_px();
    for piece in PIECES {
        let rect = geo.rect_px(piece_rect(piece, n));
        let visible = rect.intersect(geo.panel_rect);
        if !visible.is_positive() {
            continue;
        }
        let id = piece_id(piece);
        let response = ui.interact(visible, id, Sense::click_and_drag());
        // A press takes the keyboard focus (egui does not do it by itself for a plain widget).
        if response.is_pointer_button_down_on() {
            response.request_focus();
        }
        if ctx.memory(|m| m.has_focus(id)) {
            ctx.memory_mut(|m| {
                m.set_focus_lock_filter(
                    id,
                    EventFilter {
                        tab: false,
                        horizontal_arrows: true,
                        vertical_arrows: true,
                        escape: false,
                    },
                );
            });
            for ring in ring_rects(piece_rect(piece, n), 2 * k) {
                painter.rect_filled(geo.rect_px(ring), 0.0, ACCENT);
            }
        }
    }
    // The square: a texture of one texel per `u`, of the whole degree of hue the slider shows.
    let hue = (hsv.h().round() as u32) % 360;
    let texture = square_texture(&ctx, hue);
    painter.image(
        texture,
        geo.rect_px(piece_rect(Piece::Square, n)),
        Rect::from_min_max(Pos2::ZERO, Pos2::new(1.0, 1.0)),
        Color32::WHITE,
    );
    // The hue bar: a mesh of seven pairs of vertices, linear between them.
    let bar = geo.rect_px(piece_rect(Piece::HueBar, n));
    let mut mesh = Mesh::default();
    for (position, color) in hue_mesh() {
        let y = bar.min.y + position * bar.height();
        mesh.colored_vertex(Pos2::new(bar.min.x, y), to_color32(color));
        mesh.colored_vertex(Pos2::new(bar.max.x, y), to_color32(color));
    }
    for pair in 0..6u32 {
        let i = pair * 2;
        mesh.add_triangle(i, i + 1, i + 2);
        mesh.add_triangle(i + 1, i + 2, i + 3);
    }
    painter.add(Shape::mesh(mesh));
    // The alpha bar: the checker of the swatches and, over it, the color fading to transparent.
    let alpha = piece_rect(Piece::AlphaBar, n);
    for (part, tone) in block_parts(alpha.width, alpha.height, CHECK_U * k, Rgba::TRANSPARENT) {
        let part = ScreenRect {
            x: alpha.x + part.x,
            y: alpha.y + part.y,
            ..part
        };
        painter.rect_filled(geo.rect_px(part), 0.0, to_color32(tone));
    }
    let alpha_rect = geo.rect_px(alpha);
    let mut mesh = Mesh::default();
    for (position, color) in alpha_mesh(rgba) {
        let y = alpha_rect.min.y + position * alpha_rect.height();
        // Premultiplied colors: opaque at the top and nothing at the bottom.
        let color = if color.a == 255 {
            Color32::from_rgb(color.r, color.g, color.b)
        } else {
            Color32::TRANSPARENT
        };
        mesh.colored_vertex(Pos2::new(alpha_rect.min.x, y), color);
        mesh.colored_vertex(Pos2::new(alpha_rect.max.x, y), color);
    }
    mesh.add_triangle(0, 1, 2);
    mesh.add_triangle(1, 2, 3);
    painter.add(Shape::mesh(mesh));
    // The marks: they come from the models, not from the `Rgba` (a gray keeps its visible hue).
    let tone = |tone: MarkTone| match tone {
        MarkTone::Light => TEXT,
        MarkTone::Edge => SURFACE_0,
    };
    let marks = [
        square_mark(hsv.s(), hsv.v(), n),
        bar_mark(Piece::HueBar, hue, n),
        bar_mark(Piece::AlphaBar, u32::from(rgba.a), n),
    ];
    for (rect, mark_tone) in marks.into_iter().flatten() {
        painter.rect_filled(geo.rect_px(rect), 0.0, tone(mark_tone));
    }
}

/// The selector of the model of the sliders: three exclusive round options (a ring and a dot).
fn show_selector(painter: &egui::Painter, geo: &Geometry, font: &FontId) {
    let k = geo.n_px();
    let u = ui_unit(geo.ppp);
    let labels = [MODE_RGB, MODE_HSL, MODE_HSV];
    for (index, mode) in SLIDER_MODES.into_iter().enumerate() {
        let option = selector_rect(index, geo.n);
        let rect = geo.rect_px(option);
        let ring = Rect::from_min_size(
            rect.min + vec2(0.0, geo.pt((option.height - 10 * k) / 2)),
            vec2(geo.pt(10 * k), geo.pt(10 * k)),
        );
        painter.circle(
            ring.center(),
            (5.0 - 0.5) * u,
            SURFACE_0,
            Stroke::new(u, TEXT_DIM),
        );
        if mode == geo.slider_mode {
            painter.circle_filled(ring.center(), 3.0 * u, ACCENT);
        }
        painter.text(
            rect.min + vec2(geo.pt(14 * k), 0.0),
            Align2::LEFT_TOP,
            labels[index],
            font.clone(),
            TEXT,
        );
    }
}

/// What the two bars need to be painted: the image and the slots (the colors of the swatches), the
/// two choices and the role the hexadecimal field writes to.
#[derive(Clone, Copy)]
pub(crate) struct BarsView<'a> {
    pub image: &'a Image,
    pub slots: &'a SlotColors,
    pub foreground: ColorChoice,
    /// `None` in the old entry points (without two colors): only the foreground bar, no button.
    pub background: Option<ColorChoice>,
    pub editing: ColorRole,
    pub swap_icon: Option<TextureId>,
    pub enabled: bool,
}

/// Paints the two bars under the grid (`geo.top_px` is the top edge of the tail) and the swap
/// button, and registers the tooltips. A bar is painted with the color of its swatch (the resolved
/// one: the transparent entry over the checker) and says which entry or slot it is; the one of the
/// role the hexadecimal field writes to has a border of 2 u of `ACCENT` and the other 1 u of
/// `SURFACE_3`.
pub(crate) fn show_bars(ui: &mut Ui, geo: &Geometry, view: &BarsView) {
    let ctx = ui.ctx().clone();
    let n = geo.n;
    let k = geo.n_px();
    let u = ui_unit(geo.ppp);
    let font = FontId::new(font_size_points(geo.ppp), FontFamily::Proportional);
    let painter = ui.painter_at(geo.panel_rect);
    let bar_columns = columns(((text_width_u(geo.cols) - 26) * k) as f32, geo.ppp);
    let roles = [
        (ColorRole::Foreground, Some(view.foreground)),
        (ColorRole::Background, view.background),
    ];
    for (role, choice) in roles {
        let Some(choice) = choice else { continue };
        let fill = bar_fill_rect(role, geo.cols, n);
        let thickness = if role == view.editing {
            (2, ACCENT)
        } else {
            (1, SURFACE_3)
        };
        for part in ring_rects(fill, thickness.0 * k) {
            painter.rect_filled(geo.rect_px(part), 0.0, thickness.1);
        }
        let cell = cell_of_choice(view.image, choice);
        let color = cell
            .and_then(|cell| cell_color_with(view.image, cell, view.slots))
            .unwrap_or(Rgba::TRANSPARENT);
        for (part, tone) in block_parts(fill.width, fill.height, CHECK_U * k, color) {
            let part = ScreenRect {
                x: fill.x + part.x,
                y: fill.y + part.y,
                ..part
            };
            painter.rect_filled(geo.rect_px(part), 0.0, to_color32(tone));
        }
        let Some(target) = EditTarget::of(view.image, choice) else {
            continue;
        };
        let transparent =
            cell.is_some_and(|cell| is_transparent_cell_with(view.image, cell, view.slots));
        let label = bar_text(&ctx, &bar_label(target, transparent), false, bar_columns);
        let ink: BarInk = bar_ink(color);
        let (x_u, y_u) = (fill.x / k, fill.y / k);
        if ink.boxed {
            // With alpha the text goes in an opaque box over the checker.
            let width = label.chars().count() as i64 * 7 + 2;
            painter.rect_filled(geo.rect_u(x_u + 3, y_u + 1, width, 14), 0.0, ink.behind);
        }
        painter.text(
            geo.rect_u(x_u + 4, y_u + 1, 0, ROW_H_U).min,
            Align2::LEFT_TOP,
            label,
            font.clone(),
            ink.text,
        );
    }
    if view.background.is_none() {
        return;
    }
    let tip = |ui: &mut Ui, lines: &[&str]| {
        for (i, line) in lines.iter().enumerate() {
            let tone = if i == 0 { TEXT } else { TEXT_DIM };
            ui.add(
                Label::new(
                    RichText::new(printable(&ctx, line))
                        .font(font.clone())
                        .color(tone),
                )
                .extend(),
            );
        }
    };
    // One tooltip area over both cells; the pointer says which bar (the cells tile without gaps).
    let area = geo
        .rect_px(bar_cell_rect(ColorRole::Foreground, geo.cols, n))
        .union(geo.rect_px(bar_cell_rect(ColorRole::Background, geo.cols, n)))
        .intersect(geo.panel_rect);
    if area.is_positive() {
        let pointer = ui.input(|i| i.pointer.hover_pos());
        let over = pointer.and_then(|p| {
            let (x, y) = geo.rel_px(p);
            match bar_hit(geo.cols, x, y, n) {
                Some(BarHit::Bar(role)) => Some(role),
                _ => None,
            }
        });
        if let Some(role) = over {
            let name = match role {
                ColorRole::Foreground => FOREGROUND,
                ColorRole::Background => BACKGROUND,
            };
            let response = ui.interact(area, ui.unique_id().with("bar-tooltip"), Sense::hover());
            response.on_hover_ui(|ui| tip(ui, &[name, BAR_TIP]));
        }
    }
    // The swap button.
    let swap = swap_rect(geo.cols, n);
    let rect = geo.rect_px(swap);
    let hovered = ui
        .input(|i| i.pointer.hover_pos())
        .is_some_and(|p| geo.panel_rect.contains(p) && rect.contains(p));
    paint_icon(
        &painter,
        rect,
        u,
        view.swap_icon,
        &look(false, view.enabled, hovered),
    );
    let visible = rect.intersect(geo.panel_rect);
    if visible.is_positive() {
        let response = ui.interact(visible, ui.unique_id().with("swap"), Sense::click());
        response.widget_info(|| WidgetInfo::labeled(Role::Button, view.enabled, SWAP_TIP));
        response.on_hover_ui(|ui| tip(ui, &[SWAP_TIP]));
    }
}

/// Paints the hexadecimal field under the bars (`geo.top_px` is the top edge of the tail) and
/// attends it. `target`: what the field writes to; `stored`: the stored color of a target.
pub(crate) fn show_tail(
    ui: &mut Ui,
    geo: &Geometry,
    editor: &mut ColorEditor,
    target: EditTarget,
    events: &[Event],
    stored: &dyn Fn(EditTarget) -> Option<Rgba>,
) -> Output {
    let mut output = Output::default();
    let Some((rgba, _)) = editor.shown() else {
        return output;
    };
    let mode = geo.mode;
    let u = ui_unit(geo.ppp);
    let font = FontId::new(font_size_points(geo.ppp), FontFamily::Proportional);
    let painter = ui.painter_at(geo.panel_rect);

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

/// The icon the swap button paints, if the set has it.
pub(crate) fn swap_icon(icons: &IconSet) -> Option<TextureId> {
    icons.get(Icon::Swap)
}
