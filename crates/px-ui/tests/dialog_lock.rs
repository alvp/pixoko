//! Tests del bloqueo por diálogo modal en `px-ui` (specs/hito-2-dialogos-modales.md, «Casos» fila 6,
//! la parte pura: `dispatch`, `ignored_file_notice` y el cierre de ventana están en `px-app`).
//! Sin ventana: `Key` y `Modifiers` de egui y lógica pura.
//!
//! - (a) `can_open_dialog` con las 32 combinaciones de los cinco bloqueos.
//! - (b) `Modal::open` con cada bloqueo solo, con todos a falso, con uno ya abierto y con una spec
//!   inválida.
//! - (c) con `Locks { modal: true }`: `is_blocked` y `enabled` de **todas** las `AppAction`
//!   (herramientas, ranuras 1 a 9, los cuatro fondos, zoom, fichero, deshacer, rehacer y
//!   licencias) con los tres `Origin`: bloqueadas salvo `CloseLicenses` y `Quit` (decisión D1).
//! - (d) `action_for_shortcut` sobre `Key::ALL` × 32 combinaciones de modificadores × tres
//!   plataformas: lo traducido sigue sujeto a `is_blocked` con modal (solo `Quit` pasa, y solo
//!   `Ctrl+Q` en Linux lo traduce); `AppAction::opens_dialog()` es verdadero solo para
//!   `ShowLicenses`; y `is_blocked` bloquea con `removal` exactamente `SelectColor(_)` y las
//!   acciones que abren un diálogo (decisión 2, regla literal), con las demás acciones y los tres
//!   orígenes.
//!
//! Suposiciones sobre las firmas (derives no listados): `Dialog` es `Clone + PartialEq` y `Modal`
//! tiene `Default` (= `Closed`); no se exige nada más (para mirar dentro de un `Modal` abierto se
//! hace `match` sobre `Modal::Open(dialog)`). `Modal::open` con una spec inválida da `false` y
//! deja el `Modal` cerrado (spec: «false si `can_open_dialog` es falso o la spec no valida»).
//!
//! Decisión donde la spec no fija un detalle: `Locks` con `removal` solo no bloquea `Quit`, el
//! zoom, el fondo, deshacer ni rehacer (lo que ya comprobaban las pruebas de la 17); el modelo
//! completo de las 32 combinaciones está en `actions.rs`.

use egui::Key;
use px_ui::actions::{
    AppAction, Locks, Origin, Platform, UiState, action_for_shortcut, can_open_dialog, enabled,
    is_blocked,
};
use px_ui::background::CanvasBackground;
use px_ui::dialog::{Dialog, DialogKind, FieldKind, FieldSpec, FormSpec, Modal};
use px_ui::licenses_view::licenses_form;
use px_ui::tool::Tool;
use px_ui::view::Zoom;

mod common;
use common::{modifiers_2, modifiers_command};

#[path = "common/dialog_forms.rs"]
mod dialog_forms;

// ---------------------------------------------------------------------------------------------
// Ayudantes
// ---------------------------------------------------------------------------------------------

const SOURCES: [Origin; 3] = [Origin::Menu, Origin::Button, Origin::Key];
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

/// Todas las acciones: fichero, salir, deshacer, rehacer, zoom, licencias (abrir y cerrar), las
/// cinco herramientas, las nueve ranuras y los cuatro fondos del lienzo.
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
    v.extend(BACKGROUNDS.map(AppAction::CanvasBackground));
    v
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

/// Los cinco bloqueos, uno por uno, con su nombre.
fn locks_one_by_one() -> [(&'static str, Locks); 5] {
    [
        ("gesture", only(|l| l.gesture = true)),
        ("dialog", only(|l| l.dialog = true)),
        ("text_focus", only(|l| l.text_focus = true)),
        ("removal", only(|l| l.removal = true)),
        ("modal", only(|l| l.modal = true)),
    ]
}

fn state_2(locks: Locks) -> UiState {
    UiState {
        can_undo: true,
        can_redo: true,
        zoom: Zoom::from_int(5),
        tool: Tool::Pencil,
        locks,
    }
}

fn passes_with_modal(action: AppAction) -> bool {
    matches!(action, AppAction::CloseLicenses | AppAction::Quit)
}

/// Una spec inválida: dos campos con el mismo id.
fn spec_invalid() -> FormSpec {
    let box_value = |label| FieldSpec {
        id: "repetido",
        label,
        kind: FieldKind::Check { default: false },
        shown_when: None,
    };
    FormSpec {
        title: "Inválida",
        accept: "Aceptar",
        cancel: Some("Cancelar"),
        fields: vec![box_value("Uno"), box_value("Dos")],
        check: None,
    }
}

fn open_2_2(modal: &Modal) -> Option<&Dialog> {
    match modal {
        Modal::Open(dialog) => Some(dialog),
        Modal::Closed => None,
    }
}

// ---------------------------------------------------------------------------------------------
// Fila 6 (a): can_open_dialog
// ---------------------------------------------------------------------------------------------

#[test]
fn row_6a_can_open_dialog_is_true_only_with_the_five_locks_false() {
    let mut certain = 0;
    for bits in 0..32u8 {
        let locks = locks_2(bits);
        let expected_2 =
            !(locks.gesture || locks.dialog || locks.text_focus || locks.removal || locks.modal);
        assert_eq!(can_open_dialog(locks), expected_2, "{locks:?}");
        assert_eq!(can_open_dialog(locks), bits == 0, "{locks:?}");
        certain += usize::from(can_open_dialog(locks));
    }
    assert_eq!(certain, 1, "solo una de las 32 combinaciones permite abrir");
}

#[test]
fn row_6a_with_everything_false_it_can_be_opened() {
    assert!(can_open_dialog(Locks::default()));
}

#[test]
fn row_6a_each_lock_alone_prevents_opening_and_all_together_too() {
    for (name_2, locks) in locks_one_by_one() {
        assert!(!can_open_dialog(locks), "{name_2}");
    }
    let all_2 = Locks {
        gesture: true,
        stroke: false,
        dialog: true,
        text_focus: true,
        removal: true,
        modal: true,
    };
    assert!(!can_open_dialog(all_2));
}

// ---------------------------------------------------------------------------------------------
// Fila 6 (b): Modal::open
// ---------------------------------------------------------------------------------------------

#[test]
fn row_6b_open_with_everything_false_opens_the_dialog() {
    let mut modal = Modal::default();
    assert!(!modal.is_open());
    assert_eq!(modal.kind(), None);
    assert!(modal.open(
        DialogKind::Licenses,
        licenses_form("texto"),
        Locks::default()
    ));
    assert!(modal.is_open());
    assert_eq!(modal.kind(), Some(DialogKind::Licenses));
    assert!(open_2_2(&modal).is_some());
}

#[test]
fn row_6b_open_with_each_lock_alone_gives_false_and_stays_closed() {
    for (name_2, locks) in locks_one_by_one() {
        let mut modal = Modal::default();
        assert!(
            !modal.open(DialogKind::Licenses, licenses_form("texto"), locks),
            "{name_2}"
        );
        assert!(!modal.is_open(), "{name_2}");
        assert_eq!(modal.kind(), None, "{name_2}");
        assert!(matches!(modal, Modal::Closed), "{name_2}");
    }
}

#[test]
fn row_6b_open_with_the_32_combinations_opens_only_with_everything_false() {
    for bits in 0..32u8 {
        let locks = locks_2(bits);
        let mut modal = Modal::default();
        let opens = modal.open(DialogKind::Licenses, licenses_form("texto"), locks);
        assert_eq!(opens, bits == 0, "{locks:?}");
        assert_eq!(modal.is_open(), bits == 0, "{locks:?}");
        assert_eq!(modal.kind().is_some(), bits == 0, "{locks:?}");
    }
}

#[test]
fn row_6b_with_one_open_another_open_gives_false_with_any_lock_and_does_not_touch_its_values() {
    for bits in 0..32u8 {
        let locks = locks_2(bits);
        let mut modal = Modal::default();
        assert!(modal.open(
            DialogKind::Licenses,
            dialog_forms::all_kinds(),
            Locks::default()
        ));
        let before = open_2_2(&modal).expect("abierto").clone();
        // La app pone `modal: true` mientras hay uno abierto, pero la regla también se sostiene
        // sin ese bloqueo: un segundo `open` nunca sustituye al primero.
        assert!(
            !modal.open(DialogKind::Licenses, licenses_form("otro"), locks),
            "{locks:?}"
        );
        assert!(modal.is_open(), "{locks:?}");
        assert_eq!(
            open_2_2(&modal).expect("sigue abierto"),
            &before,
            "{locks:?}: el diálogo abierto no cambia"
        );
    }
}

#[test]
fn row_6b_open_with_the_modal_lock_of_the_open_dialog_itself_gives_false() {
    let mut modal = Modal::default();
    assert!(modal.open(DialogKind::Licenses, licenses_form("uno"), Locks::default()));
    let with_modal = only(|l| l.modal = modal.is_open());
    assert!(with_modal.modal);
    assert!(!modal.open(DialogKind::Licenses, licenses_form("dos"), with_modal));
    assert!(modal.is_open());
}

#[test]
fn row_6b_open_with_an_invalid_spec_gives_false_and_stays_closed() {
    let mut modal = Modal::default();
    assert!(spec_invalid().validate().is_err());
    assert!(!modal.open(DialogKind::Licenses, spec_invalid(), Locks::default()));
    assert!(!modal.is_open());
    assert!(matches!(modal, Modal::Closed));
}

#[test]
fn row_6b_it_can_be_reopened_after_closing() {
    let mut modal = Modal::default();
    assert!(modal.open(DialogKind::Licenses, licenses_form("uno"), Locks::default()));
    assert!(modal.abandon());
    assert!(!modal.is_open());
    // Cerrado, con un bloqueo no abre; sin bloqueos, sí.
    assert!(!modal.open(
        DialogKind::Licenses,
        licenses_form("dos"),
        only(|l| l.gesture = true)
    ));
    assert!(modal.open(DialogKind::Licenses, licenses_form("dos"), Locks::default()));
}

// ---------------------------------------------------------------------------------------------
// Fila 6 (c): con modal, todo bloqueado salvo CloseLicenses y Quit
// ---------------------------------------------------------------------------------------------

#[test]
fn row_6c_with_a_modal_is_blocked_is_true_except_for_close_licenses_and_quit() {
    let locks = only(|l| l.modal = true);
    let actions = all_the_actions();
    // 12 + 5 herramientas + 9 ranuras + 4 fondos.
    assert_eq!(actions.len(), 30);
    for action in actions {
        for origin in SOURCES {
            assert_eq!(
                is_blocked(action, locks, origin),
                !passes_with_modal(action),
                "{action:?} con modal y {origin:?}"
            );
        }
    }
}

#[test]
fn row_6c_with_a_modal_enabled_is_false_except_for_close_licenses_and_quit() {
    let locks = only(|l| l.modal = true);
    for action in all_the_actions() {
        assert_eq!(
            enabled(action, &state_2(locks)),
            passes_with_modal(action),
            "{action:?} con modal"
        );
    }
}

#[test]
fn row_6c_enabled_with_a_modal_does_not_depend_on_the_history_or_the_zoom_or_the_tool() {
    let locks = only(|l| l.modal = true);
    for can_undo in [false, true] {
        for can_redo in [false, true] {
            for zoom in [None, Zoom::from_int(1), Zoom::from_int(5), Some(Zoom::MAX)] {
                for tool in TEST_TOOLS {
                    let state = UiState {
                        can_undo,
                        can_redo,
                        zoom,
                        tool,
                        locks,
                    };
                    for action in all_the_actions() {
                        // `CloseLicenses` y `Quit` no dependen de la historia ni del zoom.
                        assert_eq!(
                            enabled(action, &state),
                            passes_with_modal(action),
                            "{action:?} con {state:?}"
                        );
                    }
                }
            }
        }
    }
}

#[test]
fn row_6c_quit_and_close_licenses_pass_with_a_modal_and_with_the_three_sources() {
    let locks = only(|l| l.modal = true);
    for action in [AppAction::Quit, AppAction::CloseLicenses] {
        for origin in SOURCES {
            assert!(!is_blocked(action, locks, origin), "{action:?} {origin:?}");
        }
        assert!(enabled(action, &state_2(locks)), "{action:?}");
    }
}

#[test]
fn row_6c_each_action_blocked_with_a_modal_is_also_blocked_with_the_menu_the_button_and_the_key() {
    // Las ranuras 1 a 9, las cinco herramientas, los cuatro fondos y las cuatro de zoom, una a
    // una (el zoom con el campo de texto sin foco: solo el modal lo bloquea).
    let locks = only(|l| l.modal = true);
    let without_modal = Locks::default();
    for action in all_the_actions() {
        if passes_with_modal(action) {
            continue;
        }
        for origin in SOURCES {
            assert!(
                !is_blocked(action, without_modal, origin),
                "{action:?} libre"
            );
            assert!(is_blocked(action, locks, origin), "{action:?} {origin:?}");
        }
    }
}

// ---------------------------------------------------------------------------------------------
// Fila 6 (d): teclas con modal
// ---------------------------------------------------------------------------------------------

#[test]
fn row_6d_what_action_for_shortcut_translates_stays_blocked_with_a_modal_except_quit() {
    let locks = only(|l| l.modal = true);
    let mut translated = 0usize;
    let mut quits = 0usize;
    for key in Key::ALL {
        for bits in 0..32u8 {
            let modifiers = modifiers_2(bits);
            for platform in PLATFORMS {
                let Some(action) = action_for_shortcut(*key, modifiers, platform) else {
                    continue;
                };
                translated += 1;
                let context = format!("{key:?} con {modifiers:?} en {platform:?}: {action:?}");
                for origin in SOURCES {
                    assert_eq!(
                        is_blocked(action, locks, origin),
                        action != AppAction::Quit,
                        "{context} ({origin:?})"
                    );
                }
                // Las teclas llegan con `Origin::Key`; el menú las ve con `enabled`.
                assert_eq!(
                    enabled(action, &state_2(locks)),
                    action == AppAction::Quit,
                    "{context}"
                );
                assert_ne!(
                    action,
                    AppAction::CloseLicenses,
                    "{context}: ningún atajo cierra las licencias"
                );
                quits += usize::from(action == AppAction::Quit);
            }
        }
    }
    assert!(translated > 100, "se tradujeron {translated} combinaciones");
    assert!(quits > 0, "hay combinaciones que dan Quit");
}

#[test]
fn row_6d_only_ctrl_q_on_linux_gives_quit_and_it_is_the_only_one_that_passes_with_a_modal() {
    let locks = only(|l| l.modal = true);
    for key in Key::ALL {
        for bits in 0..32u8 {
            let modifiers = modifiers_2(bits);
            for platform in PLATFORMS {
                let passes = action_for_shortcut(*key, modifiers, platform)
                    .is_some_and(|a| !is_blocked(a, locks, Origin::Key));
                if passes {
                    assert_eq!(*key, Key::Q, "{key:?} {modifiers:?} {platform:?}");
                    assert_eq!(platform, Platform::Linux, "{key:?} {modifiers:?}");
                    assert!(modifiers.command, "{key:?} {modifiers:?}");
                    assert!(!modifiers.alt && !modifiers.shift, "{key:?} {modifiers:?}");
                }
            }
        }
    }
    assert_eq!(
        action_for_shortcut(Key::Q, modifiers_command(false), Platform::Linux),
        Some(AppAction::Quit)
    );
    assert!(!is_blocked(AppAction::Quit, locks, Origin::Key));
    // En Windows y macOS `Ctrl`/`Cmd`+`Q` no es un atajo de la app.
    assert_eq!(
        action_for_shortcut(Key::Q, modifiers_command(false), Platform::Windows),
        None
    );
    assert_eq!(
        action_for_shortcut(Key::Q, modifiers_command(true), Platform::Mac),
        None
    );
}

#[test]
fn row_6d_the_tool_color_zoom_and_file_keys_are_blocked_with_a_modal() {
    let locks = only(|l| l.modal = true);
    let cases = [
        (Key::D, modifiers_2(0)),
        (Key::CloseBracket, modifiers_2(0)),
        (Key::OpenBracket, modifiers_2(0)),
        (Key::Plus, modifiers_2(0)),
        (Key::Minus, modifiers_2(0)),
        (Key::Home, modifiers_2(0)),
        (Key::Z, modifiers_command(false)),
        (Key::O, modifiers_command(false)),
        (Key::S, modifiers_command(false)),
        (Key::Num1, modifiers_command(false)),
    ];
    for (key, modifiers) in cases {
        let action = action_for_shortcut(key, modifiers, Platform::Linux)
            .unwrap_or_else(|| panic!("{key:?} con {modifiers:?} debería traducirse"));
        assert!(
            is_blocked(action, locks, Origin::Key),
            "{key:?}: {action:?}"
        );
    }
}

// ---------------------------------------------------------------------------------------------
// Fila 6 (d): opens_dialog y la regla de la quita pendiente (decisión 2)
// ---------------------------------------------------------------------------------------------

#[test]
fn row_6d_opens_dialog_is_true_only_for_show_licenses() {
    for action in all_the_actions() {
        assert_eq!(
            action.opens_dialog(),
            action == AppAction::ShowLicenses,
            "{action:?}"
        );
    }
}

#[test]
fn row_6d_with_removal_is_blocked_blocks_exactly_select_color_and_show_licenses() {
    let locks = only(|l| l.removal = true);
    let mut locked = Vec::new();
    for action in all_the_actions() {
        for origin in SOURCES {
            let expected_2 = matches!(action, AppAction::SelectColor(_) | AppAction::ShowLicenses);
            assert_eq!(
                is_blocked(action, locks, origin),
                expected_2,
                "{action:?} con removal y {origin:?}"
            );
        }
        if is_blocked(action, locks, Origin::Menu) {
            locked.push(action);
        }
    }
    // Las nueve ranuras y `ShowLicenses`, nada más.
    assert_eq!(locked.len(), 10, "{locked:?}");
}

#[test]
fn row_6d_rule_5_is_select_color_or_an_action_that_opens_a_dialog() {
    let locks = only(|l| l.removal = true);
    for action in all_the_actions() {
        for origin in SOURCES {
            assert_eq!(
                is_blocked(action, locks, origin),
                matches!(action, AppAction::SelectColor(_)) || action.opens_dialog(),
                "{action:?} {origin:?}"
            );
        }
    }
}

#[test]
fn row_6d_with_removal_quit_zoom_background_undo_and_redo_and_tools_stay_free() {
    let locks = only(|l| l.removal = true);
    let free = [
        AppAction::Quit,
        AppAction::CloseLicenses,
        AppAction::Undo,
        AppAction::Redo,
        AppAction::Open,
        AppAction::Save,
        AppAction::SaveAs,
        AppAction::ZoomIn,
        AppAction::ZoomOut,
        AppAction::ZoomFit,
        AppAction::ZoomActual,
        AppAction::SelectTool(Tool::Fill),
        AppAction::CanvasBackground(CanvasBackground::Black),
    ];
    for action in free {
        for origin in SOURCES {
            assert!(!is_blocked(action, locks, origin), "{action:?} {origin:?}");
        }
        assert!(enabled(action, &state_2(locks)), "{action:?}");
    }
    assert!(!enabled(AppAction::ShowLicenses, &state_2(locks)));
    assert!(!enabled(AppAction::SelectColor(1), &state_2(locks)));
}

#[test]
fn row_6d_show_licenses_is_blocked_exactly_when_a_dialog_cannot_be_opened() {
    for bits in 0..32u8 {
        let locks = locks_2(bits);
        for origin in SOURCES {
            assert_eq!(
                is_blocked(AppAction::ShowLicenses, locks, origin),
                !can_open_dialog(locks),
                "{locks:?} {origin:?}"
            );
        }
        assert_eq!(
            enabled(AppAction::ShowLicenses, &state_2(locks)),
            can_open_dialog(locks),
            "{locks:?}"
        );
    }
}

#[test]
fn row_6d_no_action_that_opens_a_dialog_is_let_through_if_it_cannot_be_opened() {
    for action in all_the_actions().into_iter().filter(|a| a.opens_dialog()) {
        for bits in 0..32u8 {
            let locks = locks_2(bits);
            for origin in SOURCES {
                if !can_open_dialog(locks) {
                    assert!(
                        is_blocked(action, locks, origin),
                        "{action:?} {locks:?} {origin:?}"
                    );
                }
            }
        }
    }
}

#[test]
fn row_6d_each_lock_alone_only_stops_show_licenses_with_the_three_sources() {
    for (name_2, locks) in locks_one_by_one() {
        for origin in SOURCES {
            assert!(
                is_blocked(AppAction::ShowLicenses, locks, origin),
                "{name_2} {origin:?}"
            );
        }
    }
    for origin in SOURCES {
        assert!(!is_blocked(
            AppAction::ShowLicenses,
            Locks::default(),
            origin
        ));
    }
}
