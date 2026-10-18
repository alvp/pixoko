//! Tests de `dispatch` (specs/hito-2-menu-herramientas.md, «Casos» filas 3b en la app, 4, 5, 5b y
//! 12 y propiedad 2). Sin egui más allá de `Key` y `Modifiers`: `dispatch` recibe la acción, su
//! origen y `Targets { session, tools, locks }` y no toca ninguna ventana.
//!
//! - Fila 3b: las nueve ranuras de una indexada de 9 entradas con `action_for_shortcut` +
//!   `dispatch` (lo que hará `handle_keys` traducido) cambian `choice` como antes, con la
//!   sincronización de la rejilla; `command`+`1` es solo `ZoomActual` y no cambia `choice`.
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

use super::{Dispatched, Targets, dispatch};
use crate::session::Session;

// ---------------------------------------------------------------------------------------------
// Ayudantes
// ---------------------------------------------------------------------------------------------

const HERRAMIENTAS: [Tool; 5] = [
    Tool::Pencil,
    Tool::Line,
    Tool::Fill,
    Tool::Dropper,
    Tool::Eraser,
];
const ORIGENES: [Origin; 3] = [Origin::Menu, Origin::Button, Origin::Key];
const ZOOMS: [(AppAction, ViewOp); 4] = [
    (AppAction::ZoomIn, ViewOp::ZoomIn),
    (AppAction::ZoomOut, ViewOp::ZoomOut),
    (AppAction::ZoomFit, ViewOp::Fit),
    (AppAction::ZoomActual, ViewOp::Actual),
];

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

/// Indexada de 4×4 con `n` entradas distintas y, si se pide, ese índice transparente.
fn indexada(n: usize, transparente: Option<u8>) -> Image {
    let mut paleta = Palette::from_colors(distintos(n)).expect("paleta");
    paleta
        .set_transparent_index(transparente)
        .expect("índice transparente");
    Image::new_indexed(4, 4, paleta).expect("imagen indexada")
}

/// Un píxel distinto del de fondo, del modo de `image`, según `i`.
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

/// Un paso de historia: escribe un píxel de una fila distinta cada vez (hasta 4 pasos por
/// columna 0 y otros 4 por la columna 1).
fn un_paso(document: &mut Document, i: usize) {
    let id = document.main_image();
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

/// Lo que `dispatch` no debe tocar cuando bloquea, y lo que se compara entre orígenes.
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
        // Lo que `undo` deja pendiente de avisar no es de estos tests.
        let _ = session.take_removal_cancelled();
        session.view.reveal = false;
        Mundo {
            session,
            tools: ToolState::new(),
        }
    }

    /// Indexada de 4 entradas sin transparente, con 2 pasos y 1 deshecho: se puede deshacer y
    /// rehacer, y el borrador avisa.
    fn estandar() -> Mundo {
        Mundo::nuevo(indexada(4, None), 2, 1)
    }

    fn imagen(&self) -> &Image {
        self.session
            .document
            .image(self.session.document.main_image())
            .expect("imagen")
    }

    /// Los bloqueos que sacaría `main` de este estado (sin los que no son de aquí).
    fn locks(&self) -> Locks {
        Locks {
            gesture: self.tools.busy() || self.session.busy(),
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

    fn estado_de_la_ui(&self, zoom: Option<Zoom>, locks: Locks) -> UiState {
        UiState {
            can_undo: self.session.document.can_undo(),
            can_redo: self.session.document.can_redo(),
            zoom,
            tool: self.tools.tool(),
            locks,
        }
    }
}

fn locks_con(f: impl FnOnce(&mut Locks)) -> Locks {
    let mut locks = Locks::default();
    f(&mut locks);
    locks
}

fn hecho(refresh: bool) -> Dispatched {
    Dispatched::Done {
        refresh,
        notice: None,
    }
}

// --- Gestos de verdad ---

/// Empieza un trazo del lápiz sin soltar (el documento no cambia).
fn empieza_un_trazo(mundo: &mut Mundo) {
    let id = mundo.session.document.main_image();
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

/// Deja pulsado el deslizador rojo del editor.
fn pulsa_el_deslizador(mundo: &mut Mundo) {
    mundo.session.sync_editor();
    assert!(
        mundo.session.editor.begin(Control::Red, 10),
        "el editor admite el arrastre"
    );
    assert!(mundo.session.busy());
}

/// Arrastra de verdad una muestra de la paleta.
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
    let colores = distintos(5);
    let paleta = Palette::from_colors(colores).expect("paleta");
    let mut image = Image::new_indexed(5, 1, paleta).expect("imagen indexada");
    for (x, i) in [0u8, 1, 2, 3, 3].into_iter().enumerate() {
        image
            .set_pixel(x as u32, 0, Pixel::Indexed(i))
            .expect("píxel");
    }
    let mut document = Document::new(image);
    let id = document.main_image();
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
    let destino = {
        let image = session
            .document
            .image(session.document.main_image())
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

fn todas_las_acciones() -> Vec<AppAction> {
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
        AppAction::ZoomActual,
        AppAction::ShowLicenses,
        AppAction::CloseLicenses,
    ];
    v.extend(HERRAMIENTAS.map(AppAction::SelectTool));
    v.extend((1..=9).map(AppAction::SelectColor));
    v
}

fn no_es_zoom(action: &AppAction) -> bool {
    !action.is_zoom()
}

fn tecla_digito(slot: u8) -> Key {
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

const PLATAFORMAS: [Platform; 3] = [Platform::Linux, Platform::Windows, Platform::Mac];

// ---------------------------------------------------------------------------------------------
// Fila 3b: las teclas 1 a 9 siguen eligiendo color (en la app)
// ---------------------------------------------------------------------------------------------

#[test]
fn fila_3b_las_nueve_ranuras_de_una_indexada_de_9_entradas_cambian_choice_como_antes() {
    for platform in PLATAFORMAS {
        for slot in 1..=9u8 {
            let mut mundo = Mundo::nuevo(indexada(9, None), 0, 0);
            assert_eq!(mundo.session.choice, ColorChoice::Entry(0));
            // `handle_keys` traducido: la tecla da la acción y la acción se despacha.
            let action = action_for_shortcut(tecla_digito(slot), Modifiers::NONE, platform)
                .expect("los dígitos sin modificadores dan una acción");
            assert_eq!(action, AppAction::SelectColor(slot));
            let resultado = mundo.despacha(action, Origin::Key);
            assert_eq!(resultado, hecho(false), "{platform:?} ranura {slot}");
            assert_eq!(
                mundo.session.choice,
                ColorChoice::Entry(slot - 1),
                "{platform:?} ranura {slot}"
            );
            // Con la sincronización de la rejilla: si cambió, el panel muestra su fila.
            assert_eq!(
                mundo.session.view.reveal,
                slot != 1,
                "{platform:?} ranura {slot}: reveal"
            );
        }
    }
}

#[test]
fn fila_3b_en_rgba_las_ranuras_eligen_la_ranura_de_color() {
    for slot in 1..=9u8 {
        let mut mundo = Mundo::nuevo(rgba(), 0, 0);
        let esperado = key_choice(mundo.imagen(), slot).expect("la ranura existe");
        let resultado = mundo.despacha(AppAction::SelectColor(slot), Origin::Key);
        assert_eq!(resultado, hecho(false), "ranura {slot}");
        assert_eq!(mundo.session.choice, esperado, "ranura {slot}");
    }
}

#[test]
fn fila_3b_una_ranura_sin_muestra_no_cambia_nada() {
    let mut mundo = Mundo::nuevo(indexada(4, None), 0, 0);
    let antes = mundo.foto();
    let resultado = mundo.despacha(AppAction::SelectColor(7), Origin::Key);
    assert_eq!(resultado, hecho(false));
    assert_eq!(mundo.foto(), antes);
}

#[test]
fn fila_3b_command_1_da_solo_zoom_actual_y_no_cambia_choice() {
    for platform in PLATAFORMAS {
        let mut mundo = Mundo::nuevo(indexada(9, None), 0, 0);
        mundo.session.set_choice_revealing(ColorChoice::Entry(4));
        mundo.session.view.reveal = false;
        let antes = mundo.foto();
        let action = action_for_shortcut(Key::Num1, command(platform), platform)
            .expect("command+1 da una acción");
        assert_eq!(action, AppAction::ZoomActual, "{platform:?}");
        assert!(!matches!(action, AppAction::SelectColor(_)));
        let resultado = mundo.despacha(action, Origin::Key);
        assert_eq!(resultado, Dispatched::View(ViewOp::Actual), "{platform:?}");
        assert_eq!(
            mundo.foto(),
            antes,
            "{platform:?}: no cambia choice ni nada"
        );
        assert_eq!(mundo.session.choice, ColorChoice::Entry(4));
    }
}

#[test]
fn fila_3b_num1_con_mayus_alt_o_combinados_no_da_accion_y_no_cambia_choice() {
    for platform in PLATAFORMAS {
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
    atajo: Option<(Key, Modifiers)>,
}

fn casos_de_la_fila_4() -> Vec<Caso> {
    let cmd = command(Platform::Linux);
    let cmd_mayus = Modifiers { shift: true, ..cmd };
    vec![
        Caso {
            esperada: AppAction::Undo,
            menu: Some("Deshacer"),
            boton: None,
            atajo: Some((Key::Z, cmd)),
        },
        Caso {
            esperada: AppAction::Redo,
            menu: Some("Rehacer"),
            boton: None,
            atajo: Some((Key::Y, cmd)),
        },
        Caso {
            esperada: AppAction::SelectTool(Tool::Eraser),
            menu: None,
            boton: Some("Borrador"),
            atajo: Some((Key::E, Modifiers::NONE)),
        },
        Caso {
            esperada: AppAction::SelectColor(3),
            menu: None,
            boton: None,
            atajo: Some((Key::Num3, Modifiers::NONE)),
        },
        Caso {
            esperada: AppAction::Open,
            menu: Some("Abrir…"),
            boton: None,
            atajo: Some((Key::O, cmd)),
        },
        Caso {
            esperada: AppAction::Save,
            menu: Some("Guardar"),
            boton: None,
            atajo: Some((Key::S, cmd)),
        },
        Caso {
            esperada: AppAction::SaveAs,
            menu: Some("Guardar como…"),
            boton: None,
            atajo: Some((Key::S, cmd_mayus)),
        },
        Caso {
            esperada: AppAction::Quit,
            menu: Some("Salir"),
            boton: None,
            // Solo en Linux.
            atajo: Some((Key::Q, cmd)),
        },
        Caso {
            esperada: AppAction::ZoomIn,
            menu: Some("Acercar"),
            boton: Some("Acercar"),
            atajo: Some((Key::Plus, Modifiers::NONE)),
        },
        Caso {
            esperada: AppAction::ZoomFit,
            menu: Some("Ajustar a la ventana"),
            boton: Some("Ajustar"),
            atajo: Some((Key::Home, Modifiers::NONE)),
        },
        Caso {
            esperada: AppAction::ShowLicenses,
            menu: Some("Licencias"),
            boton: None,
            atajo: None,
        },
    ]
}

#[test]
fn fila_4_menu_boton_y_atajo_dan_la_misma_accion() {
    for caso in casos_de_la_fila_4() {
        let mut obtenidas = Vec::new();
        if let Some(rotulo) = caso.menu {
            obtenidas.push(("menú", por_rotulo_de_menu(rotulo)));
        }
        if let Some(rotulo) = caso.boton {
            obtenidas.push(("botón", por_rotulo_de_boton(rotulo)));
        }
        if let Some((key, modifiers)) = caso.atajo {
            let action = action_for_shortcut(key, modifiers, Platform::Linux)
                .unwrap_or_else(|| panic!("{key:?} con {modifiers:?} no da acción"));
            obtenidas.push(("atajo", action));
        }
        assert!(!obtenidas.is_empty());
        for (origen, action) in obtenidas {
            assert_eq!(action, caso.esperada, "{:?} por el {origen}", caso.esperada);
        }
    }
}

#[test]
fn fila_4_los_tres_dispatched_y_el_estado_final_son_iguales() {
    for caso in casos_de_la_fila_4() {
        // Cada origen con su acción, sobre una copia idéntica.
        let mut fuentes: Vec<(Origin, AppAction)> = Vec::new();
        if let Some(rotulo) = caso.menu {
            fuentes.push((Origin::Menu, por_rotulo_de_menu(rotulo)));
        }
        if let Some(rotulo) = caso.boton {
            fuentes.push((Origin::Button, por_rotulo_de_boton(rotulo)));
        }
        if let Some((key, modifiers)) = caso.atajo {
            let action = action_for_shortcut(key, modifiers, Platform::Linux).expect("atajo");
            fuentes.push((Origin::Key, action));
        }

        let antes = Mundo::estandar().foto();
        let resultados: Vec<(Origin, Dispatched, Foto)> = fuentes
            .iter()
            .map(|(origin, action)| {
                let mut mundo = Mundo::estandar();
                let resultado = mundo.despacha(*action, *origin);
                (*origin, resultado, mundo.foto())
            })
            .collect();
        for pareja in resultados.windows(2) {
            assert_eq!(
                pareja[0].1, pareja[1].1,
                "{:?}: {:?} y {:?} difieren",
                caso.esperada, pareja[0].0, pareja[1].0
            );
            assert_eq!(
                pareja[0].2, pareja[1].2,
                "{:?}: el estado tras {:?} y tras {:?} difiere",
                caso.esperada, pareja[0].0, pareja[1].0
            );
        }

        // Y lo que cada una hace.
        let (_, resultado, despues) = &resultados[0];
        match caso.esperada {
            AppAction::Open => {
                assert_eq!(*resultado, Dispatched::File(FileAction::Open));
                assert_eq!(*despues, antes, "Abrir no toca el documento");
            }
            AppAction::Save => {
                assert_eq!(*resultado, Dispatched::File(FileAction::Save));
                assert_eq!(*despues, antes);
            }
            AppAction::SaveAs => {
                assert_eq!(*resultado, Dispatched::File(FileAction::SaveAs));
                assert_eq!(*despues, antes);
            }
            AppAction::Quit => assert_eq!(*resultado, Dispatched::Quit),
            AppAction::ZoomIn => assert_eq!(*resultado, Dispatched::View(ViewOp::ZoomIn)),
            AppAction::ZoomFit => assert_eq!(*resultado, Dispatched::View(ViewOp::Fit)),
            AppAction::ShowLicenses => assert_eq!(*resultado, Dispatched::Licenses(true)),
            AppAction::Undo => {
                assert_eq!(*resultado, hecho(true));
                assert_ne!(despues.state_id, antes.state_id);
                assert!(despues.can_redo);
            }
            AppAction::Redo => {
                assert_eq!(*resultado, hecho(true));
                assert_ne!(despues.state_id, antes.state_id);
                assert!(!despues.can_redo);
            }
            AppAction::SelectTool(Tool::Eraser) => {
                // Indexada sin transparencia: con aviso, y es el mismo texto desde el botón y
                // desde `E`.
                assert_eq!(
                    *resultado,
                    Dispatched::Done {
                        refresh: false,
                        notice: Some(NO_INDEX_NOTICE.to_string())
                    }
                );
                assert_eq!(despues.tool, Tool::Eraser);
            }
            AppAction::SelectColor(3) => {
                assert_eq!(*resultado, hecho(false));
                assert_eq!(despues.choice, ColorChoice::Entry(2));
            }
            otra => panic!("caso sin comprobar: {otra:?}"),
        }
    }
}

#[test]
fn fila_4_el_aviso_del_borrador_es_el_mismo_desde_el_boton_y_desde_la_tecla() {
    let mut por_boton = Mundo::estandar();
    let mut por_tecla = Mundo::estandar();
    let desde_boton = por_boton.despacha(por_rotulo_de_boton("Borrador"), Origin::Button);
    let desde_tecla = por_tecla.despacha(
        action_for_shortcut(Key::E, Modifiers::NONE, Platform::Linux).expect("E"),
        Origin::Key,
    );
    assert_eq!(desde_boton, desde_tecla);
    assert!(matches!(
        desde_boton,
        Dispatched::Done {
            notice: Some(_),
            ..
        }
    ));
    // Con una imagen que admite borrar no hay aviso.
    for image in [rgba(), gris_alfa(), indexada(4, Some(3))] {
        let mut mundo = Mundo::nuevo(image, 0, 0);
        assert_eq!(
            mundo.despacha(AppAction::SelectTool(Tool::Eraser), Origin::Button),
            hecho(false)
        );
        assert_eq!(mundo.tools.tool(), Tool::Eraser);
    }
    // Y con gris, avisa de otra cosa pero avisa.
    let mut mundo = Mundo::nuevo(gris(), 0, 0);
    assert!(matches!(
        mundo.despacha(AppAction::SelectTool(Tool::Eraser), Origin::Button),
        Dispatched::Done {
            notice: Some(_),
            ..
        }
    ));
}

#[test]
fn fila_4_las_cinco_herramientas_se_eligen_igual_desde_el_boton_y_desde_su_tecla() {
    for (tool, (key, boton)) in HERRAMIENTAS.iter().zip([
        (Key::P, "Lápiz"),
        (Key::L, "Línea"),
        (Key::F, "Relleno"),
        (Key::I, "Cuentagotas"),
        (Key::E, "Borrador"),
    ]) {
        let mut a = Mundo::nuevo(rgba(), 0, 0);
        let mut b = Mundo::nuevo(rgba(), 0, 0);
        let ra = a.despacha(por_rotulo_de_boton(boton), Origin::Button);
        let rb = b.despacha(
            action_for_shortcut(key, Modifiers::NONE, Platform::Linux).expect("tecla"),
            Origin::Key,
        );
        assert_eq!(ra, rb, "{tool:?}");
        assert_eq!(a.tools.tool(), *tool);
        assert_eq!(b.tools.tool(), *tool);
    }
}

#[test]
fn fila_4_los_cuatro_botones_de_zoom_dan_view_con_su_operacion() {
    for (rotulo, op) in [
        ("Acercar", ViewOp::ZoomIn),
        ("Alejar", ViewOp::ZoomOut),
        ("Ajustar", ViewOp::Fit),
        ("1:1", ViewOp::Actual),
    ] {
        let mut mundo = Mundo::estandar();
        let antes = mundo.foto();
        assert_eq!(
            mundo.despacha(por_rotulo_de_boton(rotulo), Origin::Button),
            Dispatched::View(op),
            "{rotulo}"
        );
        assert_eq!(mundo.foto(), antes, "el zoom no toca el documento");
    }
}

#[test]
fn fila_4_close_licenses_da_licenses_false_con_cualquier_bloqueo_y_origen() {
    for bits in 0..32u8 {
        let locks = Locks {
            gesture: bits & 1 != 0,
            dialog: bits & 2 != 0,
            text_focus: bits & 4 != 0,
            removal: bits & 8 != 0,
            modal: bits & 16 != 0,
        };
        for origin in ORIGENES {
            let mut mundo = Mundo::estandar();
            assert_eq!(
                mundo.despacha_con(AppAction::CloseLicenses, origin, locks),
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
fn fila_5_undo_y_redo_sin_historia_dan_done_sin_cambios() {
    let mut mundo = Mundo::nuevo(rgba(), 0, 0);
    let antes = mundo.foto();
    assert_eq!(mundo.despacha(AppAction::Undo, Origin::Menu), hecho(false));
    assert_eq!(mundo.despacha(AppAction::Redo, Origin::Menu), hecho(false));
    assert_eq!(mundo.foto(), antes);
}

#[test]
fn fila_5_undo_y_redo_con_historia_cambian_el_documento_y_piden_refresco() {
    let mut mundo = Mundo::nuevo(rgba(), 2, 0);
    let inicial = mundo.foto();
    assert_eq!(mundo.despacha(AppAction::Undo, Origin::Menu), hecho(true));
    let tras_undo = mundo.foto();
    assert_ne!(tras_undo.state_id, inicial.state_id);
    assert!(tras_undo.can_undo && tras_undo.can_redo);
    assert_eq!(mundo.despacha(AppAction::Redo, Origin::Key), hecho(true));
    let tras_redo = mundo.foto();
    assert_eq!(
        tras_redo.image, inicial.image,
        "rehacer deja la imagen como estaba"
    );
    assert!(tras_redo.can_undo && !tras_redo.can_redo);
}

#[test]
fn fila_5_con_gesto_dialogo_o_texto_todas_las_que_no_son_zoom_dan_blocked_sin_cambiar_nada() {
    for action in todas_las_acciones()
        .into_iter()
        .filter(no_es_zoom)
        .filter(|a| *a != AppAction::CloseLicenses)
    {
        for origin in ORIGENES {
            // Gesto de verdad: trazo, deslizador y arrastre de muestra.
            for gesto in GESTOS {
                let mut mundo = mundo_con_gesto(gesto);
                let antes = mundo.foto();
                assert_eq!(
                    mundo.despacha(action, origin),
                    Dispatched::Blocked,
                    "{action:?} {origin:?} con {gesto:?}"
                );
                assert_eq!(mundo.foto(), antes, "{action:?} {origin:?} con {gesto:?}");
            }
            // Diálogo y campo de texto.
            for (nombre, locks) in [
                ("dialog", locks_con(|l| l.dialog = true)),
                ("text_focus", locks_con(|l| l.text_focus = true)),
            ] {
                let mut mundo = Mundo::estandar();
                let antes = mundo.foto();
                assert_eq!(
                    mundo.despacha_con(action, origin, locks),
                    Dispatched::Blocked,
                    "{action:?} {origin:?} con {nombre}"
                );
                assert_eq!(mundo.foto(), antes, "{action:?} {origin:?} con {nombre}");
            }
        }
    }
}

#[test]
fn fila_5_un_undo_bloqueado_no_consume_la_historia() {
    let mut mundo = Mundo::nuevo(rgba(), 3, 0);
    let antes = mundo.foto();
    for locks in [
        locks_con(|l| l.gesture = true),
        locks_con(|l| l.dialog = true),
        locks_con(|l| l.text_focus = true),
        locks_con(|l| l.modal = true),
    ] {
        assert_eq!(
            mundo.despacha_con(AppAction::Undo, Origin::Menu, locks),
            Dispatched::Blocked
        );
        assert_eq!(
            mundo.despacha_con(AppAction::Redo, Origin::Key, locks),
            Dispatched::Blocked
        );
        assert_eq!(mundo.foto(), antes, "{locks:?}");
    }
    // Sin bloqueo, el mismo Undo sí deshace.
    assert_eq!(mundo.despacha(AppAction::Undo, Origin::Menu), hecho(true));
    assert_ne!(mundo.foto().state_id, antes.state_id);
}

#[test]
fn fila_5_con_una_quita_pendiente_solo_select_color_da_blocked() {
    for action in todas_las_acciones() {
        for origin in ORIGENES {
            let mut mundo = mundo_con_quita_pendiente();
            let resultado = mundo.despacha(action, origin);
            let bloqueada = matches!(action, AppAction::SelectColor(_));
            assert_eq!(
                resultado == Dispatched::Blocked,
                bloqueada,
                "{action:?} {origin:?}: {resultado:?}"
            );
        }
    }
}

#[test]
fn fila_5_select_color_con_quita_pendiente_no_cambia_la_seleccion_ni_la_quita() {
    let mut mundo = mundo_con_quita_pendiente();
    let antes = mundo.foto();
    for slot in 1..=5 {
        assert_eq!(
            mundo.despacha(AppAction::SelectColor(slot), Origin::Key),
            Dispatched::Blocked
        );
    }
    assert_eq!(mundo.foto(), antes);
    assert!(mundo.session.ops.removal().is_some());
}

#[test]
fn fila_5_undo_cancela_la_quita_pendiente_con_el_aviso_de_11d() {
    let mut mundo = mundo_con_quita_pendiente();
    assert!(mundo.session.document.can_undo());
    assert_eq!(mundo.despacha(AppAction::Undo, Origin::Menu), hecho(true));
    assert!(
        mundo.session.ops.removal().is_none(),
        "deshacer cancela la quita"
    );
    assert!(
        mundo.session.take_removal_cancelled(),
        "y deja pendiente el aviso de quita cancelada"
    );
}

#[test]
fn fila_5_redo_cancela_la_quita_pendiente_con_el_aviso_de_11d() {
    let mut mundo = mundo_con_quita_pendiente();
    assert!(mundo.session.document.can_redo());
    assert_eq!(mundo.despacha(AppAction::Redo, Origin::Key), hecho(true));
    assert!(mundo.session.ops.removal().is_none());
    assert!(mundo.session.take_removal_cancelled());
}

#[test]
fn fila_5_con_modal_solo_pasan_close_licenses_y_quit() {
    let locks = locks_con(|l| l.modal = true);
    for action in todas_las_acciones() {
        for origin in ORIGENES {
            let mut mundo = Mundo::estandar();
            let antes = mundo.foto();
            let resultado = mundo.despacha_con(action, origin, locks);
            match action {
                AppAction::CloseLicenses => {
                    assert_eq!(resultado, Dispatched::Licenses(false), "{origin:?}");
                }
                AppAction::Quit => assert_eq!(resultado, Dispatched::Quit, "{origin:?}"),
                otra => {
                    assert_eq!(resultado, Dispatched::Blocked, "{otra:?} {origin:?}");
                    assert_eq!(mundo.foto(), antes, "{otra:?} {origin:?}");
                }
            }
        }
    }
}

#[test]
fn fila_5_dispatch_da_blocked_exactamente_cuando_is_blocked() {
    for action in todas_las_acciones() {
        for bits in 0..32u8 {
            let locks = Locks {
                gesture: bits & 1 != 0,
                dialog: bits & 2 != 0,
                text_focus: bits & 4 != 0,
                removal: bits & 8 != 0,
                modal: bits & 16 != 0,
            };
            for origin in ORIGENES {
                let mut mundo = Mundo::estandar();
                let resultado = mundo.despacha_con(action, origin, locks);
                assert_eq!(
                    resultado == Dispatched::Blocked,
                    is_blocked(action, locks, origin),
                    "{action:?} {origin:?} {locks:?}: {resultado:?}"
                );
            }
        }
    }
}

#[test]
fn fila_5_enabled_es_falso_exactamente_donde_dispatch_da_blocked_con_origen_menu() {
    // Las acciones cuyo `enabled` solo depende del bloqueo (no de la historia ni del zoom).
    let acciones = todas_las_acciones()
        .into_iter()
        .filter(|a| !matches!(a, AppAction::Undo | AppAction::Redo) && !a.is_zoom());
    for action in acciones {
        for bits in 0..32u8 {
            let locks = Locks {
                gesture: bits & 1 != 0,
                dialog: bits & 2 != 0,
                text_focus: bits & 4 != 0,
                removal: bits & 8 != 0,
                modal: bits & 16 != 0,
            };
            let mut mundo = Mundo::estandar();
            let estado = mundo.estado_de_la_ui(Zoom::from_int(5), locks);
            let resultado = mundo.despacha_con(action, Origin::Menu, locks);
            assert_eq!(
                enabled(action, &estado),
                resultado != Dispatched::Blocked,
                "{action:?} con {locks:?}: {resultado:?}"
            );
        }
    }
}

#[test]
fn fila_5_open_save_y_save_as_dan_file_y_no_tocan_el_documento() {
    for (action, file) in [
        (AppAction::Open, FileAction::Open),
        (AppAction::Save, FileAction::Save),
        (AppAction::SaveAs, FileAction::SaveAs),
    ] {
        for origin in ORIGENES {
            let mut mundo = Mundo::estandar();
            let antes = mundo.foto();
            assert_eq!(mundo.despacha(action, origin), Dispatched::File(file));
            assert_eq!(mundo.foto(), antes);
        }
    }
}

#[test]
fn fila_5_show_licenses_da_licenses_true() {
    for origin in ORIGENES {
        let mut mundo = Mundo::estandar();
        let antes = mundo.foto();
        assert_eq!(
            mundo.despacha(AppAction::ShowLicenses, origin),
            Dispatched::Licenses(true)
        );
        assert_eq!(mundo.foto(), antes);
    }
}

#[test]
fn fila_5_select_tool_bloqueado_por_un_gesto_no_cambia_la_herramienta() {
    for gesto in GESTOS {
        let mut mundo = mundo_con_gesto(gesto);
        for tool in HERRAMIENTAS {
            assert_eq!(
                mundo.despacha(AppAction::SelectTool(tool), Origin::Button),
                Dispatched::Blocked,
                "{gesto:?} {tool:?}"
            );
        }
        assert_eq!(mundo.tools.tool(), Tool::Pencil, "{gesto:?}");
    }
}

// ---------------------------------------------------------------------------------------------
// Fila 5b: bloqueos del zoom
// ---------------------------------------------------------------------------------------------

#[test]
fn fila_5b_a_b_y_c_trazo_deslizador_y_quita_no_bloquean_el_zoom_con_ningun_origen() {
    type CreaMundo = fn() -> Mundo;
    let mundos: Vec<(&str, CreaMundo)> = vec![
        ("(a) trazo del lápiz", || mundo_con_gesto(Gesto::Trazo)),
        ("(b) deslizador del editor", || {
            mundo_con_gesto(Gesto::Deslizador)
        }),
        ("(b') arrastre de muestra", || {
            mundo_con_gesto(Gesto::Muestra)
        }),
        ("(c) quita pendiente", mundo_con_quita_pendiente),
    ];
    for (nombre, crea) in mundos {
        for (action, op) in ZOOMS {
            for origin in ORIGENES {
                let mut mundo = crea();
                let antes = mundo.foto();
                assert_eq!(
                    mundo.despacha(action, origin),
                    Dispatched::View(op),
                    "{nombre}: {action:?} {origin:?}"
                );
                assert_eq!(mundo.foto(), antes, "{nombre}: el zoom no toca nada");
                // `enabled` es verdadero con el zoom y la imagen que hacen falta.
                let estado = mundo.estado_de_la_ui(Zoom::from_int(5), mundo.locks());
                assert!(enabled(action, &estado), "{nombre}: {action:?}");
            }
        }
    }
}

#[test]
fn fila_5b_d_con_un_campo_enfocado_las_teclas_se_bloquean_y_el_menu_y_los_botones_pasan() {
    let locks = locks_con(|l| l.text_focus = true);
    for (action, op) in ZOOMS {
        let mut mundo = Mundo::estandar();
        let antes = mundo.foto();
        assert_eq!(
            mundo.despacha_con(action, Origin::Key, locks),
            Dispatched::Blocked,
            "{action:?} por tecla"
        );
        assert_eq!(mundo.foto(), antes);
        for origin in [Origin::Menu, Origin::Button] {
            assert_eq!(
                mundo.despacha_con(action, origin, locks),
                Dispatched::View(op),
                "{action:?} {origin:?}"
            );
        }
        let estado = mundo.estado_de_la_ui(Zoom::from_int(5), locks);
        assert!(enabled(action, &estado), "{action:?}");
    }
}

#[test]
fn fila_5b_e_y_f_un_dialogo_o_las_licencias_bloquean_el_zoom_con_los_tres_origenes() {
    for (nombre, locks) in [
        ("(e) diálogo", locks_con(|l| l.dialog = true)),
        ("(f) licencias", locks_con(|l| l.modal = true)),
    ] {
        for (action, _) in ZOOMS {
            for origin in ORIGENES {
                let mut mundo = Mundo::estandar();
                let antes = mundo.foto();
                assert_eq!(
                    mundo.despacha_con(action, origin, locks),
                    Dispatched::Blocked,
                    "{nombre}: {action:?} {origin:?}"
                );
                assert_eq!(mundo.foto(), antes);
            }
            let mundo = Mundo::estandar();
            let estado = mundo.estado_de_la_ui(Zoom::from_int(5), locks);
            assert!(!enabled(action, &estado), "{nombre}: {action:?}");
        }
    }
}

#[test]
fn fila_5b_el_zoom_a_mitad_de_trazo_no_corta_el_trazo_y_deja_un_solo_paso() {
    for origin in ORIGENES {
        for (action, op) in ZOOMS {
            let mut mundo = Mundo::nuevo(rgba(), 0, 0);
            let inicial = mundo.foto();
            let id = mundo.session.document.main_image();
            let pixel = mundo.session.active_pixel();
            empieza_un_trazo(&mut mundo);
            let mover = |mundo: &mut Mundo, x: i32| {
                mundo
                    .tools
                    .canvas_event_with(
                        &mut mundo.session.document,
                        id,
                        pixel,
                        CanvasEvent::StrokeMove((x, 0)),
                    )
                    .expect("mover");
            };
            mover(&mut mundo, 1);
            let a_medias = mundo.foto();

            assert_eq!(
                mundo.despacha(action, origin),
                Dispatched::View(op),
                "{action:?} {origin:?}"
            );
            assert!(mundo.tools.busy(), "el trazo sigue tras {action:?}");
            assert_eq!(mundo.foto(), a_medias);
            assert_eq!(
                mundo.foto().state_id,
                inicial.state_id,
                "intacto hasta soltar"
            );

            mover(&mut mundo, 2);
            mover(&mut mundo, 3);
            assert_eq!(mundo.foto().state_id, inicial.state_id);
            mundo
                .tools
                .canvas_event_with(
                    &mut mundo.session.document,
                    id,
                    pixel,
                    CanvasEvent::StrokeEnd,
                )
                .expect("soltar");
            assert!(!mundo.tools.busy());
            assert!(mundo.session.document.can_undo());
            mundo.session.undo().expect("un paso");
            assert!(
                !mundo.session.document.can_undo(),
                "{action:?} {origin:?}: el trazo entero es un solo paso"
            );
            assert_eq!(mundo.foto().image, inicial.image);
        }
    }
}

// ---------------------------------------------------------------------------------------------
// Fila 12: Salir
// ---------------------------------------------------------------------------------------------

#[test]
fn fila_12_quit_da_dispatched_quit_con_cualquier_origen() {
    for origin in ORIGENES {
        let mut mundo = Mundo::estandar();
        let antes = mundo.foto();
        assert_eq!(mundo.despacha(AppAction::Quit, origin), Dispatched::Quit);
        assert_eq!(
            mundo.foto(),
            antes,
            "Salir no toca el documento (cierra `App`)"
        );
    }
}

#[test]
fn fila_12_quit_con_un_trazo_en_curso_o_un_gesto_da_blocked() {
    for gesto in GESTOS {
        for origin in ORIGENES {
            let mut mundo = mundo_con_gesto(gesto);
            assert_eq!(
                mundo.despacha(AppAction::Quit, origin),
                Dispatched::Blocked,
                "{gesto:?} {origin:?}"
            );
        }
    }
}

#[test]
fn fila_12_quit_con_un_dialogo_abierto_da_blocked() {
    let locks = locks_con(|l| l.dialog = true);
    for origin in ORIGENES {
        let mut mundo = Mundo::estandar();
        assert_eq!(
            mundo.despacha_con(AppAction::Quit, origin, locks),
            Dispatched::Blocked,
            "{origin:?}"
        );
    }
}

#[test]
fn fila_12_quit_con_las_licencias_abiertas_pasa() {
    let locks = locks_con(|l| l.modal = true);
    for origin in ORIGENES {
        let mut mundo = Mundo::estandar();
        assert_eq!(
            mundo.despacha_con(AppAction::Quit, origin, locks),
            Dispatched::Quit,
            "{origin:?}"
        );
    }
}

#[test]
fn fila_12_quit_con_una_quita_pendiente_pasa_y_con_un_campo_enfocado_no() {
    let mut mundo = mundo_con_quita_pendiente();
    assert_eq!(
        mundo.despacha(AppAction::Quit, Origin::Menu),
        Dispatched::Quit
    );
    let mut mundo = Mundo::estandar();
    assert_eq!(
        mundo.despacha_con(
            AppAction::Quit,
            Origin::Key,
            locks_con(|l| l.text_focus = true)
        ),
        Dispatched::Blocked
    );
}

// ---------------------------------------------------------------------------------------------
// Propiedad 2: el origen no importa
// ---------------------------------------------------------------------------------------------

fn accion_cualquiera() -> impl Strategy<Value = AppAction> {
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
        Just(AppAction::ZoomActual),
        Just(AppAction::ShowLicenses),
        Just(AppAction::CloseLicenses),
        (0usize..5).prop_map(|i| AppAction::SelectTool(HERRAMIENTAS[i])),
        (1u8..=9).prop_map(AppAction::SelectColor),
    ]
}

fn origen_cualquiera() -> impl Strategy<Value = Origin> {
    (0usize..3).prop_map(|i| ORIGENES[i])
}

fn bloqueos_cualesquiera() -> impl Strategy<Value = Locks> {
    prop::array::uniform5(prop::bool::weighted(0.2)).prop_map(|b| Locks {
        gesture: b[0],
        dialog: b[1],
        text_focus: b[2],
        removal: b[3],
        modal: b[4],
    })
}

fn imagen_de_modo(modo: usize) -> Image {
    match modo % 4 {
        0 => rgba(),
        1 => indexada(5, None),
        2 => gris(),
        _ => gris_alfa(),
    }
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(256))]

    #[test]
    fn propiedad_2_el_origen_no_importa(
        modo in 0usize..4,
        pasos in 0usize..=4,
        deshechos_crudo in 0usize..=4,
        secuencia in prop::collection::vec(
            (accion_cualquiera(), origen_cualquiera(), bloqueos_cualesquiera()),
            0..=24,
        ),
    ) {
        let deshechos = deshechos_crudo % (pasos + 1);
        let mut real = Mundo::nuevo(imagen_de_modo(modo), pasos, deshechos);
        let mut menu = Mundo::nuevo(imagen_de_modo(modo), pasos, deshechos);
        // Modelo de la historia: un `Document` con las mismas operaciones.
        let mut modelo = real.session.document.clone();

        for (action, origin, locks) in secuencia {
            let antes = real.foto();
            let por_origen = real.despacha_con(action, origin, locks);
            let por_menu = menu.despacha_con(action, Origin::Menu, locks);

            // `Blocked` es exactamente `is_blocked`.
            prop_assert_eq!(
                por_origen == Dispatched::Blocked,
                is_blocked(action, locks, origin),
                "{:?} {:?} {:?}", action, origin, locks
            );
            prop_assert_eq!(
                por_menu == Dispatched::Blocked,
                is_blocked(action, locks, Origin::Menu),
                "{:?} (menú) {:?}", action, locks
            );

            // La única diferencia permitida entre orígenes: una acción de zoom con `text_focus` se
            // bloquea con `Key` y no con los otros dos.
            if por_origen != por_menu {
                prop_assert!(
                    action.is_zoom()
                        && locks.text_focus
                        && origin == Origin::Key
                        && por_origen == Dispatched::Blocked
                        && matches!(por_menu, Dispatched::View(_)),
                    "{:?} {:?} {:?}: {:?} y {:?}", action, origin, locks, por_origen, por_menu
                );
            }

            // Un `Blocked` no cambia el estado; el zoom nunca cambia el documento ni la historia.
            if por_origen == Dispatched::Blocked || action.is_zoom() {
                prop_assert_eq!(real.foto(), antes.clone(), "{:?} {:?}", action, locks);
            }

            // El estado final es idéntico al de aplicar las mismas acciones con origen `Menu`.
            prop_assert_eq!(real.foto(), menu.foto(), "{:?} {:?} {:?}", action, origin, locks);

            // `Undo` y `Redo` dejan la historia como `Document`.
            if por_origen != Dispatched::Blocked {
                match action {
                    AppAction::Undo if modelo.can_undo() => modelo.undo().expect("deshacer"),
                    AppAction::Redo if modelo.can_redo() => modelo.redo().expect("rehacer"),
                    _ => {}
                }
            }
            prop_assert_eq!(real.session.document.state_id(), modelo.state_id());
            prop_assert_eq!(real.session.document.can_undo(), modelo.can_undo());
            prop_assert_eq!(real.session.document.can_redo(), modelo.can_redo());
            prop_assert_eq!(real.imagen(), modelo.image(modelo.main_image()).expect("imagen"));
        }
    }
}
