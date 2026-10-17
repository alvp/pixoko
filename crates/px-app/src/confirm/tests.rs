//! Tests de `confirm` (specs/hito-2-abrir-guardar.md): filas 18, 19 y 20 y propiedad 8.
//!
//! `Confirm` no mira el reloj: el tiempo lo da quien llama, en segundos. La confirmación es por
//! repetición: la segunda petición igual dentro de `CONFIRM_SECS` confirma.

use super::*;

use std::path::PathBuf;

use proptest::prelude::*;

use crate::session::*;

fn open(name: &str) -> Request {
    Request::Open(PathBuf::from(name))
}

// ---------------------------------------------------------------------------------------
// Fila 18.
// ---------------------------------------------------------------------------------------

#[test]
fn la_ventana_de_confirmacion_son_5_segundos() {
    assert_eq!(CONFIRM_SECS, 5.0);
}

#[test]
fn fila_18_armar_confirmar_y_vaciarse() {
    let mut confirm = Confirm::new();
    assert_eq!(confirm.request(open("a"), 0.0), Confirmation::Armed);
    assert_eq!(confirm.request(open("a"), 4.9), Confirmation::Confirmed);
    // Al confirmar se vació: la siguiente vuelve a armar.
    assert_eq!(confirm.request(open("a"), 5.0), Confirmation::Armed);
}

// ---------------------------------------------------------------------------------------
// Fila 19.
// ---------------------------------------------------------------------------------------

#[test]
fn fila_19_a_los_5_segundos_justos_ya_no_confirma() {
    let mut confirm = Confirm::new();
    assert_eq!(confirm.request(open("a"), 0.0), Confirmation::Armed);
    assert_eq!(confirm.request(open("a"), 5.0), Confirmation::Armed);
}

#[test]
fn fila_19_otra_ruta_vuelve_a_armar() {
    let mut confirm = Confirm::new();
    assert_eq!(confirm.request(open("a"), 0.0), Confirmation::Armed);
    assert_eq!(confirm.request(open("b"), 1.0), Confirmation::Armed);
}

#[test]
fn fila_19_otra_peticion_vuelve_a_armar() {
    let mut confirm = Confirm::new();
    assert_eq!(confirm.request(open("a"), 0.0), Confirmation::Armed);
    assert_eq!(confirm.request(Request::Close, 1.0), Confirmation::Armed);
    assert_eq!(confirm.request(open("a"), 2.0), Confirmation::Armed);
}

#[test]
fn fila_19_close_se_confirma_con_close() {
    let mut confirm = Confirm::new();
    assert_eq!(confirm.request(Request::Close, 0.0), Confirmation::Armed);
    assert_eq!(
        confirm.request(Request::Close, 4.9),
        Confirmation::Confirmed
    );
}

#[test]
fn fila_19_clear_entre_dos_peticiones() {
    let mut confirm = Confirm::new();
    assert_eq!(confirm.request(open("a"), 0.0), Confirmation::Armed);
    confirm.clear();
    assert_eq!(confirm.request(open("a"), 1.0), Confirmation::Armed);
}

#[test]
fn fila_19_el_tiempo_hacia_atras_no_confirma() {
    let mut confirm = Confirm::new();
    assert_eq!(confirm.request(open("a"), 10.0), Confirmation::Armed);
    assert_eq!(confirm.request(open("a"), 9.0), Confirmation::Armed);
}

#[test]
fn fila_19_una_peticion_que_arma_de_nuevo_cuenta_desde_su_instante() {
    // Supuesto: al rearmar, la ventana de 5 s cuenta desde la petición que rearma.
    let mut confirm = Confirm::new();
    assert_eq!(confirm.request(open("a"), 0.0), Confirmation::Armed);
    assert_eq!(confirm.request(open("b"), 3.0), Confirmation::Armed);
    assert_eq!(confirm.request(open("b"), 7.5), Confirmation::Confirmed);

    let mut confirm = Confirm::new();
    assert_eq!(confirm.request(open("a"), 0.0), Confirmation::Armed);
    assert_eq!(confirm.request(open("a"), 5.0), Confirmation::Armed);
    assert_eq!(confirm.request(open("a"), 9.5), Confirmation::Confirmed);
}

// ---------------------------------------------------------------------------------------
// Fila 20: textos de aviso (`describe_saved` vive en `session`).
// ---------------------------------------------------------------------------------------

#[test]
fn fila_20_avisos_de_confirmacion() {
    assert_eq!(
        open_warning("casa.png"),
        "hay cambios sin guardar: suelta casa.png otra vez en 5 s para descartarlos, o guarda con Ctrl+S"
    );
    assert_eq!(
        close_warning(),
        "hay cambios sin guardar: cierra otra vez en 5 s para descartarlos, o guarda con Ctrl+S"
    );
}

#[test]
fn fila_20_describe_saved() {
    let plain = Saved {
        path: PathBuf::from("/d/casa.png"),
        copy_reason: None,
    };
    assert_eq!(describe_saved(&plain), "guardado casa.png");

    let copy = Saved {
        path: PathBuf::from("/d/casa-copia.png"),
        copy_reason: Some("gris de 4 bits ampliado a 8".to_string()),
    };
    assert_eq!(
        describe_saved(&copy),
        "guardado como casa-copia.png: el original se convierte al abrir \
         (gris de 4 bits ampliado a 8) y no se sobrescribe"
    );
}

// ---------------------------------------------------------------------------------------
// Propiedad 8.
// ---------------------------------------------------------------------------------------

#[derive(Clone, Debug)]
enum Step {
    Request(Request, f64),
    Clear,
}

fn any_request() -> impl Strategy<Value = Request> {
    prop_oneof![Just(open("a")), Just(open("b")), Just(Request::Close),]
}

/// Secuencia con tiempos que crecen en múltiplos de 0,25 s (sumas exactas en `f64`), con
/// saltos de 0, de 5 s justos y de cualquier otro valor.
fn increasing_steps() -> impl Strategy<Value = Vec<Step>> {
    // `None` es un `Confirm::clear`.
    let step = (prop::option::weighted(0.8, any_request()), 0u32..40);
    prop::collection::vec(step, 0..40).prop_map(|raw| {
        let mut now = 0.0f64;
        raw.into_iter()
            .map(|(request, quarters)| {
                now += f64::from(quarters) * 0.25;
                match request {
                    Some(request) => Step::Request(request, now),
                    None => Step::Clear,
                }
            })
            .collect()
    })
}

proptest! {
    #[test]
    fn propiedad_8_confirmed_si_y_solo_si_repite_dentro_de_la_ventana(steps in increasing_steps()) {
        let mut confirm = Confirm::new();
        // Modelo: la última petición que armó y su instante.
        let mut armed: Option<(Request, f64)> = None;
        for step in steps {
            match step {
                Step::Clear => {
                    confirm.clear();
                    armed = None;
                }
                Step::Request(request, now) => {
                    let expected = match &armed {
                        Some((previous, at)) if *previous == request && *at <= now && now < *at + CONFIRM_SECS => {
                            Confirmation::Confirmed
                        }
                        _ => Confirmation::Armed,
                    };
                    let got = confirm.request(request.clone(), now);
                    prop_assert_eq!(&got, &expected, "{:?} a {}, armado {:?}", request, now, armed);
                    armed = match got {
                        Confirmation::Confirmed => None,
                        Confirmation::Armed => Some((request, now)),
                    };
                }
            }
        }
    }

    #[test]
    fn propiedad_8_con_tiempos_arbitrarios_no_entra_en_panico(
        steps in prop::collection::vec((any_request(), proptest::num::f64::ANY), 0..40)
    ) {
        let mut confirm = Confirm::new();
        let mut last = Confirmation::Armed;
        for (request, now) in steps {
            let was_confirmed = last == Confirmation::Confirmed;
            last = confirm.request(request.clone(), now);
            if was_confirmed {
                // Tras `Confirmed`, la siguiente petición es `Armed`.
                prop_assert_eq!(&last, &Confirmation::Armed);
            }
        }
    }
}
