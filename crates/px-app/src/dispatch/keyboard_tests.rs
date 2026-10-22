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
//! Los ayudantes de `dispatch/tests.rs` son privados: aquí se duplican los mínimos (`Mundo`,
//! `Foto`, los gestos de verdad).
//!
//! Suposiciones sobre lo que la spec no fija (anotadas en el informe):
//! - `dispatch(ColorNext | ColorPrev)` da `Done { refresh: false, notice: None }` (como
//!   `SelectColor`) y `dispatch(CenterOnCursor)` da `Dispatched::View(ViewOp::CenterOnCursor)`.
//! - `key_actions` se traduce en `acciones_de_teclado`: `presses`, la tabla por defecto y el
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

const PLATAFORMAS: [Platform; 3] = [Platform::Linux, Platform::Windows, Platform::Mac];
const ORIGENES: [Origin; 3] = [Origin::Menu, Origin::Button, Origin::Key];
const NO_INDEX_NOTICE: &str = "esta imagen no tiene índice transparente";

fn distintos(n: usize) -> Vec<Rgba> {
    (0..n)
        .map(|i| Rgba::opaque(i as u8, 255 - i as u8, (i * 7) as u8))
        .collect()
}

fn rgba() -> Image {
    Image::new(4, 4, Mode::Rgba).expect("imagen RGBA")
}

fn gris() -> Image {
    Image::new(4, 4, Mode::Gray).expect("imagen Gray")
}

fn gris_alfa() -> Image {
    Image::new(4, 4, Mode::GrayAlpha).expect("imagen GrayAlpha")
}

/// Indexada de 4×4 con `n` entradas distintas y sin índice transparente.
fn indexada(n: usize) -> Image {
    let paleta = Palette::from_colors(distintos(n)).expect("paleta");
    Image::new_indexed(4, 4, paleta).expect("imagen indexada")
}

fn pixel_distinto(image: &Image, i: usize) -> Pixel {
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
fn un_paso(document: &mut Document, i: usize) {
    let id = ImageId::from_raw(0);
    let image = document.image(id).expect("imagen").clone();
    let x = (i / 4) as u32 % 4;
    let y = (i % 4) as u32;
    let cambiado = document
        .apply(Command::SetPixels {
            image: id,
            pixels: vec![PixelEdit {
                x,
                y,
                pixel: pixel_distinto(&image, i),
            }],
        })
        .expect("comando válido");
    assert!(cambiado, "el paso {i} debe cambiar la imagen");
}

/// Sesión y herramienta: todo lo que `dispatch` puede tocar.
struct Mundo {
    session: Session,
    tools: ToolState,
}

/// Lo que se compara entre orígenes y lo que un `Blocked` no debe tocar.
#[derive(Clone, Debug, PartialEq)]
struct Foto {
    state_id: u64,
    can_undo: bool,
    can_redo: bool,
    image: Image,
    choice: ColorChoice,
    tool: Tool,
    removal: Option<Removal>,
    reveal: bool,
    gesto: bool,
}

/// Lo que ni elegir color ni centrar la vista tocan: el documento y su historia.
#[derive(Clone, Debug, PartialEq)]
struct Documento {
    state_id: u64,
    can_undo: bool,
    can_redo: bool,
    image: Image,
}

impl Mundo {
    /// Sesión con `pasos` pasos de historia y, de ellos, `deshechos` deshechos.
    fn nuevo(image: Image, pasos: usize, deshechos: usize) -> Mundo {
        let mut document = Document::new(image);
        for i in 0..pasos {
            un_paso(&mut document, i);
        }
        let mut session = Session::sample(document);
        for _ in 0..deshechos {
            session.undo().expect("hay un paso que deshacer");
        }
        let _ = session.take_removal_cancelled();
        session.view.reveal = false;
        Mundo {
            session,
            tools: ToolState::new(),
        }
    }

    /// Indexada de 4 entradas sin transparente, con 2 pasos y 1 deshecho.
    fn estandar() -> Mundo {
        Mundo::nuevo(indexada(4), 2, 1)
    }

    fn imagen(&self) -> &Image {
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

    fn despacha_con(&mut self, action: AppAction, origin: Origin, locks: Locks) -> Dispatched {
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
    fn despacha(&mut self, action: AppAction, origin: Origin) -> Dispatched {
        let locks = self.locks();
        self.despacha_con(action, origin, locks)
    }

    fn foto(&self) -> Foto {
        Foto {
            state_id: self.session.document.state_id(),
            can_undo: self.session.document.can_undo(),
            can_redo: self.session.document.can_redo(),
            image: self.imagen().clone(),
            choice: self.session.choice,
            tool: self.tools.tool(),
            removal: self.session.ops.removal(),
            reveal: self.session.view.reveal,
            gesto: self.tools.busy() || self.session.busy(),
        }
    }

    fn documento(&self) -> Documento {
        Documento {
            state_id: self.session.document.state_id(),
            can_undo: self.session.document.can_undo(),
            can_redo: self.session.document.can_redo(),
            image: self.imagen().clone(),
        }
    }

    fn estado_de_la_ui(&self, locks: Locks) -> UiState {
        UiState {
            can_undo: self.session.document.can_undo(),
            can_redo: self.session.document.can_redo(),
            zoom: Zoom::from_int(5),
            tool: self.tools.tool(),
            locks,
        }
    }
}

fn hecho(refresh: bool) -> Dispatched {
    Dispatched::Done {
        refresh,
        notice: None,
    }
}

fn locks_con(f: impl FnOnce(&mut Locks)) -> Locks {
    let mut locks = Locks::default();
    f(&mut locks);
    locks
}

fn bloqueos(bits: u8) -> Locks {
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

fn tecla(key: Key, modifiers: Modifiers) -> Event {
    Event::Key {
        key,
        physical_key: Some(key),
        pressed: true,
        repeat: false,
        modifiers,
    }
}

fn repetida(key: Key, modifiers: Modifiers) -> Event {
    Event::Key {
        key,
        physical_key: Some(key),
        pressed: true,
        repeat: true,
        modifiers,
    }
}

fn texto(t: &str) -> Event {
    Event::Text(t.to_string())
}

/// La tecla con el texto que produce, como la entrega `egui-winit`.
fn escribe(key: Key, modifiers: Modifiers, t: &str) -> Vec<Event> {
    vec![tecla(key, modifiers), texto(t)]
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
fn acciones_de_teclado(events: &[Event], platform: Platform) -> Vec<AppAction> {
    let km = Keymap::default_scheme();
    presses(events)
        .iter()
        .filter_map(|p| km.action_for(p, platform).map(|a| (p.repeat, a)))
        .filter(|(repeat, action)| !(*repeat && action.ignores_repeat()))
        .map(|(_, action)| action)
        .collect()
}

/// La primera (y única) acción que da una secuencia.
fn una_accion(events: &[Event], platform: Platform) -> AppAction {
    let acciones = acciones_de_teclado(events, platform);
    assert_eq!(
        acciones.len(),
        1,
        "{events:?} en {platform:?}: {acciones:?}"
    );
    acciones[0]
}

// --- Gestos de verdad ---

fn empieza_un_trazo(mundo: &mut Mundo) {
    let id = ImageId::from_raw(0);
    let pixel = mundo.session.active_pixel();
    assert!(pixel.is_some(), "hay un color activo");
    mundo
        .tools
        .canvas_event_with(
            &mut mundo.session.document,
            id,
            pixel,
            CanvasEvent::StrokeBegin((0, 0)),
        )
        .expect("pulsar");
    assert!(mundo.tools.busy(), "el trazo está en curso");
}

fn pulsa_el_deslizador(mundo: &mut Mundo) {
    mundo.session.sync_editor();
    assert!(
        mundo.session.editor.begin(Control::Red, 10),
        "el editor admite el arrastre"
    );
    assert!(mundo.session.busy());
}

fn arrastra_una_muestra(mundo: &mut Mundo) {
    mundo.session.ops.begin_drag(1, (0, 0));
    mundo
        .session
        .ops
        .drag()
        .expect("hay arrastre")
        .update((200, 200), 1, Some(2));
    assert!(mundo.session.busy());
}

/// Quita pendiente de la entrada 3 de una indexada de 5 (que usan dos píxeles).
fn mundo_con_quita_pendiente() -> Mundo {
    let paleta = Palette::from_colors(distintos(5)).expect("paleta");
    let mut image = Image::new_indexed(5, 1, paleta).expect("imagen indexada");
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
    let destino = {
        let image = session
            .document
            .image(ImageId::from_raw(0))
            .expect("imagen");
        default_destination(image.palette().expect("indexada"), 3)
    };
    session.ops.begin_removal(3, destino);
    assert!(session.ops.removal().is_some());
    Mundo {
        session,
        tools: ToolState::new(),
    }
}

#[derive(Clone, Copy, Debug)]
enum Gesto {
    Trazo,
    Deslizador,
    Muestra,
}

const GESTOS: [Gesto; 3] = [Gesto::Trazo, Gesto::Deslizador, Gesto::Muestra];

fn mundo_con_gesto(gesto: Gesto) -> Mundo {
    let mut mundo = Mundo::estandar();
    match gesto {
        Gesto::Trazo => empieza_un_trazo(&mut mundo),
        Gesto::Deslizador => pulsa_el_deslizador(&mut mundo),
        Gesto::Muestra => arrastra_una_muestra(&mut mundo),
    }
    assert!(mundo.locks().gesture, "{gesto:?}");
    mundo
}

// ---------------------------------------------------------------------------------------------
// Fila 7: color siguiente y anterior por dispatch
// ---------------------------------------------------------------------------------------------

/// Pulsa `acción` con `Origin::Key` y comprueba lo común: `Done` sin refresco ni aviso, el
/// documento intacto y la fila elegida marcada para mostrarse.
fn pulsa_y_comprueba(mundo: &mut Mundo, action: AppAction, esperada: ColorChoice, contexto: &str) {
    let documento = mundo.documento();
    mundo.session.view.reveal = false;
    let resultado = mundo.despacha(action, Origin::Key);
    assert_eq!(resultado, hecho(false), "{contexto}: {action:?}");
    assert_eq!(mundo.session.choice, esperada, "{contexto}: {action:?}");
    assert!(
        mundo.session.view.reveal,
        "{contexto}: {action:?} no muestra la fila elegida"
    );
    assert_eq!(
        mundo.documento(),
        documento,
        "{contexto}: {action:?} cambió el documento"
    );
}

#[test]
fn fila_7_indexada_de_256_color_next_en_la_ultima_da_la_primera_y_color_prev_en_la_primera_la_ultima()
 {
    let mut mundo = Mundo::nuevo(indexada(256), 2, 1);
    mundo.session.set_choice_revealing(ColorChoice::Entry(255));
    pulsa_y_comprueba(
        &mut mundo,
        AppAction::ColorNext,
        ColorChoice::Entry(0),
        "256, de la última",
    );
    pulsa_y_comprueba(
        &mut mundo,
        AppAction::ColorPrev,
        ColorChoice::Entry(255),
        "256, de la primera",
    );
}

#[test]
fn fila_7_indexada_de_256_un_paso_por_pulsacion_en_los_dos_sentidos() {
    let mut mundo = Mundo::nuevo(indexada(256), 0, 0);
    assert_eq!(mundo.session.choice, ColorChoice::Entry(0));
    for entrada in 1..=10u8 {
        pulsa_y_comprueba(
            &mut mundo,
            AppAction::ColorNext,
            ColorChoice::Entry(entrada),
            "256 hacia delante",
        );
    }
    for entrada in (0..10u8).rev() {
        pulsa_y_comprueba(
            &mut mundo,
            AppAction::ColorPrev,
            ColorChoice::Entry(entrada),
            "256 hacia atrás",
        );
    }
}

#[test]
fn fila_7_indexada_de_9_da_la_vuelta_en_las_dos_direcciones_y_recorre_todas_las_entradas() {
    let mut mundo = Mundo::nuevo(indexada(9), 0, 0);
    let mut vistas = vec![mundo.session.choice];
    for _ in 0..9 {
        mundo.despacha(AppAction::ColorNext, Origin::Key);
        vistas.push(mundo.session.choice);
    }
    let esperadas: Vec<ColorChoice> = (0..=9u8).map(|i| ColorChoice::Entry(i % 9)).collect();
    assert_eq!(
        vistas, esperadas,
        "nueve pasos hacia delante y vuelta al principio"
    );
    let mut atras = vec![mundo.session.choice];
    for _ in 0..9 {
        mundo.despacha(AppAction::ColorPrev, Origin::Key);
        atras.push(mundo.session.choice);
    }
    let esperadas: Vec<ColorChoice> = (0..=9u8).map(|i| ColorChoice::Entry((9 - i) % 9)).collect();
    assert_eq!(
        atras, esperadas,
        "nueve pasos hacia atrás y vuelta al principio"
    );
}

#[test]
fn fila_7_rgba_slot_9_da_slot_1_y_slot_1_da_slot_9() {
    let mut mundo = Mundo::nuevo(rgba(), 2, 1);
    mundo.session.set_choice_revealing(ColorChoice::Slot(9));
    pulsa_y_comprueba(
        &mut mundo,
        AppAction::ColorNext,
        ColorChoice::Slot(1),
        "RGBA",
    );
    pulsa_y_comprueba(
        &mut mundo,
        AppAction::ColorPrev,
        ColorChoice::Slot(9),
        "RGBA",
    );
}

#[test]
fn fila_7_rgba_recorre_las_nueve_ranuras_un_paso_por_pulsacion() {
    let mut mundo = Mundo::nuevo(rgba(), 0, 0);
    assert_eq!(mundo.session.choice, ColorChoice::Slot(1));
    for ranura in 2..=9u8 {
        pulsa_y_comprueba(
            &mut mundo,
            AppAction::ColorNext,
            ColorChoice::Slot(ranura),
            "RGBA hacia delante",
        );
    }
    for ranura in (1..=8u8).rev() {
        pulsa_y_comprueba(
            &mut mundo,
            AppAction::ColorPrev,
            ColorChoice::Slot(ranura),
            "RGBA hacia atrás",
        );
    }
}

#[test]
fn fila_7_gray_y_gray_alpha_tambien_dan_la_vuelta_en_las_nueve_ranuras() {
    for (nombre, imagen) in [("Gray", gris()), ("GrayAlpha", gris_alfa())] {
        let mut mundo = Mundo::nuevo(imagen, 2, 1);
        mundo.session.set_choice_revealing(ColorChoice::Slot(9));
        pulsa_y_comprueba(
            &mut mundo,
            AppAction::ColorNext,
            ColorChoice::Slot(1),
            nombre,
        );
        pulsa_y_comprueba(
            &mut mundo,
            AppAction::ColorPrev,
            ColorChoice::Slot(9),
            nombre,
        );
        pulsa_y_comprueba(
            &mut mundo,
            AppAction::ColorNext,
            ColorChoice::Slot(1),
            nombre,
        );
        pulsa_y_comprueba(
            &mut mundo,
            AppAction::ColorNext,
            ColorChoice::Slot(2),
            nombre,
        );
    }
}

#[test]
fn fila_7_elegir_color_no_cambia_el_documento_ni_la_historia() {
    for image in [indexada(9), rgba(), gris(), gris_alfa()] {
        let mut mundo = Mundo::nuevo(image, 3, 1);
        let antes = mundo.documento();
        for _ in 0..12 {
            mundo.despacha(AppAction::ColorNext, Origin::Key);
            mundo.despacha(AppAction::ColorPrev, Origin::Key);
            mundo.despacha(AppAction::ColorPrev, Origin::Key);
        }
        assert_eq!(mundo.documento(), antes, "{:?}", mundo.imagen().mode());
    }
}

#[test]
fn fila_7_con_una_sola_entrada_color_next_y_color_prev_se_quedan_donde_estan() {
    let mut mundo = Mundo::nuevo(indexada(1), 0, 0);
    let documento = mundo.documento();
    for action in [AppAction::ColorNext, AppAction::ColorPrev] {
        assert_eq!(
            mundo.despacha(action, Origin::Key),
            hecho(false),
            "{action:?}"
        );
        assert_eq!(mundo.session.choice, ColorChoice::Entry(0), "{action:?}");
    }
    assert_eq!(mundo.documento(), documento);
}

#[test]
fn fila_7_con_una_eleccion_que_ya_no_vale_da_la_eleccion_inicial() {
    // Indexada con una ranura sin entrada (`Slot(n)` sobre una indexada solo vale si hay entrada
    // `n − 1`), indexada con una entrada fuera de la paleta, y los otros modos con una entrada
    // (no vale): `initial_choice`.
    let casos: Vec<(&str, Image, ColorChoice)> = vec![
        (
            "indexada con una ranura sin entrada",
            indexada(4),
            ColorChoice::Slot(9),
        ),
        (
            "indexada con entrada fuera",
            indexada(4),
            ColorChoice::Entry(200),
        ),
        ("RGBA con Entry", rgba(), ColorChoice::Entry(3)),
        ("Gray con Entry", gris(), ColorChoice::Entry(0)),
        ("GrayAlpha con Entry", gris_alfa(), ColorChoice::Entry(1)),
    ];
    for (nombre, image, invalida) in casos {
        for action in [AppAction::ColorNext, AppAction::ColorPrev] {
            let mut mundo = Mundo::nuevo(image.clone(), 0, 0);
            let inicial = initial_choice(mundo.imagen());
            mundo.session.choice = invalida;
            let documento = mundo.documento();
            assert_eq!(
                mundo.despacha(action, Origin::Key),
                hecho(false),
                "{nombre}: {action:?}"
            );
            assert_eq!(mundo.session.choice, inicial, "{nombre}: {action:?}");
            assert_eq!(mundo.documento(), documento, "{nombre}: {action:?}");
        }
    }
}

#[test]
fn fila_7_el_menu_y_el_boton_dan_el_mismo_resultado_que_la_tecla_con_origenes_distintos() {
    for origin in ORIGENES {
        let mut mundo = Mundo::nuevo(indexada(9), 0, 0);
        assert_eq!(
            mundo.despacha(AppAction::ColorNext, origin),
            hecho(false),
            "{origin:?}"
        );
        assert_eq!(mundo.session.choice, ColorChoice::Entry(1), "{origin:?}");
    }
}

// ---------------------------------------------------------------------------------------------
// Fila 8: bloqueos con Origin::Key
// ---------------------------------------------------------------------------------------------

#[test]
fn fila_8_color_next_y_color_prev_se_bloquean_con_cada_bloqueo_y_origen_como_select_color() {
    for action in [AppAction::ColorNext, AppAction::ColorPrev] {
        for bits in 0..32u8 {
            let locks = bloqueos(bits);
            for origin in ORIGENES {
                let mut mundo = Mundo::estandar();
                let antes = mundo.foto();
                let resultado = mundo.despacha_con(action, origin, locks);
                // Las mismas reglas que `SelectColor`: cualquier bloqueo (gesto, campo de texto,
                // quita pendiente, diálogo y modal) la para.
                let bloqueada = bits != 0;
                assert_eq!(
                    resultado == Dispatched::Blocked,
                    bloqueada,
                    "{action:?} con {locks:?} y {origin:?}: {resultado:?}"
                );
                assert_eq!(
                    is_blocked(action, locks, origin),
                    bloqueada,
                    "{action:?} con {locks:?} y {origin:?}"
                );
                assert_eq!(
                    is_blocked(action, locks, origin),
                    is_blocked(AppAction::SelectColor(3), locks, origin),
                    "{action:?} se bloquea como SelectColor con {locks:?} y {origin:?}"
                );
                if bloqueada {
                    assert_eq!(mundo.foto(), antes, "{action:?} con {locks:?}");
                } else {
                    assert_eq!(resultado, hecho(false));
                    assert_ne!(mundo.session.choice, antes.choice);
                }
            }
        }
    }
}

/// Antes de I.2a (decisión 8 de I.1) el color se bloqueaba con cualquier gesto; ahora un trazo de
/// una herramienta no lo bloquea (el trazo conserva el color con el que empezó) y el deslizador del
/// editor y el arrastre de una muestra siguen bloqueándolo.
#[test]
fn fila_8_color_next_y_color_prev_con_un_trazo_en_curso_siguen_permitidos() {
    for action in [AppAction::ColorNext, AppAction::ColorPrev] {
        let mut mundo = mundo_con_gesto(Gesto::Trazo);
        let antes = mundo.foto();
        assert_eq!(
            mundo.despacha(action, Origin::Key),
            hecho(false),
            "{action:?}"
        );
        assert_ne!(mundo.session.choice, antes.choice, "{action:?}");
        assert!(mundo.tools.busy(), "el trazo sigue en curso");
    }
}

#[test]
fn fila_8_color_next_y_color_prev_no_hacen_nada_con_el_deslizador_o_una_muestra_arrastrandose() {
    for gesto in [Gesto::Deslizador, Gesto::Muestra] {
        for action in [AppAction::ColorNext, AppAction::ColorPrev] {
            let mut mundo = mundo_con_gesto(gesto);
            let antes = mundo.foto();
            assert_eq!(
                mundo.despacha(action, Origin::Key),
                Dispatched::Blocked,
                "{gesto:?}: {action:?}"
            );
            assert_eq!(mundo.foto(), antes, "{gesto:?}: {action:?}");
        }
    }
}

#[test]
fn fila_8_color_next_y_color_prev_se_bloquean_con_una_quita_pendiente() {
    for action in [AppAction::ColorNext, AppAction::ColorPrev] {
        let mut mundo = mundo_con_quita_pendiente();
        let antes = mundo.foto();
        assert!(mundo.locks().removal);
        assert_eq!(
            mundo.despacha(action, Origin::Key),
            Dispatched::Blocked,
            "{action:?}"
        );
        assert_eq!(mundo.foto(), antes, "{action:?}");
    }
}

#[test]
fn fila_8_center_on_cursor_solo_se_bloquea_con_dialogo_modal_y_con_tecla_campo_enfocado() {
    for bits in 0..32u8 {
        let locks = bloqueos(bits);
        for origin in ORIGENES {
            let mut mundo = Mundo::estandar();
            let antes = mundo.foto();
            let resultado = mundo.despacha_con(AppAction::CenterOnCursor, origin, locks);
            let bloqueada =
                locks.dialog || locks.modal || (locks.text_focus && origin == Origin::Key);
            assert_eq!(
                resultado == Dispatched::Blocked,
                bloqueada,
                "CenterOnCursor con {locks:?} y {origin:?}: {resultado:?}"
            );
            assert_eq!(
                is_blocked(AppAction::CenterOnCursor, locks, origin),
                bloqueada,
                "CenterOnCursor con {locks:?} y {origin:?}"
            );
            if !bloqueada {
                assert_eq!(resultado, Dispatched::View(ViewOp::CenterOnCursor));
            }
            // Es una acción de vista: no toca el documento ni la elección ni la herramienta.
            assert_eq!(mundo.foto(), antes, "CenterOnCursor con {locks:?}");
        }
    }
}

#[test]
fn fila_8_center_on_cursor_no_la_paran_un_gesto_de_verdad_ni_una_quita_pendiente() {
    for gesto in GESTOS {
        let mut mundo = mundo_con_gesto(gesto);
        let antes = mundo.foto();
        assert_eq!(
            mundo.despacha(AppAction::CenterOnCursor, Origin::Key),
            Dispatched::View(ViewOp::CenterOnCursor),
            "{gesto:?}"
        );
        assert_eq!(mundo.foto(), antes, "{gesto:?}");
    }
    let mut mundo = mundo_con_quita_pendiente();
    let antes = mundo.foto();
    assert_eq!(
        mundo.despacha(AppAction::CenterOnCursor, Origin::Key),
        Dispatched::View(ViewOp::CenterOnCursor)
    );
    assert_eq!(mundo.foto(), antes);
}

#[test]
fn fila_8_is_view_es_el_zoom_mas_centrar_y_is_zoom_no_cambia() {
    assert!(AppAction::CenterOnCursor.is_view());
    for action in [
        AppAction::ZoomIn,
        AppAction::ZoomOut,
        AppAction::ZoomFit,
        AppAction::ZoomActual,
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
fn fila_8_enabled_es_lo_contrario_de_blocked_con_origen_menu_en_las_acciones_nuevas() {
    for action in [
        AppAction::ColorNext,
        AppAction::ColorPrev,
        AppAction::CenterOnCursor,
    ] {
        for bits in 0..32u8 {
            let locks = bloqueos(bits);
            let mundo = Mundo::estandar();
            assert_eq!(
                enabled(action, &mundo.estado_de_la_ui(locks)),
                !is_blocked(action, locks, Origin::Menu),
                "{action:?} con {locks:?}"
            );
        }
    }
}

/// Todas las filas activas de la tabla por defecto, con la plataforma donde valen.
fn filas_activas() -> Vec<(AppAction, Platform)> {
    let km = Keymap::default_scheme();
    let mut v = Vec::new();
    for platform in PLATAFORMAS {
        for fila in km.rows() {
            let p = px_ui::keymap::Press {
                key: Some(fila.key),
                command: fila.command,
                shift: fila.shift,
                alt: false,
                other_ctrl: false,
                repeat: false,
            };
            if km.action_for(&p, platform) == Some(fila.action) {
                v.push((fila.action, platform));
            }
        }
    }
    v
}

#[test]
fn fila_8_con_un_dialogo_de_archivo_toda_fila_de_la_tabla_da_blocked_tambien_inicio_re_pag_y_av_pag()
 {
    let filas = filas_activas();
    assert_eq!(
        filas.len(),
        35 + 34 + 34,
        "filas activas en las tres plataformas"
    );
    for (action, platform) in filas {
        let mut mundo = Mundo::estandar();
        let antes = mundo.foto();
        let locks = locks_con(|l| l.dialog = true);
        assert_eq!(
            mundo.despacha_con(action, Origin::Key, locks),
            Dispatched::Blocked,
            "{action:?} en {platform:?} con un diálogo de archivo"
        );
        assert_eq!(mundo.foto(), antes, "{action:?} en {platform:?}");
    }
}

#[test]
fn fila_8_con_el_panel_modal_toda_fila_da_blocked_salvo_salir() {
    for (action, platform) in filas_activas() {
        let mut mundo = Mundo::estandar();
        let antes = mundo.foto();
        let locks = locks_con(|l| l.modal = true);
        let resultado = mundo.despacha_con(action, Origin::Key, locks);
        if action == AppAction::Quit {
            // `Salir` pasa con el panel modal (unidad 18, D1).
            assert_eq!(resultado, Dispatched::Quit, "{platform:?}");
        } else {
            assert_eq!(
                resultado,
                Dispatched::Blocked,
                "{action:?} en {platform:?} con el panel modal"
            );
        }
        assert_eq!(mundo.foto(), antes, "{action:?} en {platform:?}");
    }
}

#[test]
fn fila_8_con_modal_las_teclas_re_pag_av_pag_e_inicio_dan_blocked_y_fin_no_da_accion() {
    let locks = locks_con(|l| l.modal = true);
    for (key, esperada) in [
        (Key::PageUp, Some(AppAction::ColorPrev)),
        (Key::PageDown, Some(AppAction::ColorNext)),
        (Key::Home, Some(AppAction::ZoomFit)),
        (Key::End, None),
    ] {
        for platform in PLATAFORMAS {
            let acciones = acciones_de_teclado(&[tecla(key, Modifiers::NONE)], platform);
            assert_eq!(
                acciones,
                esperada.into_iter().collect::<Vec<_>>(),
                "{key:?}"
            );
            for action in acciones {
                let mut mundo = Mundo::estandar();
                let antes = mundo.foto();
                assert_eq!(
                    mundo.despacha_con(action, Origin::Key, locks),
                    Dispatched::Blocked,
                    "{key:?} en {platform:?}"
                );
                assert_eq!(mundo.foto(), antes);
            }
        }
    }
}

// ---------------------------------------------------------------------------------------------
// Fila 9: la misma acción, el mismo efecto
// ---------------------------------------------------------------------------------------------

fn por_rotulo_de_menu(rotulo: &str) -> AppAction {
    MENUS
        .iter()
        .flat_map(|m| m.items.iter())
        .find(|i| i.label == rotulo)
        .unwrap_or_else(|| panic!("no hay entrada de menú {rotulo:?}"))
        .action
}

fn por_rotulo_de_boton(rotulo: &str) -> AppAction {
    TOOL_BUTTONS
        .iter()
        .chain(ZOOM_BUTTONS.iter())
        .find(|b| b.label == rotulo)
        .unwrap_or_else(|| panic!("no hay botón {rotulo:?}"))
        .action
}

struct Caso {
    esperada: AppAction,
    menu: Option<&'static str>,
    boton: Option<&'static str>,
    /// Cada secuencia de eventos que tiene que dar `esperada` (una tecla, o dos atajos).
    teclas: Vec<Vec<Event>>,
}

fn casos_de_la_fila_9() -> Vec<Caso> {
    let s = Modifiers::SHIFT;
    vec![
        Caso {
            esperada: AppAction::Undo,
            menu: Some("Deshacer"),
            boton: None,
            teclas: vec![
                vec![tecla(Key::Z, CTRL)],
                escribe(Key::U, Modifiers::NONE, "u"),
            ],
        },
        Caso {
            esperada: AppAction::SelectTool(Tool::Pencil),
            menu: None,
            boton: Some("Lápiz"),
            teclas: vec![escribe(Key::D, Modifiers::NONE, "d")],
        },
        Caso {
            esperada: AppAction::SelectTool(Tool::Line),
            menu: None,
            boton: Some("Línea"),
            teclas: vec![escribe(Key::V, Modifiers::NONE, "v")],
        },
        Caso {
            esperada: AppAction::SelectTool(Tool::Fill),
            menu: None,
            boton: Some("Relleno"),
            teclas: vec![escribe(Key::F, Modifiers::NONE, "f")],
        },
        Caso {
            esperada: AppAction::SelectTool(Tool::Dropper),
            menu: None,
            boton: Some("Cuentagotas"),
            teclas: vec![escribe(Key::Comma, Modifiers::NONE, ",")],
        },
        Caso {
            esperada: AppAction::ZoomIn,
            menu: Some("Acercar"),
            boton: Some("Acercar"),
            teclas: vec![
                escribe(Key::IntlBackslash, s, ">"),
                escribe(Key::Plus, s, "+"),
                escribe(Key::Equals, Modifiers::NONE, "="),
            ],
        },
        Caso {
            esperada: AppAction::ZoomOut,
            menu: Some("Alejar"),
            boton: Some("Alejar"),
            teclas: vec![
                escribe(Key::IntlBackslash, Modifiers::NONE, "<"),
                escribe(Key::Minus, Modifiers::NONE, "-"),
            ],
        },
        Caso {
            esperada: AppAction::ColorNext,
            menu: None,
            boton: None,
            teclas: vec![
                escribe(Key::CloseBracket, Modifiers::NONE, "]"),
                vec![tecla(Key::PageDown, Modifiers::NONE)],
            ],
        },
        Caso {
            esperada: AppAction::ColorPrev,
            menu: None,
            boton: None,
            teclas: vec![
                escribe(Key::OpenBracket, Modifiers::ALT, "["),
                vec![tecla(Key::PageUp, Modifiers::NONE)],
            ],
        },
        Caso {
            esperada: AppAction::SelectTool(Tool::Eraser),
            menu: None,
            boton: Some("Borrador"),
            teclas: vec![escribe(Key::K, Modifiers::NONE, "k")],
        },
    ]
}

#[test]
fn fila_9_menu_boton_y_tecla_dan_la_misma_accion() {
    for caso in casos_de_la_fila_9() {
        let mut obtenidas = Vec::new();
        if let Some(rotulo) = caso.menu {
            obtenidas.push(("menú", por_rotulo_de_menu(rotulo)));
        }
        if let Some(rotulo) = caso.boton {
            obtenidas.push(("botón", por_rotulo_de_boton(rotulo)));
        }
        for events in &caso.teclas {
            obtenidas.push(("tecla", una_accion(events, Platform::Linux)));
        }
        assert!(!obtenidas.is_empty(), "{:?}", caso.esperada);
        for (origen, action) in obtenidas {
            assert_eq!(action, caso.esperada, "{:?} por el {origen}", caso.esperada);
        }
    }
}

#[test]
fn fila_9_las_teclas_de_cada_caso_dan_lo_mismo_en_las_tres_plataformas() {
    for caso in casos_de_la_fila_9() {
        for events in &caso.teclas {
            for platform in PLATAFORMAS {
                assert_eq!(
                    una_accion(events, platform),
                    caso.esperada,
                    "{:?} en {platform:?}",
                    caso.esperada
                );
            }
        }
    }
}

#[test]
fn fila_9_los_dispatched_y_el_estado_final_son_iguales_desde_el_menu_el_boton_y_la_tecla() {
    for caso in casos_de_la_fila_9() {
        let mut fuentes: Vec<(String, Origin, AppAction)> = Vec::new();
        if let Some(rotulo) = caso.menu {
            fuentes.push((
                format!("menú {rotulo}"),
                Origin::Menu,
                por_rotulo_de_menu(rotulo),
            ));
        }
        if let Some(rotulo) = caso.boton {
            fuentes.push((
                format!("botón {rotulo}"),
                Origin::Button,
                por_rotulo_de_boton(rotulo),
            ));
        }
        for (i, events) in caso.teclas.iter().enumerate() {
            fuentes.push((
                format!("tecla {i}"),
                Origin::Key,
                una_accion(events, Platform::Linux),
            ));
        }
        let resultados: Vec<(String, Dispatched, Foto)> = fuentes
            .iter()
            .map(|(nombre, origin, action)| {
                let mut mundo = Mundo::estandar();
                let resultado = mundo.despacha(*action, *origin);
                (nombre.clone(), resultado, mundo.foto())
            })
            .collect();
        for pareja in resultados.windows(2) {
            assert_eq!(
                pareja[0].1, pareja[1].1,
                "{:?}: {} y {} difieren",
                caso.esperada, pareja[0].0, pareja[1].0
            );
            assert_eq!(
                pareja[0].2, pareja[1].2,
                "{:?}: el estado tras {} y tras {} difiere",
                caso.esperada, pareja[0].0, pareja[1].0
            );
        }

        // Y lo que cada una hace.
        let antes = Mundo::estandar().foto();
        let (_, resultado, despues) = &resultados[0];
        match caso.esperada {
            AppAction::Undo => {
                assert_eq!(*resultado, hecho(true));
                assert_ne!(despues.state_id, antes.state_id);
                assert!(despues.can_redo);
            }
            AppAction::SelectTool(Tool::Eraser) => {
                // Indexada sin transparencia: con aviso.
                assert_eq!(
                    *resultado,
                    Dispatched::Done {
                        refresh: false,
                        notice: Some(NO_INDEX_NOTICE.to_string())
                    }
                );
                assert_eq!(despues.tool, Tool::Eraser);
            }
            AppAction::SelectTool(tool) => {
                assert_eq!(*resultado, hecho(false));
                assert_eq!(despues.tool, tool);
                assert_eq!(despues.state_id, antes.state_id);
            }
            AppAction::ZoomIn => assert_eq!(*resultado, Dispatched::View(ViewOp::ZoomIn)),
            AppAction::ZoomOut => assert_eq!(*resultado, Dispatched::View(ViewOp::ZoomOut)),
            AppAction::ColorNext => {
                assert_eq!(*resultado, hecho(false));
                assert_eq!(despues.choice, ColorChoice::Entry(1));
            }
            AppAction::ColorPrev => {
                assert_eq!(*resultado, hecho(false));
                // Da la vuelta desde la primera entrada.
                assert_eq!(despues.choice, ColorChoice::Entry(3));
            }
            otra => panic!("caso sin comprobar: {otra:?}"),
        }
    }
}

#[test]
fn fila_9_el_aviso_del_borrador_sale_igual_desde_el_boton() {
    let mut a = Mundo::estandar();
    let mut b = Mundo::estandar();
    let desde_boton = a.despacha(por_rotulo_de_boton("Borrador"), Origin::Button);
    let directa = b.despacha(AppAction::SelectTool(Tool::Eraser), Origin::Key);
    assert_eq!(desde_boton, directa);
    assert_eq!(
        desde_boton,
        Dispatched::Done {
            refresh: false,
            notice: Some(NO_INDEX_NOTICE.to_string())
        }
    );
    assert_eq!(a.foto(), b.foto());
    // El Borrador tiene la `k` (la `e` no es nada, ni sola ni con `Mayús`).
    for letra in ["e", "E"] {
        assert_eq!(
            acciones_de_teclado(&escribe(Key::E, Modifiers::NONE, letra), Platform::Linux),
            vec![],
            "{letra}"
        );
    }
}

#[test]
fn borrador_k_elige_el_borrador_con_o_sin_bloq_mayus_y_k_con_mayus_borra_la_pagina() {
    for platform in PLATAFORMAS {
        // `k` (y la `K` que escribe el Bloq Mayús, sin `Mayús`): el Borrador.
        for texto in ["k", "K"] {
            assert_eq!(
                una_accion(&escribe(Key::K, Modifiers::NONE, texto), platform),
                AppAction::SelectTool(Tool::Eraser),
                "{texto} en {platform:?}"
            );
        }
        // `Mayús`+`k`: Borrar página, no el Borrador.
        assert_eq!(
            una_accion(&escribe(Key::K, Modifiers::SHIFT, "K"), platform),
            AppAction::ClearPage
        );
    }
    let mut mundo = Mundo::estandar();
    mundo.despacha(AppAction::SelectTool(Tool::Eraser), Origin::Key);
    assert_eq!(mundo.tools.tool(), Tool::Eraser);
    let mut mundo = Mundo::estandar();
    let antes = mundo.foto();
    mundo.despacha(AppAction::ClearPage, Origin::Key);
    assert_eq!(mundo.tools.tool(), antes.tool);
}

#[test]
fn borrador_k_no_actua_con_un_campo_enfocado_un_dialogo_abierto_o_un_trazo_en_curso() {
    let k = escribe(Key::K, Modifiers::NONE, "k");
    // Campo de texto enfocado: el filtro de teclas no deja pasar la `k`.
    assert_eq!(
        acciones_de_teclado(&px_ui::shortcuts::gate_keys(&k, true), Platform::Linux),
        vec![]
    );
    // Diálogo abierto o campo enfocado: la acción llega bloqueada y no cambia nada.
    for locks in [
        locks_con(|l| l.dialog = true),
        locks_con(|l| l.text_focus = true),
    ] {
        let mut mundo = Mundo::estandar();
        let antes = mundo.foto();
        assert_eq!(
            mundo.despacha_con(AppAction::SelectTool(Tool::Eraser), Origin::Key, locks),
            Dispatched::Blocked
        );
        assert_eq!(mundo.foto(), antes);
    }
    // Trazo en curso: bloqueada también.
    let mut mundo = mundo_con_gesto(Gesto::Trazo);
    let antes = mundo.foto();
    assert_eq!(
        mundo.despacha(AppAction::SelectTool(Tool::Eraser), Origin::Key),
        Dispatched::Blocked
    );
    assert_eq!(mundo.foto(), antes);
    assert_ne!(mundo.tools.tool(), Tool::Eraser);
}

// ---------------------------------------------------------------------------------------------
// Fila 10 (parte de dispatch): SelectColor sin tecla
// ---------------------------------------------------------------------------------------------

#[test]
fn fila_10_select_color_3_directo_en_una_indexada_de_9_cambia_choice_con_la_rejilla_sincronizada() {
    let mut mundo = Mundo::nuevo(indexada(9), 0, 0);
    let documento = mundo.documento();
    assert_eq!(
        mundo.despacha(AppAction::SelectColor(3), Origin::Key),
        hecho(false)
    );
    assert_eq!(mundo.session.choice, ColorChoice::Entry(2));
    assert!(
        mundo.session.view.reveal,
        "la rejilla muestra la fila elegida"
    );
    assert_eq!(mundo.documento(), documento);
}

#[test]
fn fila_10_select_color_3_directo_en_rgba_elige_la_ranura_3() {
    let mut mundo = Mundo::nuevo(rgba(), 0, 0);
    assert_eq!(
        mundo.despacha(AppAction::SelectColor(3), Origin::Key),
        hecho(false)
    );
    assert_eq!(mundo.session.choice, ColorChoice::Slot(3));
}

#[test]
fn fila_10_select_color_se_sigue_bloqueando_como_antes() {
    let mut mundo = mundo_con_quita_pendiente();
    let antes = mundo.foto();
    assert_eq!(
        mundo.despacha(AppAction::SelectColor(1), Origin::Key),
        Dispatched::Blocked
    );
    assert_eq!(mundo.foto(), antes);
}

#[test]
fn fila_10_las_cifras_por_teclado_no_dan_ninguna_accion_que_despachar() {
    for platform in PLATAFORMAS {
        for (key, cifra) in [
            (Key::Num1, "1"),
            (Key::Num2, "2"),
            (Key::Num3, "3"),
            (Key::Num9, "9"),
        ] {
            assert_eq!(
                acciones_de_teclado(&escribe(key, Modifiers::NONE, cifra), platform),
                vec![],
                "{cifra} en {platform:?}"
            );
        }
    }
    // `Ctrl+1` sigue dando solo `ZoomActual`.
    assert_eq!(
        acciones_de_teclado(&[tecla(Key::Num1, CTRL)], Platform::Linux),
        vec![AppAction::ZoomActual]
    );
}

// ---------------------------------------------------------------------------------------------
// Fila 12: repetición
// ---------------------------------------------------------------------------------------------

#[test]
fn fila_12_ignores_repeat_es_solo_open_save_save_as_new_image_y_quit_en_toda_la_tabla() {
    let km = Keymap::default_scheme();
    for fila in km.rows() {
        let esperado = matches!(
            fila.action,
            AppAction::Open
                | AppAction::Save
                | AppAction::SaveAs
                | AppAction::NewImage
                | AppAction::Quit
        );
        assert_eq!(
            fila.action.ignores_repeat(),
            esperado,
            "{:?} ({:?})",
            fila.action,
            fila.key
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
fn fila_12_u_repetida_deshace_un_paso_por_pulsacion() {
    let mut mundo = Mundo::nuevo(indexada(4), 4, 0);
    let events = [
        tecla(Key::U, Modifiers::NONE),
        texto("u"),
        repetida(Key::U, Modifiers::NONE),
        texto("u"),
        repetida(Key::U, Modifiers::NONE),
        texto("u"),
    ];
    let acciones = acciones_de_teclado(&events, Platform::Linux);
    assert_eq!(acciones, vec![AppAction::Undo; 3]);
    let mut ids = vec![mundo.session.document.state_id()];
    for action in acciones {
        assert_eq!(mundo.despacha(action, Origin::Key), hecho(true));
        ids.push(mundo.session.document.state_id());
    }
    // Tres pasos deshechos: queda uno por deshacer y tres por rehacer.
    assert!(mundo.session.document.can_undo());
    assert!(mundo.session.document.can_redo());
    ids.dedup();
    assert_eq!(ids.len(), 4, "cada pulsación cambió el estado: {ids:?}");
    // Una cuarta pulsación agota la historia.
    mundo.despacha(AppAction::Undo, Origin::Key);
    assert!(!mundo.session.document.can_undo());
}

#[test]
fn fila_12_corchete_y_av_pag_repetidos_dan_un_paso_de_color_por_pulsacion() {
    let casos: Vec<(&str, Vec<Event>)> = vec![
        (
            "]",
            vec![
                tecla(Key::CloseBracket, Modifiers::NONE),
                texto("]"),
                repetida(Key::CloseBracket, Modifiers::NONE),
                texto("]"),
                repetida(Key::CloseBracket, Modifiers::NONE),
                texto("]"),
                repetida(Key::CloseBracket, Modifiers::NONE),
                texto("]"),
            ],
        ),
        (
            "Av Pág",
            vec![
                tecla(Key::PageDown, Modifiers::NONE),
                repetida(Key::PageDown, Modifiers::NONE),
                repetida(Key::PageDown, Modifiers::NONE),
                repetida(Key::PageDown, Modifiers::NONE),
            ],
        ),
    ];
    for (nombre, events) in casos {
        let mut mundo = Mundo::nuevo(indexada(9), 0, 0);
        let acciones = acciones_de_teclado(&events, Platform::Linux);
        assert_eq!(acciones, vec![AppAction::ColorNext; 4], "{nombre}");
        for action in acciones {
            mundo.despacha(action, Origin::Key);
        }
        assert_eq!(
            mundo.session.choice,
            ColorChoice::Entry(4),
            "{nombre}: cuatro pulsaciones, cuatro pasos"
        );
    }
}

#[test]
fn fila_12_menor_repetido_aleja_una_vez_por_pulsacion() {
    let events = [
        tecla(Key::IntlBackslash, Modifiers::NONE),
        texto("<"),
        repetida(Key::IntlBackslash, Modifiers::NONE),
        texto("<"),
        repetida(Key::IntlBackslash, Modifiers::NONE),
        texto("<"),
    ];
    let mut mundo = Mundo::estandar();
    let resultados: Vec<Dispatched> = acciones_de_teclado(&events, Platform::Linux)
        .into_iter()
        .map(|a| mundo.despacha(a, Origin::Key))
        .collect();
    assert_eq!(resultados, vec![Dispatched::View(ViewOp::ZoomOut); 3]);
}

#[test]
fn fila_12_abrir_guardar_guardar_como_nueva_imagen_y_salir_mantenidos_no_repiten() {
    for (key, modifiers, esperada, resultado) in [
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
            tecla(key, modifiers),
            repetida(key, modifiers),
            repetida(key, modifiers),
            repetida(key, modifiers),
        ];
        let acciones = acciones_de_teclado(&events, Platform::Linux);
        assert_eq!(
            acciones,
            vec![esperada],
            "{esperada:?}: solo la primera pulsación"
        );
        let mut mundo = Mundo::estandar();
        assert_eq!(mundo.despacha(acciones[0], Origin::Key), resultado);
    }
}
