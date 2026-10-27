//! Tests of the brush strokes in `ToolState` (specs/hito-4-pincel.md, unit 4.3): rows 6 to 10 and
//! the part of row 11 that lives in `px-ui` (size keys and rows of the options window).
//!
//! Red phase: `ToolState::{brush, set_brush, spacing, set_spacing, set_modifiers, dotted, curve,
//! step_brush_size, option_rows, option_text, click_option}`, `ToolAction::{SelectDotted,
//! SelectCurve}` and `OptionRow` do not exist yet, so this file does not compile until the logic
//! of `px-ui`.

use std::collections::BTreeSet;

use px_core::{Document, Image, ImageId, Mode, Pixel, Rgba};
use px_raster::{Brush, BrushShape, Line, Spacing, curve_path, spaced_centers, stamp_pixels};
use px_ui::canvas::CanvasEvent;
use px_ui::options::options_row_count;
use px_ui::tool::{OptionRow, Tool, ToolAction, ToolState};

const SIZE: u32 = 40;

fn red() -> Pixel {
    Pixel::Rgba(Rgba::opaque(255, 0, 0))
}

struct Scene {
    doc: Document,
    id: ImageId,
    tool: ToolState,
}

impl Scene {
    fn new(action: ToolAction) -> Scene {
        let doc = Document::new(Image::new(SIZE, SIZE, Mode::Rgba).expect("valid image"));
        let mut tool = ToolState::new();
        tool.apply_action(action);
        Scene {
            doc,
            id: ImageId::from_raw(0),
            tool,
        }
    }

    fn ev(&mut self, event: CanvasEvent) -> bool {
        self.tool
            .canvas_event_with(&mut self.doc, self.id, Some(red()), event)
            .unwrap_or_else(|e| panic!("{event:?} gave {e:?}"))
    }

    fn begin(&mut self, x: i32, y: i32) {
        self.ev(CanvasEvent::StrokeBegin((x, y)));
    }

    fn mov(&mut self, x: i32, y: i32) {
        self.ev(CanvasEvent::StrokeMove((x, y)));
    }

    fn end(&mut self) {
        self.ev(CanvasEvent::StrokeEnd);
    }

    /// The painted pixels of the image.
    fn painted(&self) -> BTreeSet<(u32, u32)> {
        let image = self.doc.image(self.id).expect("image");
        (0..SIZE)
            .flat_map(|y| (0..SIZE).map(move |x| (x, y)))
            .filter(|&(x, y)| image.get_pixel(x, y) == Ok(red()))
            .collect()
    }

    fn preview(&self) -> BTreeSet<(u32, u32)> {
        self.tool.preview().map(|e| (e.x, e.y)).collect()
    }
}

fn pixels(points: &[(u32, u32)]) -> BTreeSet<(u32, u32)> {
    points.iter().copied().collect()
}

fn square(n: u32) -> Brush {
    Brush::new(BrushShape::Square, n)
}

fn row(from: (u32, u32), to: (u32, u32)) -> BTreeSet<(u32, u32)> {
    let mut out = BTreeSet::new();
    for y in from.1..=to.1 {
        for x in from.0..=to.0 {
            out.insert((x, y));
        }
    }
    out
}

// ---------------------------------------------------------------------------------------------
// Row 6: freehand
// ---------------------------------------------------------------------------------------------

#[test]
fn row_06_dotted_stamps_only_the_samples_and_continuous_joins_them() {
    let mut dotted = Scene::new(ToolAction::SelectDotted);
    dotted.tool.set_brush(square(3));
    dotted.begin(5, 5);
    dotted.mov(15, 5);
    dotted.end();
    let mut expected = row((4, 4), (6, 6));
    expected.extend(row((14, 4), (16, 6)));
    assert_eq!(dotted.painted(), expected);

    let mut continuous = Scene::new(ToolAction::Select(Tool::Pencil));
    continuous.tool.set_brush(square(3));
    continuous.begin(5, 5);
    continuous.mov(15, 5);
    continuous.end();
    assert_eq!(continuous.painted(), row((4, 4), (16, 6)));
}

#[test]
fn row_06_continuous_size_1_is_the_pencil_of_today() {
    let mut scene = Scene::new(ToolAction::Select(Tool::Pencil));
    scene.begin(2, 2);
    scene.mov(8, 5);
    scene.mov(3, 9);
    scene.end();
    let mut expected: BTreeSet<(u32, u32)> = BTreeSet::new();
    for (a, b) in [((2, 2), (8, 5)), ((8, 5), (3, 9))] {
        expected.extend(Line::new(a, b).map(|(x, y)| (x as u32, y as u32)));
    }
    assert_eq!(scene.painted(), expected);
}

#[test]
fn row_06_one_undo_step_and_the_preview_is_what_gets_applied() {
    let mut scene = Scene::new(ToolAction::Select(Tool::Pencil));
    scene.tool.set_brush(Brush::new(BrushShape::Round, 3));
    scene.begin(10, 10);
    scene.mov(20, 14);
    scene.mov(25, 30);
    let preview = scene.preview();
    assert!(!preview.is_empty());
    assert!(scene.painted().is_empty(), "nothing before the end");
    scene.end();
    assert_eq!(scene.painted(), preview);
    assert_eq!(scene.doc.undo(), Ok(()));
    assert!(scene.painted().is_empty());
    assert!(!scene.doc.can_undo(), "the gesture was a single step");
}

// ---------------------------------------------------------------------------------------------
// Row 7: Shift
// ---------------------------------------------------------------------------------------------

#[test]
fn row_07_shift_projects_the_end_of_the_line_on_the_dominant_axis() {
    for (end, expected_end) in [
        ((17, 13), (17, 10)),
        ((13, 3), (10, 3)),
        ((15, 15), (15, 10)),
    ] {
        let mut scene = Scene::new(ToolAction::Select(Tool::Line));
        scene.tool.set_modifiers(true, false);
        scene.begin(10, 10);
        scene.mov(end.0, end.1);
        scene.end();
        let expected: BTreeSet<(u32, u32)> = Line::new((10, 10), expected_end)
            .map(|(x, y)| (x as u32, y as u32))
            .collect();
        assert_eq!(scene.painted(), expected, "end {end:?}");
    }
}

#[test]
fn row_07_freehand_locks_the_axis_at_the_first_movement() {
    let mut scene = Scene::new(ToolAction::Select(Tool::Pencil));
    scene.tool.set_modifiers(true, false);
    scene.begin(10, 10);
    scene.mov(12, 11);
    scene.mov(15, 18);
    scene.end();
    assert_eq!(scene.painted(), row((10, 10), (15, 10)));

    let mut free = Scene::new(ToolAction::Select(Tool::Pencil));
    free.begin(10, 10);
    free.mov(12, 11);
    free.mov(15, 18);
    free.end();
    assert!(
        free.painted().contains(&(15, 18)),
        "without Shift nothing changes"
    );
}

// ---------------------------------------------------------------------------------------------
// Row 8: Ctrl traces
// ---------------------------------------------------------------------------------------------

fn line_set(a: (i32, i32), b: (i32, i32)) -> BTreeSet<(u32, u32)> {
    Line::new(a, b).map(|(x, y)| (x as u32, y as u32)).collect()
}

#[test]
fn row_08_ctrl_leaves_every_intermediate_figure_and_without_it_only_the_last() {
    let ends = [(14, 10), (10, 14), (14, 14)];
    let mut traced = Scene::new(ToolAction::Select(Tool::Line));
    traced.tool.set_modifiers(false, true);
    traced.begin(10, 10);
    for (x, y) in ends {
        traced.mov(x, y);
    }
    traced.end();
    let mut expected = BTreeSet::new();
    for end in ends {
        expected.extend(line_set((10, 10), end));
    }
    assert_eq!(traced.painted(), expected);
    assert_eq!(traced.doc.undo(), Ok(()));
    assert!(!traced.doc.can_undo(), "a single undo step");

    let mut plain = Scene::new(ToolAction::Select(Tool::Line));
    plain.begin(10, 10);
    for (x, y) in ends {
        plain.mov(x, y);
    }
    plain.end();
    assert_eq!(plain.painted(), line_set((10, 10), (14, 14)));
}

#[test]
fn row_08_releasing_ctrl_midway_keeps_the_traces_already_left() {
    let mut scene = Scene::new(ToolAction::Select(Tool::Line));
    scene.tool.set_modifiers(false, true);
    scene.begin(10, 10);
    scene.mov(14, 10);
    scene.tool.set_modifiers(false, false);
    scene.mov(10, 14);
    scene.end();
    let mut expected = line_set((10, 10), (14, 10));
    expected.extend(line_set((10, 10), (10, 14)));
    assert_eq!(scene.painted(), expected);
}

// ---------------------------------------------------------------------------------------------
// Row 9: spacing in the gesture
// ---------------------------------------------------------------------------------------------

#[test]
fn row_09_line_with_every_n_and_total_n() {
    for (spacing, xs) in [
        (Spacing::EveryN(4), vec![10, 14, 18]),
        (Spacing::TotalN(3), vec![10, 15, 20]),
    ] {
        let mut scene = Scene::new(ToolAction::Select(Tool::Line));
        scene.tool.set_spacing(spacing);
        scene.begin(10, 10);
        scene.mov(20, 10);
        scene.end();
        let expected: BTreeSet<(u32, u32)> = xs.iter().map(|&x| (x, 10)).collect();
        assert_eq!(scene.painted(), expected, "{spacing:?}");
    }
}

#[test]
fn row_09_curve_with_every_n_uses_the_centers_of_the_curve_path() {
    let (a, b, c) = ((10, 10), (18, 10), (14, 18));
    let mut scene = Scene::new(ToolAction::SelectCurve);
    scene.tool.set_spacing(Spacing::EveryN(2));
    scene.begin(a.0, a.1);
    scene.mov(b.0, b.1);
    scene.end();
    scene.begin(14, 10);
    scene.mov(c.0, c.1);
    scene.end();
    let centers = spaced_centers(&curve_path(a, b, c), Spacing::EveryN(2));
    let expected = stamp_pixels(Brush::new(BrushShape::Round, 1), centers, SIZE, SIZE);
    assert_eq!(scene.painted(), pixels(&expected));
}

// ---------------------------------------------------------------------------------------------
// Row 10: the curve gesture
// ---------------------------------------------------------------------------------------------

fn curve_pixels(a: (i32, i32), b: (i32, i32), c: (i32, i32)) -> BTreeSet<(u32, u32)> {
    let stamped = stamp_pixels(
        Brush::new(BrushShape::Round, 1),
        curve_path(a, b, c),
        SIZE,
        SIZE,
    );
    pixels(&stamped)
}

#[test]
fn row_10_two_drags_make_one_curve_in_one_undo_step() {
    let (a, b, c) = ((10, 10), (18, 10), (14, 18));
    let mut scene = Scene::new(ToolAction::SelectCurve);
    scene.begin(a.0, a.1);
    scene.mov(b.0, b.1);
    assert!(!scene.preview().is_empty());
    scene.end();
    assert!(scene.painted().is_empty(), "the first drag changes nothing");
    assert!(!scene.doc.can_undo());
    assert!(
        !scene.preview().is_empty(),
        "the pending segment is previewed"
    );
    scene.begin(14, 10);
    scene.mov(c.0, c.1);
    assert_eq!(scene.preview(), curve_pixels(a, b, c));
    scene.end();
    assert_eq!(scene.painted(), curve_pixels(a, b, c));
    assert_eq!(scene.doc.undo(), Ok(()));
    assert!(scene.painted().is_empty());
    assert!(!scene.doc.can_undo());
}

#[test]
fn row_10_escape_and_a_tool_change_drop_the_pending_segment() {
    let mut scene = Scene::new(ToolAction::SelectCurve);
    scene.begin(10, 10);
    scene.mov(18, 10);
    scene.end();
    scene.tool.apply_action(ToolAction::Cancel);
    assert!(scene.preview().is_empty());
    scene.begin(14, 10);
    scene.mov(14, 18);
    scene.end();
    assert!(
        scene.painted().is_empty(),
        "after Esc the drag starts a new curve"
    );

    let mut other = Scene::new(ToolAction::SelectCurve);
    other.begin(10, 10);
    other.mov(18, 10);
    other.end();
    other.tool.apply_action(ToolAction::Select(Tool::Fill));
    other.tool.apply_action(ToolAction::SelectCurve);
    other.begin(14, 10);
    other.mov(14, 18);
    other.end();
    assert!(other.painted().is_empty(), "the change of tool dropped it");
    assert!(!other.doc.can_undo());
}

// ---------------------------------------------------------------------------------------------
// Row 11: size keys and rows of the options window
// ---------------------------------------------------------------------------------------------

#[test]
fn row_11_the_size_steps_by_one_inside_1_to_100_and_only_for_stroke_tools() {
    let mut state = ToolState::new();
    assert_eq!(state.brush(), Brush::new(BrushShape::Round, 1));
    assert!(!state.step_brush_size(-1), "already 1");
    assert!(state.step_brush_size(1));
    assert_eq!(state.brush().size(), 2);
    state.set_brush(Brush::new(BrushShape::Round, 100));
    assert!(!state.step_brush_size(1));
    assert_eq!(state.brush().size(), 100);
    for tool in [Tool::Fill, Tool::Dropper, Tool::Eraser] {
        state.apply_action(ToolAction::Select(tool));
        assert!(!state.step_brush_size(-1), "{tool:?}");
        assert_eq!(state.brush().size(), 100, "{tool:?}");
    }
    for action in [
        ToolAction::Select(Tool::Line),
        ToolAction::SelectDotted,
        ToolAction::SelectCurve,
    ] {
        state.apply_action(action);
        assert!(state.step_brush_size(-1));
    }
    assert_eq!(state.brush().size(), 97);
}

#[test]
fn row_11_dotted_and_curve_are_modes_of_the_pencil_and_line_buttons() {
    let mut state = ToolState::new();
    state.apply_action(ToolAction::SelectDotted);
    assert_eq!((state.tool(), state.dotted()), (Tool::Pencil, true));
    state.apply_action(ToolAction::Select(Tool::Pencil));
    assert_eq!((state.tool(), state.dotted()), (Tool::Pencil, false));
    state.apply_action(ToolAction::SelectCurve);
    assert_eq!((state.tool(), state.curve()), (Tool::Line, true));
    state.apply_action(ToolAction::Select(Tool::Line));
    assert_eq!((state.tool(), state.curve()), (Tool::Line, false));
}

#[test]
fn row_11_the_options_window_rows_per_tool() {
    let mut state = ToolState::new();
    assert_eq!(
        state.option_rows(),
        [OptionRow::Mode, OptionRow::Shape, OptionRow::Size]
    );
    assert_eq!(options_row_count(Tool::Pencil), 3);
    state.apply_action(ToolAction::Select(Tool::Line));
    assert_eq!(
        state.option_rows(),
        [
            OptionRow::Mode,
            OptionRow::Shape,
            OptionRow::Size,
            OptionRow::Spacing
        ]
    );
    assert_eq!(options_row_count(Tool::Line), 4);
    state.set_spacing(Spacing::EveryN(4));
    assert_eq!(state.option_rows().last(), Some(&OptionRow::SpacingN));
    for tool in [Tool::Fill, Tool::Dropper, Tool::Eraser] {
        state.apply_action(ToolAction::Select(tool));
        assert!(state.option_rows().is_empty(), "{tool:?}");
        assert_eq!(options_row_count(tool), 0, "{tool:?}");
    }
}

#[test]
fn row_11_row_texts_and_clicks() {
    let mut state = ToolState::new();
    assert_eq!(state.option_text(OptionRow::Mode), "Modo: Continuo");
    assert_eq!(state.option_text(OptionRow::Shape), "Forma: Redondo");
    assert_eq!(state.option_text(OptionRow::Size), "Tamaño: 1");
    // Shape cycles forward and backward through the ten.
    state.click_option(OptionRow::Shape, true, false);
    assert_eq!(state.brush().shape(), BrushShape::Square);
    state.click_option(OptionRow::Shape, false, false);
    state.click_option(OptionRow::Shape, false, false);
    assert_eq!(state.brush().shape(), BrushShape::Cross, "wraps around");
    // Size: ±1, ±10 with Shift, clamped.
    state.click_option(OptionRow::Size, true, false);
    state.click_option(OptionRow::Size, true, true);
    assert_eq!(state.option_text(OptionRow::Size), "Tamaño: 12");
    state.click_option(OptionRow::Size, false, true);
    state.click_option(OptionRow::Size, false, true);
    assert_eq!(state.brush().size(), 1);
    // Mode toggles; for the line it reads Recta / Curva.
    state.click_option(OptionRow::Mode, true, false);
    assert_eq!(state.option_text(OptionRow::Mode), "Modo: Punteado");
    state.apply_action(ToolAction::Select(Tool::Line));
    assert_eq!(state.option_text(OptionRow::Mode), "Modo: Recta");
    state.click_option(OptionRow::Mode, true, false);
    assert_eq!(state.option_text(OptionRow::Mode), "Modo: Curva");
    // Spacing cycles Continuo → Cada N → N en total, and `N` goes up and down.
    assert_eq!(state.option_text(OptionRow::Spacing), "Espaciado: Continuo");
    state.click_option(OptionRow::Spacing, true, false);
    assert_eq!(state.option_text(OptionRow::Spacing), "Espaciado: Cada N");
    assert_eq!(state.option_text(OptionRow::SpacingN), "N: 4");
    state.click_option(OptionRow::SpacingN, true, true);
    assert_eq!(state.option_text(OptionRow::SpacingN), "N: 14");
    state.click_option(OptionRow::Spacing, true, false);
    assert_eq!(
        state.option_text(OptionRow::Spacing),
        "Espaciado: N en total"
    );
    assert_eq!(state.spacing(), Spacing::TotalN(14));
    state.click_option(OptionRow::Spacing, true, false);
    assert_eq!(state.spacing(), Spacing::Continuous);
}
