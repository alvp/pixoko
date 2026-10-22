//! Tests de `confirm` para «Nueva imagen» (specs/hito-2-nueva-imagen.md, unidad 19, fila 10 y D6):
//! `Request::NewImage` y `new_warning()`. La regla de `new_gate` (en `dialogs`) está en
//! `dialogs/new_gate_tests.rs`.
//!
//! Suposiciones donde la spec no fija un detalle:
//! - `new_warning()` es exactamente el texto de D6 con los 5 s de `CONFIRM_SECS`: «hay cambios sin
//!   guardar: elige Nueva imagen otra vez en 5 s para descartarlos, o guarda con Ctrl+S». «Sin
//!   nombrar atajos de sistema» se lee como que no nombra `Ctrl+N` ni `Cmd+N` (sí nombra `Ctrl+S`,
//!   como los demás avisos).
//! - `Request::NewImage` es una variante sin datos y se compara como las demás (`Clone`, `Debug`,
//!   `PartialEq`, `Eq` ya los tiene `Request`).

use super::*;

use std::path::PathBuf;

use px_ui::text::INTERFACE_CHARSET;

const NOTICE: &str = "hay cambios sin guardar: elige Nueva imagen otra vez en 5 s para descartarlos, o guarda con Ctrl+S";

fn other_requests() -> Vec<Request> {
    vec![
        Request::Open(PathBuf::from("a")),
        Request::Close,
        Request::OpenDialog,
        Request::SaveFallback { as_copy: false },
        Request::SaveFallback { as_copy: true },
    ]
}

// ---------------------------------------------------------------------------------------
// Fila 10: el aviso.
// ---------------------------------------------------------------------------------------

#[test]
fn row_10_new_warning_says_new_image_and_the_five_seconds() {
    let text = new_warning();
    assert_eq!(text, NOTICE);
    assert!(text.contains("Nueva imagen"));
    assert!(text.contains(&format!("{CONFIRM_SECS:.0} s")));
    assert!(text.contains("Ctrl+S"), "dice cómo guardar");
}

#[test]
fn row_10_new_warning_does_not_name_system_shortcuts_and_is_in_the_character_set() {
    let text = new_warning();
    assert!(!text.contains("Ctrl+N"), "{text}");
    assert!(!text.contains("Cmd+N"), "{text}");
    for c in text.chars() {
        assert!(
            INTERFACE_CHARSET.contains(c),
            "{text:?} lleva {c:?}, que no está en INTERFACE_CHARSET"
        );
    }
}

// ---------------------------------------------------------------------------------------
// Fila 10: la petición se arma y se confirma como las demás.
// ---------------------------------------------------------------------------------------

#[test]
fn row_10_new_image_arms_and_confirms_with_the_same_request_and_empties() {
    let mut confirm = Confirm::new();
    assert_eq!(confirm.request(Request::NewImage, 0.0), Confirmation::Armed);
    assert_eq!(
        confirm.request(Request::NewImage, 4.9),
        Confirmation::Confirmed
    );
    // Al confirmar se vació: la siguiente vuelve a armar.
    assert_eq!(confirm.request(Request::NewImage, 5.0), Confirmation::Armed);
}

#[test]
fn row_10_a_at_exactly_five_seconds_the_armed_request_has_already_expired() {
    let mut confirm = Confirm::new();
    assert_eq!(confirm.request(Request::NewImage, 0.0), Confirmation::Armed);
    assert_eq!(
        confirm.request(Request::NewImage, CONFIRM_SECS),
        Confirmation::Armed
    );
}

#[test]
fn row_10_another_request_replaces_the_armed_one_and_new_image_does_not_confirm_the_others() {
    for other in other_requests() {
        // Armada otra; `NewImage` no la confirma y la sustituye.
        let mut confirm = Confirm::new();
        assert_eq!(confirm.request(other.clone(), 0.0), Confirmation::Armed);
        assert_eq!(
            confirm.request(Request::NewImage, 1.0),
            Confirmation::Armed,
            "{other:?} armada y luego NewImage"
        );
        assert_eq!(
            confirm.request(other.clone(), 2.0),
            Confirmation::Armed,
            "NewImage sustituyó a {other:?}"
        );

        // Armada `NewImage`; la otra no la confirma y la sustituye.
        let mut confirm = Confirm::new();
        assert_eq!(confirm.request(Request::NewImage, 0.0), Confirmation::Armed);
        assert_eq!(
            confirm.request(other.clone(), 1.0),
            Confirmation::Armed,
            "NewImage armada y luego {other:?}"
        );
        assert_eq!(
            confirm.request(Request::NewImage, 2.0),
            Confirmation::Armed,
            "{other:?} sustituyó a NewImage"
        );
    }
}

#[test]
fn row_10_clear_disarms_new_image() {
    let mut confirm = Confirm::new();
    assert_eq!(confirm.request(Request::NewImage, 0.0), Confirmation::Armed);
    confirm.clear();
    assert_eq!(confirm.request(Request::NewImage, 1.0), Confirmation::Armed);
}

#[test]
fn row_10_confirm_armed_accepts_new_image_only_within_the_deadline_and_does_not_arm() {
    let mut confirm = Confirm::new();
    // Sin nada armado: falso y no arma.
    assert!(!confirm.confirm_armed(&Request::NewImage, 0.0));
    assert_eq!(
        confirm.request(Request::NewImage, 0.5),
        Confirmation::Armed,
        "confirm_armed no armó nada"
    );
    // Armada: dentro del plazo sí (y se vacía); a los 5 s justos no.
    assert!(confirm.confirm_armed(&Request::NewImage, 5.4));
    assert!(!confirm.confirm_armed(&Request::NewImage, 5.5), "se vació");
    confirm.request(Request::NewImage, 10.0);
    assert!(!confirm.confirm_armed(&Request::NewImage, 10.0 + CONFIRM_SECS));
}
