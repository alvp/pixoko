//! Unit C2 (specs/ui-single-column.md): the floating window of options of the active tool, rows 7
//! to 11 (opening and closing, content and geometry and memory, it does not capture outside and is
//! not modal, `F1` and texts) and property 3 (it fits, it is remembered and it does not capture).
//! egui bench without a window (the one of `layers_panel_view`): `Context::run_ui` with synthetic
//! `RawInput`.
//!
//! The window is `224 × (104 + 18·f)` u, with `f = clamp(rows, 1, 8)`: title bar (y = 1, 16 u, the
//! name at (5, 2) and the `x` at x = 207), the shortcut line at y = 21, the body from y = 39 in
//! rows of 18 u, and a brief help of four lines of 30 columns under the body. The POSITION is one
//! and shared by all the tools (changing the tool, by mouse or by key, changes the content without
//! moving it); only the scroll is remembered by tool. The `Esc` chain is in `c2_column.rs` (what
//! the palette leaves free) and in `px-app` (`escape_step`).

use egui::{Context, Event, PointerButton, Pos2, RawInput, Rect, vec2};
use proptest::prelude::*;
use px_core::Image;
use px_ui::actions::AppAction;
use px_ui::canvas::{Canvas, CanvasEvent, CanvasInput};
use px_ui::help::{HelpTarget, help_target_in_options};
use px_ui::help_texts::{help_entry, help_key, help_table};
use px_ui::keymap::{Keymap, default_keymap};
use px_ui::options::{
    NO_OPTIONS, OptionsWindow, WINDOW_W_U, brief_help, key_line, options_max_scroll,
    options_row_count, title, window_height_u, wrap_help,
};
use px_ui::options_view::{OptionsEvent, OptionsView, show_options_window};
use px_ui::text::{INTERFACE_CHARSET, apply_scale, text_scale};
use px_ui::theme::TEXT_DIM;
use px_ui::tool::Tool;
use px_ui::view::ScreenRect;
use px_ui::{actions::Platform, chrome::MENU_H_U, float_window, status::bar_height_px};

mod common;
use common::{Painted, button_of_2, context, in_points, move_pointer, painted, wheel_line};

const PPPS: [f32; 5] = [1.0, 1.25, 1.5, 2.0, 3.0];
const TOOLS: [Tool; 5] = [
    Tool::Pencil,
    Tool::Line,
    Tool::Fill,
    Tool::Dropper,
    Tool::Eraser,
];
const SCREEN: egui::Vec2 = vec2(900.0, 700.0);

fn n_of(ppp: f32) -> i64 {
    i64::from(text_scale(ppp))
}

fn window_px(ppp: f32) -> (i64, i64) {
    (
        (SCREEN.x * ppp).round() as i64,
        (SCREEN.y * ppp).round() as i64,
    )
}

/// The window and a canvas with an image under it, as the app stacks them.
struct Bench {
    ctx: Context,
    ppp: f32,
    tool: Tool,
    rows: usize,
    enabled: bool,
    keymap: Keymap,
    state: OptionsWindow,
    canvas: Canvas,
    time: f64,
}

struct Frame {
    painted: Painted,
    events: Vec<OptionsEvent>,
    canvas: Vec<CanvasInput>,
    zoom: px_ui::view::Zoom,
}

impl Bench {
    fn new(ppp: f32, tool: Tool, rows: usize) -> Bench {
        let image = Image::new(64, 48, px_core::Mode::Rgba).expect("imagen");
        let mut canvas = Canvas::new();
        canvas.set_image(&image);
        let mut bench = Bench {
            ctx: context(ppp),
            ppp,
            tool,
            rows,
            enabled: true,
            keymap: default_keymap().clone(),
            state: OptionsWindow {
                open: true,
                ..OptionsWindow::default()
            },
            canvas,
            time: 0.0,
        };
        for _ in 0..3 {
            bench.frame(vec![]);
        }
        bench
    }

    fn n(&self) -> i64 {
        n_of(self.ppp)
    }

    /// Rectangle of the window in physical pixels (and keeps the clamped position).
    fn rect(&mut self) -> ScreenRect {
        self.state
            .rect(self.rows, window_px(self.ppp), self.n() as u32, 16)
    }

    /// The point `(x_u, y_u)` of the window, in physical pixels.
    fn at(&mut self, x_u: i64, y_u: i64) -> (i64, i64) {
        let (n, rect) = (self.n(), self.rect());
        (rect.x + x_u * n, rect.y + y_u * n)
    }

    fn frame(&mut self, events: Vec<Event>) -> Frame {
        self.time += 0.1;
        let input = RawInput {
            screen_rect: Some(Rect::from_min_size(Pos2::ZERO, SCREEN)),
            events,
            time: Some(self.time),
            ..RawInput::default()
        };
        let window = window_px(self.ppp);
        let view = OptionsView {
            tool: self.tool,
            rows: self.rows,
            keymap: &self.keymap,
            platform: Platform::Linux,
            enabled: self.enabled,
        };
        let Bench {
            ctx, state, canvas, ..
        } = self;
        let mut result = None;
        let mut output = ctx.run_ui(input, |ui| {
            apply_scale(ui.ctx());
            // The window is shown before the canvas, as the layers panel is, so it is the layer on top.
            let events = show_options_window(ui.ctx(), &view, state, window, 16);
            let entries = canvas
                .interact_roles(ui)
                .into_iter()
                .map(|(input, _)| input)
                .collect::<Vec<_>>();
            let info = canvas.paint(ui);
            result = Some((events, entries, info.zoom));
        });
        output.textures_delta.clear();
        let (events, canvas, zoom) = result.expect("se dibuja");
        Frame {
            painted: painted(&output.shapes),
            events,
            canvas,
            zoom,
        }
    }

    fn click(&mut self, x: i64, y: i64) -> Vec<Frame> {
        let pos = in_points(self.ppp, x, y);
        vec![
            self.frame(vec![move_pointer(pos)]),
            self.frame(vec![button_of_2(pos, PointerButton::Primary, true)]),
            self.frame(vec![button_of_2(pos, PointerButton::Primary, false)]),
        ]
    }
}

fn started(frames: &[Frame]) -> bool {
    frames.iter().any(|f| {
        f.canvas
            .iter()
            .any(|e| matches!(e, CanvasInput::Stroke(CanvasEvent::StrokeBegin(_))))
    })
}

// ---------------------------------------------------------------------------------------------
// Row 7: opening and closing
// ---------------------------------------------------------------------------------------------

#[test]
fn row_7_a_right_click_on_a_tool_opens_the_window_closes_it_on_the_same_tool_and_follows_the_others()
 {
    for clicked in TOOLS {
        for other in TOOLS.into_iter().filter(|t| *t != clicked) {
            // Closed: opens with the tool clicked.
            let mut window = OptionsWindow::default();
            assert!(!window.open);
            window.right_click(other, clicked);
            assert!(window.open, "cerrada: abre con {clicked:?}");
            // Open on the same tool: a second right click closes it.
            window.right_click(clicked, clicked);
            assert!(!window.open, "segundo clic derecho en {clicked:?}");
            // Open showing another tool: stays open, now showing the clicked one.
            window.right_click(other, clicked);
            assert!(window.open);
            window.right_click(other, clicked);
            assert!(
                window.open,
                "abierta en {other:?}, clic derecho en {clicked:?}"
            );
        }
    }
    // The `x` and `Esc` (here the state's `close`) close it, and closing a closed one does nothing.
    let mut window = OptionsWindow {
        open: true,
        ..OptionsWindow::default()
    };
    window.close();
    assert!(!window.open);
    window.close();
    assert!(!window.open);
}

#[test]
fn row_7_the_x_of_the_title_bar_closes_and_nothing_responds_with_a_dialog_or_a_modal() {
    for ppp in PPPS {
        let mut bench = Bench::new(ppp, Tool::Line, 0);
        // The `x` button: x = 207 u, y = 1 u, 16 × 16 u; its center is at (215, 9) u.
        let (x, y) = bench.at(215, 9);
        let frames = bench.click(x, y);
        assert!(
            frames
                .iter()
                .any(|f| f.events.contains(&OptionsEvent::Close)),
            "ppp {ppp}: la x pide cerrar"
        );
        // The click did not reach the canvas.
        assert!(!started(&frames));
        // Elsewhere in the title bar there is no close request.
        let (x, y) = bench.at(100, 9);
        assert!(
            !bench
                .click(x, y)
                .iter()
                .any(|f| f.events.contains(&OptionsEvent::Close))
        );
        // With a dialog or a modal open the window is seen but nothing responds.
        bench.enabled = false;
        let (x, y) = bench.at(215, 9);
        let frames = bench.click(x, y);
        assert!(
            !frames
                .iter()
                .any(|f| f.events.contains(&OptionsEvent::Close))
        );
        assert!(bench.state.open);
    }
}

// ---------------------------------------------------------------------------------------------
// Row 8: content, geometry and memory
// ---------------------------------------------------------------------------------------------

#[test]
fn row_8_the_size_is_224_by_104_plus_18_per_row_and_the_content_is_where_the_spec_puts_it() {
    assert_eq!(WINDOW_W_U, 224);
    for (rows, f) in [(0usize, 1i64), (1, 1), (8, 8), (9, 8), (30, 8)] {
        assert_eq!(window_height_u(rows), 104 + 18 * f, "{rows} filas");
        for ppp in PPPS {
            for tool in TOOLS {
                let mut bench = Bench::new(ppp, tool, rows);
                let n = bench.n();
                let rect = bench.rect();
                assert_eq!((rect.width, rect.height), (224 * n, (104 + 18 * f) * n));
                assert_eq!((rect.x % n, rect.y % n), (0, 0));
                let p = bench.frame(vec![]).painted;
                let text_at = |p: &Painted, x_u: i64, y_u: i64| {
                    let (x, y) = (rect.x + x_u * n, rect.y + y_u * n);
                    p.texts
                        .iter()
                        .find(|t| {
                            (t.pos.x * ppp - x as f32).abs() < 0.2
                                && (t.pos.y * ppp - y as f32).abs() < 0.2
                        })
                        .map(|t| (t.text_2.clone(), t.color()))
                };
                // The name in the title bar and the line of the shortcut, from the table.
                assert_eq!(
                    text_at(&p, 5, 2).map(|t| t.0),
                    Some(title(tool).to_string()),
                    "ppp {ppp} {tool:?} {rows} filas"
                );
                let line = text_at(&p, 5, 21).expect("línea del atajo");
                assert_eq!(line.0, key_line(tool, default_keymap(), Platform::Linux));
                assert_eq!(line.1, TEXT_DIM);
                // `sin opciones` in the first row of the body, only when there are no rows.
                let first = text_at(&p, 5, 39);
                if rows == 0 {
                    assert_eq!(first.map(|t| t.0), Some(NO_OPTIONS.to_string()));
                } else {
                    assert_ne!(first.map(|t| t.0), Some(NO_OPTIONS.to_string()));
                }
                // The brief help: the first lines of the help of the tool, under the body.
                let lines = wrap_help(&brief_help(tool));
                assert!(!lines.is_empty() && lines.len() <= 4, "{tool:?}: {lines:?}");
                for (i, line) in lines.iter().enumerate() {
                    let found = text_at(&p, 5, 39 + 18 * f + 4 + 14 * i as i64);
                    assert_eq!(found.map(|t| t.0), Some(line.clone()), "{tool:?} línea {i}");
                }
                // The window is inside the screen and every text is of the interface set.
                assert!(rect.x >= 0 && rect.x + rect.width <= window_px(ppp).0);
                for t in &p.texts {
                    for c in t.text_2.chars() {
                        assert!(INTERFACE_CHARSET.contains(c), "{:?}: {c:?}", t.text_2);
                    }
                }
            }
        }
    }
}

#[test]
fn row_8_the_brief_help_is_the_first_sentence_of_the_help_of_each_tool_and_fits_four_lines() {
    for tool in TOOLS {
        let brief = brief_help(tool);
        assert!(!brief.is_empty());
        assert!(brief.chars().count() <= 100, "{tool:?}: {brief:?}");
        let lines = wrap_help(&brief);
        assert!(lines.len() <= 4, "{tool:?}: {lines:?}");
        assert!(lines.iter().all(|l| l.chars().count() <= 30), "{lines:?}");
        assert_eq!(
            lines.join(" "),
            brief,
            "{tool:?}: el ajuste no pierde palabras"
        );
        // It is the beginning of the description of the entry of the tool.
        let key = help_key(AppAction::SelectTool(tool)).expect("clave de la herramienta");
        let entry = help_entry(key).unwrap_or_else(|| panic!("falta {key}"));
        assert!(
            entry.description.starts_with(brief.trim_end_matches('.')),
            "{tool:?}"
        );
        for c in brief.chars() {
            assert!(INTERFACE_CHARSET.contains(c), "{tool:?}: {c:?}");
        }
    }
}

#[test]
fn row_8_the_scroll_is_by_whole_rows_and_per_tool_and_the_position_is_shared() {
    let mut window = OptionsWindow::default();
    assert_eq!(options_max_scroll(0), 0);
    assert_eq!(options_max_scroll(8), 0);
    assert_eq!(options_max_scroll(9), 1);
    assert_eq!(options_max_scroll(30), 22);
    window.scroll_by(Tool::Pencil, 4, 30);
    window.scroll_by(Tool::Eraser, 9, 30);
    assert_eq!(window.scroll_of(Tool::Pencil), 4);
    assert_eq!(window.scroll_of(Tool::Eraser), 9);
    assert_eq!(window.scroll_of(Tool::Line), 0);
    // The position is one: moving the window and changing the tool leaves it where it is.
    let n = 2;
    let screen = (1800, 1400);
    let before = window.rect(options_row_count(Tool::Pencil), screen, n, 16);
    window.drag_total(window.pos_u.expect("tras rect"), 40, 24, n);
    let moved = window.rect(options_row_count(Tool::Pencil), screen, n, 16);
    assert_eq!((moved.x - before.x, moved.y - before.y), (40, 24));
    for tool in TOOLS {
        window.right_click(Tool::Pencil, tool);
        window.right_click(tool, tool);
        window.right_click(Tool::Pencil, tool);
        assert_eq!(
            window.rect(options_row_count(tool), screen, n, 16),
            moved,
            "{tool:?}"
        );
    }
}

#[test]
fn row_8_dragging_the_title_bar_moves_the_window_by_steps_of_u_and_every_tool_finds_it_there() {
    for ppp in PPPS {
        let mut bench = Bench::new(ppp, Tool::Pencil, 0);
        let n = bench.n();
        let before = bench.rect();
        let (x, y) = bench.at(60, 8);
        let start = in_points(ppp, x, y);
        let end = in_points(ppp, x - 40 * n, y + 30 * n);
        bench.frame(vec![move_pointer(start)]);
        bench.frame(vec![button_of_2(start, PointerButton::Primary, true)]);
        bench.frame(vec![move_pointer(in_points(ppp, x - 20 * n, y + 10 * n))]);
        bench.frame(vec![move_pointer(end)]);
        bench.frame(vec![button_of_2(end, PointerButton::Primary, false)]);
        let after = bench.rect();
        assert_eq!(
            (after.x - before.x, after.y - before.y),
            (-40 * n, 30 * n),
            "ppp {ppp}"
        );
        // Another tool shows its content in the same place.
        for tool in TOOLS {
            bench.tool = tool;
            bench.frame(vec![]);
            assert_eq!(bench.rect(), after, "ppp {ppp}: {tool:?}");
        }
    }
}

#[test]
fn row_8_the_window_is_placed_again_inside_the_strip_when_the_window_or_the_scale_changes() {
    let mut window = OptionsWindow::default();
    let rows = 8;
    // Far to the bottom right, then a smaller window and another scale: always inside the strip.
    window.pos_u = Some((10_000, 10_000));
    for (screen, n) in [
        ((1800, 1400), 2u32),
        ((900, 700), 1),
        ((468, 313), 1),
        ((1404, 999), 3),
        ((960, 720), 1),
    ] {
        let rect = window.rect(rows, screen, n, 16);
        let band = float_window::band(screen, n);
        let k = i64::from(n);
        assert_eq!(
            (rect.width, rect.height),
            (224 * k, window_height_u(rows) * k)
        );
        assert!(rect.x >= 0 && rect.x + rect.width <= screen.0.max(rect.width));
        assert!(
            rect.y >= band.y
                && rect.y + rect.height <= (band.y + band.height).max(rect.y + rect.height)
        );
        assert_eq!((rect.x % k, (rect.y - band.y) % k), (0, 0));
        // Idempotent: placing the stored position gives the same rectangle.
        assert_eq!(window.rect(rows, screen, n, 16), rect);
        assert_eq!(window.current_rect(rows, screen, n, 16), rect);
    }
    // The window fits the minimum window (468 × 313 u) with eight rows (248 u).
    let n = 2;
    let screen = (468 * 2, 313 * 2);
    let rect = window.rect(rows, screen, n, 16);
    let band = float_window::band(screen, n);
    assert!(rect.y + rect.height <= band.y + band.height);
    assert_eq!(band.y, MENU_H_U * 2);
    assert_eq!(band.y + band.height, screen.1 - i64::from(bar_height_px(2)));
    // The first position: against the right of the canvas with 8 u of margin, 24 u to the left and 24
    // u lower than the layers panel (which is 8 u from the top).
    let mut fresh = OptionsWindow::default();
    let rect = fresh.rect(1, (1920, 1080), 1, 16);
    let canvas = px_ui::chrome::window_layout_cols((1920, 1080), 1, 16).canvas;
    assert_eq!(rect.x, canvas.width - 224 - 8 - 24);
    assert_eq!(rect.y, MENU_H_U + 8 + 24);
}

// ---------------------------------------------------------------------------------------------
// Row 9: it does not capture outside and it is not modal
// ---------------------------------------------------------------------------------------------

#[test]
fn row_9_outside_the_rectangle_everything_reaches_the_canvas_and_inside_nothing_does() {
    for ppp in PPPS {
        let mut bench = Bench::new(ppp, Tool::Pencil, 0);
        let n = bench.n();
        let rect = bench.rect();
        // The canvas is the whole screen here, with the image centered: a click on the image,
        // outside the window, starts a stroke.
        let zoom = bench.frame(vec![]).zoom;
        let image_x = window_px(ppp).0 / 2 - 100 * n;
        let image_y = window_px(ppp).1 / 2;
        assert!(
            image_x + 2 < rect.x || image_y > rect.y + rect.height,
            "el banco no deja la imagen bajo la ventana"
        );
        let outside = bench.click(image_x, image_y);
        assert!(
            started(&outside),
            "ppp {ppp}: fuera de la ventana el lienzo recibe el clic"
        );
        // Inside the rectangle no event reaches the canvas, not even over the image.
        for (dx, dy) in [
            (2, 2),
            (rect.width / 2, rect.height / 2),
            (rect.width - 3, rect.height - 3),
        ] {
            let inside = bench.click(rect.x + dx, rect.y + dy);
            assert!(
                !started(&inside),
                "ppp {ppp}: dentro de la ventana ({dx}, {dy})"
            );
        }
        // The wheel inside does not zoom the canvas; outside it does.
        let (cx, cy) = (rect.x + rect.width / 2, rect.y + rect.height / 2);
        bench.frame(vec![move_pointer(in_points(ppp, cx, cy))]);
        bench.frame(vec![wheel_line(true)]);
        assert_eq!(
            bench.frame(vec![]).zoom,
            zoom,
            "ppp {ppp}: la rueda dentro no hace zoom"
        );
        bench.frame(vec![move_pointer(in_points(ppp, image_x, image_y))]);
        bench.frame(vec![wheel_line(true)]);
        assert_ne!(
            bench.frame(vec![]).zoom,
            zoom,
            "ppp {ppp}: la rueda fuera sí"
        );
        // Opening, closing and moving the window produce no stroke by themselves.
        bench.state.close();
        assert!(bench.frame(vec![]).canvas.is_empty());
    }
}

#[test]
fn row_9_with_a_modal_or_a_dialog_the_window_does_not_respond_to_click_wheel_or_drag() {
    for ppp in PPPS {
        let mut bench = Bench::new(ppp, Tool::Pencil, 30);
        bench.enabled = false;
        let n = bench.n();
        let rect = bench.rect();
        let (x, y) = bench.at(215, 9);
        assert!(
            !bench
                .click(x, y)
                .iter()
                .any(|f| f.events.contains(&OptionsEvent::Close))
        );
        // The wheel over the body does not scroll it.
        let (bx, by) = bench.at(15, 49);
        bench.frame(vec![move_pointer(in_points(ppp, bx, by))]);
        bench.frame(vec![wheel_line(false)]);
        assert_eq!(bench.state.scroll_of(Tool::Pencil), 0, "ppp {ppp}");
        // Dragging the title bar does not move it.
        let (tx, ty) = bench.at(60, 8);
        let start = in_points(ppp, tx, ty);
        bench.frame(vec![move_pointer(start)]);
        bench.frame(vec![button_of_2(start, PointerButton::Primary, true)]);
        bench.frame(vec![move_pointer(in_points(ppp, tx - 30 * n, ty + 30 * n))]);
        bench.frame(vec![button_of_2(
            in_points(ppp, tx - 30 * n, ty + 30 * n),
            PointerButton::Primary,
            false,
        )]);
        assert_eq!(bench.rect(), rect, "ppp {ppp}");
    }
}

// ---------------------------------------------------------------------------------------------
// Row 11: F1, tooltips and texts
// ---------------------------------------------------------------------------------------------

#[test]
fn row_11_f1_over_the_window_asks_for_the_help_of_the_active_tool_and_elsewhere_it_does_not() {
    let rect = ScreenRect {
        x: 300,
        y: 120,
        width: 224,
        height: 122,
    };
    for point in [(300, 120), (523, 241), (400, 200)] {
        assert_eq!(
            help_target_in_options(point, rect),
            Some(HelpTarget::Active)
        );
    }
    for point in [(299, 150), (524, 150), (400, 119), (400, 242), (-5, -5)] {
        assert_eq!(help_target_in_options(point, rect), None, "{point:?}");
    }
}

#[test]
fn row_11_no_visible_text_calls_the_window_a_box_or_an_options_panel() {
    let mut texts: Vec<String> = Vec::new();
    for entry in help_table() {
        texts.push(entry.name.to_string());
        texts.push(entry.description.to_string());
        texts.extend(entry.mouse_left.map(str::to_string));
        texts.extend(entry.mouse_right.map(str::to_string));
        texts.extend(entry.options.iter().map(|(_, t)| (*t).to_string()));
    }
    for tool in TOOLS {
        texts.push(brief_help(tool));
        texts.push(title(tool).to_string());
    }
    for text in texts {
        let lower = text.to_lowercase();
        assert!(!lower.contains("recuadro"), "{text:?}");
        assert!(!lower.contains("panel de opciones"), "{text:?}");
    }
}

// ---------------------------------------------------------------------------------------------
// Property 3: it fits, it is remembered and it does not capture
// ---------------------------------------------------------------------------------------------

#[derive(Clone, Debug)]
enum Op {
    RightClick(usize),
    SwitchTool(usize),
    Close,
    Drag(i64, i64),
    Scroll(usize, i64),
    Resize(i64, i64),
    Scale(u32),
}

fn op() -> impl Strategy<Value = Op> {
    prop_oneof![
        3 => (0usize..5).prop_map(Op::RightClick),
        2 => (0usize..5).prop_map(Op::SwitchTool),
        1 => Just(Op::Close),
        2 => (-400i64..=400, -300i64..=300).prop_map(|(dx, dy)| Op::Drag(dx, dy)),
        2 => (0usize..5, -12i64..=12).prop_map(|(t, d)| Op::Scroll(t, d)),
        1 => (-400i64..=900, -300i64..=700).prop_map(|(w, h)| Op::Resize(w, h)),
        1 => (1u32..=3).prop_map(Op::Scale),
    ]
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(256))]

    #[test]
    fn property_3_the_rectangle_is_whole_and_inside_the_strip_the_state_follows_one_window_and_the_position_and_scroll_are_kept(
        ops in prop::collection::vec(op(), 0..30),
        rows in prop::sample::select(vec![0usize, 1, 8, 9, 30]),
    ) {
        let mut window = OptionsWindow::default();
        let mut screen = (960i64, 720i64);
        let mut n = 1u32;
        let mut active = Tool::Pencil;
        // The model: one window that follows the active tool, and a scroll per tool.
        let mut open = false;
        let mut scroll = [0usize; 5];
        for op in ops {
            let k = i64::from(n);
            let pos_before = window.pos_u;
            match op {
                Op::RightClick(t) => {
                    let clicked = TOOLS[t];
                    open = !(open && active == clicked);
                    window.right_click(active, clicked);
                    active = clicked;
                    // Choosing a tool never moves the window.
                    prop_assert_eq!(window.pos_u, pos_before);
                }
                Op::SwitchTool(t) => {
                    active = TOOLS[t];
                    prop_assert_eq!(window.pos_u, pos_before);
                }
                Op::Close => {
                    open = false;
                    window.close();
                }
                Op::Drag(dx, dy) => {
                    let from = window.rect(rows, screen, n, 16);
                    let from_u = window.pos_u.expect("tras rect");
                    window.drag_total(from_u, dx, dy, n);
                    let to = window.rect(rows, screen, n, 16);
                    // Moves by whole `u` and never leaves the strip.
                    prop_assert_eq!((to.x - from.x) % k, 0);
                    prop_assert_eq!((to.y - from.y) % k, 0);
                }
                Op::Scroll(t, delta) => {
                    let tool = TOOLS[t];
                    window.scroll_by(tool, delta, rows);
                    let max = options_max_scroll(rows) as i64;
                    scroll[t] = (scroll[t] as i64 + delta).clamp(0, max) as usize;
                }
                Op::Resize(dw, dh) => {
                    screen = ((960 + dw).max(100), (720 + dh).max(100));
                }
                Op::Scale(m) => n = m,
            }
            let k = i64::from(n);
            prop_assert_eq!(window.open, open);
            for (i, tool) in TOOLS.iter().enumerate() {
                prop_assert_eq!(window.scroll_of(*tool), scroll[i].min(options_max_scroll(rows)));
            }
            // Whole, inside the strip when the strip is large enough, and idempotent.
            let rect = window.rect(rows, screen, n, 16);
            let band = float_window::band(screen, n);
            prop_assert_eq!(rect.width, 224 * k);
            prop_assert_eq!(rect.height, window_height_u(rows) * k);
            prop_assert_eq!((rect.x % k, (rect.y - band.y) % k), (0, 0));
            if band.height >= rect.height && screen.0 >= rect.width {
                prop_assert!(rect.x >= 0 && rect.x + rect.width <= screen.0);
                prop_assert!(rect.y >= band.y && rect.y + rect.height <= band.y + band.height);
            }
            prop_assert_eq!(window.rect(rows, screen, n, 16), rect);
            // A point outside the rectangle is never inside it (the window captures nothing else).
            for (px, py) in [(rect.x - 1, rect.y), (rect.x + rect.width, rect.y), (rect.x, rect.y - 1), (rect.x, rect.y + rect.height)] {
                let inside = px >= rect.x && px < rect.x + rect.width && py >= rect.y && py < rect.y + rect.height;
                prop_assert!(!inside);
                prop_assert_eq!(help_target_in_options((px, py), rect), None);
            }
        }
    }
}
