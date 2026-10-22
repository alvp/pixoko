//! Tests de la capa de acciones `px_ui::actions` en su parte pura (specs/hito-2-menu-herramientas.md,
//! «Casos» filas 3, 3b, 5 y 5b, y las reglas de «Piezas»). Sin egui más allá de `Key` y `Modifiers`:
//! no hay ventana ni banco.
//!
//! - Fila 3: `action_for_shortcut` sobre `Key::ALL` por las 32 combinaciones de modificadores y las
//!   tres plataformas, contra un oráculo escrito aparte; ida y vuelta con la tabla del menú; las
//!   tres funciones de teclas anteriores se componen sin cambios.
//! - Fila 3b: las teclas `1` a `9` sin modificadores no hacen nada (unidad I.1) y `Ctrl`/`Cmd`+`1`
//!   es solo `ZoomActual`.
//! - Fila 5: `is_blocked` en el orden de la spec (con la quita pendiente también frena abrir un
//!   diálogo: unidad 18, D2), con un modelo independiente que se compara con la
//!   implementación sobre todas las acciones, orígenes y combinaciones de bloqueos; `enabled`.
//! - Fila 5b (parte pura): los bloqueos del zoom, uno por bloqueo, con los tres orígenes.
//!
//! Decisiones donde la spec no fija un detalle (anotadas aquí y en el informe):
//! - `Ctrl+Mayús+Q` en Linux da `None`: `Salir` se trata como `Open` (`command` sin `Mayús` ni
//!   `Alt`); la spec solo dice «`command`+`Q` solo en Linux».
//! - `Quit` con `modal` y otro bloqueo (por ejemplo `gesture`): la regla (2) deja pasar a `Quit` y
//!   las siguientes reglas se siguen aplicando en orden, así que el otro bloqueo lo frena.

use egui::{Key, Modifiers};
use px_ui::actions::{
    AppAction, Locks, Origin, Platform, UiState, action_for_shortcut, enabled, is_blocked,
};
use px_ui::keymap::{KeyId, Keymap, Press};
use px_ui::menu::{MENUS, Shortcut, shortcut_for};
use px_ui::shortcuts::{Action, FileAction, action_for_key, file_action_for_key};
use px_ui::tool::{Tool, ToolAction, tool_action_for_key};
use px_ui::view::Zoom;

mod common;
use common::{modifiers_2, modifiers_command};

const TEST_TOOLS: [Tool; 5] = [
    Tool::Pencil,
    Tool::Line,
    Tool::Fill,
    Tool::Dropper,
    Tool::Eraser,
];
const PLATFORMS: [Platform; 3] = [Platform::Linux, Platform::Windows, Platform::Mac];
const SOURCES: [Origin; 3] = [Origin::Menu, Origin::Button, Origin::Key];
const ZOOMS: [AppAction; 4] = [
    AppAction::ZoomIn,
    AppAction::ZoomOut,
    AppAction::ZoomFit,
    AppAction::ZoomActual,
];

/// Todas las acciones posibles: las 11 de la tabla del menú, `CloseLicenses`, las cinco
/// herramientas y las nueve ranuras de color.
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
        AppAction::ZoomActual,
        AppAction::ShowLicenses,
        AppAction::CloseLicenses,
    ];
    v.extend(TEST_TOOLS.map(AppAction::SelectTool));
    v.extend((1..=9).map(AppAction::SelectColor));
    v
}

fn es_zoom(action: AppAction) -> bool {
    matches!(
        action,
        AppAction::ZoomIn | AppAction::ZoomOut | AppAction::ZoomFit | AppAction::ZoomActual
    )
}

/// Las 32 combinaciones de los cinco bloqueos.
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

fn only(f: impl FnOnce(&mut Locks)) -> Locks {
    let mut locks = Locks::default();
    f(&mut locks);
    locks
}

/// Estado con historia, zoom ×5 y todo lo demás en reposo: nada deshabilita por sí mismo.
fn state_2(zoom: Option<Zoom>, locks: Locks) -> UiState {
    UiState {
        can_undo: true,
        can_redo: true,
        zoom,
        tool: Tool::Pencil,
        locks,
    }
}

fn zoom_x5() -> Option<Zoom> {
    Zoom::from_int(5)
}

// ---------------------------------------------------------------------------------------------
// Tipos y funciones sencillas
// ---------------------------------------------------------------------------------------------

fn assert_plane<T: Clone + Copy + std::fmt::Debug + PartialEq + Eq>() {}
fn assert_with_default<T: Default>() {}

#[test]
fn the_types_of_the_action_layer_are_plain() {
    assert_plane::<AppAction>();
    assert_plane::<Origin>();
    assert_plane::<Platform>();
    assert_plane::<Locks>();
    assert_with_default::<Locks>();
}

#[test]
fn locks_by_default_is_all_false() {
    let locks = Locks::default();
    assert!(!locks.gesture);
    assert!(!locks.dialog);
    assert!(!locks.text_focus);
    assert!(!locks.removal);
    assert!(!locks.modal);
}

#[test]
fn ignores_repeat_is_only_open_save_saveas_and_quit() {
    for action in all_the_actions() {
        let expected_2 = matches!(
            action,
            AppAction::Open | AppAction::Save | AppAction::SaveAs | AppAction::Quit
        );
        assert_eq!(action.ignores_repeat(), expected_2, "{action:?}");
    }
}

#[test]
fn is_zoom_is_only_the_four_zoom_ones() {
    for action in all_the_actions() {
        assert_eq!(action.is_zoom(), es_zoom(action), "{action:?}");
    }
}

#[test]
fn platform_current_follows_the_build_system() {
    #[cfg(target_os = "linux")]
    assert_eq!(Platform::current(), Platform::Linux);
    #[cfg(target_os = "windows")]
    assert_eq!(Platform::current(), Platform::Windows);
    #[cfg(target_os = "macos")]
    assert_eq!(Platform::current(), Platform::Mac);
}

// ---------------------------------------------------------------------------------------------
// Fila 5: is_blocked en el orden de la spec
// ---------------------------------------------------------------------------------------------

/// Modelo de `is_blocked`, escrito aparte de la implementación y en el orden de la spec:
/// (1) `CloseLicenses` nunca; (2) `modal`: todo salvo `CloseLicenses` y `Quit`; (3) `dialog`:
/// todo; (4) zoom: con `text_focus` solo si el origen es `Key`; (5) el resto: `gesture` o
/// `text_focus`, y `removal` solo `SelectColor` y las acciones que abren un diálogo (hoy, solo
/// `ShowLicenses`; unidad 18, decisión D2); (6) nada.
fn locked_according_to_the_model(action: AppAction, locks: Locks, origin: Origin) -> bool {
    if action == AppAction::CloseLicenses {
        return false;
    }
    if locks.modal && action != AppAction::Quit {
        return true;
    }
    if locks.dialog {
        return true;
    }
    if es_zoom(action) {
        return locks.text_focus && origin == Origin::Key;
    }
    if locks.gesture || locks.text_focus {
        return true;
    }
    locks.removal && matches!(action, AppAction::SelectColor(_) | AppAction::ShowLicenses)
}

#[test]
fn row_5_is_blocked_matches_the_model_in_all_the_combinations() {
    for action in all_the_actions() {
        for bits in 0..32 {
            let locks = locks_2(bits);
            for origin in SOURCES {
                assert_eq!(
                    is_blocked(action, locks, origin),
                    locked_according_to_the_model(action, locks, origin),
                    "{action:?} con {locks:?} y {origin:?}"
                );
            }
        }
    }
}

#[test]
fn row_5_without_locks_nothing_is_blocked() {
    for action in all_the_actions() {
        for origin in SOURCES {
            assert!(
                !is_blocked(action, Locks::default(), origin),
                "{action:?} con {origin:?}"
            );
        }
    }
}

#[test]
fn row_5_rule_1_close_licenses_is_never_blocked() {
    let all = Locks {
        gesture: true,
        stroke: false,
        dialog: true,
        text_focus: true,
        removal: true,
        modal: true,
    };
    for origin in SOURCES {
        assert!(!is_blocked(AppAction::CloseLicenses, all, origin));
        for bits in 0..32 {
            assert!(!is_blocked(AppAction::CloseLicenses, locks_2(bits), origin));
        }
    }
}

#[test]
fn row_5_rule_2_with_a_modal_only_close_licenses_and_quit_pass() {
    let locks = only(|l| l.modal = true);
    for action in all_the_actions() {
        for origin in SOURCES {
            let passes = matches!(action, AppAction::CloseLicenses | AppAction::Quit);
            assert_eq!(
                is_blocked(action, locks, origin),
                !passes,
                "{action:?} con modal y {origin:?}"
            );
        }
    }
}

#[test]
fn row_5_rule_3_with_a_dialog_everything_is_blocked_also_the_zoom() {
    let locks = only(|l| l.dialog = true);
    for action in all_the_actions() {
        for origin in SOURCES {
            let expected_2 = action != AppAction::CloseLicenses;
            assert_eq!(
                is_blocked(action, locks, origin),
                expected_2,
                "{action:?} con dialog y {origin:?}"
            );
        }
    }
}

#[test]
fn row_5_rule_4_the_zoom_ignores_gesture_and_removal_and_only_the_keys_stop_with_text() {
    for action in ZOOMS {
        for origin in SOURCES {
            for locks in [
                only(|l| l.gesture = true),
                only(|l| l.removal = true),
                Locks {
                    gesture: true,
                    removal: true,
                    ..Locks::default()
                },
            ] {
                assert!(
                    !is_blocked(action, locks, origin),
                    "{action:?} con {locks:?} y {origin:?}"
                );
            }
            let text_2 = only(|l| l.text_focus = true);
            assert_eq!(
                is_blocked(action, text_2, origin),
                origin == Origin::Key,
                "{action:?} con text_focus y {origin:?}"
            );
            // Gesto y texto a la vez: manda el texto, que solo para a las teclas.
            let both = Locks {
                gesture: true,
                text_focus: true,
                ..Locks::default()
            };
            assert_eq!(
                is_blocked(action, both, origin),
                origin == Origin::Key,
                "{action:?} con gesture y text_focus y {origin:?}"
            );
        }
    }
}

#[test]
fn row_5_rule_5_the_rest_stops_with_a_gesture_or_text_and_the_removal_only_stops_the_slots_and_the_dialogs()
 {
    for action in all_the_actions() {
        if es_zoom(action) || action == AppAction::CloseLicenses {
            continue;
        }
        for origin in SOURCES {
            assert!(
                is_blocked(action, only(|l| l.gesture = true), origin),
                "{action:?} con gesture y {origin:?}"
            );
            assert!(
                is_blocked(action, only(|l| l.text_focus = true), origin),
                "{action:?} con text_focus y {origin:?}"
            );
            assert_eq!(
                is_blocked(action, only(|l| l.removal = true), origin),
                matches!(action, AppAction::SelectColor(_) | AppAction::ShowLicenses),
                "{action:?} con removal y {origin:?}"
            );
        }
    }
}

#[test]
fn row_5_quit_and_show_licenses_are_also_stopped_by_a_gesture_and_only_show_licenses_by_a_removal()
{
    for action in [AppAction::Quit, AppAction::ShowLicenses] {
        for origin in SOURCES {
            assert!(is_blocked(action, only(|l| l.gesture = true), origin));
            assert!(is_blocked(action, only(|l| l.dialog = true), origin));
            assert!(is_blocked(action, only(|l| l.text_focus = true), origin));
            // Una quita pendiente no frena `Quit`; sí frena abrir un diálogo (D2).
            assert_eq!(
                is_blocked(action, only(|l| l.removal = true), origin),
                action == AppAction::ShowLicenses
            );
        }
    }
}

#[test]
fn row_5_quit_with_a_modal_passes_but_a_gesture_at_the_same_time_stops_it() {
    let modal = only(|l| l.modal = true);
    let modal_and_gesture = Locks {
        modal: true,
        gesture: true,
        ..Locks::default()
    };
    for origin in SOURCES {
        assert!(!is_blocked(AppAction::Quit, modal, origin));
        assert!(is_blocked(AppAction::Quit, modal_and_gesture, origin));
    }
}

#[test]
fn row_5_enabled_is_false_exactly_where_is_blocked_with_menu_source() {
    // Con historia y zoom ×5, `enabled` solo depende del bloqueo en todas las acciones.
    for action in all_the_actions() {
        for bits in 0..32 {
            let locks = locks_2(bits);
            assert_eq!(
                enabled(action, &state_2(zoom_x5(), locks)),
                !is_blocked(action, locks, Origin::Menu),
                "{action:?} con {locks:?}"
            );
        }
    }
}

// ---------------------------------------------------------------------------------------------
// Fila 5b: bloqueos del zoom, uno por bloqueo
// ---------------------------------------------------------------------------------------------

#[test]
fn row_5b_a_b_and_c_gesture_slider_and_removal_do_not_block_the_zoom_with_any_source() {
    // (a) trazo del lápiz, (b) deslizador del editor y (c) quita pendiente: `gesture` es la
    // reunión de (a) y (b) y `removal` es (c).
    for locks in [
        only(|l| l.gesture = true),
        only(|l| l.removal = true),
        Locks {
            gesture: true,
            removal: true,
            ..Locks::default()
        },
    ] {
        for action in ZOOMS {
            for origin in SOURCES {
                assert!(!is_blocked(action, locks, origin), "{action:?} {origin:?}");
            }
            assert!(
                enabled(action, &state_2(zoom_x5(), locks)),
                "{action:?} con {locks:?}"
            );
        }
    }
}

#[test]
fn row_5b_d_with_a_focused_field_only_the_keys_are_blocked() {
    let locks = only(|l| l.text_focus = true);
    for action in ZOOMS {
        assert!(is_blocked(action, locks, Origin::Key), "{action:?}");
        assert!(!is_blocked(action, locks, Origin::Menu), "{action:?}");
        assert!(!is_blocked(action, locks, Origin::Button), "{action:?}");
        assert!(enabled(action, &state_2(zoom_x5(), locks)), "{action:?}");
    }
}

#[test]
fn row_5b_e_an_open_dialog_blocks_the_zoom_with_the_three_sources() {
    let locks = only(|l| l.dialog = true);
    for action in ZOOMS {
        for origin in SOURCES {
            assert!(is_blocked(action, locks, origin), "{action:?} {origin:?}");
        }
        assert!(!enabled(action, &state_2(zoom_x5(), locks)), "{action:?}");
    }
}

#[test]
fn row_5b_f_the_licenses_panel_blocks_the_zoom_and_only_lets_close_and_quit_through() {
    let locks = only(|l| l.modal = true);
    for action in ZOOMS {
        for origin in SOURCES {
            assert!(is_blocked(action, locks, origin), "{action:?} {origin:?}");
        }
        assert!(!enabled(action, &state_2(zoom_x5(), locks)), "{action:?}");
    }
    for origin in SOURCES {
        assert!(!is_blocked(AppAction::CloseLicenses, locks, origin));
        assert!(!is_blocked(AppAction::Quit, locks, origin));
    }
}

// ---------------------------------------------------------------------------------------------
// Fila 3: action_for_shortcut
// ---------------------------------------------------------------------------------------------

/// Oráculo de `action_for_shortcut`, escrito aparte de la implementación.
///
/// Con `command` (y sin `Alt`): `Z` deshacer, `Mayús`+`Z` e `Y` rehacer, `S` guardar, `Mayús`+`S`
/// guardar como, `O` abrir, `N` nueva imagen (sin `Mayús`; unidad 19), `1` 1:1 (sin `Mayús`) y `Q`
/// salir solo en Linux (sin `Mayús`). Sin
/// `command`: `+`, `=`, `-`, `Home`, `Re Pág` y `Av Pág` con cualquier otro modificador, y con
/// **ningún** modificador `d v f ,`, `u`, `n`, `[` y `]` (las cifras ya no hacen nada).
fn shortcut_expected(key: Key, m: Modifiers, platform: Platform) -> Option<AppAction> {
    if m.command {
        if m.alt {
            return None;
        }
        return match (key, m.shift) {
            (Key::Z, false) => Some(AppAction::Undo),
            (Key::Z, true) | (Key::Y, false) => Some(AppAction::Redo),
            (Key::S, false) => Some(AppAction::Save),
            (Key::S, true) => Some(AppAction::SaveAs),
            (Key::O, false) => Some(AppAction::Open),
            (Key::N, false) => Some(AppAction::NewImage),
            (Key::Num1, false) => Some(AppAction::ZoomActual),
            (Key::Q, false) if platform == Platform::Linux => Some(AppAction::Quit),
            _ => None,
        };
    }
    match key {
        Key::Plus | Key::Equals => return Some(AppAction::ZoomIn),
        Key::Minus => return Some(AppAction::ZoomOut),
        Key::Home => return Some(AppAction::ZoomFit),
        // I.2a: con `Mayús`, `Re Pág` y `Av Pág` son el color de fondo; sin ella, el primer plano.
        Key::PageDown if m.shift => return Some(AppAction::BackgroundNext),
        Key::PageUp if m.shift => return Some(AppAction::BackgroundPrev),
        Key::PageDown => return Some(AppAction::ColorNext),
        Key::PageUp => return Some(AppAction::ColorPrev),
        // I.2a: `K` (con `Mayús` y nada más) borra la página.
        Key::K if m.shift && !m.alt && !m.ctrl && !m.command && !m.mac_cmd => {
            return Some(AppAction::ClearPage);
        }
        _ => {}
    }
    if !m.is_none() {
        return None;
    }
    match key {
        Key::D => Some(AppAction::SelectTool(Tool::Pencil)),
        Key::V => Some(AppAction::SelectTool(Tool::Line)),
        Key::F => Some(AppAction::SelectTool(Tool::Fill)),
        Key::Comma => Some(AppAction::SelectTool(Tool::Dropper)),
        Key::K => Some(AppAction::SelectTool(Tool::Eraser)),
        Key::U => Some(AppAction::Undo),
        Key::N => Some(AppAction::CenterOnCursor),
        Key::CloseBracket => Some(AppAction::ColorNext),
        Key::OpenBracket => Some(AppAction::ColorPrev),
        Key::CloseCurlyBracket => Some(AppAction::BackgroundNext),
        Key::OpenCurlyBracket => Some(AppAction::BackgroundPrev),
        Key::Quote => Some(AppAction::ToggleLayerPanel),
        Key::Semicolon => Some(AppAction::NewLayer),
        _ => None,
    }
}

#[test]
fn row_3_action_for_shortcut_over_key_all_by_32_combinations_and_3_platforms() {
    assert!(!Key::ALL.is_empty());
    for platform in PLATFORMS {
        for &key in Key::ALL {
            for bits in 0..32 {
                let m = modifiers_2(bits);
                assert_eq!(
                    action_for_shortcut(key, m, platform),
                    shortcut_expected(key, m, platform),
                    "{key:?} con {m:?} en {platform:?}"
                );
            }
        }
    }
}

#[test]
fn row_3_escape_gives_none_with_any_combination() {
    // `Escape` no es una acción: lo ordena `Canvas::interact_ordered` con los eventos de trazo.
    for platform in PLATFORMS {
        for bits in 0..32 {
            assert_eq!(
                action_for_shortcut(Key::Escape, modifiers_2(bits), platform),
                None,
                "{platform:?} con {bits:#07b}"
            );
        }
    }
}

#[test]
fn row_3_command_q_only_quits_on_linux() {
    let linux = modifiers_command(false);
    assert_eq!(
        action_for_shortcut(Key::Q, linux, Platform::Linux),
        Some(AppAction::Quit)
    );
    assert_eq!(
        action_for_shortcut(Key::Q, Modifiers::COMMAND, Platform::Linux),
        Some(AppAction::Quit)
    );
    for platform in [Platform::Windows, Platform::Mac] {
        for bits in 0..32 {
            assert_eq!(
                action_for_shortcut(Key::Q, modifiers_2(bits), platform),
                None,
                "{platform:?} con {bits:#07b}"
            );
        }
    }
    // Sin `command`, la `Q` sola no hace nada en ninguna plataforma.
    for platform in PLATFORMS {
        assert_eq!(action_for_shortcut(Key::Q, Modifiers::NONE, platform), None);
    }
}

#[test]
fn row_3_ctrl_shift_q_and_alt_q_do_not_quit_on_linux() {
    // Decisión: `Salir` se trata como `Open` (`command` sin `Mayús` ni `Alt`).
    let mut with_shift = modifiers_command(false);
    with_shift.shift = true;
    let mut with_alt = modifiers_command(false);
    with_alt.alt = true;
    for m in [with_shift, with_alt] {
        assert_eq!(
            action_for_shortcut(Key::Q, m, Platform::Linux),
            None,
            "{m:?}"
        );
    }
}

#[test]
fn row_3_the_zoom_keys_work_without_command_and_with_any_other_modifier() {
    for platform in PLATFORMS {
        for (key, action_2) in [
            (Key::Plus, AppAction::ZoomIn),
            (Key::Equals, AppAction::ZoomIn),
            (Key::Minus, AppAction::ZoomOut),
            (Key::Home, AppAction::ZoomFit),
        ] {
            for bits in 0..32 {
                let m = modifiers_2(bits);
                let expected_2 = (!m.command).then_some(action_2);
                assert_eq!(
                    action_for_shortcut(key, m, platform),
                    expected_2,
                    "{key:?} con {m:?} en {platform:?}"
                );
            }
            // `+` se teclea con Mayús en muchas distribuciones: sigue siendo zoom.
            assert_eq!(
                action_for_shortcut(key, Modifiers::SHIFT, platform),
                Some(action_2)
            );
        }
    }
}

#[test]
fn row_3_each_tool_and_color_key_works_without_modifiers_on_the_three_platforms() {
    for platform in PLATFORMS {
        for (key, action_2) in [
            (Key::D, AppAction::SelectTool(Tool::Pencil)),
            (Key::V, AppAction::SelectTool(Tool::Line)),
            (Key::F, AppAction::SelectTool(Tool::Fill)),
            (Key::Comma, AppAction::SelectTool(Tool::Dropper)),
            // La parte de ranuras: `]` y `[` recorren los colores.
            (Key::CloseBracket, AppAction::ColorNext),
            (Key::OpenBracket, AppAction::ColorPrev),
        ] {
            assert_eq!(
                action_for_shortcut(key, Modifiers::NONE, platform),
                Some(action_2)
            );
            // Con cualquier modificador (también `command`) no hacen nada.
            for bits in 1..32 {
                let m = modifiers_2(bits);
                assert_eq!(
                    action_for_shortcut(key, m, platform),
                    None,
                    "{key:?} con {m:?} en {platform:?}"
                );
            }
        }
    }
}

#[test]
fn row_3_composes_the_three_existing_key_functions_without_changing_them() {
    for platform in PLATFORMS {
        for &key in Key::ALL {
            for bits in 0..32 {
                let m = modifiers_2(bits);
                let shortcut_2 = action_for_shortcut(key, m, platform);
                if let Some(file) = file_action_for_key(key, m) {
                    let expected_2 = match file {
                        FileAction::Open => AppAction::Open,
                        FileAction::Save => AppAction::Save,
                        FileAction::SaveAs => AppAction::SaveAs,
                    };
                    assert_eq!(shortcut_2, Some(expected_2), "{key:?} con {m:?}");
                }
                match tool_action_for_key(key, m) {
                    Some(ToolAction::Select(tool)) => {
                        assert_eq!(
                            shortcut_2,
                            Some(AppAction::SelectTool(tool)),
                            "{key:?} con {m:?}"
                        );
                    }
                    Some(ToolAction::Cancel) => {
                        assert_eq!(shortcut_2, None, "Escape no es una acción: {m:?}");
                    }
                    None => {}
                }
                match action_for_key(key, m) {
                    Some(Action::Undo) => assert_eq!(shortcut_2, Some(AppAction::Undo)),
                    Some(Action::Redo) => assert_eq!(shortcut_2, Some(AppAction::Redo)),
                    Some(Action::SelectColor(n)) => {
                        assert_eq!(
                            shortcut_2,
                            Some(AppAction::SelectColor(n)),
                            "{key:?} con {m:?}"
                        );
                    }
                    Some(Action::ColorNext) => {
                        assert_eq!(shortcut_2, Some(AppAction::ColorNext), "{key:?} con {m:?}");
                    }
                    Some(Action::ColorPrev) => {
                        assert_eq!(shortcut_2, Some(AppAction::ColorPrev), "{key:?} con {m:?}");
                    }
                    None => {}
                }
            }
        }
    }
}

#[test]
fn row_3_the_three_previous_functions_still_give_none_for_command_q_and_command_1() {
    // Sin choque: las funciones existentes no conocen las teclas nuevas.
    for m in [
        Modifiers::COMMAND,
        modifiers_command(false),
        modifiers_command(true),
    ] {
        for key in [Key::Q, Key::Num1] {
            assert_eq!(file_action_for_key(key, m), None, "{key:?} con {m:?}");
            assert_eq!(action_for_key(key, m), None, "{key:?} con {m:?}");
            assert_eq!(tool_action_for_key(key, m), None, "{key:?} con {m:?}");
        }
    }
}

/// La pulsación que hace un `Shortcut` (la tecla que muestra, con `command` y `Mayús` si los lleva).
fn pressed_2(shortcut: Shortcut) -> Press {
    Press {
        key: Some(shortcut.key),
        command: shortcut.command,
        shift: shortcut.shift,
        alt: false,
        other_ctrl: false,
        repeat: false,
    }
}

#[test]
fn row_3_each_shortcut_shown_gives_the_action_of_its_entry_round_trip() {
    let mut with_shortcut = 0;
    for menu in MENUS.iter() {
        for item in menu.items {
            for platform in PLATFORMS {
                let Some(shortcut) = shortcut_for(item.action, platform) else {
                    continue;
                };
                with_shortcut += 1;
                assert_eq!(
                    Keymap::default_scheme().action_for(&pressed_2(shortcut), platform),
                    Some(item.action),
                    "{} ({:?}) en {platform:?}",
                    item.label,
                    shortcut
                );
            }
        }
    }
    // 12 entradas con atajo en Linux y 11 en Windows y Mac (sin `Licencias` ni, fuera de Linux,
    // `Salir`; con `Nueva image_2…` de la unidad 19 y `Borrar página` de I.2a): 16 + 15 + 15 = 46.
    assert_eq!(with_shortcut, 16 + 15 + 15);
}

// ---------------------------------------------------------------------------------------------
// Fila 3b: las teclas 1 a 9 ya no eligen color (unidad I.1)
// ---------------------------------------------------------------------------------------------

const DIGITS: [(Key, u8); 9] = [
    (Key::Num1, 1),
    (Key::Num2, 2),
    (Key::Num3, 3),
    (Key::Num4, 4),
    (Key::Num5, 5),
    (Key::Num6, 6),
    (Key::Num7, 7),
    (Key::Num8, 8),
    (Key::Num9, 9),
];

#[test]
fn row_3b_digits_without_modifiers_do_nothing_on_the_three_platforms() {
    for platform in PLATFORMS {
        for (key, slot) in DIGITS {
            assert_eq!(
                action_for_shortcut(key, Modifiers::NONE, platform),
                None,
                "{key:?} en {platform:?} (antes elegía la ranura {slot})"
            );
        }
    }
}

#[test]
fn row_3b_command_and_1_gives_only_zoom_actual_and_never_select_color() {
    for platform in PLATFORMS {
        for m in [
            Modifiers::COMMAND,
            modifiers_command(false),
            modifiers_command(true),
        ] {
            assert_eq!(
                action_for_shortcut(Key::Num1, m, platform),
                Some(AppAction::ZoomActual),
                "{m:?} en {platform:?}"
            );
        }
        // Ningún otro dígito con `command` hace nada.
        for (key, _) in DIGITS.iter().filter(|(_, slot)| *slot != 1) {
            assert_eq!(
                action_for_shortcut(*key, modifiers_command(platform == Platform::Mac), platform),
                None,
                "{key:?} en {platform:?}"
            );
        }
    }
}

#[test]
fn row_3b_num1_with_shift_alt_or_combined_gives_none() {
    for platform in PLATFORMS {
        let command = modifiers_command(platform == Platform::Mac);
        let command_shift = Modifiers {
            shift: true,
            ..command
        };
        let command_alt = Modifiers {
            alt: true,
            ..command
        };
        for (name_2, m) in [
            ("Mayús", Modifiers::SHIFT),
            ("Alt", Modifiers::ALT),
            ("command+Mayús", command_shift),
            ("command+Alt", command_alt),
        ] {
            assert_eq!(
                action_for_shortcut(Key::Num1, m, platform),
                None,
                "{name_2} en {platform:?}"
            );
        }
    }
}

// ---------------------------------------------------------------------------------------------
// Unidad 19: `AppAction::NewImage` (specs/hito-2-nueva-imagen.md, fila 9, parte de la capa de acciones)
// ---------------------------------------------------------------------------------------------
//
// `ignores_repeat`, `opens_dialog`, `is_blocked`, `enabled`, `shortcut_for` y `action_for_shortcut`
// con la acción nueva. Las pruebas de la 17 que enumeran listas fijas de acciones no la incluyen y
// no cambian; las dos que sí cambian (el oráculo de `action_for_shortcut` y la cuenta de atajos
// mostrados) ya están adaptadas.

#[test]
fn row_9_new_image_does_not_repeat_with_the_held_key() {
    assert!(AppAction::NewImage.ignores_repeat());
    // Y la lista completa pasa a ser las cuatro de fichero y esta.
    let mut all_2 = all_the_actions();
    all_2.push(AppAction::NewImage);
    for action in all_2 {
        let expected_2 = matches!(
            action,
            AppAction::Open
                | AppAction::Save
                | AppAction::SaveAs
                | AppAction::Quit
                | AppAction::NewImage
        );
        assert_eq!(action.ignores_repeat(), expected_2, "{action:?}");
    }
}

#[test]
fn row_9_new_image_opens_a_dialog_and_is_not_zoom() {
    assert!(AppAction::NewImage.opens_dialog());
    assert!(!AppAction::NewImage.is_zoom());
    let mut all_2 = all_the_actions();
    all_2.push(AppAction::NewImage);
    for action in all_2 {
        let expected_2 = matches!(action, AppAction::ShowLicenses | AppAction::NewImage);
        assert_eq!(action.opens_dialog(), expected_2, "{action:?}");
    }
}

#[test]
fn row_9_is_blocked_of_new_image_any_lock_stops_it_with_any_source() {
    for origin in SOURCES {
        assert!(
            !is_blocked(AppAction::NewImage, Locks::default(), origin),
            "{origin:?} sin bloqueos"
        );
        for (name_2, locks) in [
            ("gesture", only(|l| l.gesture = true)),
            ("dialog", only(|l| l.dialog = true)),
            ("text_focus", only(|l| l.text_focus = true)),
            // Regla literal de la 18: la quita pendiente también frena abrir un diálogo.
            ("removal", only(|l| l.removal = true)),
            ("modal", only(|l| l.modal = true)),
        ] {
            assert!(
                is_blocked(AppAction::NewImage, locks, origin),
                "{name_2}, {origin:?}"
            );
        }
    }
    for bits in 1..32u8 {
        for origin in SOURCES {
            assert!(
                is_blocked(AppAction::NewImage, locks_2(bits), origin),
                "{bits:05b}, {origin:?}"
            );
        }
    }
}

#[test]
fn row_9_enabled_of_new_image_is_the_opposite_of_is_blocked_with_menu_source() {
    for zoom in [None, zoom_x5()] {
        for bits in 0..32u8 {
            let locks = locks_2(bits);
            let state = state_2(zoom, locks);
            assert_eq!(
                enabled(AppAction::NewImage, &state),
                !is_blocked(AppAction::NewImage, locks, Origin::Menu),
                "{bits:05b}, zoom {zoom:?}"
            );
        }
    }
    // Sin historia ni zoom, con todo en reposo, sigue habilitada.
    let state = UiState {
        can_undo: false,
        can_redo: false,
        zoom: None,
        tool: Tool::Pencil,
        locks: Locks::default(),
    };
    assert!(enabled(AppAction::NewImage, &state));
}

#[test]
fn row_9_shortcut_for_of_new_image_goes_and_returns_through_action_for_shortcut() {
    for platform in PLATFORMS {
        let shortcut = shortcut_for(AppAction::NewImage, platform).expect("tiene atajo");
        assert_eq!(
            shortcut,
            Shortcut {
                key: KeyId::Char('n'),
                command: true,
                shift: false
            },
            "{platform:?}"
        );
        assert_eq!(
            Keymap::default_scheme().action_for(&pressed_2(shortcut), platform),
            Some(AppAction::NewImage),
            "{platform:?}"
        );
    }
}

#[test]
fn row_9_command_n_gives_new_image_on_the_three_platforms_and_shift_alt_or_n_alone_do_not() {
    for platform in PLATFORMS {
        let command = modifiers_command(platform == Platform::Mac);
        assert_eq!(
            action_for_shortcut(Key::N, command, platform),
            Some(AppAction::NewImage),
            "command+N en {platform:?}"
        );
        let command_shift = Modifiers {
            shift: true,
            ..command
        };
        let command_alt = Modifiers {
            alt: true,
            ..command
        };
        let command_shift_alt = Modifiers {
            shift: true,
            alt: true,
            ..command
        };
        for (name_2, m) in [
            ("Mayús+N", Modifiers::SHIFT),
            ("Alt+N", Modifiers::ALT),
            ("command+Mayús+N", command_shift),
            ("command+Alt+N", command_alt),
            ("command+Mayús+Alt+N", command_shift_alt),
        ] {
            assert_eq!(
                action_for_shortcut(Key::N, m, platform),
                None,
                "{name_2} en {platform:?}"
            );
        }
        // La `n` sola ya no es `None`: centra bajo el cursor (unidad I.1).
        assert_eq!(
            action_for_shortcut(Key::N, Modifiers::NONE, platform),
            Some(AppAction::CenterOnCursor),
            "N sola en {platform:?}"
        );
    }
}

#[test]
fn row_9_n_with_ctrl_or_mac_cmd_but_without_command_is_not_the_shortcut() {
    // `command` es lo que cuenta: en macOS, `Ctrl` solo no lo activa.
    let ctrl_only = Modifiers {
        ctrl: true,
        ..Modifiers::NONE
    };
    let cmd_only = Modifiers {
        mac_cmd: true,
        ..Modifiers::NONE
    };
    for platform in PLATFORMS {
        assert_eq!(action_for_shortcut(Key::N, ctrl_only, platform), None);
        assert_eq!(action_for_shortcut(Key::N, cmd_only, platform), None);
    }
}

// ---------------------------------------------------------------------------------------------
// Unidad 20: `AppAction::ConvertMode` (specs/hito-2-convertir-modo.md, fila 10, capa de acciones)
// ---------------------------------------------------------------------------------------------
//
// Sin atajo en ninguna plataforma (ninguna tecla con ninguna combinación la produce), no repite
// (`ignores_repeat` falso), abre un diálogo (`opens_dialog`) y la frenan los cinco bloqueos, también
// `removal` (regla literal de la 18), con los tres orígenes. El zoom y el resto no cambian. Las
// pruebas de la 17 y la 19 que enumeran listas fijas de acciones no la incluyen y no cambian.

#[test]
fn row_10_convert_mode_has_no_shortcut_on_any_platform() {
    for platform in PLATFORMS {
        assert_eq!(
            shortcut_for(AppAction::ConvertMode, platform),
            None,
            "{platform:?}"
        );
    }
}

#[test]
fn row_10_no_key_with_no_combination_gives_convert_mode() {
    assert!(!Key::ALL.is_empty());
    for platform in PLATFORMS {
        for &key in Key::ALL {
            for bits in 0..32 {
                let m = modifiers_2(bits);
                assert_ne!(
                    action_for_shortcut(key, m, platform),
                    Some(AppAction::ConvertMode),
                    "{key:?} con {m:?} en {platform:?}"
                );
            }
        }
    }
}

#[test]
fn row_10_convert_mode_does_not_repeat_with_the_held_key_and_the_others_stay_the_same() {
    assert!(!AppAction::ConvertMode.ignores_repeat());
    let mut all_2 = all_the_actions();
    all_2.push(AppAction::NewImage);
    all_2.push(AppAction::ConvertMode);
    for action in all_2 {
        let expected_2 = matches!(
            action,
            AppAction::Open
                | AppAction::Save
                | AppAction::SaveAs
                | AppAction::Quit
                | AppAction::NewImage
        );
        assert_eq!(action.ignores_repeat(), expected_2, "{action:?}");
    }
}

#[test]
fn row_10_convert_mode_opens_a_dialog_and_is_not_zoom() {
    assert!(AppAction::ConvertMode.opens_dialog());
    assert!(!AppAction::ConvertMode.is_zoom());
    let mut all_2 = all_the_actions();
    all_2.push(AppAction::NewImage);
    all_2.push(AppAction::ConvertMode);
    for action in all_2 {
        let expected_2 = matches!(
            action,
            AppAction::ShowLicenses | AppAction::NewImage | AppAction::ConvertMode
        );
        assert_eq!(action.opens_dialog(), expected_2, "{action:?}");
    }
}

#[test]
fn row_10_is_blocked_of_convert_mode_each_lock_stops_it_with_any_source() {
    for origin in SOURCES {
        assert!(
            !is_blocked(AppAction::ConvertMode, Locks::default(), origin),
            "{origin:?} sin bloqueos"
        );
        for (name_2, locks) in [
            ("gesture", only(|l| l.gesture = true)),
            ("dialog", only(|l| l.dialog = true)),
            ("text_focus", only(|l| l.text_focus = true)),
            // Regla literal de la 18: la quita pendiente también frena abrir un diálogo.
            ("removal", only(|l| l.removal = true)),
            ("modal", only(|l| l.modal = true)),
        ] {
            assert!(
                is_blocked(AppAction::ConvertMode, locks, origin),
                "{name_2}, {origin:?}"
            );
        }
    }
    for bits in 1..32u8 {
        for origin in SOURCES {
            assert!(
                is_blocked(AppAction::ConvertMode, locks_2(bits), origin),
                "{bits:05b}, {origin:?}"
            );
        }
    }
}

#[test]
fn row_10_enabled_of_convert_mode_is_the_opposite_of_is_blocked_with_menu_source() {
    for zoom in [None, zoom_x5()] {
        for bits in 0..32u8 {
            let locks = locks_2(bits);
            let state = state_2(zoom, locks);
            assert_eq!(
                enabled(AppAction::ConvertMode, &state),
                !is_blocked(AppAction::ConvertMode, locks, Origin::Menu),
                "{bits:05b}, zoom {zoom:?}"
            );
        }
    }
    // Sin historia ni zoom, con todo en reposo, sigue habilitada.
    let state = UiState {
        can_undo: false,
        can_redo: false,
        zoom: None,
        tool: Tool::Pencil,
        locks: Locks::default(),
    };
    assert!(enabled(AppAction::ConvertMode, &state));
}

#[test]
fn row_10_with_the_dialog_open_only_quit_passes_and_convert_mode_stays_blocked() {
    let modal = only(|l| l.modal = true);
    for origin in SOURCES {
        assert!(
            is_blocked(AppAction::ConvertMode, modal, origin),
            "{origin:?}"
        );
        // `Quit` sigue pasando con el diálogo abierto (regla (2) de la 18).
        assert!(!is_blocked(AppAction::Quit, modal, origin), "{origin:?}");
    }
}

#[test]
fn row_10_the_zoom_does_not_change_with_the_new_action() {
    // El zoom solo se bloquea con `text_focus` desde el teclado; un gesto o una quita pendiente no
    // lo paran (y sí paran `ConvertMode`).
    for zoom in ZOOMS {
        for origin in SOURCES {
            assert!(!is_blocked(zoom, only(|l| l.gesture = true), origin));
            assert!(!is_blocked(zoom, only(|l| l.removal = true), origin));
            assert_eq!(
                is_blocked(zoom, only(|l| l.text_focus = true), origin),
                origin == Origin::Key,
                "{zoom:?}, {origin:?}"
            );
        }
    }
    for origin in SOURCES {
        assert!(is_blocked(
            AppAction::ConvertMode,
            only(|l| l.gesture = true),
            origin
        ));
        assert!(is_blocked(
            AppAction::ConvertMode,
            only(|l| l.removal = true),
            origin
        ));
    }
}
