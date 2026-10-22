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
fn row_12_without_changes_it_closes_the_first_time() {
    let mut confirm = Confirm::new();
    assert!(close_allowed(false, &mut confirm, 0.0));
    assert!(close_allowed(false, &mut confirm, 0.1));
    assert!(close_allowed(false, &mut confirm, 1000.0));
}

#[test]
fn row_12_without_changes_it_does_not_arm_the_confirmation() {
    let mut confirm = Confirm::new();
    assert!(close_allowed(false, &mut confirm, 0.0));
    // Si ahora hubiera cambios, la primera petición sigue siendo la que arma.
    assert!(!close_allowed(true, &mut confirm, 1.0));
    assert!(close_allowed(true, &mut confirm, 2.0));
}

#[test]
fn row_12_with_changes_the_first_request_does_not_close_and_stays_armed() {
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
fn row_12_the_second_request_within_the_deadline_closes() {
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
fn row_12_after_the_deadline_it_asks_for_confirmation_again() {
    let mut confirm = Confirm::new();
    assert!(!close_allowed(true, &mut confirm, 0.0));
    // A los `CONFIRM_SECS` justos ya no confirma: arma de nuevo.
    assert!(!close_allowed(true, &mut confirm, CONFIRM_SECS));
    // Y la nueva confirmación cuenta desde ese instante.
    assert!(close_allowed(true, &mut confirm, CONFIRM_SECS + 1.0));
}

#[test]
fn row_12_two_requests_separated_by_more_than_confirm_secs_do_not_close() {
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
fn row_12_the_confirmation_is_spent_and_the_next_request_arms_again() {
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
fn row_12_another_armed_request_does_not_confirm_the_close_and_is_replaced() {
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
fn row_12_the_texts_and_the_deadline_of_the_confirmation_are_the_usual_ones() {
    assert_eq!(CONFIRM_SECS, 5.0);
    assert!(close_warning().contains("cierra otra vez en 5 s"));
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(256))]

    /// La regla es la de la `x` de la ventana: `Confirm::request(Request::Close, now)` con cambios y
    /// `true` sin ellos, sobre cualquier secuencia de instantes no decrecientes.
    #[test]
    fn row_12_close_allowed_follows_the_confirm_request_rule_with_close(
        modified: bool,
        steps_2 in prop::collection::vec(0.0f64..9.0, 1..12),
    ) {
        let mut real = Confirm::new();
        let mut model = Confirm::new();
        let mut now = 0.0;
        for step in steps_2 {
            now += step;
            let expected_2 = !modified || model.request(Request::Close, now) == Confirmation::Confirmed;
            prop_assert_eq!(
                close_allowed(modified, &mut real, now),
                expected_2,
                "modified {}, now {}", modified, now
            );
        }
    }

    /// Sin cambios, siempre `true`; con cambios, nunca `true` dos veces seguidas sin una petición
    /// armada entre medias.
    #[test]
    fn row_12_with_changes_it_never_closes_twice_in_a_row(
        steps_2 in prop::collection::vec(0.0f64..9.0, 2..12),
    ) {
        let mut confirm = Confirm::new();
        let mut now = 0.0;
        let mut previous = false;
        for step in steps_2 {
            now += step;
            let closes = close_allowed(true, &mut confirm, now);
            prop_assert!(!(previous && closes), "dos cierres seguidos en {}", now);
            previous = closes;
        }
    }
}
