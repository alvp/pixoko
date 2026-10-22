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
//! - Fila 8 (parte de px-ui, módulo `row_8`): con un diálogo o el panel modal abierto toda fila
//!   da `Blocked`; `gate_keys` con un campo de texto enfocado de verdad; `Start`, `Fin`,
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
//!   `action_for_shortcut` exigen `Start` con `Ctrl` suelto = `ZoomFit`): `other_ctrl` no cambia
//!   el resultado de `, ] [ < > + = -`, `Start`, `Re Pág` y `Av Pág`.
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
use px_ui::toolbar::{TOOL_BUTTONS, ZOOM_BUTTONS, tooltip_text};
use px_ui::view::Zoom;

mod common;
use common::toolbox::{center_u, toolbox_with};
use common::zones::toolbox_single;
use common::{Painted, button_2, context, in_points, modifiers_2, mover, n_de, painted};

const PLATFORMS: [Platform; 3] = [Platform::Linux, Platform::Windows, Platform::Mac];
const TEST_TOOLS: [Tool; 5] = [
    Tool::Pencil,
    Tool::Line,
    Tool::Fill,
    Tool::Dropper,
    Tool::Eraser,
];
const BACKGROUNDS: [CanvasBackground; 4] = [
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
fn single(key: KeyId) -> Press {
    press(key, false, false, false, false)
}

/// Con `Ctrl`/`Cmd` (`command`), con o sin `Mayús` y `Alt`.
fn with_command(key: KeyId, shift: bool, alt: bool) -> Press {
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

fn default() -> Keymap {
    Keymap::default_scheme()
}

fn action_2(km: &Keymap, press: &Press, platform: Platform) -> Option<AppAction> {
    km.action_for(press, platform)
}

/// Todas las acciones posibles (las que existen tras la unidad), con las ranuras 1 a 9 y los
/// cuatro fondos fijos.
fn all_the_actions() -> Vec<AppAction> {
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
    v.extend(TEST_TOOLS.map(AppAction::SelectTool));
    v.extend((1..=9).map(AppAction::SelectColor));
    v.extend(BACKGROUNDS.map(AppAction::CanvasBackground));
    v
}

fn assert_datum<T: Clone + std::fmt::Debug + PartialEq>() {}
fn assert_plane<T: Clone + Copy + std::fmt::Debug + PartialEq + Eq>() {}
fn assert_eq<T: Eq>() {}

fn row_2(key: KeyId, command: bool, shift: bool, action: AppAction) -> Binding {
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
struct Expected {
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
) -> Expected {
    Expected {
        action,
        key,
        command,
        shift,
        active,
        shown,
    }
}

/// Las 31 filas del «Esquema por defecto» (25 de I.1, 5 de I.2a y la `k` del Borrador), en el orden de la spec (dentro de cada fila doble, la
/// siguiente antes que la anterior; `Av Pág` y `Re Pág` después de `]` y `[`).
fn scheme_expected() -> Vec<Expected> {
    let all_2 = Platforms::All;
    let t = Some(Platforms::All);
    let lin = Platforms::Only(Platform::Linux);
    vec![
        e(
            AppAction::SelectTool(Tool::Pencil),
            c('d'),
            false,
            false,
            all_2,
            t,
        ),
        e(
            AppAction::SelectTool(Tool::Line),
            c('v'),
            false,
            false,
            all_2,
            t,
        ),
        e(
            AppAction::SelectTool(Tool::Fill),
            c('f'),
            false,
            false,
            all_2,
            t,
        ),
        e(
            AppAction::SelectTool(Tool::Dropper),
            c(','),
            false,
            false,
            all_2,
            t,
        ),
        e(
            AppAction::SelectTool(Tool::Eraser),
            c('k'),
            false,
            false,
            all_2,
            t,
        ),
        e(AppAction::Undo, c('z'), true, false, all_2, t),
        e(AppAction::Undo, c('u'), false, false, all_2, None),
        e(
            AppAction::Redo,
            c('y'),
            true,
            false,
            all_2,
            Some(Platforms::Except(Platform::Mac)),
        ),
        e(
            AppAction::Redo,
            c('z'),
            true,
            true,
            all_2,
            Some(Platforms::Only(Platform::Mac)),
        ),
        e(AppAction::ColorNext, c(']'), false, false, all_2, t),
        e(AppAction::ColorPrev, c('['), false, false, all_2, t),
        e(
            AppAction::ColorNext,
            KeyId::PageDown,
            false,
            false,
            all_2,
            None,
        ),
        e(
            AppAction::ColorPrev,
            KeyId::PageUp,
            false,
            false,
            all_2,
            None,
        ),
        e(
            AppAction::BackgroundNext,
            KeyId::PageDown,
            false,
            true,
            all_2,
            None,
        ),
        e(
            AppAction::BackgroundPrev,
            KeyId::PageUp,
            false,
            true,
            all_2,
            None,
        ),
        e(AppAction::BackgroundNext, c('}'), false, false, all_2, t),
        e(AppAction::BackgroundPrev, c('{'), false, false, all_2, t),
        e(AppAction::ZoomIn, c('>'), false, false, all_2, t),
        e(AppAction::ZoomIn, c('+'), false, false, all_2, None),
        e(AppAction::ZoomIn, c('='), false, false, all_2, None),
        e(AppAction::ZoomOut, c('<'), false, false, all_2, t),
        e(AppAction::ZoomOut, c('-'), false, false, all_2, None),
        e(AppAction::ZoomFit, KeyId::Home, false, false, all_2, t),
        e(AppAction::ZoomActual, c('1'), true, false, all_2, t),
        e(AppAction::CenterOnCursor, c('n'), false, false, all_2, t),
        e(AppAction::ClearPage, c('k'), false, true, all_2, t),
        e(AppAction::ToggleLayerPanel, c('\''), false, false, all_2, t),
        e(AppAction::NewLayer, c(';'), false, false, all_2, t),
        e(AppAction::LayerUp, c(')'), false, false, all_2, t),
        e(AppAction::LayerDown, c('('), false, false, all_2, t),
        e(AppAction::Open, c('o'), true, false, all_2, t),
        e(AppAction::Save, c('s'), true, false, all_2, t),
        e(AppAction::SaveAs, c('s'), true, true, all_2, t),
        e(AppAction::NewImage, c('n'), true, false, all_2, t),
        e(AppAction::Quit, c('q'), true, false, lin, Some(lin)),
    ]
}

#[test]
fn row_1_the_types_of_the_table_have_the_derives_of_the_spec() {
    assert_plane::<KeyId>();
    assert_plane::<Platforms>();
    assert_plane::<Chord>();
    assert_plane::<Press>();
    assert_datum::<Conflict>();
    assert_datum::<Binding>();
    assert_datum::<Keymap>();
    // `Conflict` es `Eq` y `Press` tiene todos sus campos públicos (se construye a mano arriba).
    assert_eq::<Conflict>();
    // `Chord` es lo que antes era `menu::Shortcut`: el mismo tipo.
    let chord = Chord {
        key: c('z'),
        command: true,
        shift: false,
    };
    let shortcut_2: Shortcut = chord;
    assert_eq!(shortcut_2, chord);
}

#[test]
fn row_1_the_default_table_is_the_31_rows_of_the_spec_in_order() {
    let km = default();
    let expected_2 = scheme_expected();
    assert_eq!(expected_2.len(), 35);
    assert_eq!(km.rows().len(), 35, "filas de la tabla por defecto");
    for (i, (real, expected)) in km.rows().iter().zip(&expected_2).enumerate() {
        assert_eq!(real.action, expected.action, "fila {i}: acción");
        assert_eq!(
            real.key, expected.key,
            "fila {i} ({:?}): tecla",
            real.action
        );
        assert_eq!(
            real.command, expected.command,
            "fila {i} ({:?}): command",
            real.action
        );
        assert_eq!(
            real.shift, expected.shift,
            "fila {i} ({:?}): shift",
            real.action
        );
        assert_eq!(
            real.active, expected.active,
            "fila {i} ({:?}): active",
            real.action
        );
        if let Some(shown) = expected.shown {
            assert_eq!(
                real.shown, shown,
                "fila {i} ({:?}, {:?}): shown",
                real.action, real.key
            );
        }
    }
}

#[test]
fn row_1_there_are_22_rows_without_command_and_9_with_it_and_quit_only_on_linux() {
    let km = default();
    let without = km.rows().iter().filter(|f| !f.command).count();
    let with = km.rows().iter().filter(|f| f.command).count();
    assert_eq!((without, with), (26, 9));
    let quit: Vec<&Binding> = km
        .rows()
        .iter()
        .filter(|f| f.action == AppAction::Quit)
        .collect();
    assert_eq!(quit.len(), 1);
    assert_eq!(quit[0].active, Platforms::Only(Platform::Linux));
}

#[test]
fn row_1_there_are_31_active_rows_on_linux_and_30_on_windows_and_macos() {
    let km = default();
    for (platform, expected_2) in [
        (Platform::Linux, 35),
        (Platform::Windows, 34),
        (Platform::Mac, 34),
    ] {
        let active = km
            .rows()
            .iter()
            .filter(|f| vale_en(f.active, platform))
            .count();
        assert_eq!(active, expected_2, "filas activas en {platform:?}");
    }
}

#[test]
fn row_1_the_default_table_has_no_conflicts() {
    assert_eq!(default().conflicts(), vec![]);
}

#[test]
fn row_1_new_with_the_rows_of_the_table_gives_an_equal_table() {
    let km = default();
    let copy = Keymap::new(km.rows().to_vec());
    assert_eq!(copy, km);
    assert_eq!(copy.rows(), km.rows());
    assert_eq!(km.clone(), km);
}

#[test]
fn row_1_the_rows_where_shown_is_none_are_not_the_one_that_shows_shown() {
    // `u`, `Av Pág`, `Re Pág`, `+`, `=` y `-` funcionan pero no se muestran: la fila mostrada es
    // la otra (`Ctrl+Z`, `]`, `[`, `>`, `<`).
    let km = default();
    for platform in PLATFORMS {
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
fn row_1_shown_of_redo_and_quit_depends_on_the_platform() {
    let km = default();
    for platform in PLATFORMS {
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
fn assigned_without_command() -> Vec<(KeyId, AppAction)> {
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

fn is_letter(key: KeyId) -> bool {
    matches!(key, KeyId::Char(ch) if ch.is_alphabetic())
}

#[test]
fn row_2_each_assigned_key_without_modifiers_gives_its_action_on_the_three_platforms() {
    let km = default();
    for platform in PLATFORMS {
        for (key, expected) in assigned_without_command() {
            assert_eq!(
                action_2(&km, &single(key), platform),
                Some(expected),
                "{key:?} sola en {platform:?}"
            );
        }
    }
}

#[test]
fn row_2_in_letters_shift_alt_command_and_other_ctrl_give_none_except_ctrl_n() {
    let km = default();
    for platform in PLATFORMS {
        for key in ['d', 'v', 'f', 'u', 'n'] {
            let k = c(key);
            for (name_2, p) in [
                ("shift", press(k, false, true, false, false)),
                ("alt", press(k, false, false, true, false)),
                ("shift y alt", press(k, false, true, true, false)),
                ("other_ctrl", press(k, false, false, false, true)),
                ("shift y other_ctrl", press(k, false, true, false, true)),
                ("alt y other_ctrl", press(k, false, false, true, true)),
                ("command", with_command(k, false, false)),
                ("command y shift", with_command(k, true, false)),
                ("command y alt", with_command(k, false, true)),
            ] {
                let expected_2 = if key == 'n' && name_2 == "command" {
                    Some(AppAction::NewImage)
                } else {
                    None
                };
                assert_eq!(
                    action_2(&km, &p, platform),
                    expected_2,
                    "`{key}` con {name_2} en {platform:?}"
                );
            }
        }
    }
}

#[test]
fn row_2_alt_plus_a_letter_is_not_that_letter_and_does_nothing() {
    // Decisión 1: en macOS `Option`+`d` escribe `∂`; en Linux y Windows `Alt`+`d` queda sin acción.
    let km = default();
    for platform in PLATFORMS {
        for (key, _) in assigned_without_command()
            .into_iter()
            .filter(|(k, _)| is_letter(*k))
        {
            assert_eq!(
                action_2(&km, &press(key, false, false, true, false), platform),
                None,
                "Alt+{key:?} en {platform:?}"
            );
        }
    }
}

#[test]
fn row_2_in_symbols_and_named_keys_shift_and_alt_do_not_change_the_result() {
    let km = default();
    for platform in PLATFORMS {
        for (key, expected) in assigned_without_command()
            .into_iter()
            .filter(|(k, _)| !is_letter(*k))
        {
            for (shift, alt) in [(false, false), (true, false), (false, true), (true, true)] {
                // I.2a: en `Re Pág` y `Av Pág` `Mayús` cuenta (con ella son el fondo); `Alt` no.
                let expected = match (key, shift) {
                    (KeyId::PageUp, true) => AppAction::BackgroundPrev,
                    (KeyId::PageDown, true) => AppAction::BackgroundNext,
                    _ => expected,
                };
                assert_eq!(
                    action_2(&km, &press(key, false, shift, alt, false), platform),
                    Some(expected),
                    "{key:?} con shift={shift} alt={alt} en {platform:?}"
                );
            }
        }
    }
}

#[test]
fn row_2_in_symbols_and_named_keys_command_gives_none() {
    let km = default();
    for platform in PLATFORMS {
        for (key, _) in assigned_without_command()
            .into_iter()
            .filter(|(k, _)| !is_letter(*k))
        {
            for (shift, alt) in [(false, false), (true, false), (false, true), (true, true)] {
                assert_eq!(
                    action_2(&km, &with_command(key, shift, alt), platform),
                    None,
                    "Ctrl+{key:?} con shift={shift} alt={alt} en {platform:?}"
                );
            }
        }
    }
}

#[test]
fn row_2_in_symbols_and_named_keys_other_ctrl_does_not_count_decision_1() {
    // Véase la cabecera: la decisión 1 dice «no cuenta en símbolos ni en teclas con nombre»
    // (la tabla de la fila 2 decía `None`; las pruebas de `action_for_shortcut` exigen
    // `Start` con `Ctrl` suelto = `ZoomFit`).
    let km = default();
    for platform in PLATFORMS {
        for (key, expected) in assigned_without_command()
            .into_iter()
            .filter(|(k, _)| !is_letter(*k))
        {
            assert_eq!(
                action_2(&km, &press(key, false, false, false, true), platform),
                Some(expected),
                "{key:?} con other_ctrl en {platform:?}"
            );
        }
    }
}

#[test]
fn row_2_the_result_does_not_depend_on_repeat() {
    let km = default();
    for platform in PLATFORMS {
        for (key, expected) in assigned_without_command() {
            let mut p = single(key);
            p.repeat = true;
            assert_eq!(
                action_2(&km, &p, platform),
                Some(expected),
                "{key:?} repetida"
            );
        }
    }
}

/// Todas las combinaciones de `shift`, `alt` y `other_ctrl` (sin `command`).
fn combinations_without_command(key: KeyId) -> Vec<Press> {
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
fn row_2_what_is_unassigned_gives_none_with_any_modifier_on_the_three_platforms() {
    let km = default();
    // Sin `command`: `p l i e` y `P L I E` (sin convivencia), las cifras 1 a 9, `K` `D`, las
    // teclas del hito 4, y `. / { }`.
    let mut without_assign: Vec<KeyId> = "plieKDPLIE./qrcwbtgmasxyzo"
        .chars()
        .map(KeyId::Char)
        .collect();
    without_assign.extend(('1'..='9').map(KeyId::Char));
    for platform in PLATFORMS {
        for key in &without_assign {
            for p in combinations_without_command(*key) {
                assert_eq!(
                    action_2(&km, &p, platform),
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
                    assert_eq!(action_2(&km, &p, platform), None, "{p:?} en {platform:?}");
                }
            }
        }
        for reserved in [c('\u{1b}'), c(' ')] {
            for p in combinations_without_command(reserved) {
                assert_eq!(action_2(&km, &p, platform), None, "{p:?} en {platform:?}");
            }
        }
    }
}

#[test]
fn row_2_with_command_letters_and_digits_without_a_row_give_none() {
    // Nada se cuela con `Ctrl`: solo las nueve modernas (fila 3). La lista reservada
    // (`Ctrl+C/X/V/A`) tampoco hace nada.
    let km = default();
    let mut without_row: Vec<KeyId> = "pliedvfukcxvawbtgmrKD,.[]<>+=-/"
        .chars()
        .map(KeyId::Char)
        .collect();
    without_row.extend(('2'..='9').map(KeyId::Char));
    without_row.extend([KeyId::Home, KeyId::PageUp, KeyId::PageDown]);
    for platform in PLATFORMS {
        for key in &without_row {
            for (shift, alt) in [(false, false), (true, false), (false, true), (true, true)] {
                assert_eq!(
                    action_2(&km, &with_command(*key, shift, alt), platform),
                    None,
                    "Ctrl+{key:?} con shift={shift} alt={alt} en {platform:?}"
                );
            }
        }
    }
}

#[test]
fn row_2_digits_1_to_9_without_modifiers_do_nothing() {
    let km = default();
    for platform in PLATFORMS {
        for figure in '1'..='9' {
            assert_eq!(
                action_2(&km, &single(c(figure)), platform),
                None,
                "cifra {figure} en {platform:?} (antes elegía la ranura)"
            );
        }
    }
}

#[test]
fn row_2_capitals_in_the_character_do_not_match_the_lowercase_row() {
    // Las letras de `KeyId` van en minúscula (`presses` las pasa a minúscula): `Char('D')` no
    // es ninguna fila.
    let km = default();
    for platform in PLATFORMS {
        for key in ['D', 'V', 'F', 'U', 'N'] {
            assert_eq!(
                action_2(&km, &single(c(key)), platform),
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
fn modern_expected(key: char, shift: bool, alt: bool, platform: Platform) -> Option<AppAction> {
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
fn row_3_the_nine_modern_ones_give_their_action_on_the_three_platforms() {
    let km = default();
    let cases = [
        ('z', false, AppAction::Undo),
        ('y', false, AppAction::Redo),
        ('z', true, AppAction::Redo),
        ('o', false, AppAction::Open),
        ('n', false, AppAction::NewImage),
        ('s', false, AppAction::Save),
        ('s', true, AppAction::SaveAs),
        ('1', false, AppAction::ZoomActual),
    ];
    for platform in PLATFORMS {
        for (key, shift, expected) in cases {
            assert_eq!(
                action_2(&km, &with_command(c(key), shift, false), platform),
                Some(expected),
                "Ctrl{}+{key} en {platform:?}",
                if shift { "+Mayús" } else { "" }
            );
        }
        assert_eq!(
            action_2(&km, &with_command(c('q'), false, false), platform),
            (platform == Platform::Linux).then_some(AppAction::Quit),
            "Ctrl+Q en {platform:?}"
        );
    }
}

#[test]
fn row_3_with_alt_with_extra_shift_and_with_foreign_keys_the_result_is_the_previous_one() {
    let km = default();
    let keys = ['z', 'y', 'o', 'n', 's', 'q', '1', 'd', ',', ']', 'a', 'x'];
    for platform in PLATFORMS {
        for key in keys {
            for shift in [false, true] {
                for alt in [false, true] {
                    assert_eq!(
                        action_2(&km, &with_command(c(key), shift, alt), platform),
                        modern_expected(key, shift, alt, platform),
                        "Ctrl+{key} con shift={shift} alt={alt} en {platform:?}"
                    );
                }
            }
        }
    }
}

#[test]
fn row_3_ctrl_q_on_windows_and_macos_does_nothing_with_any_modifier() {
    let km = default();
    for platform in [Platform::Windows, Platform::Mac] {
        for shift in [false, true] {
            for alt in [false, true] {
                assert_eq!(
                    action_2(&km, &with_command(c('q'), shift, alt), platform),
                    None,
                    "Ctrl+Q con shift={shift} alt={alt} en {platform:?}"
                );
            }
        }
    }
}

#[test]
fn row_3_ctrl_d_ctrl_comma_and_ctrl_bracket_do_nothing() {
    let km = default();
    for platform in PLATFORMS {
        for key in ['d', ',', ']', '[', 'u', 'v', 'f'] {
            assert_eq!(
                action_2(&km, &with_command(c(key), false, false), platform),
                None,
                "Ctrl+{key} en {platform:?}"
            );
        }
    }
}

#[test]
fn row_3_the_rows_with_command_are_exactly_the_nine_modern_ones() {
    let mut expected_2_2 = vec![
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
    let mut real: Vec<(AppAction, char, bool)> = default()
        .rows()
        .iter()
        .filter(|f| f.command)
        .map(|f| match f.key {
            KeyId::Char(ch) => (f.action, ch, f.shift),
            other => panic!("una fila con command y la tecla {other:?}"),
        })
        .collect();
    let key = |t: &(AppAction, char, bool)| format!("{t:?}");
    expected_2_2.sort_by_key(key);
    real.sort_by_key(key);
    assert_eq!(real, expected_2_2);
}

// ---------------------------------------------------------------------------------------------
// Fila 5: rótulos desde la tabla
// ---------------------------------------------------------------------------------------------

/// Lo que `shown_text` tiene que dar para cada acción en `platform` (`None` = sin rótulo).
fn labels_expected(platform: Platform) -> Vec<(AppAction, Option<String>)> {
    let control = if platform == Platform::Mac {
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
        (AppAction::SelectTool(Tool::Eraser), s("k")),
        (AppAction::Undo, Some(format!("{control}+Z"))),
        (AppAction::Redo, Some(redo)),
        (AppAction::ZoomIn, s(">")),
        (AppAction::ZoomOut, s("<")),
        (AppAction::ZoomFit, s("Inicio")),
        (AppAction::ZoomActual, Some(format!("{control}+1"))),
        (
            AppAction::Quit,
            (platform == Platform::Linux).then(|| "Ctrl+Q".to_string()),
        ),
        (AppAction::ColorNext, s("]")),
        (AppAction::ColorPrev, s("[")),
        (AppAction::CenterOnCursor, s("n")),
        (AppAction::Open, Some(format!("{control}+O"))),
        (AppAction::Save, Some(format!("{control}+S"))),
        (AppAction::SaveAs, Some(format!("{control}+Mayús+S"))),
        (AppAction::NewImage, Some(format!("{control}+N"))),
        // Sin fila.
        (AppAction::ConvertMode, None),
        (AppAction::ShowLicenses, None),
        (AppAction::CloseLicenses, None),
        (AppAction::BackgroundColor, None),
        (AppAction::SelectColor(3), None),
    ];
    v.extend(BACKGROUNDS.map(|f| (AppAction::CanvasBackground(f), None)));
    v
}

#[test]
fn row_5_shown_text_of_each_action_on_the_three_platforms() {
    let km = default();
    for platform in PLATFORMS {
        for (action, expected_2) in labels_expected(platform) {
            assert_eq!(
                km.shown_text(action, platform),
                expected_2,
                "{action:?} en {platform:?}"
            );
        }
    }
}

#[test]
fn row_5_the_digit_and_page_up_page_down_keys_are_not_shown_anywhere() {
    let km = default();
    for platform in PLATFORMS {
        for slot in 1..=9 {
            assert_eq!(km.shown_text(AppAction::SelectColor(slot), platform), None);
        }
        for action in all_the_actions() {
            if let Some(text_2) = km.shown_text(action, platform) {
                assert!(
                    !text_2.contains("Pág"),
                    "{action:?} en {platform:?} enseña {text_2:?}"
                );
            }
        }
    }
}

#[test]
fn row_5_shown_text_is_the_text_of_shown() {
    let km = default();
    for platform in PLATFORMS {
        for action in all_the_actions() {
            let expected_2 = km
                .shown(action, platform)
                .map(|chord| shortcut_text(chord, platform));
            assert_eq!(
                km.shown_text(action, platform),
                expected_2,
                "{action:?} en {platform:?}"
            );
        }
    }
}

#[test]
fn row_5_the_tool_buttons_show_d_v_f_comma_and_k() {
    let km = default();
    let expected_2 = [Some("d"), Some("v"), Some("f"), Some(","), Some("k")];
    for platform in PLATFORMS {
        let real: Vec<Option<String>> = TOOL_BUTTONS
            .iter()
            .map(|b| km.shown_text(b.action, platform))
            .collect();
        let expected_2: Vec<Option<String>> =
            expected_2.iter().map(|t| t.map(str::to_string)).collect();
        assert_eq!(real, expected_2, "{platform:?}");
    }
}

#[test]
fn row_5_the_zoom_buttons_have_no_letter_but_the_table_shows_their_shortcuts() {
    // Los botones de zoom no pintan letra (el menú Ver enseña los atajos): su acción sí tiene
    // rótulo en la tabla.
    let km = default();
    let actions: Vec<AppAction> = ZOOM_BUTTONS.iter().map(|b| b.action).collect();
    assert_eq!(
        actions,
        [
            AppAction::ZoomIn,
            AppAction::ZoomOut,
            AppAction::ZoomFit,
            AppAction::ZoomActual
        ]
    );
    for action in actions {
        assert!(
            km.shown_text(action, Platform::Linux).is_some(),
            "{action:?}"
        );
    }
}

#[test]
fn row_5_the_menu_with_shortcut_for_and_shortcut_text_comes_from_the_default_table() {
    let km = default();
    for platform in PLATFORMS {
        for menu in &MENUS {
            for item in menu.items {
                let of_the_menu =
                    shortcut_for(item.action, platform).map(|s| shortcut_text(s, platform));
                assert_eq!(
                    of_the_menu,
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
fn row_5_view_shows_greater_and_smaller_and_not_more_or_less() {
    let see = MENUS.iter().find(|m| m.title == "Ver").expect("menú Ver");
    for platform in PLATFORMS {
        let texts: Vec<Option<String>> = see
            .items
            .iter()
            .map(|i| shortcut_for(i.action, platform).map(|s| shortcut_text(s, platform)))
            .collect();
        let control = if platform == Platform::Mac {
            "Cmd"
        } else {
            "Ctrl"
        };
        assert_eq!(
            texts,
            [
                Some(">".to_string()),
                Some("<".to_string()),
                Some("Inicio".to_string()),
                Some(format!("{control}+1")),
            ],
            "{platform:?}"
        );
    }
}

#[test]
fn row_5_the_dropdown_widths_do_not_change() {
    // 12 u de márgenes + 7 u por carácter del mayor `rótulo + 2 + shortcut_2`. Escritos a mano.
    let expected_2 = |title: &str, platform: Platform| -> i64 {
        let mac = platform == Platform::Mac;
        match (title, mac) {
            ("Archivo", false) => 201,
            ("Archivo", true) => 194,
            ("Editar", false) => 124,
            ("Editar", true) => 152,
            ("Imagen", _) => 131,
            ("Capa", _) => 145,
            ("Ver", _) => 208,
            ("Ayuda", _) => 201,
            other_2 => panic!("menú {other_2:?}"),
        }
    };
    for platform in PLATFORMS {
        for menu in &MENUS {
            assert_eq!(
                popup_width_u(menu, platform),
                expected_2(menu.title, platform),
                "{} en {platform:?}",
                menu.title
            );
        }
    }
}

/// Todos los rótulos que la tabla puede enseñar.
fn all_the_labels() -> Vec<String> {
    let km = default();
    let mut v = Vec::new();
    for platform in PLATFORMS {
        for action in all_the_actions() {
            if let Some(text_2) = km.shown_text(action, platform) {
                v.push(text_2);
            }
        }
    }
    v
}

#[test]
fn row_5_no_label_loses_a_character_through_replace_missing() {
    let has = |ch: char| INTERFACE_CHARSET.contains(ch);
    let labels = all_the_labels();
    assert!(!labels.is_empty());
    for text_2 in labels {
        assert!(!text_2.is_empty());
        assert_eq!(replace_missing(&text_2, has), text_2);
        assert!(!text_2.contains('?'), "{text_2:?} lleva un `?`");
    }
}

#[test]
fn row_5_with_the_real_font_printable_changes_no_label() {
    let ctx = context(1.0);
    for text_2 in all_the_labels() {
        assert_eq!(printable(&ctx, &text_2), text_2);
    }
}

// ---------------------------------------------------------------------------------------------
// Banco de egui sin ventana: menú y columna con la tabla recibida (fila 5 pintada y fila 11)
// ---------------------------------------------------------------------------------------------

const TEST_SCREEN: Vec2 = vec2(600.0, 400.0);

fn state_ui() -> UiState {
    UiState {
        can_undo: true,
        can_redo: true,
        zoom: Zoom::from_int(5),
        tool: Tool::Pencil,
        locks: Locks::default(),
    }
}

struct BenchUi {
    ctx: Context,
    ppp: f32,
    /// Reloj de egui: avanza en cada fotograma para que terminen las animaciones del desplegable.
    time_2: f64,
}

impl BenchUi {
    fn new_2(ppp: f32) -> BenchUi {
        let ctx = context(ppp);
        ctx.set_visuals_of(egui::Theme::Dark, visuals(ppp));
        BenchUi {
            ctx,
            ppp,
            time_2: 0.0,
        }
    }

    fn frame(&mut self, events: Vec<Event>, mut draws: impl FnMut(&mut Ui)) -> Painted {
        self.time_2 += 0.25;
        let input_2 = RawInput {
            screen_rect: Some(Rect::from_min_size(Pos2::ZERO, TEST_SCREEN)),
            events,
            time: Some(self.time_2),
            ..RawInput::default()
        };
        let mut output_2 = self.ctx.run_ui(input_2, |ui| {
            apply_scale(ui.ctx());
            draws(ui);
        });
        output_2.textures_delta.clear();
        painted(&output_2.shapes)
    }

    fn menu(
        &mut self,
        events: Vec<Event>,
        km: &Keymap,
        platform: Platform,
    ) -> (Painted, Option<AppAction>) {
        let state_2 = state_ui();
        let mut chosen_2 = None;
        let p = self.frame(events, |ui| {
            chosen_2 = show_menu_in(ui, &state_2, platform, CanvasBackground::default(), km);
        });
        (p, chosen_2)
    }

    fn menu_in_rest(&mut self, km: &Keymap, platform: Platform) -> Painted {
        let mut ultimo = self.menu(vec![], km, platform).0;
        for _ in 0..5 {
            ultimo = self.menu(vec![], km, platform).0;
        }
        ultimo
    }

    /// Clic real (mover, pulsar y soltar) en `pos`; devuelve lo que eligió el menú.
    fn click_on_the_menu(
        &mut self,
        pos: Pos2,
        km: &Keymap,
        platform: Platform,
    ) -> Option<AppAction> {
        let mut chosen_2 = None;
        for events in [
            vec![mover(pos)],
            vec![button_2(pos, true)],
            vec![button_2(pos, false)],
        ] {
            chosen_2 = chosen_2.or(self.menu(events, km, platform).1);
        }
        chosen_2
    }

    /// Abre el menú `title` con un clic real y devuelve lo pintado ya asentado.
    fn opens_menu(&mut self, title: &str, km: &Keymap, platform: Platform) -> Painted {
        let bar = self.menu_in_rest(km, platform);
        let center = bar
            .text_with(title)
            .unwrap_or_else(|| panic!("no se pinta {title:?}: {:?}", bar.all_the_texts()))
            .rect()
            .center();
        self.click_on_the_menu(center, km, platform);
        self.menu_in_rest(km, platform)
    }

    fn column(
        &mut self,
        events: Vec<Event>,
        km: &Keymap,
        platform: Platform,
    ) -> (Painted, Option<AppAction>) {
        let state_2 = state_ui();
        let mut chosen_2 = None;
        let p = self.frame(events, |ui| {
            chosen_2 = toolbox_with(ui, &state_2, "×5", km, platform);
        });
        (p, chosen_2)
    }

    fn column_in_rest(&mut self, km: &Keymap, platform: Platform) -> Painted {
        self.column(vec![], km, platform);
        self.column(vec![], km, platform).0
    }

    /// Clic en el centro del botón de herramienta `index_2` (0 = Lápiz).
    fn click_on_the_tool(
        &mut self,
        index_2: i64,
        km: &Keymap,
        platform: Platform,
    ) -> Vec<AppAction> {
        self.column_in_rest(km, platform);
        let n = n_de(self.ppp);
        // Botones de 24 × 24 u en una rejilla de cuatro columnas (specs/interfaz-iconos.md).
        let toolbox_x = toolbox_single(
            (
                (TEST_SCREEN.x * self.ppp).round() as i64,
                (TEST_SCREEN.y * self.ppp).round() as i64,
            ),
            n as u32,
        )
        .x;
        let (cx_u, cy_u) = center_u(index_2 as usize);
        let pos = in_points(self.ppp, toolbox_x + cx_u * n, cy_u * n);
        let mut actions = Vec::new();
        for events in [
            vec![mover(pos)],
            vec![button_2(pos, true)],
            vec![button_2(pos, false)],
        ] {
            actions.extend(self.column(events, km, platform).1);
        }
        actions
    }
}

/// La tabla por defecto con una tecla cambiada por otra (sin `command`).
fn with_keys_changed(a: char, b: char) -> Keymap {
    let rows_2 = default()
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
    Keymap::new(rows_2)
}

#[test]
fn row_5_the_menu_painted_with_the_default_table_shows_the_shortcuts_of_the_table() {
    let km = default();
    let mut bench = BenchUi::new_2(1.0);
    let edit = bench.opens_menu("Editar", &km, Platform::Linux);
    assert!(
        edit.text_with("Ctrl+Z").is_some(),
        "{:?}",
        edit.all_the_texts()
    );
    assert!(edit.text_with("Ctrl+Y").is_some());
    let mut bench = BenchUi::new_2(1.0);
    let see = bench.opens_menu("Ver", &km, Platform::Linux);
    for text_2 in [">", "<", "Inicio", "Ctrl+1"] {
        assert!(
            see.paints(text_2) || see.text_with(text_2).is_some(),
            "Ver no enseña {text_2:?}: {:?}",
            see.all_the_texts()
        );
    }
    assert!(
        !see.paints("+") && !see.paints("-"),
        "Ver ya no enseña `+` ni `-`: {:?}",
        see.all_the_texts()
    );
}

#[test]
fn row_5_the_column_with_the_default_table_says_d_v_f_comma_and_k_in_the_tooltip() {
    // Con los iconos (I.4) la letra del atajo ya no se pinta en el botón: se dice en el tooltip,
    // que sale de la misma tabla (`tooltip_text`).
    let km = default();
    let first_line = |i: usize, km: &Keymap| {
        tooltip_text(&TOOL_BUTTONS[i], km, Platform::Linux)
            .lines()
            .next()
            .expect("hay texto")
            .to_string()
    };
    assert_eq!(first_line(0, &km), "Lápiz (d)");
    assert_eq!(first_line(1, &km), "Línea (v)");
    assert_eq!(first_line(2, &km), "Relleno (f)");
    assert_eq!(first_line(3, &km), "Cuentagotas (,)");
    assert_eq!(first_line(4, &km), "Borrador (k)");
    for ppp in [1.0, 2.0] {
        let mut bench = BenchUi::new_2(ppp);
        let p = bench.column_in_rest(&km, Platform::Linux);
        // Ninguna letra suelta pintada en la columna: ni las del atajo ni otras.
        for foreign in [
            "d", "v", "f", ",", "k", "e", "E", "p", "P", ">", "<", "Inicio",
        ] {
            assert!(
                !p.paints(foreign),
                "ppp {ppp}: la columna pinta {foreign:?}"
            );
        }
    }
}

// ---------------------------------------------------------------------------------------------
// Fila 8 (parte de px-ui): bloqueos, gate_keys y el diálogo
// ---------------------------------------------------------------------------------------------

mod row_8 {
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

    fn with_text(k: Key, modifiers: Modifiers, text_2: &str) -> Vec<Event> {
        vec![key(k, modifiers), Event::Text(text_2.to_string())]
    }

    /// Las secuencias de la fila 4 y la acción que dan sin campo enfocado.
    fn sequences() -> Vec<(&'static str, Vec<Event>, AppAction)> {
        vec![
            (
                "d",
                with_text(Key::D, Modifiers::NONE, "d"),
                AppAction::SelectTool(Tool::Pencil),
            ),
            (
                "]",
                with_text(Key::CloseBracket, Modifiers::NONE, "]"),
                AppAction::ColorNext,
            ),
            (
                "[ (AltGr)",
                with_text(Key::OpenBracket, Modifiers::ALT, "["),
                AppAction::ColorPrev,
            ),
            (
                "< (ISO)",
                with_text(Key::IntlBackslash, Modifiers::NONE, "<"),
                AppAction::ZoomOut,
            ),
            (
                "> (inglés)",
                with_text(Key::Period, Modifiers::SHIFT, ">"),
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

    /// Las acciones que saldrían de `events` tras la puerta `gate_keys`, con la tabla por defecto.
    fn actions_after_the_gate(
        events: &[Event],
        wants_keyboard: bool,
        platform: Platform,
    ) -> Vec<Option<AppAction>> {
        let seen_2 = gate_keys(events, wants_keyboard);
        let km = default();
        presses(&seen_2)
            .iter()
            .map(|p| km.action_for(p, platform))
            .collect()
    }

    #[test]
    fn row_8_with_a_really_focused_field_gate_keys_removes_key_and_text_and_no_action_comes_out() {
        for platform in PLATFORMS {
            for (name_2, events, _) in sequences() {
                let mut with_focus = BenchText::new_2();
                with_focus.frame(vec![], true);
                with_focus.frame(vec![], true);
                assert!(with_focus.ctx.text_edit_focused(), "el campo tiene el foco");
                with_focus.frame(events.clone(), false);
                let focused = with_focus.ctx.text_edit_focused();
                assert!(focused, "{name_2}: sigue enfocado");
                let seen_2 = gate_keys(&events, focused);
                assert!(
                    seen_2.is_empty(),
                    "{name_2} en {platform:?}: gate_keys deja pasar {seen_2:?} (también el Text)"
                );
                assert_eq!(
                    actions_after_the_gate(&events, focused, platform),
                    Vec::<Option<AppAction>>::new(),
                    "{name_2} en {platform:?}"
                );
            }
        }
    }

    #[test]
    fn row_8_without_a_focused_field_the_same_sequences_give_their_action() {
        for platform in PLATFORMS {
            for (name_2, events, expected) in sequences() {
                let mut without_focus = BenchText::new_2();
                without_focus.frame(vec![], false);
                without_focus.frame(vec![], false);
                assert!(!without_focus.ctx.text_edit_focused());
                let seen_2 = gate_keys(&events, without_focus.ctx.text_edit_focused());
                assert_eq!(
                    seen_2.len(),
                    events.len(),
                    "{name_2}: pasan todos los eventos"
                );
                assert_eq!(
                    actions_after_the_gate(&events, false, platform),
                    vec![Some(expected)],
                    "{name_2} en {platform:?}"
                );
            }
        }
    }

    #[test]
    fn row_8_with_a_dialog_open_every_row_of_the_table_gives_blocked_also_home_page_up_and_page_down()
     {
        let km = default();
        let dialog_2 = Locks {
            dialog: true,
            ..Locks::default()
        };
        for row_2 in km.rows() {
            assert!(
                is_blocked(row_2.action, dialog_2, Origin::Key),
                "{:?} ({:?}) con un diálogo de archivo",
                row_2.action,
                row_2.key
            );
            assert!(
                !is_blocked(row_2.action, Locks::default(), Origin::Key),
                "{:?} ({:?}) sin bloqueos",
                row_2.action,
                row_2.key
            );
        }
        for key_2 in [KeyId::Home, KeyId::PageUp, KeyId::PageDown] {
            assert!(
                km.rows().iter().any(|f| f.key == key_2),
                "la tabla tiene una fila con {key_2:?}"
            );
        }
    }

    #[test]
    fn row_8_with_the_modal_panel_open_every_row_gives_blocked_except_quit() {
        let km = default();
        let modal = Locks {
            modal: true,
            ..Locks::default()
        };
        for row_2 in km.rows() {
            // `Salir` pasa con el panel modal (decisión D1 de la unidad 18): sigue igual.
            let expected_2 = row_2.action != AppAction::Quit;
            assert_eq!(
                is_blocked(row_2.action, modal, Origin::Key),
                expected_2,
                "{:?} ({:?}) con el modal",
                row_2.action,
                row_2.key
            );
        }
    }

    /// El panel de licencias abierto como `App` (el `Modal`) y pintado con `show_dialog`.
    struct BenchDialog {
        ctx: Context,
        modal: Modal,
        focus: Option<FocusTarget>,
    }

    impl BenchDialog {
        fn new_2() -> BenchDialog {
            let ctx = context(1.0);
            ctx.set_visuals_of(egui::Theme::Dark, visuals(1.0));
            let text_2 = (1..=200)
                .map(|n| format!("linea {n}"))
                .collect::<Vec<_>>()
                .join("\n");
            let mut modal = Modal::Closed;
            assert!(modal.open(
                DialogKind::Licenses,
                licenses_form(&text_2),
                Locks::default()
            ));
            let mut bench = BenchDialog {
                ctx,
                modal,
                focus: None,
            };
            for _ in 0..3 {
                bench.frame(vec![]);
            }
            bench
        }

        fn frame(&mut self, events: Vec<Event>) {
            let BenchDialog { ctx, modal, focus } = self;
            let input_2 = RawInput {
                screen_rect: Some(Rect::from_min_size(Pos2::ZERO, vec2(960.0, 720.0))),
                events,
                ..RawInput::default()
            };
            let mut output_2 = ctx.run_ui(input_2, |ui| {
                let c = ui.ctx().clone();
                apply_scale(&c);
                if let Modal::Open(dialog) = modal {
                    let _ = show_dialog(&c, dialog, focus);
                }
            });
            output_2.textures_delta.clear();
        }

        fn first_line(&self) -> usize {
            match &self.modal {
                Modal::Open(dialog) => dialog.first_line(LICENSES_FIELD),
                Modal::Closed => panic!("el diálogo se cerró"),
            }
        }

        /// Una pulsación como `App`: la tabla traduce la tecla (y el bloqueo la frena), y el
        /// diálogo la recibe igualmente. Devuelve la acción que la tabla le daba y si se bloquea.
        fn presses_2(&mut self, k: Key) -> (Option<AppAction>, Option<bool>) {
            let events = vec![key(k, Modifiers::NONE)];
            let km = default();
            let action_2 = presses(&events)
                .first()
                .and_then(|p| km.action_for(p, Platform::Linux));
            let locks = Locks {
                modal: true,
                ..Locks::default()
            };
            let locked = action_2.map(|a| is_blocked(a, locks, Origin::Key));
            self.frame(events);
            for _ in 0..2 {
                self.frame(vec![]);
            }
            (action_2, locked)
        }
    }

    #[test]
    fn row_8_home_end_page_up_and_page_down_still_scroll_the_dialog_with_the_modal_open() {
        let mut bench = BenchDialog::new_2();
        assert_eq!(bench.first_line(), 0);

        let (action_2, locked) = bench.presses_2(Key::PageDown);
        assert_eq!(action_2, Some(AppAction::ColorNext), "Av Pág en la tabla");
        assert_eq!(locked, Some(true), "Av Pág bloqueada para la tabla");
        let after_av = bench.first_line();
        assert!(after_av > 0, "Av Pág desplaza el diálogo");

        let (action_2, _) = bench.presses_2(Key::End);
        assert_eq!(action_2, None, "Fin no está en la tabla");
        let maximum_2 = bench.first_line();
        assert!(maximum_2 > after_av, "Fin va al final");

        let (action_2, locked) = bench.presses_2(Key::PageUp);
        assert_eq!(action_2, Some(AppAction::ColorPrev), "Re Pág en la tabla");
        assert_eq!(locked, Some(true));
        assert!(bench.first_line() < maximum_2, "Re Pág desplaza el diálogo");

        let (action_2, locked) = bench.presses_2(Key::Home);
        assert_eq!(action_2, Some(AppAction::ZoomFit), "Inicio en la tabla");
        assert_eq!(locked, Some(true));
        assert_eq!(bench.first_line(), 0, "Inicio vuelve al principio");
    }
}

// ---------------------------------------------------------------------------------------------
// Fila 10 (parte de px-ui): cifras sin tecla y las cuatro funciones antiguas frente a la tabla
// ---------------------------------------------------------------------------------------------

const FIGURES: [Key; 9] = [
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
fn row_10_digits_1_to_9_without_modifiers_do_nothing_through_action_for_shortcut() {
    for platform in PLATFORMS {
        for key in FIGURES {
            assert_eq!(
                action_for_shortcut(key, Modifiers::NONE, platform),
                None,
                "{key:?} en {platform:?}"
            );
        }
    }
}

#[test]
fn row_10_digits_1_to_9_without_modifiers_do_nothing_through_the_table() {
    let km = default();
    for platform in PLATFORMS {
        for key in FIGURES {
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
fn row_10_press_from_key_of_the_table_keys_without_modifiers() {
    let cases = [
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
    for (key, expected) in cases {
        let p = press_from_key(key, Modifiers::NONE);
        assert_eq!(p.key, expected, "{key:?}");
        assert!(
            !p.command && !p.shift && !p.alt && !p.other_ctrl && !p.repeat,
            "{key:?}: {p:?}"
        );
    }
}

#[test]
fn row_10_press_from_key_passes_the_modifiers_to_the_fields_of_the_press() {
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
        for (key, expected) in [(Key::Plus, '+'), (Key::Equals, '='), (Key::Minus, '-')] {
            assert_eq!(
                press_from_key(key, m).key,
                Some(c(expected)),
                "{key:?} con {m:?}"
            );
        }
        // Las teclas con nombre valen con cualquier modificador sin `command`.
        for (key, expected) in [
            (Key::Home, KeyId::Home),
            (Key::PageUp, KeyId::PageUp),
            (Key::PageDown, KeyId::PageDown),
        ] {
            assert_eq!(
                press_from_key(key, m).key,
                Some(expected),
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
fn combinations_real() -> Vec<(u8, Modifiers)> {
    (0..32u8)
        .map(|bits| (bits, modifiers_2(bits)))
        .filter(|(_, m)| !(m.mac_cmd && !m.command))
        .collect()
}

fn is_with_name(key: Key) -> bool {
    matches!(key, Key::Home | Key::PageUp | Key::PageDown)
}

#[test]
fn row_10_action_for_shortcut_gives_the_same_as_the_table_over_key_all_by_32_combinations_and_3_platforms()
 {
    let km = default();
    assert!(Key::ALL.len() > 100);
    for platform in PLATFORMS {
        for &key in Key::ALL {
            for (bits, m) in combinations_real() {
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
fn row_10_tool_action_for_key_gives_the_same_as_the_table_where_its_type_allows() {
    let km = default();
    for platform in PLATFORMS {
        for &key in Key::ALL {
            for (bits, m) in combinations_real() {
                let de_la_tabla = km.action_for(&press_from_key(key, m), platform);
                let expected_2 = match de_la_tabla {
                    Some(AppAction::SelectTool(tool)) => Some(ToolAction::Select(tool)),
                    // `Escape` cancela y no es de la tabla (es del lienzo).
                    _ if key == Key::Escape && m.is_none() => Some(ToolAction::Cancel),
                    _ => None,
                };
                assert_eq!(
                    tool_action_for_key(key, m),
                    expected_2,
                    "{key:?} con {m:?} ({bits:#07b}) en {platform:?}"
                );
            }
        }
    }
}

#[test]
fn row_10_file_action_for_key_gives_the_same_as_the_table_where_its_type_allows() {
    let km = default();
    for platform in PLATFORMS {
        for &key in Key::ALL {
            for (bits, m) in combinations_real() {
                let expected_2 = match km.action_for(&press_from_key(key, m), platform) {
                    Some(AppAction::Open) => Some(FileAction::Open),
                    Some(AppAction::Save) => Some(FileAction::Save),
                    Some(AppAction::SaveAs) => Some(FileAction::SaveAs),
                    _ => None,
                };
                assert_eq!(
                    file_action_for_key(key, m),
                    expected_2,
                    "{key:?} con {m:?} ({bits:#07b}) en {platform:?}"
                );
            }
        }
    }
}

#[test]
fn row_10_action_for_key_gives_the_same_as_the_table_where_its_type_allows() {
    let km = default();
    for platform in PLATFORMS {
        for &key in Key::ALL {
            for (bits, m) in combinations_real() {
                // Véase la cabecera: `Re Pág`/`Av Pág` con `ctrl` suelto.
                if is_with_name(key) && m.ctrl && !m.command {
                    continue;
                }
                let expected_2 = match km.action_for(&press_from_key(key, m), platform) {
                    Some(AppAction::Undo) => Some(Action::Undo),
                    Some(AppAction::Redo) => Some(Action::Redo),
                    Some(AppAction::ColorNext) => Some(Action::ColorNext),
                    Some(AppAction::ColorPrev) => Some(Action::ColorPrev),
                    _ => None,
                };
                assert_eq!(
                    action_for_key(key, m),
                    expected_2,
                    "{key:?} con {m:?} ({bits:#07b}) en {platform:?}"
                );
            }
        }
    }
}

#[test]
fn row_10_action_for_key_never_gives_select_color() {
    for &key in Key::ALL {
        for bits in 0..32u8 {
            assert!(
                !matches!(
                    action_for_key(key, modifiers_2(bits)),
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
fn row_11_with_d_and_x_swapped_x_chooses_the_pencil_and_d_does_not() {
    let km = with_keys_changed('d', 'x');
    for platform in PLATFORMS {
        assert_eq!(
            action_2(&km, &single(c('x')), platform),
            Some(AppAction::SelectTool(Tool::Pencil)),
            "x en {platform:?}"
        );
        assert_eq!(
            action_2(&km, &single(c('d')), platform),
            None,
            "d en {platform:?}"
        );
        // El resto de la tabla sigue igual.
        assert_eq!(
            action_2(&km, &single(c('v')), platform),
            Some(AppAction::SelectTool(Tool::Line))
        );
        assert_eq!(
            action_2(&km, &with_command(c('z'), false, false), platform),
            Some(AppAction::Undo)
        );
    }
    assert_eq!(km.conflicts(), vec![]);
}

#[test]
fn row_11_with_d_and_x_swapped_the_label_is_x() {
    let km = with_keys_changed('d', 'x');
    for platform in PLATFORMS {
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
fn row_11_with_an_empty_table_no_key_works_and_no_shortcut_is_shown() {
    let km = Keymap::new(vec![]);
    assert_eq!(km.rows().len(), 0);
    assert_eq!(km.conflicts(), vec![]);
    let default = default();
    for platform in PLATFORMS {
        // Todas las pulsaciones que dan acción con la tabla por defecto dan `None` con la vacía.
        for row_2 in default.rows() {
            let p = Press {
                key: Some(row_2.key),
                command: row_2.command,
                shift: row_2.shift,
                alt: false,
                other_ctrl: false,
                repeat: false,
            };
            assert_eq!(km.action_for(&p, platform), None, "{p:?} en {platform:?}");
        }
        for action in all_the_actions() {
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
fn row_11_the_painted_menu_reads_the_received_table() {
    for ppp in [1.0, 2.0] {
        // Con la tabla por defecto, `Undo` enseña `Ctrl+Z`.
        let mut bench = BenchUi::new_2(ppp);
        let edit = bench.opens_menu("Editar", &default(), Platform::Linux);
        assert!(edit.text_with("Ctrl+Z").is_some(), "ppp {ppp}");

        // Con otra tabla (`Z` cambiada por `W`), enseña `Ctrl+W`.
        let rows_2: Vec<Binding> = default()
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
        let mut bench = BenchUi::new_2(ppp);
        let edit = bench.opens_menu("Editar", &Keymap::new(rows_2), Platform::Linux);
        assert!(edit.text_with("Ctrl+W").is_some(), "ppp {ppp}");
        assert!(edit.text_with("Ctrl+Z").is_none(), "ppp {ppp}");

        // Con la vacía no hay atajo, pero la entrada sigue ahí y se puede elegir.
        let empty = Keymap::new(vec![]);
        let mut bench = BenchUi::new_2(ppp);
        let edit = bench.opens_menu("Editar", &empty, Platform::Linux);
        assert!(edit.text_with("Deshacer").is_some(), "ppp {ppp}");
        assert!(edit.text_with("Rehacer").is_some(), "ppp {ppp}");
        assert!(
            edit.text_with("Ctrl").is_none(),
            "ppp {ppp}: {:?}",
            edit.all_the_texts()
        );
        let undo = edit
            .text_with("Deshacer")
            .expect("la entrada")
            .rect()
            .center();
        assert_eq!(
            bench.click_on_the_menu(undo, &empty, Platform::Linux),
            Some(AppAction::Undo),
            "ppp {ppp}: el menú sigue funcionando con la tabla vacía"
        );
    }
}

#[test]
fn row_11_the_column_tooltip_reads_the_received_table() {
    let text_2 = |i: usize, km: &Keymap| tooltip_text(&TOOL_BUTTONS[i], km, Platform::Linux);
    for ppp in [1.0, 2.0] {
        // `d` y `x` cambiadas: el tooltip del lápiz dice `x` y ya no `d`.
        let changed = with_keys_changed('d', 'x');
        assert!(text_2(0, &changed).starts_with("Lápiz (x)"), "ppp {ppp}");
        assert!(!text_2(0, &changed).contains("(d)"), "ppp {ppp}");
        assert!(text_2(1, &changed).starts_with("Línea (v)"), "ppp {ppp}");
        assert!(text_2(2, &changed).starts_with("Relleno (f)"), "ppp {ppp}");
        assert!(
            text_2(3, &changed).starts_with("Cuentagotas (,)"),
            "ppp {ppp}"
        );

        // Con la tabla vacía no hay ninguna letra, y los botones siguen funcionando.
        let empty = Keymap::new(vec![]);
        for i in 0..5 {
            let t = text_2(i, &empty);
            assert!(!t.contains('('), "ppp {ppp}: {t:?} lleva atajo sin tabla");
        }
        let mut bench = BenchUi::new_2(ppp);
        let p = bench.column_in_rest(&empty, Platform::Linux);
        for letter in ["d", "v", "f", ",", "x"] {
            assert!(!p.paints(letter), "ppp {ppp}: pinta {letter:?} sin tabla");
        }
        assert_eq!(
            bench.click_on_the_tool(0, &empty, Platform::Linux),
            vec![AppAction::SelectTool(Tool::Pencil)],
            "ppp {ppp}: el botón funciona con la tabla vacía"
        );
    }
}

fn conflicts(rows_2: Vec<Binding>) -> Vec<Conflict> {
    Keymap::new(rows_2).conflicts()
}

#[test]
fn row_11_two_equal_rows_give_duplicate() {
    let a = row_2(c('d'), false, false, AppAction::SelectTool(Tool::Pencil));
    let b = row_2(c('d'), false, false, AppAction::SelectTool(Tool::Line));
    assert_eq!(
        conflicts(vec![a.clone(), b]),
        vec![Conflict::Duplicate(0, 1)]
    );
    // La misma fila repetida también.
    assert_eq!(
        conflicts(vec![a.clone(), a]),
        vec![Conflict::Duplicate(0, 1)]
    );
}

#[test]
fn row_11_duplicate_comes_out_with_the_indices_of_the_rows_that_clash() {
    let rows_2 = vec![
        row_2(c('d'), false, false, AppAction::SelectTool(Tool::Pencil)),
        row_2(c('v'), false, false, AppAction::SelectTool(Tool::Line)),
        row_2(c('v'), false, false, AppAction::SelectTool(Tool::Fill)),
    ];
    assert_eq!(conflicts(rows_2), vec![Conflict::Duplicate(1, 2)]);
}

#[test]
fn row_11_those_that_differ_in_letter_with_shift_in_command_or_in_platform_do_not_clash() {
    // Letras: `shift` cuenta.
    assert_eq!(
        conflicts(vec![
            row_2(c('d'), false, false, AppAction::SelectTool(Tool::Pencil)),
            row_2(c('d'), false, true, AppAction::SelectTool(Tool::Line)),
        ]),
        vec![]
    );
    // `command` cuenta.
    assert_eq!(
        conflicts(vec![
            row_2(c('d'), false, false, AppAction::SelectTool(Tool::Pencil)),
            row_2(c('d'), true, false, AppAction::Open),
        ]),
        vec![]
    );
    // `active` sin solaparse.
    let mut linux = row_2(c('d'), false, false, AppAction::SelectTool(Tool::Pencil));
    linux.active = Platforms::Only(Platform::Linux);
    let mut windows = row_2(c('d'), false, false, AppAction::SelectTool(Tool::Line));
    windows.active = Platforms::Only(Platform::Windows);
    assert_eq!(conflicts(vec![linux.clone(), windows]), vec![]);
    let mut no_linux = row_2(c('d'), false, false, AppAction::SelectTool(Tool::Line));
    no_linux.active = Platforms::Except(Platform::Linux);
    assert_eq!(conflicts(vec![linux.clone(), no_linux]), vec![]);
    // Con `active` solapado sí chocan.
    let all_2 = row_2(c('d'), false, false, AppAction::SelectTool(Tool::Line));
    assert_eq!(
        conflicts(vec![linux, all_2]),
        vec![Conflict::Duplicate(0, 1)]
    );
}

#[test]
fn row_11_in_symbols_and_named_keys_shift_does_not_count_for_the_clash() {
    for key in [c('+'), c('<'), c(','), KeyId::Home] {
        assert_eq!(
            conflicts(vec![
                row_2(key, false, false, AppAction::ZoomIn),
                row_2(key, false, true, AppAction::ZoomOut),
            ]),
            vec![Conflict::Duplicate(0, 1)],
            "{key:?}"
        );
    }
    // I.2a: en `Re Pág` y `Av Pág` `shift` sí cuenta (con él son el color de fondo).
    for key in [KeyId::PageUp, KeyId::PageDown] {
        assert_eq!(
            conflicts(vec![
                row_2(key, false, false, AppAction::ZoomIn),
                row_2(key, false, true, AppAction::ZoomOut),
            ]),
            vec![],
            "{key:?}"
        );
    }
    // Con `command`, `shift` es siempre exacto.
    assert_eq!(
        conflicts(vec![
            row_2(c('+'), true, false, AppAction::ZoomIn),
            row_2(c('+'), true, true, AppAction::ZoomOut),
        ]),
        vec![]
    );
}

#[test]
fn row_11_escape_space_and_ctrl_c_x_v_a_give_reserved() {
    for (name_2, key, command) in [
        ("Escape", c('\u{1b}'), false),
        ("Espacio", c(' '), false),
        ("Ctrl+C", c('c'), true),
        ("Ctrl+X", c('x'), true),
        ("Ctrl+V", c('v'), true),
        ("Ctrl+A", c('a'), true),
    ] {
        assert_eq!(
            conflicts(vec![row_2(
                key,
                command,
                false,
                AppAction::SelectTool(Tool::Pencil)
            )]),
            vec![Conflict::Reserved(0)],
            "{name_2}"
        );
    }
    // El índice es el de la fila reservada.
    assert_eq!(
        conflicts(vec![
            row_2(c('d'), false, false, AppAction::SelectTool(Tool::Pencil)),
            row_2(c('c'), true, false, AppAction::Open),
        ]),
        vec![Conflict::Reserved(1)]
    );
    // Las mismas letras sin `command` no están reservadas.
    for key in ['c', 'x', 'v', 'a'] {
        assert_eq!(
            conflicts(vec![row_2(
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
fn row_11_ctrl_q_and_cmd_q_are_reserved_outside_linux() {
    let with_active = |active: Platforms| {
        let mut f = row_2(c('q'), true, false, AppAction::Quit);
        f.active = active;
        conflicts(vec![f])
    };
    assert_eq!(with_active(Platforms::Only(Platform::Linux)), vec![]);
    for active in [
        Platforms::All,
        Platforms::Only(Platform::Windows),
        Platforms::Only(Platform::Mac),
        Platforms::Except(Platform::Linux),
        Platforms::Except(Platform::Mac),
    ] {
        assert_eq!(
            with_active(active),
            vec![Conflict::Reserved(0)],
            "{active:?}"
        );
    }
}

#[test]
fn row_11_default_scheme_gives_no_conflict() {
    assert_eq!(Keymap::default_scheme().conflicts(), vec![]);
}

// ---------------------------------------------------------------------------------------------
// Propiedades (proptest, 256 casos)
// ---------------------------------------------------------------------------------------------

/// Cualquier `Press`: un carácter ASCII imprimible, `Start`, `Re Pág`, `Av Pág` o ninguna
/// tecla, con los modificadores. `other_ctrl` no se combina con `command` (egui no lo da).
fn press_any() -> impl Strategy<Value = Press> {
    let key_2 = prop_oneof![
        (0x20u8..0x7f).prop_map(|b| Some(KeyId::Char(char::from(b)))),
        Just(Some(KeyId::Home)),
        Just(Some(KeyId::PageUp)),
        Just(Some(KeyId::PageDown)),
        Just(None),
    ];
    (
        key_2,
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

fn platform_any() -> impl Strategy<Value = Platform> {
    prop::sample::select(PLATFORMS.to_vec())
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(256))]

    /// Propiedad 1: para cualquier `Press`, el resultado es el mismo con las filas en orden
    /// inverso (la tabla por defecto no tiene ambigüedad).
    #[test]
    fn property_1_the_result_does_not_depend_on_the_order_of_the_rows(
        p in press_any(),
        platform in platform_any(),
    ) {
        let km = default();
        let inverse = Keymap::new(km.rows().iter().rev().cloned().collect());
        prop_assert_eq!(
            km.action_for(&p, platform),
            inverse.action_for(&p, platform),
            "{:?} en {:?}", p, platform
        );
    }
}

#[test]
fn property_1_without_clashes_and_with_command_only_the_nine_modern_ones_on_each_platform() {
    for platform in PLATFORMS {
        let km = default();
        assert_eq!(km.conflicts(), vec![], "{platform:?}");
        // Las filas activas con `command`: ocho en Windows y macOS, nueve en Linux (`Quit`).
        let with_command: Vec<&Binding> = km
            .rows()
            .iter()
            .filter(|f| f.command && vale_en(f.active, platform))
            .collect();
        let expected_2 = if platform == Platform::Linux { 9 } else { 8 };
        assert_eq!(with_command.len(), expected_2, "{platform:?}");
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
    fn property_2_the_press_built_from_the_chord_gives_that_action(
        platform in platform_any(),
        index_2 in 0usize..all_the_actions().len(),
        repeat in any::<bool>(),
    ) {
        let km = default();
        let action = all_the_actions()[index_2];
        if let Some(chord) = km.shown(action, platform) {
            let text_2 = km.shown_text(action, platform);
            let text_2 = text_2.expect("si hay chord hay texto");
            prop_assert!(!text_2.is_empty(), "{:?} en {:?}", action, platform);
            for ch in text_2.chars() {
                prop_assert!(
                    INTERFACE_CHARSET.contains(ch),
                    "{:?} en {:?}: {:?} de {:?} no está en INTERFACE_CHARSET",
                    action, platform, ch, text_2
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
    fn property_2_the_press_of_each_row_gives_the_action_of_the_row_if_it_is_active(
        platform in platform_any(),
        index_2 in 0usize..25,
        repeat in any::<bool>(),
    ) {
        let km = default();
        let row_2 = &km.rows()[index_2];
        let p = Press {
            key: Some(row_2.key),
            command: row_2.command,
            shift: row_2.shift,
            alt: false,
            other_ctrl: false,
            repeat,
        };
        let expected_2 = vale_en(row_2.active, platform).then_some(row_2.action);
        prop_assert_eq!(
            km.action_for(&p, platform),
            expected_2,
            "fila {} ({:?}) en {:?}", index_2, row_2.key, platform
        );
    }
}

#[test]
fn property_2_exhaustive_each_active_row_is_reachable_and_the_inactive_one_does_not_act() {
    let km = default();
    for platform in PLATFORMS {
        for (i, row_2) in km.rows().iter().enumerate() {
            let p = Press {
                key: Some(row_2.key),
                command: row_2.command,
                shift: row_2.shift,
                alt: false,
                other_ctrl: false,
                repeat: false,
            };
            let obtained = km.action_for(&p, platform);
            if vale_en(row_2.active, platform) {
                assert_eq!(obtained, Some(row_2.action), "fila {i} en {platform:?}");
            } else {
                assert_eq!(
                    obtained, None,
                    "fila inactiva {i} ({:?}) en {platform:?}",
                    row_2.action
                );
            }
        }
    }
}

// --- Propiedad 3: independiente del orden, con un oráculo de búsqueda lineal ---

/// El oráculo distingue tres clases de tecla, como la decisión 1.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Class {
    Letter,
    Figure,
    Other,
}

fn class_of(key: KeyId) -> Class {
    match key {
        KeyId::Char(ch) if ch.is_alphabetic() => Class::Letter,
        KeyId::Char(ch) if ch.is_ascii_digit() => Class::Figure,
        _ => Class::Other,
    }
}

/// Si la fila casa con la `Press` en `platform`, con las reglas de la decisión 1 escritas a mano:
/// `command` exacto; con `command`, `shift` exacto y `Alt` apagado; sin él, en letras `shift`
/// exacto y `Alt` y `other_ctrl` apagados, en cifras `Alt` y `other_ctrl` apagados, y en
/// símbolos y teclas con nombre nada más (salvo `Re Pág` y `Av Pág`, con `shift` exacto).
fn matches_2(row_2: &Binding, p: &Press, platform: Platform) -> bool {
    if !vale_en(row_2.active, platform) || p.key != Some(row_2.key) || p.command != row_2.command {
        return false;
    }
    if row_2.command {
        return p.shift == row_2.shift && !p.alt;
    }
    match class_of(row_2.key) {
        Class::Letter => p.shift == row_2.shift && !p.alt && !p.other_ctrl,
        Class::Figure => !p.alt && !p.other_ctrl,
        // I.2a: en `Re Pág` y `Av Pág` `shift` es exacto.
        Class::Other if matches!(row_2.key, KeyId::PageUp | KeyId::PageDown) => {
            p.shift == row_2.shift
        }
        Class::Other => true,
    }
}

/// La primera fila que casa gana.
fn oracle(rows_2: &[Binding], p: &Press, platform: Platform) -> Option<AppAction> {
    rows_2
        .iter()
        .find(|f| matches_2(f, p, platform))
        .map(|f| f.action)
}

const KEYS: [KeyId; 8] = [
    KeyId::Char('a'),
    KeyId::Char('b'),
    KeyId::Char('1'),
    KeyId::Char(','),
    KeyId::Char('<'),
    KeyId::Char('+'),
    KeyId::Home,
    KeyId::PageUp,
];
const ACTIONS_SMALL: [AppAction; 8] = [
    AppAction::Undo,
    AppAction::Redo,
    AppAction::ZoomIn,
    AppAction::ZoomOut,
    AppAction::SelectTool(Tool::Pencil),
    AppAction::SelectTool(Tool::Line),
    AppAction::ColorNext,
    AppAction::ColorPrev,
];
const ACTIVE: [Platforms; 6] = [
    Platforms::All,
    Platforms::Only(Platform::Linux),
    Platforms::Only(Platform::Windows),
    Platforms::Only(Platform::Mac),
    Platforms::Except(Platform::Linux),
    Platforms::Except(Platform::Mac),
];

fn row_any() -> impl Strategy<Value = Binding> {
    (
        prop::sample::select(KEYS.to_vec()),
        any::<bool>(),
        any::<bool>(),
        prop::sample::select(ACTIONS_SMALL.to_vec()),
        prop::sample::select(ACTIVE.to_vec()),
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

fn press_of_a_small_table() -> impl Strategy<Value = Press> {
    let key_2 = prop_oneof![
        prop::sample::select(KEYS.to_vec()).prop_map(Some),
        Just(Some(KeyId::Char('x'))),
        Just(None),
    ];
    (
        key_2,
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

fn table_and_permutation() -> impl Strategy<Value = (Vec<Binding>, Vec<Binding>)> {
    prop::collection::vec(row_any(), 0..=12).prop_flat_map(|rows_2| {
        let copy = rows_2.clone();
        (Just(rows_2), Just(copy).prop_shuffle())
    })
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(256))]

    /// Propiedad 3: con `conflicts()` vacío, `action_for` da lo mismo con cualquier permutación
    /// de las filas y coincide con el oráculo en las tres plataformas; si no, gana la primera.
    #[test]
    fn property_3_independent_of_the_order_and_equal_to_the_oracle(
        (rows_2, permuted) in table_and_permutation(),
        p in press_of_a_small_table(),
    ) {
        let km = Keymap::new(rows_2.clone());
        let mixed = Keymap::new(permuted.clone());
        for platform in PLATFORMS {
            // Gane la primera fila que casa: vale con o sin conflictos.
            prop_assert_eq!(
                km.action_for(&p, platform),
                oracle(&rows_2, &p, platform),
                "{:?} en {:?} con {:?}", p, platform, rows_2
            );
            if km.conflicts().is_empty() {
                prop_assert_eq!(
                    mixed.action_for(&p, platform),
                    km.action_for(&p, platform),
                    "permutación distinta: {:?} en {:?}", p, platform
                );
                prop_assert_eq!(
                    oracle(&permuted, &p, platform),
                    oracle(&rows_2, &p, platform),
                    "el oráculo depende del orden sin conflictos: {:?} con {:?}", p, rows_2
                );
            }
        }
    }
}
