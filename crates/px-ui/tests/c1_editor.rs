//! Unit C1 (specs/ui-composition.md), egui bench without a window (the one of `panel_roles.rs`,
//! copied, with the time of each frame for the tooltips): rows 5 and 6 (the focus when opening and
//! `Escape` last in the chain), 11 and 12 (the icon row with the editor open and closed, and the
//! palette operations), 13 (a swatch has no tooltip), 15 (mark of the edited role, tooltips of the
//! bars and `F1`) and properties 3 and 4.

use std::cell::Cell;

use egui::epaint::{ClippedShape, Shape};
use egui::{
    Color32, Context, Event, Key, MissingGlyphPolicy, Modifiers, PointerButton, Pos2, RawInput,
    Rect, Vec2, pos2, vec2,
};
use proptest::prelude::*;
use px_core::{Image, Palette, Rgba};
use px_ui::color::{
    ColorChoice, Role, RoleFlags, SlotColors, cell_label_with, info_flags_roles,
    is_alpha0_cell_with, is_transparent_cell_with,
};
use px_ui::color_block::{bar_cell_rect, bar_fill_rect};
use px_ui::color_block_es::BAR_TIP;
use px_ui::color_edit::{ColorEditor, Control, EditTarget};
use px_ui::color_editor::hex_field_id;
use px_ui::edit_layout::{PaletteButton, button_rect_u, panel_layout, panel_layout_closed};
use px_ui::help::HelpTarget;
use px_ui::help_texts::help_entry;
use px_ui::palette_grid::{
    CELL_U, COLUMNS, GRID_X_U, GRID_Y_U, PITCH_U, PanelView, Scroll, ring_rects, total_rows,
};
use px_ui::palette_ops::{PaletteOp, PaletteOps};
use px_ui::palette_panel::{BarClick, PanelOutput, Roles, panel_info, show_roles};
use px_ui::status::columns;
use px_ui::text::{apply_scale, install_fonts, text_scale};
use px_ui::theme::{ACCENT, SURFACE_3};
use px_ui::tool::Tool;
use px_ui::view::ScreenRect;

#[path = "common/zones.rs"]
mod zones;
use zones::panel_on_the_left;

const SCREEN: Vec2 = vec2(1200.0, 1000.0);

fn indexed(count: usize) -> Image {
    let colors: Vec<Rgba> = (0..count)
        .map(|i| Rgba::opaque(i as u8, 255 - i as u8, (i * 5) as u8))
        .collect();
    let mut palette = Palette::from_colors(colors).expect("palette");
    if count > 2 {
        palette.set_transparent_index(Some(2)).expect("exists");
    }
    Image::new_indexed(8, 8, palette).expect("image")
}

struct Bench {
    ctx: Context,
    scroll: Scroll,
    view: PanelView,
    editor: ColorEditor,
    ops: PaletteOps,
    image: Image,
    roles: Roles,
    enabled: bool,
    time: Cell<Option<f64>>,
    /// Height of the screen (and of the panel) in points.
    height: f32,
}

struct Frame {
    output: PanelOutput,
    shapes: Vec<ClippedShape>,
}

impl Bench {
    fn new(ppp: f32, image: Image, roles: Roles) -> Bench {
        let ctx = Context::default();
        ctx.set_pixels_per_point(ppp);
        ctx.set_missing_glyph_policy(MissingGlyphPolicy::Panic);
        install_fonts(&ctx);
        let mut bench = Bench {
            ctx,
            scroll: Scroll::default(),
            view: PanelView::default(),
            editor: ColorEditor::default(),
            ops: PaletteOps::default(),
            image,
            roles,
            enabled: true,
            time: Cell::new(None),
            height: SCREEN.y,
        };
        for _ in 0..2 {
            bench.frame(vec![]);
        }
        bench
    }

    fn n(&self) -> i64 {
        i64::from(text_scale(self.ctx.pixels_per_point()))
    }

    fn ppp(&self) -> f32 {
        self.ctx.pixels_per_point()
    }

    fn input(&self, events: Vec<Event>) -> RawInput {
        let time = self.time.get().map(|t| {
            self.time.set(Some(t + 0.1));
            t + 0.1
        });
        RawInput {
            screen_rect: Some(Rect::from_min_size(Pos2::ZERO, vec2(SCREEN.x, self.height))),
            events,
            time,
            ..RawInput::default()
        }
    }

    fn frame(&mut self, events: Vec<Event>) -> Frame {
        let slots = SlotColors::defaults(self.image.mode());
        let edited = match self.roles.editing {
            Role::Foreground => self.roles.foreground,
            Role::Background => self.roles.background,
        };
        if let Some(target) = EditTarget::of(&self.image, edited) {
            let color = target.stored(&self.image, &slots).expect("color");
            self.editor.sync(Some((target, color)));
        }
        let input = self.input(events);
        let (image, roles, enabled) = (&self.image, self.roles, self.enabled);
        let (scroll, view, editor, ops) = (
            &mut self.scroll,
            &mut self.view,
            &mut self.editor,
            &mut self.ops,
        );
        let mut result = None;
        let mut output = self.ctx.run_ui(input, |ui| {
            apply_scale(ui.ctx());
            let (panel, ()) = panel_on_the_left(
                ui,
                |ui| show_roles(ui, image, roles, scroll, view, enabled, &slots, editor, ops),
                |_| (),
            );
            result = Some(panel);
        });
        output.textures_delta.clear();
        Frame {
            output: result.expect("the panel is painted"),
            shapes: std::mem::take(&mut output.shapes),
        }
    }

    fn rest(&mut self) -> Frame {
        self.frame(vec![])
    }

    fn move_to(&mut self, (x, y): (i64, i64)) -> Frame {
        let ppp = self.ppp();
        self.frame(vec![Event::PointerMoved(pos2(
            x as f32 / ppp,
            y as f32 / ppp,
        ))])
    }

    fn button_event(&self, (x, y): (i64, i64), button: PointerButton, pressed: bool) -> Event {
        let ppp = self.ppp();
        Event::PointerButton {
            pos: pos2(x as f32 / ppp, y as f32 / ppp),
            button,
            pressed,
            modifiers: Modifiers::NONE,
        }
    }

    /// Full click; returns what the panel said when pressing.
    fn click(&mut self, point: (i64, i64), button: PointerButton) -> PanelOutput {
        self.move_to(point);
        let pressed = self
            .frame(vec![self.button_event(point, button, true)])
            .output;
        self.frame(vec![self.button_event(point, button, false)]);
        pressed
    }

    fn escape(&mut self) -> PanelOutput {
        self.frame(vec![Event::Key {
            key: Key::Escape,
            physical_key: Some(Key::Escape),
            pressed: true,
            repeat: false,
            modifiers: Modifiers::NONE,
        }])
        .output
    }

    fn layout(&self) -> px_ui::edit_layout::PanelLayout {
        let height = (self.height * self.ppp()).round() as i64;
        let n = self.n() as u32;
        if self.view.editor_open {
            panel_layout(self.image.mode(), height, n, 0)
        } else {
            panel_layout_closed(self.image.mode(), height, n, 0)
        }
    }

    /// Top edge of the icon row: under the grid when it is short, anchored when it is tall.
    fn icons_top(&self) -> i64 {
        // The grid holds the entries and the handle (C3).
        let rows = total_rows(self.image.palette().map_or(0, |p| p.len() + 1));
        self.layout().tail_top_for(rows, self.n() as u32)
    }

    fn button_center(&self, button: PaletteButton) -> (i64, i64) {
        let (x, y, w, h) = button_rect_u(self.image.mode(), button).expect("button");
        let n = self.n();
        ((x + w / 2) * n, self.icons_top() + (y + h / 2) * n)
    }

    fn swatch_center(&self, index: usize) -> (i64, i64) {
        let n = self.n();
        let (column, row) = ((index % COLUMNS) as i64, (index / COLUMNS) as i64);
        (
            (GRID_X_U + column * PITCH_U) * n + CELL_U * n / 2,
            (GRID_Y_U + row * PITCH_U) * n + CELL_U * n / 2,
        )
    }
}

fn roles(foreground: ColorChoice, background: ColorChoice, editing: Role) -> Roles {
    Roles {
        foreground,
        background,
        editing,
    }
}

fn rect_center(r: &ScreenRect) -> (i64, i64) {
    (r.x + r.width / 2, r.y + r.height / 2)
}

/// Center of the click zone of the bar of `role`, in physical pixels from the corner of the panel.
fn bar_center(bench: &Bench, role: Role) -> (i64, i64) {
    let (x, y) = rect_center(&bar_cell_rect(role, COLUMNS as i64, bench.n() as u32));
    (x, bench.icons_top() + y)
}

/// The ring of `thickness` physical pixels around the fill of the bar of `role`, in points (the
/// bench runs at ppp 1).
fn bar_border(bench: &Bench, role: Role, thickness: i64) -> Vec<Rect> {
    let fill = bar_fill_rect(role, COLUMNS as i64, bench.n() as u32);
    border_of(fill, thickness)
        .into_iter()
        .map(|r| r.translate(vec2(0.0, bench.icons_top() as f32)))
        .collect()
}

fn rect_fills(shapes: &[ClippedShape]) -> Vec<(Rect, Color32)> {
    fn rec(shape: &Shape, out: &mut Vec<(Rect, Color32)>) {
        match shape {
            Shape::Rect(rect) => out.push((rect.rect, rect.fill)),
            Shape::Vec(shapes) => shapes.iter().for_each(|s| rec(s, out)),
            _ => {}
        }
    }
    let mut out = Vec::new();
    for shape in shapes {
        rec(&shape.shape, &mut out);
    }
    out
}

fn texts(shapes: &[ClippedShape]) -> Vec<String> {
    fn rec(shape: &Shape, out: &mut Vec<String>) {
        match shape {
            Shape::Text(text) => out.push(text.galley.text().to_string()),
            Shape::Vec(shapes) => shapes.iter().for_each(|s| rec(s, out)),
            _ => {}
        }
    }
    let mut out = Vec::new();
    for shape in shapes {
        rec(&shape.shape, &mut out);
    }
    out
}

fn as_rect(r: ScreenRect) -> Rect {
    Rect::from_min_size(
        pos2(r.x as f32, r.y as f32),
        vec2(r.width as f32, r.height as f32),
    )
}

// ---------------------------------------------------------------------------------------------
// Row 5: the focus when opening
// ---------------------------------------------------------------------------------------------

#[test]
fn row_5_a_right_click_that_switches_the_role_takes_the_focus_to_the_hex_field() {
    let mut bench = Bench::new(
        1.0,
        indexed(16),
        roles(
            ColorChoice::Entry(3),
            ColorChoice::Entry(5),
            Role::Foreground,
        ),
    );
    let at = bar_center(&bench, Role::Background);
    let out = bench.click(at, PointerButton::Secondary);
    assert_eq!(out.bar_click, Some(BarClick::FocusEditor(Role::Background)));
    bench.rest();
    bench.rest();
    assert_eq!(bench.ctx.memory(|m| m.focused()), Some(hex_field_id()));
}

#[test]
fn row_5_the_right_click_that_closes_does_not_ask_for_the_focus() {
    let mut bench = Bench::new(
        1.0,
        indexed(16),
        roles(
            ColorChoice::Entry(3),
            ColorChoice::Entry(5),
            Role::Foreground,
        ),
    );
    // The edited bar: the app closes the editor, so the field must not take the focus.
    let at = bar_center(&bench, Role::Foreground);
    let out = bench.click(at, PointerButton::Secondary);
    assert_eq!(out.bar_click, Some(BarClick::FocusEditor(Role::Foreground)));
    bench.rest();
    bench.rest();
    assert_ne!(bench.ctx.memory(|m| m.focused()), Some(hex_field_id()));
}

#[test]
fn row_5_with_the_editor_closed_the_right_click_asks_to_open_and_the_focus_follows_the_opening() {
    let mut bench = Bench::new(
        1.0,
        indexed(16),
        roles(
            ColorChoice::Entry(3),
            ColorChoice::Entry(5),
            Role::Foreground,
        ),
    );
    bench.view.editor_open = false;
    bench.rest();
    let at = bar_center(&bench, Role::Foreground);
    let out = bench.click(at, PointerButton::Secondary);
    assert_eq!(out.bar_click, Some(BarClick::FocusEditor(Role::Foreground)));
    // The app opens the editor; the field takes the focus in a frame without pointer.
    bench.view.editor_open = true;
    bench.rest();
    bench.rest();
    assert_eq!(bench.ctx.memory(|m| m.focused()), Some(hex_field_id()));
}

// ---------------------------------------------------------------------------------------------
// Row 6: Escape is last in the chain
// ---------------------------------------------------------------------------------------------

#[test]
fn row_6_escape_with_nothing_else_in_progress_asks_to_close_and_with_the_editor_closed_it_does_not()
{
    let r = roles(
        ColorChoice::Entry(3),
        ColorChoice::Entry(5),
        Role::Foreground,
    );
    let mut bench = Bench::new(1.0, indexed(16), r);
    assert!(bench.escape().close_editor);
    bench.view.editor_open = false;
    bench.rest();
    assert!(!bench.escape().close_editor);
}

#[test]
fn row_6_escape_with_a_gesture_a_slider_a_swatch_drag_or_a_pending_removal_does_not_close() {
    let r = roles(
        ColorChoice::Entry(3),
        ColorChoice::Entry(5),
        Role::Foreground,
    );
    // Disabled panel (a drawing gesture, a dialog...): nothing.
    let mut bench = Bench::new(1.0, indexed(16), r);
    bench.enabled = false;
    assert!(!bench.escape().close_editor, "disabled");
    // A slider drag: the editor cancels it and stays open.
    let mut bench = Bench::new(1.0, indexed(16), r);
    bench.editor.begin(Control::Red, 100);
    assert!(bench.editor.dragging());
    assert!(!bench.escape().close_editor, "slider");
    assert!(!bench.editor.dragging());
    // A swatch drag.
    let mut bench = Bench::new(1.0, indexed(16), r);
    bench.ops.begin_drag(3, (10, 10));
    assert!(bench.ops.drag_state().is_some());
    assert!(!bench.escape().close_editor, "swatch drag");
    assert!(bench.ops.drag_state().is_none());
    // A pending removal: the first Escape cancels it, the second closes.
    let mut bench = Bench::new(1.0, indexed(16), r);
    bench.ops.begin_removal(3, Some(0));
    assert!(!bench.escape().close_editor, "removal");
    assert!(bench.ops.removal().is_none());
    assert!(bench.escape().close_editor, "and then it closes");
}

#[test]
fn row_6_escape_with_the_hex_field_focused_only_releases_it_and_the_next_one_closes() {
    let r = roles(
        ColorChoice::Entry(3),
        ColorChoice::Entry(5),
        Role::Foreground,
    );
    let mut bench = Bench::new(1.0, indexed(16), r);
    bench.ctx.memory_mut(|m| m.request_focus(hex_field_id()));
    bench.rest();
    bench.rest();
    assert_eq!(bench.ctx.memory(|m| m.focused()), Some(hex_field_id()));
    assert!(
        !bench.escape().close_editor,
        "the first only releases the field"
    );
    bench.rest();
    assert_ne!(bench.ctx.memory(|m| m.focused()), Some(hex_field_id()));
    assert!(bench.escape().close_editor, "the second closes");
}

// ---------------------------------------------------------------------------------------------
// Rows 11 and 12: the icon row and the palette operations, editor open and closed
// ---------------------------------------------------------------------------------------------

fn image_with_unused_entries() -> Image {
    let colors: Vec<Rgba> = (0..5)
        .map(|i| Rgba::opaque(i as u8 * 40, 255 - i as u8 * 30, 7))
        .collect();
    let palette = Palette::from_colors(colors).expect("palette");
    // Everything is entry 0, so the entries 1 to 4 are unused.
    Image::new_indexed(4, 4, palette).expect("image")
}

#[test]
fn row_11_a_short_grid_keeps_the_icon_row_right_under_it_with_the_editor_open_or_closed() {
    for ppp in [1.0f32, 2.0] {
        let mut bench = Bench::new(
            ppp,
            image_with_unused_entries(),
            roles(
                ColorChoice::Entry(3),
                ColorChoice::Entry(1),
                Role::Foreground,
            ),
        );
        bench.view.editor_open = false;
        let closed = bench.icons_top();
        bench.view.editor_open = true;
        let open = bench.icons_top();
        // Five entries are one row: the row hangs at 4 + 14 = 18 u whether the editor is open or not.
        assert_eq!(closed, 18 * bench.n(), "ppp {ppp}");
        assert_eq!(open, closed, "ppp {ppp}");
    }
}

#[test]
fn row_11_a_tall_grid_anchors_the_icon_row_to_the_bottom_or_right_above_the_editor() {
    for ppp in [1.0f32, 2.0] {
        let mut bench = Bench::new(
            ppp,
            indexed(256),
            roles(
                ColorChoice::Entry(3),
                ColorChoice::Entry(1),
                Role::Foreground,
            ),
        );
        // 320 u: lower than the 330 u the 17 rows (256 entries and the handle) and the tail need,
        // and above the 308 u block.
        bench.height = 320.0;
        let height = (bench.height * ppp).round() as i64;
        let n = bench.n();
        bench.view.editor_open = false;
        let closed = bench.icons_top();
        bench.view.editor_open = true;
        let open = bench.icons_top();
        // The 16 rows of the grid do not fit: closed, the row closes the panel; open, it sits right
        // above the editor.
        assert_eq!(closed + 88 * n, height, "ppp {ppp}");
        assert!(open < closed, "ppp {ppp}");
        assert_eq!(bench.layout().top_px - open, 88 * n, "ppp {ppp}");
    }
}

#[test]
fn row_12_every_palette_operation_works_with_the_editor_closed_and_open_and_acts_on_the_edited_role()
 {
    for open in [false, true] {
        for editing in [Role::Foreground, Role::Background] {
            // Closed, the app has already put the role back on the foreground.
            if !open && editing == Role::Background {
                continue;
            }
            let r = roles(ColorChoice::Entry(3), ColorChoice::Entry(1), editing);
            let edited = if editing == Role::Foreground { 3 } else { 1 };
            let context = format!("open {open}, {editing:?}");

            let mut bench = Bench::new(1.0, image_with_unused_entries(), r);
            bench.view.editor_open = open;
            bench.rest();
            let out = bench.click(
                bench.button_center(PaletteButton::RotateLeft),
                PointerButton::Primary,
            );
            assert_eq!(
                out.op,
                Some(PaletteOp::Rotate {
                    start: 0,
                    len: 5,
                    steps: -1
                }),
                "{context}"
            );

            let mut bench = Bench::new(1.0, image_with_unused_entries(), r);
            bench.view.editor_open = open;
            bench.rest();
            let out = bench.click(
                bench.button_center(PaletteButton::Transparent),
                PointerButton::Primary,
            );
            assert_eq!(
                out.op,
                Some(PaletteOp::SetTransparent {
                    index: Some(edited)
                }),
                "{context}"
            );

            // An unused entry is removed without asking for a destination.
            let mut bench = Bench::new(1.0, image_with_unused_entries(), r);
            bench.view.editor_open = open;
            bench.rest();
            let out = bench.click(
                bench.button_center(PaletteButton::Remove),
                PointerButton::Primary,
            );
            assert!(out.op_unused, "{context}");
            assert!(
                matches!(out.op, Some(PaletteOp::Remove { index, .. }) if index == edited),
                "{context}: {:?}",
                out.op
            );
        }
    }
}

#[test]
fn row_12_dragging_a_swatch_moves_it_with_the_editor_closed() {
    let r = roles(
        ColorChoice::Entry(3),
        ColorChoice::Entry(1),
        Role::Foreground,
    );
    let mut bench = Bench::new(1.0, indexed(16), r);
    bench.view.editor_open = false;
    bench.rest();
    let (from, to) = (bench.swatch_center(0), bench.swatch_center(2));
    bench.move_to(from);
    bench.frame(vec![bench.button_event(from, PointerButton::Primary, true)]);
    bench.move_to(to);
    let out = bench
        .frame(vec![bench.button_event(to, PointerButton::Primary, false)])
        .output;
    assert_eq!(out.op, Some(PaletteOp::Move { from: 0, to: 2 }));
}

// ---------------------------------------------------------------------------------------------
// Row 13: a swatch has no tooltip (the status line keeps its text)
// ---------------------------------------------------------------------------------------------

fn tooltip_texts(bench: &mut Bench, point: (i64, i64)) -> Vec<String> {
    if bench.time.get().is_none() {
        bench.time.set(Some(1.0));
    }
    bench.move_to(point);
    let mut frame = bench.rest();
    for _ in 0..8 {
        frame = bench.rest();
    }
    texts(&frame.shapes)
}

#[test]
fn row_13_a_swatch_has_no_tooltip_and_the_status_line_keeps_its_text() {
    let r = roles(
        ColorChoice::Entry(7),
        ColorChoice::Entry(9),
        Role::Foreground,
    );
    let mut bench = Bench::new(1.0, indexed(256), r);
    let over = bench.swatch_center(3);
    let shown = tooltip_texts(&mut bench, over);
    let info = panel_info(&bench.ctx);
    // The text is still there for the status line (index, hex and flags) ...
    assert!(info.lines[0].starts_with("índice 3 "), "{:?}", info.lines);
    assert!(
        info.status_hint
            .as_deref()
            .is_some_and(|hint| hint.starts_with(&info.lines[0])),
        "{:?}",
        info.status_hint
    );
    // ... but nothing is shown next to the pointer: no line of the swatch, and nothing fixed in
    // the panel either (the chosen swatch is not described there).
    for line in info.lines.iter().filter(|line| !line.is_empty()) {
        assert!(!shown.contains(line), "tooltip over a swatch: {shown:?}");
    }
    assert!(
        !shown
            .iter()
            .any(|t| t.starts_with("índice ") || t == "frente"),
        "{shown:?}"
    );
}

#[test]
fn row_13_there_is_no_tooltip_during_a_swatch_drag_or_a_pending_removal() {
    let r = roles(
        ColorChoice::Entry(7),
        ColorChoice::Entry(9),
        Role::Foreground,
    );
    let mut bench = Bench::new(1.0, indexed(256), r);
    bench.ops.begin_removal(7, Some(0));
    let over = bench.swatch_center(3);
    let shown = tooltip_texts(&mut bench, over);
    assert!(
        !shown.iter().any(|t| t.starts_with("índice 3 ")),
        "removal: {shown:?}"
    );
    // A real drag: press on swatch 1, move past the threshold to swatch 5 and keep pressing.
    let mut bench = Bench::new(1.0, indexed(256), r);
    let (from, over) = (bench.swatch_center(1), bench.swatch_center(5));
    bench.move_to(from);
    bench.frame(vec![bench.button_event(from, PointerButton::Primary, true)]);
    bench.move_to(over);
    assert!(bench.ops.drag_state().is_some_and(|drag| drag.dragging()));
    let shown = tooltip_texts(&mut bench, over);
    assert!(
        !shown.iter().any(|t| t.starts_with("índice 5 ")),
        "drag: {shown:?}"
    );
}

// ---------------------------------------------------------------------------------------------
// Row 14 (panel part): what the status line is asked to show
// ---------------------------------------------------------------------------------------------

#[test]
fn row_14_the_panel_asks_the_status_line_for_a_drag_then_a_removal_then_the_swatch_under_the_pointer()
 {
    let r = roles(
        ColorChoice::Entry(7),
        ColorChoice::Entry(9),
        Role::Foreground,
    );
    // Nothing under the pointer: the usual line.
    let mut bench = Bench::new(1.0, indexed(256), r);
    bench.rest();
    assert_eq!(panel_info(&bench.ctx).status_hint, None);
    // A swatch under the pointer.
    let over = bench.swatch_center(3);
    bench.move_to(over);
    bench.rest();
    let hint = panel_info(&bench.ctx).status_hint.expect("hover hint");
    assert!(
        hint.starts_with("índice 3 ") && hint.contains(" · "),
        "{hint}"
    );
    // A pending removal wins over the swatch under the pointer.
    bench.ops.begin_removal(7, None);
    bench.rest();
    assert_eq!(
        panel_info(&bench.ctx).status_hint.as_deref(),
        Some("quitar 7: pulsa un destino · elige otra muestra o confirma")
    );
    bench.ops.cancel_removal();
    // A swatch drag (a removal is not pending: pressing a swatch would then choose the destination).
    let mut bench = Bench::new(1.0, indexed(256), r);
    let (from, to) = (bench.swatch_center(1), bench.swatch_center(5));
    bench.move_to(from);
    bench.frame(vec![bench.button_event(from, PointerButton::Primary, true)]);
    bench.move_to(to);
    bench.rest();
    assert_eq!(
        panel_info(&bench.ctx).status_hint.as_deref(),
        Some("mover 1 a 5 · Escape cancela")
    );
}

// ---------------------------------------------------------------------------------------------
// Row 15: mark of the edited role, tooltips of the bars and F1
// ---------------------------------------------------------------------------------------------

fn border_of(rect: ScreenRect, thickness: i64) -> Vec<Rect> {
    ring_rects(rect, thickness)
        .into_iter()
        .map(as_rect)
        .collect()
}

#[test]
fn row_15_the_edited_bar_gets_a_2_u_accent_border_and_the_other_a_1_u_dim_one() {
    for (editing, other) in [
        (Role::Foreground, Role::Background),
        (Role::Background, Role::Foreground),
    ] {
        let r = roles(ColorChoice::Entry(3), ColorChoice::Entry(5), editing);
        let mut bench = Bench::new(1.0, indexed(16), r);
        let fills = rect_fills(&bench.rest().shapes);
        for rect in bar_border(&bench, editing, 2) {
            assert!(
                fills.contains(&(rect, ACCENT)),
                "{editing:?}: missing the accent border {rect:?}"
            );
        }
        for rect in bar_border(&bench, other, 1) {
            assert!(
                fills.contains(&(rect, SURFACE_3)),
                "{editing:?}: missing the dim border {rect:?}"
            );
        }
    }
}

#[test]
fn row_15_with_the_editor_closed_the_foreground_bar_has_the_accent_border() {
    // C4: the hexadecimal field is always seen, so the border says which role it writes to.
    let r = roles(
        ColorChoice::Entry(3),
        ColorChoice::Entry(5),
        Role::Foreground,
    );
    let mut bench = Bench::new(1.0, indexed(16), r);
    bench.view.editor_open = false;
    let fills = rect_fills(&bench.rest().shapes);
    for border in bar_border(&bench, Role::Foreground, 2) {
        assert!(fills.contains(&(border, ACCENT)), "{border:?}");
    }
    for border in bar_border(&bench, Role::Background, 1) {
        assert!(fills.contains(&(border, SURFACE_3)), "{border:?}");
    }
    for border in bar_border(&bench, Role::Background, 2) {
        assert!(!fills.contains(&(border, ACCENT)), "{border:?}");
    }
}

#[test]
fn row_15_each_bar_has_its_two_line_tooltip() {
    let r = roles(
        ColorChoice::Entry(3),
        ColorChoice::Entry(5),
        Role::Foreground,
    );
    for (role, name) in [
        (Role::Foreground, "Primer plano"),
        (Role::Background, "Fondo"),
    ] {
        let mut bench = Bench::new(1.0, indexed(16), r);
        let point = bar_center(&bench, role);
        let shown = tooltip_texts(&mut bench, point);
        assert!(shown.iter().any(|t| t == name), "{name}: {shown:?}");
        assert!(shown.iter().any(|t| t == BAR_TIP), "{name}: {shown:?}");
    }
    assert_eq!(
        BAR_TIP,
        "clic: cuentagotas; clic derecho: editar este color"
    );
}

#[test]
fn row_15_f1_over_a_bar_opens_the_help_of_the_color_bars() {
    let r = roles(
        ColorChoice::Entry(3),
        ColorChoice::Entry(5),
        Role::Foreground,
    );
    for role in [Role::Foreground, Role::Background] {
        let mut bench = Bench::new(1.0, indexed(16), r);
        let point = bar_center(&bench, role);
        bench.move_to(point);
        bench.rest();
        assert_eq!(
            px_ui::palette_panel::panel_info(&bench.ctx).help,
            Some(HelpTarget::ColorBar),
            "{role:?}"
        );
    }
    assert_eq!(
        HelpTarget::ColorBar.action(Tool::Pencil),
        px_ui::actions::AppAction::ToggleColorEditor
    );
    let entry = help_entry("color.bar").expect("the entry exists");
    assert_eq!(entry.name, "Barras de color");
    for needle in ["clic derecho", "cuentagotas", "hexadecimal", "primer plano"] {
        assert!(
            entry.description.contains(needle),
            "the help does not say {needle:?}: {}",
            entry.description
        );
    }
}

#[test]
fn row_15_a_disabled_panel_neither_opens_nor_picks() {
    let r = roles(
        ColorChoice::Entry(3),
        ColorChoice::Entry(5),
        Role::Foreground,
    );
    let mut bench = Bench::new(1.0, indexed(16), r);
    bench.enabled = false;
    let at = bar_center(&bench, Role::Foreground);
    for button in [PointerButton::Primary, PointerButton::Secondary] {
        let out = bench.click(at, button);
        assert_eq!(out.bar_click, None, "{button:?}");
    }
}

// ---------------------------------------------------------------------------------------------
// Property 4: the information is not lost
// ---------------------------------------------------------------------------------------------

thread_local! {
    static SHARED_BENCH: std::cell::RefCell<Option<Bench>> = const { std::cell::RefCell::new(None) };
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(128))]

    #[test]
    fn property_4_the_tooltip_and_status_texts_are_what_the_pure_functions_say(
        cell in 0usize..256,
        foreground in 0u8..=255,
        background in 0u8..=255,
    ) {
        SHARED_BENCH.with(|shared| {
            let mut shared = shared.borrow_mut();
            let bench = shared.get_or_insert_with(|| {
                Bench::new(
                    1.0,
                    indexed(256),
                    roles(ColorChoice::Entry(0), ColorChoice::Entry(0), Role::Foreground),
                )
            });
            bench.roles = roles(
                ColorChoice::Entry(foreground),
                ColorChoice::Entry(background),
                Role::Foreground,
            );
            let point = bench.swatch_center(cell);
            bench.move_to(point);
            bench.rest();
            let info = panel_info(&bench.ctx);
            let slots = SlotColors::defaults(bench.image.mode());
            let expected_label = cell_label_with(&bench.image, cell, &slots).unwrap_or_default();
            let expected_flags = info_flags_roles(
                RoleFlags {
                    hover: true,
                    foreground: cell == usize::from(foreground),
                    background: cell == usize::from(background),
                    in_range: false,
                    transparent: is_transparent_cell_with(&bench.image, cell, &slots),
                    alpha0: is_alpha0_cell_with(&bench.image, cell, &slots),
                },
                columns((px_ui::palette_grid::TEXT_WIDTH_U) as f32, 1.0),
            );
            prop_assert_eq!(&info.lines[0], &expected_label);
            prop_assert_eq!(&info.lines[1], &expected_flags);
            let joined = if expected_flags.is_empty() {
                expected_label.clone()
            } else {
                format!("{expected_label} · {expected_flags}")
            };
            prop_assert_eq!(info.status_hint, Some(joined));
            prop_assert!(expected_flags.chars().count() <= 32);
            Ok(())
        })?;
    }
}
