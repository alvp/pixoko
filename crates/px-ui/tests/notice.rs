//! Tests de `Notice` (specs/hito-2-linea-relleno-app.md, sección 3 y fila 15): el aviso del título
//! caduca con una función pura que recibe el tiempo como parámetro, sin ventana.

use proptest::prelude::*;
use px_ui::notice::{NOTICE_SECS, Notice};

#[test]
fn el_aviso_dura_cinco_segundos() {
    assert_eq!(NOTICE_SECS, 5.0);
}

#[test]
fn fila_15_el_aviso_se_ve_hasta_cinco_segundos_despues_de_crearlo() {
    let aviso = Notice::new("x".into(), 10.0);
    assert_eq!(aviso.visible(10.0), Some("x"));
    assert_eq!(aviso.visible(14.9), Some("x"));
    assert_eq!(aviso.visible(15.0), None, "now < instante + 5: 15,0 ya no");
    assert_eq!(aviso.visible(100.0), None);
}

#[test]
fn fila_15_un_aviso_nuevo_sustituye_al_anterior() {
    let mut aviso = Some(Notice::new("x".into(), 10.0));
    assert_eq!(aviso.as_ref().and_then(|n| n.visible(14.0)), Some("x"));

    // Un segundo error a los 12 s: el primero habría caducado a los 15 s, el segundo a los 17.
    aviso = Some(Notice::new("y".into(), 12.0));
    assert_eq!(aviso.as_ref().and_then(|n| n.visible(15.0)), Some("y"));
    assert_eq!(aviso.as_ref().and_then(|n| n.visible(16.9)), Some("y"));
    assert_eq!(aviso.as_ref().and_then(|n| n.visible(17.0)), None);
}

#[test]
fn antes_de_crearlo_la_formula_de_la_spec_lo_da_por_visible() {
    // Spec: `Some` si `now < instante + NOTICE_SECS`. Un `now` anterior a la creación cumple eso.
    let aviso = Notice::new("x".into(), 10.0);
    assert_eq!(aviso.visible(9.0), Some("x"));
}

#[test]
fn el_texto_se_devuelve_tal_cual() {
    let aviso = Notice::new(
        "no hay memoria para guardar el paso en la historia".into(),
        0.0,
    );
    assert_eq!(
        aviso.visible(0.0),
        Some("no hay memoria para guardar el paso en la historia")
    );
    assert_eq!(Notice::new(String::new(), 0.0).visible(1.0), Some(""));
}

proptest! {
    /// `visible(now)` es `Some` si y solo si `now < creación + NOTICE_SECS`, con el texto dado.
    #[test]
    fn visible_sigue_la_formula_de_la_spec(
        texto in ".{0,20}",
        creado in 0.0f64..1.0e6,
        despues in -10.0f64..20.0,
    ) {
        let aviso = Notice::new(texto.clone(), creado);
        let ahora = creado + despues;
        let esperado = (ahora < creado + NOTICE_SECS).then_some(texto.as_str());
        prop_assert_eq!(aviso.visible(ahora), esperado);
    }
}
