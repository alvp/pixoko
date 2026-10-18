//! Tests de `Confirm::confirm_armed` (specs/hito-2-dialogos-archivo.md, fila 9). `open_gate`
//! (fila 7) y los textos (fila 12) están en `dialogs/tests.rs`.
//!
//! `confirm_armed` sirve para la «segunda pulsación» sin armar la primera: `true` y vacía la
//! pendiente si es igual a la petición y `armada ≤ now < armada + 5`; si no, `false` y no
//! cambia nada (no arma).

use super::*;

use proptest::prelude::*;

fn fallback(as_copy: bool) -> Request {
    Request::SaveFallback { as_copy }
}

#[test]
fn fila_9_sin_armar_da_falso_y_no_arma() {
    let mut confirm = Confirm::new();
    assert!(!confirm.confirm_armed(&fallback(false), 0.0));
    // No armó: la primera `request` sigue siendo la que arma.
    assert_eq!(
        confirm.request(fallback(false), 1.0),
        Confirmation::Armed,
        "confirm_armed no debe armar"
    );
}

#[test]
fn fila_9_armada_y_dentro_de_5_s_da_verdadero_y_vacia() {
    let mut confirm = Confirm::new();
    assert_eq!(confirm.request(fallback(false), 0.0), Confirmation::Armed);

    assert!(confirm.confirm_armed(&fallback(false), 4.9));

    // Vaciada: ni una segunda confirmación ni una `request` ven la pendiente.
    assert!(!confirm.confirm_armed(&fallback(false), 4.95));
    assert_eq!(confirm.request(fallback(false), 4.96), Confirmation::Armed);
}

#[test]
fn fila_9_el_mismo_instante_en_que_se_armo_ya_vale() {
    let mut confirm = Confirm::new();
    assert_eq!(confirm.request(fallback(true), 3.0), Confirmation::Armed);
    assert!(confirm.confirm_armed(&fallback(true), 3.0));
}

#[test]
fn fila_9_a_los_5_segundos_justos_da_falso() {
    let mut confirm = Confirm::new();
    assert_eq!(confirm.request(fallback(false), 0.0), Confirmation::Armed);
    assert!(!confirm.confirm_armed(&fallback(false), 5.0));
    assert!(!confirm.confirm_armed(&fallback(false), 7.0));
}

#[test]
fn fila_9_un_instante_anterior_al_armado_da_falso() {
    let mut confirm = Confirm::new();
    assert_eq!(confirm.request(fallback(false), 3.0), Confirmation::Armed);
    assert!(!confirm.confirm_armed(&fallback(false), 2.0));
}

#[test]
fn fila_9_otra_clase_de_peticion_da_falso() {
    let mut confirm = Confirm::new();
    assert_eq!(confirm.request(fallback(true), 0.0), Confirmation::Armed);
    assert!(!confirm.confirm_armed(&fallback(false), 1.0));

    let mut confirm = Confirm::new();
    assert_eq!(confirm.request(Request::Close, 0.0), Confirmation::Armed);
    assert!(!confirm.confirm_armed(&Request::OpenDialog, 1.0));
    assert!(!confirm.confirm_armed(&fallback(false), 1.0));

    let mut confirm = Confirm::new();
    assert_eq!(
        confirm.request(Request::Open(PathBuf::from("a.png")), 0.0),
        Confirmation::Armed
    );
    assert!(!confirm.confirm_armed(&Request::Open(PathBuf::from("b.png")), 1.0));
    assert!(!confirm.confirm_armed(&Request::OpenDialog, 1.0));
}

#[test]
fn fila_9_un_falso_no_cambia_la_pendiente() {
    // Otra clase: la armada sigue ahí.
    let mut confirm = Confirm::new();
    assert_eq!(confirm.request(fallback(false), 0.0), Confirmation::Armed);
    assert!(!confirm.confirm_armed(&fallback(true), 1.0));
    assert!(confirm.confirm_armed(&fallback(false), 2.0));

    // Caducada (se pregunta con un instante posterior a 5 s): tampoco la borra ni la sustituye.
    let mut confirm = Confirm::new();
    assert_eq!(confirm.request(fallback(false), 0.0), Confirmation::Armed);
    assert!(!confirm.confirm_armed(&fallback(false), 6.0));
    assert!(confirm.confirm_armed(&fallback(false), 4.0));
}

#[test]
fn fila_9_funciona_con_cualquier_peticion_armada_por_request() {
    for request in [
        Request::Open(PathBuf::from("a.png")),
        Request::Close,
        Request::OpenDialog,
        fallback(false),
        fallback(true),
    ] {
        let mut confirm = Confirm::new();
        assert_eq!(confirm.request(request.clone(), 0.0), Confirmation::Armed);
        assert!(confirm.confirm_armed(&request, 1.0), "{request:?}");
        assert!(!confirm.confirm_armed(&request, 1.0), "{request:?}");
    }
}

#[test]
fn fila_9_armar_con_otra_peticion_sustituye_a_la_anterior() {
    let mut confirm = Confirm::new();
    assert_eq!(confirm.request(fallback(false), 0.0), Confirmation::Armed);
    assert_eq!(confirm.request(Request::Close, 1.0), Confirmation::Armed);
    assert!(!confirm.confirm_armed(&fallback(false), 2.0));
    assert!(confirm.confirm_armed(&Request::Close, 2.0));
}

#[test]
fn fila_9_clear_la_borra() {
    let mut confirm = Confirm::new();
    assert_eq!(confirm.request(fallback(false), 0.0), Confirmation::Armed);
    confirm.clear();
    assert!(!confirm.confirm_armed(&fallback(false), 1.0));
}

fn any_request() -> impl Strategy<Value = Request> {
    prop_oneof![
        Just(Request::Close),
        Just(Request::OpenDialog),
        any::<bool>().prop_map(|as_copy| Request::SaveFallback { as_copy }),
        "[a-c]".prop_map(|name| Request::Open(PathBuf::from(name))),
    ]
}

proptest! {
    /// `confirm_armed` coincide con `request` salvo en que no arma: confirma exactamente cuando
    /// `request` daría `Confirmed`, y en ese caso deja el mismo estado.
    #[test]
    fn fila_9_propiedad_confirm_armed_coincide_con_request(
        armed in any_request(),
        asked in any_request(),
        armed_at in 0.0f64..100.0,
        now in 0.0f64..110.0,
    ) {
        let mut base = Confirm::new();
        prop_assert_eq!(base.request(armed.clone(), armed_at), Confirmation::Armed);

        let mut by_request = base.clone();
        let expected = by_request.request(asked.clone(), now) == Confirmation::Confirmed;

        let mut by_armed = base.clone();
        prop_assert_eq!(by_armed.confirm_armed(&asked, now), expected);

        if expected {
            // Vaciada en los dos.
            prop_assert!(!by_armed.confirm_armed(&asked, now));
        } else {
            // No cambió nada: sigue pudiendo confirmar la que estaba armada.
            prop_assert!(by_armed.confirm_armed(&armed, armed_at));
        }
    }
}
