//! Tests de la parte pura del desplazamiento del panel con la ventana baja
//! (specs/hito-2-panel-ventana-baja.md, «B. Respaldo» y «Mostrar la elegida»): `PanelView`,
//! `PanelView::scroll_by`, `panel_thumb` y `Scroll::reveal`, sin tipos de `egui`.
//!
//! Cubre las filas 4 (`scroll_by` con deltas sueltos, `rest_px` y los dos extremos; `panel_thumb`)
//! y 11 (`Scroll::reveal`: arriba, abajo, ya visible, `row ≥ total`, `visible` 0 y 1, `rest`) en
//! su parte pura. Lo que se ve en la ventana (la rueda, la barra, el banco con `PanelView`) está
//! en `panel_baja.rs`.
//!
//! El modelo del desplazamiento usa `i128` y `div_euclid`, sin las fórmulas de la implementación:
//! con `total = offset + rest + delta`, dentro de `0..=max` el desplazamiento es
//! `(total div n)·n` y el resto `total mod n`; fuera, el extremo y resto 0.

use proptest::collection::vec;
use proptest::prelude::*;
use px_ui::palette_grid::{PanelView, Scroll, max_first_row, panel_thumb};

// `PanelView`.

#[test]
fn panel_view_por_defecto_esta_quieto_y_sin_historia() {
    let vista = PanelView::default();
    assert_eq!(vista.offset_px, 0);
    assert_eq!(vista.rest_px, 0);
    assert_eq!(
        vista.last_visible, None,
        "el primer fotograma no tiene anterior"
    );
    assert!(!vista.reveal);
    assert_eq!(vista, PanelView::default());
}

// Fila 4: `scroll_by`.

fn por(vista: &mut PanelView, delta: i64, n: u32, max: i64) -> (i64, i64) {
    vista.scroll_by(delta, n, max);
    (vista.offset_px, vista.rest_px)
}

#[test]
fn fila_4_deltas_sueltos_con_n_3() {
    // Se empieza en el interior (offset 3) para no mezclar el resto con el extremo superior.
    let mut v = PanelView {
        offset_px: 3,
        ..PanelView::default()
    };
    let max = 30;
    // Menos de `n` se acumula en `rest_px` y no mueve nada.
    assert_eq!(por(&mut v, 1, 3, max), (3, 1));
    assert_eq!(por(&mut v, 1, 3, max), (3, 2));
    // Al completar `n` avanza un paso.
    assert_eq!(por(&mut v, 1, 3, max), (6, 0));
    // Varios pasos a la vez y un resto.
    assert_eq!(por(&mut v, 7, 3, max), (12, 1));
    // Hacia atrás: el resto se suma antes de dividir (1 − 2 = −1 → un paso atrás y resto 2).
    assert_eq!(por(&mut v, -2, 3, max), (9, 2));
    // Delta 0: sin cambios.
    let antes = v;
    assert_eq!(por(&mut v, 0, 3, max), (9, 2));
    assert_eq!(v, antes);
    // Mucho hacia arriba: se recorta a 0 y no queda resto.
    assert_eq!(por(&mut v, -100, 3, max), (0, 0));
}

/// Arriba el resto **sí se acumula** (los saltos de menos de `n` de un trackpad tienen que poder
/// salir de 0, a `ppp` 2); solo se suelta al recortar y en el máximo.
#[test]
fn fila_4_en_el_extremo_superior_el_resto_se_acumula() {
    let mut v = PanelView::default();
    assert_eq!(por(&mut v, 1, 3, 30), (0, 1));
    assert_eq!(por(&mut v, 1, 3, 30), (0, 2));
    // El tercer píxel completa un paso.
    assert_eq!(por(&mut v, 1, 3, 30), (3, 0));
    // Volver con un delta que deja resto: la posición real es 2 px (offset 0 y resto 2).
    let mut v = PanelView {
        offset_px: 3,
        ..PanelView::default()
    };
    assert_eq!(por(&mut v, -1, 3, 30), (0, 2));
    // Pasarse de 0 hacia arriba recorta y suelta el resto.
    assert_eq!(por(&mut v, -3, 3, 30), (0, 0));
}

#[test]
fn fila_4_con_n_1_no_hay_resto() {
    let mut v = PanelView::default();
    assert_eq!(por(&mut v, 7, 1, 100), (7, 0));
    assert_eq!(por(&mut v, -3, 1, 100), (4, 0));
}

#[test]
fn fila_4_los_dos_extremos_recortan_y_no_dejan_resto() {
    let (n, max) = (3, 30);
    let mut v = PanelView::default();
    // Abajo del todo.
    assert_eq!(por(&mut v, 1_000_000_000_000, n, max), (max, 0));
    assert_eq!(por(&mut v, 1, n, max), (max, 0));
    assert_eq!(por(&mut v, 2, n, max), (max, 0));
    // Arriba del todo.
    assert_eq!(por(&mut v, -1_000_000_000_000, n, max), (0, 0));
    assert_eq!(por(&mut v, -1, n, max), (0, 0));
    // Extremos de `i64`: sin pánico ni desbordamiento.
    assert_eq!(por(&mut v, i64::MAX, n, max), (max, 0));
    assert_eq!(por(&mut v, i64::MIN, n, max), (0, 0));
}

#[test]
fn fila_4_llegar_justo_al_maximo_deja_el_resto_en_cero() {
    let (n, max) = (3, 30);
    let mut v = PanelView {
        offset_px: 27,
        rest_px: 0,
        ..PanelView::default()
    };
    // 27 + 5 = 32 > 30: se recorta.
    assert_eq!(por(&mut v, 5, n, max), (max, 0));
    // Con resto pendiente: 27 + 2 (resto) + 1 = 30 exacto.
    let mut v = PanelView {
        offset_px: 27,
        rest_px: 2,
        ..PanelView::default()
    };
    assert_eq!(por(&mut v, 1, n, max), (max, 0));
}

#[test]
fn fila_4_con_maximo_cero_no_se_mueve() {
    let mut v = PanelView::default();
    assert_eq!(por(&mut v, 5, 3, 0), (0, 0));
    assert_eq!(por(&mut v, -5, 3, 0), (0, 0));
    assert_eq!(por(&mut v, i64::MAX, 3, 0), (0, 0));
}

#[test]
fn fila_4_si_el_maximo_baja_el_desplazamiento_se_recorta() {
    // La ventana crece entre dos fotogramas: `max` pasa de 30 a 6 y el desplazamiento antiguo
    // (27) no se puede mantener, ni con delta 0.
    let mut v = PanelView {
        offset_px: 27,
        rest_px: 1,
        ..PanelView::default()
    };
    assert_eq!(por(&mut v, 0, 3, 6), (6, 0));
}

#[test]
fn scroll_by_no_toca_last_visible_ni_reveal() {
    let mut v = PanelView {
        last_visible: Some(4),
        reveal: true,
        ..PanelView::default()
    };
    v.scroll_by(10, 2, 100);
    assert_eq!(v.last_visible, Some(4));
    assert!(v.reveal);
}

fn deltas() -> impl Strategy<Value = i64> {
    prop_oneof![
        -40i64..40,
        -5000i64..5000,
        Just(i64::MIN),
        Just(i64::MAX),
        any::<i64>(),
    ]
}

proptest! {
    #[test]
    fn scroll_by_sigue_al_modelo(
        n in 1u32..=8,
        max_pasos in 0i64..60,
        pasos in vec(deltas(), 0..30),
    ) {
        let n_i = i64::from(n);
        let max = max_pasos * n_i;
        let mut v = PanelView::default();
        for delta in pasos {
            let total = i128::from(v.offset_px) + i128::from(v.rest_px) + i128::from(delta);
            v.scroll_by(delta, n, max);
            let (offset, resto) = if total <= 0 {
                (0, 0)
            } else if total >= i128::from(max) {
                (i128::from(max), 0)
            } else {
                let offset = total.div_euclid(i128::from(n)) * i128::from(n);
                // El resto se acumula también arriba; solo se suelta al recortar y en el máximo.
                (offset, total.rem_euclid(i128::from(n)))
            };
            prop_assert_eq!(i128::from(v.offset_px), offset, "delta {}", delta);
            prop_assert_eq!(i128::from(v.rest_px), resto, "delta {}", delta);
            prop_assert_eq!(v.offset_px % n_i, 0);
            prop_assert!(v.offset_px >= 0 && v.offset_px <= max);
            prop_assert!(v.rest_px >= 0 && v.rest_px < n_i);
            if v.offset_px == max {
                prop_assert_eq!(v.rest_px, 0);
            }
        }
    }
}

// Fila 4: `panel_thumb`.

#[test]
fn fila_4_panel_thumb_sin_desplazamiento_es_none() {
    for n in [1u32, 2, 3] {
        // El bloque cabe (igual o menor que el panel): no hay barra.
        assert_eq!(panel_thumb(0, 241, 241, n), None);
        assert_eq!(panel_thumb(0, 300, 241, n), None);
        assert_eq!(panel_thumb(5, 700, 241, n), None);
    }
}

#[test]
fn fila_4_panel_thumb_como_thumb_con_el_panel_de_pista() {
    // Pista de 200 px, bloque de 241: alto = 200·200/241 = 165 (como `thumb`, en píxeles).
    assert_eq!(panel_thumb(0, 200, 241, 1), Some((0, 165)));
    // Con n = 3 el mismo cociente: 600·600/723 = 497.
    assert_eq!(panel_thumb(0, 600, 723, 3), Some((0, 497)));
}

#[test]
fn fila_4_panel_thumb_tiene_alto_minimo_de_2n() {
    for n in [1u32, 2, 3] {
        let (_, alto) = panel_thumb(0, 20, 2000, n).expect("hay desplazamiento");
        assert_eq!(alto, 2 * i64::from(n), "n = {n}");
        let (y, alto) = panel_thumb(1980, 20, 2000, n).expect("hay desplazamiento");
        assert_eq!(alto, 2 * i64::from(n));
        assert_eq!(
            y,
            20 - alto,
            "al final el indicador llega al fondo de la pista"
        );
    }
}

#[test]
fn fila_4_panel_thumb_baja_con_el_desplazamiento_y_no_se_sale_de_la_pista() {
    for n in [1u32, 2, 3] {
        let n_i = i64::from(n);
        let (panel, bloque) = (200 * n_i, 241 * n_i);
        let max = (bloque - panel).div_euclid(n_i) * n_i;
        let mut anterior = -1;
        let mut offset = 0;
        while offset <= max {
            let (y, alto) = panel_thumb(offset, panel, bloque, n).expect("hay desplazamiento");
            assert!(y >= 0, "n = {n}, offset {offset}");
            assert!(alto >= 2 * n_i, "n = {n}, offset {offset}");
            assert!(
                y + alto <= panel,
                "n = {n}, offset {offset}: se sale de la pista"
            );
            assert!(y >= anterior, "n = {n}, offset {offset}: no baja");
            anterior = y;
            offset += n_i;
        }
        // Arriba del todo, arriba; abajo del todo, más abajo.
        let (arriba, _) = panel_thumb(0, panel, bloque, n).expect("hay desplazamiento");
        let (abajo, _) = panel_thumb(max, panel, bloque, n).expect("hay desplazamiento");
        assert_eq!(arriba, 0);
        assert!(abajo > arriba);
    }
}

proptest! {
    #[test]
    fn panel_thumb_siempre_cabe_en_la_pista(
        n in 1u32..=8,
        panel in 1i64..4000,
        extra in 1i64..4000,
        offset_pasos in 0i64..500,
    ) {
        let n_i = i64::from(n);
        let bloque = panel + extra;
        let max = (bloque - panel).div_euclid(n_i) * n_i;
        let offset = (offset_pasos * n_i).min(max);
        let (y, alto) = panel_thumb(offset, panel, bloque, n).expect("hay desplazamiento");
        // Con un alto mínimo mayor que la pista no cabe nada: solo se exige con pista suficiente.
        if panel >= 2 * n_i {
            prop_assert!(alto >= 2 * n_i);
            prop_assert!(y >= 0);
            prop_assert!(y + alto <= panel);
        }
    }
}

// Fila 11: `Scroll::reveal`.

fn con(first_row: usize, rest: i64) -> Scroll {
    Scroll { first_row, rest }
}

fn reveal(scroll: Scroll, row: usize, total: usize, visible: usize) -> Scroll {
    let mut s = scroll;
    s.reveal(row, total, visible);
    s
}

#[test]
fn fila_11_reveal_una_fila_por_encima_sube_lo_justo() {
    // 16 filas, 5 visibles: con la primera en 5 se ven 5..=9; la 2 pide subir hasta la 2.
    assert_eq!(reveal(con(5, 0), 2, 16, 5), con(2, 0));
    // Una sola por encima.
    assert_eq!(reveal(con(5, 0), 4, 16, 5), con(4, 0));
    // El resto de la fila siguiente se borra al moverse.
    assert_eq!(reveal(con(5, 9), 2, 16, 5), con(2, 0));
}

#[test]
fn fila_11_reveal_una_fila_por_debajo_baja_lo_justo() {
    // Con la primera en 0 se ven 0..=4; la 7 pide que la última visible sea la 7 (primera = 3).
    assert_eq!(reveal(con(0, 0), 7, 16, 5), con(3, 0));
    // Justo la siguiente a la última visible: baja una.
    assert_eq!(reveal(con(0, 0), 5, 16, 5), con(1, 0));
    assert_eq!(reveal(con(0, 11), 5, 16, 5), con(1, 0), "el resto se borra");
}

#[test]
fn fila_11_reveal_no_mueve_lo_que_ya_se_ve_ni_toca_el_resto() {
    // Con la primera en 3 se ven 3..=7: las dos puntas y el medio.
    for fila in 3..=7 {
        assert_eq!(reveal(con(3, 9), fila, 16, 5), con(3, 9), "fila {fila}");
    }
}

#[test]
fn fila_11_reveal_se_recorta_al_maximo() {
    // La última fila deja la primera en `total − visible`.
    assert_eq!(reveal(con(0, 0), 15, 16, 5), con(11, 0));
    // Con más filas visibles que totales no hay a dónde moverse.
    assert_eq!(reveal(con(0, 0), 3, 16, 20), con(0, 0));
    // Fila 0 desde abajo.
    assert_eq!(reveal(con(11, 0), 0, 16, 5), con(0, 0));
}

#[test]
fn fila_11_reveal_con_una_fila_visible_la_pone_arriba() {
    assert_eq!(reveal(con(0, 0), 7, 16, 1), con(7, 0));
    assert_eq!(reveal(con(9, 0), 2, 16, 1), con(2, 0));
    assert_eq!(
        reveal(con(7, 3), 7, 16, 1),
        con(7, 3),
        "ya visible: no toca el resto"
    );
    assert_eq!(reveal(con(0, 0), 15, 16, 1), con(15, 0));
}

#[test]
fn fila_11_reveal_con_cero_visibles_no_da_panico_ni_sale_del_rango() {
    for row in [0usize, 3, 15] {
        let s = reveal(con(2, 0), row, 16, 0);
        assert!(s.first_row <= max_first_row(16, 0), "fila {row}: {s:?}");
    }
}

#[test]
fn fila_11_reveal_con_una_fila_fuera_de_la_paleta_no_hace_nada() {
    for (row, total) in [(16usize, 16usize), (17, 16), (usize::MAX, 16), (0, 0)] {
        let inicial = con(2, 5);
        assert_eq!(
            reveal(inicial, row, total, 5),
            inicial,
            "fila {row}, total {total}"
        );
    }
}

proptest! {
    #[test]
    fn reveal_deja_la_fila_visible_con_el_minimo_movimiento(
        total in 1usize..300,
        visible in 1usize..40,
        primera in 0usize..300,
        fila in 0usize..300,
        resto in 0i64..13,
    ) {
        prop_assume!(fila < total);
        let max = max_first_row(total, visible);
        prop_assume!(primera <= max);
        let antes = con(primera, resto);
        let despues = reveal(antes, fila, total, visible);
        // La fila queda dentro de la ventana de filas visibles.
        prop_assert!(despues.first_row <= fila && fila < despues.first_row + visible);
        prop_assert!(despues.first_row <= max);
        // Mínimo movimiento: si ya se veía, no se mueve; si no, la distancia es la justa.
        let ya_visible = antes.first_row <= fila && fila < antes.first_row + visible;
        if ya_visible {
            prop_assert_eq!(despues, antes);
        } else if fila < antes.first_row {
            prop_assert_eq!(despues.first_row, fila);
            prop_assert_eq!(despues.rest, 0);
        } else {
            prop_assert_eq!(despues.first_row, fila + 1 - visible);
            prop_assert_eq!(despues.rest, 0);
        }
    }
}
