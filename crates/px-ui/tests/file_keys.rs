//! Tests de `shortcuts::file_action_for_key` (specs/hito-2-abrir-guardar.md, fila 22).
//!
//! La función es pura y total: recibe una `egui::Key` y unos `egui::Modifiers` como datos, sin
//! `Context`. Además de la tabla de la fila 22, un recorrido exhaustivo (todas las teclas de
//! `Key::ALL` por las 32 combinaciones de los cinco booleanos) comprueba el modelo completo.

use egui::{Key, Modifiers};
use px_ui::shortcuts::{Action, FileAction, action_for_key, file_action_for_key};

/// Los cinco booleanos de `Modifiers`, en el orden de una máscara de 5 bits.
fn modificadores(mascara: u8) -> Modifiers {
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
fn file_action_deriva_los_traits_pedidos() {
    assert_traits::<FileAction>();
    assert_ne!(FileAction::Save, FileAction::SaveAs);
    let _ = format!("{:?}", FileAction::Save);
}

#[test]
fn fila_22_command_s_guarda() {
    // Fila 22: `S` con `COMMAND` guarda.
    assert_eq!(
        file_action_for_key(Key::S, Modifiers::COMMAND),
        Some(FileAction::Save)
    );
}

#[test]
fn fila_22_command_mayus_s_guarda_como() {
    // Fila 22: `S` con `COMMAND` y `SHIFT` guarda como.
    assert_eq!(
        file_action_for_key(Key::S, Modifiers::COMMAND | Modifiers::SHIFT),
        Some(FileAction::SaveAs)
    );
}

#[test]
fn fila_22_en_la_practica_command_llega_junto_a_ctrl_o_mac_cmd() {
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
fn fila_22_sin_command_no_hay_atajo() {
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
fn fila_22_command_con_alt_no_es_atajo() {
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
fn fila_22_en_macos_ctrl_solo_no_hace_nada() {
    // Fila 22: `ctrl` solo (macOS: `command` falso) no guarda, ni con Mayús.
    let ctrl = Modifiers {
        ctrl: true,
        ..Modifiers::NONE
    };
    assert!(!ctrl.command);
    assert_eq!(file_action_for_key(Key::S, ctrl), None);
    assert_eq!(file_action_for_key(Key::S, ctrl | Modifiers::SHIFT), None);
    // Tampoco `mac_cmd` sin `command`: lo que decide es `command`, que egui resuelve.
    let cmd_sin_command = Modifiers {
        mac_cmd: true,
        ..Modifiers::NONE
    };
    assert_eq!(file_action_for_key(Key::S, cmd_sin_command), None);
}

#[test]
fn fila_22_otras_teclas_con_command_no_son_atajo_de_fichero() {
    // Fila 22: `P`, `Z` y `O` con `COMMAND` dan `None` (`Ctrl+O` es de la unidad 11).
    for key in [Key::P, Key::Z, Key::O] {
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
}

#[test]
fn fila_22_action_for_key_no_conoce_la_tecla_s() {
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
/// Solo `S` con `command` y sin `alt` da `Some`, y es `SaveAs` si y solo si hay `shift`.
#[test]
fn fila_22_recorrido_de_todas_las_teclas_y_combinaciones() {
    assert!(Key::ALL.len() > 100, "Key::ALL incluye todas las teclas");
    let mut guardados = 0;
    for key in Key::ALL.iter().copied() {
        for mascara in 0..32u8 {
            let m = modificadores(mascara);
            // No entra en pánico (la llamada ya lo comprueba).
            let accion = file_action_for_key(key, m);

            let esperado = if key == Key::S && m.command && !m.alt {
                Some(if m.shift {
                    FileAction::SaveAs
                } else {
                    FileAction::Save
                })
            } else {
                None
            };
            assert_eq!(accion, esperado, "{key:?} con {m:?}");

            // Lo dicho de otra manera: `Some` implica S + command sin alt.
            if let Some(a) = accion {
                guardados += 1;
                assert_eq!(key, Key::S, "{key:?} con {m:?} dio {a:?}");
                assert!(m.command && !m.alt, "{key:?} con {m:?} dio {a:?}");
                assert_eq!(a == FileAction::SaveAs, m.shift, "{key:?} con {m:?}");
            }
        }
    }
    // Con S hay 8 combinaciones de (ctrl, mac_cmd, shift) con command y sin alt: 4 de Save y 4 de SaveAs.
    assert_eq!(guardados, 8);
}
