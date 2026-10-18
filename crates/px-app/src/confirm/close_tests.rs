//! Tests de `confirm::close_allowed` (specs/hito-2-menu-herramientas.md, «Casos» fila 12 y decisión
//! 10): la regla de cierre por repetición como función pura,
//! `close_allowed(modified, &mut Confirm, now) -> bool`, que usan `handle_close` (la `x` de la
//! ventana y `Alt`+`F4`) y `Archivo → Salir` por igual.
//!
//! Sin cambios: `true` a la primera. Con cambios sin guardar: la primera petición da `false` (queda
//! armada; `handle_close` deja el aviso `close_warning` y `CancelClose`, que necesitan un
//! `egui::Context` y se prueban a mano), la segunda dentro de `CONFIRM_SECS` da `true` y, pasado el
//! plazo, vuelve a dar `false`. Es la misma regla que `Confirm::request(Request::Close, now)`.
//!
//! Decisión donde la spec no fija un detalle: con el documento sin cambios `close_allowed` no
//! arma nada (como `handle_close`, que vuelve antes de pedir la confirmación).

use super::*;

use proptest::prelude::*;

fn open(name: &str) -> Request {
    Request::Open(std::path::PathBuf::from(name))
}

#[test]
fn fila_12_sin_cambios_cierra_a_la_primera() {
    let mut confirm = Confirm::new();
    assert!(close_allowed(false, &mut confirm, 0.0));
    assert!(close_allowed(false, &mut confirm, 0.1));
    assert!(close_allowed(false, &mut confirm, 1000.0));
}

#[test]
fn fila_12_sin_cambios_no_arma_la_confirmacion() {
    let mut confirm = Confirm::new();
    assert!(close_allowed(false, &mut confirm, 0.0));
    // Si ahora hubiera cambios, la primera petición sigue siendo la que arma.
    assert!(!close_allowed(true, &mut confirm, 1.0));
    assert!(close_allowed(true, &mut confirm, 2.0));
}

#[test]
fn fila_12_con_cambios_la_primera_peticion_no_cierra_y_queda_armada() {
    let mut confirm = Confirm::new();
    assert!(!close_allowed(true, &mut confirm, 0.0));
    // Queda armada: la misma petición de cierre por `Confirm` confirma.
    assert_eq!(
        confirm.request(Request::Close, 1.0),
        Confirmation::Confirmed,
        "la primera petición dejó armado `Request::Close`"
    );
}

#[test]
fn fila_12_la_segunda_peticion_dentro_del_plazo_cierra() {
    let mut confirm = Confirm::new();
    assert!(!close_allowed(true, &mut confirm, 0.0));
    assert!(
        close_allowed(true, &mut confirm, 0.0),
        "el mismo instante ya vale"
    );

    let mut confirm = Confirm::new();
    assert!(!close_allowed(true, &mut confirm, 10.0));
    assert!(close_allowed(true, &mut confirm, 10.0 + CONFIRM_SECS - 0.1));
}

#[test]
fn fila_12_pasado_el_plazo_vuelve_a_pedir_confirmacion() {
    let mut confirm = Confirm::new();
    assert!(!close_allowed(true, &mut confirm, 0.0));
    // A los `CONFIRM_SECS` justos ya no confirma: arma de nuevo.
    assert!(!close_allowed(true, &mut confirm, CONFIRM_SECS));
    // Y la nueva confirmación cuenta desde ese instante.
    assert!(close_allowed(true, &mut confirm, CONFIRM_SECS + 1.0));
}

#[test]
fn fila_12_dos_peticiones_separadas_por_mas_de_confirm_secs_no_cierran() {
    let mut confirm = Confirm::new();
    assert!(!close_allowed(true, &mut confirm, 0.0));
    assert!(!close_allowed(true, &mut confirm, CONFIRM_SECS + 1.0));
    assert!(!close_allowed(
        true,
        &mut confirm,
        2.0 * (CONFIRM_SECS + 1.0)
    ));
}

#[test]
fn fila_12_la_confirmacion_se_gasta_y_la_siguiente_peticion_vuelve_a_armar() {
    let mut confirm = Confirm::new();
    assert!(!close_allowed(true, &mut confirm, 0.0));
    assert!(close_allowed(true, &mut confirm, 1.0));
    assert!(
        !close_allowed(true, &mut confirm, 1.5),
        "la confirmación ya se gastó"
    );
    assert!(close_allowed(true, &mut confirm, 2.0));
}

#[test]
fn fila_12_otra_peticion_armada_no_confirma_el_cierre_y_se_sustituye() {
    let mut confirm = Confirm::new();
    assert_eq!(confirm.request(open("a.png"), 0.0), Confirmation::Armed);
    assert!(
        !close_allowed(true, &mut confirm, 1.0),
        "la armada era de abrir, no de cerrar"
    );
    // Cerrar sustituyó a abrir: abrir vuelve a armar.
    assert_eq!(confirm.request(open("a.png"), 2.0), Confirmation::Armed);
}

#[test]
fn fila_12_los_textos_y_el_plazo_de_la_confirmacion_son_los_de_siempre() {
    assert_eq!(CONFIRM_SECS, 5.0);
    assert!(close_warning().contains("cierra otra vez en 5 s"));
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(256))]

    /// La regla es la de la `x` de la ventana: `Confirm::request(Request::Close, now)` con cambios y
    /// `true` sin ellos, sobre cualquier secuencia de instantes no decrecientes.
    #[test]
    fn fila_12_close_allowed_sigue_la_regla_de_confirm_request_con_cierre(
        modified: bool,
        pasos in prop::collection::vec(0.0f64..9.0, 1..12),
    ) {
        let mut real = Confirm::new();
        let mut modelo = Confirm::new();
        let mut now = 0.0;
        for paso in pasos {
            now += paso;
            let esperado = !modified || modelo.request(Request::Close, now) == Confirmation::Confirmed;
            prop_assert_eq!(
                close_allowed(modified, &mut real, now),
                esperado,
                "modified {}, now {}", modified, now
            );
        }
    }

    /// Sin cambios, siempre `true`; con cambios, nunca `true` dos veces seguidas sin una petición
    /// armada entre medias.
    #[test]
    fn fila_12_con_cambios_nunca_cierra_dos_veces_seguidas(
        pasos in prop::collection::vec(0.0f64..9.0, 2..12),
    ) {
        let mut confirm = Confirm::new();
        let mut now = 0.0;
        let mut anterior = false;
        for paso in pasos {
            now += paso;
            let cierra = close_allowed(true, &mut confirm, now);
            prop_assert!(!(anterior && cierra), "dos cierres seguidos en {}", now);
            anterior = cierra;
        }
    }
}
