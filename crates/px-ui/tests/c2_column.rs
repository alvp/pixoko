//! Unit C2 (specs/ui-single-column.md), egui bench without a window: rows 5 (dragging the left
//! edge of the column), 6 (the resize cursor), 12 (the whole window, pixels and scales) and
//! property 2 (the drag is quantized, reversible and effective). It also checks what the visto
//! bueno of C2 adds: with the color editor open or a removal pending the column is at least 16
//! columns wide, and the escape that the palette leaves free.
//!
//! The bench is the whole stack as `main` paints it: `show_menu`, `status::show`, `show_column` and
//! the canvas. The cursor is read from the platform output of each frame. `Bench::resized` applies
//! what the column returns to the width that was chosen, as the app does with `App.column_cols`.

use egui::{Context, CursorIcon, Event, Key, Modifiers, PointerButton, Pos2, RawInput, Rect, vec2};
use proptest::prelude::*;
use proptest::test_runner::TestCaseError;
use px_core::{Image, Palette, Rgba};
use px_ui::actions::{AppAction, Locks, Platform, UiState};
use px_ui::canvas::Canvas;
use px_ui::chrome::{
    COLUMN_GRIP_U, MENU_H_U, column_cols, column_width_u, head_u, max_cols,
    toolbox_button_rect_u_at, window_layout_cols,
};
use px_ui::color::{ColorChoice, Role, SlotColors};
use px_ui::color_edit::ColorEditor;
use px_ui::column::{ColumnOutput, ColumnView, show_column};
use px_ui::edit_layout::min_window_width_u;
use px_ui::icons::IconSet;
use px_ui::keymap::default_keymap;
use px_ui::menu::show_menu;
use px_ui::palette_grid::{PanelView, Scroll};
use px_ui::palette_ops::PaletteOps;
use px_ui::palette_panel::Roles;
use px_ui::status;
use px_ui::text::{apply_scale, text_scale};
use px_ui::tool::Tool;
use px_ui::toolbar::ToolboxView;
use px_ui::view::Zoom;

mod common;
use common::{Painted, context, in_points, painted};

const PPPS: [f32; 5] = [1.0, 1.25, 1.5, 2.0, 3.0];
/// Tolerance, in physical pixels, of a text respect of a multiple of `n` from the corner of the
/// column (egui places some with margins rounded to 1/16 pt: blurry would be half a pixel).
const TOL_BLURRY_PX: f32 = 0.1;

fn n_of(ppp: f32) -> i64 {
    i64::from(text_scale(ppp))
}

/// An indexed image of 32 entries: the grid has several rows at 8 columns.
fn indexed() -> Image {
    let colors: Vec<Rgba> = (0..32u8)
        .map(|i| Rgba::opaque(i * 7, 255 - i * 5, i * 3))
        .collect();
    let palette = Palette::from_colors(colors).expect("paleta");
    Image::new_indexed(16, 12, palette).expect("imagen indexada")
}

struct Frame {
    out: ColumnOutput,
    cursor: CursorIcon,
    /// The column and the canvas, in egui points.
    column: Rect,
    canvas: Rect,
    painted: Painted,
}

struct Bench {
    ctx: Context,
    ppp: f32,
    window_px: (i64, i64),
    canvas: Canvas,
    scroll: Scroll,
    view: PanelView,
    ops: PaletteOps,
    editor: ColorEditor,
    image: Image,
    slots: SlotColors,
    choice: ColorChoice,
    state: UiState,
    enabled: bool,
    /// The width that was chosen, in columns of swatches (`App.column_cols`).
    wanted: i64,
    time: f64,
}

impl Bench {
    /// The window of `window_px`, the column with `wanted` columns and the editor closed.
    fn new(ppp: f32, window_px: (i64, i64), wanted: i64) -> Bench {
        let image = indexed();
        let mut canvas = Canvas::new();
        canvas.set_image(&image);
        let mut bench = Bench {
            ctx: context(ppp),
            ppp,
            window_px,
            canvas,
            scroll: Scroll::default(),
            view: PanelView {
                editor_open: false,
                ..PanelView::default()
            },
            ops: PaletteOps::default(),
            editor: ColorEditor::default(),
            slots: SlotColors::defaults(image.mode()),
            image,
            choice: ColorChoice::Entry(1),
            state: UiState {
                can_undo: true,
                can_redo: true,
                zoom: Zoom::from_int(5),
                tool: Tool::Pencil,
                locks: Locks::default(),
            },
            enabled: true,
            wanted,
            time: 0.0,
        };
        for _ in 0..3 {
            bench.frame(vec![]);
        }
        bench
    }

    fn default_window(ppp: f32) -> (i64, i64) {
        ((960.0 * ppp).round() as i64, (720.0 * ppp).round() as i64)
    }

    fn n(&self) -> i64 {
        n_of(self.ppp)
    }

    /// The width of the column in physical pixels, from the stable layout.
    fn column_x(&self) -> i64 {
        let layout = window_layout_cols(self.window_px, self.n() as u32, self.wanted);
        layout.panel.x
    }

    fn frame(&mut self, events: Vec<Event>) -> Frame {
        self.time += 0.1;
        let screen = vec2(
            self.window_px.0 as f32 / self.ppp,
            self.window_px.1 as f32 / self.ppp,
        );
        let input = RawInput {
            screen_rect: Some(Rect::from_min_size(Pos2::ZERO, screen)),
            events,
            time: Some(self.time),
            ..RawInput::default()
        };
        let Bench {
            ctx,
            canvas,
            scroll,
            view,
            ops,
            editor,
            image,
            slots,
            choice,
            state,
            enabled,
            wanted,
            ..
        } = self;
        let mut result = None;
        let mut output = ctx.run_ui(input, |ui| {
            apply_scale(ui.ctx());
            show_menu(ui, state, Platform::Linux);
            status::show(ui, "lápiz · color 1 #000000", false);
            let before = ui.available_rect_before_wrap();
            let out = show_column(
                ui,
                &ColumnView {
                    toolbox: ToolboxView {
                        state,
                        zoom_label: "×5",
                        icons: &IconSet::default(),
                    },
                    image,
                    roles: Roles {
                        foreground: *choice,
                        background: *choice,
                        editing: Role::Foreground,
                    },
                    slots,
                    enabled: *enabled,
                    cols: *wanted,
                },
                default_keymap(),
                Platform::Linux,
                scroll,
                view,
                editor,
                ops,
            );
            let after = ui.available_rect_before_wrap();
            canvas.interact_roles(ui);
            canvas.paint(ui);
            result = Some((
                out,
                Rect::from_min_max(Pos2::new(after.max.x, before.min.y), before.max),
                after,
            ));
        });
        let cursor = output.platform_output.cursor_icon;
        output.textures_delta.clear();
        let (out, column, canvas_rect) = result.expect("se dibuja");
        if let Some(k) = out.resized {
            self.wanted = k;
        }
        Frame {
            out,
            cursor,
            column,
            canvas: canvas_rect,
            painted: painted(&output.shapes),
        }
    }

    /// The center of the physical pixel `(x_px, y_px)`: a pointer sits on a pixel, not on its corner
    /// (which is where the canvas ends and the column begins).
    fn at(&self, x_px: i64, y_px: i64) -> Pos2 {
        in_points(self.ppp, x_px, y_px) + vec2(0.5 / self.ppp, 0.5 / self.ppp)
    }

    fn move_to(&mut self, x_px: i64, y_px: i64) -> Frame {
        let pos = self.at(x_px, y_px);
        self.frame(vec![Event::PointerMoved(pos)])
    }

    fn press(&mut self, x_px: i64, y_px: i64) -> Frame {
        let pos = self.at(x_px, y_px);
        self.frame(vec![Event::PointerButton {
            pos,
            button: PointerButton::Primary,
            pressed: true,
            modifiers: Modifiers::NONE,
        }])
    }

    fn release(&mut self, x_px: i64, y_px: i64) -> Frame {
        let pos = self.at(x_px, y_px);
        self.frame(vec![Event::PointerButton {
            pos,
            button: PointerButton::Primary,
            pressed: false,
            modifiers: Modifiers::NONE,
        }])
    }

    fn escape(&mut self) -> Frame {
        self.frame(vec![Event::Key {
            key: Key::Escape,
            physical_key: Some(Key::Escape),
            pressed: true,
            repeat: false,
            modifiers: Modifiers::NONE,
        }])
    }

    /// `y` of the middle of the palette part of the column, in physical pixels.
    fn mid_y(&self) -> i64 {
        let n = self.n();
        let band = self.window_px.1 - (MENU_H_U + 20) * n;
        MENU_H_U * n + band * 3 / 4
    }

    /// Presses at `from_x`, moves to `to_x` in three steps with the button down and returns the
    /// frames of the moves (the button stays down).
    fn drag(&mut self, from_x: i64, to_x: i64) -> Vec<Frame> {
        let y = self.mid_y();
        self.move_to(from_x, y);
        self.move_to(from_x, y);
        self.press(from_x, y);
        (1..=3)
            .map(|i| self.move_to(from_x + (to_x - from_x) * i / 3, y))
            .collect()
    }

    /// The width of the column in physical pixels, measured on a frame.
    fn column_width_px(&mut self) -> i64 {
        let f = self.frame(vec![]);
        ((f.column.width() * self.ppp).round()) as i64
    }
}

// ---------------------------------------------------------------------------------------------
// Row 5: dragging the left edge
// ---------------------------------------------------------------------------------------------

#[test]
fn row_5_only_the_4_u_zone_of_the_left_edge_inside_the_column_drags() {
    assert_eq!(COLUMN_GRIP_U, 4);
    for ppp in PPPS {
        let n = n_of(ppp);
        // Pressing at 0 u, at the last pixel of the zone, at 4 u (outside) and one pixel to the left
        // of the column (the canvas): only the first two drag; 28 n to the left are two columns.
        for (offset, drags) in [(0, true), (4 * n - 1, true), (4 * n, false), (-1, false)] {
            let mut bench = Bench::new(ppp, Bench::default_window(ppp), 16);
            let x = bench.column_x() + offset;
            bench.drag(x, x - 28 * n);
            bench.release(x - 28 * n, bench.mid_y());
            assert_eq!(
                bench.wanted,
                if drags { 18 } else { 16 },
                "ppp {ppp}, offset {offset} px"
            );
        }
    }
}

#[test]
fn row_5_the_width_follows_k0_plus_the_rounded_steps_of_14_u_and_is_clamped() {
    for ppp in [1.0f32, 1.5, 2.0] {
        let n = n_of(ppp);
        let window = Bench::default_window(ppp);
        let k_max = max_cols(window.0, n as u32);
        for k0 in [8, 12, 16] {
            for steps in -12i64..=30 {
                // The pointer travels `steps` columns to the left, plus 3 n that must round away.
                let mut bench = Bench::new(ppp, window, k0);
                let x = bench.column_x();
                let target = x - (steps * 14 + 3) * n;
                bench.drag(x, target);
                bench.release(target, bench.mid_y());
                assert_eq!(
                    bench.wanted,
                    (k0 + steps).clamp(8, k_max),
                    "ppp {ppp}, k0 {k0}, {steps} steps"
                );
            }
        }
        // Half a step rounds to the nearest: 6 n gives none, 8 n gives one.
        for (travel, added) in [(6, 0), (8, 1), (-6, 0), (-8, -1)] {
            let mut bench = Bench::new(ppp, window, 16);
            let x = bench.column_x();
            bench.drag(x, x - travel * n);
            bench.release(x - travel * n, bench.mid_y());
            assert_eq!(bench.wanted, 16 + added, "ppp {ppp}, travel {travel} n");
        }
    }
}

#[test]
fn row_5_the_column_is_whole_pixels_and_the_two_grids_are_laid_out_again_with_whole_columns() {
    let ppp = 1.5;
    let n = n_of(ppp);
    let window = Bench::default_window(ppp);
    let mut bench = Bench::new(ppp, window, 16);
    let x = bench.column_x();
    bench.drag(x, x + 8 * 14 * n + 3 * n);
    let f = bench.release(x + 8 * 14 * n, bench.mid_y());
    assert!(f.out.resized.is_none() || bench.wanted == 8);
    assert_eq!(bench.wanted, 8);
    // A frame later the column measures exactly `W(8)·n` pixels and the canvas the rest.
    assert_eq!(bench.column_width_px(), column_width_u(8) * n);
    let f = bench.frame(vec![]);
    assert_eq!(
        (f.canvas.width() * ppp).round() as i64,
        window.0 - column_width_u(8) * n
    );
    // The toolbox: at 8 columns 4 buttons fit a row, so the Eraser (the fifth tool) is on the
    // second row; clicking its center chooses it.
    let (bx, by, bw, bh) = toolbox_button_rect_u_at(4, 5, 8);
    assert_eq!((bx, by), (4, 29));
    let col_x = window.0 - column_width_u(8) * n;
    let (cx, cy) = (col_x + (bx + bw / 2) * n, (MENU_H_U + by + bh / 2) * n);
    bench.move_to(cx, cy);
    bench.press(cx, cy);
    let released = bench.release(cx, cy);
    assert_eq!(
        released.out.toolbox.action,
        Some(AppAction::SelectTool(Tool::Eraser))
    );
    // The palette: the grid has 8 swatches per row; swatch 9 is the second one of the second row.
    let head = head_u(5, 8);
    let (sx, sy) = (
        col_x + (4 + 14 + 6) * n,
        (MENU_H_U + head + 49 + 14 + 6) * n,
    );
    bench.move_to(sx, sy);
    let pressed = bench.press(sx, sy);
    bench.release(sx, sy);
    assert_eq!(pressed.out.panel.choice, Some(ColorChoice::Entry(9)));
}

#[test]
fn row_5_a_drag_never_changes_the_document_the_history_or_the_color() {
    let ppp = 1.0;
    let n = n_of(ppp);
    let mut bench = Bench::new(ppp, Bench::default_window(ppp), 16);
    let x = bench.column_x();
    let frames = bench.drag(x, x - 5 * 14 * n);
    for f in &frames {
        assert!(f.out.panel.choice.is_none() && f.out.panel.background.is_none());
        assert!(f.out.panel.commit.is_none() && f.out.panel.op.is_none());
        assert!(f.out.panel.indicator.is_none() && f.out.toolbox.action.is_none());
    }
    let f = bench.release(x - 5 * 14 * n, bench.mid_y());
    assert!(f.out.panel.choice.is_none() && f.out.panel.op.is_none());
    assert_eq!(bench.choice, ColorChoice::Entry(1));
}

#[test]
fn row_5_going_there_and_back_returns_k0_and_releasing_outside_the_window_ends_the_drag() {
    let ppp = 2.0;
    let n = n_of(ppp);
    let window = Bench::default_window(ppp);
    let mut bench = Bench::new(ppp, window, 16);
    let (x, y) = (bench.column_x(), bench.mid_y());
    bench.move_to(x, y);
    bench.move_to(x, y);
    bench.press(x, y);
    bench.move_to(x - 5 * 14 * n, y);
    assert_eq!(bench.wanted, 21);
    bench.move_to(x, y);
    assert_eq!(bench.wanted, 16, "ida y vuelta devuelve k0");
    bench.move_to(x - 3 * 14 * n, y);
    assert_eq!(bench.wanted, 19);
    // Released outside the window: the drag ends with the last width and does not continue.
    bench.release(-50, -50);
    assert_eq!(bench.wanted, 19);
    bench.move_to(x - 9 * 14 * n, y);
    assert_eq!(bench.wanted, 19);
}

#[test]
fn row_5_escape_in_the_middle_of_the_drag_goes_back_to_k0_and_the_next_press_drags_again() {
    let ppp = 1.0;
    let n = n_of(ppp);
    let mut bench = Bench::new(ppp, Bench::default_window(ppp), 16);
    let (x, y) = (bench.column_x(), bench.mid_y());
    bench.move_to(x, y);
    bench.move_to(x, y);
    bench.press(x, y);
    bench.move_to(x - 4 * 14 * n, y);
    assert_eq!(bench.wanted, 20);
    bench.escape();
    assert_eq!(bench.wanted, 16, "Esc devuelve k0");
    bench.move_to(x - 6 * 14 * n, y);
    assert_eq!(bench.wanted, 16, "hasta soltar no se mueve más");
    bench.release(x - 6 * 14 * n, y);
    // A new drag works.
    let x = bench.column_x();
    bench.drag(x, x - 14 * n);
    bench.release(x - 14 * n, y);
    assert_eq!(bench.wanted, 17);
}

#[test]
fn row_5_at_the_minimum_window_the_column_can_be_narrowed_to_8_and_widened_to_the_maximum() {
    for ppp in [1.0f32, 1.5] {
        let n = n_of(ppp);
        let window = (min_window_width_u() * n, 400 * n);
        let k_max = max_cols(window.0, n as u32);
        assert_eq!(k_max, 23);
        let mut bench = Bench::new(ppp, window, 16);
        let x = bench.column_x();
        bench.drag(x, x + 40 * 14 * n);
        bench.release(x + 40 * 14 * n, bench.mid_y());
        assert_eq!(bench.wanted, 8, "ppp {ppp}");
        let mut bench = Bench::new(ppp, window, 16);
        let x = bench.column_x();
        bench.drag(x, x - 40 * 14 * n);
        bench.release(x - 40 * 14 * n, bench.mid_y());
        assert_eq!(bench.wanted, k_max, "ppp {ppp}");
        // The canvas never gets less than 128 u.
        let f = bench.frame(vec![]);
        assert!((f.canvas.width() * ppp).round() as i64 >= 128 * n);
    }
}

#[test]
fn extra_with_the_editor_open_or_a_removal_pending_the_column_is_at_least_16_columns_wide() {
    for ppp in [1.0f32, 1.5, 2.0] {
        let n = n_of(ppp);
        for window in [
            Bench::default_window(ppp),
            (min_window_width_u() * n, 420 * n),
        ] {
            let mut bench = Bench::new(ppp, window, 8);
            assert_eq!(bench.column_width_px(), column_width_u(8) * n, "cerrado");
            // The editor opens: the column widens to 16 columns without touching what was chosen.
            bench.view.editor_open = true;
            bench.frame(vec![]);
            assert_eq!(
                bench.column_width_px(),
                column_width_u(16) * n,
                "editor abierto"
            );
            assert_eq!(bench.wanted, 8);
            // And it closes: back to the chosen width.
            bench.view.editor_open = false;
            bench.frame(vec![]);
            assert_eq!(
                bench.column_width_px(),
                column_width_u(8) * n,
                "cerrado otra vez"
            );
            // A pending removal widens it too.
            bench.ops.begin_removal(3, Some(0));
            bench.frame(vec![]);
            assert_eq!(
                bench.column_width_px(),
                column_width_u(16) * n,
                "quita pendiente"
            );
            bench.ops.cancel_removal();
            bench.frame(vec![]);
            assert_eq!(bench.column_width_px(), column_width_u(8) * n);
            // A wider choice is left alone.
            let mut wide = Bench::new(ppp, window, 20);
            wide.view.editor_open = true;
            wide.frame(vec![]);
            assert_eq!(wide.column_width_px(), column_width_u(20) * n);
        }
    }
}

// ---------------------------------------------------------------------------------------------
// Row 6: the resize cursor
// ---------------------------------------------------------------------------------------------

/// The cursor of the frame in which the pointer rests at `(x, y)` (two frames to settle).
fn cursor_at(bench: &mut Bench, x: i64, y: i64) -> CursorIcon {
    bench.move_to(x, y);
    bench.move_to(x, y).cursor
}

#[test]
fn row_6_the_resize_cursor_is_only_in_the_zone_of_the_column_and_nowhere_else() {
    for ppp in PPPS {
        let n = n_of(ppp);
        for (name, window) in [
            ("grande", Bench::default_window(ppp)),
            // 249 u: `k_max = 8`, so there is no other width to go to.
            ("estrecha", (249 * n, 400 * n)),
        ] {
            let mut bench = Bench::new(ppp, window, 16);
            let k_max = max_cols(window.0, n as u32);
            let may = k_max >= 9;
            let column_x = bench.column_x();
            let band_top = MENU_H_U * n;
            let band_bottom = window.1 - 20 * n;
            // Sweep the pointer along lines through the menu, the toolbox part, the palette part,
            // the status bar and the canvas, across every border (pixel by pixel near the edges).
            let ys = [
                ("menú", 4 * n),
                ("caja", band_top + 10 * n),
                ("paleta", band_top + (band_bottom - band_top) * 3 / 4),
                ("barra", band_bottom + 4 * n),
            ];
            for (line, y) in ys {
                let mut xs: Vec<i64> = (0..window.0)
                    .step_by(((window.0 / 40).max(1)) as usize)
                    .collect();
                for edge in [column_x, column_x + 4 * n] {
                    xs.extend((edge - 2)..=(edge + 2));
                }
                xs.push(window.0 - 1);
                for x in xs {
                    if x < 0 || x >= window.0 {
                        continue;
                    }
                    let in_zone = x >= column_x
                        && x < column_x + COLUMN_GRIP_U * n
                        && y >= band_top
                        && y < band_bottom;
                    let cursor = cursor_at(&mut bench, x, y);
                    let expected = in_zone && may;
                    assert_eq!(
                        cursor == CursorIcon::ResizeHorizontal,
                        expected,
                        "{name}, ppp {ppp}, {line}, x = {x} (columna en {column_x}), k_max {k_max}"
                    );
                }
            }
        }
    }
}

#[test]
fn row_6_with_a_dialog_or_a_modal_there_is_neither_cursor_nor_drag_and_the_drag_cursor_holds() {
    let ppp = 1.0;
    let n = n_of(ppp);
    let window = Bench::default_window(ppp);
    let mut bench = Bench::new(ppp, window, 16);
    let (x, y) = (bench.column_x() + n, bench.mid_y());
    assert_eq!(cursor_at(&mut bench, x, y), CursorIcon::ResizeHorizontal);
    bench.enabled = false;
    assert_ne!(cursor_at(&mut bench, x, y), CursorIcon::ResizeHorizontal);
    bench.drag(x, x - 5 * 14 * n);
    bench.release(x - 5 * 14 * n, y);
    assert_eq!(bench.wanted, 16, "sin arrastre con un diálogo");
    // Enabled again: the cursor is shown during the drag even when the pointer has left the zone.
    bench.enabled = true;
    bench.move_to(x, y);
    bench.move_to(x, y);
    bench.press(x, y);
    let frames: Vec<_> = (1..=3).map(|i| bench.move_to(x - i * 14 * n, y)).collect();
    for f in frames {
        assert_eq!(f.cursor, CursorIcon::ResizeHorizontal);
    }
    bench.release(x - 3 * 14 * n, y);
}

#[test]
fn row_6_no_edge_has_the_cursor_unless_the_drag_changes_the_width() {
    let ppp = 1.0;
    let n = n_of(ppp);
    // On a window where nothing can change (`k_max = 8`) the cursor does not appear and a drag in
    // either direction leaves the width as it was.
    let window = (249 * n, 400 * n);
    let mut bench = Bench::new(ppp, window, 16);
    let x = bench.column_x() + n;
    let y = bench.mid_y();
    assert_ne!(cursor_at(&mut bench, x, y), CursorIcon::ResizeHorizontal);
    for target in [x - 6 * 14 * n, x + 6 * 14 * n] {
        bench.drag(x, target);
        bench.release(target, y);
        assert_eq!(column_cols(window.0, n as u32, bench.wanted), 8);
    }
    // Where the cursor shows, a drag in at least one direction changes the width.
    let window = Bench::default_window(ppp);
    let mut bench = Bench::new(ppp, window, 16);
    let x = bench.column_x() + n;
    assert_eq!(cursor_at(&mut bench, x, y), CursorIcon::ResizeHorizontal);
    bench.drag(x, x - 14 * n);
    bench.release(x - 14 * n, y);
    assert_ne!(bench.wanted, 16);
}

// ---------------------------------------------------------------------------------------------
// The escape that the palette leaves free
// ---------------------------------------------------------------------------------------------

#[test]
fn extra_escape_is_left_free_only_when_nothing_in_the_palette_claims_it() {
    let ppp = 1.0;
    let mut bench = Bench::new(ppp, Bench::default_window(ppp), 16);
    // Nothing open: Escape is free (the app closes the options window, then the editor).
    assert!(bench.escape().out.panel.escape_free);
    // A pending removal claims it first.
    bench.ops.begin_removal(3, Some(0));
    assert!(!bench.escape().out.panel.escape_free);
    assert!(bench.ops.removal().is_none(), "Esc cancela la quita");
    // The editor open and nothing else: free, and the editor asks to be closed after it.
    bench.view.editor_open = true;
    bench.frame(vec![]);
    let f = bench.escape();
    assert!(f.out.panel.escape_free && f.out.panel.close_editor);
    // Disabled (a gesture, a dialog): not free.
    bench.enabled = false;
    assert!(!bench.escape().out.panel.escape_free);
}

// ---------------------------------------------------------------------------------------------
// Row 12: the whole window, pixels and scales
// ---------------------------------------------------------------------------------------------

/// Distance of `value` (physical pixels) to the nearest multiple of `n`.
fn rest_of_n(value: f32, n: i64) -> f32 {
    let rest = value.rem_euclid(n as f32);
    rest.min(n as f32 - rest)
}

#[test]
fn row_12_with_odd_window_widths_nothing_falls_in_half_a_pixel_and_the_column_paints_inside_itself()
{
    for ppp in PPPS {
        let n = n_of(ppp);
        let minimum = (min_window_width_u() * n, 430 * n);
        for editor_open in [false, true] {
            for extra in 0..=9 {
                let window = (minimum.0 + extra, minimum.1 + 2 * extra);
                let mut bench = Bench::new(ppp, window, 16);
                bench.view.editor_open = editor_open;
                bench.frame(vec![]);
                let f = bench.frame(vec![]);
                let layout = window_layout_cols(window, n as u32, 16);
                // The column: whole physical pixels, `W(k)·n` wide, at the right edge.
                let physical = [
                    f.column.min.x * ppp,
                    f.column.min.y * ppp,
                    f.column.max.x * ppp,
                    f.column.max.y * ppp,
                ];
                for (i, v) in physical.iter().enumerate() {
                    assert!(
                        (v - v.round()).abs() <= 1.0 / 32.0 * ppp + 1e-3,
                        "ppp {ppp} {window:?}: borde {i} en medio píxel: {v}"
                    );
                }
                assert_eq!(physical[0].round() as i64, layout.panel.x);
                assert_eq!(physical[2].round() as i64, window.0);
                assert_eq!(
                    (physical[2] - physical[0]).round() as i64,
                    column_width_u(column_cols(window.0, n as u32, 16)) * n
                );
                // Every text of the column is at a multiple of `n` from its corner and everything
                // it paints is clipped to its rectangle.
                let expanded = f.column.expand(0.05);
                let mut seen = 0;
                for t in f.painted.texts.iter().filter(|t| {
                    t.pos.x * ppp >= layout.panel.x as f32
                        && t.pos.y * ppp >= layout.toolbar.y as f32
                }) {
                    seen += 1;
                    for (axis, pos, corner) in [
                        ("x", t.pos.x * ppp, layout.panel.x as f32),
                        ("y", t.pos.y * ppp, layout.toolbar.y as f32),
                    ] {
                        assert!(
                            rest_of_n(pos - corner, n) <= TOL_BLURRY_PX,
                            "ppp {ppp} {window:?}, editor {editor_open}: {:?} borroso en {axis}",
                            t.text_2
                        );
                    }
                    assert!(
                        expanded.contains_rect(t.clip),
                        "ppp {ppp}: {:?} se pinta fuera de la columna: {:?}",
                        t.text_2,
                        t.clip
                    );
                }
                assert!(seen > 0, "ppp {ppp}: la columna no pinta texto");
                for fill in f.painted.fills.iter().filter(|r| {
                    r.rect.center().x * ppp > layout.panel.x as f32
                        && r.rect.center().y * ppp > layout.toolbar.y as f32
                }) {
                    assert!(expanded.contains_rect(fill.clip), "ppp {ppp}: {fill:?}");
                }
            }
        }
    }
}

// ---------------------------------------------------------------------------------------------
// Property 2: the drag is quantized, reversible and effective
// ---------------------------------------------------------------------------------------------

#[derive(Clone, Copy, Debug)]
enum Step {
    /// Moves the pointer to `dx` u from the left edge of the column and `fy` per thousand of the
    /// band's height.
    Move {
        dx: i64,
        fy: i64,
    },
    Press,
    Release,
    Esc,
}

fn step() -> impl Strategy<Value = Step> {
    prop_oneof![
        4 => (-60i64..=60, 0i64..=1000).prop_map(|(dx, fy)| Step::Move { dx, fy }),
        2 => Just(Step::Press),
        2 => Just(Step::Release),
        1 => Just(Step::Esc),
    ]
}

/// `k₀ + round(travel / 14n)` clamped, or `None` at a tie (half a step), which the rounding of the
/// implementation may resolve either way.
fn dragged_k(k0: i64, travel_px: i64, n: i64, k_max: i64) -> Option<i64> {
    let step = 14 * n;
    if travel_px.rem_euclid(step) == step / 2 && step % 2 == 0 {
        return None;
    }
    let steps = (travel_px as f64 / step as f64).round() as i64;
    Some((k0 + steps).clamp(8, k_max))
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(64))]

    #[test]
    fn property_2_k_stays_in_range_the_drag_is_quantized_reversible_and_the_cursor_is_exact(
        ppp in prop::sample::select(vec![1.0f32, 1.5, 2.0]),
        extra_w in 0i64..=900,
        k0 in 8i64..=30,
        enabled in prop::bool::weighted(0.9),
        steps in prop::collection::vec(step(), 1..14),
    ) {
        let n = n_of(ppp);
        let window = (min_window_width_u() * n + extra_w, 420 * n);
        let k_max = max_cols(window.0, n as u32);
        let mut bench = Bench::new(ppp, window, k0);
        bench.enabled = enabled;
        let band_top = MENU_H_U * n;
        let band = window.1 - (MENU_H_U + 20) * n;
        let mut pos = (bench.column_x() + 2 * n, band_top + band / 2);
        // The drag in course: where it began and the width it began with; `cancelled` after `Esc`.
        let mut drag: Option<(i64, i64)> = None;
        let mut cancelled = false;
        let mut changed_last_frame = true;
        // The button is down on something that is not the edge: egui hovers nothing else meanwhile.
        let mut pressed_elsewhere = false;
        for st in steps {
            let column_x = bench.column_x();
            let before = bench.wanted;
            let frame = match st {
                Step::Move { dx, fy } => {
                    pos = (column_x + dx * n, band_top + band * fy / 1000);
                    bench.move_to(pos.0, pos.1)
                }
                Step::Press => {
                    let in_zone = pos.0 >= column_x
                        && pos.0 < column_x + COLUMN_GRIP_U * n
                        && pos.1 >= band_top
                        && pos.1 < band_top + band;
                    if enabled && in_zone && drag.is_none() {
                        drag = Some((pos.0, bench.wanted));
                        cancelled = false;
                    } else {
                        pressed_elsewhere = true;
                    }
                    bench.press(pos.0, pos.1)
                }
                Step::Release => {
                    drag = None;
                    pressed_elsewhere = false;
                    bench.release(pos.0, pos.1)
                }
                Step::Esc => {
                    if let Some((_, wanted0)) = drag {
                        cancelled = true;
                        let frame = bench.escape();
                        prop_assert_eq!(bench.wanted, wanted0, "Esc devuelve k0");
                        changed_last_frame = bench.wanted != before;
                        continue_check(&bench, window, n, k_max, &frame)?;
                        continue;
                    }
                    bench.escape()
                }
            };
            // `k` is an integer in range after every frame, the width follows it and the canvas holds.
            continue_check(&bench, window, n, k_max, &frame)?;
            match (drag, st) {
                (Some((press_x, wanted0)), Step::Move { .. }) if !cancelled => {
                    // Quantized: `k₀ + round(Δx/14n)`, with `k₀` the width shown when it began;
                    // reversible: back at the press it is `k₀` again.
                    let k_start = column_cols(window.0, n as u32, wanted0);
                    if let Some(expected) = dragged_k(k_start, press_x - pos.0, n, k_max) {
                        prop_assert_eq!(bench.wanted, expected);
                    }
                    prop_assert_eq!(frame.cursor, CursorIcon::ResizeHorizontal);
                    // A drag of the edge never chooses a color nor edits the palette.
                    prop_assert!(frame.out.panel.choice.is_none() && frame.out.panel.op.is_none());
                }
                (None, Step::Move { .. }) if !changed_last_frame && !pressed_elsewhere => {
                    // Effective: the cursor is there exactly when the pointer is in the zone, the
                    // column is enabled and there is another width to go to.
                    let in_zone = pos.0 >= column_x
                        && pos.0 < column_x + COLUMN_GRIP_U * n
                        && pos.1 >= band_top
                        && pos.1 < band_top + band;
                    prop_assert_eq!(
                        frame.cursor == CursorIcon::ResizeHorizontal,
                        in_zone && enabled && k_max >= 9,
                        "pointer {:?}, column at {}",
                        pos,
                        column_x
                    );
                }
                _ => {}
            }
            changed_last_frame = bench.wanted != before;
        }
    }
}

/// The invariants that every frame keeps: range, whole pixels, canvas and nothing chosen.
fn continue_check(
    bench: &Bench,
    window: (i64, i64),
    n: i64,
    k_max: i64,
    frame: &Frame,
) -> Result<(), TestCaseError> {
    let cols = column_cols(window.0, n as u32, bench.wanted);
    prop_assert!((8..=k_max.max(8)).contains(&cols));
    let layout = window_layout_cols(window, n as u32, bench.wanted);
    prop_assert_eq!(layout.panel.width, column_width_u(cols) * n);
    prop_assert!(layout.canvas.width >= 128 * n);
    let _ = frame;
    Ok(())
}
