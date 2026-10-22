//! Tests de `Notice` (specs/hito-2-linea-relleno-app.md, sección 3 y fila 15): el aviso del título
//! caduca con una función pura que recibe el tiempo como parámetro, sin ventana.

use proptest::prelude::*;
use px_ui::notice::{NOTICE_SECS, Notice};

#[test]
fn the_notice_lasts_five_seconds() {
    assert_eq!(NOTICE_SECS, 5.0);
}

#[test]
fn row_15_the_notice_is_seen_up_to_five_seconds_after_creating_it() {
    let notice = Notice::new("x".into(), 10.0);
    assert_eq!(notice.visible(10.0), Some("x"));
    assert_eq!(notice.visible(14.9), Some("x"));
    assert_eq!(notice.visible(15.0), None, "now < instante + 5: 15,0 ya no");
    assert_eq!(notice.visible(100.0), None);
}

#[test]
fn row_15_a_new_notice_replaces_the_previous_one() {
    let mut notice = Some(Notice::new("x".into(), 10.0));
    assert_eq!(notice.as_ref().and_then(|n| n.visible(14.0)), Some("x"));

    // Un segundo error a los 12 s: el primero habría caducado a los 15 s, el segundo a los 17.
    notice = Some(Notice::new("y".into(), 12.0));
    assert_eq!(notice.as_ref().and_then(|n| n.visible(15.0)), Some("y"));
    assert_eq!(notice.as_ref().and_then(|n| n.visible(16.9)), Some("y"));
    assert_eq!(notice.as_ref().and_then(|n| n.visible(17.0)), None);
}

#[test]
fn before_creating_it_the_spec_formula_gives_it_as_visible() {
    // Spec: `Some` si `now < instante + NOTICE_SECS`. Un `now` anterior a la creación cumple eso.
    let notice = Notice::new("x".into(), 10.0);
    assert_eq!(notice.visible(9.0), Some("x"));
}

#[test]
fn the_text_is_returned_as_is() {
    let notice = Notice::new(
        "no hay memoria para guardar el paso en la historia".into(),
        0.0,
    );
    assert_eq!(
        notice.visible(0.0),
        Some("no hay memoria para guardar el paso en la historia")
    );
    assert_eq!(Notice::new(String::new(), 0.0).visible(1.0), Some(""));
}

proptest! {
    /// `visible(now)` es `Some` si y solo si `now < creación + NOTICE_SECS`, con el texto dado.
    #[test]
    fn visible_follows_the_formula_of_the_spec(
        text_2 in ".{0,20}",
        created in 0.0f64..1.0e6,
        after_2 in -10.0f64..20.0,
    ) {
        let notice = Notice::new(text_2.clone(), created);
        let now = created + after_2;
        let expected_2 = (now < created + NOTICE_SECS).then_some(text_2.as_str());
        prop_assert_eq!(notice.visible(now), expected_2);
    }
}
