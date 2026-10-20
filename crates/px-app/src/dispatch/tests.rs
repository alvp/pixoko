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
fn fila_5_con_una_quita_pendiente_solo_select_color_y_show_licenses_dan_blocked() {
    for action in todas_las_acciones() {
        for origin in ORIGENES {
            let mut mundo = mundo_con_quita_pendiente();
            let resultado = mundo.despacha(action, origin);
            // `ShowLicenses` abre un diálogo: una quita pendiente también lo frena (unidad 18, D2).
            let bloqueada = matches!(action, AppAction::SelectColor(_) | AppAction::ShowLicenses);
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

const FONDOS: [CanvasBackground; 4] = [
    CanvasBackground::Checker,
    CanvasBackground::Black,
    CanvasBackground::Gray,
    CanvasBackground::White,
];

fn accion_de_fondo(b: CanvasBackground) -> AppAction {
    AppAction::CanvasBackground(b)
}

/// Las sesiones de la fila: con historia y una edición deshecha, con una quita pendiente, con
/// historia en una RGBA y sin historia.
fn mundo_del_caso(caso: usize) -> (&'static str, Mundo) {
    match caso {
        0 => (
            "indexada con historia y una edición deshecha",
            Mundo::estandar(),
        ),
        1 => ("con una quita pendiente", mundo_con_quita_pendiente()),
        2 => (
            "RGBA con historia y un paso deshecho",
            Mundo::nuevo(rgba(), 3, 1),
        ),
        _ => ("sin historia", Mundo::nuevo(rgba(), 0, 0)),
    }
}

const CASOS_DE_LA_FILA_13: usize = 4;

/// Lo que el despacho del fondo no debe tocar, además de la `Foto` de siempre: los bytes de la
/// historia, el indicador de modificado y el gesto en curso.
fn extras(mundo: &Mundo) -> (usize, bool, bool) {
    (
        mundo.session.document.history_bytes(),
        mundo.session.modified(),
        mundo.session.busy(),
    )
}

#[test]
fn fila_13_las_cuatro_acciones_con_cada_origen_dan_background_y_no_tocan_nada() {
    for caso in 0..CASOS_DE_LA_FILA_13 {
        for b in FONDOS {
            for origin in ORIGENES {
                let (nombre, mut mundo) = mundo_del_caso(caso);
                let antes = mundo.foto();
                let extras_antes = extras(&mundo);
                assert_eq!(
                    mundo.despacha(accion_de_fondo(b), origin),
                    Dispatched::Background(b),
                    "{nombre}: {b:?} con {origin:?}"
                );
                // La foto de la fila 3b de la 17: `state_id`, `can_undo`, `can_redo`, imagen,
                // elección, herramienta, quita pendiente y gesto, iguales.
                assert_eq!(mundo.foto(), antes, "{nombre}: {b:?} con {origin:?}");
                assert_eq!(
                    extras(&mundo),
                    extras_antes,
                    "{nombre}: {b:?} con {origin:?}"
                );
                assert_eq!(mundo.session.document.state_id(), antes.state_id);
                assert_eq!(mundo.session.document.can_undo(), antes.can_undo);
                assert_eq!(mundo.session.document.can_redo(), antes.can_redo);
                assert_eq!(mundo.session.choice, antes.choice);
                assert_eq!(mundo.tools.tool(), antes.tool);
            }
        }
    }
}

#[test]
fn fila_13_la_accion_no_marca_el_documento_como_modificado_ni_crea_paso() {
    let (_, mut mundo) = mundo_del_caso(0);
    assert!(mundo.session.document.can_undo(), "hay historia");
    assert!(mundo.session.document.can_redo(), "y una edición deshecha");
    let modificado = mundo.session.modified();
    let bytes = mundo.session.document.history_bytes();
    for b in FONDOS {
        mundo.despacha(accion_de_fondo(b), Origin::Menu);
    }
    assert_eq!(mundo.session.modified(), modificado);
    assert_eq!(mundo.session.document.history_bytes(), bytes);
    assert!(
        mundo.session.document.can_redo(),
        "la edición deshecha se puede rehacer: no se ha creado ningún paso"
    );
    // Una sesión recién abierta sigue sin cambios.
    let (_, mut nueva) = mundo_del_caso(3);
    assert!(!nueva.session.modified());
    nueva.despacha(accion_de_fondo(CanvasBackground::Black), Origin::Menu);
    assert!(!nueva.session.modified());
    assert!(!nueva.session.document.can_undo());
}

#[test]
fn fila_13_no_es_zoom_ni_repite_y_el_despacho_no_da_view() {
    for b in FONDOS {
        let action = accion_de_fondo(b);
        assert!(!action.is_zoom(), "{b:?}");
        assert!(!action.ignores_repeat(), "{b:?}");
        let (_, mut mundo) = mundo_del_caso(0);
        let resultado = mundo.despacha(action, Origin::Key);
        assert!(!matches!(resultado, Dispatched::View(_)), "{b:?}");
        assert_ne!(resultado, Dispatched::Blocked);
    }
}

#[test]
fn fila_13_cada_bloqueo_solo_y_el_despacho_sin_bloqueos() {
    // Bloqueada solo con `dialog`, con `modal` y con `text_focus`, con cualquier origen; no con
    // `gesture` ni con `removal`.
    let casos: [(&str, Locks, bool); 6] = [
        ("ninguno", Locks::default(), false),
        ("gesture", locks_con(|l| l.gesture = true), false),
        ("removal", locks_con(|l| l.removal = true), false),
        ("dialog", locks_con(|l| l.dialog = true), true),
        ("modal", locks_con(|l| l.modal = true), true),
        ("text_focus", locks_con(|l| l.text_focus = true), true),
    ];
    for caso in 0..CASOS_DE_LA_FILA_13 {
        for b in FONDOS {
            for (nombre, locks, bloqueada) in casos {
                for origin in ORIGENES {
                    let (sesion, mut mundo) = mundo_del_caso(caso);
                    let antes = mundo.foto();
                    let resultado = mundo.despacha_con(accion_de_fondo(b), origin, locks);
                    let esperado = if bloqueada {
                        Dispatched::Blocked
                    } else {
                        Dispatched::Background(b)
                    };
                    assert_eq!(
                        resultado, esperado,
                        "{sesion}: {b:?} con {nombre} y {origin:?}"
                    );
                    assert_eq!(mundo.foto(), antes, "{sesion}: {b:?} con {nombre}");
                }
            }
        }
    }
}

#[test]
fn fila_13_un_gesto_de_verdad_o_una_quita_pendiente_no_bloquean_el_fondo() {
    // Con los bloqueos que saca `main` de un trazo, un deslizador, un arrastre de muestra o una
    // quita pendiente (`Mundo::locks`), el fondo se despacha con los tres orígenes.
    for gesto in GESTOS {
        for b in FONDOS {
            for origin in ORIGENES {
                let mut mundo = mundo_con_gesto(gesto);
                assert!(mundo.locks().gesture);
                let antes = mundo.foto();
                assert_eq!(
                    mundo.despacha(accion_de_fondo(b), origin),
                    Dispatched::Background(b),
                    "{gesto:?} {b:?} {origin:?}"
                );
                assert_eq!(mundo.foto(), antes, "el gesto sigue en curso: {gesto:?}");
                assert!(mundo.locks().gesture, "{gesto:?}");
            }
        }
    }
    for b in FONDOS {
        for origin in ORIGENES {
            let mut mundo = mundo_con_quita_pendiente();
            assert!(mundo.locks().removal);
            let antes = mundo.foto();
            assert_eq!(
                mundo.despacha(accion_de_fondo(b), origin),
                Dispatched::Background(b)
            );
            assert_eq!(mundo.foto(), antes, "la quita sigue pendiente");
            assert!(mundo.session.ops.removal().is_some());
        }
    }
}

#[test]
fn fila_13_con_los_bloqueos_combinados_solo_cuentan_dialog_modal_y_text_focus() {
    for caso in 0..CASOS_DE_LA_FILA_13 {
        for bits in 0..32u8 {
            let locks = Locks {
                gesture: bits & 1 != 0,
                dialog: bits & 2 != 0,
                text_focus: bits & 4 != 0,
                removal: bits & 8 != 0,
                modal: bits & 16 != 0,
            };
            for b in FONDOS {
                for origin in ORIGENES {
                    let (sesion, mut mundo) = mundo_del_caso(caso);
                    let antes = mundo.foto();
                    let bloqueada = locks.dialog || locks.modal || locks.text_focus;
                    let resultado = mundo.despacha_con(accion_de_fondo(b), origin, locks);
                    assert_eq!(
                        resultado == Dispatched::Blocked,
                        bloqueada,
                        "{sesion}: {b:?} con {locks:?} y {origin:?}"
                    );
                    if !bloqueada {
                        assert_eq!(resultado, Dispatched::Background(b));
                    }
                    assert_eq!(mundo.foto(), antes);
                }
            }
        }
    }
}

#[test]
fn fila_13_enabled_es_lo_contrario_de_blocked_con_el_origen_menu() {
    for caso in 0..CASOS_DE_LA_FILA_13 {
        for bits in 0..32u8 {
            let locks = Locks {
                gesture: bits & 1 != 0,
                dialog: bits & 2 != 0,
                text_focus: bits & 4 != 0,
                removal: bits & 8 != 0,
                modal: bits & 16 != 0,
            };
            for b in FONDOS {
                for zoom in [None, Zoom::from_int(5)] {
                    let (sesion, mut mundo) = mundo_del_caso(caso);
                    let estado = mundo.estado_de_la_ui(zoom, locks);
                    let habilitada = enabled(accion_de_fondo(b), &estado);
                    let resultado = mundo.despacha_con(accion_de_fondo(b), Origin::Menu, locks);
                    assert_eq!(
                        habilitada,
                        resultado != Dispatched::Blocked,
                        "{sesion}: {b:?} con {locks:?} y zoom {zoom:?}"
                    );
                }
            }
        }
    }
}

#[test]
fn fila_13_las_acciones_anteriores_se_despachan_igual_que_antes_con_el_fondo_en_medio() {
    // Intercalar acciones de fondo entre las demás no cambia lo que hacen: el resultado y el
    // estado final son los mismos que sin ellas.
    let secuencia = [
        AppAction::SelectColor(2),
        AppAction::Undo,
        AppAction::ZoomIn,
        AppAction::Redo,
        AppAction::SelectTool(Tool::Line),
    ];
    let mut sin = Mundo::estandar();
    let mut con = Mundo::estandar();
    for (i, action) in secuencia.into_iter().enumerate() {
        con.despacha(accion_de_fondo(FONDOS[i % 4]), Origin::Menu);
        assert_eq!(
            con.despacha(action, Origin::Key),
            sin.despacha(action, Origin::Key),
            "{action:?}"
        );
        assert_eq!(con.foto(), sin.foto(), "{action:?}");
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
// - Los bloqueos de (c) y (d) son los de la sesión (`Mundo::locks`) más `modal: true`; con la
//   sesión de la quita pendiente, `removal` también está activo (Quit sigue pasando: la regla
//   (2) lo deja y `removal` solo frena a `SelectColor` y a `ShowLicenses`).
// - Un `Modal` y un `Confirm` no se comunican: abandonar el diálogo no arma ni desarma la
//   confirmación (la primera petición con cambios da `false` aunque haya un diálogo abierto).
// - Suposiciones sobre las firmas de «Piezas»: `Modal: Default` y `Reaction: Clone + Debug +
//   PartialEq` (derives no listados).

use px_ui::dialog::{DialogKind as ModalKind, Event as DialogEvent, Modal, Reaction};
use px_ui::licenses_view::licenses_form;

use crate::confirm::{CONFIRM_SECS, Confirm, Confirmation, Request, close_allowed};

/// Las 30 acciones: las de `todas_las_acciones` más los cuatro fondos del lienzo.
fn todas_las_acciones_con_fondos() -> Vec<AppAction> {
    let mut v = todas_las_acciones();
    v.extend(FONDOS.map(accion_de_fondo));
    v
}

/// Las 32 combinaciones de los cinco modificadores de egui.
fn modificadores_de_bits(bits: u8) -> Modifiers {
    Modifiers {
        alt: bits & 1 != 0,
        ctrl: bits & 2 != 0,
        shift: bits & 4 != 0,
        mac_cmd: bits & 8 != 0,
        command: bits & 16 != 0,
    }
}

/// Los bloqueos de la sesión con el diálogo modal abierto.
fn con_modal(mundo: &Mundo) -> Locks {
    Locks {
        modal: true,
        ..mundo.locks()
    }
}

fn pasa_con_modal(action: AppAction) -> bool {
    matches!(action, AppAction::CloseLicenses | AppAction::Quit)
}

fn abre_licencias(modal: &mut Modal) -> bool {
    modal.open(
        ModalKind::Licenses,
        licenses_form("texto"),
        Locks::default(),
    )
}

// --- (c) todas las acciones con modal ---

#[test]
fn fila_6c_con_modal_dispatch_da_blocked_en_todas_salvo_close_licenses_y_quit() {
    let acciones = todas_las_acciones_con_fondos();
    assert_eq!(acciones.len(), 30);
    for caso in 0..CASOS_DE_LA_FILA_13 {
        for action in &acciones {
            for origin in ORIGENES {
                let (sesion, mut mundo) = mundo_del_caso(caso);
                let locks = con_modal(&mundo);
                let antes = mundo.foto();
                let extras_antes = extras(&mundo);
                let resultado = mundo.despacha_con(*action, origin, locks);
                let esperado = match action {
                    AppAction::CloseLicenses => Dispatched::Licenses(false),
                    AppAction::Quit => Dispatched::Quit,
                    _ => Dispatched::Blocked,
                };
                assert_eq!(resultado, esperado, "{sesion}: {action:?} con {origin:?}");
                assert_eq!(
                    mundo.foto(),
                    antes,
                    "{sesion}: {action:?} con {origin:?} tocó el estado"
                );
                assert_eq!(extras(&mundo), extras_antes, "{sesion}: {action:?}");
            }
        }
    }
}

#[test]
fn fila_6c_con_modal_no_cambian_state_id_historia_herramienta_ni_eleccion() {
    for caso in 0..CASOS_DE_LA_FILA_13 {
        let (sesion, mut mundo) = mundo_del_caso(caso);
        let locks = con_modal(&mundo);
        let state_id = mundo.session.document.state_id();
        let can_undo = mundo.session.document.can_undo();
        let can_redo = mundo.session.document.can_redo();
        let bytes = mundo.session.document.history_bytes();
        let modificado = mundo.session.modified();
        let eleccion = mundo.session.choice;
        let herramienta = mundo.tools.tool();
        let quita = mundo.session.ops.removal();
        for action in todas_las_acciones_con_fondos() {
            for origin in ORIGENES {
                mundo.despacha_con(action, origin, locks);
            }
        }
        assert_eq!(mundo.session.document.state_id(), state_id, "{sesion}");
        assert_eq!(mundo.session.document.can_undo(), can_undo, "{sesion}");
        assert_eq!(mundo.session.document.can_redo(), can_redo, "{sesion}");
        assert_eq!(mundo.session.document.history_bytes(), bytes, "{sesion}");
        assert_eq!(mundo.session.modified(), modificado, "{sesion}");
        assert_eq!(mundo.session.choice, eleccion, "{sesion}");
        assert_eq!(mundo.tools.tool(), herramienta, "{sesion}");
        assert_eq!(mundo.session.ops.removal(), quita, "{sesion}");
    }
}

#[test]
fn fila_6c_con_modal_las_nueve_ranuras_y_las_cinco_herramientas_no_cambian_nada() {
    for origin in ORIGENES {
        let mut mundo = Mundo::nuevo(indexada(9, None), 2, 1);
        let locks = con_modal(&mundo);
        let antes = mundo.foto();
        for slot in 1..=9u8 {
            assert_eq!(
                mundo.despacha_con(AppAction::SelectColor(slot), origin, locks),
                Dispatched::Blocked,
                "ranura {slot} con {origin:?}"
            );
        }
        for tool in HERRAMIENTAS {
            assert_eq!(
                mundo.despacha_con(AppAction::SelectTool(tool), origin, locks),
                Dispatched::Blocked,
                "{tool:?} con {origin:?}"
            );
        }
        assert_eq!(mundo.foto(), antes, "{origin:?}");
    }
}

#[test]
fn fila_6c_con_modal_deshacer_y_rehacer_no_consumen_la_historia_ni_cancelan_la_quita() {
    for origin in ORIGENES {
        let mut mundo = Mundo::estandar();
        let locks = con_modal(&mundo);
        let antes = mundo.foto();
        assert!(antes.can_undo && antes.can_redo);
        assert_eq!(
            mundo.despacha_con(AppAction::Undo, origin, locks),
            Dispatched::Blocked
        );
        assert_eq!(
            mundo.despacha_con(AppAction::Redo, origin, locks),
            Dispatched::Blocked
        );
        assert_eq!(mundo.foto(), antes, "{origin:?}");

        // Con una quita pendiente, deshacer bloqueado no la cancela (a diferencia de la fila 5).
        let mut mundo = mundo_con_quita_pendiente();
        let locks = con_modal(&mundo);
        assert!(locks.removal && locks.modal);
        assert_eq!(
            mundo.despacha_con(AppAction::Undo, origin, locks),
            Dispatched::Blocked
        );
        assert!(mundo.session.ops.removal().is_some(), "{origin:?}");
        assert!(!mundo.session.take_removal_cancelled(), "{origin:?}");
    }
}

#[test]
fn fila_6c_con_modal_enabled_es_falso_salvo_close_licenses_y_quit() {
    for caso in 0..CASOS_DE_LA_FILA_13 {
        for zoom in [None, Zoom::from_int(5)] {
            let (sesion, mundo) = mundo_del_caso(caso);
            let locks = con_modal(&mundo);
            let estado = mundo.estado_de_la_ui(zoom, locks);
            for action in todas_las_acciones_con_fondos() {
                assert_eq!(
                    enabled(action, &estado),
                    pasa_con_modal(action),
                    "{sesion}: {action:?} con zoom {zoom:?}"
                );
            }
        }
    }
}

#[test]
fn fila_6c_con_modal_dispatch_da_blocked_exactamente_cuando_is_blocked_en_las_treinta_acciones() {
    for action in todas_las_acciones_con_fondos() {
        for origin in ORIGENES {
            let mut mundo = Mundo::estandar();
            let locks = con_modal(&mundo);
            let resultado = mundo.despacha_con(action, origin, locks);
            assert_eq!(
                resultado == Dispatched::Blocked,
                is_blocked(action, locks, origin),
                "{action:?} {origin:?}: {resultado:?}"
            );
        }
    }
}

#[test]
fn fila_6c_un_dialogo_abierto_con_show_licenses_cierra_con_close_licenses_y_no_deja_abrir_otro() {
    // El recorrido de `main`: `Licenses(true)` abre; con el diálogo abierto `ShowLicenses` ya no
    // pasa; `Licenses(false)` cierra solo el diálogo de Licencias.
    let mut mundo = Mundo::estandar();
    let mut modal = Modal::default();
    let locks = Locks {
        modal: modal.is_open(),
        ..mundo.locks()
    };
    assert_eq!(
        mundo.despacha_con(AppAction::ShowLicenses, Origin::Menu, locks),
        Dispatched::Licenses(true)
    );
    assert!(modal.open(ModalKind::Licenses, licenses_form("texto"), locks));

    let locks = Locks {
        modal: modal.is_open(),
        ..mundo.locks()
    };
    assert!(locks.modal);
    for origin in ORIGENES {
        assert_eq!(
            mundo.despacha_con(AppAction::ShowLicenses, origin, locks),
            Dispatched::Blocked,
            "{origin:?}"
        );
    }
    assert!(!modal.open(ModalKind::Licenses, licenses_form("otro"), locks));

    assert_eq!(
        mundo.despacha_con(AppAction::CloseLicenses, Origin::Button, locks),
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
fn fila_6d_ninguna_tecla_cambia_el_estado_con_modal_y_solo_ctrl_q_en_linux_da_quit() {
    let mut mundo = Mundo::estandar();
    let locks = con_modal(&mundo);
    let antes = mundo.foto();
    let extras_antes = extras(&mundo);
    let mut traducidas = 0usize;
    let mut quits = 0usize;
    for &key in Key::ALL {
        for bits in 0..32u8 {
            let modifiers = modificadores_de_bits(bits);
            for platform in PLATAFORMAS {
                let Some(action) = action_for_shortcut(key, modifiers, platform) else {
                    continue;
                };
                traducidas += 1;
                let contexto = format!("{key:?} con {modifiers:?} en {platform:?}: {action:?}");
                let resultado = mundo.despacha_con(action, Origin::Key, locks);
                match resultado {
                    Dispatched::Quit => {
                        quits += 1;
                        assert_eq!(action, AppAction::Quit, "{contexto}");
                        assert_eq!(key, Key::Q, "{contexto}");
                        assert_eq!(platform, Platform::Linux, "{contexto}");
                        assert!(modifiers.command, "{contexto}");
                        assert!(!modifiers.alt && !modifiers.shift, "{contexto}");
                    }
                    Dispatched::Blocked => {
                        assert_ne!(action, AppAction::Quit, "{contexto}");
                    }
                    otro => panic!("{contexto}: con modal no puede dar {otro:?}"),
                }
                assert_eq!(mundo.foto(), antes, "{contexto}");
                assert_eq!(extras(&mundo), extras_antes, "{contexto}");
            }
        }
    }
    assert!(traducidas > 100, "se tradujeron {traducidas} combinaciones");
    assert!(quits > 0, "Ctrl+Q en Linux da Quit");
}

#[test]
fn fila_6d_ctrl_q_en_linux_da_quit_con_modal_y_en_windows_y_mac_no_hay_atajo() {
    let mut mundo = Mundo::estandar();
    let locks = con_modal(&mundo);
    let action = action_for_shortcut(Key::Q, command(Platform::Linux), Platform::Linux)
        .expect("Ctrl+Q en Linux es un atajo");
    assert_eq!(action, AppAction::Quit);
    assert_eq!(
        mundo.despacha_con(action, Origin::Key, locks),
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
fn fila_6d_las_teclas_de_herramienta_color_zoom_y_fichero_dan_blocked_con_modal() {
    let casos = [
        (Key::P, Modifiers::NONE),
        (Key::L, Modifiers::NONE),
        (Key::F, Modifiers::NONE),
        (Key::I, Modifiers::NONE),
        (Key::E, Modifiers::NONE),
        (Key::Num1, Modifiers::NONE),
        (Key::Num5, Modifiers::NONE),
        (Key::Num9, Modifiers::NONE),
        (Key::Plus, Modifiers::NONE),
        (Key::Minus, Modifiers::NONE),
        (Key::Home, Modifiers::NONE),
        (Key::Z, command(Platform::Linux)),
        (Key::O, command(Platform::Linux)),
        (Key::S, command(Platform::Linux)),
        (Key::Num1, command(Platform::Linux)),
    ];
    for (key, modifiers) in casos {
        let mut mundo = Mundo::estandar();
        let locks = con_modal(&mundo);
        let antes = mundo.foto();
        let action = action_for_shortcut(key, modifiers, Platform::Linux)
            .unwrap_or_else(|| panic!("{key:?} con {modifiers:?} debería traducirse"));
        assert_eq!(
            mundo.despacha_con(action, Origin::Key, locks),
            Dispatched::Blocked,
            "{key:?} con {modifiers:?}: {action:?}"
        );
        assert_eq!(mundo.foto(), antes, "{key:?} con {modifiers:?}");
    }
}

// --- (f) cerrar la ventana con un diálogo abierto ---

#[test]
fn fila_6f_abandonar_el_dialogo_no_da_reaccion_y_deja_todo_cerrado() {
    let mut modal = Modal::default();
    assert!(abre_licencias(&mut modal));
    assert!(modal.abandon(), "había un diálogo que abandonar");
    assert!(!modal.is_open());
    assert_eq!(modal.kind(), None);
    assert!(!modal.abandon(), "la segunda vez no hay nada");
    // Tras abandonar, ningún evento hace nada: ni `Accepted` ni `Cancelled`.
    for evento in [DialogEvent::Accept, DialogEvent::Cancel] {
        assert_eq!(modal.event(evento), Reaction::None);
    }
}

#[test]
fn fila_6f_sin_cambios_el_dialogo_se_abandona_y_la_ventana_cierra_a_la_primera() {
    let mut modal = Modal::default();
    let mut confirm = Confirm::new();
    assert!(abre_licencias(&mut modal));
    assert!(modal.abandon());
    assert!(close_allowed(false, &mut confirm, 0.0));
    assert!(!modal.is_open());
}

#[test]
fn fila_6f_con_cambios_el_dialogo_se_abandona_y_hacen_falta_dos_peticiones() {
    let mut modal = Modal::default();
    let mut confirm = Confirm::new();
    assert!(abre_licencias(&mut modal));

    // Primera petición: el diálogo se abandona siempre y la ventana no cierra.
    assert!(modal.abandon());
    assert!(!close_allowed(true, &mut confirm, 10.0));
    assert!(!modal.is_open());

    // Segunda, dentro del plazo: cierra (ya no hay diálogo que abandonar).
    assert!(!modal.abandon());
    assert!(close_allowed(true, &mut confirm, 10.0 + CONFIRM_SECS - 0.1));
}

#[test]
fn fila_6f_con_cambios_y_un_dialogo_abierto_en_cada_peticion_se_abandona_cada_vez() {
    let mut modal = Modal::default();
    let mut confirm = Confirm::new();

    assert!(abre_licencias(&mut modal));
    assert!(modal.abandon());
    assert!(!close_allowed(true, &mut confirm, 0.0));

    // El usuario vuelve a abrir el diálogo y pide cerrar otra vez dentro del plazo.
    assert!(abre_licencias(&mut modal));
    assert!(modal.abandon(), "se abandona también la segunda vez");
    assert!(close_allowed(true, &mut confirm, 2.0));
    assert!(!modal.is_open());
}

#[test]
fn fila_6f_fuera_del_plazo_la_segunda_peticion_vuelve_a_armar_y_la_tercera_cierra() {
    let mut modal = Modal::default();
    let mut confirm = Confirm::new();

    assert!(abre_licencias(&mut modal));
    assert!(modal.abandon());
    assert!(!close_allowed(true, &mut confirm, 0.0));

    assert!(abre_licencias(&mut modal));
    assert!(modal.abandon());
    assert!(
        !close_allowed(true, &mut confirm, CONFIRM_SECS),
        "a los CONFIRM_SECS justos la primera ya caducó"
    );
    assert!(!modal.is_open());

    assert!(close_allowed(true, &mut confirm, CONFIRM_SECS + 1.0));
}

#[test]
fn fila_6f_abandonar_no_arma_ni_desarma_la_confirmacion() {
    let mut modal = Modal::default();
    let mut confirm = Confirm::new();
    // Abandonar sin pedir cerrar no deja nada armado: la primera petición con cambios no cierra.
    assert!(abre_licencias(&mut modal));
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
    fn fila_6f_abandonar_el_dialogo_no_cambia_la_regla_de_cierre(
        pasos in prop::collection::vec((any::<bool>(), any::<bool>(), 0.0f64..9.0), 1..12),
    ) {
        let mut real = Confirm::new();
        let mut modelo = Confirm::new();
        let mut modal = Modal::default();
        let mut now = 0.0;
        for (modified, abre, paso) in pasos {
            now += paso;
            if abre {
                let _ = abre_licencias(&mut modal);
            }
            let estaba_abierto = modal.is_open();
            prop_assert_eq!(modal.abandon(), estaba_abierto);
            prop_assert!(!modal.is_open());
            let esperado =
                !modified || modelo.request(Request::Close, now) == Confirmation::Confirmed;
            prop_assert_eq!(
                close_allowed(modified, &mut real, now),
                esperado,
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
fn mundos_sin_quita() -> Vec<(&'static str, Mundo)> {
    vec![
        (
            "indexada con historia y una edición deshecha",
            Mundo::estandar(),
        ),
        (
            "RGBA con historia y un paso deshecho",
            Mundo::nuevo(rgba(), 3, 1),
        ),
        ("sin historia", Mundo::nuevo(rgba(), 0, 0)),
    ]
}

fn command_de(plataforma: Platform) -> Modifiers {
    if plataforma == Platform::Mac {
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

const PLATAFORMAS_DE_LA_19: [Platform; 3] = [Platform::Linux, Platform::Windows, Platform::Mac];

/// Cada uno de los cinco bloqueos, solo.
fn bloqueos_de_uno_en_uno() -> [(&'static str, Locks); 5] {
    [
        ("gesture", locks_con(|l| l.gesture = true)),
        ("dialog", locks_con(|l| l.dialog = true)),
        ("text_focus", locks_con(|l| l.text_focus = true)),
        ("removal", locks_con(|l| l.removal = true)),
        ("modal", locks_con(|l| l.modal = true)),
    ]
}

#[test]
fn fila_9_nueva_imagen_da_dispatched_new_image_con_cada_origen_y_no_toca_nada() {
    for (nombre, mut mundo) in mundos_sin_quita() {
        for origin in ORIGENES {
            let antes = (mundo.foto(), extras(&mundo));
            let resultado = mundo.despacha(AppAction::NewImage, origin);
            assert_eq!(resultado, Dispatched::NewImage, "{nombre}, {origin:?}");
            assert!(
                !matches!(resultado, Dispatched::View(_)),
                "{nombre}: no es zoom"
            );
            assert_eq!(
                (mundo.foto(), extras(&mundo)),
                antes,
                "{nombre}, {origin:?}: despachar no toca nada (lo hace App)"
            );
        }
    }
}

#[test]
fn fila_9_menu_boton_y_atajo_dan_la_misma_accion_y_el_mismo_dispatched() {
    // El menú: la entrada «Nueva imagen…» de la tabla.
    let entrada = MENUS
        .iter()
        .flat_map(|m| m.items.iter())
        .find(|item| item.label == "Nueva imagen…")
        .expect("el menú tiene «Nueva imagen…»");
    assert_eq!(entrada.action, AppAction::NewImage);

    for plataforma in PLATAFORMAS_DE_LA_19 {
        // El atajo: `command`+`N`.
        let atajo = action_for_shortcut(Key::N, command_de(plataforma), plataforma);
        assert_eq!(atajo, Some(AppAction::NewImage), "{plataforma:?}");

        let mut por_menu = Mundo::estandar();
        let mut por_atajo = Mundo::estandar();
        let mut por_boton = Mundo::estandar();
        let a = por_menu.despacha(entrada.action, Origin::Menu);
        let b = por_atajo.despacha(atajo.expect("hay atajo"), Origin::Key);
        let c = por_boton.despacha(AppAction::NewImage, Origin::Button);
        assert_eq!(a, Dispatched::NewImage, "{plataforma:?}");
        assert_eq!(a, b, "{plataforma:?}: menú y atajo");
        assert_eq!(a, c, "{plataforma:?}: menú y botón");
        assert_eq!(por_menu.foto(), por_atajo.foto());
        assert_eq!(por_menu.foto(), por_boton.foto());
    }
}

#[test]
fn fila_9_la_tecla_n_solo_da_nueva_imagen_con_command_sin_mayus_ni_alt() {
    for plataforma in PLATAFORMAS_DE_LA_19 {
        for bits in 0..32u8 {
            let m = modificadores_de_bits(bits);
            let esperado = m.command && !m.shift && !m.alt;
            let accion = action_for_shortcut(Key::N, m, plataforma);
            assert_eq!(
                accion == Some(AppAction::NewImage),
                esperado,
                "{plataforma:?}, modificadores {bits:05b}: {accion:?}"
            );
            // Y con la tecla que la lleva a `dispatch` pasa lo mismo.
            if let Some(accion) = accion {
                let mut mundo = Mundo::estandar();
                let r = mundo.despacha(accion, Origin::Key);
                assert_eq!(r == Dispatched::NewImage, esperado, "{bits:05b}");
            }
        }
    }
}

#[test]
fn fila_9_cada_bloqueo_solo_bloquea_nueva_imagen_con_cualquier_origen_y_no_toca_nada() {
    for (nombre, locks) in bloqueos_de_uno_en_uno() {
        for origin in ORIGENES {
            let mut mundo = Mundo::estandar();
            let antes = (mundo.foto(), extras(&mundo));
            assert_eq!(
                mundo.despacha_con(AppAction::NewImage, origin, locks),
                Dispatched::Blocked,
                "{nombre}, {origin:?}"
            );
            assert!(is_blocked(AppAction::NewImage, locks, origin), "{nombre}");
            assert_eq!(
                (mundo.foto(), extras(&mundo)),
                antes,
                "{nombre}, {origin:?}: un NewImage bloqueado no toca nada"
            );
        }
    }
}

#[test]
fn fila_9_con_las_32_combinaciones_de_bloqueos_solo_pasa_sin_ninguno() {
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
            let esperado = if bits == 0 {
                Dispatched::NewImage
            } else {
                Dispatched::Blocked
            };
            assert_eq!(
                mundo.despacha_con(AppAction::NewImage, origin, locks),
                esperado,
                "{bits:05b}, {origin:?}"
            );
        }
    }
}

#[test]
fn fila_9_enabled_es_falso_exactamente_donde_dispatch_da_blocked_con_origen_menu() {
    for zoom in [None, Zoom::from_int(5)] {
        for bits in 0..32u8 {
            let locks = Locks {
                gesture: bits & 1 != 0,
                dialog: bits & 2 != 0,
                text_focus: bits & 4 != 0,
                removal: bits & 8 != 0,
                modal: bits & 16 != 0,
            };
            let mut mundo = Mundo::estandar();
            let estado = mundo.estado_de_la_ui(zoom, locks);
            let bloqueada =
                mundo.despacha_con(AppAction::NewImage, Origin::Menu, locks) == Dispatched::Blocked;
            assert_eq!(
                enabled(AppAction::NewImage, &estado),
                !bloqueada,
                "{bits:05b}, zoom {zoom:?}"
            );
        }
    }
}

#[test]
fn fila_9_un_gesto_de_verdad_o_una_quita_pendiente_bloquean_nueva_imagen() {
    // Un trazo en curso.
    let mut mundo = Mundo::estandar();
    empieza_un_trazo(&mut mundo);
    let antes = mundo.foto();
    assert_eq!(
        mundo.despacha(AppAction::NewImage, Origin::Menu),
        Dispatched::Blocked,
        "trazo"
    );
    assert_eq!(mundo.foto(), antes);
    assert!(!enabled(
        AppAction::NewImage,
        &mundo.estado_de_la_ui(Zoom::from_int(5), mundo.locks())
    ));

    // El deslizador del editor.
    let mut mundo = Mundo::estandar();
    pulsa_el_deslizador(&mut mundo);
    assert_eq!(
        mundo.despacha(AppAction::NewImage, Origin::Key),
        Dispatched::Blocked,
        "deslizador"
    );

    // El arrastre de una muestra.
    let mut mundo = Mundo::estandar();
    arrastra_una_muestra(&mut mundo);
    assert_eq!(
        mundo.despacha(AppAction::NewImage, Origin::Button),
        Dispatched::Blocked,
        "arrastre de muestra"
    );

    // Una quita de paleta pendiente: abrir un diálogo también se bloquea (regla literal de la 18).
    let mut mundo = mundo_con_quita_pendiente();
    assert!(mundo.locks().removal);
    let antes = (mundo.foto(), extras(&mundo));
    for origin in ORIGENES {
        assert_eq!(
            mundo.despacha(AppAction::NewImage, origin),
            Dispatched::Blocked,
            "quita pendiente, {origin:?}"
        );
    }
    assert_eq!((mundo.foto(), extras(&mundo)), antes);
    assert!(!enabled(
        AppAction::NewImage,
        &mundo.estado_de_la_ui(Zoom::from_int(5), mundo.locks())
    ));
}

#[test]
fn fila_9_con_el_dialogo_de_nueva_imagen_abierto_todo_esta_bloqueado_salvo_quit() {
    let mut mundo = Mundo::estandar();
    let mut modal = Modal::default();
    assert!(open_new_image(&mut modal, 16384, mundo.locks()));
    assert_eq!(modal.kind(), Some(ModalKind::NewImage));
    let locks = con_modal(&mundo);
    assert!(locks.modal);

    let antes = (mundo.foto(), extras(&mundo));
    for action in todas_las_acciones()
        .into_iter()
        .chain([AppAction::NewImage])
    {
        for origin in ORIGENES {
            let esperado = match action {
                AppAction::Quit => Dispatched::Quit,
                // `CloseLicenses` no es de este diálogo, pero la regla (1) la deja pasar siempre.
                AppAction::CloseLicenses => Dispatched::Licenses(false),
                _ => Dispatched::Blocked,
            };
            assert_eq!(
                mundo.despacha_con(action, origin, locks),
                esperado,
                "{action:?}, {origin:?}"
            );
        }
    }
    assert_eq!(
        (mundo.foto(), extras(&mundo)),
        antes,
        "con el diálogo abierto el documento no cambia"
    );
}

#[test]
fn fila_9_un_dialogo_abierto_no_deja_abrir_otro_ni_otra_nueva_imagen() {
    let mundo = Mundo::estandar();
    let mut modal = Modal::default();
    assert!(open_new_image(&mut modal, 16384, mundo.locks()));
    assert!(
        !open_new_image(&mut modal, 16384, mundo.locks()),
        "ya hay uno abierto"
    );
    assert!(!abre_licencias(&mut modal));
    assert_eq!(modal.kind(), Some(ModalKind::NewImage));
}

#[test]
fn fila_9_cada_bloqueo_impide_abrir_el_dialogo_de_nueva_imagen() {
    for (nombre, locks) in bloqueos_de_uno_en_uno() {
        let mut modal = Modal::default();
        assert!(!open_new_image(&mut modal, 16384, locks), "{nombre}");
        assert!(!modal.is_open(), "{nombre}");
    }
    let mut modal = Modal::default();
    assert!(open_new_image(&mut modal, 16384, Locks::default()));
}

#[test]
fn fila_9_quit_con_el_dialogo_abierto_lo_abandona_y_sigue_la_regla_de_cierre() {
    for modified in [false, true] {
        let mut mundo = Mundo::estandar();
        let mut modal = Modal::default();
        let mut confirm = Confirm::new();
        assert!(open_new_image(&mut modal, 16384, mundo.locks()));
        let locks = con_modal(&mundo);

        // `Quit` pasa con el diálogo abierto, con cualquier origen.
        for origin in ORIGENES {
            assert_eq!(
                mundo.despacha_con(AppAction::Quit, origin, locks),
                Dispatched::Quit,
                "{origin:?}"
            );
        }
        // `App` lo abandona sin reacción y pregunta a la regla de cierre.
        let antes = (mundo.foto(), extras(&mundo));
        assert!(modal.abandon());
        assert!(!modal.is_open());
        let cierra = close_allowed(modified, &mut confirm, 0.0);
        assert_eq!(cierra, !modified, "primera petición, modified {modified}");
        if modified {
            assert!(close_allowed(true, &mut confirm, 1.0), "la segunda cierra");
        }
        assert_eq!(
            (mundo.foto(), extras(&mundo)),
            antes,
            "no se creó nada: la sesión es la de antes"
        );
    }
}

// ---------------------------------------------------------------------------------------------
// Unidad 20: «Convertir modo» (specs/hito-2-convertir-modo.md, fila 10, parte de `dispatch`)
// ---------------------------------------------------------------------------------------------
//
// - `Dispatched::ConvertMode` desde el menú (la entrada `Imagen → Convertir modo…`) y el botón, con
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
// locks)` es la de «Piezas».

use crate::convert_mode::open_convert_mode;

/// Una sesión por modo de la imagen, sin quita pendiente.
fn mundos_de_cada_modo() -> Vec<(&'static str, Mundo)> {
    vec![
        ("indexada", Mundo::estandar()),
        ("RGBA con historia", Mundo::nuevo(rgba(), 3, 1)),
        ("gris", Mundo::nuevo(gris(), 2, 0)),
        (
            "gris con alfa sin historia",
            Mundo::nuevo(gris_alfa(), 0, 0),
        ),
    ]
}

fn modo_de(mundo: &Mundo) -> Mode {
    mundo.imagen().mode()
}

#[test]
fn fila_10_convertir_modo_da_dispatched_convert_mode_con_cada_origen_y_cada_modo_sin_tocar_nada() {
    for (nombre, mut mundo) in mundos_de_cada_modo() {
        for origin in ORIGENES {
            let antes = (mundo.foto(), extras(&mundo));
            let resultado = mundo.despacha(AppAction::ConvertMode, origin);
            assert_eq!(resultado, Dispatched::ConvertMode, "{nombre}, {origin:?}");
            assert!(
                !matches!(resultado, Dispatched::View(_)),
                "{nombre}: no es zoom"
            );
            assert_eq!(
                (mundo.foto(), extras(&mundo)),
                antes,
                "{nombre}, {origin:?}: despachar no toca nada (lo hace App)"
            );
        }
    }
}

#[test]
fn fila_10_la_entrada_del_menu_imagen_da_dispatched_convert_mode() {
    let menu = MENUS
        .iter()
        .find(|m| m.title == "Imagen")
        .expect("el menú tiene «Imagen»");
    let entrada = menu
        .items
        .iter()
        .find(|item| item.label == "Convertir modo…")
        .expect("Imagen tiene «Convertir modo…»");
    assert_eq!(entrada.action, AppAction::ConvertMode);
    for (nombre, mut mundo) in mundos_de_cada_modo() {
        let antes = mundo.foto();
        let por_menu = mundo.despacha(entrada.action, Origin::Menu);
        assert_eq!(por_menu, Dispatched::ConvertMode, "{nombre}");
        assert_eq!(mundo.foto(), antes, "{nombre}");
    }
    // El botón (la misma acción) da lo mismo.
    let mut por_menu = Mundo::estandar();
    let mut por_boton = Mundo::estandar();
    let a = por_menu.despacha(entrada.action, Origin::Menu);
    let b = por_boton.despacha(AppAction::ConvertMode, Origin::Button);
    assert_eq!(a, b);
    assert_eq!(por_menu.foto(), por_boton.foto());
}

#[test]
fn fila_10_ninguna_tecla_con_ninguna_combinacion_da_convertir_modo() {
    for plataforma in PLATAFORMAS_DE_LA_19 {
        for &tecla in Key::ALL {
            for bits in 0..32u8 {
                let m = modificadores_de_bits(bits);
                assert_ne!(
                    action_for_shortcut(tecla, m, plataforma),
                    Some(AppAction::ConvertMode),
                    "{plataforma:?}, {tecla:?}, {bits:05b}"
                );
            }
        }
    }
}

#[test]
fn fila_10_cada_bloqueo_bloquea_convertir_modo_con_cada_origen_y_cada_modo_sin_tocar_nada() {
    for (nombre_mundo, mut mundo) in mundos_de_cada_modo() {
        for (nombre, locks) in bloqueos_de_uno_en_uno() {
            for origin in ORIGENES {
                let antes = (mundo.foto(), extras(&mundo));
                assert_eq!(
                    mundo.despacha_con(AppAction::ConvertMode, origin, locks),
                    Dispatched::Blocked,
                    "{nombre_mundo}, {nombre}, {origin:?}"
                );
                assert!(
                    is_blocked(AppAction::ConvertMode, locks, origin),
                    "{nombre}"
                );
                assert_eq!(
                    (mundo.foto(), extras(&mundo)),
                    antes,
                    "{nombre_mundo}, {nombre}, {origin:?}: bloqueada no toca nada"
                );
            }
        }
    }
}

#[test]
fn fila_10_con_las_32_combinaciones_de_bloqueos_solo_pasa_sin_ninguno() {
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
            let esperado = if bits == 0 {
                Dispatched::ConvertMode
            } else {
                Dispatched::Blocked
            };
            assert_eq!(
                mundo.despacha_con(AppAction::ConvertMode, origin, locks),
                esperado,
                "{bits:05b}, {origin:?}"
            );
        }
    }
}

#[test]
fn fila_10_enabled_es_falso_exactamente_donde_dispatch_da_blocked_con_origen_menu() {
    for zoom in [None, Zoom::from_int(5)] {
        for bits in 0..32u8 {
            let locks = Locks {
                gesture: bits & 1 != 0,
                dialog: bits & 2 != 0,
                text_focus: bits & 4 != 0,
                removal: bits & 8 != 0,
                modal: bits & 16 != 0,
            };
            let mut mundo = Mundo::estandar();
            let estado = mundo.estado_de_la_ui(zoom, locks);
            let bloqueada = mundo.despacha_con(AppAction::ConvertMode, Origin::Menu, locks)
                == Dispatched::Blocked;
            assert_eq!(
                enabled(AppAction::ConvertMode, &estado),
                !bloqueada,
                "{bits:05b}, zoom {zoom:?}"
            );
        }
    }
}

#[test]
fn fila_10_un_gesto_de_verdad_o_una_quita_pendiente_bloquean_convertir_modo() {
    // Un trazo en curso.
    let mut mundo = Mundo::estandar();
    empieza_un_trazo(&mut mundo);
    let antes = mundo.foto();
    assert_eq!(
        mundo.despacha(AppAction::ConvertMode, Origin::Menu),
        Dispatched::Blocked,
        "trazo"
    );
    assert_eq!(mundo.foto(), antes);
    assert!(!enabled(
        AppAction::ConvertMode,
        &mundo.estado_de_la_ui(Zoom::from_int(5), mundo.locks())
    ));

    // El deslizador del editor.
    let mut mundo = Mundo::estandar();
    pulsa_el_deslizador(&mut mundo);
    assert_eq!(
        mundo.despacha(AppAction::ConvertMode, Origin::Button),
        Dispatched::Blocked,
        "deslizador"
    );

    // El arrastre de una muestra.
    let mut mundo = Mundo::estandar();
    arrastra_una_muestra(&mut mundo);
    assert_eq!(
        mundo.despacha(AppAction::ConvertMode, Origin::Menu),
        Dispatched::Blocked,
        "arrastre de muestra"
    );

    // Una quita de paleta pendiente: abrir un diálogo también se bloquea (regla literal de la 18).
    let mut mundo = mundo_con_quita_pendiente();
    assert!(mundo.locks().removal);
    let antes = (mundo.foto(), extras(&mundo));
    for origin in ORIGENES {
        assert_eq!(
            mundo.despacha(AppAction::ConvertMode, origin),
            Dispatched::Blocked,
            "quita pendiente, {origin:?}"
        );
    }
    assert_eq!((mundo.foto(), extras(&mundo)), antes);
    assert!(!enabled(
        AppAction::ConvertMode,
        &mundo.estado_de_la_ui(Zoom::from_int(5), mundo.locks())
    ));
}

#[test]
fn fila_10_con_el_dialogo_de_convertir_abierto_todo_esta_bloqueado_salvo_quit() {
    for (nombre_mundo, mut mundo) in mundos_de_cada_modo() {
        let mut modal = Modal::default();
        assert!(open_convert_mode(
            &mut modal,
            modo_de(&mundo),
            mundo.locks()
        ));
        assert_eq!(modal.kind(), Some(ModalKind::ConvertMode), "{nombre_mundo}");
        let locks = con_modal(&mundo);
        assert!(locks.modal);

        let antes = (mundo.foto(), extras(&mundo));
        for action in todas_las_acciones()
            .into_iter()
            .chain([AppAction::NewImage, AppAction::ConvertMode])
        {
            for origin in ORIGENES {
                let esperado = match action {
                    AppAction::Quit => Dispatched::Quit,
                    // `CloseLicenses` no es de este diálogo, pero la regla (1) la deja pasar siempre.
                    AppAction::CloseLicenses => Dispatched::Licenses(false),
                    _ => Dispatched::Blocked,
                };
                assert_eq!(
                    mundo.despacha_con(action, origin, locks),
                    esperado,
                    "{nombre_mundo}, {action:?}, {origin:?}"
                );
            }
        }
        assert_eq!(
            (mundo.foto(), extras(&mundo)),
            antes,
            "{nombre_mundo}: con el diálogo abierto el documento no cambia"
        );
    }
}

#[test]
fn fila_10_un_dialogo_abierto_no_deja_abrir_otro_ni_otro_convertir_modo() {
    let mundo = Mundo::estandar();
    let mut modal = Modal::default();
    assert!(open_convert_mode(&mut modal, Mode::Indexed, mundo.locks()));
    let antes = modal.clone();
    assert!(
        !open_convert_mode(&mut modal, Mode::Rgba, mundo.locks()),
        "ya hay uno abierto"
    );
    assert_eq!(modal, antes, "no cambia el diálogo abierto");
    assert!(!abre_licencias(&mut modal));
    assert!(!open_new_image(&mut modal, 16384, mundo.locks()));
    assert_eq!(modal.kind(), Some(ModalKind::ConvertMode));
}

#[test]
fn fila_10_cada_bloqueo_impide_abrir_el_dialogo_de_convertir_modo() {
    for modo in [Mode::Indexed, Mode::Rgba, Mode::GrayAlpha, Mode::Gray] {
        for (nombre, locks) in bloqueos_de_uno_en_uno() {
            let mut modal = Modal::default();
            assert!(
                !open_convert_mode(&mut modal, modo, locks),
                "{modo:?}, {nombre}"
            );
            assert!(!modal.is_open(), "{modo:?}, {nombre}");
        }
        let mut modal = Modal::default();
        assert!(open_convert_mode(&mut modal, modo, Locks::default()));
    }
}

#[test]
fn fila_10_quit_con_el_dialogo_abierto_lo_abandona_y_sigue_la_regla_de_cierre() {
    for modified in [false, true] {
        let mut mundo = Mundo::estandar();
        let mut modal = Modal::default();
        let mut confirm = Confirm::new();
        assert!(open_convert_mode(&mut modal, Mode::Indexed, mundo.locks()));
        let locks = con_modal(&mundo);

        // `Quit` pasa con el diálogo abierto, con cualquier origen.
        for origin in ORIGENES {
            assert_eq!(
                mundo.despacha_con(AppAction::Quit, origin, locks),
                Dispatched::Quit,
                "{origin:?}"
            );
        }
        // `App` lo abandona sin reacción y pregunta a la regla de cierre.
        let antes = (mundo.foto(), extras(&mundo));
        assert!(modal.abandon());
        assert!(!modal.is_open());
        let cierra = close_allowed(modified, &mut confirm, 0.0);
        assert_eq!(cierra, !modified, "primera petición, modified {modified}");
        if modified {
            assert!(close_allowed(true, &mut confirm, 1.0), "la segunda cierra");
        }
        assert_eq!(
            (mundo.foto(), extras(&mundo)),
            antes,
            "no se convirtió nada: la sesión es la de antes"
        );
    }
}

#[test]
fn fila_10_el_zoom_no_cambia_con_la_accion_nueva() {
    // Los cuatro zooms siguen dando `View` con cualquier origen salvo `text_focus` desde el teclado,
    // y no los paran ni un gesto ni una quita pendiente (sí paran `ConvertMode`).
    for (accion, op) in ZOOMS {
        for origin in ORIGENES {
            let mut mundo = Mundo::estandar();
            assert_eq!(
                mundo.despacha_con(accion, origin, locks_con(|l| l.removal = true)),
                Dispatched::View(op),
                "{accion:?}, {origin:?}"
            );
            assert_eq!(
                mundo.despacha_con(accion, origin, locks_con(|l| l.gesture = true)),
                Dispatched::View(op),
                "{accion:?}, {origin:?}"
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
//   cuatro fijas (`Negro`...) siguen dando `Dispatched::Background`.
// - Con el diálogo de color abierto todo está bloqueado salvo `Quit`, que lo abandona sin aplicar
//   nada y sigue la regla de cierre de siempre.
//
// Suposiciones: `Dispatched::BackgroundColor` es una variante sin datos (`Clone, Debug, PartialEq`
// ya los tiene `Dispatched`); `open_background_color(modal, current, locks) -> bool` es la de «Piezas»;
// `DialogKind::BackgroundColor` es el tipo del diálogo.

use crate::background_color::open_background_color;

/// Un fondo activo de cada clase, para abrir el diálogo desde cada uno.
const FONDOS_DE_PARTIDA: [CanvasBackground; 5] = [
    CanvasBackground::Checker,
    CanvasBackground::Black,
    CanvasBackground::Gray,
    CanvasBackground::White,
    CanvasBackground::Custom(Rgba::opaque(0x11, 0x22, 0x33)),
];

#[test]
fn fila_3_color_da_dispatched_background_color_con_cada_origen_y_cada_modo_sin_tocar_nada() {
    let mut mundos = mundos_de_cada_modo();
    mundos.extend(mundos_sin_quita());
    for (nombre, mut mundo) in mundos {
        for origin in ORIGENES {
            let antes = (mundo.foto(), extras(&mundo));
            let resultado = mundo.despacha(AppAction::BackgroundColor, origin);
            assert_eq!(
                resultado,
                Dispatched::BackgroundColor,
                "{nombre}, {origin:?}"
            );
            assert!(
                !matches!(
                    resultado,
                    Dispatched::View(_) | Dispatched::Background(_) | Dispatched::Done { .. }
                ),
                "{nombre}: no es zoom, ni uno de los fondos fijos, ni una acción de documento"
            );
            assert_eq!(
                (mundo.foto(), extras(&mundo)),
                antes,
                "{nombre}, {origin:?}: despachar no toca nada (lo hace App)"
            );
        }
    }
}

#[test]
fn fila_3_la_entrada_color_del_submenu_da_dispatched_background_color() {
    let submenu = MENUS
        .iter()
        .find(|m| m.title == "Ver")
        .expect("el menú tiene «Ver»")
        .submenus
        .first()
        .expect("Ver tiene «Fondo del lienzo»");
    let entrada = submenu
        .items
        .iter()
        .find(|item| item.label == "Color…")
        .expect("el submenú tiene «Color…»");
    assert_eq!(entrada.action, AppAction::BackgroundColor);
    let mut por_menu = Mundo::estandar();
    let mut por_boton = Mundo::estandar();
    let a = por_menu.despacha(entrada.action, Origin::Menu);
    let b = por_boton.despacha(AppAction::BackgroundColor, Origin::Button);
    assert_eq!(a, Dispatched::BackgroundColor);
    assert_eq!(a, b);
    assert_eq!(por_menu.foto(), por_boton.foto());
}

#[test]
fn fila_3_ninguna_tecla_con_ninguna_combinacion_da_background_color() {
    for plataforma in PLATAFORMAS_DE_LA_19 {
        for &tecla in Key::ALL {
            for bits in 0..32u8 {
                let m = modificadores_de_bits(bits);
                assert_ne!(
                    action_for_shortcut(tecla, m, plataforma),
                    Some(AppAction::BackgroundColor),
                    "{plataforma:?}, {tecla:?}, {bits:05b}"
                );
            }
        }
    }
}

#[test]
fn fila_3_cada_bloqueo_bloquea_background_color_con_cada_origen_y_cada_modo_sin_tocar_nada() {
    let mut mundos = mundos_de_cada_modo();
    mundos.push(("con una quita pendiente", mundo_con_quita_pendiente()));
    for (nombre_mundo, mut mundo) in mundos {
        for (nombre, locks) in bloqueos_de_uno_en_uno() {
            for origin in ORIGENES {
                let antes = (mundo.foto(), extras(&mundo));
                assert_eq!(
                    mundo.despacha_con(AppAction::BackgroundColor, origin, locks),
                    Dispatched::Blocked,
                    "{nombre_mundo}, {nombre}, {origin:?}"
                );
                assert!(
                    is_blocked(AppAction::BackgroundColor, locks, origin),
                    "{nombre}"
                );
                assert_eq!(
                    (mundo.foto(), extras(&mundo)),
                    antes,
                    "{nombre_mundo}, {nombre}, {origin:?}: bloqueada no toca nada"
                );
            }
        }
    }
}

#[test]
fn fila_3_con_las_32_combinaciones_de_bloqueos_solo_pasa_sin_ninguno() {
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
            let esperado = if bits == 0 {
                Dispatched::BackgroundColor
            } else {
                Dispatched::Blocked
            };
            assert_eq!(
                mundo.despacha_con(AppAction::BackgroundColor, origin, locks),
                esperado,
                "{bits:05b}, {origin:?}"
            );
        }
    }
}

#[test]
fn fila_3_enabled_de_background_color_es_falso_exactamente_donde_dispatch_da_blocked() {
    for zoom in [None, Zoom::from_int(5)] {
        for bits in 0..32u8 {
            let locks = Locks {
                gesture: bits & 1 != 0,
                dialog: bits & 2 != 0,
                text_focus: bits & 4 != 0,
                removal: bits & 8 != 0,
                modal: bits & 16 != 0,
            };
            let mut mundo = Mundo::estandar();
            let estado = mundo.estado_de_la_ui(zoom, locks);
            let bloqueada = mundo.despacha_con(AppAction::BackgroundColor, Origin::Menu, locks)
                == Dispatched::Blocked;
            assert_eq!(
                enabled(AppAction::BackgroundColor, &estado),
                !bloqueada,
                "{bits:05b}, zoom {zoom:?}"
            );
        }
    }
}

#[test]
fn fila_3_un_gesto_de_verdad_o_una_quita_pendiente_bloquean_color_y_no_las_cuatro_fijas() {
    let cuatro = [
        CanvasBackground::Checker,
        CanvasBackground::Black,
        CanvasBackground::Gray,
        CanvasBackground::White,
    ];
    let comprueba = |nombre: &str, mundo: &mut Mundo| {
        for origin in ORIGENES {
            let antes = (mundo.foto(), extras(mundo));
            assert_eq!(
                mundo.despacha(AppAction::BackgroundColor, origin),
                Dispatched::Blocked,
                "{nombre}, {origin:?}: `Color…` bloqueada"
            );
            assert!(!enabled(
                AppAction::BackgroundColor,
                &mundo.estado_de_la_ui(Zoom::from_int(5), mundo.locks())
            ));
            // Las cuatro fijas pasan con el mismo gesto o la misma quita.
            for fondo in cuatro {
                assert_eq!(
                    mundo.despacha(AppAction::CanvasBackground(fondo), origin),
                    Dispatched::Background(fondo),
                    "{nombre}, {origin:?}: {fondo:?} sigue habilitada"
                );
                assert!(enabled(
                    AppAction::CanvasBackground(fondo),
                    &mundo.estado_de_la_ui(Zoom::from_int(5), mundo.locks())
                ));
            }
            assert_eq!(
                (mundo.foto(), extras(mundo)),
                antes,
                "{nombre}, {origin:?}: nada cambia"
            );
        }
    };
    // Un trazo en curso.
    let mut mundo = Mundo::estandar();
    empieza_un_trazo(&mut mundo);
    comprueba("trazo", &mut mundo);
    // El deslizador del editor.
    let mut mundo = Mundo::estandar();
    pulsa_el_deslizador(&mut mundo);
    comprueba("deslizador", &mut mundo);
    // El arrastre de una muestra.
    let mut mundo = Mundo::estandar();
    arrastra_una_muestra(&mut mundo);
    comprueba("arrastre de muestra", &mut mundo);
    // Una quita de paleta pendiente.
    let mut mundo = mundo_con_quita_pendiente();
    assert!(mundo.locks().removal);
    comprueba("quita pendiente", &mut mundo);
}

#[test]
fn fila_3_con_el_dialogo_de_color_abierto_todo_esta_bloqueado_salvo_quit() {
    for fondo in FONDOS_DE_PARTIDA {
        for (nombre_mundo, mut mundo) in mundos_de_cada_modo() {
            let mut modal = Modal::default();
            assert!(open_background_color(&mut modal, fondo, mundo.locks()));
            assert_eq!(
                modal.kind(),
                Some(ModalKind::BackgroundColor),
                "{nombre_mundo}"
            );
            let locks = con_modal(&mundo);
            assert!(locks.modal);
            let antes = (mundo.foto(), extras(&mundo));
            for action in todas_las_acciones()
                .into_iter()
                .chain(FONDOS.map(accion_de_fondo))
                .chain([
                    AppAction::NewImage,
                    AppAction::ConvertMode,
                    AppAction::BackgroundColor,
                    accion_de_fondo(CanvasBackground::Custom(Rgba::opaque(1, 2, 3))),
                ])
            {
                for origin in ORIGENES {
                    let esperado = match action {
                        AppAction::Quit => Dispatched::Quit,
                        AppAction::CloseLicenses => Dispatched::Licenses(false),
                        _ => Dispatched::Blocked,
                    };
                    assert_eq!(
                        mundo.despacha_con(action, origin, locks),
                        esperado,
                        "{nombre_mundo}, {fondo:?}, {action:?}, {origin:?}"
                    );
                }
            }
            assert_eq!(
                (mundo.foto(), extras(&mundo)),
                antes,
                "{nombre_mundo}: con el diálogo abierto el documento no cambia"
            );
        }
    }
}

#[test]
fn fila_3_un_dialogo_abierto_no_deja_abrir_otro_ni_otro_color() {
    let mundo = Mundo::estandar();
    let mut modal = Modal::default();
    assert!(open_background_color(
        &mut modal,
        CanvasBackground::Black,
        mundo.locks()
    ));
    let antes = modal.clone();
    assert!(
        !open_background_color(&mut modal, CanvasBackground::White, mundo.locks()),
        "ya hay uno abierto"
    );
    assert_eq!(modal, antes, "no cambia el diálogo abierto");
    assert!(!abre_licencias(&mut modal));
    assert!(!open_new_image(&mut modal, 16384, mundo.locks()));
    assert!(!open_convert_mode(&mut modal, Mode::Indexed, mundo.locks()));
    assert_eq!(modal.kind(), Some(ModalKind::BackgroundColor));
    // Y al revés: con otro diálogo abierto no se abre el de color.
    let mut otro = Modal::default();
    assert!(abre_licencias(&mut otro));
    assert!(!open_background_color(
        &mut otro,
        CanvasBackground::Checker,
        Locks::default()
    ));
    assert_eq!(otro.kind(), Some(ModalKind::Licenses));
}

#[test]
fn fila_3_quit_con_el_dialogo_de_color_abierto_lo_abandona_y_sigue_la_regla_de_cierre() {
    for modified in [false, true] {
        let mut mundo = Mundo::estandar();
        let mut modal = Modal::default();
        let mut confirm = Confirm::new();
        assert!(open_background_color(
            &mut modal,
            CanvasBackground::Custom(Rgba::opaque(0x11, 0x22, 0x33)),
            mundo.locks()
        ));
        let locks = con_modal(&mundo);

        // `Quit` pasa con el diálogo abierto, con cualquier origen.
        for origin in ORIGENES {
            assert_eq!(
                mundo.despacha_con(AppAction::Quit, origin, locks),
                Dispatched::Quit,
                "{origin:?}"
            );
        }
        // `App` lo abandona sin reacción y pregunta a la regla de cierre.
        let antes = (mundo.foto(), extras(&mundo));
        assert!(modal.abandon());
        assert!(!modal.is_open());
        let cierra = close_allowed(modified, &mut confirm, 0.0);
        assert_eq!(cierra, !modified, "primera petición, modified {modified}");
        if modified {
            assert!(close_allowed(true, &mut confirm, 1.0), "la segunda cierra");
        }
        assert_eq!(
            (mundo.foto(), extras(&mundo)),
            antes,
            "no se aplicó nada: la sesión es la de antes"
        );
    }
}
