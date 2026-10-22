//! Tests del cuentagotas y del gesto con las dos herramientas nuevas, sin ventana
//! (specs/hito-2-cuentagotas-borrador.md, «Casos límite» filas 5, 6, 7 y 8 en lo que toca a
//! `px-ui`, y propiedades 2 (parte de `px-ui`) y 4).
//!
//! Los eventos se dan a mano a `ToolState::canvas_event_with`; el cuentagotas no necesita color
//! activo, así que se le pasa `None`. Lo que elige sale por `ToolState::take_events`.
//!
//! La propiedad 4 amplía la propiedad 6 de la unidad 7 (`tests/tool.rs`) a las cinco herramientas
//! sin tocarla. El oráculo de la propiedad 2 mira `Image::get_pixel` y no usa `picked_at`, que
//! se compara aparte.
//!
//! El «banco de egui» de la fila 8 es un `Context` con `RawInput` sintético y un campo de texto
//! enfocado, para ver `gate_keys` con el `text_edit_focused()` real.

use egui::{Context, Event, Key, Modifiers, Pos2, RawInput, Rect, TextEdit, vec2};
use proptest::prelude::*;
use proptest::test_runner::TestCaseError;
use px_core::{Command, Document, Image, ImageId, Mode, Palette, Pixel, PixelEdit, Rgba};
use px_ui::canvas::{CanvasEvent, CanvasInput};
use px_ui::color::pixel_for_slot;
use px_ui::pick::{Dropper, EraseError, Picked, eraser_pixel, picked_at};
use px_ui::shortcuts::gate_keys;
use px_ui::tool::{Tool, ToolAction, ToolEvent, ToolState, tool_action_for_key};

const MIN: i32 = i32::MIN;
const MAX: i32 = i32::MAX;
const TEST_MODES: [Mode; 4] = [Mode::Rgba, Mode::Indexed, Mode::Gray, Mode::GrayAlpha];
const TEST_ALL: [Tool; 5] = [
    Tool::Pencil,
    Tool::Line,
    Tool::Fill,
    Tool::Dropper,
    Tool::Eraser,
];

// Apoyo común.

fn rgba(r: u8, g: u8, b: u8, a: u8) -> Rgba {
    Rgba::new(r, g, b, a)
}

fn begin(x: i32, y: i32) -> CanvasEvent {
    CanvasEvent::StrokeBegin((x, y))
}

fn mov(x: i32, y: i32) -> CanvasEvent {
    CanvasEvent::StrokeMove((x, y))
}

fn fin() -> CanvasEvent {
    CanvasEvent::StrokeEnd
}

fn picked(picked: Picked) -> ToolEvent {
    ToolEvent::Picked(picked)
}

fn color(r: u8, g: u8, b: u8, a: u8) -> ToolEvent {
    ToolEvent::Picked(Picked::Color(rgba(r, g, b, a)))
}

/// Imagen RGBA de 8×8 en la que cada píxel tiene un color propio: `(10x, 10y, 5, 255)`.
fn rgba_single() -> Image {
    let mut image = Image::new(8, 8, Mode::Rgba).expect("imagen válida");
    for y in 0..8u8 {
        for x in 0..8u8 {
            image
                .set_pixel(
                    u32::from(x),
                    u32::from(y),
                    Pixel::Rgba(Rgba::opaque(10 * x, 10 * y, 5)),
                )
                .expect("dentro");
        }
    }
    image
}

/// El color de `(x, y)` en `rgba_single`, como lo elige el cuentagotas.
fn of_rgba_single(x: u8, y: u8) -> ToolEvent {
    color(10 * x, 10 * y, 5, 255)
}

/// Imagen RGBA de 8×8 azul opaca.
fn rgba_blue() -> Image {
    let mut image = Image::new(8, 8, Mode::Rgba).expect("imagen válida");
    for y in 0..8 {
        for x in 0..8 {
            image
                .set_pixel(x, y, Pixel::Rgba(Rgba::opaque(0, 0, 255)))
                .expect("dentro");
        }
    }
    image
}

/// Indexada de 8×8 con paleta de 6; la fila 0 lleva los índices `0…5` en `x = 0…5`.
fn indexed_with_row_0() -> Image {
    let palette =
        Palette::from_colors((0..6).map(|i| Rgba::opaque(40 * i, 10, 10))).expect("paleta válida");
    let mut image = Image::new_indexed(8, 8, palette).expect("imagen válida");
    for x in 0..6u8 {
        image
            .set_pixel(u32::from(x), 0, Pixel::Indexed(x))
            .expect("dentro");
    }
    image
}

/// Documento y herramienta juntos.
struct Scene {
    doc: Document,
    id: ImageId,
    tool: ToolState,
}

impl Scene {
    fn new(image: Image, tool_2: Tool) -> Scene {
        let doc = Document::new(image);
        let id = ImageId::from_raw(0);
        let mut tool = ToolState::new();
        tool.apply_action(ToolAction::Select(tool_2));
        assert_eq!(tool.tool(), tool_2);
        Scene { doc, id, tool }
    }

    fn dropper(image: Image) -> Scene {
        Scene::new(image, Tool::Dropper)
    }

    fn image_2(&self) -> &Image {
        self.doc.image(self.id).expect("hay imagen")
    }

    /// Da el evento a la herramienta sin color activo; un `Err` aquí es un fallo del test.
    fn ev(&mut self, event: CanvasEvent) -> bool {
        self.tool
            .canvas_event_with(&mut self.doc, self.id, None, event)
            .unwrap_or_else(|e| panic!("{event:?} dio el error {e:?}"))
    }

    fn escape(&mut self) -> bool {
        self.tool
            .canvas_input_with(&mut self.doc, self.id, None, CanvasInput::Escape)
            .unwrap_or_else(|e| panic!("Escape dio el error {e:?}"))
    }

    fn events_2(&mut self) -> Vec<ToolEvent> {
        self.tool.take_events()
    }

    fn preview(&self) -> Vec<PixelEdit> {
        self.tool.preview().collect()
    }

    /// Imagen, `state_id`, `can_undo`, `can_redo` e `history_bytes`.
    fn state_2(&self) -> (Image, u64, bool, bool, usize) {
        state_2(&self.doc, self.id)
    }
}

fn state_2(doc: &Document, id: ImageId) -> (Image, u64, bool, bool, usize) {
    (
        doc.image(id).expect("hay imagen").clone(),
        doc.state_id(),
        doc.can_undo(),
        doc.can_redo(),
        doc.history_bytes(),
    )
}

/// Un paso previo en la historia: `0` nada, `1` un paso, `2` un paso deshecho (hay rehacer).
fn step_previous(doc: &mut Document, id: ImageId, variant: u8) {
    if variant == 0 {
        return;
    }
    let image = doc.image(id).expect("hay imagen");
    let actual = image.get_pixel(0, 0).unwrap();
    let other_2 = pool(image.mode())
        .into_iter()
        .find(|&p| p != actual)
        .expect("el fondo tiene al menos dos valores");
    let command = Command::SetPixels {
        image: id,
        pixels: vec![PixelEdit {
            x: 0,
            y: 0,
            pixel: other_2,
        }],
    };
    assert_eq!(doc.apply(command), Ok(true));
    if variant == 2 {
        assert_eq!(doc.undo(), Ok(()));
    }
}

// Fila 5.

/// La secuencia de la fila 5 y lo que debe dar cada paso: `(event, events_2 de tool_2,
/// busy después)`.
fn sequence_of_row_5() -> Vec<(CanvasEvent, Vec<ToolEvent>, bool)> {
    vec![
        (begin(3, 0), vec![picked(Picked::Entry(3))], true),
        (mov(3, 0), vec![], true),
        (mov(4, 0), vec![picked(Picked::Entry(4))], true),
        (mov(4, 0), vec![], true),
        (mov(5, 0), vec![picked(Picked::Entry(5))], true),
        (fin(), vec![], false),
        (mov(1, 0), vec![], false),
    ]
}

#[test]
fn row_5_indexed_dropper_one_event_per_new_pixel() {
    for previous in 0u8..=2 {
        let mut es = Scene::dropper(indexed_with_row_0());
        step_previous(&mut es.doc, es.id, previous);
        let antes = es.state_2();
        assert!(!es.tool.busy());

        for (event, expected_2_2, busy) in sequence_of_row_5() {
            assert!(!es.ev(event), "{event:?}: Ok(false) siempre");
            assert_eq!(es.events_2(), expected_2_2, "{event:?} (previo {previous})");
            assert_eq!(es.tool.busy(), busy, "{event:?}");
            assert_eq!(es.preview(), vec![], "{event:?}: sin vista previa");
            assert_eq!(es.state_2(), antes, "{event:?}: el documento no cambia");
        }
    }
}

#[test]
fn row_5_all_the_events_together_in_order() {
    let mut es = Scene::dropper(indexed_with_row_0());
    let mut all_2 = Vec::new();
    for (event, _, _) in sequence_of_row_5() {
        es.ev(event);
        all_2.extend(es.events_2());
    }
    assert_eq!(
        all_2,
        vec![
            picked(Picked::Entry(3)),
            picked(Picked::Entry(4)),
            picked(Picked::Entry(5)),
        ]
    );
}

#[test]
fn take_events_empties_the_queue() {
    let mut es = Scene::dropper(indexed_with_row_0());
    es.ev(begin(3, 0));
    es.ev(mov(4, 0));
    es.ev(mov(5, 0));
    assert_eq!(es.events_2().len(), 3, "los tres a la vez, en orden");
    assert_eq!(es.events_2(), vec![], "la segunda vez no queda nada");
}

#[test]
fn the_uncollected_events_accumulate_in_order() {
    let mut es = Scene::dropper(indexed_with_row_0());
    es.ev(begin(1, 0));
    es.ev(mov(2, 0));
    es.ev(fin());
    es.ev(begin(4, 0));
    es.ev(fin());
    assert_eq!(
        es.events_2(),
        vec![
            picked(Picked::Entry(1)),
            picked(Picked::Entry(2)),
            picked(Picked::Entry(4)),
        ]
    );
}

#[test]
fn the_dropper_does_not_draw_lines_between_positions() {
    // Cada `Move(q)` elige `q` y nada más: el hueco entre dos posiciones no se recorre.
    let mut es = Scene::dropper(rgba_single());
    es.ev(begin(0, 0));
    es.ev(mov(5, 0));
    es.ev(mov(5, 4));
    es.ev(fin());
    assert_eq!(
        es.events_2(),
        vec![
            of_rgba_single(0, 0),
            of_rgba_single(5, 0),
            of_rgba_single(5, 4)
        ]
    );
}

#[test]
fn two_consecutive_clicks_on_the_same_pixel_choose_twice() {
    // `Begin(p)` elige `p` siempre; el «último visitado» es de cada gesto.
    let mut es = Scene::dropper(rgba_single());
    for _ in 0..2 {
        assert!(!es.ev(begin(2, 3)));
        assert!(!es.ev(fin()));
        assert_eq!(es.events_2(), vec![of_rgba_single(2, 3)]);
    }
}

#[test]
fn the_dropper_does_not_change_the_document_in_any_mode() {
    let mut gray_2 = Image::new(8, 8, Mode::Gray).expect("imagen válida");
    gray_2.set_pixel(1, 1, Pixel::Gray(50)).expect("dentro");
    let mut gray_alpha = Image::new(8, 8, Mode::GrayAlpha).expect("imagen válida");
    gray_alpha
        .set_pixel(1, 1, Pixel::GrayAlpha { gray: 5, alpha: 0 })
        .expect("dentro");
    for image in [rgba_single(), indexed_with_row_0(), gray_2, gray_alpha] {
        let mut es = Scene::dropper(image);
        step_previous(&mut es.doc, es.id, 2);
        let antes = es.state_2();
        for event in [begin(1, 1), mov(2, 2), mov(0, 0), fin()] {
            assert!(!es.ev(event));
            assert_eq!(es.state_2(), antes, "{event:?}");
        }
        assert_eq!(es.events_2().len(), 3);
        assert!(es.doc.can_redo(), "el rehacer sigue disponible");
    }
}

// `Dropper` suelto.

#[test]
fn dropper_visit_gives_some_only_inside_and_on_a_pixel_different_from_the_last() {
    let image = indexed_with_row_0();
    let mut dropper = Dropper::new();
    assert_eq!(dropper.visit(&image, (3, 0)), Some(Picked::Entry(3)));
    assert_eq!(dropper.visit(&image, (3, 0)), None, "el mismo píxel");
    assert_eq!(dropper.visit(&image, (4, 0)), Some(Picked::Entry(4)));
    assert_eq!(dropper.visit(&image, (4, 0)), None);
    assert_eq!(dropper.visit(&image, (5, 0)), Some(Picked::Entry(5)));
}

#[test]
fn dropper_visit_outside_the_image_chooses_nothing() {
    let image = indexed_with_row_0();
    let mut dropper = Dropper::new();
    for pos in [(-1, 0), (8, 0), (0, 8), (0, -1), (MIN, MAX), (MAX, MIN)] {
        assert_eq!(dropper.visit(&image, pos), None, "{pos:?}");
    }
    // Y no estropea lo siguiente: un píxel de dentro sigue eligiéndose.
    assert_eq!(dropper.visit(&image, (2, 0)), Some(Picked::Entry(2)));
}

#[test]
fn dropper_visit_matches_picked_at() {
    let image = rgba_single();
    for y in 0..8u32 {
        for x in 0..8u32 {
            let mut dropper = Dropper::new();
            assert_eq!(
                dropper.visit(&image, (x as i32, y as i32)),
                picked_at(&image, x, y),
                "({x}, {y})"
            );
        }
    }
}

#[test]
fn dropper_visit_a_new_dropper_remembers_nothing() {
    let image = indexed_with_row_0();
    let mut a = Dropper::new();
    assert!(a.visit(&image, (3, 0)).is_some());
    let mut b = Dropper::new();
    assert_eq!(b.visit(&image, (3, 0)), Some(Picked::Entry(3)));
}

// Fila 6.

/// Un clic en `(2, 3)`: `Begin` y `End`. Devuelve lo que eligió.
fn click_at_2_3(image: Image) -> Vec<ToolEvent> {
    let mut es = Scene::dropper(image);
    assert!(!es.ev(begin(2, 3)));
    assert!(!es.ev(fin()));
    es.events_2()
}

fn with_pixel(mode: Mode, pixel: Pixel) -> Image {
    let mut image = Image::new(8, 8, mode).expect("imagen válida");
    image.set_pixel(2, 3, pixel).expect("dentro");
    image
}

#[test]
fn row_6_rgba_with_alpha_128() {
    let image = with_pixel(Mode::Rgba, Pixel::Rgba(rgba(10, 20, 30, 128)));
    assert_eq!(click_at_2_3(image), vec![color(10, 20, 30, 128)]);
}

#[test]
fn row_6_rgba_with_alpha_0_is_copied_as_is() {
    let image = with_pixel(Mode::Rgba, Pixel::Rgba(rgba(10, 20, 30, 0)));
    assert_eq!(click_at_2_3(image), vec![color(10, 20, 30, 0)]);
}

#[test]
fn row_6_gray() {
    let image = with_pixel(Mode::Gray, Pixel::Gray(96));
    assert_eq!(click_at_2_3(image), vec![color(96, 96, 96, 255)]);
}

#[test]
fn row_6_gray_with_alpha_200() {
    let image = with_pixel(
        Mode::GrayAlpha,
        Pixel::GrayAlpha {
            gray: 7,
            alpha: 200,
        },
    );
    assert_eq!(click_at_2_3(image), vec![color(7, 7, 7, 200)]);
}

#[test]
fn row_6_gray_with_alpha_0() {
    let image = with_pixel(Mode::GrayAlpha, Pixel::GrayAlpha { gray: 7, alpha: 0 });
    assert_eq!(click_at_2_3(image), vec![color(7, 7, 7, 0)]);
}

#[test]
fn row_6_indexed_chooses_the_entry_also_the_transparent_one() {
    let mut palette =
        Palette::from_colors((0..4).map(|i| Rgba::opaque(60 * i, 0, 0))).expect("paleta válida");
    palette.set_transparent_index(Some(2)).expect("existe");
    let mut image = Image::new_indexed(8, 8, palette).expect("imagen válida");
    image.set_pixel(2, 3, Pixel::Indexed(2)).expect("dentro");
    assert_eq!(click_at_2_3(image), vec![picked(Picked::Entry(2))]);
}

// Fila 7.

#[test]
fn row_7_starting_outside_chooses_nothing_but_leaves_the_gesture_busy() {
    for (x, y) in [(-1, 0), (8, 0), (0, 8), (0, -1), (MIN, MAX), (MAX, MAX)] {
        let mut es = Scene::dropper(rgba_single());
        let antes = es.state_2();
        assert!(!es.ev(begin(x, y)));
        assert!(es.tool.busy(), "({x}, {y}): busy desde el Begin");
        assert_eq!(es.events_2(), vec![], "({x}, {y})");
        assert_eq!(es.preview(), vec![]);
        assert!(!es.ev(fin()));
        assert!(!es.tool.busy(), "({x}, {y}): libre tras el End");
        assert_eq!(es.events_2(), vec![]);
        assert_eq!(es.state_2(), antes);
    }
}

#[test]
fn row_7_a_drag_that_starts_outside_and_enters_chooses_on_entering() {
    let mut es = Scene::dropper(rgba_single());
    assert!(!es.ev(begin(-3, 2)));
    assert_eq!(es.events_2(), vec![]);
    assert!(!es.ev(mov(2, 2)));
    assert_eq!(es.events_2(), vec![of_rgba_single(2, 2)]);
    assert!(es.tool.busy());
    assert!(!es.ev(fin()));
    assert!(!es.tool.busy());
    assert_eq!(es.events_2(), vec![]);
}

#[test]
fn row_7_a_drag_that_leaves_neither_chooses_nor_cancels_and_on_returning_chooses_another() {
    let mut es = Scene::dropper(rgba_single());
    let antes = es.state_2();
    assert!(!es.ev(begin(2, 2)));
    assert_eq!(es.events_2(), vec![of_rgba_single(2, 2)]);
    assert!(!es.ev(mov(20, 2)));
    assert_eq!(es.events_2(), vec![], "fuera: ni elige ni anula");
    assert!(!es.ev(mov(3, 2)));
    assert_eq!(es.events_2(), vec![of_rgba_single(3, 2)]);
    assert!(es.tool.busy());
    assert!(!es.ev(fin()));
    assert!(!es.tool.busy());
    assert_eq!(
        es.events_2(),
        vec![],
        "lo elegido se queda: nada más que decir"
    );
    assert_eq!(es.state_2(), antes);
}

// Fila 8: gesto y teclado.

#[test]
fn row_8_during_an_eraser_stroke_select_and_escape_do_nothing() {
    let copy = rgba_blue();
    let mut es = Scene::new(copy.clone(), Tool::Eraser);
    assert!(!es.ev(begin(1, 1)));
    assert!(!es.ev(mov(5, 1)));
    let pending = es.preview();
    assert_eq!(pending.len(), 5);

    es.tool.apply_action(ToolAction::Select(Tool::Pencil));
    assert_eq!(
        es.tool.tool(),
        Tool::Eraser,
        "Select se ignora con el gesto"
    );
    es.tool.apply_action(ToolAction::Select(Tool::Dropper));
    assert_eq!(es.tool.tool(), Tool::Eraser);
    assert!(!es.escape(), "Escape: Ok(false)");
    es.tool.apply_action(ToolAction::Cancel);
    assert!(es.tool.busy());
    assert_eq!(es.preview(), pending, "el trazo sigue ahí");
    assert_eq!(es.image_2(), &copy, "y el documento no cambió");

    assert!(es.ev(fin()), "el borrador se aplica al soltar");
    for x in 1..=5 {
        assert_eq!(
            es.image_2().get_pixel(x, 1),
            Ok(Pixel::Rgba(Rgba::TRANSPARENT))
        );
    }
    assert!(es.doc.can_undo());
    assert!(!es.tool.busy());
}

#[test]
fn row_8_during_a_dropper_drag_select_and_escape_do_nothing() {
    let mut es = Scene::dropper(rgba_single());
    let antes = es.state_2();
    assert!(!es.ev(begin(2, 2)));
    assert!(!es.ev(mov(3, 2)));
    assert_eq!(
        es.events_2(),
        vec![of_rgba_single(2, 2), of_rgba_single(3, 2)]
    );

    es.tool.apply_action(ToolAction::Select(Tool::Pencil));
    assert_eq!(
        es.tool.tool(),
        Tool::Dropper,
        "Select se ignora con el gesto"
    );
    es.tool.apply_action(ToolAction::Select(Tool::Eraser));
    assert_eq!(es.tool.tool(), Tool::Dropper);
    assert!(!es.escape(), "Escape: Ok(false)");
    es.tool.apply_action(ToolAction::Cancel);
    assert!(es.tool.busy());
    assert_eq!(es.events_2(), vec![], "Escape no elige ni anula nada");

    // El arrastre sigue después de `Escape`, y lo elegido se queda.
    assert!(!es.ev(mov(4, 2)));
    assert_eq!(es.events_2(), vec![of_rgba_single(4, 2)]);
    assert!(!es.ev(fin()));
    assert!(!es.tool.busy());
    assert_eq!(es.state_2(), antes);
}

#[test]
fn row_8_after_the_end_select_does_change_tool() {
    for tool_2 in [Tool::Eraser, Tool::Dropper] {
        let mut es = Scene::new(rgba_blue(), tool_2);
        es.ev(begin(1, 1));
        es.ev(mov(2, 1));
        es.ev(fin());
        assert!(!es.tool.busy());
        es.tool.apply_action(ToolAction::Select(Tool::Pencil));
        assert_eq!(es.tool.tool(), Tool::Pencil, "desde {tool_2:?}");
        es.tool.apply_action(ToolAction::Select(Tool::Eraser));
        assert_eq!(es.tool.tool(), Tool::Eraser);
        es.tool.apply_action(ToolAction::Select(Tool::Dropper));
        assert_eq!(es.tool.tool(), Tool::Dropper);
    }
}

#[test]
fn escape_at_rest_with_the_new_tools_does_nothing() {
    for tool_2 in [Tool::Dropper, Tool::Eraser] {
        let mut es = Scene::new(rgba_blue(), tool_2);
        let antes = es.state_2();
        assert!(!es.escape());
        assert_eq!(es.tool.tool(), tool_2);
        assert!(!es.tool.busy());
        assert_eq!(es.events_2(), vec![]);
        assert_eq!(es.state_2(), antes);
    }
}

#[test]
fn busy_goes_from_begin_to_end_with_and_without_a_real_gesture_in_the_two_tools() {
    // Con gesto real (RGBA), sin él (el borrador en gris, el cuentagotas fuera de la imagen).
    let cases = [
        (rgba_blue(), Tool::Eraser, (1, 1)),
        (rgba_blue(), Tool::Dropper, (1, 1)),
        (rgba_blue(), Tool::Dropper, (-4, -4)),
        (
            Image::new(8, 8, Mode::Gray).expect("imagen válida"),
            Tool::Eraser,
            (1, 1),
        ),
    ];
    for (image, tool_2, (x, y)) in cases {
        let mut es = Scene::new(image, tool_2);
        assert!(!es.tool.busy());
        es.ev(begin(x, y));
        assert!(es.tool.busy(), "{tool_2:?}: tras Begin");
        es.ev(mov(x + 1, y));
        assert!(es.tool.busy(), "{tool_2:?}: tras Move");
        es.ev(fin());
        assert!(!es.tool.busy(), "{tool_2:?}: tras End");
    }
}

fn key_2(key: Key, modifiers: Modifiers) -> Event {
    Event::Key {
        key,
        physical_key: Some(key),
        pressed: true,
        repeat: false,
        modifiers,
    }
}

/// Lo que haría la app con los eventos que le dejó pasar `gate_keys`.
fn applies_tool_keys(tool: &mut ToolState, events_2: &[Event]) {
    for event in events_2 {
        if let Event::Key {
            key,
            pressed: true,
            modifiers,
            ..
        } = event
            && let Some(action_2) = tool_action_for_key(*key, *modifiers)
        {
            tool.apply_action(action_2);
        }
    }
}

fn keys_comma_and_e() -> Vec<Event> {
    vec![
        key_2(Key::Comma, Modifiers::NONE),
        Event::Text(",".to_string()),
        key_2(Key::E, Modifiers::NONE),
        Event::Text("e".to_string()),
    ]
}

#[test]
fn row_8_gate_keys_with_text_focus_lets_neither_comma_nor_e_through() {
    let events_2 = keys_comma_and_e();
    assert!(gate_keys(&events_2, true).is_empty());
    for event in &events_2 {
        assert!(gate_keys(std::slice::from_ref(event), true).is_empty());
    }
    let mut tool = ToolState::new();
    applies_tool_keys(&mut tool, &gate_keys(&events_2, true));
    assert_eq!(tool.tool(), Tool::Pencil, "la herramienta no cambia");
}

#[test]
fn row_8_gate_keys_without_focus_lets_comma_and_e_through() {
    let events_2 = keys_comma_and_e();
    assert_eq!(gate_keys(&events_2, false), events_2);
    let mut tool = ToolState::new();
    applies_tool_keys(&mut tool, &gate_keys(&key_single(Key::Comma), false));
    assert_eq!(tool.tool(), Tool::Dropper);
    // La `E` ya no es atajo: el filtro la deja pasar y no cambia nada. El borrador se elige por
    // su acción (la del botón).
    applies_tool_keys(&mut tool, &gate_keys(&key_single(Key::E), false));
    assert_eq!(tool.tool(), Tool::Dropper);
    tool.apply_action(ToolAction::Select(Tool::Eraser));
    assert_eq!(tool.tool(), Tool::Eraser);
}

fn key_single(key: Key) -> Vec<Event> {
    vec![key_2(key, Modifiers::NONE)]
}

/// Un `Context` con un campo de texto que se puede enfocar o no.
struct BenchText {
    ctx: Context,
    text_2: String,
}

impl BenchText {
    fn new_2() -> BenchText {
        BenchText {
            ctx: Context::default(),
            text_2: String::new(),
        }
    }

    fn frame(&mut self, events: Vec<Event>, focus: bool) {
        let input = RawInput {
            screen_rect: Some(Rect::from_min_size(Pos2::ZERO, vec2(400.0, 300.0))),
            events,
            ..RawInput::default()
        };
        let text_2 = &mut self.text_2;
        let mut output_2 = self.ctx.run_ui(input, |ui| {
            let response = ui.add(TextEdit::singleline(text_2));
            if focus {
                response.request_focus();
            }
        });
        output_2.textures_delta.clear();
    }
}

/// **[DEPENDE DE EGUI]** Con un campo de texto enfocado de verdad, `text_edit_focused()` es cierto
/// y `gate_keys` no deja pasar ni `,` ni `E`; sin él, los deja pasar.
#[test]
fn row_8_egui_bench_with_a_focused_field() {
    let events_2 = keys_comma_and_e();

    let mut with_focus = BenchText::new_2();
    with_focus.frame(vec![], true);
    with_focus.frame(vec![], true);
    assert!(with_focus.ctx.text_edit_focused(), "el campo tiene el foco");
    with_focus.frame(events_2.clone(), false);
    assert!(with_focus.ctx.text_edit_focused());
    let seen_2 = gate_keys(&events_2, with_focus.ctx.text_edit_focused());
    assert!(seen_2.is_empty());
    let mut tool = ToolState::new();
    applies_tool_keys(&mut tool, &seen_2);
    assert_eq!(tool.tool(), Tool::Pencil);

    let mut without_focus = BenchText::new_2();
    without_focus.frame(vec![], false);
    without_focus.frame(vec![], false);
    assert!(!without_focus.ctx.text_edit_focused());
    let seen_2 = gate_keys(&events_2, without_focus.ctx.text_edit_focused());
    assert_eq!(seen_2, events_2);
    let mut tool = ToolState::new();
    applies_tool_keys(&mut tool, &seen_2);
    assert_eq!(tool.tool(), Tool::Dropper, "`,` y luego E (sin atajo)");
    tool.apply_action(ToolAction::Select(Tool::Eraser));
    assert_eq!(tool.tool(), Tool::Eraser, "el borrador, por su acción");
}

// Estrategias de las propiedades 2 y 4.

fn mode_2() -> impl Strategy<Value = Mode> {
    prop::sample::select(TEST_MODES.to_vec())
}

/// Paleta de seis entradas; con `transparent`, la 5 es el índice transparente.
fn palette_six(transparent: bool) -> Palette {
    let mut palette = Palette::from_colors([
        rgba(200, 0, 0, 255),
        rgba(200, 0, 0, 255),
        rgba(200, 0, 0, 128),
        rgba(0, 0, 200, 255),
        rgba(0, 200, 0, 255),
        rgba(5, 5, 5, 0),
    ])
    .unwrap();
    if transparent {
        palette.set_transparent_index(Some(5)).unwrap();
    }
    palette
}

/// Valores posibles del fondo, con transparentes y casi duplicados.
fn pool(mode: Mode) -> Vec<Pixel> {
    match mode {
        Mode::Rgba => vec![
            Pixel::Rgba(rgba(10, 20, 30, 255)),
            Pixel::Rgba(rgba(10, 20, 30, 128)),
            Pixel::Rgba(rgba(10, 20, 30, 0)),
            Pixel::Rgba(rgba(0, 0, 0, 0)),
            Pixel::Rgba(rgba(40, 50, 60, 255)),
        ],
        Mode::Indexed => (0..6).map(Pixel::Indexed).collect(),
        Mode::Gray => vec![
            Pixel::Gray(5),
            Pixel::Gray(6),
            Pixel::Gray(7),
            Pixel::Gray(8),
        ],
        Mode::GrayAlpha => vec![
            Pixel::GrayAlpha {
                gray: 5,
                alpha: 255,
            },
            Pixel::GrayAlpha { gray: 5, alpha: 0 },
            Pixel::GrayAlpha { gray: 6, alpha: 0 },
            Pixel::GrayAlpha {
                gray: 7,
                alpha: 128,
            },
        ],
    }
}

/// Cualquier `i32`, con el interior de un lienzo de hasta 16 de lado y los extremos siempre
/// presentes.
fn coordinate() -> impl Strategy<Value = i32> {
    prop_oneof![
        6 => -2i32..=17,
        2 => any::<i32>(),
        1 => Just(MIN),
        1 => Just(MAX),
        1 => Just(MIN + 1),
        1 => Just(MAX - 1),
    ]
}

fn position() -> impl Strategy<Value = (i32, i32)> {
    (coordinate(), coordinate())
}

fn tool_2() -> impl Strategy<Value = Tool> {
    prop::sample::select(TEST_ALL.to_vec())
}

/// Imagen de 1 a 16 de lado en cualquier modo, con fondo variado, y una ranura de 1 a 9. Las
/// indexadas llevan a veces índice transparente.
fn images() -> impl Strategy<Value = (Image, u8)> {
    (mode_2(), 1u32..=16, 1u32..=16, 1u8..=9, any::<bool>())
        .prop_flat_map(|(mode, w, h, slot, transparent)| {
            let cels = usize::try_from(w * h).unwrap();
            (
                Just((mode, w, h, slot, transparent)),
                prop::collection::vec(any::<u8>(), cels),
            )
        })
        .prop_map(|((mode, w, h, slot, transparent), cels)| {
            let pool = pool(mode);
            let mut image = match mode {
                Mode::Indexed => Image::new_indexed(w, h, palette_six(transparent)).unwrap(),
                _ => Image::new(w, h, mode).unwrap(),
            };
            for y in 0..h {
                for x in 0..w {
                    let byte = cels[usize::try_from(y * w + x).unwrap()];
                    let k = usize::from(byte) % pool.len();
                    image.set_pixel(x, y, pool[k]).unwrap();
                }
            }
            (image, slot)
        })
}

/// Lo que debe elegir el cuentagotas con un píxel, escrito aparte de `picked_at`.
fn chosen_expected(pixel: Pixel) -> Picked {
    match pixel {
        Pixel::Indexed(i) => Picked::Entry(i),
        Pixel::Rgba(c) => Picked::Color(c),
        Pixel::Gray(g) => Picked::Color(Rgba::opaque(g, g, g)),
        Pixel::GrayAlpha { gray, alpha } => Picked::Color(rgba(gray, gray, gray, alpha)),
    }
}

/// El píxel de la posición, si cae dentro de la imagen.
fn inside(image: &Image, pos: (i32, i32)) -> Option<(u32, u32)> {
    let (x, y) = (u32::try_from(pos.0).ok()?, u32::try_from(pos.1).ok()?);
    (x < image.width() && y < image.height()).then_some((x, y))
}

/// El oráculo del cuentagotas: elige un píxel de dentro distinto del último de dentro que eligió
/// en este gesto.
#[derive(Default)]
struct Oracle {
    ultimo: Option<(u32, u32)>,
}

impl Oracle {
    fn visits(&mut self, image: &Image, pos: (i32, i32)) -> Option<Picked> {
        let (x, y) = inside(image, pos)?;
        if self.ultimo == Some((x, y)) {
            return None;
        }
        self.ultimo = Some((x, y));
        Some(chosen_expected(image.get_pixel(x, y).unwrap()))
    }
}

fn matches_picked_at(
    image: &Image,
    pos: (i32, i32),
    expected_2: Picked,
) -> Result<(), TestCaseError> {
    let (x, y) = inside(image, pos).expect("el píxel elegido está dentro");
    prop_assert_eq!(
        picked_at(image, x, y),
        Some(expected_2),
        "picked_at ({}, {})",
        x,
        y
    );
    Ok(())
}

// Propiedad 2: el cuentagotas no escribe y es exacto (parte de `px-ui`).

proptest! {
    #[test]
    fn property_2_the_dropper_does_not_write_and_chooses_each_new_pixel_and_inside(
        (image, _slot) in images(),
        previous in 0u8..=2,
        gestures in prop::collection::vec(
            (position(), prop::collection::vec(position(), 0..=6)),
            1..=4,
        ),
    ) {
        let mut es = Scene::dropper(image);
        step_previous(&mut es.doc, es.id, previous);
        let antes = es.state_2();

        for (source_2, movements) in gestures {
            let mut oracle = Oracle::default();
            let mut events_2 = vec![(begin(source_2.0, source_2.1), source_2)];
            events_2.extend(movements.iter().map(|&p| (mov(p.0, p.1), p)));
            for (event, pos) in events_2 {
                let expected_2 = oracle.visits(es.image_2(), pos);
                prop_assert!(!es.ev(event), "{:?}: Ok(false)", event);
                let received = es.events_2();
                let expected_2_2: Vec<ToolEvent> = expected_2.map(ToolEvent::Picked).into_iter().collect();
                prop_assert_eq!(&received, &expected_2_2, "{:?}", event);
                if let Some(picked) = expected_2 {
                    matches_picked_at(es.image_2(), pos, picked)?;
                }
                prop_assert_eq!(es.state_2(), antes.clone(), "{:?}: el documento no cambia", event);
                prop_assert!(es.tool.busy());
                prop_assert!(es.preview().is_empty());
            }
            prop_assert!(!es.ev(fin()));
            prop_assert!(!es.tool.busy());
            prop_assert!(es.events_2().is_empty());
            prop_assert_eq!(es.state_2(), antes.clone());
        }
    }
}

// Propiedad 4: herramienta estable con las cinco.

/// Un paso de la secuencia. Solo se entregan los que `Canvas` puede emitir: `Begin` en reposo y
/// `Move` y `End` con un gesto abierto; el resto se descarta.
#[derive(Clone, Debug)]
enum Operation {
    Begin((i32, i32)),
    Move((i32, i32)),
    End,
    Escape,
    Select(Tool),
    Cancel,
}

fn operation() -> impl Strategy<Value = Operation> {
    prop_oneof![
        3 => position().prop_map(Operation::Begin),
        3 => position().prop_map(Operation::Move),
        3 => Just(Operation::End),
        1 => Just(Operation::Escape),
        4 => tool_2().prop_map(Operation::Select),
        1 => Just(Operation::Cancel),
    ]
}

proptest! {
    #[test]
    fn property_4_stable_tool_with_the_five(
        (image, slot) in images(),
        operations in prop::collection::vec(operation(), 0..=50),
    ) {
        let mut es = Scene::new(image, Tool::Pencil);
        let active_2_2 = pixel_for_slot(es.image_2(), slot);
        let error_of_deleted = eraser_pixel(es.image_2()).err();
        prop_assert!(!es.tool.busy());
        prop_assert!(es.preview().is_empty(), "vacía en reposo");
        prop_assert!(es.tool.take_events().is_empty());

        let mut busy = false;
        for operation in operations {
            let tool_before = es.tool.tool();
            let mut expected_2_2: Vec<ToolEvent> = Vec::new();
            match operation {
                Operation::Begin(p) if !busy => {
                    let busy_with = es.tool.tool();
                    es.tool
                        .canvas_event_with(&mut es.doc, es.id, active_2_2, begin(p.0, p.1))
                        .unwrap();
                    busy = true;
                    if busy_with == Tool::Eraser
                        && let Some(error) = error_of_deleted
                    {
                        expected_2_2.push(ToolEvent::Refused(error));
                    }
                    if busy_with == Tool::Dropper && inside(es.image_2(), p).is_some() {
                        let received_2 = es.tool.take_events();
                        prop_assert_eq!(received_2.len(), 1, "el Begin dentro elige un píxel");
                        prop_assert!(matches!(received_2[0], ToolEvent::Picked(_)));
                    } else {
                        prop_assert_eq!(es.tool.take_events(), expected_2_2, "Begin con {:?}", busy_with);
                    }
                }
                Operation::Move(p) if busy => {
                    es.tool
                        .canvas_event_with(&mut es.doc, es.id, active_2_2, mov(p.0, p.1))
                        .unwrap();
                    let received_2 = es.tool.take_events();
                    if es.tool.tool() == Tool::Dropper {
                        prop_assert!(received_2.len() <= 1);
                        prop_assert!(received_2.iter().all(|e| matches!(e, ToolEvent::Picked(_))));
                    } else {
                        prop_assert!(received_2.is_empty(), "Move con {:?}: {:?}", es.tool.tool(), received_2);
                    }
                }
                Operation::End if busy => {
                    es.tool
                        .canvas_event_with(&mut es.doc, es.id, active_2_2, fin())
                        .unwrap();
                    busy = false;
                    let received_2 = es.tool.take_events();
                    prop_assert!(received_2.is_empty(), "End no emite eventos: {:?}", received_2);
                }
                Operation::Escape => {
                    es.tool
                        .canvas_input_with(&mut es.doc, es.id, active_2_2, CanvasInput::Escape)
                        .unwrap();
                    prop_assert!(es.tool.take_events().is_empty());
                }
                Operation::Cancel => {
                    es.tool.apply_action(ToolAction::Cancel);
                    prop_assert!(es.tool.take_events().is_empty());
                }
                Operation::Select(tool_2) => {
                    es.tool.apply_action(ToolAction::Select(tool_2));
                    let expected = if busy { tool_before } else { tool_2 };
                    prop_assert_eq!(es.tool.tool(), expected, "Select con busy = {}", busy);
                    prop_assert!(es.tool.take_events().is_empty());
                }
                _ => {}
            }
            if !matches!(operation, Operation::Select(_)) {
                prop_assert_eq!(es.tool.tool(), tool_before, "solo Select cambia la herramienta");
            }
            prop_assert_eq!(es.tool.busy(), busy, "busy es cierto entre Begin y End");
            if es.tool.tool() == Tool::Dropper {
                prop_assert!(es.preview().is_empty(), "el cuentagotas nunca tiene vista previa");
            }
            if !busy {
                prop_assert!(es.preview().is_empty(), "vacía en reposo");
                prop_assert!(es.tool.take_events().is_empty(), "sin eventos en reposo");
            }
        }
    }
}

/// `EraseError` solo se usa en la propiedad 4 a través de `ToolEvent::Refused`.
#[test]
fn the_erase_error_travels_in_the_event() {
    let event = ToolEvent::Refused(EraseError::NoTransparency);
    assert_eq!(event.clone(), event);
    assert_ne!(event, ToolEvent::Refused(EraseError::NoTransparentIndex));
    assert_ne!(event, ToolEvent::Picked(Picked::Entry(0)));
}
