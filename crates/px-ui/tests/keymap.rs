//! Tests de la tabla de teclas `px_ui::keymap` y de lo que la lee (specs/interfaz-teclado.md,
//! unidad I.1). En ROJO hasta la lógica: `keymap`, `key_events`, `shortcut_in`, `show_menu_in`,
//! `show_toolbar_with` y las acciones nuevas no existen todavía.
//!
//! Filas y propiedades de la spec que cubre este fichero:
//!
//! - Fila 1: las 25 filas de `Keymap::default_scheme()` (acción, tecla, `command`, `shift`,
//!   `active`, `shown`) en el orden de la spec; 25 activas en Linux y 24 en Windows y macOS;
//!   `conflicts()` vacío; derives.
//! - Fila 2: asignadas y sin asignar, con `shift`, `alt`, `command` y `other_ctrl`, en las tres
//!   plataformas, sobre `Press` construidas a mano.
//! - Fila 3: los nueve modernos intactos contra un oráculo escrito a mano (el de las funciones
//!   de antes de la unidad).
//! - Fila 5: `shown`, `shown_text`, `TOOL_BUTTONS`, `MENUS` con `shortcut_for`/`shortcut_text`,
//!   `popup_width_u` (los anchos no cambian) y ningún rótulo pierde un carácter.
//! - Fila 8 (parte de px-ui, módulo `fila_8`): con un diálogo o el panel modal abierto toda fila
//!   da `Blocked`; `gate_keys` con un campo de texto enfocado de verdad; `Inicio`, `Fin`,
//!   `Re Pág` y `Av Pág` siguen llegando al diálogo (banco con `show_dialog`).
//! - Fila 10 (parte de px-ui): las cifras 1 a 9 sin modificadores no hacen nada y las cuatro
//!   funciones antiguas (`tool_action_for_key`, `action_for_key`, `file_action_for_key`,
//!   `action_for_shortcut`) dan lo mismo que la tabla sobre `Key::ALL` × 32 combinaciones × tres
//!   plataformas con `press_from_key`.
//! - Fila 11: tabla sustituible (`Keymap::new`, `shortcut_in`, `show_menu_in`, `show_toolbar_with`
//!   en un banco de egui sin ventana) y `conflicts()`.
//! - Propiedades 1 a 3 (proptest, 256 casos).
//!
//! Suposiciones sobre lo que la spec no fija (anotadas en el informe):
//! - `Escape` y `Espacio` (no asignables) se representan en las pruebas de `conflicts()` como
//!   `KeyId::Char('\u{1b}')` y `KeyId::Char(' ')`; `Ctrl+C/X/V/A` como `Char('c' | 'x' | 'v' | 'a')`
//!   con `command`. `Conflict::Duplicate(i, j)` va con `i < j`.
//! - `Platforms` no tiene variante para «ninguna»: las filas cuyo `shown` es «ninguna» (`u`,
//!   `Re Pág`, `Av Pág`, `+`, `=` y `-`) no se comparan por el campo `shown`, sino por lo que dice
//!   `Keymap::shown` (la fila mostrada es la otra).
//! - **`other_ctrl` en símbolos y teclas con nombre.** La decisión 1 dice que no cuenta; la
//!   fila 2 dice que da `None`. Aquí manda la decisión 1 (las pruebas anteriores de
//!   `action_for_shortcut` exigen `Inicio` con `Ctrl` suelto = `ZoomFit`): `other_ctrl` no cambia
//!   el resultado de `, ] [ < > + = -`, `Inicio`, `Re Pág` y `Av Pág`.
//! - Una `Press` con `command` y `other_ctrl` a la vez no existe (`other_ctrl` es `ctrl` sin
//!   `command`): las pruebas no la generan.
//! - `Alt` con `command` anula la fila (como las funciones de antes: `Ctrl+Alt+Z` = `None`).
//! - Fila 10: en la comparación con la tabla se saltan las combinaciones con `mac_cmd` sin
//!   `command` (no existen en egui) y, solo para `action_for_key`, `Re Pág`/`Av Pág` con `ctrl`
//!   sin `command` (las pruebas anteriores de `shortcuts.rs` dan `None` ahí y las de `actions.rs`
//!   dan `ColorNext`/`ColorPrev` para `action_for_shortcut`).

use egui::{Context, Event, Key, Modifiers, Pos2, RawInput, Rect, TextEdit, Ui, Vec2, vec2};
use proptest::prelude::*;
use px_ui::actions::{
    AppAction, Locks, Origin, Platform, UiState, action_for_shortcut, is_blocked,
};
use px_ui::background::CanvasBackground;
use px_ui::key_events::{press_from_key, presses};
use px_ui::keymap::{Binding, Chord, Conflict, KeyId, Keymap, Platforms, Press};
use px_ui::menu::{
    MENUS, Shortcut, popup_width_u, shortcut_for, shortcut_in, shortcut_text, show_menu_in,
};
use px_ui::shortcuts::{Action, FileAction, action_for_key, file_action_for_key, gate_keys};
use px_ui::text::{INTERFACE_CHARSET, apply_scale, printable, replace_missing};
use px_ui::theme::visuals;
use px_ui::tool::{Tool, ToolAction, tool_action_for_key};
use px_ui::toolbar::{TOOL_BUTTONS, ZOOM_BUTTONS, show_toolbar_with};
use px_ui::view::Zoom;

mod common;
use common::{Pintado, boton, contexto, en_puntos, modificadores, mover, n_de, pintado};

const PLATAFORMAS: [Platform; 3] = [Platform::Linux, Platform::Windows, Platform::Mac];
const HERRAMIENTAS: [Tool; 5] = [
    Tool::Pencil,
    Tool::Line,
    Tool::Fill,
    Tool::Dropper,
    Tool::Eraser,
];
const FONDOS: [CanvasBackground; 4] = [
    CanvasBackground::Checker,
    CanvasBackground::Black,
    CanvasBackground::Gray,
    CanvasBackground::White,
];

// ---------------------------------------------------------------------------------------------
// Ayudantes
// ---------------------------------------------------------------------------------------------

fn c(ch: char) -> KeyId {
    KeyId::Char(ch)
}

fn press(key: KeyId, command: bool, shift: bool, alt: bool, other_ctrl: bool) -> Press {
    Press {
        key: Some(key),
        command,
        shift,
        alt,
        other_ctrl,
        repeat: false,
    }
}

/// La tecla sola, sin ningún modificador.
fn sola(key: KeyId) -> Press {
    press(key, false, false, false, false)
}

/// Con `Ctrl`/`Cmd` (`command`), con o sin `Mayús` y `Alt`.
fn con_command(key: KeyId, shift: bool, alt: bool) -> Press {
    press(key, true, shift, alt, false)
}

/// Si la fila (o el atajo) vale en `platform`. El comodín cubre una variante más de `Platforms`.
#[allow(unreachable_patterns)]
fn vale_en(platforms: Platforms, platform: Platform) -> bool {
    match platforms {
        Platforms::All => true,
        Platforms::Only(p) => p == platform,
        Platforms::Except(p) => p != platform,
        _ => false,
    }
}

fn por_defecto() -> Keymap {
    Keymap::default_scheme()
}

fn accion(km: &Keymap, press: &Press, platform: Platform) -> Option<AppAction> {
    km.action_for(press, platform)
}

/// Todas las acciones posibles (las que existen tras la unidad), con las ranuras 1 a 9 y los
/// cuatro fondos fijos.
fn todas_las_acciones() -> Vec<AppAction> {
    let mut v = vec![
        AppAction::NewImage,
        AppAction::ConvertMode,
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
        AppAction::BackgroundColor,
        AppAction::ColorNext,
        AppAction::ColorPrev,
        AppAction::CenterOnCursor,
    ];
    v.extend(HERRAMIENTAS.map(AppAction::SelectTool));
    v.extend((1..=9).map(AppAction::SelectColor));
    v.extend(FONDOS.map(AppAction::CanvasBackground));
    v
}

fn assert_dato<T: Clone + std::fmt::Debug + PartialEq>() {}
fn assert_plano<T: Clone + Copy + std::fmt::Debug + PartialEq + Eq>() {}
fn assert_eq<T: Eq>() {}

fn fila(key: KeyId, command: bool, shift: bool, action: AppAction) -> Binding {
    Binding {
        key,
        command,
        shift,
        action,
        active: Platforms::All,
        shown: Platforms::All,
    }
}

// ---------------------------------------------------------------------------------------------
// Fila 1: la tabla por defecto
// ---------------------------------------------------------------------------------------------

/// Una fila esperada. `shown: None` = «ninguna» (no representable en `Platforms`, véase la
/// cabecera): no se compara el campo.
struct Esperada {
    action: AppAction,
    key: KeyId,
    command: bool,
    shift: bool,
    active: Platforms,
    shown: Option<Platforms>,
}

fn e(
    action: AppAction,
    key: KeyId,
    command: bool,
    shift: bool,
    active: Platforms,
    shown: Option<Platforms>,
) -> Esperada {
    Esperada {
        action,
        key,
        command,
        shift,
        active,
        shown,
    }
}

/// Las 30 filas del «Esquema por defecto» (25 de I.1 y 5 de I.2a), en el orden de la spec (dentro de cada fila doble, la
/// siguiente antes que la anterior; `Av Pág` y `Re Pág` después de `]` y `[`).
fn esquema_esperado() -> Vec<Esperada> {
    let todas = Platforms::All;
    let t = Some(Platforms::All);
    let lin = Platforms::Only(Platform::Linux);
    vec![
        e(
            AppAction::SelectTool(Tool::Pencil),
            c('d'),
            false,
            false,
            todas,
            t,
        ),
        e(
            AppAction::SelectTool(Tool::Line),
            c('v'),
            false,
            false,
            todas,
            t,
        ),
        e(
            AppAction::SelectTool(Tool::Fill),
            c('f'),
            false,
            false,
            todas,
            t,
        ),
        e(
            AppAction::SelectTool(Tool::Dropper),
            c(','),
            false,
            false,
            todas,
            t,
        ),
        e(AppAction::Undo, c('z'), true, false, todas, t),
        e(AppAction::Undo, c('u'), false, false, todas, None),
        e(
            AppAction::Redo,
            c('y'),
            true,
            false,
            todas,
            Some(Platforms::Except(Platform::Mac)),
        ),
        e(
            AppAction::Redo,
            c('z'),
            true,
            true,
            todas,
            Some(Platforms::Only(Platform::Mac)),
        ),
        e(AppAction::ColorNext, c(']'), false, false, todas, t),
        e(AppAction::ColorPrev, c('['), false, false, todas, t),
        e(
            AppAction::ColorNext,
            KeyId::PageDown,
            false,
            false,
            todas,
            None,
        ),
        e(
            AppAction::ColorPrev,
            KeyId::PageUp,
            false,
            false,
            todas,
            None,
        ),
        e(
            AppAction::BackgroundNext,
            KeyId::PageDown,
            false,
            true,
            todas,
            None,
        ),
        e(
            AppAction::BackgroundPrev,
            KeyId::PageUp,
            false,
            true,
            todas,
            None,
        ),
        e(AppAction::BackgroundNext, c('}'), false, false, todas, t),
        e(AppAction::BackgroundPrev, c('{'), false, false, todas, t),
        e(AppAction::ZoomIn, c('>'), false, false, todas, t),
        e(AppAction::ZoomIn, c('+'), false, false, todas, None),
        e(AppAction::ZoomIn, c('='), false, false, todas, None),
        e(AppAction::ZoomOut, c('<'), false, false, todas, t),
        e(AppAction::ZoomOut, c('-'), false, false, todas, None),
        e(AppAction::ZoomFit, KeyId::Home, false, false, todas, t),
        e(AppAction::ZoomActual, c('1'), true, false, todas, t),
        e(AppAction::CenterOnCursor, c('n'), false, false, todas, t),
        e(AppAction::ClearPage, c('k'), false, true, todas, t),
        e(AppAction::Open, c('o'), true, false, todas, t),
        e(AppAction::Save, c('s'), true, false, todas, t),
        e(AppAction::SaveAs, c('s'), true, true, todas, t),
        e(AppAction::NewImage, c('n'), true, false, todas, t),
        e(AppAction::Quit, c('q'), true, false, lin, Some(lin)),
    ]
}

#[test]
fn fila_1_los_tipos_de_la_tabla_tienen_los_derives_de_la_spec() {
    assert_plano::<KeyId>();
    assert_plano::<Platforms>();
    assert_plano::<Chord>();
    assert_plano::<Press>();
    assert_dato::<Conflict>();
    assert_dato::<Binding>();
    assert_dato::<Keymap>();
    // `Conflict` es `Eq` y `Press` tiene todos sus campos públicos (se construye a mano arriba).
    assert_eq::<Conflict>();
    // `Chord` es lo que antes era `menu::Shortcut`: el mismo tipo.
    let chord = Chord {
        key: c('z'),
        command: true,
        shift: false,
    };
    let atajo: Shortcut = chord;
    assert_eq!(atajo, chord);
}

#[test]
fn fila_1_la_tabla_por_defecto_son_las_30_filas_de_la_spec_en_orden() {
    let km = por_defecto();
    let esperado = esquema_esperado();
    assert_eq!(esperado.len(), 30);
    assert_eq!(km.rows().len(), 30, "filas de la tabla por defecto");
    for (i, (real, esperada)) in km.rows().iter().zip(&esperado).enumerate() {
        assert_eq!(real.action, esperada.action, "fila {i}: acción");
        assert_eq!(
            real.key, esperada.key,
            "fila {i} ({:?}): tecla",
            real.action
        );
        assert_eq!(
            real.command, esperada.command,
            "fila {i} ({:?}): command",
            real.action
        );
        assert_eq!(
            real.shift, esperada.shift,
            "fila {i} ({:?}): shift",
            real.action
        );
        assert_eq!(
            real.active, esperada.active,
            "fila {i} ({:?}): active",
            real.action
        );
        if let Some(shown) = esperada.shown {
            assert_eq!(
                real.shown, shown,
                "fila {i} ({:?}, {:?}): shown",
                real.action, real.key
            );
        }
    }
}

#[test]
fn fila_1_hay_21_filas_sin_command_y_9_con_el_y_quit_solo_en_linux() {
    let km = por_defecto();
    let sin = km.rows().iter().filter(|f| !f.command).count();
    let con = km.rows().iter().filter(|f| f.command).count();
    assert_eq!((sin, con), (21, 9));
    let quit: Vec<&Binding> = km
        .rows()
        .iter()
        .filter(|f| f.action == AppAction::Quit)
        .collect();
    assert_eq!(quit.len(), 1);
    assert_eq!(quit[0].active, Platforms::Only(Platform::Linux));
}

#[test]
fn fila_1_hay_30_filas_activas_en_linux_y_29_en_windows_y_macos() {
    let km = por_defecto();
    for (platform, esperado) in [
        (Platform::Linux, 30),
        (Platform::Windows, 29),
        (Platform::Mac, 29),
    ] {
        let activas = km
            .rows()
            .iter()
            .filter(|f| vale_en(f.active, platform))
            .count();
        assert_eq!(activas, esperado, "filas activas en {platform:?}");
    }
}

#[test]
fn fila_1_la_tabla_por_defecto_no_tiene_conflictos() {
    assert_eq!(por_defecto().conflicts(), vec![]);
}

#[test]
fn fila_1_new_con_las_filas_de_la_tabla_da_una_tabla_igual() {
    let km = por_defecto();
    let copia = Keymap::new(km.rows().to_vec());
    assert_eq!(copia, km);
    assert_eq!(copia.rows(), km.rows());
    assert_eq!(km.clone(), km);
}

#[test]
fn fila_1_las_filas_donde_shown_no_es_ninguna_no_son_la_que_muestra_shown() {
    // `u`, `Av Pág`, `Re Pág`, `+`, `=` y `-` funcionan pero no se muestran: la fila mostrada es
    // la otra (`Ctrl+Z`, `]`, `[`, `>`, `<`).
    let km = por_defecto();
    for platform in PLATAFORMAS {
        let chord = |a| km.shown(a, platform);
        assert_eq!(
            chord(AppAction::Undo),
            Some(Chord {
                key: c('z'),
                command: true,
                shift: false
            }),
            "{platform:?}"
        );
        assert_eq!(
            chord(AppAction::ColorNext),
            Some(Chord {
                key: c(']'),
                command: false,
                shift: false
            }),
            "{platform:?}: `Av Pág` no se muestra"
        );
        assert_eq!(
            chord(AppAction::ColorPrev),
            Some(Chord {
                key: c('['),
                command: false,
                shift: false
            }),
            "{platform:?}: `Re Pág` no se muestra"
        );
        assert_eq!(
            chord(AppAction::ZoomIn),
            Some(Chord {
                key: c('>'),
                command: false,
                shift: false
            }),
            "{platform:?}: `+` y `=` no se muestran"
        );
        assert_eq!(
            chord(AppAction::ZoomOut),
            Some(Chord {
                key: c('<'),
                command: false,
                shift: false
            }),
            "{platform:?}: `-` no se muestra"
        );
    }
}

#[test]
fn fila_1_shown_de_rehacer_y_de_salir_depende_de_la_plataforma() {
    let km = por_defecto();
    for platform in PLATAFORMAS {
        let redo = if platform == Platform::Mac {
            Chord {
                key: c('z'),
                command: true,
                shift: true,
            }
        } else {
            Chord {
                key: c('y'),
                command: true,
                shift: false,
            }
        };
        assert_eq!(
            km.shown(AppAction::Redo, platform),
            Some(redo),
            "{platform:?}"
        );
        let quit = (platform == Platform::Linux).then_some(Chord {
            key: c('q'),
            command: true,
            shift: false,
        });
        assert_eq!(km.shown(AppAction::Quit, platform), quit, "{platform:?}");
    }
}

// ---------------------------------------------------------------------------------------------
// Fila 2: asignadas y sin asignar
// ---------------------------------------------------------------------------------------------

/// Las teclas sin `command` asignadas, con su acción.
fn asignadas_sin_command() -> Vec<(KeyId, AppAction)> {
    vec![
        (c('d'), AppAction::SelectTool(Tool::Pencil)),
        (c('v'), AppAction::SelectTool(Tool::Line)),
        (c('f'), AppAction::SelectTool(Tool::Fill)),
        (c(','), AppAction::SelectTool(Tool::Dropper)),
        (c('u'), AppAction::Undo),
        (c('n'), AppAction::CenterOnCursor),
        (c(']'), AppAction::ColorNext),
        (c('['), AppAction::ColorPrev),
        (c('}'), AppAction::BackgroundNext),
        (c('{'), AppAction::BackgroundPrev),
        (c('<'), AppAction::ZoomOut),
        (c('>'), AppAction::ZoomIn),
        (c('+'), AppAction::ZoomIn),
        (c('='), AppAction::ZoomIn),
        (c('-'), AppAction::ZoomOut),
        (KeyId::Home, AppAction::ZoomFit),
        (KeyId::PageUp, AppAction::ColorPrev),
        (KeyId::PageDown, AppAction::ColorNext),
    ]
}

fn es_letra(key: KeyId) -> bool {
    matches!(key, KeyId::Char(ch) if ch.is_alphabetic())
}

#[test]
fn fila_2_cada_tecla_asignada_sin_modificadores_da_su_accion_en_las_tres_plataformas() {
    let km = por_defecto();
    for platform in PLATAFORMAS {
        for (key, esperada) in asignadas_sin_command() {
            assert_eq!(
                accion(&km, &sola(key), platform),
                Some(esperada),
                "{key:?} sola en {platform:?}"
            );
        }
    }
}

#[test]
fn fila_2_en_letras_shift_alt_command_y_other_ctrl_dan_none_salvo_ctrl_n() {
    let km = por_defecto();
    for platform in PLATAFORMAS {
        for key in ['d', 'v', 'f', 'u', 'n'] {
            let k = c(key);
            for (nombre, p) in [
                ("shift", press(k, false, true, false, false)),
                ("alt", press(k, false, false, true, false)),
                ("shift y alt", press(k, false, true, true, false)),
                ("other_ctrl", press(k, false, false, false, true)),
                ("shift y other_ctrl", press(k, false, true, false, true)),
                ("alt y other_ctrl", press(k, false, false, true, true)),
                ("command", con_command(k, false, false)),
                ("command y shift", con_command(k, true, false)),
                ("command y alt", con_command(k, false, true)),
            ] {
                let esperado = if key == 'n' && nombre == "command" {
                    Some(AppAction::NewImage)
                } else {
                    None
                };
                assert_eq!(
                    accion(&km, &p, platform),
                    esperado,
                    "`{key}` con {nombre} en {platform:?}"
                );
            }
        }
    }
}

#[test]
fn fila_2_alt_mas_una_letra_no_es_esa_letra_y_no_hace_nada() {
    // Decisión 1: en macOS `Option`+`d` escribe `∂`; en Linux y Windows `Alt`+`d` queda sin acción.
    let km = por_defecto();
    for platform in PLATAFORMAS {
        for (key, _) in asignadas_sin_command()
            .into_iter()
            .filter(|(k, _)| es_letra(*k))
        {
            assert_eq!(
                accion(&km, &press(key, false, false, true, false), platform),
                None,
                "Alt+{key:?} en {platform:?}"
            );
        }
    }
}

#[test]
fn fila_2_en_simbolos_y_teclas_con_nombre_shift_y_alt_no_cambian_el_resultado() {
    let km = por_defecto();
    for platform in PLATAFORMAS {
        for (key, esperada) in asignadas_sin_command()
            .into_iter()
            .filter(|(k, _)| !es_letra(*k))
        {
            for (shift, alt) in [(false, false), (true, false), (false, true), (true, true)] {
                // I.2a: en `Re Pág` y `Av Pág` `Mayús` cuenta (con ella son el fondo); `Alt` no.
                let esperada = match (key, shift) {
                    (KeyId::PageUp, true) => AppAction::BackgroundPrev,
                    (KeyId::PageDown, true) => AppAction::BackgroundNext,
                    _ => esperada,
                };
                assert_eq!(
                    accion(&km, &press(key, false, shift, alt, false), platform),
                    Some(esperada),
                    "{key:?} con shift={shift} alt={alt} en {platform:?}"
                );
            }
        }
    }
}

#[test]
fn fila_2_en_simbolos_y_teclas_con_nombre_command_da_none() {
    let km = por_defecto();
    for platform in PLATAFORMAS {
        for (key, _) in asignadas_sin_command()
            .into_iter()
            .filter(|(k, _)| !es_letra(*k))
        {
            for (shift, alt) in [(false, false), (true, false), (false, true), (true, true)] {
                assert_eq!(
                    accion(&km, &con_command(key, shift, alt), platform),
                    None,
                    "Ctrl+{key:?} con shift={shift} alt={alt} en {platform:?}"
                );
            }
        }
    }
}

#[test]
fn fila_2_en_simbolos_y_teclas_con_nombre_other_ctrl_no_cuenta_decision_1() {
    // Véase la cabecera: la decisión 1 dice «no cuenta en símbolos ni en teclas con nombre»
    // (la tabla de la fila 2 decía `None`; las pruebas de `action_for_shortcut` exigen
    // `Inicio` con `Ctrl` suelto = `ZoomFit`).
    let km = por_defecto();
    for platform in PLATAFORMAS {
        for (key, esperada) in asignadas_sin_command()
            .into_iter()
            .filter(|(k, _)| !es_letra(*k))
        {
            assert_eq!(
                accion(&km, &press(key, false, false, false, true), platform),
                Some(esperada),
                "{key:?} con other_ctrl en {platform:?}"
            );
        }
    }
}

#[test]
fn fila_2_el_resultado_no_depende_de_repeat() {
    let km = por_defecto();
    for platform in PLATAFORMAS {
        for (key, esperada) in asignadas_sin_command() {
            let mut p = sola(key);
            p.repeat = true;
            assert_eq!(
                accion(&km, &p, platform),
                Some(esperada),
                "{key:?} repetida"
            );
        }
    }
}

/// Todas las combinaciones de `shift`, `alt` y `other_ctrl` (sin `command`).
fn combinaciones_sin_command(key: KeyId) -> Vec<Press> {
    let mut v = Vec::new();
    for shift in [false, true] {
        for alt in [false, true] {
            for other_ctrl in [false, true] {
                v.push(press(key, false, shift, alt, other_ctrl));
            }
        }
    }
    v
}

#[test]
fn fila_2_lo_sin_asignar_da_none_con_cualquier_modificador_en_las_tres_plataformas() {
    let km = por_defecto();
    // Sin `command`: `p l i e` y `P L I E` (sin convivencia), las cifras 1 a 9, `K` `D`, las
    // teclas del hito 4, y `. / { }`.
    let mut sin_asignar: Vec<KeyId> = "plieKDPLIE./qrcwbtgmasxyzo"
        .chars()
        .map(KeyId::Char)
        .collect();
    sin_asignar.extend(('1'..='9').map(KeyId::Char));
    for platform in PLATAFORMAS {
        for key in &sin_asignar {
            for p in combinaciones_sin_command(*key) {
                assert_eq!(
                    accion(&km, &p, platform),
                    None,
                    "{key:?} con {p:?} en {platform:?}"
                );
            }
        }
        // `Fin`, `Escape` y `Espacio` no son asignables: las teclas sin carácter (`key: None`).
        for command in [false, true] {
            for shift in [false, true] {
                for alt in [false, true] {
                    let p = Press {
                        key: None,
                        command,
                        shift,
                        alt,
                        other_ctrl: false,
                        repeat: false,
                    };
                    assert_eq!(accion(&km, &p, platform), None, "{p:?} en {platform:?}");
                }
            }
        }
        for reservada in [c('\u{1b}'), c(' ')] {
            for p in combinaciones_sin_command(reservada) {
                assert_eq!(accion(&km, &p, platform), None, "{p:?} en {platform:?}");
            }
        }
    }
}

#[test]
fn fila_2_con_command_las_letras_y_cifras_sin_fila_dan_none() {
    // Nada se cuela con `Ctrl`: solo las nueve modernas (fila 3). La lista reservada
    // (`Ctrl+C/X/V/A`) tampoco hace nada.
    let km = por_defecto();
    let mut sin_fila: Vec<KeyId> = "pliedvfukcxvawbtgmrKD,.[]<>+=-/"
        .chars()
        .map(KeyId::Char)
        .collect();
    sin_fila.extend(('2'..='9').map(KeyId::Char));
    sin_fila.extend([KeyId::Home, KeyId::PageUp, KeyId::PageDown]);
    for platform in PLATAFORMAS {
        for key in &sin_fila {
            for (shift, alt) in [(false, false), (true, false), (false, true), (true, true)] {
                assert_eq!(
                    accion(&km, &con_command(*key, shift, alt), platform),
                    None,
                    "Ctrl+{key:?} con shift={shift} alt={alt} en {platform:?}"
                );
            }
        }
    }
}

#[test]
fn fila_2_las_cifras_1_a_9_sin_modificadores_no_hacen_nada() {
    let km = por_defecto();
    for platform in PLATAFORMAS {
        for cifra in '1'..='9' {
            assert_eq!(
                accion(&km, &sola(c(cifra)), platform),
                None,
                "cifra {cifra} en {platform:?} (antes elegía la ranura)"
            );
        }
    }
}

#[test]
fn fila_2_mayusculas_en_el_caracter_no_casan_con_la_fila_en_minuscula() {
    // Las letras de `KeyId` van en minúscula (`presses` las pasa a minúscula): `Char('D')` no
    // es ninguna fila.
    let km = por_defecto();
    for platform in PLATAFORMAS {
        for key in ['D', 'V', 'F', 'U', 'N'] {
            assert_eq!(
                accion(&km, &sola(c(key)), platform),
                None,
                "{key} en {platform:?}"
            );
        }
    }
}

// ---------------------------------------------------------------------------------------------
// Fila 3: los nueve modernos intactos
// ---------------------------------------------------------------------------------------------

/// Oráculo escrito a mano con las reglas de las funciones de antes de la unidad: con `command`,
/// `Alt` apagado y `Mayús` exacto; `Q` solo en Linux.
fn moderno_esperado(key: char, shift: bool, alt: bool, platform: Platform) -> Option<AppAction> {
    if alt {
        return None;
    }
    match (key, shift) {
        ('z', false) => Some(AppAction::Undo),
        ('z', true) | ('y', false) => Some(AppAction::Redo),
        ('o', false) => Some(AppAction::Open),
        ('n', false) => Some(AppAction::NewImage),
        ('s', false) => Some(AppAction::Save),
        ('s', true) => Some(AppAction::SaveAs),
        ('1', false) => Some(AppAction::ZoomActual),
        ('q', false) if platform == Platform::Linux => Some(AppAction::Quit),
        _ => None,
    }
}

#[test]
fn fila_3_los_nueve_modernos_dan_su_accion_en_las_tres_plataformas() {
    let km = por_defecto();
    let casos = [
        ('z', false, AppAction::Undo),
        ('y', false, AppAction::Redo),
        ('z', true, AppAction::Redo),
        ('o', false, AppAction::Open),
        ('n', false, AppAction::NewImage),
        ('s', false, AppAction::Save),
        ('s', true, AppAction::SaveAs),
        ('1', false, AppAction::ZoomActual),
    ];
    for platform in PLATAFORMAS {
        for (key, shift, esperada) in casos {
            assert_eq!(
                accion(&km, &con_command(c(key), shift, false), platform),
                Some(esperada),
                "Ctrl{}+{key} en {platform:?}",
                if shift { "+Mayús" } else { "" }
            );
        }
        assert_eq!(
            accion(&km, &con_command(c('q'), false, false), platform),
            (platform == Platform::Linux).then_some(AppAction::Quit),
            "Ctrl+Q en {platform:?}"
        );
    }
}

#[test]
fn fila_3_con_alt_con_shift_de_mas_y_con_las_teclas_ajenas_el_resultado_es_el_de_antes() {
    let km = por_defecto();
    let teclas = ['z', 'y', 'o', 'n', 's', 'q', '1', 'd', ',', ']', 'a', 'x'];
    for platform in PLATAFORMAS {
        for key in teclas {
            for shift in [false, true] {
                for alt in [false, true] {
                    assert_eq!(
                        accion(&km, &con_command(c(key), shift, alt), platform),
                        moderno_esperado(key, shift, alt, platform),
                        "Ctrl+{key} con shift={shift} alt={alt} en {platform:?}"
                    );
                }
            }
        }
    }
}

#[test]
fn fila_3_ctrl_q_en_windows_y_macos_no_hace_nada_con_ningun_modificador() {
    let km = por_defecto();
    for platform in [Platform::Windows, Platform::Mac] {
        for shift in [false, true] {
            for alt in [false, true] {
                assert_eq!(
                    accion(&km, &con_command(c('q'), shift, alt), platform),
                    None,
                    "Ctrl+Q con shift={shift} alt={alt} en {platform:?}"
                );
            }
        }
    }
}

#[test]
fn fila_3_ctrl_d_ctrl_coma_y_ctrl_corchete_no_hacen_nada() {
    let km = por_defecto();
    for platform in PLATAFORMAS {
        for key in ['d', ',', ']', '[', 'u', 'v', 'f'] {
            assert_eq!(
                accion(&km, &con_command(c(key), false, false), platform),
                None,
                "Ctrl+{key} en {platform:?}"
            );
        }
    }
}

#[test]
fn fila_3_las_filas_con_command_son_exactamente_las_nueve_modernas() {
    let mut esperadas = vec![
        (AppAction::Undo, 'z', false),
        (AppAction::Redo, 'y', false),
        (AppAction::Redo, 'z', true),
        (AppAction::Open, 'o', false),
        (AppAction::NewImage, 'n', false),
        (AppAction::Save, 's', false),
        (AppAction::SaveAs, 's', true),
        (AppAction::ZoomActual, '1', false),
        (AppAction::Quit, 'q', false),
    ];
    let mut reales: Vec<(AppAction, char, bool)> = por_defecto()
        .rows()
        .iter()
        .filter(|f| f.command)
        .map(|f| match f.key {
            KeyId::Char(ch) => (f.action, ch, f.shift),
            otra => panic!("una fila con command y la tecla {otra:?}"),
        })
        .collect();
    let clave = |t: &(AppAction, char, bool)| format!("{t:?}");
    esperadas.sort_by_key(clave);
    reales.sort_by_key(clave);
    assert_eq!(reales, esperadas);
}

// ---------------------------------------------------------------------------------------------
// Fila 5: rótulos desde la tabla
// ---------------------------------------------------------------------------------------------

/// Lo que `shown_text` tiene que dar para cada acción en `platform` (`None` = sin rótulo).
fn rotulos_esperados(platform: Platform) -> Vec<(AppAction, Option<String>)> {
    let mando = if platform == Platform::Mac {
        "Cmd"
    } else {
        "Ctrl"
    };
    let redo = if platform == Platform::Mac {
        "Cmd+Mayús+Z".to_string()
    } else {
        "Ctrl+Y".to_string()
    };
    let s = |t: &str| Some(t.to_string());
    let mut v = vec![
        (AppAction::SelectTool(Tool::Pencil), s("d")),
        (AppAction::SelectTool(Tool::Line), s("v")),
        (AppAction::SelectTool(Tool::Fill), s("f")),
        (AppAction::SelectTool(Tool::Dropper), s(",")),
        (AppAction::SelectTool(Tool::Eraser), None),
        (AppAction::Undo, Some(format!("{mando}+Z"))),
        (AppAction::Redo, Some(redo)),
        (AppAction::ZoomIn, s(">")),
        (AppAction::ZoomOut, s("<")),
        (AppAction::ZoomFit, s("Inicio")),
        (AppAction::ZoomActual, Some(format!("{mando}+1"))),
        (
            AppAction::Quit,
            (platform == Platform::Linux).then(|| "Ctrl+Q".to_string()),
        ),
        (AppAction::ColorNext, s("]")),
        (AppAction::ColorPrev, s("[")),
        (AppAction::CenterOnCursor, s("n")),
        (AppAction::Open, Some(format!("{mando}+O"))),
        (AppAction::Save, Some(format!("{mando}+S"))),
        (AppAction::SaveAs, Some(format!("{mando}+Mayús+S"))),
        (AppAction::NewImage, Some(format!("{mando}+N"))),
        // Sin fila.
        (AppAction::ConvertMode, None),
        (AppAction::ShowLicenses, None),
        (AppAction::CloseLicenses, None),
        (AppAction::BackgroundColor, None),
        (AppAction::SelectColor(3), None),
    ];
    v.extend(FONDOS.map(|f| (AppAction::CanvasBackground(f), None)));
    v
}

#[test]
fn fila_5_shown_text_de_cada_accion_en_las_tres_plataformas() {
    let km = por_defecto();
    for platform in PLATAFORMAS {
        for (action, esperado) in rotulos_esperados(platform) {
            assert_eq!(
                km.shown_text(action, platform),
                esperado,
                "{action:?} en {platform:?}"
            );
        }
    }
}

#[test]
fn fila_5_las_teclas_de_las_cifras_y_de_re_pag_av_pag_no_se_muestran_en_ningun_sitio() {
    let km = por_defecto();
    for platform in PLATAFORMAS {
        for slot in 1..=9 {
            assert_eq!(km.shown_text(AppAction::SelectColor(slot), platform), None);
        }
        for action in todas_las_acciones() {
            if let Some(texto) = km.shown_text(action, platform) {
                assert!(
                    !texto.contains("Pág"),
                    "{action:?} en {platform:?} enseña {texto:?}"
                );
            }
        }
    }
}

#[test]
fn fila_5_shown_text_es_el_texto_de_shown() {
    let km = por_defecto();
    for platform in PLATAFORMAS {
        for action in todas_las_acciones() {
            let esperado = km
                .shown(action, platform)
                .map(|chord| shortcut_text(chord, platform));
            assert_eq!(
                km.shown_text(action, platform),
                esperado,
                "{action:?} en {platform:?}"
            );
        }
    }
}

#[test]
fn fila_5_los_botones_de_herramienta_ensenan_d_v_f_coma_y_el_borrador_nada() {
    let km = por_defecto();
    let esperado = [Some("d"), Some("v"), Some("f"), Some(","), None];
    for platform in PLATAFORMAS {
        let reales: Vec<Option<String>> = TOOL_BUTTONS
            .iter()
            .map(|b| km.shown_text(b.action, platform))
            .collect();
        let esperado: Vec<Option<String>> =
            esperado.iter().map(|t| t.map(str::to_string)).collect();
        assert_eq!(reales, esperado, "{platform:?}");
    }
}

#[test]
fn fila_5_los_botones_de_zoom_no_tienen_letra_pero_la_tabla_muestra_sus_atajos() {
    // Los botones de zoom no pintan letra (el menú Ver enseña los atajos): su acción sí tiene
    // rótulo en la tabla.
    let km = por_defecto();
    let acciones: Vec<AppAction> = ZOOM_BUTTONS.iter().map(|b| b.action).collect();
    assert_eq!(
        acciones,
        [
            AppAction::ZoomIn,
            AppAction::ZoomOut,
            AppAction::ZoomFit,
            AppAction::ZoomActual
        ]
    );
    for action in acciones {
        assert!(
            km.shown_text(action, Platform::Linux).is_some(),
            "{action:?}"
        );
    }
}

#[test]
fn fila_5_el_menu_con_shortcut_for_y_shortcut_text_sale_de_la_tabla_por_defecto() {
    let km = por_defecto();
    for platform in PLATAFORMAS {
        for menu in &MENUS {
            for item in menu.items {
                let del_menu =
                    shortcut_for(item.action, platform).map(|s| shortcut_text(s, platform));
                assert_eq!(
                    del_menu,
                    km.shown_text(item.action, platform),
                    "{} / {} en {platform:?}",
                    menu.title,
                    item.label
                );
                assert_eq!(
                    shortcut_for(item.action, platform),
                    km.shown(item.action, platform),
                    "{} / {} en {platform:?}",
                    menu.title,
                    item.label
                );
                assert_eq!(
                    shortcut_in(&km, item.action, platform),
                    km.shown(item.action, platform),
                    "{} / {} en {platform:?}",
                    menu.title,
                    item.label
                );
            }
        }
    }
}

#[test]
fn fila_5_ver_enseña_mayor_y_menor_y_no_mas_ni_menos() {
    let ver = MENUS.iter().find(|m| m.title == "Ver").expect("menú Ver");
    for platform in PLATAFORMAS {
        let textos: Vec<Option<String>> = ver
            .items
            .iter()
            .map(|i| shortcut_for(i.action, platform).map(|s| shortcut_text(s, platform)))
            .collect();
        let mando = if platform == Platform::Mac {
            "Cmd"
        } else {
            "Ctrl"
        };
        assert_eq!(
            textos,
            [
                Some(">".to_string()),
                Some("<".to_string()),
                Some("Inicio".to_string()),
                Some(format!("{mando}+1")),
            ],
            "{platform:?}"
        );
    }
}

#[test]
fn fila_5_los_anchos_de_los_desplegables_no_cambian() {
    // 12 u de márgenes + 7 u por carácter del mayor `rótulo + 2 + atajo`. Escritos a mano.
    let esperado = |titulo: &str, platform: Platform| -> i64 {
        let mac = platform == Platform::Mac;
        match (titulo, mac) {
            ("Archivo", false) => 201,
            ("Archivo", true) => 194,
            ("Editar", false) => 124,
            ("Editar", true) => 152,
            ("Imagen", _) => 131,
            ("Ver", _) => 208,
            ("Ayuda", _) => 89,
            otro => panic!("menú {otro:?}"),
        }
    };
    for platform in PLATAFORMAS {
        for menu in &MENUS {
            assert_eq!(
                popup_width_u(menu, platform),
                esperado(menu.title, platform),
                "{} en {platform:?}",
                menu.title
            );
        }
    }
}

/// Todos los rótulos que la tabla puede enseñar.
fn todos_los_rotulos() -> Vec<String> {
    let km = por_defecto();
    let mut v = Vec::new();
    for platform in PLATAFORMAS {
        for action in todas_las_acciones() {
            if let Some(texto) = km.shown_text(action, platform) {
                v.push(texto);
            }
        }
    }
    v
}

#[test]
fn fila_5_ningun_rotulo_pierde_un_caracter_por_replace_missing() {
    let tiene = |ch: char| INTERFACE_CHARSET.contains(ch);
    let rotulos = todos_los_rotulos();
    assert!(!rotulos.is_empty());
    for texto in rotulos {
        assert!(!texto.is_empty());
        assert_eq!(replace_missing(&texto, tiene), texto);
        assert!(!texto.contains('?'), "{texto:?} lleva un `?`");
    }
}

#[test]
fn fila_5_con_la_fuente_real_printable_no_cambia_ningun_rotulo() {
    let ctx = contexto(1.0);
    for texto in todos_los_rotulos() {
        assert_eq!(printable(&ctx, &texto), texto);
    }
}

// ---------------------------------------------------------------------------------------------
// Banco de egui sin ventana: menú y columna con la tabla recibida (fila 5 pintada y fila 11)
// ---------------------------------------------------------------------------------------------

const PANTALLA: Vec2 = vec2(600.0, 400.0);

fn estado_ui() -> UiState {
    UiState {
        can_undo: true,
        can_redo: true,
        zoom: Zoom::from_int(5),
        tool: Tool::Pencil,
        locks: Locks::default(),
    }
}

struct BancoUi {
    ctx: Context,
    ppp: f32,
    /// Reloj de egui: avanza en cada fotograma para que terminen las animaciones del desplegable.
    tiempo: f64,
}

impl BancoUi {
    fn nuevo(ppp: f32) -> BancoUi {
        let ctx = contexto(ppp);
        ctx.set_visuals_of(egui::Theme::Dark, visuals(ppp));
        BancoUi {
            ctx,
            ppp,
            tiempo: 0.0,
        }
    }

    fn fotograma(&mut self, events: Vec<Event>, mut dibuja: impl FnMut(&mut Ui)) -> Pintado {
        self.tiempo += 0.25;
        let entrada = RawInput {
            screen_rect: Some(Rect::from_min_size(Pos2::ZERO, PANTALLA)),
            events,
            time: Some(self.tiempo),
            ..RawInput::default()
        };
        let mut salida = self.ctx.run_ui(entrada, |ui| {
            apply_scale(ui.ctx());
            dibuja(ui);
        });
        salida.textures_delta.clear();
        pintado(&salida.shapes)
    }

    fn menu(
        &mut self,
        events: Vec<Event>,
        km: &Keymap,
        platform: Platform,
    ) -> (Pintado, Option<AppAction>) {
        let estado = estado_ui();
        let mut elegida = None;
        let p = self.fotograma(events, |ui| {
            elegida = show_menu_in(ui, &estado, platform, CanvasBackground::default(), km);
        });
        (p, elegida)
    }

    fn menu_en_reposo(&mut self, km: &Keymap, platform: Platform) -> Pintado {
        let mut ultimo = self.menu(vec![], km, platform).0;
        for _ in 0..5 {
            ultimo = self.menu(vec![], km, platform).0;
        }
        ultimo
    }

    /// Clic real (mover, pulsar y soltar) en `pos`; devuelve lo que eligió el menú.
    fn clic_en_el_menu(&mut self, pos: Pos2, km: &Keymap, platform: Platform) -> Option<AppAction> {
        let mut elegida = None;
        for events in [
            vec![mover(pos)],
            vec![boton(pos, true)],
            vec![boton(pos, false)],
        ] {
            elegida = elegida.or(self.menu(events, km, platform).1);
        }
        elegida
    }

    /// Abre el menú `titulo` con un clic real y devuelve lo pintado ya asentado.
    fn abre_menu(&mut self, titulo: &str, km: &Keymap, platform: Platform) -> Pintado {
        let barra = self.menu_en_reposo(km, platform);
        let centro = barra
            .texto_con(titulo)
            .unwrap_or_else(|| panic!("no se pinta {titulo:?}: {:?}", barra.todos_los_textos()))
            .rect()
            .center();
        self.clic_en_el_menu(centro, km, platform);
        self.menu_en_reposo(km, platform)
    }

    fn columna(
        &mut self,
        events: Vec<Event>,
        km: &Keymap,
        platform: Platform,
    ) -> (Pintado, Option<AppAction>) {
        let estado = estado_ui();
        let mut elegida = None;
        let p = self.fotograma(events, |ui| {
            elegida = show_toolbar_with(ui, &estado, "×5", km, platform);
        });
        (p, elegida)
    }

    fn columna_en_reposo(&mut self, km: &Keymap, platform: Platform) -> Pintado {
        self.columna(vec![], km, platform);
        self.columna(vec![], km, platform).0
    }

    /// Clic en el centro del botón de herramienta `indice` (0 = Lápiz).
    fn clic_en_la_herramienta(
        &mut self,
        indice: i64,
        km: &Keymap,
        platform: Platform,
    ) -> Vec<AppAction> {
        self.columna_en_reposo(km, platform);
        let n = n_de(self.ppp);
        // Botones de 99 × 16 u con 4 u de margen; herramientas en `y = 4 + 18·i`.
        let pos = en_puntos(self.ppp, (4 + 99 / 2) * n, (4 + 18 * indice) * n + 8 * n);
        let mut acciones = Vec::new();
        for events in [
            vec![mover(pos)],
            vec![boton(pos, true)],
            vec![boton(pos, false)],
        ] {
            acciones.extend(self.columna(events, km, platform).1);
        }
        acciones
    }
}

/// La tabla por defecto con una tecla cambiada por otra (sin `command`).
fn con_teclas_cambiadas(a: char, b: char) -> Keymap {
    let filas = por_defecto()
        .rows()
        .iter()
        .cloned()
        .map(|mut f| {
            if !f.command && f.key == c(a) {
                f.key = c(b);
            } else if !f.command && f.key == c(b) {
                f.key = c(a);
            }
            f
        })
        .collect();
    Keymap::new(filas)
}

#[test]
fn fila_5_el_menu_pintado_con_la_tabla_por_defecto_enseña_los_atajos_de_la_tabla() {
    let km = por_defecto();
    let mut banco = BancoUi::nuevo(1.0);
    let editar = banco.abre_menu("Editar", &km, Platform::Linux);
    assert!(
        editar.texto_con("Ctrl+Z").is_some(),
        "{:?}",
        editar.todos_los_textos()
    );
    assert!(editar.texto_con("Ctrl+Y").is_some());
    let mut banco = BancoUi::nuevo(1.0);
    let ver = banco.abre_menu("Ver", &km, Platform::Linux);
    for texto in [">", "<", "Inicio", "Ctrl+1"] {
        assert!(
            ver.pinta(texto) || ver.texto_con(texto).is_some(),
            "Ver no enseña {texto:?}: {:?}",
            ver.todos_los_textos()
        );
    }
    assert!(
        !ver.pinta("+") && !ver.pinta("-"),
        "Ver ya no enseña `+` ni `-`: {:?}",
        ver.todos_los_textos()
    );
}

#[test]
fn fila_5_la_columna_pintada_con_la_tabla_por_defecto_enseña_d_v_f_coma_y_no_enseña_nada_del_borrador()
 {
    let km = por_defecto();
    for ppp in [1.0, 2.0] {
        let mut banco = BancoUi::nuevo(ppp);
        let p = banco.columna_en_reposo(&km, Platform::Linux);
        for letra in ["d", "v", "f", ","] {
            assert!(p.pinta(letra), "ppp {ppp}: falta la letra {letra:?}");
        }
        assert!(p.pinta("Borrador"));
        // Ninguna letra suelta más: el Borrador no tiene fila y los botones de zoom no llevan letra.
        for ajena in ["e", "E", "p", "P", ">", "<", "Inicio"] {
            assert!(!p.pinta(ajena), "ppp {ppp}: la columna pinta {ajena:?}");
        }
    }
}

// ---------------------------------------------------------------------------------------------
// Fila 8 (parte de px-ui): bloqueos, gate_keys y el diálogo
// ---------------------------------------------------------------------------------------------

mod fila_8 {
    use super::*;
    use px_ui::dialog::{DialogKind, Modal};
    use px_ui::dialog_keys::FocusTarget;
    use px_ui::dialog_view::show_dialog;
    use px_ui::licenses_view::{LICENSES_FIELD, licenses_form};

    fn key(k: Key, modifiers: Modifiers) -> Event {
        Event::Key {
            key: k,
            physical_key: Some(k),
            pressed: true,
            repeat: false,
            modifiers,
        }
    }

    fn con_texto(k: Key, modifiers: Modifiers, texto: &str) -> Vec<Event> {
        vec![key(k, modifiers), Event::Text(texto.to_string())]
    }

    /// Las secuencias de la fila 4 y la acción que dan sin campo enfocado.
    fn secuencias() -> Vec<(&'static str, Vec<Event>, AppAction)> {
        vec![
            (
                "d",
                con_texto(Key::D, Modifiers::NONE, "d"),
                AppAction::SelectTool(Tool::Pencil),
            ),
            (
                "]",
                con_texto(Key::CloseBracket, Modifiers::NONE, "]"),
                AppAction::ColorNext,
            ),
            (
                "[ (AltGr)",
                con_texto(Key::OpenBracket, Modifiers::ALT, "["),
                AppAction::ColorPrev,
            ),
            (
                "< (ISO)",
                con_texto(Key::IntlBackslash, Modifiers::NONE, "<"),
                AppAction::ZoomOut,
            ),
            (
                "> (inglés)",
                con_texto(Key::Period, Modifiers::SHIFT, ">"),
                AppAction::ZoomIn,
            ),
            (
                "Re Pág",
                vec![key(Key::PageUp, Modifiers::NONE)],
                AppAction::ColorPrev,
            ),
            (
                "Inicio",
                vec![key(Key::Home, Modifiers::NONE)],
                AppAction::ZoomFit,
            ),
        ]
    }

    /// Un `Context` con un campo de texto que se puede enfocar o no.
    struct BancoTexto {
        ctx: Context,
        texto: String,
    }

    impl BancoTexto {
        fn nuevo() -> BancoTexto {
            BancoTexto {
                ctx: Context::default(),
                texto: String::new(),
            }
        }

        fn fotograma(&mut self, events: Vec<Event>, enfocar: bool) {
            let input = RawInput {
                screen_rect: Some(Rect::from_min_size(Pos2::ZERO, vec2(400.0, 300.0))),
                events,
                ..RawInput::default()
            };
            let texto = &mut self.texto;
            let mut salida = self.ctx.run_ui(input, |ui| {
                let respuesta = ui.add(TextEdit::singleline(texto));
                if enfocar {
                    respuesta.request_focus();
                }
            });
            salida.textures_delta.clear();
        }
    }

    /// Las acciones que saldrían de `events` tras la puerta `gate_keys`, con la tabla por defecto.
    fn acciones_tras_la_puerta(
        events: &[Event],
        wants_keyboard: bool,
        platform: Platform,
    ) -> Vec<Option<AppAction>> {
        let vistos = gate_keys(events, wants_keyboard);
        let km = por_defecto();
        presses(&vistos)
            .iter()
            .map(|p| km.action_for(p, platform))
            .collect()
    }

    #[test]
    fn fila_8_con_un_campo_enfocado_de_verdad_gate_keys_quita_key_y_text_y_no_sale_ninguna_accion()
    {
        for platform in PLATAFORMAS {
            for (nombre, events, _) in secuencias() {
                let mut con_foco = BancoTexto::nuevo();
                con_foco.fotograma(vec![], true);
                con_foco.fotograma(vec![], true);
                assert!(con_foco.ctx.text_edit_focused(), "el campo tiene el foco");
                con_foco.fotograma(events.clone(), false);
                let enfocado = con_foco.ctx.text_edit_focused();
                assert!(enfocado, "{nombre}: sigue enfocado");
                let vistos = gate_keys(&events, enfocado);
                assert!(
                    vistos.is_empty(),
                    "{nombre} en {platform:?}: gate_keys deja pasar {vistos:?} (también el Text)"
                );
                assert_eq!(
                    acciones_tras_la_puerta(&events, enfocado, platform),
                    Vec::<Option<AppAction>>::new(),
                    "{nombre} en {platform:?}"
                );
            }
        }
    }

    #[test]
    fn fila_8_sin_campo_enfocado_las_mismas_secuencias_dan_su_accion() {
        for platform in PLATAFORMAS {
            for (nombre, events, esperada) in secuencias() {
                let mut sin_foco = BancoTexto::nuevo();
                sin_foco.fotograma(vec![], false);
                sin_foco.fotograma(vec![], false);
                assert!(!sin_foco.ctx.text_edit_focused());
                let vistos = gate_keys(&events, sin_foco.ctx.text_edit_focused());
                assert_eq!(
                    vistos.len(),
                    events.len(),
                    "{nombre}: pasan todos los eventos"
                );
                assert_eq!(
                    acciones_tras_la_puerta(&events, false, platform),
                    vec![Some(esperada)],
                    "{nombre} en {platform:?}"
                );
            }
        }
    }

    #[test]
    fn fila_8_con_un_dialogo_abierto_toda_fila_de_la_tabla_da_blocked_tambien_inicio_re_pag_y_av_pag()
     {
        let km = por_defecto();
        let dialogo = Locks {
            dialog: true,
            ..Locks::default()
        };
        for fila in km.rows() {
            assert!(
                is_blocked(fila.action, dialogo, Origin::Key),
                "{:?} ({:?}) con un diálogo de archivo",
                fila.action,
                fila.key
            );
            assert!(
                !is_blocked(fila.action, Locks::default(), Origin::Key),
                "{:?} ({:?}) sin bloqueos",
                fila.action,
                fila.key
            );
        }
        for tecla in [KeyId::Home, KeyId::PageUp, KeyId::PageDown] {
            assert!(
                km.rows().iter().any(|f| f.key == tecla),
                "la tabla tiene una fila con {tecla:?}"
            );
        }
    }

    #[test]
    fn fila_8_con_el_panel_modal_abierto_toda_fila_da_blocked_salvo_salir() {
        let km = por_defecto();
        let modal = Locks {
            modal: true,
            ..Locks::default()
        };
        for fila in km.rows() {
            // `Salir` pasa con el panel modal (decisión D1 de la unidad 18): sigue igual.
            let esperado = fila.action != AppAction::Quit;
            assert_eq!(
                is_blocked(fila.action, modal, Origin::Key),
                esperado,
                "{:?} ({:?}) con el modal",
                fila.action,
                fila.key
            );
        }
    }

    /// El panel de licencias abierto como `App` (el `Modal`) y pintado con `show_dialog`.
    struct BancoDialogo {
        ctx: Context,
        modal: Modal,
        focus: Option<FocusTarget>,
    }

    impl BancoDialogo {
        fn nuevo() -> BancoDialogo {
            let ctx = contexto(1.0);
            ctx.set_visuals_of(egui::Theme::Dark, visuals(1.0));
            let texto = (1..=200)
                .map(|n| format!("linea {n}"))
                .collect::<Vec<_>>()
                .join("\n");
            let mut modal = Modal::Closed;
            assert!(modal.open(
                DialogKind::Licenses,
                licenses_form(&texto),
                Locks::default()
            ));
            let mut banco = BancoDialogo {
                ctx,
                modal,
                focus: None,
            };
            for _ in 0..3 {
                banco.fotograma(vec![]);
            }
            banco
        }

        fn fotograma(&mut self, events: Vec<Event>) {
            let BancoDialogo { ctx, modal, focus } = self;
            let entrada = RawInput {
                screen_rect: Some(Rect::from_min_size(Pos2::ZERO, vec2(960.0, 720.0))),
                events,
                ..RawInput::default()
            };
            let mut salida = ctx.run_ui(entrada, |ui| {
                let c = ui.ctx().clone();
                apply_scale(&c);
                if let Modal::Open(dialog) = modal {
                    let _ = show_dialog(&c, dialog, focus);
                }
            });
            salida.textures_delta.clear();
        }

        fn primera_linea(&self) -> usize {
            match &self.modal {
                Modal::Open(dialog) => dialog.first_line(LICENSES_FIELD),
                Modal::Closed => panic!("el diálogo se cerró"),
            }
        }

        /// Una pulsación como `App`: la tabla traduce la tecla (y el bloqueo la frena), y el
        /// diálogo la recibe igualmente. Devuelve la acción que la tabla le daba y si se bloquea.
        fn pulsa(&mut self, k: Key) -> (Option<AppAction>, Option<bool>) {
            let events = vec![key(k, Modifiers::NONE)];
            let km = por_defecto();
            let accion = presses(&events)
                .first()
                .and_then(|p| km.action_for(p, Platform::Linux));
            let locks = Locks {
                modal: true,
                ..Locks::default()
            };
            let bloqueada = accion.map(|a| is_blocked(a, locks, Origin::Key));
            self.fotograma(events);
            for _ in 0..2 {
                self.fotograma(vec![]);
            }
            (accion, bloqueada)
        }
    }

    #[test]
    fn fila_8_inicio_fin_re_pag_y_av_pag_siguen_desplazando_el_dialogo_con_el_modal_abierto() {
        let mut banco = BancoDialogo::nuevo();
        assert_eq!(banco.primera_linea(), 0);

        let (accion, bloqueada) = banco.pulsa(Key::PageDown);
        assert_eq!(accion, Some(AppAction::ColorNext), "Av Pág en la tabla");
        assert_eq!(bloqueada, Some(true), "Av Pág bloqueada para la tabla");
        let tras_av = banco.primera_linea();
        assert!(tras_av > 0, "Av Pág desplaza el diálogo");

        let (accion, _) = banco.pulsa(Key::End);
        assert_eq!(accion, None, "Fin no está en la tabla");
        let maximo = banco.primera_linea();
        assert!(maximo > tras_av, "Fin va al final");

        let (accion, bloqueada) = banco.pulsa(Key::PageUp);
        assert_eq!(accion, Some(AppAction::ColorPrev), "Re Pág en la tabla");
        assert_eq!(bloqueada, Some(true));
        assert!(banco.primera_linea() < maximo, "Re Pág desplaza el diálogo");

        let (accion, bloqueada) = banco.pulsa(Key::Home);
        assert_eq!(accion, Some(AppAction::ZoomFit), "Inicio en la tabla");
        assert_eq!(bloqueada, Some(true));
        assert_eq!(banco.primera_linea(), 0, "Inicio vuelve al principio");
    }
}

// ---------------------------------------------------------------------------------------------
// Fila 10 (parte de px-ui): cifras sin tecla y las cuatro funciones antiguas frente a la tabla
// ---------------------------------------------------------------------------------------------

const CIFRAS: [Key; 9] = [
    Key::Num1,
    Key::Num2,
    Key::Num3,
    Key::Num4,
    Key::Num5,
    Key::Num6,
    Key::Num7,
    Key::Num8,
    Key::Num9,
];

#[test]
fn fila_10_las_cifras_1_a_9_sin_modificadores_no_hacen_nada_por_action_for_shortcut() {
    for platform in PLATAFORMAS {
        for key in CIFRAS {
            assert_eq!(
                action_for_shortcut(key, Modifiers::NONE, platform),
                None,
                "{key:?} en {platform:?}"
            );
        }
    }
}

#[test]
fn fila_10_las_cifras_1_a_9_sin_modificadores_no_hacen_nada_por_la_tabla() {
    let km = por_defecto();
    for platform in PLATAFORMAS {
        for key in CIFRAS {
            let p = press_from_key(key, Modifiers::NONE);
            assert_eq!(
                km.action_for(&p, platform),
                None,
                "{key:?} en {platform:?}: {p:?}"
            );
        }
    }
}

#[test]
fn fila_10_press_from_key_de_las_teclas_de_la_tabla_sin_modificadores() {
    let casos = [
        (Key::D, Some(c('d'))),
        (Key::Comma, Some(c(','))),
        (Key::OpenBracket, Some(c('['))),
        (Key::CloseBracket, Some(c(']'))),
        (Key::Num1, Some(c('1'))),
        (Key::Plus, Some(c('+'))),
        (Key::Equals, Some(c('='))),
        (Key::Minus, Some(c('-'))),
        (Key::Home, Some(KeyId::Home)),
        (Key::PageUp, Some(KeyId::PageUp)),
        (Key::PageDown, Some(KeyId::PageDown)),
    ];
    for (key, esperada) in casos {
        let p = press_from_key(key, Modifiers::NONE);
        assert_eq!(p.key, esperada, "{key:?}");
        assert!(
            !p.command && !p.shift && !p.alt && !p.other_ctrl && !p.repeat,
            "{key:?}: {p:?}"
        );
    }
}

#[test]
fn fila_10_press_from_key_pasa_los_modificadores_a_los_campos_de_la_press() {
    let linux = Modifiers {
        ctrl: true,
        command: true,
        ..Modifiers::NONE
    };
    let mac_cmd = Modifiers {
        mac_cmd: true,
        command: true,
        ..Modifiers::NONE
    };
    let mac_ctrl = Modifiers {
        ctrl: true,
        ..Modifiers::NONE
    };
    let p = press_from_key(Key::Z, linux | Modifiers::SHIFT);
    assert_eq!(p.key, Some(c('z')));
    assert!(p.command && p.shift && !p.alt && !p.other_ctrl);
    let p = press_from_key(Key::Z, mac_cmd);
    assert!(p.command && !p.other_ctrl);
    let p = press_from_key(Key::D, mac_ctrl);
    assert!(
        !p.command && p.other_ctrl,
        "un Ctrl que no es command: {p:?}"
    );
    let p = press_from_key(Key::D, Modifiers::ALT);
    assert!(p.alt && !p.command, "{p:?}");
    // Con `Mayús`, `Alt` o un `Ctrl` suelto, una cifra o un símbolo quedan sin carácter (cuál
    // sale depende de la distribución), salvo `+`, `=` y `-`.
    for m in [Modifiers::SHIFT, Modifiers::ALT, mac_ctrl] {
        for key in [Key::Num1, Key::Comma, Key::OpenBracket, Key::CloseBracket] {
            assert_eq!(press_from_key(key, m).key, None, "{key:?} con {m:?}");
        }
        for (key, esperada) in [(Key::Plus, '+'), (Key::Equals, '='), (Key::Minus, '-')] {
            assert_eq!(
                press_from_key(key, m).key,
                Some(c(esperada)),
                "{key:?} con {m:?}"
            );
        }
        // Las teclas con nombre valen con cualquier modificador sin `command`.
        for (key, esperada) in [
            (Key::Home, KeyId::Home),
            (Key::PageUp, KeyId::PageUp),
            (Key::PageDown, KeyId::PageDown),
        ] {
            assert_eq!(
                press_from_key(key, m).key,
                Some(esperada),
                "{key:?} con {m:?}"
            );
        }
    }
    // Las letras llevan `Mayús` y `Alt` aparte.
    for m in [Modifiers::SHIFT, Modifiers::ALT] {
        assert_eq!(press_from_key(Key::D, m).key, Some(c('d')), "D con {m:?}");
    }
}

/// Combinaciones de los cinco modificadores que egui puede entregar: sin `mac_cmd` suelto.
fn combinaciones_reales() -> Vec<(u8, Modifiers)> {
    (0..32u8)
        .map(|bits| (bits, modificadores(bits)))
        .filter(|(_, m)| !(m.mac_cmd && !m.command))
        .collect()
}

fn es_con_nombre(key: Key) -> bool {
    matches!(key, Key::Home | Key::PageUp | Key::PageDown)
}

#[test]
fn fila_10_action_for_shortcut_da_lo_mismo_que_la_tabla_sobre_key_all_por_32_combinaciones_y_3_plataformas()
 {
    let km = por_defecto();
    assert!(Key::ALL.len() > 100);
    for platform in PLATAFORMAS {
        for &key in Key::ALL {
            for (bits, m) in combinaciones_reales() {
                assert_eq!(
                    action_for_shortcut(key, m, platform),
                    km.action_for(&press_from_key(key, m), platform),
                    "{key:?} con {m:?} ({bits:#07b}) en {platform:?}"
                );
            }
        }
    }
}

#[test]
fn fila_10_tool_action_for_key_da_lo_mismo_que_la_tabla_donde_su_tipo_lo_permite() {
    let km = por_defecto();
    for platform in PLATAFORMAS {
        for &key in Key::ALL {
            for (bits, m) in combinaciones_reales() {
                let de_la_tabla = km.action_for(&press_from_key(key, m), platform);
                let esperado = match de_la_tabla {
                    Some(AppAction::SelectTool(tool)) => Some(ToolAction::Select(tool)),
                    // `Escape` cancela y no es de la tabla (es del lienzo).
                    _ if key == Key::Escape && m.is_none() => Some(ToolAction::Cancel),
                    _ => None,
                };
                assert_eq!(
                    tool_action_for_key(key, m),
                    esperado,
                    "{key:?} con {m:?} ({bits:#07b}) en {platform:?}"
                );
            }
        }
    }
}

#[test]
fn fila_10_file_action_for_key_da_lo_mismo_que_la_tabla_donde_su_tipo_lo_permite() {
    let km = por_defecto();
    for platform in PLATAFORMAS {
        for &key in Key::ALL {
            for (bits, m) in combinaciones_reales() {
                let esperado = match km.action_for(&press_from_key(key, m), platform) {
                    Some(AppAction::Open) => Some(FileAction::Open),
                    Some(AppAction::Save) => Some(FileAction::Save),
                    Some(AppAction::SaveAs) => Some(FileAction::SaveAs),
                    _ => None,
                };
                assert_eq!(
                    file_action_for_key(key, m),
                    esperado,
                    "{key:?} con {m:?} ({bits:#07b}) en {platform:?}"
                );
            }
        }
    }
}

#[test]
fn fila_10_action_for_key_da_lo_mismo_que_la_tabla_donde_su_tipo_lo_permite() {
    let km = por_defecto();
    for platform in PLATAFORMAS {
        for &key in Key::ALL {
            for (bits, m) in combinaciones_reales() {
                // Véase la cabecera: `Re Pág`/`Av Pág` con `ctrl` suelto.
                if es_con_nombre(key) && m.ctrl && !m.command {
                    continue;
                }
                let esperado = match km.action_for(&press_from_key(key, m), platform) {
                    Some(AppAction::Undo) => Some(Action::Undo),
                    Some(AppAction::Redo) => Some(Action::Redo),
                    Some(AppAction::ColorNext) => Some(Action::ColorNext),
                    Some(AppAction::ColorPrev) => Some(Action::ColorPrev),
                    _ => None,
                };
                assert_eq!(
                    action_for_key(key, m),
                    esperado,
                    "{key:?} con {m:?} ({bits:#07b}) en {platform:?}"
                );
            }
        }
    }
}

#[test]
fn fila_10_action_for_key_nunca_da_select_color() {
    for &key in Key::ALL {
        for bits in 0..32u8 {
            assert!(
                !matches!(
                    action_for_key(key, modificadores(bits)),
                    Some(Action::SelectColor(_))
                ),
                "{key:?} con {bits:#07b}"
            );
        }
    }
}

// ---------------------------------------------------------------------------------------------
// Fila 11: tabla sustituible y conflictos
// ---------------------------------------------------------------------------------------------

#[test]
fn fila_11_con_d_y_x_cambiadas_x_elige_el_lapiz_y_d_no() {
    let km = con_teclas_cambiadas('d', 'x');
    for platform in PLATAFORMAS {
        assert_eq!(
            accion(&km, &sola(c('x')), platform),
            Some(AppAction::SelectTool(Tool::Pencil)),
            "x en {platform:?}"
        );
        assert_eq!(
            accion(&km, &sola(c('d')), platform),
            None,
            "d en {platform:?}"
        );
        // El resto de la tabla sigue igual.
        assert_eq!(
            accion(&km, &sola(c('v')), platform),
            Some(AppAction::SelectTool(Tool::Line))
        );
        assert_eq!(
            accion(&km, &con_command(c('z'), false, false), platform),
            Some(AppAction::Undo)
        );
    }
    assert_eq!(km.conflicts(), vec![]);
}

#[test]
fn fila_11_con_d_y_x_cambiadas_el_rotulo_es_x() {
    let km = con_teclas_cambiadas('d', 'x');
    for platform in PLATAFORMAS {
        assert_eq!(
            km.shown_text(AppAction::SelectTool(Tool::Pencil), platform),
            Some("x".to_string())
        );
        assert_eq!(
            km.shown(AppAction::SelectTool(Tool::Pencil), platform),
            Some(Chord {
                key: c('x'),
                command: false,
                shift: false
            })
        );
        assert_eq!(
            shortcut_in(&km, AppAction::SelectTool(Tool::Pencil), platform),
            Some(Chord {
                key: c('x'),
                command: false,
                shift: false
            })
        );
    }
}

#[test]
fn fila_11_con_una_tabla_vacia_no_funciona_ninguna_tecla_y_no_se_muestra_atajo() {
    let km = Keymap::new(vec![]);
    assert_eq!(km.rows().len(), 0);
    assert_eq!(km.conflicts(), vec![]);
    let por_defecto = por_defecto();
    for platform in PLATAFORMAS {
        // Todas las pulsaciones que dan acción con la tabla por defecto dan `None` con la vacía.
        for fila in por_defecto.rows() {
            let p = Press {
                key: Some(fila.key),
                command: fila.command,
                shift: fila.shift,
                alt: false,
                other_ctrl: false,
                repeat: false,
            };
            assert_eq!(km.action_for(&p, platform), None, "{p:?} en {platform:?}");
        }
        for action in todas_las_acciones() {
            assert_eq!(
                km.shown(action, platform),
                None,
                "{action:?} en {platform:?}"
            );
            assert_eq!(
                km.shown_text(action, platform),
                None,
                "{action:?} en {platform:?}"
            );
            assert_eq!(shortcut_in(&km, action, platform), None);
        }
    }
}

#[test]
fn fila_11_el_menu_pintado_lee_la_tabla_recibida() {
    for ppp in [1.0, 2.0] {
        // Con la tabla por defecto, `Deshacer` enseña `Ctrl+Z`.
        let mut banco = BancoUi::nuevo(ppp);
        let editar = banco.abre_menu("Editar", &por_defecto(), Platform::Linux);
        assert!(editar.texto_con("Ctrl+Z").is_some(), "ppp {ppp}");

        // Con otra tabla (`Z` cambiada por `W`), enseña `Ctrl+W`.
        let filas: Vec<Binding> = por_defecto()
            .rows()
            .iter()
            .cloned()
            .map(|mut f| {
                if f.action == AppAction::Undo && f.command {
                    f.key = c('w');
                }
                f
            })
            .collect();
        let mut banco = BancoUi::nuevo(ppp);
        let editar = banco.abre_menu("Editar", &Keymap::new(filas), Platform::Linux);
        assert!(editar.texto_con("Ctrl+W").is_some(), "ppp {ppp}");
        assert!(editar.texto_con("Ctrl+Z").is_none(), "ppp {ppp}");

        // Con la vacía no hay atajo, pero la entrada sigue ahí y se puede elegir.
        let vacia = Keymap::new(vec![]);
        let mut banco = BancoUi::nuevo(ppp);
        let editar = banco.abre_menu("Editar", &vacia, Platform::Linux);
        assert!(editar.texto_con("Deshacer").is_some(), "ppp {ppp}");
        assert!(editar.texto_con("Rehacer").is_some(), "ppp {ppp}");
        assert!(
            editar.texto_con("Ctrl").is_none(),
            "ppp {ppp}: {:?}",
            editar.todos_los_textos()
        );
        let deshacer = editar
            .texto_con("Deshacer")
            .expect("la entrada")
            .rect()
            .center();
        assert_eq!(
            banco.clic_en_el_menu(deshacer, &vacia, Platform::Linux),
            Some(AppAction::Undo),
            "ppp {ppp}: el menú sigue funcionando con la tabla vacía"
        );
    }
}

#[test]
fn fila_11_la_columna_pintada_lee_la_tabla_recibida() {
    for ppp in [1.0, 2.0] {
        // `d` y `x` cambiadas: el botón del lápiz enseña `x` y ya no `d`.
        let cambiada = con_teclas_cambiadas('d', 'x');
        let mut banco = BancoUi::nuevo(ppp);
        let p = banco.columna_en_reposo(&cambiada, Platform::Linux);
        assert!(p.pinta("x"), "ppp {ppp}: {:?}", p.todos_los_textos());
        assert!(!p.pinta("d"), "ppp {ppp}");
        assert!(p.pinta("v") && p.pinta("f") && p.pinta(","), "ppp {ppp}");

        // Con la tabla vacía no hay ninguna letra, y los botones siguen funcionando.
        let vacia = Keymap::new(vec![]);
        let mut banco = BancoUi::nuevo(ppp);
        let p = banco.columna_en_reposo(&vacia, Platform::Linux);
        for letra in ["d", "v", "f", ",", "x"] {
            assert!(!p.pinta(letra), "ppp {ppp}: pinta {letra:?} sin tabla");
        }
        assert!(p.pinta("Lápiz") && p.pinta("Borrador"), "ppp {ppp}");
        assert_eq!(
            banco.clic_en_la_herramienta(0, &vacia, Platform::Linux),
            vec![AppAction::SelectTool(Tool::Pencil)],
            "ppp {ppp}: el botón funciona con la tabla vacía"
        );
    }
}

fn conflictos(filas: Vec<Binding>) -> Vec<Conflict> {
    Keymap::new(filas).conflicts()
}

#[test]
fn fila_11_dos_filas_iguales_dan_duplicate() {
    let a = fila(c('d'), false, false, AppAction::SelectTool(Tool::Pencil));
    let b = fila(c('d'), false, false, AppAction::SelectTool(Tool::Line));
    assert_eq!(
        conflictos(vec![a.clone(), b]),
        vec![Conflict::Duplicate(0, 1)]
    );
    // La misma fila repetida también.
    assert_eq!(
        conflictos(vec![a.clone(), a]),
        vec![Conflict::Duplicate(0, 1)]
    );
}

#[test]
fn fila_11_duplicate_sale_con_los_indices_de_las_filas_que_chocan() {
    let filas = vec![
        fila(c('d'), false, false, AppAction::SelectTool(Tool::Pencil)),
        fila(c('v'), false, false, AppAction::SelectTool(Tool::Line)),
        fila(c('v'), false, false, AppAction::SelectTool(Tool::Fill)),
    ];
    assert_eq!(conflictos(filas), vec![Conflict::Duplicate(1, 2)]);
}

#[test]
fn fila_11_no_chocan_las_que_difieren_en_letra_con_shift_en_command_o_en_plataforma() {
    // Letras: `shift` cuenta.
    assert_eq!(
        conflictos(vec![
            fila(c('d'), false, false, AppAction::SelectTool(Tool::Pencil)),
            fila(c('d'), false, true, AppAction::SelectTool(Tool::Line)),
        ]),
        vec![]
    );
    // `command` cuenta.
    assert_eq!(
        conflictos(vec![
            fila(c('d'), false, false, AppAction::SelectTool(Tool::Pencil)),
            fila(c('d'), true, false, AppAction::Open),
        ]),
        vec![]
    );
    // `active` sin solaparse.
    let mut linux = fila(c('d'), false, false, AppAction::SelectTool(Tool::Pencil));
    linux.active = Platforms::Only(Platform::Linux);
    let mut windows = fila(c('d'), false, false, AppAction::SelectTool(Tool::Line));
    windows.active = Platforms::Only(Platform::Windows);
    assert_eq!(conflictos(vec![linux.clone(), windows]), vec![]);
    let mut no_linux = fila(c('d'), false, false, AppAction::SelectTool(Tool::Line));
    no_linux.active = Platforms::Except(Platform::Linux);
    assert_eq!(conflictos(vec![linux.clone(), no_linux]), vec![]);
    // Con `active` solapado sí chocan.
    let todas = fila(c('d'), false, false, AppAction::SelectTool(Tool::Line));
    assert_eq!(
        conflictos(vec![linux, todas]),
        vec![Conflict::Duplicate(0, 1)]
    );
}

#[test]
fn fila_11_en_simbolos_y_teclas_con_nombre_shift_no_cuenta_para_el_choque() {
    for key in [c('+'), c('<'), c(','), KeyId::Home] {
        assert_eq!(
            conflictos(vec![
                fila(key, false, false, AppAction::ZoomIn),
                fila(key, false, true, AppAction::ZoomOut),
            ]),
            vec![Conflict::Duplicate(0, 1)],
            "{key:?}"
        );
    }
    // I.2a: en `Re Pág` y `Av Pág` `shift` sí cuenta (con él son el color de fondo).
    for key in [KeyId::PageUp, KeyId::PageDown] {
        assert_eq!(
            conflictos(vec![
                fila(key, false, false, AppAction::ZoomIn),
                fila(key, false, true, AppAction::ZoomOut),
            ]),
            vec![],
            "{key:?}"
        );
    }
    // Con `command`, `shift` es siempre exacto.
    assert_eq!(
        conflictos(vec![
            fila(c('+'), true, false, AppAction::ZoomIn),
            fila(c('+'), true, true, AppAction::ZoomOut),
        ]),
        vec![]
    );
}

#[test]
fn fila_11_escape_espacio_y_ctrl_c_x_v_a_dan_reserved() {
    for (nombre, key, command) in [
        ("Escape", c('\u{1b}'), false),
        ("Espacio", c(' '), false),
        ("Ctrl+C", c('c'), true),
        ("Ctrl+X", c('x'), true),
        ("Ctrl+V", c('v'), true),
        ("Ctrl+A", c('a'), true),
    ] {
        assert_eq!(
            conflictos(vec![fila(
                key,
                command,
                false,
                AppAction::SelectTool(Tool::Pencil)
            )]),
            vec![Conflict::Reserved(0)],
            "{nombre}"
        );
    }
    // El índice es el de la fila reservada.
    assert_eq!(
        conflictos(vec![
            fila(c('d'), false, false, AppAction::SelectTool(Tool::Pencil)),
            fila(c('c'), true, false, AppAction::Open),
        ]),
        vec![Conflict::Reserved(1)]
    );
    // Las mismas letras sin `command` no están reservadas.
    for key in ['c', 'x', 'v', 'a'] {
        assert_eq!(
            conflictos(vec![fila(
                c(key),
                false,
                false,
                AppAction::SelectTool(Tool::Pencil)
            )]),
            vec![],
            "{key} sola"
        );
    }
}

#[test]
fn fila_11_ctrl_q_y_cmd_q_estan_reservadas_fuera_de_linux() {
    let con_activa = |active: Platforms| {
        let mut f = fila(c('q'), true, false, AppAction::Quit);
        f.active = active;
        conflictos(vec![f])
    };
    assert_eq!(con_activa(Platforms::Only(Platform::Linux)), vec![]);
    for active in [
        Platforms::All,
        Platforms::Only(Platform::Windows),
        Platforms::Only(Platform::Mac),
        Platforms::Except(Platform::Linux),
        Platforms::Except(Platform::Mac),
    ] {
        assert_eq!(
            con_activa(active),
            vec![Conflict::Reserved(0)],
            "{active:?}"
        );
    }
}

#[test]
fn fila_11_default_scheme_no_da_ningun_conflicto() {
    assert_eq!(Keymap::default_scheme().conflicts(), vec![]);
}

// ---------------------------------------------------------------------------------------------
// Propiedades (proptest, 256 casos)
// ---------------------------------------------------------------------------------------------

/// Cualquier `Press`: un carácter ASCII imprimible, `Inicio`, `Re Pág`, `Av Pág` o ninguna
/// tecla, con los modificadores. `other_ctrl` no se combina con `command` (egui no lo da).
fn press_cualquiera() -> impl Strategy<Value = Press> {
    let tecla = prop_oneof![
        (0x20u8..0x7f).prop_map(|b| Some(KeyId::Char(char::from(b)))),
        Just(Some(KeyId::Home)),
        Just(Some(KeyId::PageUp)),
        Just(Some(KeyId::PageDown)),
        Just(None),
    ];
    (
        tecla,
        any::<bool>(),
        any::<bool>(),
        any::<bool>(),
        any::<bool>(),
        any::<bool>(),
    )
        .prop_map(|(key, command, shift, alt, other_ctrl, repeat)| Press {
            key,
            command,
            shift,
            alt,
            other_ctrl: other_ctrl && !command,
            repeat,
        })
}

fn plataforma_cualquiera() -> impl Strategy<Value = Platform> {
    prop::sample::select(PLATAFORMAS.to_vec())
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(256))]

    /// Propiedad 1: para cualquier `Press`, el resultado es el mismo con las filas en orden
    /// inverso (la tabla por defecto no tiene ambigüedad).
    #[test]
    fn propiedad_1_el_resultado_no_depende_del_orden_de_las_filas(
        p in press_cualquiera(),
        platform in plataforma_cualquiera(),
    ) {
        let km = por_defecto();
        let inversa = Keymap::new(km.rows().iter().rev().cloned().collect());
        prop_assert_eq!(
            km.action_for(&p, platform),
            inversa.action_for(&p, platform),
            "{:?} en {:?}", p, platform
        );
    }
}

#[test]
fn propiedad_1_sin_choques_y_con_command_solo_las_nueve_modernas_en_cada_plataforma() {
    for platform in PLATAFORMAS {
        let km = por_defecto();
        assert_eq!(km.conflicts(), vec![], "{platform:?}");
        // Las filas activas con `command`: ocho en Windows y macOS, nueve en Linux (`Quit`).
        let con_command: Vec<&Binding> = km
            .rows()
            .iter()
            .filter(|f| f.command && vale_en(f.active, platform))
            .collect();
        let esperado = if platform == Platform::Linux { 9 } else { 8 };
        assert_eq!(con_command.len(), esperado, "{platform:?}");
        // Ninguna de `Re Pág` y `Av Pág` lleva `command`, ni ninguna letra suelta.
        for f in km.rows() {
            if matches!(f.key, KeyId::PageUp | KeyId::PageDown | KeyId::Home) {
                assert!(!f.command, "{:?} lleva command", f.key);
            }
            if f.command {
                assert!(
                    matches!(f.key, KeyId::Char('z' | 'y' | 'o' | 'n' | 's' | 'q' | '1')),
                    "una fila con command y {:?} ({:?})",
                    f.key,
                    f.action
                );
            }
        }
    }
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(256))]

    /// Propiedad 2: lo que se muestra funciona (cada acción con `shown` en cada plataforma).
    #[test]
    fn propiedad_2_la_press_construida_desde_el_chord_da_esa_accion(
        platform in plataforma_cualquiera(),
        indice in 0usize..todas_las_acciones().len(),
        repeat in any::<bool>(),
    ) {
        let km = por_defecto();
        let action = todas_las_acciones()[indice];
        if let Some(chord) = km.shown(action, platform) {
            let texto = km.shown_text(action, platform);
            let texto = texto.expect("si hay chord hay texto");
            prop_assert!(!texto.is_empty(), "{:?} en {:?}", action, platform);
            for ch in texto.chars() {
                prop_assert!(
                    INTERFACE_CHARSET.contains(ch),
                    "{:?} en {:?}: {:?} de {:?} no está en INTERFACE_CHARSET",
                    action, platform, ch, texto
                );
            }
            let p = Press {
                key: Some(chord.key),
                command: chord.command,
                shift: chord.shift,
                alt: false,
                other_ctrl: false,
                repeat,
            };
            prop_assert_eq!(
                km.action_for(&p, platform),
                Some(action),
                "el chord {:?} de {:?} en {:?}", chord, action, platform
            );
        } else {
            prop_assert_eq!(km.shown_text(action, platform), None);
        }
    }

    /// Propiedad 2, segunda parte: para toda fila activa, la `Press` de la fila da la acción de
    /// la fila (ninguna fila es inalcanzable); una fila inactiva en la plataforma no actúa.
    #[test]
    fn propiedad_2_la_press_de_cada_fila_da_la_accion_de_la_fila_si_esta_activa(
        platform in plataforma_cualquiera(),
        indice in 0usize..25,
        repeat in any::<bool>(),
    ) {
        let km = por_defecto();
        let fila = &km.rows()[indice];
        let p = Press {
            key: Some(fila.key),
            command: fila.command,
            shift: fila.shift,
            alt: false,
            other_ctrl: false,
            repeat,
        };
        let esperado = vale_en(fila.active, platform).then_some(fila.action);
        prop_assert_eq!(
            km.action_for(&p, platform),
            esperado,
            "fila {} ({:?}) en {:?}", indice, fila.key, platform
        );
    }
}

#[test]
fn propiedad_2_exhaustiva_cada_fila_activa_es_alcanzable_y_la_inactiva_no_actua() {
    let km = por_defecto();
    for platform in PLATAFORMAS {
        for (i, fila) in km.rows().iter().enumerate() {
            let p = Press {
                key: Some(fila.key),
                command: fila.command,
                shift: fila.shift,
                alt: false,
                other_ctrl: false,
                repeat: false,
            };
            let obtenida = km.action_for(&p, platform);
            if vale_en(fila.active, platform) {
                assert_eq!(obtenida, Some(fila.action), "fila {i} en {platform:?}");
            } else {
                assert_eq!(
                    obtenida, None,
                    "fila inactiva {i} ({:?}) en {platform:?}",
                    fila.action
                );
            }
        }
    }
}

// --- Propiedad 3: independiente del orden, con un oráculo de búsqueda lineal ---

/// El oráculo distingue tres clases de tecla, como la decisión 1.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Clase {
    Letra,
    Cifra,
    Otra,
}

fn clase_de(key: KeyId) -> Clase {
    match key {
        KeyId::Char(ch) if ch.is_alphabetic() => Clase::Letra,
        KeyId::Char(ch) if ch.is_ascii_digit() => Clase::Cifra,
        _ => Clase::Otra,
    }
}

/// Si la fila casa con la `Press` en `platform`, con las reglas de la decisión 1 escritas a mano:
/// `command` exacto; con `command`, `shift` exacto y `Alt` apagado; sin él, en letras `shift`
/// exacto y `Alt` y `other_ctrl` apagados, en cifras `Alt` y `other_ctrl` apagados, y en
/// símbolos y teclas con nombre nada más (salvo `Re Pág` y `Av Pág`, con `shift` exacto).
fn casa(fila: &Binding, p: &Press, platform: Platform) -> bool {
    if !vale_en(fila.active, platform) || p.key != Some(fila.key) || p.command != fila.command {
        return false;
    }
    if fila.command {
        return p.shift == fila.shift && !p.alt;
    }
    match clase_de(fila.key) {
        Clase::Letra => p.shift == fila.shift && !p.alt && !p.other_ctrl,
        Clase::Cifra => !p.alt && !p.other_ctrl,
        // I.2a: en `Re Pág` y `Av Pág` `shift` es exacto.
        Clase::Otra if matches!(fila.key, KeyId::PageUp | KeyId::PageDown) => p.shift == fila.shift,
        Clase::Otra => true,
    }
}

/// La primera fila que casa gana.
fn oraculo(filas: &[Binding], p: &Press, platform: Platform) -> Option<AppAction> {
    filas
        .iter()
        .find(|f| casa(f, p, platform))
        .map(|f| f.action)
}

const CLAVES: [KeyId; 8] = [
    KeyId::Char('a'),
    KeyId::Char('b'),
    KeyId::Char('1'),
    KeyId::Char(','),
    KeyId::Char('<'),
    KeyId::Char('+'),
    KeyId::Home,
    KeyId::PageUp,
];
const ACCIONES_PEQUENAS: [AppAction; 8] = [
    AppAction::Undo,
    AppAction::Redo,
    AppAction::ZoomIn,
    AppAction::ZoomOut,
    AppAction::SelectTool(Tool::Pencil),
    AppAction::SelectTool(Tool::Line),
    AppAction::ColorNext,
    AppAction::ColorPrev,
];
const ACTIVAS: [Platforms; 6] = [
    Platforms::All,
    Platforms::Only(Platform::Linux),
    Platforms::Only(Platform::Windows),
    Platforms::Only(Platform::Mac),
    Platforms::Except(Platform::Linux),
    Platforms::Except(Platform::Mac),
];

fn fila_cualquiera() -> impl Strategy<Value = Binding> {
    (
        prop::sample::select(CLAVES.to_vec()),
        any::<bool>(),
        any::<bool>(),
        prop::sample::select(ACCIONES_PEQUENAS.to_vec()),
        prop::sample::select(ACTIVAS.to_vec()),
    )
        .prop_map(|(key, command, shift, action, active)| Binding {
            key,
            // Una cifra sin `command` no tiene regla de `shift` en la spec: no se genera.
            command: command || key == KeyId::Char('1'),
            shift,
            action,
            active,
            shown: Platforms::All,
        })
}

fn press_de_tabla_pequena() -> impl Strategy<Value = Press> {
    let tecla = prop_oneof![
        prop::sample::select(CLAVES.to_vec()).prop_map(Some),
        Just(Some(KeyId::Char('x'))),
        Just(None),
    ];
    (
        tecla,
        any::<bool>(),
        any::<bool>(),
        any::<bool>(),
        any::<bool>(),
        any::<bool>(),
    )
        .prop_map(|(key, command, shift, alt, other_ctrl, repeat)| Press {
            key,
            command,
            shift,
            alt,
            other_ctrl: other_ctrl && !command,
            repeat,
        })
}

fn tabla_y_permutacion() -> impl Strategy<Value = (Vec<Binding>, Vec<Binding>)> {
    prop::collection::vec(fila_cualquiera(), 0..=12).prop_flat_map(|filas| {
        let copia = filas.clone();
        (Just(filas), Just(copia).prop_shuffle())
    })
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(256))]

    /// Propiedad 3: con `conflicts()` vacío, `action_for` da lo mismo con cualquier permutación
    /// de las filas y coincide con el oráculo en las tres plataformas; si no, gana la primera.
    #[test]
    fn propiedad_3_independiente_del_orden_y_igual_al_oraculo(
        (filas, permutadas) in tabla_y_permutacion(),
        p in press_de_tabla_pequena(),
    ) {
        let km = Keymap::new(filas.clone());
        let mezclada = Keymap::new(permutadas.clone());
        for platform in PLATAFORMAS {
            // Gane la primera fila que casa: vale con o sin conflictos.
            prop_assert_eq!(
                km.action_for(&p, platform),
                oraculo(&filas, &p, platform),
                "{:?} en {:?} con {:?}", p, platform, filas
            );
            if km.conflicts().is_empty() {
                prop_assert_eq!(
                    mezclada.action_for(&p, platform),
                    km.action_for(&p, platform),
                    "permutación distinta: {:?} en {:?}", p, platform
                );
                prop_assert_eq!(
                    oraculo(&permutadas, &p, platform),
                    oraculo(&filas, &p, platform),
                    "el oráculo depende del orden sin conflictos: {:?} con {:?}", p, filas
                );
            }
        }
    }
}
