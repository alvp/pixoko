//! Tests de `dispatch` (specs/hito-2-menu-herramientas.md, «Casos» filas 3b en la app, 4, 5, 5b y
//! 12 y propiedad 2). Sin egui más allá de `Key` y `Modifiers`: `dispatch` recibe la acción, su
//! origen y `Targets { session, tools, locks }` y no toca ninguna ventana.
//!
//! - Fila 3b: las cifras 1 a 9 sin modificadores no hacen nada (unidad I.1); las nueve entradas de
//!   una indexada de 9 se recorren con `ColorNext` (por `action_for_shortcut` + `dispatch`, lo que
//!   hace `key_actions` traducido) con la sincronización de la rejilla; `command`+`1` es solo
//!   `ZoomActualSize` y no cambia `choice`.
//! - Fila 4: la misma acción por el menú (buscada por rótulo), por el botón (por rótulo de
//!   `TOOL_BUTTONS` o `ZOOM_BUTTONS`) y por el atajo: las tres acciones, los tres `Dispatched` y el
//!   estado final son iguales sobre copias idénticas de sesión y herramienta.
//! - Fila 5: los bloqueos de lo que no es zoom, con los gestos de verdad (trazo del lápiz,
//!   deslizador del editor y arrastre de muestra) y con `dialog`, `text_focus`, `removal` y
//!   `modal`; `enabled` es falso exactamente donde `dispatch` da `Blocked` con `Origin::Menu`.
//! - Fila 5b: los bloqueos del zoom, uno por bloqueo y con los tres orígenes; el zoom a mitad de
//!   trazo no corta el trazo y deja un solo paso.
//! - Fila 12: `dispatch(Quit)` con el panel de licencias abierto, con un trazo en curso y con un
//!   diálogo abierto. (`confirm::close_allowed` está en `confirm/close_tests.rs`.)
//! - Propiedad 2: el origen no importa.
//!
//! Suposiciones sobre las firmas de «Piezas» (derives no listados): `Dispatched` es
//! `Clone, Debug, PartialEq` y `ViewOp` es `Clone, Copy, Debug, PartialEq, Eq`. Sin detalle en la
//! spec, se elige lo natural: `Undo` y `Redo` hechos dan `Done { refresh: true, notice: None }`;
//! sin historia, `Done { refresh: false, notice: None }`; `SelectColor` da
//! `Done { refresh: false, notice: None }`; `SelectTool` da `Done { refresh: false, notice }` con
//! el aviso del borrador.

use eframe::egui::{Key, Modifiers};
use proptest::prelude::*;
use px_core::ImageId;
use px_core::{Command, Document, Image, Mode, Palette, Pixel, PixelEdit, Rgba};
use px_ui::actions::{
    AppAction, Locks, Origin, Platform, UiState, action_for_shortcut, enabled, is_blocked,
};
use px_ui::canvas::CanvasEvent;
use px_ui::color::{ColorChoice, key_choice};
use px_ui::color_edit::Control;
use px_ui::menu::MENUS;
use px_ui::palette_ops::{Removal, default_destination};
use px_ui::shortcuts::FileAction;
use px_ui::tool::{Tool, ToolState};
use px_ui::toolbar::{TOOL_BUTTONS, ZOOM_BUTTONS};
use px_ui::view::{ViewOp, Zoom};

mod brush;

use super::{Dispatched, Targets, dispatch};
use crate::session::Session;

// ---------------------------------------------------------------------------------------------
// Ayudantes
// ---------------------------------------------------------------------------------------------

const TOOLS: [Tool; 5] = [
    Tool::Pencil,
    Tool::Line,
    Tool::Fill,
    Tool::Dropper,
    Tool::Eraser,
];
const SOURCES: [Origin; 3] = [Origin::Menu, Origin::Button, Origin::Key];
const ZOOMS: [(AppAction, ViewOp); 4] = [
    (AppAction::ZoomIn, ViewOp::ZoomIn),
    (AppAction::ZoomOut, ViewOp::ZoomOut),
    (AppAction::ZoomFit, ViewOp::Fit),
    (AppAction::ZoomActualSize, ViewOp::ActualSize),
];

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

/// Indexada de 4×4 con `n` entradas distintas y, si se pide, ese índice transparente.
fn indexed(n: usize, transparent: Option<u8>) -> Image {
    let mut palette = Palette::from_colors(different(n)).expect("paleta");
    palette
        .set_transparent_index(transparent)
        .expect("índice transparente");
    Image::new_indexed(4, 4, palette).expect("imagen indexada")
}

/// Un píxel distinto del de fondo, del modo de `image`, según `i`.
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

/// Un paso de historia: escribe un píxel de una fila distinta cada vez (hasta 4 pasos por
/// columna 0 y otros 4 por la columna 1).
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

/// Lo que `dispatch` no debe tocar cuando bloquea, y lo que se compara entre orígenes.
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
        // Lo que `undo` deja pendiente de avisar no es de estos tests.
        let _ = session.take_removal_cancelled();
        session.view.reveal = false;
        World {
            session,
            tools: ToolState::new(),
        }
    }

    /// Indexada de 4 entradas sin transparente, con 2 pasos y 1 deshecho: se puede deshacer y
    /// rehacer, y el borrador avisa.
    fn standard() -> World {
        World::new_2(indexed(4, None), 2, 1)
    }

    fn image_2(&self) -> &Image {
        self.session
            .document
            .image(ImageId::from_raw(0))
            .expect("imagen")
    }

    /// Los bloqueos que sacaría `main` de este estado (sin los que no son de aquí).
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

    fn ui_state(&self, zoom: Option<Zoom>, locks: Locks) -> UiState {
        UiState {
            can_undo: self.session.document.can_undo(),
            can_redo: self.session.document.can_redo(),
            zoom,
            tool: self.tools.tool(),
            locks,
        }
    }
}

fn locks_with(f: impl FnOnce(&mut Locks)) -> Locks {
    let mut locks = Locks::default();
    f(&mut locks);
    locks
}

fn done(refresh: bool) -> Dispatched {
    Dispatched::Done {
        refresh,
        notice: None,
    }
}

// --- Gestos de verdad ---

/// Empieza un trazo del lápiz sin soltar (el documento no cambia).
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

/// Deja pulsado el deslizador rojo del editor.
fn presses_the_slider(world: &mut World) {
    world.session.sync_editor();
    assert!(
        world.session.editor.begin(Control::Red, 10),
        "el editor admite el arrastre"
    );
    assert!(world.session.busy());
}

/// Arrastra de verdad una muestra de la paleta.
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
    let colors = different(5);
    let palette = Palette::from_colors(colors).expect("paleta");
    let mut image = Image::new_indexed(5, 1, palette).expect("imagen indexada");
    for (x, i) in [0u8, 1, 2, 3, 3].into_iter().enumerate() {
        image
            .set_pixel(x as u32, 0, Pixel::Indexed(i))
            .expect("píxel");
    }
    let mut document = Document::new(image);
    let id = ImageId::from_raw(0);
    // Un paso de historia (y un deshecho) que no toca los píxeles de la entrada 3.
    document
        .apply(Command::SetPixels {
            image: id,
            pixels: vec![PixelEdit {
                x: 0,
                y: 0,
                pixel: Pixel::Indexed(1),
            }],
        })
        .expect("paso");
    document
        .apply(Command::SetPixels {
            image: id,
            pixels: vec![PixelEdit {
                x: 1,
                y: 0,
                pixel: Pixel::Indexed(0),
            }],
        })
        .expect("paso");
    let mut session = Session::sample(document);
    session.undo().expect("deshacer");
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

fn all_the_actions() -> Vec<AppAction> {
    let mut v = vec![
        AppAction::Open,
        AppAction::Save,
        AppAction::SaveAs,
        AppAction::Quit,
        AppAction::Undo,
        AppAction::Redo,
        AppAction::ZoomIn,
        AppAction::ZoomOut,
        AppAction::ZoomFit,
        AppAction::ZoomActualSize,
        AppAction::ShowLicenses,
        AppAction::CloseLicenses,
    ];
    v.extend(TOOLS.map(AppAction::SelectTool));
    v.extend((1..=9).map(AppAction::SelectColor));
    v
}

fn no_es_zoom(action: &AppAction) -> bool {
    !action.is_zoom()
}

fn key_digit(slot: u8) -> Key {
    match slot {
        1 => Key::Num1,
        2 => Key::Num2,
        3 => Key::Num3,
        4 => Key::Num4,
        5 => Key::Num5,
        6 => Key::Num6,
        7 => Key::Num7,
        8 => Key::Num8,
        _ => Key::Num9,
    }
}

fn command(platform: Platform) -> Modifiers {
    if platform == Platform::Mac {
        Modifiers {
            mac_cmd: true,
            command: true,
            ..Modifiers::NONE
        }
    } else {
        Modifiers {
            ctrl: true,
            command: true,
            ..Modifiers::NONE
        }
    }
}

const PLATFORMS: [Platform; 3] = [Platform::Linux, Platform::Windows, Platform::Mac];

// ---------------------------------------------------------------------------------------------
// Fila 3b: las teclas 1 a 9 ya no eligen color; `]` y `[` recorren los colores (en la app)
// ---------------------------------------------------------------------------------------------

#[test]
fn row_3b_digits_without_modifiers_do_nothing() {
    for platform in PLATFORMS {
        for slot in 1..=9u8 {
            let world = World::new_2(indexed(9, None), 0, 0);
            let before = world.snapshot();
            // `key_actions` traducido: la tecla no da ninguna acción que despachar.
            assert_eq!(
                action_for_shortcut(key_digit(slot), Modifiers::NONE, platform),
                None,
                "{platform:?} cifra {slot} (antes elegía la ranura)"
            );
            assert_eq!(world.session.choice, ColorChoice::Entry(0));
            assert_eq!(world.snapshot(), before);
        }
    }
}

#[test]
fn row_3b_color_next_walks_the_nine_entries_of_an_indexed_one_with_the_grid_synchronized() {
    for platform in PLATFORMS {
        let mut world = World::new_2(indexed(9, None), 0, 0);
        assert_eq!(world.session.choice, ColorChoice::Entry(0));
        for input in 1..=8u8 {
            world.session.view.reveal = false;
            // `key_actions` traducido: la tecla da la acción y la acción se despacha.
            let action = action_for_shortcut(Key::CloseBracket, Modifiers::NONE, platform)
                .expect("`]` sin modificadores da una acción");
            assert_eq!(action, AppAction::ColorNext);
            let result = world.dispatches(action, Origin::Key);
            assert_eq!(result, done(false), "{platform:?} entrada {input}");
            assert_eq!(
                world.session.choice,
                ColorChoice::Entry(input),
                "{platform:?} entrada {input}"
            );
            // Con la sincronización de la rejilla: si cambió, el panel muestra su fila.
            assert!(
                world.session.view.reveal,
                "{platform:?} entrada {input}: reveal"
            );
        }
    }
}

#[test]
fn row_3b_in_rgba_the_slots_choose_the_color_slot() {
    for slot in 1..=9u8 {
        let mut world = World::new_2(rgba(), 0, 0);
        let expected_2 = key_choice(world.image_2(), slot).expect("la ranura existe");
        let result = world.dispatches(AppAction::SelectColor(slot), Origin::Key);
        assert_eq!(result, done(false), "ranura {slot}");
        assert_eq!(world.session.choice, expected_2, "ranura {slot}");
    }
}

#[test]
fn row_3b_a_slot_without_a_swatch_changes_nothing() {
    let mut world = World::new_2(indexed(4, None), 0, 0);
    let before = world.snapshot();
    let result = world.dispatches(AppAction::SelectColor(7), Origin::Key);
    assert_eq!(result, done(false));
    assert_eq!(world.snapshot(), before);
}

#[test]
fn row_3b_command_1_gives_only_zoom_actual_and_does_not_change_choice() {
    for platform in PLATFORMS {
        let mut world = World::new_2(indexed(9, None), 0, 0);
        world.session.set_choice_revealing(ColorChoice::Entry(4));
        world.session.view.reveal = false;
        let before = world.snapshot();
        let action = action_for_shortcut(Key::Num1, command(platform), platform)
            .expect("command+1 da una acción");
        assert_eq!(action, AppAction::ZoomActualSize, "{platform:?}");
        assert!(!matches!(action, AppAction::SelectColor(_)));
        let result = world.dispatches(action, Origin::Key);
        assert_eq!(result, Dispatched::View(ViewOp::ActualSize), "{platform:?}");
        assert_eq!(
            world.snapshot(),
            before,
            "{platform:?}: no cambia choice ni nada"
        );
        assert_eq!(world.session.choice, ColorChoice::Entry(4));
    }
}

#[test]
fn row_3b_num1_with_shift_alt_or_combined_gives_no_action_and_does_not_change_choice() {
    for platform in PLATFORMS {
        let cmd = command(platform);
        for m in [
            Modifiers::SHIFT,
            Modifiers::ALT,
            Modifiers { shift: true, ..cmd },
            Modifiers { alt: true, ..cmd },
        ] {
            assert_eq!(
                action_for_shortcut(Key::Num1, m, platform),
                None,
                "{platform:?} {m:?}"
            );
        }
    }
}

// ---------------------------------------------------------------------------------------------
// Fila 4: la misma acción, el mismo efecto
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
    shortcut: Option<(Key, Modifiers)>,
}

fn cases_of_row_4() -> Vec<TestCase> {
    let cmd = command(Platform::Linux);
    let cmd_shift = Modifiers { shift: true, ..cmd };
    vec![
        TestCase {
            expected: AppAction::Undo,
            menu: Some("Deshacer"),
            button_2: None,
            shortcut: Some((Key::Z, cmd)),
        },
        TestCase {
            expected: AppAction::Redo,
            menu: Some("Rehacer"),
            button_2: None,
            shortcut: Some((Key::Y, cmd)),
        },
        TestCase {
            expected: AppAction::SelectTool(Tool::Eraser),
            menu: None,
            button_2: Some("Borrador"),
            shortcut: Some((Key::K, Modifiers::NONE)),
        },
        TestCase {
            expected: AppAction::ColorNext,
            menu: None,
            button_2: None,
            shortcut: Some((Key::CloseBracket, Modifiers::NONE)),
        },
        TestCase {
            expected: AppAction::Open,
            menu: Some("Abrir…"),
            button_2: None,
            shortcut: Some((Key::O, cmd)),
        },
        TestCase {
            expected: AppAction::Save,
            menu: Some("Guardar"),
            button_2: None,
            shortcut: Some((Key::S, cmd)),
        },
        TestCase {
            expected: AppAction::SaveAs,
            menu: Some("Guardar como…"),
            button_2: None,
            shortcut: Some((Key::S, cmd_shift)),
        },
        TestCase {
            expected: AppAction::Quit,
            menu: Some("Salir"),
            button_2: None,
            // Solo en Linux.
            shortcut: Some((Key::Q, cmd)),
        },
        TestCase {
            expected: AppAction::ZoomIn,
            menu: Some("Acercar"),
            button_2: Some("Acercar"),
            shortcut: None,
        },
        TestCase {
            expected: AppAction::ZoomFit,
            menu: Some("Ajustar a la ventana"),
            button_2: Some("Ajustar"),
            shortcut: Some((Key::Home, Modifiers::NONE)),
        },
        TestCase {
            expected: AppAction::ShowLicenses,
            menu: Some("Licencias"),
            button_2: None,
            shortcut: None,
        },
    ]
}

#[test]
fn row_4_menu_button_and_shortcut_give_the_same_action() {
    for case in cases_of_row_4() {
        let mut obtained = Vec::new();
        if let Some(label) = case.menu {
            obtained.push(("menú", by_menu_label(label)));
        }
        if let Some(label) = case.button_2 {
            obtained.push(("botón", by_button_label(label)));
        }
        if let Some((key, modifiers)) = case.shortcut {
            let action = action_for_shortcut(key, modifiers, Platform::Linux)
                .unwrap_or_else(|| panic!("{key:?} con {modifiers:?} no da acción"));
            obtained.push(("atajo", action));
        }
        assert!(!obtained.is_empty());
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
fn row_4_the_three_dispatched_and_the_final_state_are_equal() {
    for case in cases_of_row_4() {
        // Cada origen con su acción, sobre una copia idéntica.
        let mut sources: Vec<(Origin, AppAction)> = Vec::new();
        if let Some(label) = case.menu {
            sources.push((Origin::Menu, by_menu_label(label)));
        }
        if let Some(label) = case.button_2 {
            sources.push((Origin::Button, by_button_label(label)));
        }
        if let Some((key, modifiers)) = case.shortcut {
            let action = action_for_shortcut(key, modifiers, Platform::Linux).expect("atajo");
            sources.push((Origin::Key, action));
        }

        let before = World::standard().snapshot();
        let results: Vec<(Origin, Dispatched, TestSnapshot)> = sources
            .iter()
            .map(|(origin, action)| {
                let mut world = World::standard();
                let result = world.dispatches(*action, *origin);
                (*origin, result, world.snapshot())
            })
            .collect();
        for pair in results.windows(2) {
            assert_eq!(
                pair[0].1, pair[1].1,
                "{:?}: {:?} y {:?} difieren",
                case.expected, pair[0].0, pair[1].0
            );
            assert_eq!(
                pair[0].2, pair[1].2,
                "{:?}: el estado tras {:?} y tras {:?} difiere",
                case.expected, pair[0].0, pair[1].0
            );
        }

        // Y lo que cada una hace.
        let (_, result, after_2) = &results[0];
        match case.expected {
            AppAction::Open => {
                assert_eq!(*result, Dispatched::File(FileAction::Open));
                assert_eq!(*after_2, before, "Abrir no toca el documento");
            }
            AppAction::Save => {
                assert_eq!(*result, Dispatched::File(FileAction::Save));
                assert_eq!(*after_2, before);
            }
            AppAction::SaveAs => {
                assert_eq!(*result, Dispatched::File(FileAction::SaveAs));
                assert_eq!(*after_2, before);
            }
            AppAction::Quit => assert_eq!(*result, Dispatched::Quit),
            AppAction::ZoomIn => assert_eq!(*result, Dispatched::View(ViewOp::ZoomIn)),
            AppAction::ZoomFit => assert_eq!(*result, Dispatched::View(ViewOp::Fit)),
            AppAction::ShowLicenses => assert_eq!(*result, Dispatched::Licenses(true)),
            AppAction::Undo => {
                assert_eq!(*result, done(true));
                assert_ne!(after_2.state_id, before.state_id);
                assert!(after_2.can_redo);
            }
            AppAction::Redo => {
                assert_eq!(*result, done(true));
                assert_ne!(after_2.state_id, before.state_id);
                assert!(!after_2.can_redo);
            }
            AppAction::SelectTool(Tool::Eraser) => {
                // Indexada sin transparencia: con aviso, y es el mismo texto desde el botón y
                // desde `E`.
                assert_eq!(
                    *result,
                    Dispatched::Done {
                        refresh: false,
                        notice: Some(NO_INDEX_NOTICE.to_string())
                    }
                );
                assert_eq!(after_2.tool, Tool::Eraser);
            }
            AppAction::ColorNext => {
                assert_eq!(*result, done(false));
                assert_eq!(after_2.choice, ColorChoice::Entry(1));
            }
            other => panic!("caso sin comprobar: {other:?}"),
        }
    }
}

#[test]
fn row_4_the_eraser_notice_is_the_same_with_any_source() {
    // La acción del borrador da lo mismo desde el botón que con cualquier otro origen.
    let mut per_button = World::standard();
    let mut per_other_source = World::standard();
    let from_button = per_button.dispatches(by_button_label("Borrador"), Origin::Button);
    let from_key = per_other_source.dispatches(AppAction::SelectTool(Tool::Eraser), Origin::Key);
    assert_eq!(from_button, from_key);
    assert!(matches!(
        from_button,
        Dispatched::Done {
            notice: Some(_),
            ..
        }
    ));
    // Con una imagen que admite borrar no hay aviso.
    for image in [rgba(), gray_alpha(), indexed(4, Some(3))] {
        let mut world = World::new_2(image, 0, 0);
        assert_eq!(
            world.dispatches(AppAction::SelectTool(Tool::Eraser), Origin::Button),
            done(false)
        );
        assert_eq!(world.tools.tool(), Tool::Eraser);
    }
    // Y con gris, avisa de otra cosa pero avisa.
    let mut world = World::new_2(gray(), 0, 0);
    assert!(matches!(
        world.dispatches(AppAction::SelectTool(Tool::Eraser), Origin::Button),
        Dispatched::Done {
            notice: Some(_),
            ..
        }
    ));
}

#[test]
fn row_4_the_five_tools_are_chosen_from_the_button_and_also_from_their_key() {
    for (tool, (key, button_2)) in TOOLS.iter().zip([
        (Some(Key::D), "Lápiz"),
        (Some(Key::V), "Línea"),
        (Some(Key::F), "Relleno"),
        (Some(Key::Comma), "Cuentagotas"),
        (Some(Key::K), "Borrador"),
    ]) {
        let mut a = World::new_2(rgba(), 0, 0);
        let ra = a.dispatches(by_button_label(button_2), Origin::Button);
        assert_eq!(a.tools.tool(), *tool);
        if let Some(key) = key {
            let mut b = World::new_2(rgba(), 0, 0);
            let rb = b.dispatches(
                action_for_shortcut(key, Modifiers::NONE, Platform::Linux).expect("tecla"),
                Origin::Key,
            );
            assert_eq!(ra, rb, "{tool:?}");
            assert_eq!(b.tools.tool(), *tool);
        }
    }
}

#[test]
fn row_4_the_four_zoom_buttons_give_view_with_their_operation() {
    for (label, op) in [
        ("Acercar", ViewOp::ZoomIn),
        ("Alejar", ViewOp::ZoomOut),
        ("Ajustar", ViewOp::Fit),
        ("1:1", ViewOp::ActualSize),
    ] {
        let mut world = World::standard();
        let before = world.snapshot();
        assert_eq!(
            world.dispatches(by_button_label(label), Origin::Button),
            Dispatched::View(op),
            "{label}"
        );
        assert_eq!(world.snapshot(), before, "el zoom no toca el documento");
    }
}

#[test]
fn row_4_close_licenses_gives_licenses_false_with_any_lock_and_source() {
    for bits in 0..32u8 {
        let locks = Locks {
            gesture: bits & 1 != 0,
            stroke: false,
            dialog: bits & 2 != 0,
            text_focus: bits & 4 != 0,
            removal: bits & 8 != 0,
            modal: bits & 16 != 0,
        };
        for origin in SOURCES {
            let mut world = World::standard();
            assert_eq!(
                world.dispatches_with(AppAction::CloseLicenses, origin, locks),
                Dispatched::Licenses(false),
                "{locks:?} {origin:?}"
            );
        }
    }
}

// ---------------------------------------------------------------------------------------------
// Fila 5: bloqueos de lo que no es zoom
// ---------------------------------------------------------------------------------------------

#[test]
fn row_5_undo_and_redo_without_history_give_done_without_changes() {
    let mut world = World::new_2(rgba(), 0, 0);
    let before = world.snapshot();
    assert_eq!(world.dispatches(AppAction::Undo, Origin::Menu), done(false));
    assert_eq!(world.dispatches(AppAction::Redo, Origin::Menu), done(false));
    assert_eq!(world.snapshot(), before);
}

#[test]
fn row_5_undo_and_redo_with_history_change_the_document_and_ask_for_a_refresh() {
    let mut world = World::new_2(rgba(), 2, 0);
    let initial = world.snapshot();
    assert_eq!(world.dispatches(AppAction::Undo, Origin::Menu), done(true));
    let after_undo = world.snapshot();
    assert_ne!(after_undo.state_id, initial.state_id);
    assert!(after_undo.can_undo && after_undo.can_redo);
    assert_eq!(world.dispatches(AppAction::Redo, Origin::Key), done(true));
    let after_redo = world.snapshot();
    assert_eq!(
        after_redo.image, initial.image,
        "rehacer deja la imagen como estaba"
    );
    assert!(after_redo.can_undo && !after_redo.can_redo);
}

#[test]
fn row_5_with_a_gesture_dialog_or_text_all_those_that_are_not_zoom_give_blocked_except_color_with_a_stroke()
 {
    for action in all_the_actions()
        .into_iter()
        .filter(no_es_zoom)
        .filter(|a| *a != AppAction::CloseLicenses)
    {
        for origin in SOURCES {
            // Gesto de verdad: trazo, deslizador y arrastre de muestra.
            for gesture in GESTURES {
                let mut world = world_with_gesture(gesture);
                let before = world.snapshot();
                // I.2a: con un trazo en curso el color se puede cambiar (afecta al trazo
                // siguiente); con el deslizador o una muestra arrastrándose, no.
                if matches!(action, AppAction::SelectColor(_))
                    && matches!(gesture, TestGesture::Stroke)
                {
                    assert_ne!(
                        world.dispatches(action, origin),
                        Dispatched::Blocked,
                        "{action:?} {origin:?} con {gesture:?}"
                    );
                    continue;
                }
                assert_eq!(
                    world.dispatches(action, origin),
                    Dispatched::Blocked,
                    "{action:?} {origin:?} con {gesture:?}"
                );
                assert_eq!(
                    world.snapshot(),
                    before,
                    "{action:?} {origin:?} con {gesture:?}"
                );
            }
            // Diálogo y campo de texto.
            for (name_2, locks) in [
                ("dialog", locks_with(|l| l.dialog = true)),
                ("text_focus", locks_with(|l| l.text_focus = true)),
            ] {
                let mut world = World::standard();
                let before = world.snapshot();
                assert_eq!(
                    world.dispatches_with(action, origin, locks),
                    Dispatched::Blocked,
                    "{action:?} {origin:?} con {name_2}"
                );
                assert_eq!(
                    world.snapshot(),
                    before,
                    "{action:?} {origin:?} con {name_2}"
                );
            }
        }
    }
}

#[test]
fn row_5_a_blocked_undo_does_not_consume_the_history() {
    let mut world = World::new_2(rgba(), 3, 0);
    let before = world.snapshot();
    for locks in [
        locks_with(|l| l.gesture = true),
        locks_with(|l| l.dialog = true),
        locks_with(|l| l.text_focus = true),
        locks_with(|l| l.modal = true),
    ] {
        assert_eq!(
            world.dispatches_with(AppAction::Undo, Origin::Menu, locks),
            Dispatched::Blocked
        );
        assert_eq!(
            world.dispatches_with(AppAction::Redo, Origin::Key, locks),
            Dispatched::Blocked
        );
        assert_eq!(world.snapshot(), before, "{locks:?}");
    }
    // Sin bloqueo, el mismo Undo sí deshace.
    assert_eq!(world.dispatches(AppAction::Undo, Origin::Menu), done(true));
    assert_ne!(world.snapshot().state_id, before.state_id);
}

#[test]
fn row_5_with_a_pending_removal_only_select_color_and_show_licenses_give_blocked() {
    for action in all_the_actions() {
        for origin in SOURCES {
            let mut world = world_with_a_pending_removal();
            let result = world.dispatches(action, origin);
            // `ShowLicenses` abre un diálogo: una quita pendiente también lo frena (unidad 18, D2).
            let locked_2 = matches!(action, AppAction::SelectColor(_) | AppAction::ShowLicenses);
            assert_eq!(
                result == Dispatched::Blocked,
                locked_2,
                "{action:?} {origin:?}: {result:?}"
            );
        }
    }
}

#[test]
fn row_5_select_color_with_a_pending_removal_changes_neither_the_selection_nor_the_removal() {
    let mut world = world_with_a_pending_removal();
    let before = world.snapshot();
    for slot in 1..=5 {
        assert_eq!(
            world.dispatches(AppAction::SelectColor(slot), Origin::Key),
            Dispatched::Blocked
        );
    }
    assert_eq!(world.snapshot(), before);
    assert!(world.session.ops.removal().is_some());
}

#[test]
fn row_5_undo_cancels_the_pending_removal_with_the_11d_notice() {
    let mut world = world_with_a_pending_removal();
    assert!(world.session.document.can_undo());
    assert_eq!(world.dispatches(AppAction::Undo, Origin::Menu), done(true));
    assert!(
        world.session.ops.removal().is_none(),
        "deshacer cancela la quita"
    );
    assert!(
        world.session.take_removal_cancelled(),
        "y deja pendiente el aviso de quita cancelada"
    );
}

#[test]
fn row_5_redo_cancels_the_pending_removal_with_the_11d_notice() {
    let mut world = world_with_a_pending_removal();
    assert!(world.session.document.can_redo());
    assert_eq!(world.dispatches(AppAction::Redo, Origin::Key), done(true));
    assert!(world.session.ops.removal().is_none());
    assert!(world.session.take_removal_cancelled());
}

#[test]
fn row_5_with_a_modal_only_close_licenses_and_quit_pass() {
    let locks = locks_with(|l| l.modal = true);
    for action in all_the_actions() {
        for origin in SOURCES {
            let mut world = World::standard();
            let before = world.snapshot();
            let result = world.dispatches_with(action, origin, locks);
            match action {
                AppAction::CloseLicenses => {
                    assert_eq!(result, Dispatched::Licenses(false), "{origin:?}");
                }
                AppAction::Quit => assert_eq!(result, Dispatched::Quit, "{origin:?}"),
                other => {
                    assert_eq!(result, Dispatched::Blocked, "{other:?} {origin:?}");
                    assert_eq!(world.snapshot(), before, "{other:?} {origin:?}");
                }
            }
        }
    }
}

#[test]
fn row_5_dispatch_gives_blocked_exactly_when_is_blocked() {
    for action in all_the_actions() {
        for bits in 0..32u8 {
            let locks = Locks {
                gesture: bits & 1 != 0,
                stroke: false,
                dialog: bits & 2 != 0,
                text_focus: bits & 4 != 0,
                removal: bits & 8 != 0,
                modal: bits & 16 != 0,
            };
            for origin in SOURCES {
                let mut world = World::standard();
                let result = world.dispatches_with(action, origin, locks);
                assert_eq!(
                    result == Dispatched::Blocked,
                    is_blocked(action, locks, origin),
                    "{action:?} {origin:?} {locks:?}: {result:?}"
                );
            }
        }
    }
}

#[test]
fn row_5_enabled_is_false_exactly_where_dispatch_gives_blocked_with_menu_source() {
    // Las acciones cuyo `enabled` solo depende del bloqueo (no de la historia ni del zoom).
    let actions = all_the_actions()
        .into_iter()
        .filter(|a| !matches!(a, AppAction::Undo | AppAction::Redo) && !a.is_zoom());
    for action in actions {
        for bits in 0..32u8 {
            let locks = Locks {
                gesture: bits & 1 != 0,
                stroke: false,
                dialog: bits & 2 != 0,
                text_focus: bits & 4 != 0,
                removal: bits & 8 != 0,
                modal: bits & 16 != 0,
            };
            let mut world = World::standard();
            let state_2 = world.ui_state(Zoom::from_int(5), locks);
            let result = world.dispatches_with(action, Origin::Menu, locks);
            assert_eq!(
                enabled(action, &state_2),
                result != Dispatched::Blocked,
                "{action:?} con {locks:?}: {result:?}"
            );
        }
    }
}

#[test]
fn row_5_open_save_and_save_as_give_file_and_do_not_touch_the_document() {
    for (action, file) in [
        (AppAction::Open, FileAction::Open),
        (AppAction::Save, FileAction::Save),
        (AppAction::SaveAs, FileAction::SaveAs),
    ] {
        for origin in SOURCES {
            let mut world = World::standard();
            let before = world.snapshot();
            assert_eq!(world.dispatches(action, origin), Dispatched::File(file));
            assert_eq!(world.snapshot(), before);
        }
    }
}

#[test]
fn row_5_show_licenses_gives_licenses_true() {
    for origin in SOURCES {
        let mut world = World::standard();
        let before = world.snapshot();
        assert_eq!(
            world.dispatches(AppAction::ShowLicenses, origin),
            Dispatched::Licenses(true)
        );
        assert_eq!(world.snapshot(), before);
    }
}

#[test]
fn row_5_select_tool_blocked_by_a_gesture_does_not_change_the_tool() {
    for gesture in GESTURES {
        let mut world = world_with_gesture(gesture);
        for tool in TOOLS {
            assert_eq!(
                world.dispatches(AppAction::SelectTool(tool), Origin::Button),
                Dispatched::Blocked,
                "{gesture:?} {tool:?}"
            );
        }
        assert_eq!(world.tools.tool(), Tool::Pencil, "{gesture:?}");
    }
}

// ---------------------------------------------------------------------------------------------
// Fila 5b: bloqueos del zoom
// ---------------------------------------------------------------------------------------------

#[test]
fn row_5b_a_b_and_c_stroke_slider_and_removal_do_not_block_the_zoom_with_any_source() {
    type CreatesWorld = fn() -> World;
    let worlds: Vec<(&str, CreatesWorld)> = vec![
        ("(a) trazo del lápiz", || {
            world_with_gesture(TestGesture::Stroke)
        }),
        ("(b) deslizador del editor", || {
            world_with_gesture(TestGesture::Slider)
        }),
        ("(b') arrastre de muestra", || {
            world_with_gesture(TestGesture::Swatch)
        }),
        ("(c) quita pendiente", world_with_a_pending_removal),
    ];
    for (name_2, creates) in worlds {
        for (action, op) in ZOOMS {
            for origin in SOURCES {
                let mut world = creates();
                let before = world.snapshot();
                assert_eq!(
                    world.dispatches(action, origin),
                    Dispatched::View(op),
                    "{name_2}: {action:?} {origin:?}"
                );
                assert_eq!(world.snapshot(), before, "{name_2}: el zoom no toca nada");
                // `enabled` es verdadero con el zoom y la imagen que hacen falta.
                let state_2 = world.ui_state(Zoom::from_int(5), world.locks());
                assert!(enabled(action, &state_2), "{name_2}: {action:?}");
            }
        }
    }
}

#[test]
fn row_5b_d_with_a_focused_field_the_keys_are_blocked_and_the_menu_and_the_buttons_pass() {
    let locks = locks_with(|l| l.text_focus = true);
    for (action, op) in ZOOMS {
        let mut world = World::standard();
        let before = world.snapshot();
        assert_eq!(
            world.dispatches_with(action, Origin::Key, locks),
            Dispatched::Blocked,
            "{action:?} por tecla"
        );
        assert_eq!(world.snapshot(), before);
        for origin in [Origin::Menu, Origin::Button] {
            assert_eq!(
                world.dispatches_with(action, origin, locks),
                Dispatched::View(op),
                "{action:?} {origin:?}"
            );
        }
        let state_2 = world.ui_state(Zoom::from_int(5), locks);
        assert!(enabled(action, &state_2), "{action:?}");
    }
}

#[test]
fn row_5b_e_and_f_a_dialog_or_the_licenses_block_the_zoom_with_the_three_sources() {
    for (name_2, locks) in [
        ("(e) diálogo", locks_with(|l| l.dialog = true)),
        ("(f) licencias", locks_with(|l| l.modal = true)),
    ] {
        for (action, _) in ZOOMS {
            for origin in SOURCES {
                let mut world = World::standard();
                let before = world.snapshot();
                assert_eq!(
                    world.dispatches_with(action, origin, locks),
                    Dispatched::Blocked,
                    "{name_2}: {action:?} {origin:?}"
                );
                assert_eq!(world.snapshot(), before);
            }
            let world = World::standard();
            let state_2 = world.ui_state(Zoom::from_int(5), locks);
            assert!(!enabled(action, &state_2), "{name_2}: {action:?}");
        }
    }
}

#[test]
fn row_5b_zoom_in_the_middle_of_a_stroke_does_not_cut_the_stroke_and_leaves_a_single_step() {
    for origin in SOURCES {
        for (action, op) in ZOOMS {
            let mut world = World::new_2(rgba(), 0, 0);
            let initial = world.snapshot();
            let id = ImageId::from_raw(0);
            let pixel = world.session.active_pixel();
            starts_a_stroke(&mut world);
            let move_pointer = |world: &mut World, x: i32| {
                world
                    .tools
                    .canvas_event_with(
                        &mut world.session.document,
                        id,
                        pixel,
                        CanvasEvent::StrokeMove((x, 0)),
                    )
                    .expect("mover");
            };
            move_pointer(&mut world, 1);
            let half_way = world.snapshot();

            assert_eq!(
                world.dispatches(action, origin),
                Dispatched::View(op),
                "{action:?} {origin:?}"
            );
            assert!(world.tools.busy(), "el trazo sigue tras {action:?}");
            assert_eq!(world.snapshot(), half_way);
            assert_eq!(
                world.snapshot().state_id,
                initial.state_id,
                "intacto hasta soltar"
            );

            move_pointer(&mut world, 2);
            move_pointer(&mut world, 3);
            assert_eq!(world.snapshot().state_id, initial.state_id);
            world
                .tools
                .canvas_event_with(
                    &mut world.session.document,
                    id,
                    pixel,
                    CanvasEvent::StrokeEnd,
                )
                .expect("soltar");
            assert!(!world.tools.busy());
            assert!(world.session.document.can_undo());
            world.session.undo().expect("un paso");
            assert!(
                !world.session.document.can_undo(),
                "{action:?} {origin:?}: el trazo entero es un solo paso"
            );
            assert_eq!(world.snapshot().image, initial.image);
        }
    }
}

// ---------------------------------------------------------------------------------------------
// Fila 12: Salir
// ---------------------------------------------------------------------------------------------

#[test]
fn row_12_quit_gives_dispatched_quit_with_any_source() {
    for origin in SOURCES {
        let mut world = World::standard();
        let before = world.snapshot();
        assert_eq!(world.dispatches(AppAction::Quit, origin), Dispatched::Quit);
        assert_eq!(
            world.snapshot(),
            before,
            "Salir no toca el documento (cierra `App`)"
        );
    }
}

#[test]
fn row_12_quit_with_a_stroke_in_progress_or_a_gesture_gives_blocked() {
    for gesture in GESTURES {
        for origin in SOURCES {
            let mut world = world_with_gesture(gesture);
            assert_eq!(
                world.dispatches(AppAction::Quit, origin),
                Dispatched::Blocked,
                "{gesture:?} {origin:?}"
            );
        }
    }
}

#[test]
fn row_12_quit_with_an_open_dialog_gives_blocked() {
    let locks = locks_with(|l| l.dialog = true);
    for origin in SOURCES {
        let mut world = World::standard();
        assert_eq!(
            world.dispatches_with(AppAction::Quit, origin, locks),
            Dispatched::Blocked,
            "{origin:?}"
        );
    }
}

#[test]
fn row_12_quit_with_the_licenses_open_passes() {
    let locks = locks_with(|l| l.modal = true);
    for origin in SOURCES {
        let mut world = World::standard();
        assert_eq!(
            world.dispatches_with(AppAction::Quit, origin, locks),
            Dispatched::Quit,
            "{origin:?}"
        );
    }
}

#[test]
fn row_12_quit_with_a_pending_removal_passes_and_with_a_focused_field_it_does_not() {
    let mut world = world_with_a_pending_removal();
    assert_eq!(
        world.dispatches(AppAction::Quit, Origin::Menu),
        Dispatched::Quit
    );
    let mut world = World::standard();
    assert_eq!(
        world.dispatches_with(
            AppAction::Quit,
            Origin::Key,
            locks_with(|l| l.text_focus = true)
        ),
        Dispatched::Blocked
    );
}

// ---------------------------------------------------------------------------------------------
// Propiedad 2: el origen no importa
// ---------------------------------------------------------------------------------------------

fn action_any() -> impl Strategy<Value = AppAction> {
    prop_oneof![
        Just(AppAction::Open),
        Just(AppAction::Save),
        Just(AppAction::SaveAs),
        Just(AppAction::Quit),
        Just(AppAction::Undo),
        Just(AppAction::Redo),
        Just(AppAction::ZoomIn),
        Just(AppAction::ZoomOut),
        Just(AppAction::ZoomFit),
        Just(AppAction::ZoomActualSize),
        Just(AppAction::ShowLicenses),
        Just(AppAction::CloseLicenses),
        (0usize..5).prop_map(|i| AppAction::SelectTool(TOOLS[i])),
        (1u8..=9).prop_map(AppAction::SelectColor),
    ]
}

fn source_any() -> impl Strategy<Value = Origin> {
    (0usize..3).prop_map(|i| SOURCES[i])
}

fn locks_any() -> impl Strategy<Value = Locks> {
    prop::array::uniform5(prop::bool::weighted(0.2)).prop_map(|b| Locks {
        gesture: b[0],
        stroke: false,
        dialog: b[1],
        text_focus: b[2],
        removal: b[3],
        modal: b[4],
    })
}

fn image_of_mode(mode_2: usize) -> Image {
    match mode_2 % 4 {
        0 => rgba(),
        1 => indexed(5, None),
        2 => gray(),
        _ => gray_alpha(),
    }
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(256))]

    #[test]
    fn property_2_the_source_does_not_matter(
        mode_2 in 0usize..4,
        steps_2 in 0usize..=4,
        undone_raw in 0usize..=4,
        sequence in prop::collection::vec(
            (action_any(), source_any(), locks_any()),
            0..=24,
        ),
    ) {
        let undone = undone_raw % (steps_2 + 1);
        let mut real = World::new_2(image_of_mode(mode_2), steps_2, undone);
        let mut menu = World::new_2(image_of_mode(mode_2), steps_2, undone);
        // Modelo de la historia: un `Document` con las mismas operaciones.
        let mut model = real.session.document.clone();

        for (action, origin, locks) in sequence {
            let before = real.snapshot();
            let per_source = real.dispatches_with(action, origin, locks);
            let per_menu = menu.dispatches_with(action, Origin::Menu, locks);

            // `Blocked` es exactamente `is_blocked`.
            prop_assert_eq!(
                per_source == Dispatched::Blocked,
                is_blocked(action, locks, origin),
                "{:?} {:?} {:?}", action, origin, locks
            );
            prop_assert_eq!(
                per_menu == Dispatched::Blocked,
                is_blocked(action, locks, Origin::Menu),
                "{:?} (menú) {:?}", action, locks
            );

            // La única diferencia permitida entre orígenes: una acción de zoom con `text_focus` se
            // bloquea con `Key` y no con los otros dos.
            if per_source != per_menu {
                prop_assert!(
                    action.is_zoom()
                        && locks.text_focus
                        && origin == Origin::Key
                        && per_source == Dispatched::Blocked
                        && matches!(per_menu, Dispatched::View(_)),
                    "{:?} {:?} {:?}: {:?} y {:?}", action, origin, locks, per_source, per_menu
                );
            }

            // Un `Blocked` no cambia el estado; el zoom nunca cambia el documento ni la historia.
            if per_source == Dispatched::Blocked || action.is_zoom() {
                prop_assert_eq!(real.snapshot(), before.clone(), "{:?} {:?}", action, locks);
            }

            // El estado final es idéntico al de aplicar las mismas acciones con origen `Menu`.
            prop_assert_eq!(real.snapshot(), menu.snapshot(), "{:?} {:?} {:?}", action, origin, locks);

            // `Undo` y `Redo` dejan la historia como `Document`.
            if per_source != Dispatched::Blocked {
                match action {
                    AppAction::Undo if model.can_undo() => model.undo().expect("deshacer"),
                    AppAction::Redo if model.can_redo() => model.redo().expect("rehacer"),
                    _ => {}
                }
            }
            prop_assert_eq!(real.session.document.state_id(), model.state_id());
            prop_assert_eq!(real.session.document.can_undo(), model.can_undo());
            prop_assert_eq!(real.session.document.can_redo(), model.can_redo());
            prop_assert_eq!(real.image_2(), model.image(ImageId::from_raw(0)).expect("imagen"));
        }
    }
}

// ---------------------------------------------------------------------------------------------
// Unidad 17b, fila 13 (specs/hito-2-damero-lienzo.md): el despacho del fondo del lienzo
// ---------------------------------------------------------------------------------------------
//
// `AppAction::CanvasBackground(b)` da `Dispatched::Background(b)` y no toca la sesión: ni
// `state_id`, ni la historia, ni el indicador de modificado, ni la imagen, ni la elección, ni la
// herramienta. No es zoom ni repite; solo la bloquean `dialog`, `modal` y `text_focus` (con
// cualquier origen), no `gesture` ni `removal`. La parte pura de los bloqueos (`is_blocked` y
// `enabled` con los 32 bloqueos) está en `px-ui/tests/background_menu.rs`.
//
// Suposición sobre las firmas: `Dispatched::Background` lleva el `CanvasBackground` elegido (los
// derives de `Dispatched`, `Clone, Debug, PartialEq`, ya los tiene).

use px_ui::background::CanvasBackground;

const BACKGROUNDS: [CanvasBackground; 4] = [
    CanvasBackground::Checker,
    CanvasBackground::Black,
    CanvasBackground::Gray,
    CanvasBackground::White,
];

fn action_of_background(b: CanvasBackground) -> AppAction {
    AppAction::CanvasBackground(b)
}

/// Las sesiones de la fila: con historia y una edición deshecha, con una quita pendiente, con
/// historia en una RGBA y sin historia.
fn world_of_the_case(case: usize) -> (&'static str, World) {
    match case {
        0 => (
            "indexada con historia y una edición deshecha",
            World::standard(),
        ),
        1 => ("con una quita pendiente", world_with_a_pending_removal()),
        2 => (
            "RGBA con historia y un paso deshecho",
            World::new_2(rgba(), 3, 1),
        ),
        _ => ("sin historia", World::new_2(rgba(), 0, 0)),
    }
}

const CASES_OF_ROW_13: usize = 4;

/// Lo que el despacho del fondo no debe tocar, además de la `TestSnapshot` de siempre: los bytes de la
/// historia, el indicador de modificado y el gesto en curso.
fn extras(world: &World) -> (usize, bool, bool) {
    (
        world.session.document.history_bytes(),
        world.session.modified(),
        world.session.busy(),
    )
}

#[test]
fn row_13_the_four_actions_with_each_source_give_background_and_touch_nothing() {
    for case in 0..CASES_OF_ROW_13 {
        for b in BACKGROUNDS {
            for origin in SOURCES {
                let (name_2, mut world) = world_of_the_case(case);
                let before = world.snapshot();
                let extras_before = extras(&world);
                assert_eq!(
                    world.dispatches(action_of_background(b), origin),
                    Dispatched::Background(b),
                    "{name_2}: {b:?} con {origin:?}"
                );
                // La foto de la fila 3b de la 17: `state_id`, `can_undo`, `can_redo`, imagen,
                // elección, herramienta, quita pendiente y gesto, iguales.
                assert_eq!(world.snapshot(), before, "{name_2}: {b:?} con {origin:?}");
                assert_eq!(
                    extras(&world),
                    extras_before,
                    "{name_2}: {b:?} con {origin:?}"
                );
                assert_eq!(world.session.document.state_id(), before.state_id);
                assert_eq!(world.session.document.can_undo(), before.can_undo);
                assert_eq!(world.session.document.can_redo(), before.can_redo);
                assert_eq!(world.session.choice, before.choice);
                assert_eq!(world.tools.tool(), before.tool);
            }
        }
    }
}

#[test]
fn row_13_the_action_does_not_mark_the_document_as_modified_or_create_a_step() {
    let (_, mut world) = world_of_the_case(0);
    assert!(world.session.document.can_undo(), "hay historia");
    assert!(world.session.document.can_redo(), "y una edición deshecha");
    let modified_2 = world.session.modified();
    let bytes = world.session.document.history_bytes();
    for b in BACKGROUNDS {
        world.dispatches(action_of_background(b), Origin::Menu);
    }
    assert_eq!(world.session.modified(), modified_2);
    assert_eq!(world.session.document.history_bytes(), bytes);
    assert!(
        world.session.document.can_redo(),
        "la edición deshecha se puede rehacer: no se ha creado ningún paso"
    );
    // Una sesión recién abierta sigue sin cambios.
    let (_, mut new) = world_of_the_case(3);
    assert!(!new.session.modified());
    new.dispatches(action_of_background(CanvasBackground::Black), Origin::Menu);
    assert!(!new.session.modified());
    assert!(!new.session.document.can_undo());
}

#[test]
fn row_13_it_is_not_zoom_or_repeat_and_the_dispatch_does_not_give_view() {
    for b in BACKGROUNDS {
        let action = action_of_background(b);
        assert!(!action.is_zoom(), "{b:?}");
        assert!(!action.ignores_repeat(), "{b:?}");
        let (_, mut world) = world_of_the_case(0);
        let result = world.dispatches(action, Origin::Key);
        assert!(!matches!(result, Dispatched::View(_)), "{b:?}");
        assert_ne!(result, Dispatched::Blocked);
    }
}

#[test]
fn row_13_each_lock_alone_and_the_dispatch_without_locks() {
    // Bloqueada solo con `dialog`, con `modal` y con `text_focus`, con cualquier origen; no con
    // `gesture` ni con `removal`.
    let cases: [(&str, Locks, bool); 6] = [
        ("ninguno", Locks::default(), false),
        ("gesture", locks_with(|l| l.gesture = true), false),
        ("removal", locks_with(|l| l.removal = true), false),
        ("dialog", locks_with(|l| l.dialog = true), true),
        ("modal", locks_with(|l| l.modal = true), true),
        ("text_focus", locks_with(|l| l.text_focus = true), true),
    ];
    for case in 0..CASES_OF_ROW_13 {
        for b in BACKGROUNDS {
            for (name_2, locks, locked_2) in cases {
                for origin in SOURCES {
                    let (session_2, mut world) = world_of_the_case(case);
                    let before = world.snapshot();
                    let result = world.dispatches_with(action_of_background(b), origin, locks);
                    let expected_2 = if locked_2 {
                        Dispatched::Blocked
                    } else {
                        Dispatched::Background(b)
                    };
                    assert_eq!(
                        result, expected_2,
                        "{session_2}: {b:?} con {name_2} y {origin:?}"
                    );
                    assert_eq!(world.snapshot(), before, "{session_2}: {b:?} con {name_2}");
                }
            }
        }
    }
}

#[test]
fn row_13_a_real_gesture_or_a_pending_removal_do_not_block_the_background() {
    // Con los bloqueos que saca `main` de un trazo, un deslizador, un arrastre de muestra o una
    // quita pendiente (`World::locks`), el fondo se despacha con los tres orígenes.
    for gesture in GESTURES {
        for b in BACKGROUNDS {
            for origin in SOURCES {
                let mut world = world_with_gesture(gesture);
                assert!(world.locks().gesture);
                let before = world.snapshot();
                assert_eq!(
                    world.dispatches(action_of_background(b), origin),
                    Dispatched::Background(b),
                    "{gesture:?} {b:?} {origin:?}"
                );
                assert_eq!(
                    world.snapshot(),
                    before,
                    "el gesto sigue en curso: {gesture:?}"
                );
                assert!(world.locks().gesture, "{gesture:?}");
            }
        }
    }
    for b in BACKGROUNDS {
        for origin in SOURCES {
            let mut world = world_with_a_pending_removal();
            assert!(world.locks().removal);
            let before = world.snapshot();
            assert_eq!(
                world.dispatches(action_of_background(b), origin),
                Dispatched::Background(b)
            );
            assert_eq!(world.snapshot(), before, "la quita sigue pendiente");
            assert!(world.session.ops.removal().is_some());
        }
    }
}

#[test]
fn row_13_with_combined_locks_only_dialog_modal_and_text_focus_count() {
    for case in 0..CASES_OF_ROW_13 {
        for bits in 0..32u8 {
            let locks = Locks {
                gesture: bits & 1 != 0,
                stroke: false,
                dialog: bits & 2 != 0,
                text_focus: bits & 4 != 0,
                removal: bits & 8 != 0,
                modal: bits & 16 != 0,
            };
            for b in BACKGROUNDS {
                for origin in SOURCES {
                    let (session_2, mut world) = world_of_the_case(case);
                    let before = world.snapshot();
                    let locked_2 = locks.dialog || locks.modal || locks.text_focus;
                    let result = world.dispatches_with(action_of_background(b), origin, locks);
                    assert_eq!(
                        result == Dispatched::Blocked,
                        locked_2,
                        "{session_2}: {b:?} con {locks:?} y {origin:?}"
                    );
                    if !locked_2 {
                        assert_eq!(result, Dispatched::Background(b));
                    }
                    assert_eq!(world.snapshot(), before);
                }
            }
        }
    }
}

#[test]
fn row_13_enabled_is_the_opposite_of_blocked_with_the_menu_source() {
    for case in 0..CASES_OF_ROW_13 {
        for bits in 0..32u8 {
            let locks = Locks {
                gesture: bits & 1 != 0,
                stroke: false,
                dialog: bits & 2 != 0,
                text_focus: bits & 4 != 0,
                removal: bits & 8 != 0,
                modal: bits & 16 != 0,
            };
            for b in BACKGROUNDS {
                for zoom in [None, Zoom::from_int(5)] {
                    let (session_2, mut world) = world_of_the_case(case);
                    let state_2 = world.ui_state(zoom, locks);
                    let enabled_2 = enabled(action_of_background(b), &state_2);
                    let result =
                        world.dispatches_with(action_of_background(b), Origin::Menu, locks);
                    assert_eq!(
                        enabled_2,
                        result != Dispatched::Blocked,
                        "{session_2}: {b:?} con {locks:?} y zoom {zoom:?}"
                    );
                }
            }
        }
    }
}

#[test]
fn row_13_the_previous_actions_dispatch_the_same_as_before_with_the_background_in_between() {
    // Intercalar acciones de fondo entre las demás no cambia lo que hacen: el resultado y el
    // estado final son los mismos que sin ellas.
    let sequence = [
        AppAction::SelectColor(2),
        AppAction::Undo,
        AppAction::ZoomIn,
        AppAction::Redo,
        AppAction::SelectTool(Tool::Line),
    ];
    let mut without = World::standard();
    let mut with = World::standard();
    for (i, action) in sequence.into_iter().enumerate() {
        with.dispatches(action_of_background(BACKGROUNDS[i % 4]), Origin::Menu);
        assert_eq!(
            with.dispatches(action, Origin::Key),
            without.dispatches(action, Origin::Key),
            "{action:?}"
        );
        assert_eq!(with.snapshot(), without.snapshot(), "{action:?}");
    }
}

// ---------------------------------------------------------------------------------------------
// Unidad 18, fila 6 (c), (d) y (f): bloqueo por diálogo modal a nivel `dispatch`
// (specs/hito-2-dialogos-modales.md). La parte pura (`can_open_dialog`, `Modal::open`,
// `is_blocked` y `enabled` con los 32 bloqueos) está en `px-ui/tests/dialog_lock.rs`, y
// `ignored_file_notice` (6e), en `dialogs/tests.rs`.
//
// - (c) Con `modal`, `dispatch` de **todas** las `AppAction` (herramientas, ranuras 1 a 9, los
//   cuatro fondos, zoom, fichero, deshacer, rehacer y licencias) con los tres orígenes y cuatro
//   sesiones distintas: `Dispatched::Blocked` salvo `CloseLicenses` (`Licenses(false)`) y `Quit`
//   (`Quit`, decisión D1), con `state_id`, historia, herramienta y elección intactos.
// - (d) Lo mismo con todo lo que traduce `action_for_shortcut` sobre `Key::ALL` × 32
//   combinaciones de modificadores × tres plataformas: solo `Ctrl+Q` en Linux da `Quit`.
// - (f) Cerrar la ventana con un diálogo abierto: `Modal::abandon` (sin `Reaction`) y después
//   `close_allowed(modified, &mut Confirm, now)`, con y sin cambios y con dos peticiones dentro y
//   fuera de `CONFIRM_SECS`. La regla de cierre no cambia por haber un diálogo.
//
// Decisiones donde la spec no fija un detalle:
// - Los bloqueos de (c) y (d) son los de la sesión (`World::locks`) más `modal: true`; con la
//   sesión de la quita pendiente, `removal` también está activo (Quit sigue pasando: la regla
//   (2) lo deja y `removal` solo frena a `SelectColor` y a `ShowLicenses`).
// - Un `Modal` y un `Confirm` no se comunican: abandonar el diálogo no arma ni desarma la
//   confirmación (la primera petición con cambios da `false` aunque haya un diálogo abierto).
// - Suposiciones sobre las firmas de «Piezas»: `Modal: Default` y `Reaction: Clone + Debug +
//   PartialEq` (derives no listados).

use px_ui::dialog::{DialogKind as ModalKind, Event as DialogEvent, Modal, Reaction};
use px_ui::licenses_view::licenses_form;

use crate::confirm::{CONFIRM_SECS, Confirm, Confirmation, Request, close_allowed};

/// Las 30 acciones: las de `all_the_actions` más los cuatro fondos del lienzo.
fn all_the_actions_with_backgrounds() -> Vec<AppAction> {
    let mut v = all_the_actions();
    v.extend(BACKGROUNDS.map(action_of_background));
    v
}

/// Las 32 combinaciones de los cinco modificadores de egui.
fn modifiers_of_bits(bits: u8) -> Modifiers {
    Modifiers {
        alt: bits & 1 != 0,
        ctrl: bits & 2 != 0,
        shift: bits & 4 != 0,
        mac_cmd: bits & 8 != 0,
        command: bits & 16 != 0,
    }
}

/// Los bloqueos de la sesión con el diálogo modal abierto.
fn with_modal(world: &World) -> Locks {
    Locks {
        modal: true,
        ..world.locks()
    }
}

fn passes_with_modal(action: AppAction) -> bool {
    matches!(action, AppAction::CloseLicenses | AppAction::Quit)
}

fn opens_licenses(modal: &mut Modal) -> bool {
    modal.open(
        ModalKind::Licenses,
        licenses_form("texto"),
        Locks::default(),
    )
}

// --- (c) todas las acciones con modal ---

#[test]
fn row_6c_with_a_modal_dispatch_gives_blocked_in_all_but_close_licenses_and_quit() {
    let actions = all_the_actions_with_backgrounds();
    assert_eq!(actions.len(), 30);
    for case in 0..CASES_OF_ROW_13 {
        for action in &actions {
            for origin in SOURCES {
                let (session_2, mut world) = world_of_the_case(case);
                let locks = with_modal(&world);
                let before = world.snapshot();
                let extras_before = extras(&world);
                let result = world.dispatches_with(*action, origin, locks);
                let expected_2 = match action {
                    AppAction::CloseLicenses => Dispatched::Licenses(false),
                    AppAction::Quit => Dispatched::Quit,
                    _ => Dispatched::Blocked,
                };
                assert_eq!(result, expected_2, "{session_2}: {action:?} con {origin:?}");
                assert_eq!(
                    world.snapshot(),
                    before,
                    "{session_2}: {action:?} con {origin:?} tocó el estado"
                );
                assert_eq!(extras(&world), extras_before, "{session_2}: {action:?}");
            }
        }
    }
}

#[test]
fn row_6c_with_a_modal_state_id_history_tool_and_choice_do_not_change() {
    for case in 0..CASES_OF_ROW_13 {
        let (session_2, mut world) = world_of_the_case(case);
        let locks = with_modal(&world);
        let state_id = world.session.document.state_id();
        let can_undo = world.session.document.can_undo();
        let can_redo = world.session.document.can_redo();
        let bytes = world.session.document.history_bytes();
        let modified_2 = world.session.modified();
        let choice = world.session.choice;
        let tool_2 = world.tools.tool();
        let removes = world.session.ops.removal();
        for action in all_the_actions_with_backgrounds() {
            for origin in SOURCES {
                world.dispatches_with(action, origin, locks);
            }
        }
        assert_eq!(world.session.document.state_id(), state_id, "{session_2}");
        assert_eq!(world.session.document.can_undo(), can_undo, "{session_2}");
        assert_eq!(world.session.document.can_redo(), can_redo, "{session_2}");
        assert_eq!(world.session.document.history_bytes(), bytes, "{session_2}");
        assert_eq!(world.session.modified(), modified_2, "{session_2}");
        assert_eq!(world.session.choice, choice, "{session_2}");
        assert_eq!(world.tools.tool(), tool_2, "{session_2}");
        assert_eq!(world.session.ops.removal(), removes, "{session_2}");
    }
}

#[test]
fn row_6c_with_a_modal_the_nine_slots_and_the_five_tools_change_nothing() {
    for origin in SOURCES {
        let mut world = World::new_2(indexed(9, None), 2, 1);
        let locks = with_modal(&world);
        let before = world.snapshot();
        for slot in 1..=9u8 {
            assert_eq!(
                world.dispatches_with(AppAction::SelectColor(slot), origin, locks),
                Dispatched::Blocked,
                "ranura {slot} con {origin:?}"
            );
        }
        for tool in TOOLS {
            assert_eq!(
                world.dispatches_with(AppAction::SelectTool(tool), origin, locks),
                Dispatched::Blocked,
                "{tool:?} con {origin:?}"
            );
        }
        assert_eq!(world.snapshot(), before, "{origin:?}");
    }
}

#[test]
fn row_6c_with_a_modal_undo_and_redo_do_not_consume_the_history_or_cancel_the_removal() {
    for origin in SOURCES {
        let mut world = World::standard();
        let locks = with_modal(&world);
        let before = world.snapshot();
        assert!(before.can_undo && before.can_redo);
        assert_eq!(
            world.dispatches_with(AppAction::Undo, origin, locks),
            Dispatched::Blocked
        );
        assert_eq!(
            world.dispatches_with(AppAction::Redo, origin, locks),
            Dispatched::Blocked
        );
        assert_eq!(world.snapshot(), before, "{origin:?}");

        // Con una quita pendiente, deshacer bloqueado no la cancela (a diferencia de la fila 5).
        let mut world = world_with_a_pending_removal();
        let locks = with_modal(&world);
        assert!(locks.removal && locks.modal);
        assert_eq!(
            world.dispatches_with(AppAction::Undo, origin, locks),
            Dispatched::Blocked
        );
        assert!(world.session.ops.removal().is_some(), "{origin:?}");
        assert!(!world.session.take_removal_cancelled(), "{origin:?}");
    }
}

#[test]
fn row_6c_with_a_modal_enabled_is_false_except_close_licenses_and_quit() {
    for case in 0..CASES_OF_ROW_13 {
        for zoom in [None, Zoom::from_int(5)] {
            let (session_2, world) = world_of_the_case(case);
            let locks = with_modal(&world);
            let state_2 = world.ui_state(zoom, locks);
            for action in all_the_actions_with_backgrounds() {
                assert_eq!(
                    enabled(action, &state_2),
                    passes_with_modal(action),
                    "{session_2}: {action:?} con zoom {zoom:?}"
                );
            }
        }
    }
}

#[test]
fn row_6c_with_a_modal_dispatch_gives_blocked_exactly_when_is_blocked_in_the_thirty_actions() {
    for action in all_the_actions_with_backgrounds() {
        for origin in SOURCES {
            let mut world = World::standard();
            let locks = with_modal(&world);
            let result = world.dispatches_with(action, origin, locks);
            assert_eq!(
                result == Dispatched::Blocked,
                is_blocked(action, locks, origin),
                "{action:?} {origin:?}: {result:?}"
            );
        }
    }
}

#[test]
fn row_6c_a_dialog_open_with_show_licenses_closes_with_close_licenses_and_does_not_allow_opening_another()
 {
    // El recorrido de `main`: `Licenses(true)` abre; con el diálogo abierto `ShowLicenses` ya no
    // pasa; `Licenses(false)` cierra solo el diálogo de Licencias.
    let mut world = World::standard();
    let mut modal = Modal::default();
    let locks = Locks {
        modal: modal.is_open(),
        ..world.locks()
    };
    assert_eq!(
        world.dispatches_with(AppAction::ShowLicenses, Origin::Menu, locks),
        Dispatched::Licenses(true)
    );
    assert!(modal.open(ModalKind::Licenses, licenses_form("texto"), locks));

    let locks = Locks {
        modal: modal.is_open(),
        ..world.locks()
    };
    assert!(locks.modal);
    for origin in SOURCES {
        assert_eq!(
            world.dispatches_with(AppAction::ShowLicenses, origin, locks),
            Dispatched::Blocked,
            "{origin:?}"
        );
    }
    assert!(!modal.open(ModalKind::Licenses, licenses_form("otro"), locks));

    assert_eq!(
        world.dispatches_with(AppAction::CloseLicenses, Origin::Button, locks),
        Dispatched::Licenses(false)
    );
    assert!(modal.close_kind(ModalKind::Licenses));
    assert!(!modal.is_open());
    assert!(
        !modal.close_kind(ModalKind::Licenses),
        "ya no hay nada que cerrar"
    );
}

// --- (d) todas las teclas con modal ---

#[test]
fn row_6d_no_key_changes_the_state_with_a_modal_and_only_ctrl_q_on_linux_gives_quit() {
    let mut world = World::standard();
    let locks = with_modal(&world);
    let before = world.snapshot();
    let extras_before = extras(&world);
    let mut translated_2 = 0usize;
    let mut quits = 0usize;
    for &key in Key::ALL {
        for bits in 0..32u8 {
            let modifiers = modifiers_of_bits(bits);
            for platform in PLATFORMS {
                let Some(action) = action_for_shortcut(key, modifiers, platform) else {
                    continue;
                };
                translated_2 += 1;
                let context = format!("{key:?} con {modifiers:?} en {platform:?}: {action:?}");
                let result = world.dispatches_with(action, Origin::Key, locks);
                match result {
                    Dispatched::Quit => {
                        quits += 1;
                        assert_eq!(action, AppAction::Quit, "{context}");
                        assert_eq!(key, Key::Q, "{context}");
                        assert_eq!(platform, Platform::Linux, "{context}");
                        assert!(modifiers.command, "{context}");
                        assert!(!modifiers.alt && !modifiers.shift, "{context}");
                    }
                    Dispatched::Blocked => {
                        assert_ne!(action, AppAction::Quit, "{context}");
                    }
                    other_2 => panic!("{context}: con modal no puede dar {other_2:?}"),
                }
                assert_eq!(world.snapshot(), before, "{context}");
                assert_eq!(extras(&world), extras_before, "{context}");
            }
        }
    }
    assert!(
        translated_2 > 100,
        "se tradujeron {translated_2} combinaciones"
    );
    assert!(quits > 0, "Ctrl+Q en Linux da Quit");
}

#[test]
fn row_6d_ctrl_q_on_linux_gives_quit_with_a_modal_and_on_windows_and_mac_there_is_no_shortcut() {
    let mut world = World::standard();
    let locks = with_modal(&world);
    let action = action_for_shortcut(Key::Q, command(Platform::Linux), Platform::Linux)
        .expect("Ctrl+Q en Linux es un atajo");
    assert_eq!(action, AppAction::Quit);
    assert_eq!(
        world.dispatches_with(action, Origin::Key, locks),
        Dispatched::Quit
    );
    for platform in [Platform::Windows, Platform::Mac] {
        assert_eq!(
            action_for_shortcut(Key::Q, command(platform), platform),
            None,
            "{platform:?}"
        );
    }
}

#[test]
fn row_6d_the_tool_color_zoom_and_file_keys_give_blocked_with_a_modal() {
    let cases = [
        (Key::D, Modifiers::NONE),
        (Key::V, Modifiers::NONE),
        (Key::F, Modifiers::NONE),
        (Key::Comma, Modifiers::NONE),
        (Key::CloseBracket, Modifiers::NONE),
        (Key::OpenBracket, Modifiers::NONE),
        (Key::Plus, Modifiers::NONE),
        (Key::Minus, Modifiers::NONE),
        (Key::Home, Modifiers::NONE),
        (Key::Z, command(Platform::Linux)),
        (Key::O, command(Platform::Linux)),
        (Key::S, command(Platform::Linux)),
        (Key::Num1, command(Platform::Linux)),
    ];
    for (key, modifiers) in cases {
        let mut world = World::standard();
        let locks = with_modal(&world);
        let before = world.snapshot();
        let action = action_for_shortcut(key, modifiers, Platform::Linux)
            .unwrap_or_else(|| panic!("{key:?} con {modifiers:?} debería traducirse"));
        assert_eq!(
            world.dispatches_with(action, Origin::Key, locks),
            Dispatched::Blocked,
            "{key:?} con {modifiers:?}: {action:?}"
        );
        assert_eq!(world.snapshot(), before, "{key:?} con {modifiers:?}");
    }
}

// --- (f) cerrar la ventana con un diálogo abierto ---

#[test]
fn row_6f_abandoning_the_dialog_gives_no_reaction_and_leaves_everything_closed() {
    let mut modal = Modal::default();
    assert!(opens_licenses(&mut modal));
    assert!(modal.abandon(), "había un diálogo que abandonar");
    assert!(!modal.is_open());
    assert_eq!(modal.kind(), None);
    assert!(!modal.abandon(), "la segunda vez no hay nada");
    // Tras abandonar, ningún evento hace nada: ni `Accepted` ni `Cancelled`.
    for event_2 in [DialogEvent::Accept, DialogEvent::Cancel] {
        assert_eq!(modal.event(event_2), Reaction::None);
    }
}

#[test]
fn row_6f_without_changes_the_dialog_is_abandoned_and_the_window_closes_the_first_time() {
    let mut modal = Modal::default();
    let mut confirm = Confirm::new();
    assert!(opens_licenses(&mut modal));
    assert!(modal.abandon());
    assert!(close_allowed(false, &mut confirm, 0.0));
    assert!(!modal.is_open());
}

#[test]
fn row_6f_with_changes_the_dialog_is_abandoned_and_two_requests_are_needed() {
    let mut modal = Modal::default();
    let mut confirm = Confirm::new();
    assert!(opens_licenses(&mut modal));

    // Primera petición: el diálogo se abandona siempre y la ventana no cierra.
    assert!(modal.abandon());
    assert!(!close_allowed(true, &mut confirm, 10.0));
    assert!(!modal.is_open());

    // Segunda, dentro del plazo: cierra (ya no hay diálogo que abandonar).
    assert!(!modal.abandon());
    assert!(close_allowed(true, &mut confirm, 10.0 + CONFIRM_SECS - 0.1));
}

#[test]
fn row_6f_with_changes_and_a_dialog_open_at_each_request_it_is_abandoned_each_time() {
    let mut modal = Modal::default();
    let mut confirm = Confirm::new();

    assert!(opens_licenses(&mut modal));
    assert!(modal.abandon());
    assert!(!close_allowed(true, &mut confirm, 0.0));

    // El usuario vuelve a abrir el diálogo y pide cerrar otra vez dentro del plazo.
    assert!(opens_licenses(&mut modal));
    assert!(modal.abandon(), "se abandona también la segunda vez");
    assert!(close_allowed(true, &mut confirm, 2.0));
    assert!(!modal.is_open());
}

#[test]
fn row_6f_outside_the_deadline_the_second_request_arms_again_and_the_third_closes() {
    let mut modal = Modal::default();
    let mut confirm = Confirm::new();

    assert!(opens_licenses(&mut modal));
    assert!(modal.abandon());
    assert!(!close_allowed(true, &mut confirm, 0.0));

    assert!(opens_licenses(&mut modal));
    assert!(modal.abandon());
    assert!(
        !close_allowed(true, &mut confirm, CONFIRM_SECS),
        "a los CONFIRM_SECS justos la primera ya caducó"
    );
    assert!(!modal.is_open());

    assert!(close_allowed(true, &mut confirm, CONFIRM_SECS + 1.0));
}

#[test]
fn row_6f_abandoning_neither_arms_nor_disarms_the_confirmation() {
    let mut modal = Modal::default();
    let mut confirm = Confirm::new();
    // Abandonar sin pedir cerrar no deja nada armado: la primera petición con cambios no cierra.
    assert!(opens_licenses(&mut modal));
    assert!(modal.abandon());
    assert!(!close_allowed(true, &mut confirm, 1.0));
    // Y la armada de cerrar sigue siendo la de `Request::Close`.
    assert_eq!(
        confirm.request(Request::Close, 2.0),
        Confirmation::Confirmed,
        "la primera petición dejó armado `Request::Close`"
    );
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(256))]

    /// Haya o no un diálogo abierto al pedir cerrar, `close_allowed` sigue la regla de siempre
    /// (`Confirm::request(Request::Close, now)` con cambios y `true` sin ellos), el diálogo
    /// siempre acaba cerrado y `abandon` solo da `true` si estaba abierto.
    #[test]
    fn row_6f_abandoning_the_dialog_does_not_change_the_close_rule(
        steps_2 in prop::collection::vec((any::<bool>(), any::<bool>(), 0.0f64..9.0), 1..12),
    ) {
        let mut real = Confirm::new();
        let mut model = Confirm::new();
        let mut modal = Modal::default();
        let mut now = 0.0;
        for (modified, opens, step) in steps_2 {
            now += step;
            if opens {
                let _ = opens_licenses(&mut modal);
            }
            let was_open = modal.is_open();
            prop_assert_eq!(modal.abandon(), was_open);
            prop_assert!(!modal.is_open());
            let expected_2 =
                !modified || model.request(Request::Close, now) == Confirmation::Confirmed;
            prop_assert_eq!(
                close_allowed(modified, &mut real, now),
                expected_2,
                "modified {}, now {}", modified, now
            );
        }
    }
}

// ---------------------------------------------------------------------------------------------
// Unidad 19: «Nueva imagen» (specs/hito-2-nueva-imagen.md, fila 9, parte de `dispatch`)
// ---------------------------------------------------------------------------------------------
//
// - `Dispatched::NewImage` desde el menú, el botón y el atajo (`Ctrl+N`, `Cmd+N` en macOS): la
//   misma acción, el mismo resultado y nada tocado (lo ejecuta `App`, como `Licenses`).
// - Los cinco bloqueos (`gesture`, `dialog`, `text_focus`, `removal`, `modal`) la bloquean con
//   cualquier origen y `enabled` es falso exactamente donde `dispatch` da `Blocked`; un gesto de
//   verdad y una quita pendiente también.
// - Con el diálogo de nueva imagen abierto todo está bloqueado salvo `Quit`, que lo abandona sin
//   crear nada y sigue la regla de cierre de siempre.
//
// Suposición: `Dispatched::NewImage` es una variante sin datos (`Clone, Debug, PartialEq` ya los
// tiene `Dispatched`).

use crate::new_image::open_new_image;

/// Las sesiones sin quita pendiente de la fila 13 (la quita bloquea esta acción y se prueba aparte).
fn worlds_without_removes() -> Vec<(&'static str, World)> {
    vec![
        (
            "indexada con historia y una edición deshecha",
            World::standard(),
        ),
        (
            "RGBA con historia y un paso deshecho",
            World::new_2(rgba(), 3, 1),
        ),
        ("sin historia", World::new_2(rgba(), 0, 0)),
    ]
}

fn command_of(platform: Platform) -> Modifiers {
    if platform == Platform::Mac {
        Modifiers {
            mac_cmd: true,
            command: true,
            ..Modifiers::NONE
        }
    } else {
        Modifiers {
            ctrl: true,
            command: true,
            ..Modifiers::NONE
        }
    }
}

const PLATFORMS_OF_19: [Platform; 3] = [Platform::Linux, Platform::Windows, Platform::Mac];

/// Cada uno de los cinco bloqueos, solo.
fn locks_one_by_one() -> [(&'static str, Locks); 5] {
    [
        ("gesture", locks_with(|l| l.gesture = true)),
        ("dialog", locks_with(|l| l.dialog = true)),
        ("text_focus", locks_with(|l| l.text_focus = true)),
        ("removal", locks_with(|l| l.removal = true)),
        ("modal", locks_with(|l| l.modal = true)),
    ]
}

#[test]
fn row_9_new_image_gives_dispatched_new_image_with_each_source_and_touches_nothing() {
    for (name_2, mut world) in worlds_without_removes() {
        for origin in SOURCES {
            let before = (world.snapshot(), extras(&world));
            let result = world.dispatches(AppAction::NewImage, origin);
            assert_eq!(result, Dispatched::NewImage, "{name_2}, {origin:?}");
            assert!(
                !matches!(result, Dispatched::View(_)),
                "{name_2}: no es zoom"
            );
            assert_eq!(
                (world.snapshot(), extras(&world)),
                before,
                "{name_2}, {origin:?}: despachar no toca nada (lo hace App)"
            );
        }
    }
}

#[test]
fn row_9_menu_button_and_shortcut_give_the_same_action_and_the_same_dispatched() {
    // El menú: la entrada «Nueva imagen…» de la tabla.
    let input = MENUS
        .iter()
        .flat_map(|m| m.items.iter())
        .find(|item| item.label == "Nueva imagen…")
        .expect("el menú tiene «Nueva imagen…»");
    assert_eq!(input.action, AppAction::NewImage);

    for platform in PLATFORMS_OF_19 {
        // El atajo: `command`+`N`.
        let shortcut = action_for_shortcut(Key::N, command_of(platform), platform);
        assert_eq!(shortcut, Some(AppAction::NewImage), "{platform:?}");

        let mut per_menu = World::standard();
        let mut per_shortcut = World::standard();
        let mut per_button = World::standard();
        let a = per_menu.dispatches(input.action, Origin::Menu);
        let b = per_shortcut.dispatches(shortcut.expect("hay atajo"), Origin::Key);
        let c = per_button.dispatches(AppAction::NewImage, Origin::Button);
        assert_eq!(a, Dispatched::NewImage, "{platform:?}");
        assert_eq!(a, b, "{platform:?}: menú y atajo");
        assert_eq!(a, c, "{platform:?}: menú y botón");
        assert_eq!(per_menu.snapshot(), per_shortcut.snapshot());
        assert_eq!(per_menu.snapshot(), per_button.snapshot());
    }
}

#[test]
fn row_9_the_n_key_only_gives_new_image_with_command_without_shift_or_alt() {
    for platform in PLATFORMS_OF_19 {
        for bits in 0..32u8 {
            let m = modifiers_of_bits(bits);
            let expected_2 = m.command && !m.shift && !m.alt;
            let action = action_for_shortcut(Key::N, m, platform);
            assert_eq!(
                action == Some(AppAction::NewImage),
                expected_2,
                "{platform:?}, modificadores {bits:05b}: {action:?}"
            );
            // Y con la tecla que la lleva a `dispatch` pasa lo mismo.
            if let Some(action) = action {
                let mut world = World::standard();
                let r = world.dispatches(action, Origin::Key);
                assert_eq!(r == Dispatched::NewImage, expected_2, "{bits:05b}");
            }
        }
    }
}

#[test]
fn row_9_each_lock_only_blocks_new_image_with_any_source_and_touches_nothing() {
    for (name_2, locks) in locks_one_by_one() {
        for origin in SOURCES {
            let mut world = World::standard();
            let before = (world.snapshot(), extras(&world));
            assert_eq!(
                world.dispatches_with(AppAction::NewImage, origin, locks),
                Dispatched::Blocked,
                "{name_2}, {origin:?}"
            );
            assert!(is_blocked(AppAction::NewImage, locks, origin), "{name_2}");
            assert_eq!(
                (world.snapshot(), extras(&world)),
                before,
                "{name_2}, {origin:?}: un NewImage bloqueado no toca nada"
            );
        }
    }
}

#[test]
fn row_9_with_the_32_lock_combinations_only_the_one_with_none_passes() {
    for bits in 0..32u8 {
        let locks = Locks {
            gesture: bits & 1 != 0,
            stroke: false,
            dialog: bits & 2 != 0,
            text_focus: bits & 4 != 0,
            removal: bits & 8 != 0,
            modal: bits & 16 != 0,
        };
        for origin in SOURCES {
            let mut world = World::standard();
            let expected_2 = if bits == 0 {
                Dispatched::NewImage
            } else {
                Dispatched::Blocked
            };
            assert_eq!(
                world.dispatches_with(AppAction::NewImage, origin, locks),
                expected_2,
                "{bits:05b}, {origin:?}"
            );
        }
    }
}

#[test]
fn row_9_enabled_is_false_exactly_where_dispatch_gives_blocked_with_menu_source() {
    for zoom in [None, Zoom::from_int(5)] {
        for bits in 0..32u8 {
            let locks = Locks {
                gesture: bits & 1 != 0,
                stroke: false,
                dialog: bits & 2 != 0,
                text_focus: bits & 4 != 0,
                removal: bits & 8 != 0,
                modal: bits & 16 != 0,
            };
            let mut world = World::standard();
            let state_2 = world.ui_state(zoom, locks);
            let locked_2 = world.dispatches_with(AppAction::NewImage, Origin::Menu, locks)
                == Dispatched::Blocked;
            assert_eq!(
                enabled(AppAction::NewImage, &state_2),
                !locked_2,
                "{bits:05b}, zoom {zoom:?}"
            );
        }
    }
}

#[test]
fn row_9_a_real_gesture_or_a_pending_removal_block_new_image() {
    // Un trazo en curso.
    let mut world = World::standard();
    starts_a_stroke(&mut world);
    let before = world.snapshot();
    assert_eq!(
        world.dispatches(AppAction::NewImage, Origin::Menu),
        Dispatched::Blocked,
        "trazo"
    );
    assert_eq!(world.snapshot(), before);
    assert!(!enabled(
        AppAction::NewImage,
        &world.ui_state(Zoom::from_int(5), world.locks())
    ));

    // El deslizador del editor.
    let mut world = World::standard();
    presses_the_slider(&mut world);
    assert_eq!(
        world.dispatches(AppAction::NewImage, Origin::Key),
        Dispatched::Blocked,
        "deslizador"
    );

    // El arrastre de una muestra.
    let mut world = World::standard();
    drags_a_swatch(&mut world);
    assert_eq!(
        world.dispatches(AppAction::NewImage, Origin::Button),
        Dispatched::Blocked,
        "arrastre de muestra"
    );

    // Una quita de paleta pendiente: abrir un diálogo también se bloquea (regla literal de la 18).
    let mut world = world_with_a_pending_removal();
    assert!(world.locks().removal);
    let before = (world.snapshot(), extras(&world));
    for origin in SOURCES {
        assert_eq!(
            world.dispatches(AppAction::NewImage, origin),
            Dispatched::Blocked,
            "quita pendiente, {origin:?}"
        );
    }
    assert_eq!((world.snapshot(), extras(&world)), before);
    assert!(!enabled(
        AppAction::NewImage,
        &world.ui_state(Zoom::from_int(5), world.locks())
    ));
}

#[test]
fn row_9_with_the_new_image_dialog_open_everything_is_locked_except_quit() {
    let mut world = World::standard();
    let mut modal = Modal::default();
    assert!(open_new_image(&mut modal, 16384, world.locks()));
    assert_eq!(modal.kind(), Some(ModalKind::NewImage));
    let locks = with_modal(&world);
    assert!(locks.modal);

    let before = (world.snapshot(), extras(&world));
    for action in all_the_actions().into_iter().chain([AppAction::NewImage]) {
        for origin in SOURCES {
            let expected_2 = match action {
                AppAction::Quit => Dispatched::Quit,
                // `CloseLicenses` no es de este diálogo, pero la regla (1) la deja pasar siempre.
                AppAction::CloseLicenses => Dispatched::Licenses(false),
                _ => Dispatched::Blocked,
            };
            assert_eq!(
                world.dispatches_with(action, origin, locks),
                expected_2,
                "{action:?}, {origin:?}"
            );
        }
    }
    assert_eq!(
        (world.snapshot(), extras(&world)),
        before,
        "con el diálogo abierto el documento no cambia"
    );
}

#[test]
fn row_9_an_open_dialog_does_not_allow_opening_another_or_another_new_image() {
    let world = World::standard();
    let mut modal = Modal::default();
    assert!(open_new_image(&mut modal, 16384, world.locks()));
    assert!(
        !open_new_image(&mut modal, 16384, world.locks()),
        "ya hay uno abierto"
    );
    assert!(!opens_licenses(&mut modal));
    assert_eq!(modal.kind(), Some(ModalKind::NewImage));
}

#[test]
fn row_9_each_lock_prevents_opening_the_new_image_dialog() {
    for (name_2, locks) in locks_one_by_one() {
        let mut modal = Modal::default();
        assert!(!open_new_image(&mut modal, 16384, locks), "{name_2}");
        assert!(!modal.is_open(), "{name_2}");
    }
    let mut modal = Modal::default();
    assert!(open_new_image(&mut modal, 16384, Locks::default()));
}

#[test]
fn row_9_quit_with_the_dialog_open_abandons_it_and_follows_the_close_rule() {
    for modified in [false, true] {
        let mut world = World::standard();
        let mut modal = Modal::default();
        let mut confirm = Confirm::new();
        assert!(open_new_image(&mut modal, 16384, world.locks()));
        let locks = with_modal(&world);

        // `Quit` pasa con el diálogo abierto, con cualquier origen.
        for origin in SOURCES {
            assert_eq!(
                world.dispatches_with(AppAction::Quit, origin, locks),
                Dispatched::Quit,
                "{origin:?}"
            );
        }
        // `App` lo abandona sin reacción y pregunta a la regla de cierre.
        let before = (world.snapshot(), extras(&world));
        assert!(modal.abandon());
        assert!(!modal.is_open());
        let closes = close_allowed(modified, &mut confirm, 0.0);
        assert_eq!(closes, !modified, "primera petición, modified {modified}");
        if modified {
            assert!(close_allowed(true, &mut confirm, 1.0), "la segunda cierra");
        }
        assert_eq!(
            (world.snapshot(), extras(&world)),
            before,
            "no se creó nada: la sesión es la de antes"
        );
    }
}

// ---------------------------------------------------------------------------------------------
// Unidad 20: «Convertir modo» (specs/hito-2-convertir-modo.md, fila 10, parte de `dispatch`)
// ---------------------------------------------------------------------------------------------
//
// - `Dispatched::ConvertMode` desde el menú (la entrada `TestImage → Convert mode_2…`) y el botón, con
//   cualquier origen y con una imagen de cada uno de los cuatro modos: la misma acción, el mismo
//   resultado y nada tocado (lo ejecuta `App`, como `NewImage`). Sin atajo: ninguna tecla la da.
// - Los cinco bloqueos (`gesture`, `dialog`, `text_focus`, `removal`, `modal`) la bloquean con
//   cualquier origen y con cualquier modo, y `enabled` es falso exactamente donde `dispatch` da
//   `Blocked`; un gesto de verdad y una quita pendiente también.
// - Con el diálogo de convertir abierto todo está bloqueado salvo `Quit`, que lo abandona sin
//   convertir nada y sigue la regla de cierre de siempre.
// - El zoom no cambia.
//
// Suposición: `Dispatched::ConvertMode` es una variante sin datos. `open_convert_mode(modal, from,
// locks)` es la from_index «Piezas».

use crate::convert_mode::open_convert_mode;

/// Una sesión por modo de la imagen, sin quita pendiente.
fn worlds_of_each_mode() -> Vec<(&'static str, World)> {
    vec![
        ("indexada", World::standard()),
        ("RGBA con historia", World::new_2(rgba(), 3, 1)),
        ("gris", World::new_2(gray(), 2, 0)),
        (
            "gris con alfa sin historia",
            World::new_2(gray_alpha(), 0, 0),
        ),
    ]
}

fn mode_of(world: &World) -> Mode {
    world.image_2().mode()
}

#[test]
fn row_10_convert_mode_gives_dispatched_convert_mode_with_each_source_and_each_mode_touching_nothing()
 {
    for (name_2, mut world) in worlds_of_each_mode() {
        for origin in SOURCES {
            let before = (world.snapshot(), extras(&world));
            let result = world.dispatches(AppAction::ConvertMode, origin);
            assert_eq!(result, Dispatched::ConvertMode, "{name_2}, {origin:?}");
            assert!(
                !matches!(result, Dispatched::View(_)),
                "{name_2}: no es zoom"
            );
            assert_eq!(
                (world.snapshot(), extras(&world)),
                before,
                "{name_2}, {origin:?}: despachar no toca nada (lo hace App)"
            );
        }
    }
}

#[test]
fn row_10_the_image_menu_entry_gives_dispatched_convert_mode() {
    let menu = MENUS
        .iter()
        .find(|m| m.title == "Imagen")
        .expect("el menú tiene «Imagen»");
    let input = menu
        .items
        .iter()
        .find(|item| item.label == "Convertir modo…")
        .expect("Imagen tiene «Convertir modo…»");
    assert_eq!(input.action, AppAction::ConvertMode);
    for (name_2, mut world) in worlds_of_each_mode() {
        let before = world.snapshot();
        let per_menu = world.dispatches(input.action, Origin::Menu);
        assert_eq!(per_menu, Dispatched::ConvertMode, "{name_2}");
        assert_eq!(world.snapshot(), before, "{name_2}");
    }
    // El botón (la misma acción) da lo mismo.
    let mut per_menu = World::standard();
    let mut per_button = World::standard();
    let a = per_menu.dispatches(input.action, Origin::Menu);
    let b = per_button.dispatches(AppAction::ConvertMode, Origin::Button);
    assert_eq!(a, b);
    assert_eq!(per_menu.snapshot(), per_button.snapshot());
}

#[test]
fn row_10_no_key_with_no_combination_gives_convert_mode() {
    for platform in PLATFORMS_OF_19 {
        for &key_2 in Key::ALL {
            for bits in 0..32u8 {
                let m = modifiers_of_bits(bits);
                assert_ne!(
                    action_for_shortcut(key_2, m, platform),
                    Some(AppAction::ConvertMode),
                    "{platform:?}, {key_2:?}, {bits:05b}"
                );
            }
        }
    }
}

#[test]
fn row_10_each_lock_blocks_convert_mode_with_each_source_and_each_mode_touching_nothing() {
    for (name_world, mut world) in worlds_of_each_mode() {
        for (name_2, locks) in locks_one_by_one() {
            for origin in SOURCES {
                let before = (world.snapshot(), extras(&world));
                assert_eq!(
                    world.dispatches_with(AppAction::ConvertMode, origin, locks),
                    Dispatched::Blocked,
                    "{name_world}, {name_2}, {origin:?}"
                );
                assert!(
                    is_blocked(AppAction::ConvertMode, locks, origin),
                    "{name_2}"
                );
                assert_eq!(
                    (world.snapshot(), extras(&world)),
                    before,
                    "{name_world}, {name_2}, {origin:?}: bloqueada no toca nada"
                );
            }
        }
    }
}

#[test]
fn row_10_with_the_32_lock_combinations_only_the_one_with_none_passes() {
    for bits in 0..32u8 {
        let locks = Locks {
            gesture: bits & 1 != 0,
            stroke: false,
            dialog: bits & 2 != 0,
            text_focus: bits & 4 != 0,
            removal: bits & 8 != 0,
            modal: bits & 16 != 0,
        };
        for origin in SOURCES {
            let mut world = World::standard();
            let expected_2 = if bits == 0 {
                Dispatched::ConvertMode
            } else {
                Dispatched::Blocked
            };
            assert_eq!(
                world.dispatches_with(AppAction::ConvertMode, origin, locks),
                expected_2,
                "{bits:05b}, {origin:?}"
            );
        }
    }
}

#[test]
fn row_10_enabled_is_false_exactly_where_dispatch_gives_blocked_with_menu_source() {
    for zoom in [None, Zoom::from_int(5)] {
        for bits in 0..32u8 {
            let locks = Locks {
                gesture: bits & 1 != 0,
                stroke: false,
                dialog: bits & 2 != 0,
                text_focus: bits & 4 != 0,
                removal: bits & 8 != 0,
                modal: bits & 16 != 0,
            };
            let mut world = World::standard();
            let state_2 = world.ui_state(zoom, locks);
            let locked_2 = world.dispatches_with(AppAction::ConvertMode, Origin::Menu, locks)
                == Dispatched::Blocked;
            assert_eq!(
                enabled(AppAction::ConvertMode, &state_2),
                !locked_2,
                "{bits:05b}, zoom {zoom:?}"
            );
        }
    }
}

#[test]
fn row_10_a_real_gesture_or_a_pending_removal_block_convert_mode() {
    // Un trazo en curso.
    let mut world = World::standard();
    starts_a_stroke(&mut world);
    let before = world.snapshot();
    assert_eq!(
        world.dispatches(AppAction::ConvertMode, Origin::Menu),
        Dispatched::Blocked,
        "trazo"
    );
    assert_eq!(world.snapshot(), before);
    assert!(!enabled(
        AppAction::ConvertMode,
        &world.ui_state(Zoom::from_int(5), world.locks())
    ));

    // El deslizador del editor.
    let mut world = World::standard();
    presses_the_slider(&mut world);
    assert_eq!(
        world.dispatches(AppAction::ConvertMode, Origin::Button),
        Dispatched::Blocked,
        "deslizador"
    );

    // El arrastre de una muestra.
    let mut world = World::standard();
    drags_a_swatch(&mut world);
    assert_eq!(
        world.dispatches(AppAction::ConvertMode, Origin::Menu),
        Dispatched::Blocked,
        "arrastre de muestra"
    );

    // Una quita de paleta pendiente: abrir un diálogo también se bloquea (regla literal de la 18).
    let mut world = world_with_a_pending_removal();
    assert!(world.locks().removal);
    let before = (world.snapshot(), extras(&world));
    for origin in SOURCES {
        assert_eq!(
            world.dispatches(AppAction::ConvertMode, origin),
            Dispatched::Blocked,
            "quita pendiente, {origin:?}"
        );
    }
    assert_eq!((world.snapshot(), extras(&world)), before);
    assert!(!enabled(
        AppAction::ConvertMode,
        &world.ui_state(Zoom::from_int(5), world.locks())
    ));
}

#[test]
fn row_10_with_the_convert_dialog_open_everything_is_locked_except_quit() {
    for (name_world, mut world) in worlds_of_each_mode() {
        let mut modal = Modal::default();
        assert!(open_convert_mode(
            &mut modal,
            mode_of(&world),
            world.locks()
        ));
        assert_eq!(modal.kind(), Some(ModalKind::ConvertMode), "{name_world}");
        let locks = with_modal(&world);
        assert!(locks.modal);

        let before = (world.snapshot(), extras(&world));
        for action in all_the_actions()
            .into_iter()
            .chain([AppAction::NewImage, AppAction::ConvertMode])
        {
            for origin in SOURCES {
                let expected_2 = match action {
                    AppAction::Quit => Dispatched::Quit,
                    // `CloseLicenses` no es de este diálogo, pero la regla (1) la deja pasar siempre.
                    AppAction::CloseLicenses => Dispatched::Licenses(false),
                    _ => Dispatched::Blocked,
                };
                assert_eq!(
                    world.dispatches_with(action, origin, locks),
                    expected_2,
                    "{name_world}, {action:?}, {origin:?}"
                );
            }
        }
        assert_eq!(
            (world.snapshot(), extras(&world)),
            before,
            "{name_world}: con el diálogo abierto el documento no cambia"
        );
    }
}

#[test]
fn row_10_an_open_dialog_does_not_allow_opening_another_or_another_convert_mode() {
    let world = World::standard();
    let mut modal = Modal::default();
    assert!(open_convert_mode(&mut modal, Mode::Indexed, world.locks()));
    let before = modal.clone();
    assert!(
        !open_convert_mode(&mut modal, Mode::Rgba, world.locks()),
        "ya hay uno abierto"
    );
    assert_eq!(modal, before, "no cambia el diálogo abierto");
    assert!(!opens_licenses(&mut modal));
    assert!(!open_new_image(&mut modal, 16384, world.locks()));
    assert_eq!(modal.kind(), Some(ModalKind::ConvertMode));
}

#[test]
fn row_10_each_lock_prevents_opening_the_convert_dialog() {
    for mode_2 in [Mode::Indexed, Mode::Rgba, Mode::GrayAlpha, Mode::Gray] {
        for (name_2, locks) in locks_one_by_one() {
            let mut modal = Modal::default();
            assert!(
                !open_convert_mode(&mut modal, mode_2, locks),
                "{mode_2:?}, {name_2}"
            );
            assert!(!modal.is_open(), "{mode_2:?}, {name_2}");
        }
        let mut modal = Modal::default();
        assert!(open_convert_mode(&mut modal, mode_2, Locks::default()));
    }
}

#[test]
fn row_10_quit_with_the_dialog_open_abandons_it_and_follows_the_close_rule() {
    for modified in [false, true] {
        let mut world = World::standard();
        let mut modal = Modal::default();
        let mut confirm = Confirm::new();
        assert!(open_convert_mode(&mut modal, Mode::Indexed, world.locks()));
        let locks = with_modal(&world);

        // `Quit` pasa con el diálogo abierto, con cualquier origen.
        for origin in SOURCES {
            assert_eq!(
                world.dispatches_with(AppAction::Quit, origin, locks),
                Dispatched::Quit,
                "{origin:?}"
            );
        }
        // `App` lo abandona sin reacción y pregunta a la regla de cierre.
        let before = (world.snapshot(), extras(&world));
        assert!(modal.abandon());
        assert!(!modal.is_open());
        let closes = close_allowed(modified, &mut confirm, 0.0);
        assert_eq!(closes, !modified, "primera petición, modified {modified}");
        if modified {
            assert!(close_allowed(true, &mut confirm, 1.0), "la segunda cierra");
        }
        assert_eq!(
            (world.snapshot(), extras(&world)),
            before,
            "no se convirtió nada: la sesión es la de antes"
        );
    }
}

#[test]
fn row_10_the_zoom_does_not_change_with_the_new_action() {
    // Los cuatro zooms siguen dando `View` con cualquier origen salvo `text_focus` desde el teclado,
    // y no los paran ni un gesto ni una quita pendiente (sí paran `ConvertMode`).
    for (action, op) in ZOOMS {
        for origin in SOURCES {
            let mut world = World::standard();
            assert_eq!(
                world.dispatches_with(action, origin, locks_with(|l| l.removal = true)),
                Dispatched::View(op),
                "{action:?}, {origin:?}"
            );
            assert_eq!(
                world.dispatches_with(action, origin, locks_with(|l| l.gesture = true)),
                Dispatched::View(op),
                "{action:?}, {origin:?}"
            );
        }
    }
    assert!(!AppAction::ConvertMode.is_zoom());
}

// ---------------------------------------------------------------------------------------------
// Unidad 19b: «Color…» (specs/hito-2-fondo-color.md, fila 3, parte de `dispatch`)
// ---------------------------------------------------------------------------------------------
//
// - `Dispatched::BackgroundColor` desde el menú, el botón y la tecla (la misma acción), con una
//   imagen de cada modo y con historia, una edición deshecha o sin historia: el mismo resultado y
//   nada tocado (`state_id`, historia, imagen, elección, herramienta; lo ejecuta `App`, como
//   `NewImage`). Sin atajo: ninguna tecla la da.
// - Los cinco bloqueos (`gesture`, `dialog`, `text_focus`, `removal`, `modal`) la bloquean, solos y
//   combinados, con cualquier origen; un gesto de verdad y una quita pendiente también, mientras las
//   cuatro fijas (`Black`...) siguen dando `Dispatched::Background`.
// - Con el diálogo de color abierto todo está bloqueado salvo `Quit`, que lo abandona sin aplicar
//   nada y sigue la regla de cierre de siempre.
//
// Suposiciones: `Dispatched::BackgroundColor` es una variante sin datos (`Clone, Debug, PartialEq`
// ya los tiene `Dispatched`); `open_background_color(modal, current, locks) -> bool` es la de «Piezas»;
// `DialogKind::BackgroundColor` es el tipo del diálogo.

use crate::background_color::open_background_color;

/// Un fondo activo de cada clase, para abrir el diálogo desde cada uno.
const BACKGROUNDS_OF_SPLIT: [CanvasBackground; 5] = [
    CanvasBackground::Checker,
    CanvasBackground::Black,
    CanvasBackground::Gray,
    CanvasBackground::White,
    CanvasBackground::Custom(Rgba::opaque(0x11, 0x22, 0x33)),
];

#[test]
fn row_3_color_gives_dispatched_background_color_with_each_source_and_each_mode_touching_nothing() {
    let mut worlds = worlds_of_each_mode();
    worlds.extend(worlds_without_removes());
    for (name_2, mut world) in worlds {
        for origin in SOURCES {
            let before = (world.snapshot(), extras(&world));
            let result = world.dispatches(AppAction::BackgroundColor, origin);
            assert_eq!(result, Dispatched::BackgroundColor, "{name_2}, {origin:?}");
            assert!(
                !matches!(
                    result,
                    Dispatched::View(_) | Dispatched::Background(_) | Dispatched::Done { .. }
                ),
                "{name_2}: no es zoom, ni uno de los fondos fijos, ni una acción de documento"
            );
            assert_eq!(
                (world.snapshot(), extras(&world)),
                before,
                "{name_2}, {origin:?}: despachar no toca nada (lo hace App)"
            );
        }
    }
}

#[test]
fn row_3_the_color_entry_of_the_submenu_gives_dispatched_background_color() {
    let submenu = MENUS
        .iter()
        .find(|m| m.title == "Ver")
        .expect("el menú tiene «Ver»")
        .submenus
        .first()
        .expect("Ver tiene «Fondo del lienzo»");
    let input = submenu
        .items
        .iter()
        .find(|item| item.label == "Color…")
        .expect("el submenú tiene «Color…»");
    assert_eq!(input.action, AppAction::BackgroundColor);
    let mut per_menu = World::standard();
    let mut per_button = World::standard();
    let a = per_menu.dispatches(input.action, Origin::Menu);
    let b = per_button.dispatches(AppAction::BackgroundColor, Origin::Button);
    assert_eq!(a, Dispatched::BackgroundColor);
    assert_eq!(a, b);
    assert_eq!(per_menu.snapshot(), per_button.snapshot());
}

#[test]
fn row_3_no_key_with_no_combination_gives_background_color() {
    for platform in PLATFORMS_OF_19 {
        for &key_2 in Key::ALL {
            for bits in 0..32u8 {
                let m = modifiers_of_bits(bits);
                assert_ne!(
                    action_for_shortcut(key_2, m, platform),
                    Some(AppAction::BackgroundColor),
                    "{platform:?}, {key_2:?}, {bits:05b}"
                );
            }
        }
    }
}

#[test]
fn row_3_each_lock_blocks_background_color_with_each_source_and_each_mode_touching_nothing() {
    let mut worlds = worlds_of_each_mode();
    worlds.push(("con una quita pendiente", world_with_a_pending_removal()));
    for (name_world, mut world) in worlds {
        for (name_2, locks) in locks_one_by_one() {
            for origin in SOURCES {
                let before = (world.snapshot(), extras(&world));
                assert_eq!(
                    world.dispatches_with(AppAction::BackgroundColor, origin, locks),
                    Dispatched::Blocked,
                    "{name_world}, {name_2}, {origin:?}"
                );
                assert!(
                    is_blocked(AppAction::BackgroundColor, locks, origin),
                    "{name_2}"
                );
                assert_eq!(
                    (world.snapshot(), extras(&world)),
                    before,
                    "{name_world}, {name_2}, {origin:?}: bloqueada no toca nada"
                );
            }
        }
    }
}

#[test]
fn row_3_with_the_32_lock_combinations_only_the_one_with_none_passes() {
    for bits in 0..32u8 {
        let locks = Locks {
            gesture: bits & 1 != 0,
            stroke: false,
            dialog: bits & 2 != 0,
            text_focus: bits & 4 != 0,
            removal: bits & 8 != 0,
            modal: bits & 16 != 0,
        };
        for origin in SOURCES {
            let mut world = World::standard();
            let expected_2 = if bits == 0 {
                Dispatched::BackgroundColor
            } else {
                Dispatched::Blocked
            };
            assert_eq!(
                world.dispatches_with(AppAction::BackgroundColor, origin, locks),
                expected_2,
                "{bits:05b}, {origin:?}"
            );
        }
    }
}

#[test]
fn row_3_enabled_of_background_color_is_false_exactly_where_dispatch_gives_blocked() {
    for zoom in [None, Zoom::from_int(5)] {
        for bits in 0..32u8 {
            let locks = Locks {
                gesture: bits & 1 != 0,
                stroke: false,
                dialog: bits & 2 != 0,
                text_focus: bits & 4 != 0,
                removal: bits & 8 != 0,
                modal: bits & 16 != 0,
            };
            let mut world = World::standard();
            let state_2 = world.ui_state(zoom, locks);
            let locked_2 = world.dispatches_with(AppAction::BackgroundColor, Origin::Menu, locks)
                == Dispatched::Blocked;
            assert_eq!(
                enabled(AppAction::BackgroundColor, &state_2),
                !locked_2,
                "{bits:05b}, zoom {zoom:?}"
            );
        }
    }
}

#[test]
fn row_3_a_real_gesture_or_a_pending_removal_block_color_and_not_the_four_fixed_ones() {
    let four = [
        CanvasBackground::Checker,
        CanvasBackground::Black,
        CanvasBackground::Gray,
        CanvasBackground::White,
    ];
    let checks = |name_2: &str, world: &mut World| {
        for origin in SOURCES {
            let before = (world.snapshot(), extras(world));
            assert_eq!(
                world.dispatches(AppAction::BackgroundColor, origin),
                Dispatched::Blocked,
                "{name_2}, {origin:?}: `Color…` bloqueada"
            );
            assert!(!enabled(
                AppAction::BackgroundColor,
                &world.ui_state(Zoom::from_int(5), world.locks())
            ));
            // Las cuatro fijas pasan con el mismo gesto o la misma quita.
            for background_2 in four {
                assert_eq!(
                    world.dispatches(AppAction::CanvasBackground(background_2), origin),
                    Dispatched::Background(background_2),
                    "{name_2}, {origin:?}: {background_2:?} sigue habilitada"
                );
                assert!(enabled(
                    AppAction::CanvasBackground(background_2),
                    &world.ui_state(Zoom::from_int(5), world.locks())
                ));
            }
            assert_eq!(
                (world.snapshot(), extras(world)),
                before,
                "{name_2}, {origin:?}: nada cambia"
            );
        }
    };
    // Un trazo en curso.
    let mut world = World::standard();
    starts_a_stroke(&mut world);
    checks("trazo", &mut world);
    // El deslizador del editor.
    let mut world = World::standard();
    presses_the_slider(&mut world);
    checks("deslizador", &mut world);
    // El arrastre de una muestra.
    let mut world = World::standard();
    drags_a_swatch(&mut world);
    checks("arrastre de muestra", &mut world);
    // Una quita de paleta pendiente.
    let mut world = world_with_a_pending_removal();
    assert!(world.locks().removal);
    checks("quita pendiente", &mut world);
}

#[test]
fn row_3_with_the_color_dialog_open_everything_is_locked_except_quit() {
    for background_2 in BACKGROUNDS_OF_SPLIT {
        for (name_world, mut world) in worlds_of_each_mode() {
            let mut modal = Modal::default();
            assert!(open_background_color(
                &mut modal,
                background_2,
                world.locks()
            ));
            assert_eq!(
                modal.kind(),
                Some(ModalKind::BackgroundColor),
                "{name_world}"
            );
            let locks = with_modal(&world);
            assert!(locks.modal);
            let before = (world.snapshot(), extras(&world));
            for action in all_the_actions()
                .into_iter()
                .chain(BACKGROUNDS.map(action_of_background))
                .chain([
                    AppAction::NewImage,
                    AppAction::ConvertMode,
                    AppAction::BackgroundColor,
                    action_of_background(CanvasBackground::Custom(Rgba::opaque(1, 2, 3))),
                ])
            {
                for origin in SOURCES {
                    let expected_2 = match action {
                        AppAction::Quit => Dispatched::Quit,
                        AppAction::CloseLicenses => Dispatched::Licenses(false),
                        _ => Dispatched::Blocked,
                    };
                    assert_eq!(
                        world.dispatches_with(action, origin, locks),
                        expected_2,
                        "{name_world}, {background_2:?}, {action:?}, {origin:?}"
                    );
                }
            }
            assert_eq!(
                (world.snapshot(), extras(&world)),
                before,
                "{name_world}: con el diálogo abierto el documento no cambia"
            );
        }
    }
}

#[test]
fn row_3_an_open_dialog_does_not_allow_opening_another_or_another_color() {
    let world = World::standard();
    let mut modal = Modal::default();
    assert!(open_background_color(
        &mut modal,
        CanvasBackground::Black,
        world.locks()
    ));
    let before = modal.clone();
    assert!(
        !open_background_color(&mut modal, CanvasBackground::White, world.locks()),
        "ya hay uno abierto"
    );
    assert_eq!(modal, before, "no cambia el diálogo abierto");
    assert!(!opens_licenses(&mut modal));
    assert!(!open_new_image(&mut modal, 16384, world.locks()));
    assert!(!open_convert_mode(&mut modal, Mode::Indexed, world.locks()));
    assert_eq!(modal.kind(), Some(ModalKind::BackgroundColor));
    // Y al revés: con otro diálogo abierto no se abre el de color.
    let mut other_2 = Modal::default();
    assert!(opens_licenses(&mut other_2));
    assert!(!open_background_color(
        &mut other_2,
        CanvasBackground::Checker,
        Locks::default()
    ));
    assert_eq!(other_2.kind(), Some(ModalKind::Licenses));
}

#[test]
fn row_3_quit_with_the_color_dialog_open_abandons_it_and_follows_the_close_rule() {
    for modified in [false, true] {
        let mut world = World::standard();
        let mut modal = Modal::default();
        let mut confirm = Confirm::new();
        assert!(open_background_color(
            &mut modal,
            CanvasBackground::Custom(Rgba::opaque(0x11, 0x22, 0x33)),
            world.locks()
        ));
        let locks = with_modal(&world);

        // `Quit` pasa con el diálogo abierto, con cualquier origen.
        for origin in SOURCES {
            assert_eq!(
                world.dispatches_with(AppAction::Quit, origin, locks),
                Dispatched::Quit,
                "{origin:?}"
            );
        }
        // `App` lo abandona sin reacción y pregunta a la regla de cierre.
        let before = (world.snapshot(), extras(&world));
        assert!(modal.abandon());
        assert!(!modal.is_open());
        let closes = close_allowed(modified, &mut confirm, 0.0);
        assert_eq!(closes, !modified, "primera petición, modified {modified}");
        if modified {
            assert!(close_allowed(true, &mut confirm, 1.0), "la segunda cierra");
        }
        assert_eq!(
            (world.snapshot(), extras(&world)),
            before,
            "no se aplicó nada: la sesión es la de antes"
        );
    }
}

// ---------------------------------------------------------------------------------------------
// I.2b, fila 8: botones laterales (specs/interfaz-raton.md). Eventos sintéticos de egui al nivel al
// que llegarían desde el hardware (`PointerButton::Extra1` y `Extra2`), traducidos por la tabla de
// gestos y despachados con `Origin::Button`.
// ---------------------------------------------------------------------------------------------

mod buttons_side {
    use super::*;
    use eframe::egui::{Event, PointerButton, Pos2};
    use px_ui::gestures::{Button, GestureAction, GestureMap, GestureRow, Input, Mods, Scope};
    use px_ui::keymap::Platforms;
    use px_ui::pointer_events::window_actions;

    fn presses(button: PointerButton, pressed: bool) -> Event {
        Event::PointerButton {
            pos: Pos2::new(5.0, 5.0),
            button,
            pressed,
            modifiers: Modifiers::NONE,
        }
    }

    fn actions(events: &[Event]) -> Vec<AppAction> {
        window_actions(events, &GestureMap::default_scheme(), Platform::Linux)
    }

    #[test]
    fn row_8_back_undoes_and_forward_redoes_the_same_as_the_keyboard() {
        assert_eq!(
            actions(&[presses(PointerButton::Extra1, true)]),
            [AppAction::Undo]
        );
        assert_eq!(
            actions(&[presses(PointerButton::Extra2, true)]),
            [AppAction::Redo]
        );
        for (button_2, action) in [
            (PointerButton::Extra1, AppAction::Undo),
            (PointerButton::Extra2, AppAction::Redo),
        ] {
            let mut side = World::standard();
            let mut keyboard = World::standard();
            let [translated] = actions(&[presses(button_2, true)])[..] else {
                panic!("una acción")
            };
            assert_eq!(translated, action);
            let a = side.dispatches(translated, Origin::Button);
            let b = keyboard.dispatches(action, Origin::Key);
            assert_eq!(a, b, "mismo Dispatched");
            assert_eq!(a, done(true));
            assert_eq!(side.snapshot(), keyboard.snapshot(), "mismo estado");
        }
    }

    #[test]
    fn row_8_without_history_they_do_nothing() {
        let mut m = World::new_2(rgba(), 0, 0);
        let before = m.snapshot();
        assert_eq!(m.dispatches(AppAction::Undo, Origin::Button), done(false));
        assert_eq!(m.dispatches(AppAction::Redo, Origin::Button), done(false));
        assert_eq!(m.snapshot(), before);
    }

    #[test]
    fn row_8_blocked_with_gesture_modal_dialog_and_field() {
        for action in [AppAction::Undo, AppAction::Redo] {
            for locks in [
                locks_with(|l| l.dialog = true),
                locks_with(|l| l.modal = true),
                locks_with(|l| l.text_focus = true),
                locks_with(|l| l.gesture = true),
            ] {
                let mut m = World::standard();
                let before = m.snapshot();
                assert_eq!(
                    m.dispatches_with(action, Origin::Button, locks),
                    Dispatched::Blocked
                );
                assert_eq!(m.snapshot(), before);
            }
        }
        // Con un trazo de verdad en curso no se deshace a mitad de trazo.
        let mut m = World::standard();
        starts_a_stroke(&mut m);
        let before = m.snapshot();
        assert_eq!(
            m.dispatches(AppAction::Undo, Origin::Button),
            Dispatched::Blocked
        );
        assert_eq!(m.snapshot(), before);
    }

    #[test]
    fn row_8_releasing_u_or_other_buttons_or_the_wheel_do_nothing() {
        assert!(actions(&[presses(PointerButton::Extra1, false)]).is_empty());
        assert!(actions(&[presses(PointerButton::Primary, true)]).is_empty());
        assert!(actions(&[presses(PointerButton::Middle, true)]).is_empty());
        assert!(actions(&[presses(PointerButton::Secondary, true)]).is_empty());
        let wheel = Event::MouseWheel {
            unit: eframe::egui::MouseWheelUnit::Line,
            delta: eframe::egui::vec2(0.0, 1.0),
            phase: eframe::egui::TouchPhase::Move,
            source: eframe::egui::MouseWheelSource::Unknown,
            modifiers: Modifiers::NONE,
        };
        assert!(
            actions(&[wheel]).is_empty(),
            "la rueda es del lienzo, no de la ventana"
        );
        // Un pulsado y un soltado: una sola acción.
        assert_eq!(
            actions(&[
                presses(PointerButton::Extra1, true),
                presses(PointerButton::Extra1, false)
            ]),
            [AppAction::Undo]
        );
    }

    #[test]
    fn row_8_with_the_table_replaced_what_each_button_does_changes() {
        let back_to_zoom = GestureMap::new(vec![GestureRow {
            input: Input::Press(Button::Back),
            mods: Mods {
                command: None,
                shift: None,
                space: None,
            },
            action: GestureAction::App(AppAction::ZoomIn),
            scope: Scope::Window,
            active: Platforms::All,
        }]);
        let actions_of = |table: &GestureMap, button_2| {
            window_actions(&[presses(button_2, true)], table, Platform::Windows)
        };
        assert_eq!(
            actions_of(&back_to_zoom, PointerButton::Extra1),
            [AppAction::ZoomIn]
        );
        assert!(actions_of(&back_to_zoom, PointerButton::Extra2).is_empty());
        let mut m = World::standard();
        assert_eq!(
            m.dispatches(AppAction::ZoomIn, Origin::Button),
            Dispatched::View(ViewOp::ZoomIn)
        );
        let empty = GestureMap::new(vec![]);
        assert!(actions_of(&empty, PointerButton::Extra1).is_empty());
        assert!(actions_of(&empty, PointerButton::Extra2).is_empty());
    }
}

// ---------------------------------------------------------------------------------------------
// Unidad I.3 (specs/interfaz-caja-derecha.md): clic derecho en la caja de herramientas
// ---------------------------------------------------------------------------------------------

mod toolbox_right {
    use super::*;
    use px_ui::toolbar::{SecondaryOutcome, secondary_outcome};

    fn per_label(label: &str) -> &'static px_ui::toolbar::Button {
        TOOL_BUTTONS
            .iter()
            .chain(ZOOM_BUTTONS.iter())
            .find(|b| b.label == label)
            .expect("el botón existe")
    }

    /// Fila 5: el clic derecho en una herramienta es el mismo `SelectTool` con `Origin::Button`:
    /// mismo `Dispatched`, mismo aviso del borrador y mismo estado que el clic izquierdo.
    #[test]
    fn row_5_the_right_click_on_a_tool_dispatches_the_same_as_the_left() {
        for button in &TOOL_BUTTONS {
            let mut right = World::standard();
            let state_2 = right.ui_state(Zoom::from_int(5), Locks::default());
            let SecondaryOutcome::SelectAndToggle(action) = secondary_outcome(button, &state_2)
            else {
                panic!("{}: el clic derecho elige y abre la ventana", button.label);
            };
            let mut left = World::standard();
            let d = right.dispatches(action, Origin::Button);
            let i = left.dispatches(button.action, Origin::Button);
            assert_eq!(d, i, "{}", button.label);
            assert_eq!(right.snapshot(), left.snapshot(), "{}", button.label);
            assert_eq!(right.tools.tool(), left.tools.tool(), "{}", button.label);
        }
        // El aviso del borrador en una indexada sin transparencia sale igual.
        let mut m = World::standard();
        let Dispatched::Done { notice, .. } =
            m.dispatches(AppAction::SelectTool(Tool::Eraser), Origin::Button)
        else {
            panic!("el borrador se elige");
        };
        assert_eq!(notice.as_deref(), Some(NO_INDEX_NOTICE));
    }

    /// Fila 5: con un bloqueo, el clic derecho no hace nada y `dispatch` también da `Blocked`.
    #[test]
    fn row_5_locked_the_right_click_does_not_come_out_and_dispatch_gives_blocked() {
        for put in [
            (|l: &mut Locks| l.gesture = true) as fn(&mut Locks),
            |l| l.dialog = true,
            |l| l.modal = true,
            |l| l.text_focus = true,
        ] {
            let locks = locks_with(put);
            let mut m = World::standard();
            let state_2 = m.ui_state(Zoom::from_int(5), locks);
            for button in &TOOL_BUTTONS {
                assert_eq!(
                    secondary_outcome(button, &state_2),
                    SecondaryOutcome::Nothing
                );
                assert_eq!(
                    m.dispatches_with(button.action, Origin::Button, locks),
                    Dispatched::Blocked
                );
            }
        }
    }

    /// Fila 6: `Acercar` con el derecho despacha lo que `Alejar` con el izquierdo, y al revés.
    #[test]
    fn row_6_right_click_on_zoom_in_and_out_dispatches_what_the_left_of_the_other_does() {
        let a = World::standard();
        let state_2 = a.ui_state(Zoom::from_int(5), Locks::default());
        for (label, opposite) in [("Acercar", "Alejar"), ("Alejar", "Acercar")] {
            let SecondaryOutcome::Action(action) = secondary_outcome(per_label(label), &state_2)
            else {
                panic!("{label}: el clic derecho da una acción");
            };
            let mut right = World::standard();
            let mut left = World::standard();
            let d = right.dispatches(action, Origin::Button);
            let i = left.dispatches(per_label(opposite).action, Origin::Button);
            assert_eq!(d, i, "{label}");
            assert!(matches!(d, Dispatched::View(_)), "{label}: {d:?}");
            assert_eq!(right.snapshot(), left.snapshot(), "{label}");
        }
        for label in ["Ajustar", "1:1"] {
            assert_eq!(
                secondary_outcome(per_label(label), &state_2),
                SecondaryOutcome::Nothing
            );
        }
    }

    proptest! {
        #![proptest_config(ProptestConfig::with_cases(256))]

        /// Propiedad 3 (parte del documento): ninguna secuencia de clics derechos en la caja
        /// cambia `state_id`, `can_undo` ni `can_redo`, con cualquier bloqueo.
        #[test]
        fn property_3_right_clicks_do_not_touch_the_document_or_the_history(
            clicks in prop::collection::vec(
                (0usize..9, any::<bool>(), any::<bool>(), any::<bool>(), 0usize..1000),
                0..24,
            ),
        ) {
            let levels: Vec<Zoom> = Zoom::levels().collect();
            let mut m = World::standard();
            let before = m.snapshot();
            for (index_2, gesture, dialog, text_focus, level) in clicks {
                let locks = Locks { gesture, dialog, text_focus, ..Locks::default() };
                let zoom = Some(levels[level % levels.len()]);
                let state_2 = m.ui_state(zoom, locks);
                let button_2 = TOOL_BUTTONS.iter().chain(ZOOM_BUTTONS.iter()).nth(index_2).expect("hay nueve botones");
                match secondary_outcome(button_2, &state_2) {
                    SecondaryOutcome::Nothing => {}
                    SecondaryOutcome::Action(a) | SecondaryOutcome::SelectAndToggle(a) => {
                        let d = m.dispatches_with(a, Origin::Button, locks);
                        if !matches!(a, AppAction::SelectTool(_)) {
                            prop_assert!(matches!(d, Dispatched::View(_)), "{a:?}: {d:?}");
                        }
                    }
                }
                let now = m.snapshot();
                prop_assert_eq!(now.state_id, before.state_id);
                prop_assert_eq!(now.can_undo, before.can_undo);
                prop_assert_eq!(now.can_redo, before.can_redo);
            }
        }
    }
}
