//! Tests of the shapes in `ToolState` and of the box with six buttons (specs/hito-4-formas.md,
//! unit 4.4): rows 7 to 10 and the part of row 11 that lives in `px-ui`.
//!
//! Red phase: `Tool::Shape`, `ShapeKind`, `ShapeFill`, `ToolAction::{SelectShape, Finish}`,
//! `ToolState::{shape_kind, shape_fill, set_shape_fill, set_other_pixel}`, `OptionRow::{Kind,
//! Fill}` and the six-button box do not exist yet, so this file does not compile until the logic
//! of `px-ui`.

use std::collections::{BTreeMap, BTreeSet};

use px_core::{Command, Document, Image, ImageId, Mode, Pixel, Rgba, Span};
use px_raster::{
    Brush, BrushShape, Spacing, ellipse_ring, ellipse_spans, polygon_ring, polygon_spans,
    rect_ring, rect_spans, spaced_centers_closed, stamp_pixels,
};
use px_ui::actions::AppAction;
use px_ui::canvas::CanvasEvent;
use px_ui::chrome::{TOOL_LABELS, TOOLBOX_BUTTONS, head_u, tool_rows};
use px_ui::edit_layout::min_window_height_pt;
use px_ui::options::options_row_count;
use px_ui::tool::{OptionRow, ShapeFill, ShapeKind, Tool, ToolAction, ToolState};
use px_ui::toolbar::TOOL_BUTTONS;

const SIZE: u32 = 40;

fn red() -> Pixel {
    Pixel::Rgba(Rgba::opaque(255, 0, 0))
}

fn blue() -> Pixel {
    Pixel::Rgba(Rgba::opaque(0, 0, 255))
}

struct Scene {
    doc: Document,
    id: ImageId,
    tool: ToolState,
}

impl Scene {
    fn new(kind: ShapeKind, fill: ShapeFill) -> Scene {
        let doc = Document::new(Image::new(SIZE, SIZE, Mode::Rgba).expect("valid image"));
        let mut tool = ToolState::new();
        tool.apply_action(ToolAction::SelectShape(kind));
        tool.set_shape_fill(fill);
        tool.set_other_pixel(Some(blue()));
        Scene {
            doc,
            id: ImageId::from_raw(0),
            tool,
        }
    }

    fn ev(&mut self, event: CanvasEvent) {
        self.tool
            .canvas_event_with(&mut self.doc, self.id, Some(red()), event)
            .unwrap_or_else(|e| panic!("{event:?} gave {e:?}"));
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

    /// A whole gesture: press, moves and release.
    fn drag(&mut self, points: &[(i32, i32)]) {
        self.begin(points[0].0, points[0].1);
        for &(x, y) in &points[1..] {
            self.mov(x, y);
        }
        self.end();
    }

    /// A click: press and release at the same place.
    fn click(&mut self, x: i32, y: i32) {
        self.drag(&[(x, y)]);
    }

    fn shift(&mut self, on: bool) {
        self.tool.set_modifiers(on, false);
    }

    /// The painted pixels of the image with their colors.
    fn painted(&self) -> BTreeMap<(u32, u32), Pixel> {
        let image = self.doc.image(self.id).expect("image");
        let mut out = BTreeMap::new();
        for y in 0..SIZE {
            for x in 0..SIZE {
                let pixel = image.get_pixel(x, y).expect("pixel");
                if pixel != Pixel::Rgba(Rgba::TRANSPARENT) {
                    out.insert((x, y), pixel);
                }
            }
        }
        out
    }

    fn preview(&self) -> BTreeMap<(u32, u32), Pixel> {
        self.tool.preview().map(|e| ((e.x, e.y), e.pixel)).collect()
    }
}

fn spans_set(spans: &[Span]) -> BTreeSet<(u32, u32)> {
    spans
        .iter()
        .flat_map(|s| (s.x_start..s.x_end).map(move |x| (x, s.y)))
        .collect()
}

fn ring_set(ring: &[(i32, i32)]) -> BTreeSet<(u32, u32)> {
    ring.iter().map(|&(x, y)| (x as u32, y as u32)).collect()
}

fn paint(set: &BTreeSet<(u32, u32)>, pixel: Pixel) -> BTreeMap<(u32, u32), Pixel> {
    set.iter().map(|&p| (p, pixel)).collect()
}

/// The model of «outline, fill or both»: the fill in `blue` (red if only the fill) and the outline
/// in red on top.
fn model(
    fill: ShapeFill,
    fill_set: &BTreeSet<(u32, u32)>,
    ring: &[(i32, i32)],
    brush: Brush,
    spacing: Spacing,
) -> BTreeMap<(u32, u32), Pixel> {
    let mut out = BTreeMap::new();
    if fill != ShapeFill::Outline {
        let color = if fill == ShapeFill::Both { blue() } else { red() };
        out.extend(paint(fill_set, color));
    }
    if fill != ShapeFill::Filled {
        let centers = spaced_centers_closed(ring, spacing);
        let stamped: BTreeSet<(u32, u32)> = stamp_pixels(brush, centers, SIZE, SIZE)
            .into_iter()
            .collect();
        out.extend(paint(&stamped, red()));
    }
    out
}

const ALL_FILLS: [ShapeFill; 3] = [ShapeFill::Outline, ShapeFill::Filled, ShapeFill::Both];

fn one_brush() -> Brush {
    Brush::new(BrushShape::Round, 1)
}

// ---------------------------------------------------------------------------------------------
// Row 7: rectangle, circle and ellipse gestures
// ---------------------------------------------------------------------------------------------

#[test]
fn row_07_rectangle_outline_fill_and_both() {
    for fill in ALL_FILLS {
        let mut scene = Scene::new(ShapeKind::Rectangle, fill);
        scene.drag(&[(5, 5), (9, 8), (12, 9)]);
        let fill_set = spans_set(&rect_spans((5, 5), (12, 9), SIZE, SIZE));
        let ring = rect_ring((5, 5), (12, 9), SIZE, SIZE, 1);
        assert_eq!(
            scene.painted(),
            model(fill, &fill_set, &ring, one_brush(), Spacing::Continuous),
            "{fill:?}"
        );
    }
}

#[test]
fn row_07_both_paints_the_interior_with_the_other_slot_and_the_outline_on_top() {
    let mut scene = Scene::new(ShapeKind::Rectangle, ShapeFill::Both);
    scene.drag(&[(5, 5), (12, 9)]);
    let painted = scene.painted();
    assert_eq!(painted[&(5, 5)], red());
    assert_eq!(painted[&(12, 9)], red());
    assert_eq!(painted[&(8, 7)], blue());
    assert_eq!(painted.len(), 8 * 5);
    // Without a color for the other slot, the fill uses the same color.
    let mut same = Scene::new(ShapeKind::Rectangle, ShapeFill::Both);
    same.tool.set_other_pixel(None);
    same.drag(&[(5, 5), (12, 9)]);
    assert!(same.painted().values().all(|&p| p == red()));
}

#[test]
fn row_07_circle_and_ellipse_from_the_center() {
    for fill in ALL_FILLS {
        let mut circle = Scene::new(ShapeKind::Circle, fill);
        circle.drag(&[(20, 20), (23, 24)]);
        let fill_set = spans_set(&ellipse_spans((20, 20), 5, 5, SIZE, SIZE));
        let ring = ellipse_ring((20, 20), 5, 5, SIZE, SIZE, 1);
        assert_eq!(
            circle.painted(),
            model(fill, &fill_set, &ring, one_brush(), Spacing::Continuous),
            "círculo {fill:?}"
        );
        let mut ellipse = Scene::new(ShapeKind::Ellipse, fill);
        ellipse.drag(&[(20, 20), (24, 22)]);
        let fill_set = spans_set(&ellipse_spans((20, 20), 4, 2, SIZE, SIZE));
        let ring = ellipse_ring((20, 20), 4, 2, SIZE, SIZE, 1);
        assert_eq!(
            ellipse.painted(),
            model(fill, &fill_set, &ring, one_brush(), Spacing::Continuous),
            "elipse {fill:?}"
        );
    }
}

#[test]
fn row_07_shift_makes_a_square_and_a_circle() {
    let mut rect = Scene::new(ShapeKind::Rectangle, ShapeFill::Filled);
    rect.shift(true);
    rect.drag(&[(5, 5), (12, 9)]);
    assert_eq!(
        rect.painted(),
        paint(&spans_set(&rect_spans((5, 5), (12, 12), SIZE, SIZE)), red()),
        "cuadrado de lado 7 hacia abajo y a la derecha"
    );
    let mut up = Scene::new(ShapeKind::Rectangle, ShapeFill::Filled);
    up.shift(true);
    up.drag(&[(20, 20), (17, 10)]);
    assert_eq!(
        up.painted(),
        paint(&spans_set(&rect_spans((20, 20), (10, 10), SIZE, SIZE)), red()),
        "el lado es el mayor desplazamiento, en el sentido del arrastre"
    );
    let mut ellipse = Scene::new(ShapeKind::Ellipse, ShapeFill::Filled);
    ellipse.shift(true);
    ellipse.drag(&[(20, 20), (24, 22)]);
    assert_eq!(
        ellipse.painted(),
        paint(&spans_set(&ellipse_spans((20, 20), 4, 4, SIZE, SIZE)), red())
    );
    // The circle ignores Shift.
    let mut circle = Scene::new(ShapeKind::Circle, ShapeFill::Filled);
    circle.shift(true);
    circle.drag(&[(20, 20), (23, 24)]);
    assert_eq!(
        circle.painted(),
        paint(&spans_set(&ellipse_spans((20, 20), 5, 5, SIZE, SIZE)), red())
    );
}

#[test]
fn row_07_one_undo_step_and_the_preview_is_what_gets_applied() {
    for fill in ALL_FILLS {
        let mut scene = Scene::new(ShapeKind::Ellipse, fill);
        scene.begin(20, 20);
        scene.mov(25, 22);
        scene.mov(24, 23);
        let preview = scene.preview();
        assert!(!preview.is_empty());
        assert!(scene.painted().is_empty(), "nada en el documento hasta soltar");
        scene.end();
        assert_eq!(scene.painted(), preview, "{fill:?}");
        assert!(scene.preview().is_empty());
        assert!(scene.doc.undo().is_ok());
        assert!(scene.painted().is_empty(), "un paso deshace la forma entera");
        assert!(!scene.doc.can_undo(), "solo había un paso");
    }
}

#[test]
fn row_07_escape_and_a_gesture_without_size_in_the_corner_case() {
    let mut scene = Scene::new(ShapeKind::Rectangle, ShapeFill::Both);
    scene.begin(5, 5);
    scene.mov(12, 9);
    scene.tool.apply_action(ToolAction::Cancel);
    scene.end();
    assert!(scene.painted().is_empty());
    // A click without moving draws the one-pixel rectangle.
    let mut dot = Scene::new(ShapeKind::Rectangle, ShapeFill::Outline);
    dot.click(7, 7);
    assert_eq!(dot.painted(), paint(&BTreeSet::from([(7, 7)]), red()));
}

// ---------------------------------------------------------------------------------------------
// Row 8: brush and spacing on the outline
// ---------------------------------------------------------------------------------------------

#[test]
fn row_08_the_outline_uses_the_brush_and_the_spacing() {
    let brush = Brush::new(BrushShape::Round, 3);
    for spacing in [
        Spacing::Continuous,
        Spacing::EveryN(4),
        Spacing::TotalN(5),
    ] {
        let mut scene = Scene::new(ShapeKind::Rectangle, ShapeFill::Outline);
        scene.tool.set_brush(brush);
        scene.tool.set_spacing(spacing);
        scene.drag(&[(10, 10), (24, 20)]);
        let ring = rect_ring((10, 10), (24, 20), SIZE, SIZE, 3);
        assert_eq!(
            scene.painted(),
            model(ShapeFill::Outline, &BTreeSet::new(), &ring, brush, spacing),
            "{spacing:?}"
        );
    }
    let mut five = Scene::new(ShapeKind::Circle, ShapeFill::Outline);
    five.tool.set_brush(Brush::new(BrushShape::Square, 1));
    five.tool.set_spacing(Spacing::TotalN(5));
    five.drag(&[(20, 20), (26, 20)]);
    assert_eq!(five.painted().len(), 5, "N en total = cinco sellos de un píxel");
}

#[test]
fn row_08_the_fill_ignores_the_brush() {
    let mut plain = Scene::new(ShapeKind::Circle, ShapeFill::Filled);
    plain.drag(&[(20, 20), (26, 20)]);
    let mut brushed = Scene::new(ShapeKind::Circle, ShapeFill::Filled);
    brushed.tool.set_brush(Brush::new(BrushShape::Square, 9));
    brushed.tool.set_spacing(Spacing::EveryN(7));
    brushed.drag(&[(20, 20), (26, 20)]);
    assert_eq!(plain.painted(), brushed.painted());
}

// ---------------------------------------------------------------------------------------------
// Row 9: polygon
// ---------------------------------------------------------------------------------------------

const TRIANGLE: [(i32, i32); 3] = [(5, 5), (15, 5), (5, 15)];

fn polygon_model(
    fill: ShapeFill,
    vertices: &[(i32, i32)],
) -> BTreeMap<(u32, u32), Pixel> {
    model(
        fill,
        &spans_set(&polygon_spans(vertices, SIZE, SIZE)),
        &polygon_ring(vertices, SIZE, SIZE, 1),
        one_brush(),
        Spacing::Continuous,
    )
}

#[test]
fn row_09_clicks_then_finish_apply_one_step() {
    for fill in ALL_FILLS {
        let mut scene = Scene::new(ShapeKind::Polygon, fill);
        for &(x, y) in &TRIANGLE {
            scene.click(x, y);
        }
        assert!(scene.painted().is_empty(), "pendiente: el documento no cambia");
        assert!(!scene.preview().is_empty(), "la vista previa es el polígono");
        scene.tool.apply_action(ToolAction::Finish);
        assert_eq!(scene.painted(), polygon_model(fill, &TRIANGLE), "{fill:?}");
        assert!(scene.preview().is_empty());
        assert!(scene.doc.undo().is_ok());
        assert!(scene.painted().is_empty());
        assert!(!scene.doc.can_undo());
    }
}

#[test]
fn row_09_pressing_the_first_corner_again_closes_it() {
    let mut scene = Scene::new(ShapeKind::Polygon, ShapeFill::Filled);
    for &(x, y) in &TRIANGLE {
        scene.click(x, y);
    }
    scene.click(5, 5);
    assert_eq!(
        scene.painted(),
        polygon_model(ShapeFill::Filled, &TRIANGLE)
    );
    // With only two corners, pressing the first does not close (it is not a polygon yet).
    let mut two = Scene::new(ShapeKind::Polygon, ShapeFill::Filled);
    two.click(5, 5);
    two.click(15, 5);
    two.click(5, 5);
    assert!(two.painted().is_empty());
}

#[test]
fn row_09_escape_tool_change_and_kind_change_discard_the_pending_polygon() {
    for discard in 0..3 {
        let mut scene = Scene::new(ShapeKind::Polygon, ShapeFill::Outline);
        scene.click(5, 5);
        scene.click(15, 5);
        scene.click(5, 15);
        match discard {
            0 => scene.tool.apply_action(ToolAction::Cancel),
            1 => scene.tool.apply_action(ToolAction::Select(Tool::Pencil)),
            _ => scene
                .tool
                .apply_action(ToolAction::SelectShape(ShapeKind::Rectangle)),
        }
        assert!(scene.preview().is_empty(), "descartado ({discard})");
        scene.tool.apply_action(ToolAction::SelectShape(ShapeKind::Polygon));
        scene.tool.apply_action(ToolAction::Finish);
        assert!(scene.painted().is_empty(), "no queda nada ({discard})");
        assert!(!scene.doc.can_undo());
    }
}

#[test]
fn row_09_finish_with_fewer_than_two_corners_does_nothing_and_two_make_a_line() {
    let mut none = Scene::new(ShapeKind::Polygon, ShapeFill::Outline);
    none.tool.apply_action(ToolAction::Finish);
    assert!(none.painted().is_empty());
    let mut one = Scene::new(ShapeKind::Polygon, ShapeFill::Outline);
    one.click(5, 5);
    one.tool.apply_action(ToolAction::Finish);
    assert!(one.painted().is_empty());
    assert!(!one.doc.can_undo());
    let mut two = Scene::new(ShapeKind::Polygon, ShapeFill::Outline);
    two.click(5, 5);
    two.click(15, 9);
    two.tool.apply_action(ToolAction::Finish);
    assert_eq!(
        two.painted(),
        polygon_model(ShapeFill::Outline, &[(5, 5), (15, 9)])
    );
}

#[test]
fn row_09_a_dragged_corner_is_where_it_is_released_and_shift_snaps_to_an_axis() {
    let mut scene = Scene::new(ShapeKind::Polygon, ShapeFill::Outline);
    scene.click(5, 5);
    scene.drag(&[(15, 5), (20, 7), (25, 5)]);
    scene.click(5, 15);
    scene.tool.apply_action(ToolAction::Finish);
    assert_eq!(
        scene.painted(),
        polygon_model(ShapeFill::Outline, &[(5, 5), (25, 5), (5, 15)])
    );
    let mut shifted = Scene::new(ShapeKind::Polygon, ShapeFill::Outline);
    shifted.click(5, 5);
    shifted.shift(true);
    shifted.click(15, 8);
    shifted.click(16, 30);
    shifted.shift(false);
    shifted.tool.apply_action(ToolAction::Finish);
    assert_eq!(
        shifted.painted(),
        polygon_model(ShapeFill::Outline, &[(5, 5), (15, 5), (15, 30)]),
        "cada esquina con Mayús va al eje del mayor desplazamiento desde la anterior"
    );
}

// ---------------------------------------------------------------------------------------------
// Row 10: freeform
// ---------------------------------------------------------------------------------------------

#[test]
fn row_10_freeform_closes_with_a_straight_line_on_release() {
    for fill in ALL_FILLS {
        let mut scene = Scene::new(ShapeKind::Freeform, fill);
        scene.drag(&[(5, 5), (10, 5), (15, 5), (15, 10), (15, 15)]);
        let path = [(5, 5), (10, 5), (15, 5), (15, 10), (15, 15)];
        assert_eq!(scene.painted(), polygon_model(fill, &path), "{fill:?}");
        assert!(scene.doc.undo().is_ok());
        assert!(!scene.doc.can_undo(), "un paso");
    }
}

#[test]
fn row_10_a_crossing_path_fills_even_odd() {
    let mut scene = Scene::new(ShapeKind::Freeform, ShapeFill::Filled);
    let path = [(5, 5), (15, 15), (15, 5), (5, 15)];
    scene.drag(&path);
    assert_eq!(scene.painted(), polygon_model(ShapeFill::Filled, &path));
    assert!(!scene.painted().contains_key(&(5, 10)), "el hueco lateral está vacío");
}

#[test]
fn row_10_the_freeform_preview_is_the_closed_shape() {
    let mut scene = Scene::new(ShapeKind::Freeform, ShapeFill::Filled);
    scene.begin(5, 5);
    scene.mov(15, 5);
    scene.mov(15, 15);
    let preview = scene.preview();
    scene.end();
    assert_eq!(scene.painted(), preview);
}

// ---------------------------------------------------------------------------------------------
// Row 11: selection, options and the box
// ---------------------------------------------------------------------------------------------

#[test]
fn row_11_a_kind_key_sets_the_kind_and_keeps_the_fill_and_the_button_keeps_the_kind() {
    let mut state = ToolState::new();
    assert_eq!(state.shape_kind(), ShapeKind::Rectangle);
    assert_eq!(state.shape_fill(), ShapeFill::Outline);
    state.set_shape_fill(ShapeFill::Both);
    for kind in [
        ShapeKind::Circle,
        ShapeKind::Ellipse,
        ShapeKind::Polygon,
        ShapeKind::Freeform,
        ShapeKind::Rectangle,
    ] {
        state.apply_action(ToolAction::SelectShape(kind));
        assert_eq!(state.tool(), Tool::Shape);
        assert_eq!(state.shape_kind(), kind);
        assert_eq!(state.shape_fill(), ShapeFill::Both);
    }
    state.apply_action(ToolAction::SelectShape(ShapeKind::Ellipse));
    state.apply_action(ToolAction::Select(Tool::Eraser));
    state.apply_action(ToolAction::Select(Tool::Shape));
    assert_eq!(state.shape_kind(), ShapeKind::Ellipse, "el botón conserva el tipo");
    assert_eq!(state.shape_fill(), ShapeFill::Both);
}

#[test]
fn row_11_the_option_rows_depend_on_the_fill_and_the_spacing() {
    let mut state = ToolState::new();
    state.apply_action(ToolAction::Select(Tool::Shape));
    use OptionRow::{Fill, Kind, Shape, Size, Spacing as SpacingRow, SpacingN};
    assert_eq!(
        state.option_rows(),
        vec![Kind, Fill, Shape, Size, SpacingRow]
    );
    assert_eq!(options_row_count(Tool::Shape), 5);
    state.set_spacing(Spacing::EveryN(4));
    assert_eq!(
        state.option_rows(),
        vec![Kind, Fill, Shape, Size, SpacingRow, SpacingN]
    );
    state.set_shape_fill(ShapeFill::Both);
    assert_eq!(state.option_rows().len(), 6, "con ambos hay contorno");
    state.set_shape_fill(ShapeFill::Filled);
    assert_eq!(state.option_rows(), vec![Kind, Fill], "solo relleno: sin pincel");
}

#[test]
fn row_11_clicks_on_the_kind_and_fill_rows_cycle_and_the_texts_say_so() {
    let mut state = ToolState::new();
    state.apply_action(ToolAction::Select(Tool::Shape));
    let kinds = [
        (ShapeKind::Rectangle, "Tipo: Rectángulo"),
        (ShapeKind::Circle, "Tipo: Círculo"),
        (ShapeKind::Ellipse, "Tipo: Elipse"),
        (ShapeKind::Polygon, "Tipo: Polígono"),
        (ShapeKind::Freeform, "Tipo: Forma libre"),
    ];
    for (i, (kind, text)) in kinds.iter().enumerate() {
        assert_eq!(state.shape_kind(), *kind);
        assert_eq!(state.option_text(OptionRow::Kind), *text);
        state.click_option(OptionRow::Kind, true, false);
        assert_eq!(state.shape_kind(), kinds[(i + 1) % 5].0);
    }
    state.click_option(OptionRow::Kind, false, false);
    assert_eq!(state.shape_kind(), ShapeKind::Freeform, "el derecho va hacia atrás");
    let fills = [
        (ShapeFill::Outline, "Relleno: Contorno"),
        (ShapeFill::Filled, "Relleno: Solo relleno"),
        (ShapeFill::Both, "Relleno: Ambos"),
    ];
    for (i, (fill, text)) in fills.iter().enumerate() {
        assert_eq!(state.shape_fill(), *fill);
        assert_eq!(state.option_text(OptionRow::Fill), *text);
        state.click_option(OptionRow::Fill, true, false);
        assert_eq!(state.shape_fill(), fills[(i + 1) % 3].0);
    }
    state.click_option(OptionRow::Fill, false, false);
    assert_eq!(state.shape_fill(), ShapeFill::Both);
}

#[test]
fn row_11_shift_steps_the_size_of_the_shape_brush_by_ten() {
    let mut state = ToolState::new();
    state.apply_action(ToolAction::Select(Tool::Shape));
    state.click_option(OptionRow::Size, true, true);
    assert_eq!(state.brush().size(), 11);
    assert!(state.step_brush_size(1), "+ y - también valen con las formas");
    assert_eq!(state.brush().size(), 12);
}

#[test]
fn row_11_the_box_has_six_buttons_and_the_minimum_window_does_not_grow() {
    assert_eq!(TOOL_BUTTONS.len(), 6);
    let last = TOOL_BUTTONS[5];
    assert_eq!(last.label, "Formas");
    assert_eq!(last.action, AppAction::SelectTool(Tool::Shape));
    assert_eq!(TOOL_LABELS.len(), 6);
    assert_eq!(TOOL_LABELS[5], "Formas");
    assert_eq!(TOOLBOX_BUTTONS, 10);
    assert_eq!(Tool::Shape.label(), "formas");
    // c(10) = 5 per row: two rows; from 11 columns, one row.
    assert_eq!(tool_rows(6, 10), 2);
    for cols in 11..=40 {
        assert_eq!(tool_rows(6, cols), 1, "{cols} columnas");
    }
    assert_eq!(head_u(6, 10), 109);
    // The capacity rule (20 tools, 159 u) already covers it: the minimum height is the layers
    // panel's, 313 u, at every scale.
    for (ppp, pt) in [
        (1.0_f32, 313.0_f32),
        (1.25, 250.4),
        (1.5, 417.33),
        (2.0, 313.0),
        (3.0, 313.0),
    ] {
        assert!((min_window_height_pt(ppp) - pt).abs() < 0.01, "ppp {ppp}");
        assert!(min_window_height_pt(ppp) <= 432.0, "techo de 432 pt");
    }
    assert!(head_u(6, 10) < head_u(20, 10));
}

// ---------------------------------------------------------------------------------------------
// Row 12 (the part that lives in `px-ui`): an empty cell
// ---------------------------------------------------------------------------------------------

#[test]
fn row_12_an_empty_cell_is_filled_by_a_shape_without_materializing_it_before() {
    let mut doc = Document::new(Image::new(SIZE, SIZE, Mode::Rgba).expect("valid image"));
    doc.apply(Command::AddLayer {
        name: "B".to_string(),
        at: 1,
    })
    .expect("a new layer");
    let id = doc
        .layers()
        .iter()
        .map(|layer| layer.cel())
        .find(|&cel| doc.image(cel).is_none())
        .expect("the new layer has an empty cell");
    for fill in ALL_FILLS {
        let mut tool = ToolState::new();
        tool.apply_action(ToolAction::SelectShape(ShapeKind::Rectangle));
        tool.set_shape_fill(fill);
        tool.set_other_pixel(Some(blue()));
        for event in [
            CanvasEvent::StrokeBegin((5, 5)),
            CanvasEvent::StrokeMove((12, 9)),
            CanvasEvent::StrokeEnd,
        ] {
            tool.canvas_event_with(&mut doc, id, Some(red()), event)
                .expect("shape");
        }
        let cell = doc.image(id).expect("the cell has pixels now");
        assert_eq!(cell.get_pixel(5, 5), Ok(red()), "{fill:?}");
        let inside = if fill == ShapeFill::Outline {
            Pixel::Rgba(Rgba::TRANSPARENT)
        } else if fill == ShapeFill::Both {
            blue()
        } else {
            red()
        };
        assert_eq!(cell.get_pixel(8, 7), Ok(inside), "{fill:?}");
        doc.undo().expect("one step");
        assert!(doc.image(id).is_none(), "undo leaves the cell empty ({fill:?})");
    }
}
