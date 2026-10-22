//! Tests de `shortcuts::file_action_for_key` (specs/hito-2-abrir-guardar.md, fila 22).
//!
//! La función es pura y total: recibe una `egui::Key` y unos `egui::Modifiers` como datos, sin
//! `Context`. Además de la tabla de la fila 22, un recorrido exhaustivo (todas las teclas de
//! `Key::ALL` por las 32 combinaciones de los cinco booleanos) comprueba el modelo completo.

use egui::{Key, Modifiers};
use px_ui::shortcuts::{Action, FileAction, action_for_key, file_action_for_key};

/// Los cinco booleanos de `Modifiers`, en el orden de una máscara de 5 bits.
fn modifiers_2(mascara: u8) -> Modifiers {
    Modifiers {
        alt: mascara & 1 != 0,
        ctrl: mascara & 2 != 0,
        shift: mascara & 4 != 0,
        mac_cmd: mascara & 8 != 0,
        command: mascara & 16 != 0,
    }
}

/// Lo que llega en Linux y Windows: `ctrl` y `command` a la vez.
fn ctrl_command() -> Modifiers {
    Modifiers {
        ctrl: true,
        command: true,
        ..Modifiers::NONE
    }
}

/// Lo que llega en macOS: `mac_cmd` y `command` a la vez.
fn mac_command() -> Modifiers {
    Modifiers {
        mac_cmd: true,
        command: true,
        ..Modifiers::NONE
    }
}

// Comprobación en tiempo de compilación de los traits exigidos a `FileAction`.
fn assert_traits<T: Clone + Copy + std::fmt::Debug + PartialEq + Eq>() {}

#[test]
fn file_action_derives_the_requested_traits() {
    assert_traits::<FileAction>();
    assert_ne!(FileAction::Save, FileAction::SaveAs);
    let _ = format!("{:?}", FileAction::Save);
}

#[test]
fn row_22_command_s_saves() {
    // Fila 22: `S` con `COMMAND` guarda.
    assert_eq!(
        file_action_for_key(Key::S, Modifiers::COMMAND),
        Some(FileAction::Save)
    );
}

#[test]
fn row_22_command_shift_s_saves_as() {
    // Fila 22: `S` con `COMMAND` y `SHIFT` guarda como.
    assert_eq!(
        file_action_for_key(Key::S, Modifiers::COMMAND | Modifiers::SHIFT),
        Some(FileAction::SaveAs)
    );
}

#[test]
fn row_22_in_practice_command_arrives_together_with_ctrl_or_mac_cmd() {
    // Fila 22: `command` + `ctrl` a la vez (Linux) y `mac_cmd` + `command` (macOS).
    for m in [ctrl_command(), mac_command()] {
        assert_eq!(
            file_action_for_key(Key::S, m),
            Some(FileAction::Save),
            "{m:?}"
        );
        assert_eq!(
            file_action_for_key(Key::S, m | Modifiers::SHIFT),
            Some(FileAction::SaveAs),
            "{m:?}"
        );
        // Con Alt no hay atajo, aunque lleguen los dos modificadores.
        assert_eq!(
            file_action_for_key(Key::S, m | Modifiers::ALT),
            None,
            "{m:?}"
        );
        assert_eq!(
            file_action_for_key(Key::S, m | Modifiers::ALT | Modifiers::SHIFT),
            None,
            "{m:?}"
        );
    }
}

#[test]
fn row_22_without_command_there_is_no_shortcut() {
    // Fila 22: `S` sola, con `SHIFT` solo, con `ALT`.
    assert_eq!(file_action_for_key(Key::S, Modifiers::NONE), None);
    assert_eq!(file_action_for_key(Key::S, Modifiers::SHIFT), None);
    assert_eq!(file_action_for_key(Key::S, Modifiers::ALT), None);
    assert_eq!(
        file_action_for_key(Key::S, Modifiers::SHIFT | Modifiers::ALT),
        None
    );
}

#[test]
fn row_22_command_with_alt_is_not_a_shortcut() {
    // Fila 22: `COMMAND` + `ALT`; tampoco con Mayús.
    assert_eq!(
        file_action_for_key(Key::S, Modifiers::COMMAND | Modifiers::ALT),
        None
    );
    assert_eq!(
        file_action_for_key(
            Key::S,
            Modifiers::COMMAND | Modifiers::ALT | Modifiers::SHIFT
        ),
        None
    );
}

#[test]
fn row_22_on_macos_ctrl_alone_does_nothing() {
    // Fila 22: `ctrl` solo (macOS: `command` falso) no guarda, ni con Mayús.
    let ctrl = Modifiers {
        ctrl: true,
        ..Modifiers::NONE
    };
    assert!(!ctrl.command);
    assert_eq!(file_action_for_key(Key::S, ctrl), None);
    assert_eq!(file_action_for_key(Key::S, ctrl | Modifiers::SHIFT), None);
    // Tampoco `mac_cmd` sin `command`: lo que decide es `command`, que egui resuelve.
    let cmd_without_command = Modifiers {
        mac_cmd: true,
        ..Modifiers::NONE
    };
    assert_eq!(file_action_for_key(Key::S, cmd_without_command), None);
}

#[test]
fn row_22_other_keys_with_command_are_not_a_file_shortcut() {
    // Fila 22: `P` y `Z` con `COMMAND` dan `None`. `O` con `COMMAND` es `Open` desde la unidad 16
    // (`Ctrl+O` era «de la unidad 11»); con `Mayús` sigue sin ser atajo.
    for key in [Key::P, Key::Z] {
        assert_eq!(
            file_action_for_key(key, Modifiers::COMMAND),
            None,
            "{key:?}"
        );
        assert_eq!(
            file_action_for_key(key, Modifiers::COMMAND | Modifiers::SHIFT),
            None,
            "{key:?}"
        );
    }
    assert_eq!(
        file_action_for_key(Key::O, Modifiers::COMMAND),
        Some(FileAction::Open)
    );
    assert_eq!(
        file_action_for_key(Key::O, Modifiers::COMMAND | Modifiers::SHIFT),
        None
    );
}

#[test]
fn row_10_unit_16_command_o_opens_and_only_with_command_without_shift_or_alt() {
    // specs/hito-2-dialogos-archivo.md, fila 10: `Ctrl+O` (Linux y Windows) y `Cmd+O` (macOS)
    // abren; `Ctrl+Mayús+O`, `Ctrl+Alt+O` y `O` sin modificador siguen dando `None`.
    for command in [ctrl_command(), mac_command()] {
        assert_eq!(
            file_action_for_key(Key::O, command),
            Some(FileAction::Open),
            "{command:?}"
        );
        let with_shift = Modifiers {
            shift: true,
            ..command
        };
        let with_alt = Modifiers {
            alt: true,
            ..command
        };
        assert_eq!(
            file_action_for_key(Key::O, with_shift),
            None,
            "{with_shift:?}"
        );
        assert_eq!(file_action_for_key(Key::O, with_alt), None, "{with_alt:?}");
    }
    assert_eq!(file_action_for_key(Key::O, Modifiers::NONE), None);
    // En macOS, `Ctrl` solo (sin `command`) no es atajo.
    assert_eq!(file_action_for_key(Key::O, Modifiers::CTRL), None);
    // `O` no cambia lo que `action_for_key` hace con ella.
    assert_eq!(action_for_key(Key::O, Modifiers::COMMAND), None);
}

#[test]
fn row_22_action_for_key_does_not_know_the_s_key() {
    // Fila 22: `action_for_key` con `S` sigue dando `None` (su contrato no cambia).
    assert_eq!(action_for_key(Key::S, Modifiers::COMMAND), None);
    assert_eq!(
        action_for_key(Key::S, Modifiers::COMMAND | Modifiers::SHIFT),
        None
    );
    assert_eq!(action_for_key(Key::S, Modifiers::NONE), None);
    // Y los atajos de la unidad 6 no dan atajo de fichero.
    assert_eq!(
        action_for_key(Key::Z, Modifiers::COMMAND),
        Some(Action::Undo)
    );
    assert_eq!(file_action_for_key(Key::Z, Modifiers::COMMAND), None);
}

/// Recorrido exhaustivo: todas las teclas por las 32 combinaciones de modificadores.
/// Solo `S` con `command` y sin `alt` da `Save` o `SaveAs` (esta si y solo si hay `shift`), y solo
/// `O` con `command`, sin `shift` ni `alt`, da `Open` (unidad 16). Todo lo demás da `None`.
#[test]
fn row_22_walk_through_all_the_keys_and_combinations() {
    assert!(Key::ALL.len() > 100, "Key::ALL incluye todas las teclas");
    let (mut saved, mut open) = (0, 0);
    for key in Key::ALL.iter().copied() {
        for mascara in 0..32u8 {
            let m = modifiers_2(mascara);
            // No entra en pánico (la llamada ya lo comprueba).
            let action_2 = file_action_for_key(key, m);

            let expected_2 = if key == Key::S && m.command && !m.alt {
                Some(if m.shift {
                    FileAction::SaveAs
                } else {
                    FileAction::Save
                })
            } else if key == Key::O && m.command && !m.alt && !m.shift {
                Some(FileAction::Open)
            } else {
                None
            };
            assert_eq!(action_2, expected_2, "{key:?} con {m:?}");

            // Lo dicho de otra manera: `Some` implica command sin alt y la tecla de su acción.
            if let Some(a) = action_2 {
                assert!(m.command && !m.alt, "{key:?} con {m:?} dio {a:?}");
                match a {
                    FileAction::Open => {
                        open += 1;
                        assert_eq!(key, Key::O, "{key:?} con {m:?} dio {a:?}");
                        assert!(!m.shift, "{key:?} con {m:?} dio {a:?}");
                    }
                    FileAction::Save | FileAction::SaveAs => {
                        saved += 1;
                        assert_eq!(key, Key::S, "{key:?} con {m:?} dio {a:?}");
                        assert_eq!(a == FileAction::SaveAs, m.shift, "{key:?} con {m:?}");
                    }
                }
            }
        }
    }
    // Con S hay 8 combinaciones de (ctrl, mac_cmd, shift) con command y sin alt: 4 de Save y 4 de
    // SaveAs. Con O, 4 de (ctrl, mac_cmd) con command y sin alt ni shift: todas de Open.
    assert_eq!(saved, 8);
    assert_eq!(open, 4);
    assert_eq!(saved + open, 12);
}
