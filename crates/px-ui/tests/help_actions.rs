//! Tests de las acciones de ayuda, su destino y sus bloqueos (specs/interfaz-ayuda.md, unidad I.5).
//! En ROJO hasta la lógica: `help`, `AppAction::{ShowHelp, ShowShortcuts}` y la entrada de menú no
//! existen todavía.
//!
//! - Fila 3: destino de `F1` (`help_target_for_button`, `help_target_at`) y menú.
//! - Fila 4: bloqueos de `ShowHelp` y `ShowShortcuts` con las 64 combinaciones de `Locks`.
//! - Fila 12 (parte de menú): las tres entradas de `Ayuda` y sus rótulos.

use px_ui::actions::{
    AppAction, Locks, Origin, Platform, UiState, can_open_dialog, enabled, is_blocked,
};
use px_ui::chrome::{TOOLBOX_BUTTONS, toolbox_button_rect_u, window_layout};
use px_ui::help::{HelpTarget, help_target_at, help_target_for_button};
use px_ui::menu::{MENUS, popup_width_u, shortcut_for};
use px_ui::tool::Tool;

const SOURCES: [Origin; 3] = [Origin::Menu, Origin::Button, Origin::Key];

fn locks_2(bits: u8) -> Locks {
    Locks {
        gesture: bits & 1 != 0,
        stroke: bits & 2 != 0,
        dialog: bits & 4 != 0,
        text_focus: bits & 8 != 0,
        removal: bits & 16 != 0,
        modal: bits & 32 != 0,
    }
}

fn state_2(locks: Locks) -> UiState {
    UiState {
        can_undo: false,
        can_redo: false,
        zoom: None,
        tool: Tool::Pencil,
        locks,
    }
}

const ACTIONS: [AppAction; 2] = [
    AppAction::ShowHelp(HelpTarget::Active),
    AppAction::ShowShortcuts,
];

// ---------------------------------------------------------------------------------------------
// Fila 3: destino
// ---------------------------------------------------------------------------------------------

#[test]
fn row_3_each_toolbox_button_has_its_target() {
    let expected_2 = [
        HelpTarget::Tool(Tool::Pencil),
        HelpTarget::Tool(Tool::Line),
        HelpTarget::Tool(Tool::Fill),
        HelpTarget::Tool(Tool::Dropper),
        HelpTarget::Tool(Tool::Eraser),
        HelpTarget::ZoomIn,
        HelpTarget::ZoomOut,
        HelpTarget::ZoomFit,
        HelpTarget::ZoomActualSize,
    ];
    assert_eq!(TOOLBOX_BUTTONS, 9);
    for (i, target_2) in expected_2.into_iter().enumerate() {
        assert_eq!(help_target_for_button(i), Some(target_2), "botón {i}");
    }
    assert_eq!(help_target_for_button(9), None);
}

#[test]
fn row_3_the_pointer_over_a_button_gives_the_target_and_outside_them_the_active_tool() {
    for n in [1u32, 2, 3] {
        let k = i64::from(n);
        let window = (960 * k, 720 * k);
        let layout = window_layout(window, n);
        // Sobre cada botón (el centro y las dos esquinas), también si estuviera atenuado.
        for i in 0..TOOLBOX_BUTTONS {
            let (x, y, w, h) = toolbox_button_rect_u(i);
            for (dx, dy) in [(w * k / 2, h * k / 2), (0, 0), (w * k - 1, h * k - 1)] {
                let point = (layout.toolbar.x + x * k + dx, layout.toolbar.y + y * k + dy);
                assert_eq!(
                    help_target_at(point, window, n),
                    help_target_for_button(i).unwrap(),
                    "botón {i} a n={n} en {point:?}"
                );
            }
        }
        // En un hueco entre botones, un píxel fuera, el recuadro de opciones, el lienzo, el panel,
        // el menú, la barra de estado y fuera de la ventana: la activa.
        let (x0, y0, w0, h0) = toolbox_button_rect_u(0);
        let outside = [
            (
                layout.toolbar.x + (x0 + w0) * k,
                layout.toolbar.y + y0 * k + h0 * k / 2,
            ),
            (layout.toolbar.x + x0 * k - 1, layout.toolbar.y + y0 * k + 1),
            (layout.toolbar.x + 10 * k, layout.toolbar.y + 200 * k),
            (layout.canvas.x + 10, layout.canvas.y + 10),
            (layout.panel.x + 10, layout.panel.y + 10),
            (10, 1),
            (10, window.1 - 1),
            (-5, -5),
            (window.0 + 50, window.1 + 50),
        ];
        for point in outside {
            assert_eq!(
                help_target_at(point, window, n),
                HelpTarget::Active,
                "{point:?} a n={n}"
            );
        }
    }
}

#[test]
fn row_3_the_help_menu_has_three_entries_and_f1_only_on_the_first() {
    let help = MENUS
        .iter()
        .find(|m| m.title == "Ayuda")
        .expect("menú Ayuda");
    let entries: Vec<(&str, AppAction)> = help.items.iter().map(|i| (i.label, i.action)).collect();
    assert_eq!(
        entries,
        [
            (
                "Ayuda de la herramienta",
                AppAction::ShowHelp(HelpTarget::Active)
            ),
            ("Atajos de teclado…", AppAction::ShowShortcuts),
            ("Licencias", AppAction::ShowLicenses),
        ]
    );
    // `F1` es fijo y no sale de la tabla de teclado.
    for platform in [Platform::Linux, Platform::Windows, Platform::Mac] {
        for action in ACTIONS {
            assert_eq!(shortcut_for(action, platform), None, "{action:?}");
        }
        assert_eq!(popup_width_u(help, platform), 201, "{platform:?}");
    }
    assert_eq!(
        px_ui::menu::fixed_shortcut(AppAction::ShowHelp(HelpTarget::Active)),
        Some("F1")
    );
    assert_eq!(px_ui::menu::fixed_shortcut(AppAction::ShowShortcuts), None);
}

// ---------------------------------------------------------------------------------------------
// Fila 4: bloqueos
// ---------------------------------------------------------------------------------------------

#[test]
fn row_4_they_are_blocked_exactly_like_an_action_that_opens_a_dialog() {
    for action in ACTIONS {
        assert!(action.opens_dialog(), "{action:?}");
        assert!(AppAction::ShowHelp(HelpTarget::Tool(Tool::Fill)).opens_dialog());
        for bits in 0..64u8 {
            let locks = locks_2(bits);
            for source_2 in SOURCES {
                let expected_2 = locks.modal
                    || locks.dialog
                    || locks.gesture
                    || locks.text_focus
                    || locks.removal;
                assert_eq!(
                    is_blocked(action, locks, source_2),
                    expected_2,
                    "{action:?} {locks:?} {source_2:?}"
                );
            }
            assert_eq!(
                enabled(action, &state_2(locks)),
                !is_blocked(action, locks, Origin::Menu)
            );
        }
        // Sin imagen (zoom `None`) tampoco se bloquea.
        assert!(enabled(action, &state_2(Locks::default())));
        assert!(can_open_dialog(Locks::default()));
    }
}

#[test]
fn row_4_the_help_does_not_repeat_with_the_held_key() {
    assert!(AppAction::ShowHelp(HelpTarget::Active).ignores_repeat());
    assert!(AppAction::ShowHelp(HelpTarget::ZoomIn).ignores_repeat());
}
