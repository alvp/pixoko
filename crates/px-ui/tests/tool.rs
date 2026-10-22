//! Tests de la lógica de herramientas, sin ventana (specs/hito-2-linea-relleno-app.md, «Casos
//! límite» filas 1 a 12, 15 (en `tests/notice.rs`), 16, 17 (primera parte), 18 y 19, y
//! propiedades 1 a 6).
//!
//! Los eventos se dan a mano a `ToolState::canvas_event`. El oráculo de la línea es
//! `px_raster::line_command` (propiedad 1) y el del relleno es un recorrido ingenuo pixel a pixel
//! con `VecDeque`, 4 vecinos y `Image::get_pixel` (propiedad 1 de specs/hito-2-relleno.md), que no
//! usa nada de `px-raster`.
//!
//! La propiedad 5 recorre todo `Key::ALL` por las 32 combinaciones de modificadores: se escribe
//! como `proptest!` (muestreo) y, además, como recorrido exhaustivo, que es más fuerte.

use std::collections::{BTreeSet, VecDeque};

use egui::{Key, Modifiers};
use proptest::prelude::*;
use proptest::test_runner::TestCaseError;
use px_core::{
    Command, Document, DocumentError, Image, ImageId, Mode, Palette, Pixel, PixelEdit, Rgba,
};
use px_raster::{flood_fill, line_command};
use px_ui::canvas::{CanvasEvent, CanvasInput};
use px_ui::color::pixel_for_slot;
use px_ui::shortcuts::action_for_key;
use px_ui::tool::{Tool, ToolAction, ToolState, status_text, tool_action_for_key};

const MIN: i32 = i32::MIN;
const MAX: i32 = i32::MAX;
const TEST_MODES: [Mode; 4] = [Mode::Rgba, Mode::Indexed, Mode::Gray, Mode::GrayAlpha];
const TEST_TOOLS: [Tool; 3] = [Tool::Pencil, Tool::Line, Tool::Fill];

// Apoyo común.

fn rgba(r: u8, g: u8, b: u8, a: u8) -> Rgba {
    Rgba::new(r, g, b, a)
}

/// Ranura 3: rojo opaco.
fn red() -> Pixel {
    Pixel::Rgba(Rgba::opaque(255, 0, 0))
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

/// Imagen RGBA de 8×8 vacía (todo `A` a cero).
fn image_8x8() -> Image {
    Image::new(8, 8, Mode::Rgba).expect("imagen válida")
}

/// Imagen indexada de 8×8 con una paleta de 3 entradas.
fn indexed_image_8x8() -> Image {
    let palette = Palette::from_colors([
        Rgba::opaque(10, 10, 10),
        Rgba::opaque(20, 20, 20),
        Rgba::opaque(30, 30, 30),
    ])
    .expect("paleta válida");
    Image::new_indexed(8, 8, palette).expect("imagen válida")
}

/// Documento, herramienta y ranura juntos: lo que hace `px-app` con los eventos del lienzo.
struct Scene {
    doc: Document,
    id: ImageId,
    tool: ToolState,
    slot: u8,
}

impl Scene {
    fn new(image: Image, tool_2: Tool, slot: u8) -> Scene {
        let doc = Document::new(image);
        let id = ImageId::from_raw(0);
        let mut tool = ToolState::new();
        tool.apply_action(ToolAction::Select(tool_2));
        Scene {
            doc,
            id,
            tool,
            slot,
        }
    }

    fn rgba_8x8(tool_2: Tool) -> Scene {
        Scene::new(image_8x8(), tool_2, 3)
    }

    fn image_2(&self) -> &Image {
        self.doc.image(self.id).expect("hay imagen")
    }

    /// Da el evento a la herramienta; un `Err` aquí es un fallo del test.
    fn ev(&mut self, event: CanvasEvent) -> bool {
        self.tool
            .canvas_event(&mut self.doc, self.id, self.slot, event)
            .unwrap_or_else(|e| panic!("{event:?} dio el error {e:?}"))
    }

    /// `Escape` por la vía de la entrada ordenada (`canvas_input`).
    fn escape(&mut self) -> bool {
        self.tool
            .canvas_input(&mut self.doc, self.id, self.slot, CanvasInput::Escape)
            .unwrap_or_else(|e| panic!("Escape dio el error {e:?}"))
    }

    /// Un evento de trazo por la vía de la entrada ordenada (`canvas_input`).
    fn input_2(&mut self, event: CanvasEvent) -> bool {
        self.tool
            .canvas_input(
                &mut self.doc,
                self.id,
                self.slot,
                CanvasInput::Stroke(event),
            )
            .unwrap_or_else(|e| panic!("{event:?} dio el error {e:?}"))
    }

    /// Lo que hay pendiente en la superposición.
    fn preview(&self) -> Vec<PixelEdit> {
        self.tool.preview().collect()
    }

    fn history(&self) -> (bool, bool, usize) {
        history(&self.doc)
    }
}

/// `can_undo`, `can_redo` e `history_bytes`.
fn history(doc: &Document) -> (bool, bool, usize) {
    (doc.can_undo(), doc.can_redo(), doc.history_bytes())
}

fn set(points: impl IntoIterator<Item = (u32, u32)>) -> BTreeSet<(u32, u32)> {
    points.into_iter().collect()
}

/// Coordenadas de los píxeles que difieren entre dos imágenes del mismo tamaño.
fn changed(a: &Image, b: &Image) -> BTreeSet<(u32, u32)> {
    assert_eq!((a.width(), a.height()), (b.width(), b.height()));
    (0..a.height())
        .flat_map(|y| (0..a.width()).map(move |x| (x, y)))
        .filter(|&(x, y)| a.get_pixel(x, y).unwrap() != b.get_pixel(x, y).unwrap())
        .collect()
}

/// `n` píxeles de la fila `y`, de `x0` a `x1` incluidos, con `pixel`.
fn row_of(y: u32, x0: u32, x1: u32, pixel: Pixel) -> Vec<PixelEdit> {
    (x0..=x1).map(|x| PixelEdit { x, y, pixel }).collect()
}

/// La línea de la fila 5: `Begin(1,1)`, `Move(5,1)` y `End`, que da los píxeles `(1,1)…(5,1)`.
fn line_of_row_5(es: &mut Scene) -> bool {
    assert!(!es.ev(begin(1, 1)));
    assert!(!es.ev(mov(5, 1)));
    es.ev(fin())
}

// Fila 1.

#[test]
fn row_1_d_v_f_and_escape_without_modifiers() {
    let n = Modifiers::NONE;
    assert_eq!(
        tool_action_for_key(Key::D, n),
        Some(ToolAction::Select(Tool::Pencil))
    );
    assert_eq!(
        tool_action_for_key(Key::V, n),
        Some(ToolAction::Select(Tool::Line))
    );
    assert_eq!(
        tool_action_for_key(Key::F, n),
        Some(ToolAction::Select(Tool::Fill))
    );
    assert_eq!(
        tool_action_for_key(Key::Escape, n),
        Some(ToolAction::Cancel)
    );
}

// Fila 2.

#[test]
fn row_2_with_shift_command_or_alt_there_is_no_action() {
    for modifiers in [Modifiers::SHIFT, Modifiers::COMMAND, Modifiers::ALT] {
        for key in [Key::D, Key::V, Key::F, Key::Escape] {
            assert_eq!(
                tool_action_for_key(key, modifiers),
                None,
                "{key:?} con {modifiers:?}"
            );
        }
    }
}

#[test]
fn row_2_other_keys_without_modifiers_do_nothing() {
    for key in [Key::Z, Key::Num1, Key::Space, Key::Home] {
        assert_eq!(tool_action_for_key(key, Modifiers::NONE), None, "{key:?}");
    }
}

#[test]
fn row_2_action_for_key_still_gives_none_for_d_v_f_and_escape() {
    for key in [Key::D, Key::V, Key::F, Key::Escape] {
        assert_eq!(action_for_key(key, Modifiers::NONE), None, "{key:?}");
    }
}

// Fila 3.

#[test]
fn row_3_status_text() {
    assert_eq!(
        status_text(Tool::Line, Some("color 3 #ff0000")),
        "línea · color 3 #ff0000"
    );
    assert_eq!(status_text(Tool::Pencil, None), "lápiz");
    assert_eq!(
        status_text(Tool::Fill, Some("color 9 transparente")),
        "relleno · color 9 transparente"
    );
}

#[test]
fn the_names_of_the_tools() {
    assert_eq!(Tool::Pencil.label(), "lápiz");
    assert_eq!(Tool::Line.label(), "línea");
    assert_eq!(Tool::Fill.label(), "relleno");
}

#[test]
fn the_initial_tool_is_the_pencil_and_nothing_is_pending() {
    let tool = ToolState::new();
    assert_eq!(tool.tool(), Tool::Pencil);
    assert!(!tool.busy());
    assert_eq!(tool.preview().count(), 0);
}

// Fila 4.

#[test]
fn row_4_the_line_in_progress_is_only_seen_in_the_preview() {
    let mut es = Scene::rgba_8x8(Tool::Line);
    let copy = es.image_2().clone();
    assert!(!es.ev(begin(1, 1)), "Begin no cambia el documento");
    assert!(!es.ev(mov(5, 1)), "Move no cambia el documento");
    assert_eq!(es.image_2(), &copy);
    assert!(!es.doc.can_undo());
    assert_eq!(es.preview(), row_of(1, 1, 5, red()));
    assert!(es.tool.busy());
}

// Fila 5.

#[test]
fn row_5_end_applies_the_line_as_a_single_step() {
    let mut es = Scene::rgba_8x8(Tool::Line);
    let copy = es.image_2().clone();
    assert!(line_of_row_5(&mut es), "End da Ok(true)");
    assert_eq!(changed(es.image_2(), &copy), set((1..=5).map(|x| (x, 1))));
    for x in 1..=5 {
        assert_eq!(es.image_2().get_pixel(x, 1), Ok(red()));
    }
    assert!(!es.tool.busy());
    assert_eq!(es.preview(), vec![], "nada pendiente tras el End");

    let edited = es.image_2().clone();
    assert_eq!(es.doc.undo(), Ok(()));
    assert_eq!(es.image_2(), &copy, "un solo paso");
    assert!(!es.doc.can_undo());
    assert_eq!(es.doc.redo(), Ok(()));
    assert_eq!(es.image_2(), &edited);
}

// Fila 6.

#[test]
fn row_6_the_intermediate_targets_do_not_accumulate() {
    let mut es = Scene::rgba_8x8(Tool::Line);
    let copy = es.image_2().clone();
    assert!(!es.ev(begin(0, 0)));
    assert!(!es.ev(mov(7, 0)));
    assert!(!es.ev(mov(3, 3)));
    assert!(es.ev(fin()));
    assert_eq!(
        changed(es.image_2(), &copy),
        set([(0, 0), (1, 1), (2, 2), (3, 3)])
    );
    assert_eq!(es.doc.undo(), Ok(()));
    assert_eq!(es.image_2(), &copy, "un solo paso");
}

// Fila 7.

#[test]
fn row_7_a_click_without_movement_paints_a_pixel() {
    let mut es = Scene::rgba_8x8(Tool::Line);
    let copy = es.image_2().clone();
    assert!(!es.ev(begin(2, 3)));
    assert!(es.ev(fin()));
    assert_eq!(changed(es.image_2(), &copy), set([(2, 3)]));
    assert_eq!(es.image_2().get_pixel(2, 3), Ok(red()));
    assert_eq!(es.doc.undo(), Ok(()));
    assert_eq!(es.image_2(), &copy, "un solo paso");
    assert!(!es.doc.can_undo());
}

// Fila 8.

#[test]
fn row_8_the_extreme_outside_is_clipped() {
    let mut es = Scene::rgba_8x8(Tool::Line);
    let copy = es.image_2().clone();
    assert!(!es.ev(begin(-3, 2)));
    assert!(!es.ev(mov(3, 2)));
    assert_eq!(
        es.preview(),
        row_of(2, 0, 3, red()),
        "la vista previa también va recortada"
    );
    assert!(es.ev(fin()));
    assert_eq!(changed(es.image_2(), &copy), set((0..=3).map(|x| (x, 2))));
    assert_eq!(es.doc.undo(), Ok(()));
    assert_eq!(es.image_2(), &copy, "un solo paso");
    assert!(!es.doc.can_undo());
}

#[test]
fn row_8_a_whole_line_outside_gives_no_step_and_does_not_clear_the_redo() {
    let mut es = Scene::rgba_8x8(Tool::Line);
    let copy = es.image_2().clone();
    assert!(line_of_row_5(&mut es));
    assert_eq!(es.doc.undo(), Ok(()));
    assert!(es.doc.can_redo());
    let antes = es.history();

    assert!(!es.ev(begin(-5, -5)));
    assert!(!es.ev(mov(-1, -1)));
    assert_eq!(es.preview(), vec![], "nada dentro del lienzo que mostrar");
    assert!(!es.ev(fin()), "Ok(false): fuera del lienzo");
    assert_eq!(es.image_2(), &copy);
    assert_eq!(es.history(), antes);
    assert!(es.doc.can_redo(), "el rehacer sigue disponible");
    assert!(!es.doc.can_undo());
}

#[test]
fn row_8_repeating_the_line_over_already_red_pixels_gives_no_step() {
    let mut es = Scene::rgba_8x8(Tool::Line);
    assert!(line_of_row_5(&mut es));
    let edited = es.image_2().clone();
    let antes = es.history();

    assert!(!line_of_row_5(&mut es), "Ok(false): sin cambios");
    assert_eq!(es.image_2(), &edited);
    assert_eq!(es.history(), antes);
    // Solo hay un paso: un `undo` lo deja todo como al principio.
    assert_eq!(es.doc.undo(), Ok(()));
    assert!(!es.doc.can_undo());
}

#[test]
fn the_line_uses_the_color_of_the_slot_of_the_begin() {
    // Spec, sección 2: `Begin` guarda el `Pixel` de la ranura activa en ese momento.
    let mut es = Scene::rgba_8x8(Tool::Line);
    es.slot = 3;
    assert!(!es.ev(begin(1, 1)));
    es.slot = 5;
    assert!(!es.ev(mov(5, 1)));
    assert_eq!(es.preview(), row_of(1, 1, 5, red()));
    es.slot = 1;
    assert!(es.ev(fin()));
    for x in 1..=5 {
        assert_eq!(es.image_2().get_pixel(x, 1), Ok(red()));
    }
}

// Fila 9.

#[test]
fn row_9_cancel_discards_the_line_and_the_gesture_stays_busy_until_released() {
    let mut es = Scene::rgba_8x8(Tool::Line);
    let copy = es.image_2().clone();
    let antes = es.history();
    assert!(!es.ev(begin(1, 1)));
    assert!(!es.ev(mov(5, 1)));
    es.tool.apply_action(ToolAction::Cancel);
    assert_eq!(es.preview(), vec![], "la línea pendiente se descarta");
    assert!(es.tool.busy(), "ocupado hasta el End");
    assert!(!es.ev(mov(6, 1)));
    assert!(es.tool.busy());
    assert!(!es.ev(fin()));
    assert!(!es.tool.busy());
    assert_eq!(es.image_2(), &copy);
    assert_eq!(es.history(), antes);
}

#[test]
fn row_9_cancelling_at_rest_does_nothing() {
    for tool_2 in TEST_TOOLS {
        let mut es = Scene::rgba_8x8(tool_2);
        let copy = es.image_2().clone();
        let antes = es.history();
        es.tool.apply_action(ToolAction::Cancel);
        assert_eq!(es.tool.tool(), tool_2);
        assert!(!es.tool.busy());
        assert_eq!(es.preview(), vec![]);
        assert_eq!(es.image_2(), &copy);
        assert_eq!(es.history(), antes);
    }
}

#[test]
fn row_9_cancelling_with_the_pencil_does_not_cancel_the_stroke() {
    let mut es = Scene::rgba_8x8(Tool::Pencil);
    let copy = es.image_2().clone();
    assert!(!es.ev(begin(1, 1)));
    assert!(!es.ev(mov(5, 1)));
    es.tool.apply_action(ToolAction::Cancel);
    assert!(es.tool.busy());
    assert_eq!(es.preview(), row_of(1, 1, 5, red()), "el trazo sigue ahí");
    assert!(es.ev(fin()), "el trazo del lápiz se aplica");
    assert_eq!(changed(es.image_2(), &copy), set((1..=5).map(|x| (x, 1))));
    assert!(es.doc.can_undo());
}

#[test]
fn row_9_cancelling_with_the_fill_does_not_undo_the_fill() {
    let mut es = Scene::rgba_8x8(Tool::Fill);
    let copy = es.image_2().clone();
    assert!(es.ev(begin(3, 3)), "el relleno se hace al pulsar");
    let fills = es.image_2().clone();
    let after_2 = es.history();
    es.tool.apply_action(ToolAction::Cancel);
    assert!(es.tool.busy(), "ocupado hasta el End");
    assert!(!es.ev(mov(0, 0)));
    assert!(!es.ev(fin()));
    assert!(!es.tool.busy());
    assert_eq!(es.image_2(), &fills);
    assert_eq!(es.history(), after_2);
    assert_eq!(es.doc.undo(), Ok(()));
    assert_eq!(es.image_2(), &copy);
}

// Fila 10.

#[test]
fn row_10_changing_tool_during_a_gesture_is_ignored() {
    let mut es = Scene::rgba_8x8(Tool::Line);
    assert!(!es.ev(begin(1, 1)));
    es.tool.apply_action(ToolAction::Select(Tool::Fill));
    assert_eq!(es.tool.tool(), Tool::Line, "ignorado con el gesto en curso");
    assert!(!es.ev(mov(5, 1)));
    es.tool.apply_action(ToolAction::Select(Tool::Pencil));
    assert_eq!(es.tool.tool(), Tool::Line);
    assert!(es.ev(fin()), "y la línea sigue siendo una línea");
    assert_eq!(es.preview(), vec![]);

    es.tool.apply_action(ToolAction::Select(Tool::Fill));
    assert_eq!(es.tool.tool(), Tool::Fill, "tras el End sí cambia");
}

#[test]
fn row_10_a_slot_without_color_leaves_the_gesture_busy_but_changes_nothing() {
    for tool_2 in TEST_TOOLS {
        // Paleta de 3 entradas: la ranura 5 no tiene color.
        let mut es = Scene::new(indexed_image_8x8(), tool_2, 5);
        assert_eq!(pixel_for_slot(es.image_2(), 5), None);
        let copy = es.image_2().clone();
        let antes = es.history();

        assert!(!es.ev(begin(2, 2)), "{tool_2:?}");
        assert!(es.tool.busy(), "{tool_2:?}: ocupado entre Begin y End");
        es.tool.apply_action(ToolAction::Select(Tool::Line));
        es.tool.apply_action(ToolAction::Select(Tool::Fill));
        assert_eq!(es.tool.tool(), tool_2, "{tool_2:?}: sin cambio");
        assert_eq!(es.preview(), vec![], "{tool_2:?}: no hay gesto real");
        assert!(!es.ev(mov(4, 2)));
        assert!(es.tool.busy());
        assert!(!es.ev(fin()));
        assert!(!es.tool.busy(), "{tool_2:?}: no queda bloqueado");

        assert_eq!(es.image_2(), &copy);
        assert_eq!(es.history(), antes);
        es.tool.apply_action(ToolAction::Select(Tool::Fill));
        assert_eq!(es.tool.tool(), Tool::Fill, "{tool_2:?}: ya se puede");
    }
}

// Fila 11.

#[test]
fn row_11_the_fill_is_done_on_press_and_the_drag_does_nothing_more() {
    let mut es = Scene::rgba_8x8(Tool::Fill);
    let copy = es.image_2().clone();
    assert!(es.ev(begin(3, 3)), "Ok(true) en el Begin");
    assert!(!es.ev(mov(0, 0)));
    assert!(!es.ev(mov(7, 7)));
    assert!(es.tool.busy());
    assert!(!es.ev(fin()));
    assert!(!es.tool.busy());

    for y in 0..8 {
        for x in 0..8 {
            assert_eq!(es.image_2().get_pixel(x, y), Ok(red()), "({x}, {y})");
        }
    }
    // Un solo `FillSpans`: lo mismo que `flood_fill` directamente sobre otro documento.
    let mut reference = Document::new(copy.clone());
    let id = ImageId::from_raw(0);
    assert_eq!(flood_fill(&mut reference, id, 3, 3, red()), Ok(true));
    assert_eq!(es.doc.history_bytes(), reference.history_bytes());
    assert_eq!(es.image_2(), reference.image(id).unwrap());

    assert_eq!(es.doc.undo(), Ok(()));
    assert_eq!(es.image_2(), &copy, "un solo paso");
    assert!(!es.doc.can_undo());
}

// Fila 12.

#[test]
fn row_12_seeds_outside_the_canvas_do_nothing() {
    for (x, y) in [(-1, 0), (8, 0), (0, 8), (0, -1), (MIN, MAX)] {
        let mut es = Scene::rgba_8x8(Tool::Fill);
        let copy = es.image_2().clone();
        let antes = es.history();
        assert!(!es.ev(begin(x, y)), "semilla ({x}, {y})");
        assert!(es.tool.busy());
        assert!(!es.ev(fin()));
        assert!(!es.tool.busy());
        assert_eq!(es.image_2(), &copy);
        assert_eq!(es.history(), antes);
    }
}

#[test]
fn row_12_a_seed_already_red_gives_no_step() {
    let mut es = Scene::rgba_8x8(Tool::Fill);
    assert!(es.ev(begin(3, 3)));
    assert!(!es.ev(fin()));
    let fills = es.image_2().clone();
    let antes = es.history();

    assert!(!es.ev(begin(3, 3)), "segundo clic igual: Ok(false)");
    assert!(!es.ev(fin()));
    assert_eq!(es.image_2(), &fills);
    assert_eq!(es.history(), antes);
}

#[test]
fn row_12_a_seed_already_red_after_an_undo_does_not_clear_the_redo() {
    // (0, 0) ya es rojo; el relleno verde (ranura 4) toma todo lo demás y se deshace.
    let mut image_2 = image_8x8();
    image_2.set_pixel(0, 0, red()).unwrap();
    let mut es = Scene::new(image_2.clone(), Tool::Fill, 4);
    assert!(es.ev(begin(5, 5)));
    assert!(!es.ev(fin()));
    assert_eq!(es.doc.undo(), Ok(()));
    assert!(es.doc.can_redo());
    assert_eq!(es.image_2(), &image_2);
    let antes = es.history();

    es.slot = 3;
    assert!(!es.ev(begin(0, 0)), "la semilla ya vale el color activo");
    assert!(!es.ev(fin()));
    assert_eq!(es.image_2(), &image_2);
    assert_eq!(es.history(), antes);
    assert!(es.doc.can_redo(), "el rehacer sigue disponible");
}

// Fila 16.

#[test]
fn row_16_ctrl_d_v_f_do_not_change_tool() {
    // En Linux egui da `ctrl` y `command` a la vez.
    let ctrl_y_command = Modifiers {
        ctrl: true,
        command: true,
        ..Modifiers::NONE
    };
    let combinations = [
        Modifiers::CTRL,
        Modifiers::COMMAND,
        Modifiers::MAC_CMD,
        ctrl_y_command,
    ];
    for modifiers in combinations {
        for key in [Key::D, Key::V, Key::F] {
            assert_eq!(
                tool_action_for_key(key, modifiers),
                None,
                "{key:?} con {modifiers:?}"
            );
        }
    }
}

// Fila 17, primera parte.

#[test]
fn row_17_the_held_fill_does_not_repeat() {
    let mut es = Scene::rgba_8x8(Tool::Fill);
    let copy = es.image_2().clone();
    let mut true_value = usize::from(es.ev(begin(3, 3)));
    for _ in 0..100 {
        true_value += usize::from(es.ev(mov(3, 3)));
    }
    assert!(es.tool.busy());
    true_value += usize::from(es.ev(fin()));
    assert_eq!(true_value, 1, "un solo Ok(true)");
    assert_eq!(es.doc.undo(), Ok(()));
    assert_eq!(es.image_2(), &copy, "un solo paso");
    assert!(!es.doc.can_undo());
}

#[test]
fn canvas_input_with_stroke_events_is_equivalent_to_canvas_event() {
    // La línea de la fila 5 por la vía ordenada: mismos resultados y mismo documento.
    let mut ordered = Scene::rgba_8x8(Tool::Line);
    let mut direct = Scene::rgba_8x8(Tool::Line);
    for event in [begin(1, 1), mov(5, 1), fin()] {
        assert_eq!(ordered.input_2(event), direct.ev(event), "{event:?}");
        assert_eq!(ordered.preview(), direct.preview());
        assert_eq!(ordered.tool.busy(), direct.tool.busy());
    }
    assert_eq!(ordered.image_2(), direct.image_2());
    assert_eq!(ordered.history(), direct.history());
}

// Fila 18.

#[test]
fn row_18_escape_before_the_release_cancels_the_line() {
    let mut es = Scene::rgba_8x8(Tool::Line);
    let copy = es.image_2().clone();
    let antes = es.history();
    assert!(!es.ev(begin(1, 1)));
    assert!(!es.ev(mov(5, 1)));
    assert!(!es.escape(), "Escape: Ok(false)");
    assert!(es.tool.busy(), "ocupado hasta el End");
    assert_eq!(es.preview(), vec![]);
    assert!(!es.ev(mov(5, 1)));
    assert!(es.tool.busy());
    assert!(!es.ev(fin()));
    assert!(!es.tool.busy());
    assert_eq!(es.image_2(), &copy);
    assert_eq!(es.history(), antes);
}

// Fila 19.

#[test]
fn row_19_escape_after_the_release_does_not_undo_the_line() {
    let mut es = Scene::rgba_8x8(Tool::Line);
    let copy = es.image_2().clone();
    assert!(!es.ev(begin(1, 1)));
    assert!(!es.ev(mov(5, 1)));
    assert!(!es.ev(mov(5, 1)));
    assert!(es.ev(fin()), "End aplica la línea");
    assert_eq!(changed(es.image_2(), &copy), set((1..=5).map(|x| (x, 1))));
    for x in 1..=5 {
        assert_eq!(es.image_2().get_pixel(x, 1), Ok(red()));
    }
    let edited = es.image_2().clone();
    let after_2 = es.history();

    assert!(!es.escape(), "Escape: Ok(false)");
    assert!(es.doc.can_undo(), "no deshace");
    assert_eq!(es.image_2(), &edited);
    assert_eq!(es.history(), after_2);
    assert!(!es.tool.busy());
    assert_eq!(es.doc.undo(), Ok(()));
    assert_eq!(es.image_2(), &copy, "un solo paso");
}

#[test]
fn escape_at_rest_with_any_tool_does_nothing() {
    for tool_2 in TEST_TOOLS {
        let mut es = Scene::rgba_8x8(tool_2);
        let copy = es.image_2().clone();
        let antes = es.history();
        assert!(!es.escape());
        assert!(!es.tool.busy());
        assert_eq!(es.tool.tool(), tool_2);
        assert_eq!(es.image_2(), &copy);
        assert_eq!(es.history(), antes);
    }
}

// Oráculo del relleno y estrategias de las propiedades.

/// Recorrido ingenuo pixel a pixel con cola: 4 vecinos y valor exacto (`get_pixel`).
fn region_naive(img: &Image, sx: u32, sy: u32) -> BTreeSet<(u32, u32)> {
    let valor = img.get_pixel(sx, sy).unwrap();
    let mut region = BTreeSet::new();
    let mut queue = VecDeque::new();
    region.insert((sx, sy));
    queue.push_back((sx, sy));
    while let Some((x, y)) = queue.pop_front() {
        let (xi, yi) = (i64::from(x), i64::from(y));
        for (nx, ny) in [(xi - 1, yi), (xi + 1, yi), (xi, yi - 1), (xi, yi + 1)] {
            let (Ok(nx), Ok(ny)) = (u32::try_from(nx), u32::try_from(ny)) else {
                continue;
            };
            if !region.contains(&(nx, ny)) && img.get_pixel(nx, ny) == Ok(valor) {
                region.insert((nx, ny));
                queue.push_back((nx, ny));
            }
        }
    }
    region
}

/// Imagen original con los píxeles de `points` escritos con `pixel`, solo con `set_pixel`.
fn with_points_painted(
    original: &Image,
    points: impl IntoIterator<Item = (u32, u32)>,
    pixel: Pixel,
) -> Image {
    let mut img = original.clone();
    for (x, y) in points {
        img.set_pixel(x, y, pixel).unwrap();
    }
    img
}

fn mode_2() -> impl Strategy<Value = Mode> {
    prop::sample::select(TEST_MODES.to_vec())
}

/// Paleta de seis entradas con casi duplicados: 0 y 1 igual color, 2 mismo RGB con otro alfa.
fn palette_six() -> Palette {
    Palette::from_colors([
        rgba(200, 0, 0, 255),
        rgba(200, 0, 0, 255),
        rgba(200, 0, 0, 128),
        rgba(0, 0, 200, 255),
        rgba(0, 200, 0, 255),
        rgba(5, 5, 5, 0),
    ])
    .unwrap()
}

fn image_empty(mode: Mode, width: u32, height: u32) -> Image {
    match mode {
        Mode::Indexed => Image::new_indexed(width, height, palette_six()).unwrap(),
        _ => Image::new(width, height, mode).unwrap(),
    }
}

/// Valores posibles del fondo, con casi duplicados (hay al menos cuatro por modo).
fn pool_background(mode: Mode) -> Vec<Pixel> {
    match mode {
        Mode::Rgba => vec![
            Pixel::Rgba(rgba(10, 20, 30, 255)),
            Pixel::Rgba(rgba(10, 20, 30, 254)),
            Pixel::Rgba(rgba(10, 20, 30, 0)),
            Pixel::Rgba(rgba(9, 9, 9, 0)),
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
            Pixel::GrayAlpha {
                gray: 5,
                alpha: 254,
            },
            Pixel::GrayAlpha { gray: 5, alpha: 0 },
            Pixel::GrayAlpha { gray: 6, alpha: 0 },
            Pixel::GrayAlpha {
                gray: 6,
                alpha: 255,
            },
        ],
    }
}

/// Valores del fondo más el color de la ranura (si lo tiene): así la herramienta encuentra a
/// veces su propio color ya en la imagen.
fn pool_with_slot(mode: Mode, slot: u8) -> Vec<Pixel> {
    let mut pool = pool_background(mode);
    if let Some(pixel) = pixel_for_slot(&image_empty(mode, 1, 1), slot) {
        pool.push(pixel);
    }
    pool
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
    prop::sample::select(TEST_TOOLS.to_vec())
}

/// Imagen de 1 a 16 de lado en cualquier modo, con fondo variado, y una ranura de 1 a 9.
fn images() -> impl Strategy<Value = (Image, u8)> {
    (mode_2(), 1u32..=16, 1u32..=16, 1u8..=9)
        .prop_flat_map(|(mode, w, h, slot)| {
            let cels = usize::try_from(w * h).unwrap();
            (
                Just((mode, w, h, slot)),
                prop::collection::vec(any::<u8>(), cels),
            )
        })
        .prop_map(|((mode, w, h, slot), cels)| {
            let pool = pool_with_slot(mode, slot);
            let mut image = image_empty(mode, w, h);
            for y in 0..h {
                for x in 0..w {
                    let byte = cels[usize::try_from(y * w + x).unwrap()];
                    // La mitad de las casillas valen el primer valor: ganan las regiones grandes.
                    let k = if byte < 128 {
                        0
                    } else {
                        usize::from(byte) % pool.len()
                    };
                    image.set_pixel(x, y, pool[k]).unwrap();
                }
            }
            (image, slot)
        })
}

/// Imagen con un fondo de 2 a 4 valores (casi duplicados incluidos, y a veces el color de la
/// ranura) y la ranura.
fn short_background_images() -> impl Strategy<Value = (Image, u8)> {
    (mode_2(), 1u32..=16, 1u32..=16, 1u8..=9)
        .prop_flat_map(|(mode, w, h, slot)| {
            let cels = usize::try_from(w * h).unwrap();
            (
                Just((mode, w, h, slot)),
                prop::sample::subsequence(pool_with_slot(mode, slot), 2..=4),
                prop::collection::vec(any::<u8>(), cels),
            )
        })
        .prop_map(|((mode, w, h, slot), background_2, cels)| {
            let mut image = image_empty(mode, w, h);
            for y in 0..h {
                for x in 0..w {
                    let byte = cels[usize::try_from(y * w + x).unwrap()];
                    let k = if byte < 128 {
                        0
                    } else {
                        usize::from(byte) % background_2.len()
                    };
                    image.set_pixel(x, y, background_2[k]).unwrap();
                }
            }
            (image, slot)
        })
}

/// Un paso previo en la historia: `0` nada, `1` un paso, `2` un paso deshecho (hay rehacer).
/// Tras `0` y `2` la imagen vuelve a ser la original.
fn step_previous(es: &mut Scene, variant: u8) {
    if variant == 0 {
        return;
    }
    let actual = es.image_2().get_pixel(0, 0).unwrap();
    let other_2 = pool_background(es.image_2().mode())
        .into_iter()
        .find(|&p| p != actual)
        .expect("el fondo tiene al menos dos valores");
    let command = Command::SetPixels {
        image: es.id,
        pixels: vec![PixelEdit {
            x: 0,
            y: 0,
            pixel: other_2,
        }],
    };
    assert_eq!(es.doc.apply(command), Ok(true));
    if variant == 2 {
        assert_eq!(es.doc.undo(), Ok(()));
    }
}

/// Los píxeles de `line_command(a, b)`, o vacío si no cae ninguno.
fn line_expected(
    es: &Scene,
    a: (i32, i32),
    b: (i32, i32),
    pixel: Pixel,
) -> Result<Vec<PixelEdit>, TestCaseError> {
    match line_command(&es.doc, es.id, a, b, pixel) {
        Ok(Some(Command::SetPixels { pixels, .. })) => Ok(pixels),
        Ok(None) => Ok(vec![]),
        other_2 => Err(TestCaseError::fail(format!(
            "line_command dio {other_2:?} con {a:?} y {b:?}"
        ))),
    }
}

/// `preview` es el conjunto de `expected_2`, sin repetidos y todos con `pixel`.
fn checks_view_preview(
    es: &Scene,
    expected_2: &[PixelEdit],
    pixel: Pixel,
) -> Result<(), TestCaseError> {
    let vista = es.preview();
    prop_assert_eq!(vista.len(), expected_2.len(), "sin repetidos ni faltas");
    for editing in &vista {
        prop_assert_eq!(editing.pixel, pixel);
    }
    let a = set(vista.iter().map(|e| (e.x, e.y)));
    let b = set(expected_2.iter().map(|e| (e.x, e.y)));
    prop_assert_eq!(a, b);
    // Perezosa en cada fotograma: pedirla dos veces da lo mismo.
    prop_assert_eq!(es.preview(), vista);
    Ok(())
}

// Propiedades.

proptest! {
    // Propiedad 1: la vista previa, lo aplicado y `draw_line` coinciden.
    #[test]
    fn property_1_preview_equals_applied_equals_line_command(
        (image, slot) in images(),
        a in position(),
        b in position(),
    ) {
        let original = image.clone();
        let pixel = pixel_for_slot(&original, slot);
        let mut es = Scene::new(image, Tool::Line, slot);

        // Tras `Begin`: la línea de `a` a `a`.
        prop_assert!(!es.ev(begin(a.0, a.1)));
        prop_assert_eq!(es.image_2(), &original);
        prop_assert!(!es.doc.can_undo());
        let (expected_a, expected_ab) = match pixel {
            Some(pixel) => (
                line_expected(&es, a, a, pixel)?,
                line_expected(&es, a, b, pixel)?,
            ),
            None => (vec![], vec![]),
        };
        checks_view_preview(&es, &expected_a, pixel.unwrap_or(Pixel::Gray(0)))?;

        // Tras `Move(b)`: la línea de `a` a `b`; el documento no cambia.
        prop_assert!(!es.ev(mov(b.0, b.1)));
        prop_assert_eq!(es.image_2(), &original);
        prop_assert!(!es.doc.can_undo());
        checks_view_preview(&es, &expected_ab, pixel.unwrap_or(Pixel::Gray(0)))?;

        // Tras `End`: la original con esos píxeles en `pixel` y nada más.
        let change = es.ev(fin());
        let expected = match pixel {
            Some(pixel) => with_points_painted(
                &original,
                expected_ab.iter().map(|e| (e.x, e.y)),
                pixel,
            ),
            None => original.clone(),
        };
        prop_assert_eq!(es.image_2(), &expected);
        prop_assert_eq!(change, expected != original, "Ok(true) si y solo si cambió");
        prop_assert_eq!(es.doc.can_undo(), change);
        prop_assert!(!es.tool.busy());
        prop_assert!(es.preview().is_empty());
    }

    // Propiedad 2: un gesto, un paso.
    #[test]
    fn property_2_one_gesture_one_step(
        (image, slot) in images(),
        gestures in prop::collection::vec(
            (tool_2(), position(), prop::collection::vec(position(), 0..=4)),
            1..=6,
        ),
    ) {
        let initial = image.clone();
        let mut es = Scene::new(image, Tool::Pencil, slot);
        let mut applied = 0usize;
        for (tool_2, source_2, movements) in gestures {
            es.tool.apply_action(ToolAction::Select(tool_2));
            prop_assert_eq!(es.tool.tool(), tool_2);
            let antes = es.image_2().clone();
            let mut true_value = usize::from(es.ev(begin(source_2.0, source_2.1)));
            for (x, y) in movements {
                true_value += usize::from(es.ev(mov(x, y)));
            }
            true_value += usize::from(es.ev(fin()));
            prop_assert!(true_value <= 1, "{:?}: {} pasos en un gesto", tool_2, true_value);
            prop_assert_eq!(
                true_value == 1,
                es.image_2() != &antes,
                "{:?}: Ok(true) si y solo si cambió la imagen",
                tool_2
            );
            prop_assert!(!es.tool.busy());
            applied += true_value;
        }
        let final_ = es.image_2().clone();
        for _ in 0..applied {
            prop_assert_eq!(es.doc.undo(), Ok(()));
        }
        prop_assert_eq!(
            es.doc.undo(),
            Err(DocumentError::NothingToUndo),
            "ni un paso de más"
        );
        prop_assert_eq!(es.image_2(), &initial, "k deshacer dejan la imagen como al principio");
        for _ in 0..applied {
            prop_assert_eq!(es.doc.redo(), Ok(()));
        }
        prop_assert_eq!(es.doc.redo(), Err(DocumentError::NothingToRedo));
        prop_assert_eq!(es.image_2(), &final_, "k rehacer la dejan como al final");
    }

    // Propiedad 3: cancelar.
    #[test]
    fn property_3_cancelling_a_line(
        (image, slot) in images(),
        previous in 0u8..=2,
        a in position(),
        movements in prop::collection::vec(position(), 0..=4),
        cut in 0usize..=4,
        with_escape in any::<bool>(),
    ) {
        let cut = cut.min(movements.len());
        let mut es = Scene::new(image, Tool::Line, slot);
        step_previous(&mut es, previous);
        let image_0 = es.image_2().clone();
        let history_0 = es.history();

        let cancel = |es: &mut Scene| -> bool {
            if with_escape {
                es.escape()
            } else {
                es.tool.apply_action(ToolAction::Cancel);
                false
            }
        };

        prop_assert!(!es.ev(begin(a.0, a.1)));
        for &(x, y) in &movements[..cut] {
            prop_assert!(!es.ev(mov(x, y)));
        }
        prop_assert!(!cancel(&mut es), "cancelar no cambia el documento");
        prop_assert!(es.tool.busy(), "ocupado hasta el End");
        prop_assert!(es.preview().is_empty(), "la línea pendiente se descarta");
        for &(x, y) in &movements[cut..] {
            prop_assert!(!es.ev(mov(x, y)), "los Move posteriores no aplican nada");
            prop_assert!(es.tool.busy());
            prop_assert!(es.preview().is_empty());
        }
        prop_assert!(!es.ev(fin()), "el End posterior no aplica nada");
        prop_assert!(!es.tool.busy());
        prop_assert_eq!(es.image_2(), &image_0);
        prop_assert_eq!(es.history(), history_0);

        // Cancelar después del End: nada de lo aplicado cambia.
        prop_assert!(!es.ev(begin(a.0, a.1)));
        for &(x, y) in &movements {
            prop_assert!(!es.ev(mov(x, y)));
        }
        es.ev(fin());
        let image_1 = es.image_2().clone();
        let history_1 = es.history();
        prop_assert!(!cancel(&mut es));
        prop_assert!(!es.tool.busy());
        prop_assert_eq!(es.image_2(), &image_1);
        prop_assert_eq!(es.history(), history_1);
    }

    // Propiedad 4: el relleno contra el oráculo ingenuo.
    #[test]
    fn property_4_the_fill_is_that_of_the_naive_oracle(
        (image, slot) in short_background_images(),
        seed in position(),
        with_redo in any::<bool>(),
    ) {
        let mut es = Scene::new(image, Tool::Fill, slot);
        step_previous(&mut es, if with_redo { 2 } else { 0 });
        let base = es.image_2().clone();
        let history_0 = es.history();
        let pixel = pixel_for_slot(&base, slot);

        // Qué debe pasar, calculado sin la herramienta.
        let inside = match (u32::try_from(seed.0), u32::try_from(seed.1)) {
            (Ok(x), Ok(y)) if x < base.width() && y < base.height() => Some((x, y)),
            _ => None,
        };
        let expected = match (pixel, inside) {
            (Some(pixel), Some((x, y))) if base.get_pixel(x, y).unwrap() != pixel => {
                let region = region_naive(&base, x, y);
                Some(with_points_painted(&base, region, pixel))
            }
            _ => None,
        };

        let change = es.ev(begin(seed.0, seed.1));
        prop_assert_eq!(change, expected.is_some(), "Ok(true) si y solo si dentro y distinto");
        match &expected {
            Some(expected) => {
                prop_assert_eq!(es.image_2(), expected);
                prop_assert!(es.doc.can_undo());
            }
            None => {
                prop_assert_eq!(es.image_2(), &base);
                prop_assert_eq!(es.history(), history_0, "historia intacta sin cambio");
            }
        }
        prop_assert!(!es.ev(mov(seed.0, seed.1)));
        prop_assert!(!es.ev(fin()));
        prop_assert!(!es.tool.busy());
        if let Some(expected) = expected {
            // Un solo paso, exacto.
            prop_assert_eq!(es.image_2(), &expected);
            prop_assert_eq!(es.doc.undo(), Ok(()));
            prop_assert_eq!(es.image_2(), &base);
            prop_assert_eq!(es.doc.redo(), Ok(()));
            prop_assert_eq!(es.image_2(), &expected);
        }
    }

    // Propiedad 6: herramienta estable y `busy` exacto.
    #[test]
    fn property_6_select_only_changes_the_tool_at_rest(
        (image, slot) in images(),
        operations in prop::collection::vec(operation(), 0..=40),
    ) {
        let mut es = Scene::new(image, Tool::Pencil, slot);
        prop_assert!(!es.tool.busy());
        prop_assert!(es.preview().is_empty(), "vacía en reposo");
        let mut busy = false;
        for operation in operations {
            let tool_before = es.tool.tool();
            match operation {
                Operation::Begin(p) if !busy => {
                    es.ev(begin(p.0, p.1));
                    busy = true;
                }
                Operation::Move(p) if busy => {
                    es.ev(mov(p.0, p.1));
                }
                Operation::End if busy => {
                    es.ev(fin());
                    busy = false;
                }
                Operation::Escape => {
                    es.escape();
                }
                Operation::Cancel => es.tool.apply_action(ToolAction::Cancel),
                Operation::Select(tool_2) => {
                    es.tool.apply_action(ToolAction::Select(tool_2));
                    let expected = if busy { tool_before } else { tool_2 };
                    prop_assert_eq!(es.tool.tool(), expected, "Select con busy = {}", busy);
                }
                _ => {}
            }
            if !matches!(operation, Operation::Select(_)) {
                prop_assert_eq!(es.tool.tool(), tool_before, "solo Select cambia la herramienta");
            }
            prop_assert_eq!(es.tool.busy(), busy, "busy es cierto entre Begin y End");
            if !busy {
                prop_assert!(es.preview().is_empty(), "vacía en reposo");
            }
        }
    }
}

/// Un paso de la secuencia de la propiedad 6. Solo se entregan los que `Canvas` puede emitir:
/// `Begin` en reposo y `Move` y `End` con un gesto abierto; el resto se descarta.
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
        3 => tool_2().prop_map(Operation::Select),
        1 => Just(Operation::Cancel),
    ]
}

// Propiedad 5: teclas.

fn modifiers_2(bits: u8) -> Modifiers {
    Modifiers {
        alt: bits & 1 != 0,
        ctrl: bits & 2 != 0,
        shift: bits & 4 != 0,
        mac_cmd: bits & 8 != 0,
        command: bits & 16 != 0,
    }
}

/// Lo que debe dar `tool_action_for_key`, escrito aparte de la implementación.
fn action_expected(key: Key, modifiers: Modifiers) -> Option<ToolAction> {
    let without_modifiers = !(modifiers.alt
        || modifiers.ctrl
        || modifiers.shift
        || modifiers.mac_cmd
        || modifiers.command);
    if !without_modifiers {
        return None;
    }
    match key {
        Key::D => Some(ToolAction::Select(Tool::Pencil)),
        Key::V => Some(ToolAction::Select(Tool::Line)),
        Key::F => Some(ToolAction::Select(Tool::Fill)),
        Key::Comma => Some(ToolAction::Select(Tool::Dropper)),
        Key::K => Some(ToolAction::Select(Tool::Eraser)),
        Key::Escape => Some(ToolAction::Cancel),
        _ => None,
    }
}

fn checks_keys(key: Key, modifiers: Modifiers) -> Result<(), TestCaseError> {
    let action_2 = tool_action_for_key(key, modifiers);
    prop_assert_eq!(
        action_2,
        action_expected(key, modifiers),
        "{:?} con {:?}",
        key,
        modifiers
    );
    if matches!(
        key,
        Key::D | Key::V | Key::F | Key::Comma | Key::K | Key::Escape
    ) {
        prop_assert_eq!(
            action_for_key(key, modifiers),
            None,
            "action_for_key {:?} con {:?}",
            key,
            modifiers
        );
    }
    Ok(())
}

proptest! {
    #[test]
    fn property_5_keys_by_sampling(
        key in prop::sample::select(Key::ALL.to_vec()),
        bits in 0u8..32,
    ) {
        checks_keys(key, modifiers_2(bits))?;
    }
}

#[test]
fn property_5_exhaustive_keys_key_all_by_the_32_combinations() {
    assert!(!Key::ALL.is_empty());
    for &key in Key::ALL {
        for bits in 0..32 {
            let modifiers = modifiers_2(bits);
            if let Err(e) = checks_keys(key, modifiers) {
                panic!("{e}");
            }
        }
    }
}

// Unidad 14c: límites del relleno (specs/hito-2-limites-relleno.md, fila 11).

mod limits_fill {
    use super::*;

    /// Peine RGBA de 16384 x 368 (`S = 3 006 465`, por encima del tope real): fila 0 toda `A`
    /// y, en el resto, `A` en las columnas pares y `B` en las impares.
    fn comb() -> Image {
        let (width_2, height_2) = (16384usize, 368usize);
        let a = [10u8, 20, 30, 255];
        let b = [40u8, 50, 60, 255];
        let mut data = Vec::with_capacity(width_2 * height_2 * 4);
        for y in 0..height_2 {
            for x in 0..width_2 {
                data.extend_from_slice(if y == 0 || x % 2 == 0 { &a } else { &b });
            }
        }
        Image::from_bytes(16384, 368, Mode::Rgba, data).expect("imagen válida")
    }

    #[test]
    fn row_11_a_too_fragmented_fill_gives_an_error_and_the_next_click_works() {
        let mut es = Scene::new(comb(), Tool::Fill, 3);
        let image_2 = es.image_2().clone();
        let antes = es.history();
        let state_2 = es.doc.state_id();

        // Clic en (0, 0): el relleno se rechaza.
        let result = es
            .tool
            .canvas_event(&mut es.doc, es.id, es.slot, begin(0, 0));
        assert!(
            matches!(result, Err(DocumentError::FillTooFragmented { .. })),
            "salió {result:?}"
        );
        // Soltar termina el gesto (tras el `Err` queda en espera de su `End`).
        assert!(!es.ev(fin()));
        assert!(!es.tool.busy());

        assert!(
            es.tool.take_changes().is_empty(),
            "no hay cambios que registrar"
        );
        assert!(es.image_2() == &image_2, "la imagen no cambia");
        assert_eq!(es.history(), antes);
        assert_eq!(es.doc.state_id(), state_2);
        assert!(!es.doc.can_undo());

        // El mismo estado de la herramienta rellena un documento de 8 x 8 con el mismo clic.
        let mut small = Document::new(image_8x8());
        let id = ImageId::from_raw(0);
        assert_eq!(
            es.tool.canvas_event(&mut small, id, 3, begin(0, 0)),
            Ok(true)
        );
        assert!(!es.ev(fin()));
        assert!(!es.tool.busy());
        for y in 0..8 {
            for x in 0..8 {
                assert_eq!(
                    small.image(id).unwrap().get_pixel(x, y),
                    Ok(red()),
                    "({x}, {y})"
                );
            }
        }
        assert!(
            !es.tool.take_changes().is_empty(),
            "el relleno sí se registra"
        );
    }
}
