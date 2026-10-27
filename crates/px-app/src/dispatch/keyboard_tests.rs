//! Tests de `dispatch` con el teclado como tabla de datos (specs/interfaz-teclado.md, unidad I.1).
//! En ROJO hasta la lógica: `AppAction::{ColorNext, ColorPrev, CenterOnCursor}`,
//! `ViewOp::CenterOnCursor`, `Session::cycle_choice`, `key_events` y `keymap` no existen todavía.
//!
//! Filas de la spec que cubre este fichero (la parte de `dispatch`; `Session::cycle_choice` tiene
//! sus pruebas en `session/color_cycle_tests.rs`):
//!
//! - Fila 7: `ColorNext` y `ColorPrev` por `dispatch` (indexada de 256 con `Entry(255)` y
//!   `Entry(0)`, RGBA con `Slot(9)` y `Slot(1)`, Gray, GrayAlpha, una sola entrada, `choice` que
//!   ya no vale): dan la vuelta, un paso por pulsación, la fila elegida se muestra
//!   (`view.reveal`), el documento, `state_id`, `can_undo` y `can_redo` no cambian.
//! - Fila 8: bloqueos con `Origin::Key` de `ColorNext`, `ColorPrev` y `CenterOnCursor` con cada
//!   combinación de `Locks` y con los gestos de verdad (trazo, deslizador, muestra arrastrada) y la
//!   quita pendiente; con un diálogo o el panel modal abierto, **todas** las filas de la tabla
//!   (también `Inicio`, `Re Pág` y `Av Pág`) dan `Blocked` (salvo `Salir` con el modal, que pasa
//!   desde la unidad 18). El banco de egui con `gate_keys` y con el diálogo está en
//!   `px-ui/tests/keymap.rs` (módulo `fila_8`).
//! - Fila 9: menú, botón y tecla de `Undo`, las cuatro herramientas con letra, `ZoomIn`,
//!   `ZoomOut`, `ColorNext` y `SelectTool(Eraser)` (solo botón): mismas acciones, mismos
//!   `Dispatched`, mismo estado final.
//! - Fila 10 (parte de `dispatch`): `dispatch(SelectColor(3))` directo en indexada de 9 y en RGBA
//!   hace lo de antes; por teclado, las cifras no hacen nada.
//! - Fila 12: `ignores_repeat` de las acciones de la tabla y la repetición traducida como lo hace
//!   `key_actions`: `u`, `]`, `Av Pág` y `<` repetidos repiten (n pulsaciones, n pasos); `Open`,
//!   `Save`, `SaveAs`, `NewImage` y `Quit` mantenidos no repiten.
//!
//! Los ayudantes de `dispatch/tests.rs` son privados: aquí se duplican los mínimos (`World`,
//! `TestSnapshot`, los gestos de verdad).
//!
//! Suposiciones sobre lo que la spec no fija (anotadas en el informe):
//! - `dispatch(ColorNext | ColorPrev)` da `Done { refresh: false, notice: None }` (como
//!   `SelectColor`) y `dispatch(CenterOnCursor)` da `Dispatched::View(ViewOp::CenterOnCursor)`.
//! - `key_actions` se traduce en `actions_of_keyboard`: `presses`, la tabla por defecto y el
//!   descarte de las repeticiones de las acciones con `ignores_repeat`.
//! - Con `modal` el atajo de salir (`Ctrl+Q`, Linux) sigue pasando (regla 2 de `is_blocked`).

use eframe::egui::{Event, Key, Modifiers};
use px_core::ImageId;
use px_core::{Command, Document, Image, Mode, Palette, Pixel, PixelEdit, Rgba};
use px_ui::actions::{AppAction, Locks, Origin, Platform, UiState, enabled, is_blocked};
use px_ui::canvas::CanvasEvent;
use px_ui::color::{ColorChoice, initial_choice};
use px_ui::color_edit::Control;
use px_ui::key_events::presses;
use px_ui::keymap::Keymap;
use px_ui::menu::MENUS;
use px_ui::palette_ops::{Removal, default_destination};
use px_ui::shortcuts::FileAction;
use px_ui::tool::{Tool, ToolState};
use px_ui::toolbar::{TOOL_BUTTONS, ZOOM_BUTTONS};
use px_ui::view::{ViewOp, Zoom};

use super::{Dispatched, Targets, dispatch};
use crate::session::Session;

// ---------------------------------------------------------------------------------------------
// Ayudantes (copia de los mínimos de `dispatch/tests.rs`)
// ---------------------------------------------------------------------------------------------

const PLATFORMS: [Platform; 3] = [Platform::Linux, Platform::Windows, Platform::Mac];
const SOURCES: [Origin; 3] = [Origin::Menu, Origin::Button, Origin::Key];
const NO_INDEX_NOTICE: &str = "esta imagen no tiene índice transparente";

fn different(n: usize) -> Vec<Rgba> {
    (0..n)
        .map(|i| Rgba::opaque(i as u8, 255 - i as u8, (i * 7) as u8))
        .collect()
}

fn rgba() -> Image {
    Image::new(4, 4, Mode::Rgba).expect("imagen RGBA")
}

fn gray() -> Image {
    Image::new(4, 4, Mode::Gray).expect("imagen Gray")
}

fn gray_alpha() -> Image {
    Image::new(4, 4, Mode::GrayAlpha).expect("imagen GrayAlpha")
}

/// Indexada de 4×4 con `n` entradas distintas y sin índice transparente.
fn indexed(n: usize) -> Image {
    let palette = Palette::from_colors(different(n)).expect("paleta");
    Image::new_indexed(4, 4, palette).expect("imagen indexada")
}

fn pixel_different(image: &Image, i: usize) -> Pixel {
    match image.mode() {
        Mode::Rgba => Pixel::Rgba(Rgba::opaque(10 + i as u8, 20, 30)),
        Mode::Indexed => {
            let len = image.palette().map_or(1, |p| p.len());
            Pixel::Indexed(((i % (len - 1).max(1)) + 1).min(len - 1) as u8)
        }
        Mode::Gray => Pixel::Gray(10 + i as u8),
        Mode::GrayAlpha => Pixel::GrayAlpha {
            gray: 10 + i as u8,
            alpha: 255,
        },
    }
}

/// Un paso de historia: escribe un píxel de una fila distinta cada vez.
fn a_step(document: &mut Document, i: usize) {
    let id = ImageId::from_raw(0);
    let image = document.image(id).expect("imagen").clone();
    let x = (i / 4) as u32 % 4;
    let y = (i % 4) as u32;
    let changed = document
        .apply(Command::SetPixels {
            image: id,
            pixels: vec![PixelEdit {
                x,
                y,
                pixel: pixel_different(&image, i),
            }],
        })
        .expect("comando válido");
    assert!(changed, "el paso {i} debe cambiar la imagen");
}

/// Sesión y herramienta: todo lo que `dispatch` puede tocar.
struct World {
    session: Session,
    tools: ToolState,
}

/// Lo que se compara entre orígenes y lo que un `Blocked` no debe tocar.
#[derive(Clone, Debug, PartialEq)]
struct TestSnapshot {
    state_id: u64,
    can_undo: bool,
    can_redo: bool,
    image: Image,
    choice: ColorChoice,
    tool: Tool,
    removal: Option<Removal>,
    reveal: bool,
    gesture: bool,
}

/// Lo que ni elegir color ni centrar la vista tocan: el documento y su historia.
#[derive(Clone, Debug, PartialEq)]
struct TestDocument {
    state_id: u64,
    can_undo: bool,
    can_redo: bool,
    image: Image,
}

impl World {
    /// Sesión con `steps_2` pasos de historia y, de ellos, `undone` deshechos.
    fn new_2(image: Image, steps_2: usize, undone: usize) -> World {
        let mut document = Document::new(image);
        for i in 0..steps_2 {
            a_step(&mut document, i);
        }
        let mut session = Session::sample(document);
        for _ in 0..undone {
            session.undo().expect("hay un paso que deshacer");
        }
        let _ = session.take_removal_cancelled();
        session.view.reveal = false;
        World {
            session,
            tools: ToolState::new(),
        }
    }

    /// Indexada de 4 entradas sin transparente, con 2 pasos y 1 deshecho.
    fn standard() -> World {
        World::new_2(indexed(4), 2, 1)
    }

    fn image_2(&self) -> &Image {
        self.session
            .document
            .image(ImageId::from_raw(0))
            .expect("imagen")
    }

    fn locks(&self) -> Locks {
        Locks {
            gesture: self.tools.busy() || self.session.busy(),
            stroke: self.tools.busy() && !self.session.busy(),
            removal: self.session.ops.removal().is_some(),
            ..Locks::default()
        }
    }

    fn dispatches_with(&mut self, action: AppAction, origin: Origin, locks: Locks) -> Dispatched {
        dispatch(
            action,
            origin,
            &mut Targets {
                session: &mut self.session,
                tools: &mut self.tools,
                locks,
            },
        )
    }

    /// Despacha con los bloqueos del estado real.
    fn dispatches(&mut self, action: AppAction, origin: Origin) -> Dispatched {
        let locks = self.locks();
        self.dispatches_with(action, origin, locks)
    }

    fn snapshot(&self) -> TestSnapshot {
        TestSnapshot {
            state_id: self.session.document.state_id(),
            can_undo: self.session.document.can_undo(),
            can_redo: self.session.document.can_redo(),
            image: self.image_2().clone(),
            choice: self.session.choice,
            tool: self.tools.tool(),
            removal: self.session.ops.removal(),
            reveal: self.session.view.reveal,
            gesture: self.tools.busy() || self.session.busy(),
        }
    }

    fn document(&self) -> TestDocument {
        TestDocument {
            state_id: self.session.document.state_id(),
            can_undo: self.session.document.can_undo(),
            can_redo: self.session.document.can_redo(),
            image: self.image_2().clone(),
        }
    }

    fn ui_state(&self, locks: Locks) -> UiState {
        UiState {
            can_undo: self.session.document.can_undo(),
            can_redo: self.session.document.can_redo(),
            zoom: Zoom::from_int(5),
            tool: self.tools.tool(),
            locks,
        }
    }
}

fn done(refresh: bool) -> Dispatched {
    Dispatched::Done {
        refresh,
        notice: None,
    }
}

fn locks_with(f: impl FnOnce(&mut Locks)) -> Locks {
    let mut locks = Locks::default();
    f(&mut locks);
    locks
}

fn locks_2(bits: u8) -> Locks {
    Locks {
        gesture: bits & 1 != 0,
        stroke: false,
        dialog: bits & 2 != 0,
        text_focus: bits & 4 != 0,
        removal: bits & 8 != 0,
        modal: bits & 16 != 0,
    }
}

// --- Teclas como las entrega egui ---

fn key_2(key: Key, modifiers: Modifiers) -> Event {
    Event::Key {
        key,
        physical_key: Some(key),
        pressed: true,
        repeat: false,
        modifiers,
    }
}

fn repeated(key: Key, modifiers: Modifiers) -> Event {
    Event::Key {
        key,
        physical_key: Some(key),
        pressed: true,
        repeat: true,
        modifiers,
    }
}

fn text_2(t: &str) -> Event {
    Event::Text(t.to_string())
}

/// La tecla con el texto que produce, como la entrega `egui-winit`.
fn writes(key: Key, modifiers: Modifiers, t: &str) -> Vec<Event> {
    vec![key_2(key, modifiers), text_2(t)]
}

const CTRL: Modifiers = Modifiers {
    alt: false,
    ctrl: true,
    shift: false,
    mac_cmd: false,
    command: true,
};

/// `key_actions` traducido: las pulsaciones de `presses`, la tabla por defecto y, si la tecla se
/// repite, las acciones que no repiten se descartan (`ignores_repeat`).
fn actions_of_keyboard(events: &[Event], platform: Platform) -> Vec<AppAction> {
    let km = Keymap::default_scheme();
    presses(events)
        .iter()
        .filter_map(|p| km.action_for(p, platform).map(|a| (p.repeat, a)))
        .filter(|(repeat, action)| !(*repeat && action.ignores_repeat()))
        .map(|(_, action)| action)
        .collect()
}

/// La primera (y única) acción que da una secuencia.
fn a_action(events: &[Event], platform: Platform) -> AppAction {
    let actions = actions_of_keyboard(events, platform);
    assert_eq!(actions.len(), 1, "{events:?} en {platform:?}: {actions:?}");
    actions[0]
}

// --- Gestos de verdad ---

fn starts_a_stroke(world: &mut World) {
    let id = ImageId::from_raw(0);
    let pixel = world.session.active_pixel();
    assert!(pixel.is_some(), "hay un color activo");
    world
        .tools
        .canvas_event_with(
            &mut world.session.document,
            id,
            pixel,
            CanvasEvent::StrokeBegin((0, 0)),
        )
        .expect("pulsar");
    assert!(world.tools.busy(), "el trazo está en curso");
}

fn presses_the_slider(world: &mut World) {
    world.session.sync_editor();
    assert!(
        world.session.editor.begin(Control::Red, 10),
        "el editor admite el arrastre"
    );
    assert!(world.session.busy());
}

fn drags_a_swatch(world: &mut World) {
    world.session.ops.begin_drag(1, (0, 0));
    world
        .session
        .ops
        .drag()
        .expect("hay arrastre")
        .update((200, 200), 1, Some(2));
    assert!(world.session.busy());
}

/// Quita pendiente de la entrada 3 de una indexada de 5 (que usan dos píxeles).
fn world_with_a_pending_removal() -> World {
    let palette = Palette::from_colors(different(5)).expect("paleta");
    let mut image = Image::new_indexed(5, 1, palette).expect("imagen indexada");
    for (x, i) in [0u8, 1, 2, 3, 3].into_iter().enumerate() {
        image
            .set_pixel(x as u32, 0, Pixel::Indexed(i))
            .expect("píxel");
    }
    let mut session = Session::sample(Document::new(image));
    session.set_choice_revealing(ColorChoice::Entry(3));
    session.sync_editor();
    session.normalize();
    let _ = session.take_removal_cancelled();
    session.view.reveal = false;
    let target_2 = {
        let image = session
            .document
            .image(ImageId::from_raw(0))
            .expect("imagen");
        default_destination(image.palette().expect("indexada"), 3)
    };
    session.ops.begin_removal(3, target_2);
    assert!(session.ops.removal().is_some());
    World {
        session,
        tools: ToolState::new(),
    }
}

#[derive(Clone, Copy, Debug)]
enum TestGesture {
    Stroke,
    Slider,
    Swatch,
}

const GESTURES: [TestGesture; 3] = [
    TestGesture::Stroke,
    TestGesture::Slider,
    TestGesture::Swatch,
];

fn world_with_gesture(gesture: TestGesture) -> World {
    let mut world = World::standard();
    match gesture {
        TestGesture::Stroke => starts_a_stroke(&mut world),
        TestGesture::Slider => presses_the_slider(&mut world),
        TestGesture::Swatch => drags_a_swatch(&mut world),
    }
    assert!(world.locks().gesture, "{gesture:?}");
    world
}

// ---------------------------------------------------------------------------------------------
// Fila 7: color siguiente y anterior por dispatch
// ---------------------------------------------------------------------------------------------

/// Pulsa `acción` con `Origin::Key` y comprueba lo común: `Done` sin refresco ni aviso, el
/// documento intacto y la fila elegida marcada para mostrarse.
fn presses_and_checks(world: &mut World, action: AppAction, expected: ColorChoice, context: &str) {
    let document = world.document();
    world.session.view.reveal = false;
    let result = world.dispatches(action, Origin::Key);
    assert_eq!(result, done(false), "{context}: {action:?}");
    assert_eq!(world.session.choice, expected, "{context}: {action:?}");
    assert!(
        world.session.view.reveal,
        "{context}: {action:?} no muestra la fila elegida"
    );
    assert_eq!(
        world.document(),
        document,
        "{context}: {action:?} cambió el documento"
    );
}

#[test]
fn row_7_indexed_of_256_color_next_at_the_last_gives_the_first_and_color_prev_at_the_first_the_last()
 {
    let mut world = World::new_2(indexed(256), 2, 1);
    world.session.set_choice_revealing(ColorChoice::Entry(255));
    presses_and_checks(
        &mut world,
        AppAction::ColorNext,
        ColorChoice::Entry(0),
        "256, de la última",
    );
    presses_and_checks(
        &mut world,
        AppAction::ColorPrev,
        ColorChoice::Entry(255),
        "256, de la primera",
    );
}

#[test]
fn row_7_indexed_of_256_one_step_per_press_in_both_directions() {
    let mut world = World::new_2(indexed(256), 0, 0);
    assert_eq!(world.session.choice, ColorChoice::Entry(0));
    for input in 1..=10u8 {
        presses_and_checks(
            &mut world,
            AppAction::ColorNext,
            ColorChoice::Entry(input),
            "256 hacia delante",
        );
    }
    for input in (0..10u8).rev() {
        presses_and_checks(
            &mut world,
            AppAction::ColorPrev,
            ColorChoice::Entry(input),
            "256 hacia atrás",
        );
    }
}

#[test]
fn row_7_indexed_of_9_wraps_around_in_both_directions_and_walks_all_the_entries() {
    let mut world = World::new_2(indexed(9), 0, 0);
    let mut views = vec![world.session.choice];
    for _ in 0..9 {
        world.dispatches(AppAction::ColorNext, Origin::Key);
        views.push(world.session.choice);
    }
    let expected_2_2: Vec<ColorChoice> = (0..=9u8).map(|i| ColorChoice::Entry(i % 9)).collect();
    assert_eq!(
        views, expected_2_2,
        "nueve pasos hacia delante y vuelta al principio"
    );
    let mut back = vec![world.session.choice];
    for _ in 0..9 {
        world.dispatches(AppAction::ColorPrev, Origin::Key);
        back.push(world.session.choice);
    }
    let expected_2_2: Vec<ColorChoice> =
        (0..=9u8).map(|i| ColorChoice::Entry((9 - i) % 9)).collect();
    assert_eq!(
        back, expected_2_2,
        "nueve pasos hacia atrás y vuelta al principio"
    );
}

#[test]
fn row_7_rgba_slot_9_gives_slot_1_and_slot_1_gives_slot_9() {
    let mut world = World::new_2(rgba(), 2, 1);
    world.session.set_choice_revealing(ColorChoice::Slot(9));
    presses_and_checks(
        &mut world,
        AppAction::ColorNext,
        ColorChoice::Slot(1),
        "RGBA",
    );
    presses_and_checks(
        &mut world,
        AppAction::ColorPrev,
        ColorChoice::Slot(9),
        "RGBA",
    );
}

#[test]
fn row_7_rgba_walks_the_nine_slots_one_step_per_press() {
    let mut world = World::new_2(rgba(), 0, 0);
    assert_eq!(world.session.choice, ColorChoice::Slot(1));
    for slot in 2..=9u8 {
        presses_and_checks(
            &mut world,
            AppAction::ColorNext,
            ColorChoice::Slot(slot),
            "RGBA hacia delante",
        );
    }
    for slot in (1..=8u8).rev() {
        presses_and_checks(
            &mut world,
            AppAction::ColorPrev,
            ColorChoice::Slot(slot),
            "RGBA hacia atrás",
        );
    }
}

#[test]
fn row_7_gray_and_gray_alpha_also_wrap_around_in_the_nine_slots() {
    for (name_2, image_2) in [("Gray", gray()), ("GrayAlpha", gray_alpha())] {
        let mut world = World::new_2(image_2, 2, 1);
        world.session.set_choice_revealing(ColorChoice::Slot(9));
        presses_and_checks(
            &mut world,
            AppAction::ColorNext,
            ColorChoice::Slot(1),
            name_2,
        );
        presses_and_checks(
            &mut world,
            AppAction::ColorPrev,
            ColorChoice::Slot(9),
            name_2,
        );
        presses_and_checks(
            &mut world,
            AppAction::ColorNext,
            ColorChoice::Slot(1),
            name_2,
        );
        presses_and_checks(
            &mut world,
            AppAction::ColorNext,
            ColorChoice::Slot(2),
            name_2,
        );
    }
}

#[test]
fn row_7_choosing_a_color_does_not_change_the_document_or_the_history() {
    for image in [indexed(9), rgba(), gray(), gray_alpha()] {
        let mut world = World::new_2(image, 3, 1);
        let before = world.document();
        for _ in 0..12 {
            world.dispatches(AppAction::ColorNext, Origin::Key);
            world.dispatches(AppAction::ColorPrev, Origin::Key);
            world.dispatches(AppAction::ColorPrev, Origin::Key);
        }
        assert_eq!(world.document(), before, "{:?}", world.image_2().mode());
    }
}

#[test]
fn row_7_with_a_single_entry_color_next_and_color_prev_stay_where_they_are() {
    let mut world = World::new_2(indexed(1), 0, 0);
    let document = world.document();
    for action in [AppAction::ColorNext, AppAction::ColorPrev] {
        assert_eq!(
            world.dispatches(action, Origin::Key),
            done(false),
            "{action:?}"
        );
        assert_eq!(world.session.choice, ColorChoice::Entry(0), "{action:?}");
    }
    assert_eq!(world.document(), document);
}

#[test]
fn row_7_with_a_choice_that_is_no_longer_valid_it_gives_the_initial_choice() {
    // Indexada con una ranura sin entrada (`Slot(n)` sobre una indexada solo vale si hay entrada
    // `n − 1`), indexada con una entrada fuera de la paleta, y los otros modos con una entrada
    // (no vale): `initial_choice`.
    let cases: Vec<(&str, Image, ColorChoice)> = vec![
        (
            "indexada con una ranura sin entrada",
            indexed(4),
            ColorChoice::Slot(9),
        ),
        (
            "indexada con entrada fuera",
            indexed(4),
            ColorChoice::Entry(200),
        ),
        ("RGBA con Entry", rgba(), ColorChoice::Entry(3)),
        ("Gray con Entry", gray(), ColorChoice::Entry(0)),
        ("GrayAlpha con Entry", gray_alpha(), ColorChoice::Entry(1)),
    ];
    for (name_2, image, invalid) in cases {
        for action in [AppAction::ColorNext, AppAction::ColorPrev] {
            let mut world = World::new_2(image.clone(), 0, 0);
            let initial = initial_choice(world.image_2());
            world.session.choice = invalid;
            let document = world.document();
            assert_eq!(
                world.dispatches(action, Origin::Key),
                done(false),
                "{name_2}: {action:?}"
            );
            assert_eq!(world.session.choice, initial, "{name_2}: {action:?}");
            assert_eq!(world.document(), document, "{name_2}: {action:?}");
        }
    }
}

#[test]
fn row_7_the_menu_and_the_button_give_the_same_result_as_the_key_with_different_sources() {
    for origin in SOURCES {
        let mut world = World::new_2(indexed(9), 0, 0);
        assert_eq!(
            world.dispatches(AppAction::ColorNext, origin),
            done(false),
            "{origin:?}"
        );
        assert_eq!(world.session.choice, ColorChoice::Entry(1), "{origin:?}");
    }
}

// ---------------------------------------------------------------------------------------------
// Fila 8: bloqueos con Origin::Key
// ---------------------------------------------------------------------------------------------

#[test]
fn row_8_color_next_and_color_prev_are_blocked_with_each_lock_and_source_like_select_color() {
    for action in [AppAction::ColorNext, AppAction::ColorPrev] {
        for bits in 0..32u8 {
            let locks = locks_2(bits);
            for origin in SOURCES {
                let mut world = World::standard();
                let before = world.snapshot();
                let result = world.dispatches_with(action, origin, locks);
                // Las mismas reglas que `SelectColor`: cualquier bloqueo (gesto, campo de texto,
                // quita pendiente, diálogo y modal) la para.
                let locked_2 = bits != 0;
                assert_eq!(
                    result == Dispatched::Blocked,
                    locked_2,
                    "{action:?} con {locks:?} y {origin:?}: {result:?}"
                );
                assert_eq!(
                    is_blocked(action, locks, origin),
                    locked_2,
                    "{action:?} con {locks:?} y {origin:?}"
                );
                assert_eq!(
                    is_blocked(action, locks, origin),
                    is_blocked(AppAction::SelectColor(3), locks, origin),
                    "{action:?} se bloquea como SelectColor con {locks:?} y {origin:?}"
                );
                if locked_2 {
                    assert_eq!(world.snapshot(), before, "{action:?} con {locks:?}");
                } else {
                    assert_eq!(result, done(false));
                    assert_ne!(world.session.choice, before.choice);
                }
            }
        }
    }
}

/// Antes de I.2a (decisión 8 de I.1) el color se bloqueaba con cualquier gesto; ahora un trazo de
/// una herramienta no lo bloquea (el trazo conserva el color con el que empezó) y el deslizador del
/// editor y el arrastre de una muestra siguen bloqueándolo.
#[test]
fn row_8_color_next_and_color_prev_with_a_stroke_in_progress_are_still_allowed() {
    for action in [AppAction::ColorNext, AppAction::ColorPrev] {
        let mut world = world_with_gesture(TestGesture::Stroke);
        let before = world.snapshot();
        assert_eq!(
            world.dispatches(action, Origin::Key),
            done(false),
            "{action:?}"
        );
        assert_ne!(world.session.choice, before.choice, "{action:?}");
        assert!(world.tools.busy(), "el trazo sigue en curso");
    }
}

#[test]
fn row_8_color_next_and_color_prev_do_nothing_with_the_slider_or_a_swatch_being_dragged() {
    for gesture in [TestGesture::Slider, TestGesture::Swatch] {
        for action in [AppAction::ColorNext, AppAction::ColorPrev] {
            let mut world = world_with_gesture(gesture);
            let before = world.snapshot();
            assert_eq!(
                world.dispatches(action, Origin::Key),
                Dispatched::Blocked,
                "{gesture:?}: {action:?}"
            );
            assert_eq!(world.snapshot(), before, "{gesture:?}: {action:?}");
        }
    }
}

#[test]
fn row_8_color_next_and_color_prev_are_blocked_with_a_pending_removal() {
    for action in [AppAction::ColorNext, AppAction::ColorPrev] {
        let mut world = world_with_a_pending_removal();
        let before = world.snapshot();
        assert!(world.locks().removal);
        assert_eq!(
            world.dispatches(action, Origin::Key),
            Dispatched::Blocked,
            "{action:?}"
        );
        assert_eq!(world.snapshot(), before, "{action:?}");
    }
}

#[test]
fn row_8_center_on_cursor_is_blocked_only_by_a_modal_dialog_and_by_a_key_with_a_focused_field() {
    for bits in 0..32u8 {
        let locks = locks_2(bits);
        for origin in SOURCES {
            let mut world = World::standard();
            let before = world.snapshot();
            let result = world.dispatches_with(AppAction::CenterOnCursor, origin, locks);
            let locked_2 =
                locks.dialog || locks.modal || (locks.text_focus && origin == Origin::Key);
            assert_eq!(
                result == Dispatched::Blocked,
                locked_2,
                "CenterOnCursor con {locks:?} y {origin:?}: {result:?}"
            );
            assert_eq!(
                is_blocked(AppAction::CenterOnCursor, locks, origin),
                locked_2,
                "CenterOnCursor con {locks:?} y {origin:?}"
            );
            if !locked_2 {
                assert_eq!(result, Dispatched::View(ViewOp::CenterOnCursor));
            }
            // Es una acción de vista: no toca el documento ni la elección ni la herramienta.
            assert_eq!(world.snapshot(), before, "CenterOnCursor con {locks:?}");
        }
    }
}

#[test]
fn row_8_center_on_cursor_is_stopped_neither_by_a_real_gesture_nor_by_a_pending_removal() {
    for gesture in GESTURES {
        let mut world = world_with_gesture(gesture);
        let before = world.snapshot();
        assert_eq!(
            world.dispatches(AppAction::CenterOnCursor, Origin::Key),
            Dispatched::View(ViewOp::CenterOnCursor),
            "{gesture:?}"
        );
        assert_eq!(world.snapshot(), before, "{gesture:?}");
    }
    let mut world = world_with_a_pending_removal();
    let before = world.snapshot();
    assert_eq!(
        world.dispatches(AppAction::CenterOnCursor, Origin::Key),
        Dispatched::View(ViewOp::CenterOnCursor)
    );
    assert_eq!(world.snapshot(), before);
}

#[test]
fn row_8_is_view_is_the_zoom_plus_center_and_is_zoom_does_not_change() {
    assert!(AppAction::CenterOnCursor.is_view());
    for action in [
        AppAction::ZoomIn,
        AppAction::ZoomOut,
        AppAction::ZoomFit,
        AppAction::ZoomActualSize,
    ] {
        assert!(action.is_view(), "{action:?}");
        assert!(action.is_zoom(), "{action:?}");
    }
    // `is_zoom` no cambia: el centrado no es zoom.
    assert!(!AppAction::CenterOnCursor.is_zoom());
    for action in [
        AppAction::ColorNext,
        AppAction::ColorPrev,
        AppAction::Undo,
        AppAction::SelectTool(Tool::Pencil),
        AppAction::SelectColor(3),
        AppAction::Open,
    ] {
        assert!(!action.is_view(), "{action:?}");
    }
}

#[test]
fn row_8_enabled_is_the_opposite_of_blocked_with_menu_source_in_the_new_actions() {
    for action in [
        AppAction::ColorNext,
        AppAction::ColorPrev,
        AppAction::CenterOnCursor,
    ] {
        for bits in 0..32u8 {
            let locks = locks_2(bits);
            let world = World::standard();
            assert_eq!(
                enabled(action, &world.ui_state(locks)),
                !is_blocked(action, locks, Origin::Menu),
                "{action:?} con {locks:?}"
            );
        }
    }
}

/// Todas las filas activas de la tabla por defecto, con la plataforma donde valen.
fn rows_active() -> Vec<(AppAction, Platform)> {
    let km = Keymap::default_scheme();
    let mut v = Vec::new();
    for platform in PLATFORMS {
        for row in km.rows() {
            let p = px_ui::keymap::Press {
                key: Some(row.key),
                command: row.command,
                shift: row.shift,
                alt: false,
                other_ctrl: false,
                repeat: false,
            };
            if km.action_for(&p, platform) == Some(row.action) {
                v.push((row.action, platform));
            }
        }
    }
    v
}

#[test]
fn row_8_with_a_file_dialog_every_row_of_the_table_gives_blocked_also_home_page_up_and_page_down() {
    let rows = rows_active();
    assert_eq!(
        rows.len(),
        38 + 37 + 37,
        "filas activas en las tres plataformas"
    );
    for (action, platform) in rows {
        let mut world = World::standard();
        let before = world.snapshot();
        let locks = locks_with(|l| l.dialog = true);
        assert_eq!(
            world.dispatches_with(action, Origin::Key, locks),
            Dispatched::Blocked,
            "{action:?} en {platform:?} con un diálogo de archivo"
        );
        assert_eq!(world.snapshot(), before, "{action:?} en {platform:?}");
    }
}

#[test]
fn row_8_with_the_modal_panel_every_row_gives_blocked_except_quit() {
    for (action, platform) in rows_active() {
        let mut world = World::standard();
        let before = world.snapshot();
        let locks = locks_with(|l| l.modal = true);
        let result = world.dispatches_with(action, Origin::Key, locks);
        if action == AppAction::Quit {
            // `Salir` pasa con el panel modal (unidad 18, D1).
            assert_eq!(result, Dispatched::Quit, "{platform:?}");
        } else {
            assert_eq!(
                result,
                Dispatched::Blocked,
                "{action:?} en {platform:?} con el panel modal"
            );
        }
        assert_eq!(world.snapshot(), before, "{action:?} en {platform:?}");
    }
}

#[test]
fn row_8_with_a_modal_the_page_up_page_down_home_keys_give_blocked_and_end_gives_no_action() {
    let locks = locks_with(|l| l.modal = true);
    for (key, expected) in [
        (Key::PageUp, Some(AppAction::ColorPrev)),
        (Key::PageDown, Some(AppAction::ColorNext)),
        (Key::Home, Some(AppAction::ZoomFit)),
        (Key::End, None),
    ] {
        for platform in PLATFORMS {
            let actions = actions_of_keyboard(&[key_2(key, Modifiers::NONE)], platform);
            assert_eq!(actions, expected.into_iter().collect::<Vec<_>>(), "{key:?}");
            for action in actions {
                let mut world = World::standard();
                let before = world.snapshot();
                assert_eq!(
                    world.dispatches_with(action, Origin::Key, locks),
                    Dispatched::Blocked,
                    "{key:?} en {platform:?}"
                );
                assert_eq!(world.snapshot(), before);
            }
        }
    }
}

// ---------------------------------------------------------------------------------------------
// Fila 9: la misma acción, el mismo efecto
// ---------------------------------------------------------------------------------------------

fn by_menu_label(label: &str) -> AppAction {
    MENUS
        .iter()
        .flat_map(|m| m.items.iter())
        .find(|i| i.label == label)
        .unwrap_or_else(|| panic!("no hay entrada de menú {label:?}"))
        .action
}

fn by_button_label(label: &str) -> AppAction {
    TOOL_BUTTONS
        .iter()
        .chain(ZOOM_BUTTONS.iter())
        .find(|b| b.label == label)
        .unwrap_or_else(|| panic!("no hay botón {label:?}"))
        .action
}

struct TestCase {
    expected: AppAction,
    menu: Option<&'static str>,
    button_2: Option<&'static str>,
    /// Cada secuencia de eventos que tiene que dar `expected` (una tecla, o dos atajos).
    keys_2: Vec<Vec<Event>>,
}

fn cases_of_row_9() -> Vec<TestCase> {
    let s = Modifiers::SHIFT;
    vec![
        TestCase {
            expected: AppAction::Undo,
            menu: Some("Deshacer"),
            button_2: None,
            keys_2: vec![
                vec![key_2(Key::Z, CTRL)],
                writes(Key::U, Modifiers::NONE, "u"),
            ],
        },
        TestCase {
            expected: AppAction::SelectTool(Tool::Pencil),
            menu: None,
            button_2: Some("Lápiz"),
            keys_2: vec![writes(Key::D, Modifiers::NONE, "d")],
        },
        TestCase {
            expected: AppAction::SelectTool(Tool::Line),
            menu: None,
            button_2: Some("Línea"),
            keys_2: vec![writes(Key::V, Modifiers::NONE, "v")],
        },
        TestCase {
            expected: AppAction::SelectTool(Tool::Fill),
            menu: None,
            button_2: Some("Relleno"),
            keys_2: vec![writes(Key::F, Modifiers::NONE, "f")],
        },
        TestCase {
            expected: AppAction::SelectTool(Tool::Dropper),
            menu: None,
            button_2: Some("Cuentagotas"),
            keys_2: vec![writes(Key::Comma, Modifiers::NONE, ",")],
        },
        TestCase {
            expected: AppAction::ZoomIn,
            menu: Some("Acercar"),
            button_2: Some("Acercar"),
            keys_2: vec![writes(Key::IntlBackslash, s, ">")],
        },
        TestCase {
            expected: AppAction::ZoomOut,
            menu: Some("Alejar"),
            button_2: Some("Alejar"),
            keys_2: vec![writes(Key::IntlBackslash, Modifiers::NONE, "<")],
        },
        TestCase {
            expected: AppAction::ColorNext,
            menu: None,
            button_2: None,
            keys_2: vec![
                writes(Key::CloseBracket, Modifiers::NONE, "]"),
                vec![key_2(Key::PageDown, Modifiers::NONE)],
            ],
        },
        TestCase {
            expected: AppAction::ColorPrev,
            menu: None,
            button_2: None,
            keys_2: vec![
                writes(Key::OpenBracket, Modifiers::ALT, "["),
                vec![key_2(Key::PageUp, Modifiers::NONE)],
            ],
        },
        TestCase {
            expected: AppAction::SelectTool(Tool::Eraser),
            menu: None,
            button_2: Some("Borrador"),
            keys_2: vec![writes(Key::K, Modifiers::NONE, "k")],
        },
    ]
}

#[test]
fn row_9_menu_button_and_key_give_the_same_action() {
    for case in cases_of_row_9() {
        let mut obtained = Vec::new();
        if let Some(label) = case.menu {
            obtained.push(("menú", by_menu_label(label)));
        }
        if let Some(label) = case.button_2 {
            obtained.push(("botón", by_button_label(label)));
        }
        for events in &case.keys_2 {
            obtained.push(("tecla", a_action(events, Platform::Linux)));
        }
        assert!(!obtained.is_empty(), "{:?}", case.expected);
        for (source_2, action) in obtained {
            assert_eq!(
                action, case.expected,
                "{:?} por el {source_2}",
                case.expected
            );
        }
    }
}

#[test]
fn row_9_the_keys_of_each_case_give_the_same_on_the_three_platforms() {
    for case in cases_of_row_9() {
        for events in &case.keys_2 {
            for platform in PLATFORMS {
                assert_eq!(
                    a_action(events, platform),
                    case.expected,
                    "{:?} en {platform:?}",
                    case.expected
                );
            }
        }
    }
}

#[test]
fn row_9_the_dispatched_and_the_final_state_are_equal_from_the_menu_the_button_and_the_key() {
    for case in cases_of_row_9() {
        let mut sources: Vec<(String, Origin, AppAction)> = Vec::new();
        if let Some(label) = case.menu {
            sources.push((format!("menú {label}"), Origin::Menu, by_menu_label(label)));
        }
        if let Some(label) = case.button_2 {
            sources.push((
                format!("botón {label}"),
                Origin::Button,
                by_button_label(label),
            ));
        }
        for (i, events) in case.keys_2.iter().enumerate() {
            sources.push((
                format!("tecla {i}"),
                Origin::Key,
                a_action(events, Platform::Linux),
            ));
        }
        let results: Vec<(String, Dispatched, TestSnapshot)> = sources
            .iter()
            .map(|(name_2, origin, action)| {
                let mut world = World::standard();
                let result = world.dispatches(*action, *origin);
                (name_2.clone(), result, world.snapshot())
            })
            .collect();
        for pair in results.windows(2) {
            assert_eq!(
                pair[0].1, pair[1].1,
                "{:?}: {} y {} difieren",
                case.expected, pair[0].0, pair[1].0
            );
            assert_eq!(
                pair[0].2, pair[1].2,
                "{:?}: el estado tras {} y tras {} difiere",
                case.expected, pair[0].0, pair[1].0
            );
        }

        // Y lo que cada una hace.
        let before = World::standard().snapshot();
        let (_, result, after_2) = &results[0];
        match case.expected {
            AppAction::Undo => {
                assert_eq!(*result, done(true));
                assert_ne!(after_2.state_id, before.state_id);
                assert!(after_2.can_redo);
            }
            AppAction::SelectTool(Tool::Eraser) => {
                // Indexada sin transparencia: con aviso.
                assert_eq!(
                    *result,
                    Dispatched::Done {
                        refresh: false,
                        notice: Some(NO_INDEX_NOTICE.to_string())
                    }
                );
                assert_eq!(after_2.tool, Tool::Eraser);
            }
            AppAction::SelectTool(tool) => {
                assert_eq!(*result, done(false));
                assert_eq!(after_2.tool, tool);
                assert_eq!(after_2.state_id, before.state_id);
            }
            AppAction::ZoomIn => assert_eq!(*result, Dispatched::View(ViewOp::ZoomIn)),
            AppAction::ZoomOut => assert_eq!(*result, Dispatched::View(ViewOp::ZoomOut)),
            AppAction::ColorNext => {
                assert_eq!(*result, done(false));
                assert_eq!(after_2.choice, ColorChoice::Entry(1));
            }
            AppAction::ColorPrev => {
                assert_eq!(*result, done(false));
                // Da la vuelta desde la primera entrada.
                assert_eq!(after_2.choice, ColorChoice::Entry(3));
            }
            other => panic!("caso sin comprobar: {other:?}"),
        }
    }
}

#[test]
fn row_9_the_eraser_notice_comes_out_the_same_from_the_button() {
    let mut a = World::standard();
    let mut b = World::standard();
    let from_button = a.dispatches(by_button_label("Borrador"), Origin::Button);
    let direct = b.dispatches(AppAction::SelectTool(Tool::Eraser), Origin::Key);
    assert_eq!(from_button, direct);
    assert_eq!(
        from_button,
        Dispatched::Done {
            refresh: false,
            notice: Some(NO_INDEX_NOTICE.to_string())
        }
    );
    assert_eq!(a.snapshot(), b.snapshot());
    // El Borrador tiene la `k` (la `j` no es nada, ni sola ni con `Mayús`).
    for letter in ["j", "J"] {
        assert_eq!(
            actions_of_keyboard(&writes(Key::J, Modifiers::NONE, letter), Platform::Linux),
            vec![],
            "{letter}"
        );
    }
}

#[test]
fn eraser_k_chooses_the_eraser_with_or_without_caps_lock_and_shift_k_clears_the_page() {
    for platform in PLATFORMS {
        // `k` (y la `K` que escribe el Bloq Mayús, sin `Mayús`): el Borrador.
        for text_2 in ["k", "K"] {
            assert_eq!(
                a_action(&writes(Key::K, Modifiers::NONE, text_2), platform),
                AppAction::SelectTool(Tool::Eraser),
                "{text_2} en {platform:?}"
            );
        }
        // `Mayús`+`k`: Borrar página, no el Borrador.
        assert_eq!(
            a_action(&writes(Key::K, Modifiers::SHIFT, "K"), platform),
            AppAction::ClearPage
        );
    }
    let mut world = World::standard();
    world.dispatches(AppAction::SelectTool(Tool::Eraser), Origin::Key);
    assert_eq!(world.tools.tool(), Tool::Eraser);
    let mut world = World::standard();
    let before = world.snapshot();
    world.dispatches(AppAction::ClearPage, Origin::Key);
    assert_eq!(world.tools.tool(), before.tool);
}

#[test]
fn eraser_k_does_not_act_with_a_focused_field_an_open_dialog_or_a_stroke_in_progress() {
    let k = writes(Key::K, Modifiers::NONE, "k");
    // Campo de texto enfocado: el filtro de teclas no deja pasar la `k`.
    assert_eq!(
        actions_of_keyboard(&px_ui::shortcuts::gate_keys(&k, true), Platform::Linux),
        vec![]
    );
    // Diálogo abierto o campo enfocado: la acción llega bloqueada y no cambia nada.
    for locks in [
        locks_with(|l| l.dialog = true),
        locks_with(|l| l.text_focus = true),
    ] {
        let mut world = World::standard();
        let before = world.snapshot();
        assert_eq!(
            world.dispatches_with(AppAction::SelectTool(Tool::Eraser), Origin::Key, locks),
            Dispatched::Blocked
        );
        assert_eq!(world.snapshot(), before);
    }
    // Trazo en curso: bloqueada también.
    let mut world = world_with_gesture(TestGesture::Stroke);
    let before = world.snapshot();
    assert_eq!(
        world.dispatches(AppAction::SelectTool(Tool::Eraser), Origin::Key),
        Dispatched::Blocked
    );
    assert_eq!(world.snapshot(), before);
    assert_ne!(world.tools.tool(), Tool::Eraser);
}

// ---------------------------------------------------------------------------------------------
// Fila 10 (parte de dispatch): SelectColor sin tecla
// ---------------------------------------------------------------------------------------------

#[test]
fn row_10_select_color_3_direct_in_an_indexed_of_9_changes_choice_with_the_grid_synchronized() {
    let mut world = World::new_2(indexed(9), 0, 0);
    let document = world.document();
    assert_eq!(
        world.dispatches(AppAction::SelectColor(3), Origin::Key),
        done(false)
    );
    assert_eq!(world.session.choice, ColorChoice::Entry(2));
    assert!(
        world.session.view.reveal,
        "la rejilla muestra la fila elegida"
    );
    assert_eq!(world.document(), document);
}

#[test]
fn row_10_select_color_3_direct_in_rgba_chooses_slot_3() {
    let mut world = World::new_2(rgba(), 0, 0);
    assert_eq!(
        world.dispatches(AppAction::SelectColor(3), Origin::Key),
        done(false)
    );
    assert_eq!(world.session.choice, ColorChoice::Slot(3));
}

#[test]
fn row_10_select_color_keeps_blocking_as_before() {
    let mut world = world_with_a_pending_removal();
    let before = world.snapshot();
    assert_eq!(
        world.dispatches(AppAction::SelectColor(1), Origin::Key),
        Dispatched::Blocked
    );
    assert_eq!(world.snapshot(), before);
}

#[test]
fn row_10_digits_on_the_keyboard_give_no_action_to_dispatch() {
    for platform in PLATFORMS {
        for (key, figure) in [
            (Key::Num1, "1"),
            (Key::Num2, "2"),
            (Key::Num3, "3"),
            (Key::Num9, "9"),
        ] {
            assert_eq!(
                actions_of_keyboard(&writes(key, Modifiers::NONE, figure), platform),
                vec![],
                "{figure} en {platform:?}"
            );
        }
    }
    // `Ctrl+1` sigue dando solo `ZoomActualSize`.
    assert_eq!(
        actions_of_keyboard(&[key_2(Key::Num1, CTRL)], Platform::Linux),
        vec![AppAction::ZoomActualSize]
    );
}

// ---------------------------------------------------------------------------------------------
// Fila 12: repetición
// ---------------------------------------------------------------------------------------------

#[test]
fn row_12_ignores_repeat_is_only_open_save_save_as_new_image_and_quit_in_the_whole_table() {
    let km = Keymap::default_scheme();
    for row in km.rows() {
        let expected_2 = matches!(
            row.action,
            AppAction::Open
                | AppAction::Save
                | AppAction::SaveAs
                | AppAction::NewImage
                | AppAction::Quit
        );
        assert_eq!(
            row.action.ignores_repeat(),
            expected_2,
            "{:?} ({:?})",
            row.action,
            row.key
        );
    }
    for action in [
        AppAction::ColorNext,
        AppAction::ColorPrev,
        AppAction::CenterOnCursor,
        AppAction::Undo,
        AppAction::Redo,
        AppAction::ZoomIn,
        AppAction::ZoomOut,
        AppAction::ZoomFit,
        AppAction::SelectTool(Tool::Pencil),
    ] {
        assert!(!action.ignores_repeat(), "{action:?}");
    }
}

#[test]
fn row_12_u_repeated_undoes_one_step_per_press() {
    let mut world = World::new_2(indexed(4), 4, 0);
    let events = [
        key_2(Key::U, Modifiers::NONE),
        text_2("u"),
        repeated(Key::U, Modifiers::NONE),
        text_2("u"),
        repeated(Key::U, Modifiers::NONE),
        text_2("u"),
    ];
    let actions = actions_of_keyboard(&events, Platform::Linux);
    assert_eq!(actions, vec![AppAction::Undo; 3]);
    let mut ids = vec![world.session.document.state_id()];
    for action in actions {
        assert_eq!(world.dispatches(action, Origin::Key), done(true));
        ids.push(world.session.document.state_id());
    }
    // Tres pasos deshechos: queda uno por deshacer y tres por rehacer.
    assert!(world.session.document.can_undo());
    assert!(world.session.document.can_redo());
    ids.dedup();
    assert_eq!(ids.len(), 4, "cada pulsación cambió el estado: {ids:?}");
    // Una cuarta pulsación agota la historia.
    world.dispatches(AppAction::Undo, Origin::Key);
    assert!(!world.session.document.can_undo());
}

#[test]
fn row_12_bracket_and_page_down_repeated_give_one_color_step_per_press() {
    let cases: Vec<(&str, Vec<Event>)> = vec![
        (
            "]",
            vec![
                key_2(Key::CloseBracket, Modifiers::NONE),
                text_2("]"),
                repeated(Key::CloseBracket, Modifiers::NONE),
                text_2("]"),
                repeated(Key::CloseBracket, Modifiers::NONE),
                text_2("]"),
                repeated(Key::CloseBracket, Modifiers::NONE),
                text_2("]"),
            ],
        ),
        (
            "Av Pág",
            vec![
                key_2(Key::PageDown, Modifiers::NONE),
                repeated(Key::PageDown, Modifiers::NONE),
                repeated(Key::PageDown, Modifiers::NONE),
                repeated(Key::PageDown, Modifiers::NONE),
            ],
        ),
    ];
    for (name_2, events) in cases {
        let mut world = World::new_2(indexed(9), 0, 0);
        let actions = actions_of_keyboard(&events, Platform::Linux);
        assert_eq!(actions, vec![AppAction::ColorNext; 4], "{name_2}");
        for action in actions {
            world.dispatches(action, Origin::Key);
        }
        assert_eq!(
            world.session.choice,
            ColorChoice::Entry(4),
            "{name_2}: cuatro pulsaciones, cuatro pasos"
        );
    }
}

#[test]
fn row_12_repeated_smaller_zooms_out_once_per_press() {
    let events = [
        key_2(Key::IntlBackslash, Modifiers::NONE),
        text_2("<"),
        repeated(Key::IntlBackslash, Modifiers::NONE),
        text_2("<"),
        repeated(Key::IntlBackslash, Modifiers::NONE),
        text_2("<"),
    ];
    let mut world = World::standard();
    let results: Vec<Dispatched> = actions_of_keyboard(&events, Platform::Linux)
        .into_iter()
        .map(|a| world.dispatches(a, Origin::Key))
        .collect();
    assert_eq!(results, vec![Dispatched::View(ViewOp::ZoomOut); 3]);
}

#[test]
fn row_12_open_save_save_as_new_image_and_quit_held_down_do_not_repeat() {
    for (key, modifiers, expected, result) in [
        (
            Key::O,
            CTRL,
            AppAction::Open,
            Dispatched::File(FileAction::Open),
        ),
        (
            Key::S,
            CTRL,
            AppAction::Save,
            Dispatched::File(FileAction::Save),
        ),
        (
            Key::S,
            CTRL | Modifiers::SHIFT,
            AppAction::SaveAs,
            Dispatched::File(FileAction::SaveAs),
        ),
        (Key::N, CTRL, AppAction::NewImage, Dispatched::NewImage),
        (Key::Q, CTRL, AppAction::Quit, Dispatched::Quit),
    ] {
        let events = [
            key_2(key, modifiers),
            repeated(key, modifiers),
            repeated(key, modifiers),
            repeated(key, modifiers),
        ];
        let actions = actions_of_keyboard(&events, Platform::Linux);
        assert_eq!(
            actions,
            vec![expected],
            "{expected:?}: solo la primera pulsación"
        );
        let mut world = World::standard();
        assert_eq!(world.dispatches(actions[0], Origin::Key), result);
    }
}
