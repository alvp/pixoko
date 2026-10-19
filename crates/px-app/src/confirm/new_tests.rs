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

const AVISO: &str = "hay cambios sin guardar: elige Nueva imagen otra vez en 5 s para descartarlos, o guarda con Ctrl+S";

fn otras_peticiones() -> Vec<Request> {
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
fn fila_10_new_warning_dice_nueva_imagen_y_los_cinco_segundos() {
    let text = new_warning();
    assert_eq!(text, AVISO);
    assert!(text.contains("Nueva imagen"));
    assert!(text.contains(&format!("{CONFIRM_SECS:.0} s")));
    assert!(text.contains("Ctrl+S"), "dice cómo guardar");
}

#[test]
fn fila_10_new_warning_no_nombra_atajos_de_sistema_y_esta_en_el_juego_de_caracteres() {
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
fn fila_10_nueva_imagen_se_arma_y_se_confirma_con_la_misma_peticion_y_se_vacia() {
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
fn fila_10_a_los_cinco_segundos_justos_la_peticion_armada_ya_caduco() {
    let mut confirm = Confirm::new();
    assert_eq!(confirm.request(Request::NewImage, 0.0), Confirmation::Armed);
    assert_eq!(
        confirm.request(Request::NewImage, CONFIRM_SECS),
        Confirmation::Armed
    );
}

#[test]
fn fila_10_otra_peticion_sustituye_a_la_armada_y_nueva_imagen_no_confirma_a_las_demas() {
    for otra in otras_peticiones() {
        // Armada otra; `NewImage` no la confirma y la sustituye.
        let mut confirm = Confirm::new();
        assert_eq!(confirm.request(otra.clone(), 0.0), Confirmation::Armed);
        assert_eq!(
            confirm.request(Request::NewImage, 1.0),
            Confirmation::Armed,
            "{otra:?} armada y luego NewImage"
        );
        assert_eq!(
            confirm.request(otra.clone(), 2.0),
            Confirmation::Armed,
            "NewImage sustituyó a {otra:?}"
        );

        // Armada `NewImage`; la otra no la confirma y la sustituye.
        let mut confirm = Confirm::new();
        assert_eq!(confirm.request(Request::NewImage, 0.0), Confirmation::Armed);
        assert_eq!(
            confirm.request(otra.clone(), 1.0),
            Confirmation::Armed,
            "NewImage armada y luego {otra:?}"
        );
        assert_eq!(
            confirm.request(Request::NewImage, 2.0),
            Confirmation::Armed,
            "{otra:?} sustituyó a NewImage"
        );
    }
}

#[test]
fn fila_10_clear_desarma_nueva_imagen() {
    let mut confirm = Confirm::new();
    assert_eq!(confirm.request(Request::NewImage, 0.0), Confirmation::Armed);
    confirm.clear();
    assert_eq!(confirm.request(Request::NewImage, 1.0), Confirmation::Armed);
}

#[test]
fn fila_10_confirm_armed_acepta_nueva_imagen_solo_dentro_del_plazo_y_no_arma() {
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
