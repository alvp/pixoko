//! Tests de `shortcuts::action_for_key` (specs/hito-2-lapiz-deshacer.md, sección 5; fila 12 y
//! propiedad 6).
//!
//! La función es pura y total: recibe una `egui::Key` y unos `egui::Modifiers` como datos, sin
//! `Context`. La propiedad 6 es exhaustiva (todas las combinaciones de los cinco booleanos y
//! todas las teclas de `Key::ALL`), así que no hace falta generar nada al azar.

use egui::{Key, Modifiers};
use px_ui::shortcuts::{Action, action_for_key};

/// Los cinco booleanos de `Modifiers`, en el orden de una máscara de 5 bits.
fn modifiers_2(mask: u8) -> Modifiers {
    Modifiers {
        alt: mask & 1 != 0,
        ctrl: mask & 2 != 0,
        shift: mask & 4 != 0,
        mac_cmd: mask & 8 != 0,
        command: mask & 16 != 0,
    }
}

#[test]
fn row_12_undo_and_redo() {
    assert_eq!(
        action_for_key(Key::Z, Modifiers::COMMAND),
        Some(Action::Undo)
    );
    assert_eq!(
        action_for_key(Key::Y, Modifiers::COMMAND),
        Some(Action::Redo)
    );
    assert_eq!(
        action_for_key(Key::Z, Modifiers::COMMAND | Modifiers::SHIFT),
        Some(Action::Redo)
    );
}

#[test]
fn row_12_what_is_not_a_shortcut_gives_none() {
    assert_eq!(action_for_key(Key::Z, Modifiers::NONE), None);
    assert_eq!(
        action_for_key(Key::Y, Modifiers::COMMAND | Modifiers::SHIFT),
        None
    );
    assert_eq!(
        action_for_key(Key::Z, Modifiers::COMMAND | Modifiers::ALT),
        None
    );
    // Sin `command` no hay deshacer, ni con Mayús ni con Alt.
    assert_eq!(action_for_key(Key::Z, Modifiers::SHIFT), None);
    assert_eq!(action_for_key(Key::Y, Modifiers::NONE), None);
    assert_eq!(action_for_key(Key::Y, Modifiers::ALT), None);
}

#[test]
fn row_12_the_numeric_keys_without_modifiers_do_nothing() {
    let expected_2 = [
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
    for (key, n) in expected_2 {
        assert_eq!(
            action_for_key(key, Modifiers::NONE),
            None,
            "{key:?} (antes elegía la ranura {n})"
        );
    }
    assert_eq!(action_for_key(Key::Num0, Modifiers::NONE), None);
    assert_eq!(action_for_key(Key::Num1, Modifiers::SHIFT), None);
    assert_eq!(action_for_key(Key::Num1, Modifiers::COMMAND), None);
}

#[test]
fn in_practice_command_arrives_together_with_ctrl_or_mac_cmd() {
    // En Linux y Windows egui pone `ctrl` y `command`; en macOS, `mac_cmd` y `command`.
    let ctrl = Modifiers {
        ctrl: true,
        command: true,
        ..Modifiers::NONE
    };
    let cmd = Modifiers {
        mac_cmd: true,
        command: true,
        ..Modifiers::NONE
    };
    for m in [ctrl, cmd] {
        assert_eq!(action_for_key(Key::Z, m), Some(Action::Undo), "{m:?}");
        assert_eq!(action_for_key(Key::Y, m), Some(Action::Redo), "{m:?}");
        assert_eq!(
            action_for_key(Key::Z, m | Modifiers::SHIFT),
            Some(Action::Redo),
            "{m:?}"
        );
        assert_eq!(action_for_key(Key::Z, m | Modifiers::ALT), None, "{m:?}");
        assert_eq!(action_for_key(Key::Num1, m), None, "{m:?}");
    }
}

#[test]
fn the_action_is_comparable_and_can_be_printed() {
    // Los tests y la app comparan acciones: `Debug` y `PartialEq`.
    let a = Action::SelectColor(3);
    assert_eq!(a, Action::SelectColor(3));
    assert_ne!(Action::Undo, Action::Redo);
    assert_ne!(Action::SelectColor(1), Action::SelectColor(2));
    let _ = format!("{a:?}");
}

/// Propiedad 6, exhaustiva: 32 combinaciones de modificadores por todas las teclas.
#[test]
fn property_6_all_the_combinations_and_all_the_keys() {
    assert!(Key::ALL.len() > 100, "Key::ALL incluye todas las teclas");
    for key in Key::ALL.iter().copied() {
        for mask in 0..32u8 {
            let m = modifiers_2(mask);
            // No entra en pánico (la llamada ya lo comprueba).
            let action_2 = action_for_key(key, m);
            let none = !(m.alt || m.ctrl || m.shift || m.mac_cmd || m.command);

            // `Undo` y `Redo` solo con `command` y sin `alt` (y `Undo` también con `u` sola).
            if matches!(action_2, Some(Action::Undo | Action::Redo)) {
                assert!(
                    (m.command && !m.alt) || (key == Key::U && none),
                    "{key:?} con {m:?} dio {action_2:?}"
                );
            }
            // `ColorNext` y `ColorPrev` nunca con `command`, `ctrl` ni `mac_cmd`.
            if matches!(action_2, Some(Action::ColorNext | Action::ColorPrev)) {
                assert!(
                    !(m.command || m.ctrl || m.mac_cmd),
                    "{key:?} con {m:?} dio {action_2:?}"
                );
            }
            // Las cifras ya no eligen ranura: `SelectColor` no sale de ninguna tecla.
            assert!(
                !matches!(action_2, Some(Action::SelectColor(_))),
                "{key:?} con {m:?} dio {action_2:?}"
            );

            // Y la tabla completa de la sección 5 como modelo.
            let expected_2 = if m.command && !m.alt && !m.shift && key == Key::Z {
                Some(Action::Undo)
            } else if m.command
                && !m.alt
                && ((key == Key::Y && !m.shift) || (key == Key::Z && m.shift))
            {
                Some(Action::Redo)
            } else if m.command || m.ctrl || m.mac_cmd {
                None
            } else if matches!(key, Key::PageDown | Key::PageUp) {
                // Teclas con nombre: Alt no cuenta; con Mayús (I.2a) son el color de fondo y
                // esta vista no lo conoce (`None`).
                if m.shift {
                    None
                } else if key == Key::PageDown {
                    Some(Action::ColorNext)
                } else {
                    Some(Action::ColorPrev)
                }
            } else if none {
                match key {
                    Key::U => Some(Action::Undo),
                    Key::CloseBracket => Some(Action::ColorNext),
                    Key::OpenBracket => Some(Action::ColorPrev),
                    _ => None,
                }
            } else {
                None
            };
            assert_eq!(action_2, expected_2, "{key:?} con {m:?}");
        }
    }
}
