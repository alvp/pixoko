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
fn panel_view_by_default_is_still_and_without_history() {
    let view = PanelView::default();
    assert_eq!(view.offset_px, 0);
    assert_eq!(view.rest_px, 0);
    assert_eq!(
        view.last_visible, None,
        "el primer fotograma no tiene anterior"
    );
    assert!(!view.reveal);
    assert_eq!(view, PanelView::default());
}

// Fila 4: `scroll_by`.

fn per(view: &mut PanelView, delta: i64, n: u32, max: i64) -> (i64, i64) {
    view.scroll_by(delta, n, max);
    (view.offset_px, view.rest_px)
}

#[test]
fn row_4_loose_deltas_with_n_3() {
    // Se empieza en el interior (offset 3) para no mezclar el resto con el extremo superior.
    let mut v = PanelView {
        offset_px: 3,
        ..PanelView::default()
    };
    let max = 30;
    // Menos de `n` se acumula en `rest_px` y no mueve nada.
    assert_eq!(per(&mut v, 1, 3, max), (3, 1));
    assert_eq!(per(&mut v, 1, 3, max), (3, 2));
    // Al completar `n` avanza un paso.
    assert_eq!(per(&mut v, 1, 3, max), (6, 0));
    // Varios pasos a la vez y un resto.
    assert_eq!(per(&mut v, 7, 3, max), (12, 1));
    // Hacia atrás: el resto se suma antes de dividir (1 − 2 = −1 → un paso atrás y resto 2).
    assert_eq!(per(&mut v, -2, 3, max), (9, 2));
    // Delta 0: sin cambios.
    let before = v;
    assert_eq!(per(&mut v, 0, 3, max), (9, 2));
    assert_eq!(v, before);
    // Mucho hacia arriba: se recorta a 0 y no queda resto.
    assert_eq!(per(&mut v, -100, 3, max), (0, 0));
}

/// Arriba el resto **sí se acumula** (los saltos de menos de `n` de un trackpad tienen que poder
/// salir de 0, a `ppp` 2); solo se suelta al recortar y en el máximo.
#[test]
fn row_4_at_the_upper_extreme_the_remainder_accumulates() {
    let mut v = PanelView::default();
    assert_eq!(per(&mut v, 1, 3, 30), (0, 1));
    assert_eq!(per(&mut v, 1, 3, 30), (0, 2));
    // El tercer píxel completa un paso.
    assert_eq!(per(&mut v, 1, 3, 30), (3, 0));
    // Volver con un delta que deja resto: la posición real es 2 px (offset 0 y resto 2).
    let mut v = PanelView {
        offset_px: 3,
        ..PanelView::default()
    };
    assert_eq!(per(&mut v, -1, 3, 30), (0, 2));
    // Pasarse de 0 hacia arriba recorta y suelta el resto.
    assert_eq!(per(&mut v, -3, 3, 30), (0, 0));
}

#[test]
fn row_4_with_n_1_there_is_no_remainder() {
    let mut v = PanelView::default();
    assert_eq!(per(&mut v, 7, 1, 100), (7, 0));
    assert_eq!(per(&mut v, -3, 1, 100), (4, 0));
}

#[test]
fn row_4_the_two_extremes_clip_and_leave_no_remainder() {
    let (n, max) = (3, 30);
    let mut v = PanelView::default();
    // Abajo del todo.
    assert_eq!(per(&mut v, 1_000_000_000_000, n, max), (max, 0));
    assert_eq!(per(&mut v, 1, n, max), (max, 0));
    assert_eq!(per(&mut v, 2, n, max), (max, 0));
    // Arriba del todo.
    assert_eq!(per(&mut v, -1_000_000_000_000, n, max), (0, 0));
    assert_eq!(per(&mut v, -1, n, max), (0, 0));
    // Extremos de `i64`: sin pánico ni desbordamiento.
    assert_eq!(per(&mut v, i64::MAX, n, max), (max, 0));
    assert_eq!(per(&mut v, i64::MIN, n, max), (0, 0));
}

#[test]
fn row_4_reaching_exactly_the_maximum_leaves_the_remainder_at_zero() {
    let (n, max) = (3, 30);
    let mut v = PanelView {
        offset_px: 27,
        rest_px: 0,
        ..PanelView::default()
    };
    // 27 + 5 = 32 > 30: se recorta.
    assert_eq!(per(&mut v, 5, n, max), (max, 0));
    // Con resto pendiente: 27 + 2 (resto) + 1 = 30 exacto.
    let mut v = PanelView {
        offset_px: 27,
        rest_px: 2,
        ..PanelView::default()
    };
    assert_eq!(per(&mut v, 1, n, max), (max, 0));
}

#[test]
fn row_4_with_maximum_zero_it_does_not_move() {
    let mut v = PanelView::default();
    assert_eq!(per(&mut v, 5, 3, 0), (0, 0));
    assert_eq!(per(&mut v, -5, 3, 0), (0, 0));
    assert_eq!(per(&mut v, i64::MAX, 3, 0), (0, 0));
}

#[test]
fn row_4_if_the_maximum_goes_down_the_scroll_is_clipped() {
    // La ventana crece entre dos fotogramas: `max` pasa de 30 a 6 y el desplazamiento antiguo
    // (27) no se puede mantener, ni con delta 0.
    let mut v = PanelView {
        offset_px: 27,
        rest_px: 1,
        ..PanelView::default()
    };
    assert_eq!(per(&mut v, 0, 3, 6), (6, 0));
}

#[test]
fn scroll_by_does_not_touch_last_visible_or_reveal() {
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
    fn scroll_by_follows_the_model(
        n in 1u32..=8,
        max_steps in 0i64..60,
        steps_2 in vec(deltas(), 0..30),
    ) {
        let n_i = i64::from(n);
        let max = max_steps * n_i;
        let mut v = PanelView::default();
        for delta in steps_2 {
            let total = i128::from(v.offset_px) + i128::from(v.rest_px) + i128::from(delta);
            v.scroll_by(delta, n, max);
            let (offset, rest_2) = if total <= 0 {
                (0, 0)
            } else if total >= i128::from(max) {
                (i128::from(max), 0)
            } else {
                let offset = total.div_euclid(i128::from(n)) * i128::from(n);
                // El resto se acumula también arriba; solo se suelta al recortar y en el máximo.
                (offset, total.rem_euclid(i128::from(n)))
            };
            prop_assert_eq!(i128::from(v.offset_px), offset, "delta {}", delta);
            prop_assert_eq!(i128::from(v.rest_px), rest_2, "delta {}", delta);
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
fn row_4_panel_thumb_without_scroll_is_none() {
    for n in [1u32, 2, 3] {
        // El bloque cabe (igual o menor que el panel): no hay barra.
        assert_eq!(panel_thumb(0, 241, 241, n), None);
        assert_eq!(panel_thumb(0, 300, 241, n), None);
        assert_eq!(panel_thumb(5, 700, 241, n), None);
    }
}

#[test]
fn row_4_panel_thumb_like_thumb_with_the_track_panel() {
    // Pista de 200 px, bloque de 241: alto = 200·200/241 = 165 (como `thumb`, en píxeles).
    assert_eq!(panel_thumb(0, 200, 241, 1), Some((0, 165)));
    // Con n = 3 el mismo cociente: 600·600/723 = 497.
    assert_eq!(panel_thumb(0, 600, 723, 3), Some((0, 497)));
}

#[test]
fn row_4_panel_thumb_has_a_minimum_height_of_2n() {
    for n in [1u32, 2, 3] {
        let (_, height_2) = panel_thumb(0, 20, 2000, n).expect("hay desplazamiento");
        assert_eq!(height_2, 2 * i64::from(n), "n = {n}");
        let (y, height_2) = panel_thumb(1980, 20, 2000, n).expect("hay desplazamiento");
        assert_eq!(height_2, 2 * i64::from(n));
        assert_eq!(
            y,
            20 - height_2,
            "al final el indicador llega al fondo de la pista"
        );
    }
}

#[test]
fn row_4_panel_thumb_goes_down_with_the_scroll_and_does_not_leave_the_track() {
    for n in [1u32, 2, 3] {
        let n_i = i64::from(n);
        let (panel, block) = (200 * n_i, 241 * n_i);
        let max = (block - panel).div_euclid(n_i) * n_i;
        let mut previous = -1;
        let mut offset = 0;
        while offset <= max {
            let (y, height_2) = panel_thumb(offset, panel, block, n).expect("hay desplazamiento");
            assert!(y >= 0, "n = {n}, offset {offset}");
            assert!(height_2 >= 2 * n_i, "n = {n}, offset {offset}");
            assert!(
                y + height_2 <= panel,
                "n = {n}, offset {offset}: se sale de la pista"
            );
            assert!(y >= previous, "n = {n}, offset {offset}: no baja");
            previous = y;
            offset += n_i;
        }
        // Arriba del todo, arriba; abajo del todo, más abajo.
        let (top, _) = panel_thumb(0, panel, block, n).expect("hay desplazamiento");
        let (below, _) = panel_thumb(max, panel, block, n).expect("hay desplazamiento");
        assert_eq!(top, 0);
        assert!(below > top);
    }
}

proptest! {
    #[test]
    fn panel_thumb_always_fits_in_the_track(
        n in 1u32..=8,
        panel in 1i64..4000,
        extra in 1i64..4000,
        offset_steps in 0i64..500,
    ) {
        let n_i = i64::from(n);
        let block = panel + extra;
        let max = (block - panel).div_euclid(n_i) * n_i;
        let offset = (offset_steps * n_i).min(max);
        let (y, height_2) = panel_thumb(offset, panel, block, n).expect("hay desplazamiento");
        // Con un alto mínimo mayor que la pista no cabe nada: solo se exige con pista suficiente.
        if panel >= 2 * n_i {
            prop_assert!(height_2 >= 2 * n_i);
            prop_assert!(y >= 0);
            prop_assert!(y + height_2 <= panel);
        }
    }
}

// Fila 11: `Scroll::reveal`.

fn with(first_row: usize, rest: i64) -> Scroll {
    Scroll { first_row, rest }
}

fn reveal(scroll: Scroll, row: usize, total: usize, visible: usize) -> Scroll {
    let mut s = scroll;
    s.reveal(row, total, visible);
    s
}

#[test]
fn row_11_reveal_a_row_above_raises_just_enough() {
    // 16 filas, 5 visibles: con la primera en 5 se ven 5..=9; la 2 pide subir hasta la 2.
    assert_eq!(reveal(with(5, 0), 2, 16, 5), with(2, 0));
    // Una sola por encima.
    assert_eq!(reveal(with(5, 0), 4, 16, 5), with(4, 0));
    // El resto de la fila siguiente se borra al moverse.
    assert_eq!(reveal(with(5, 9), 2, 16, 5), with(2, 0));
}

#[test]
fn row_11_reveal_a_row_below_lowers_just_enough() {
    // Con la primera en 0 se ven 0..=4; la 7 pide que la última visible sea la 7 (primera = 3).
    assert_eq!(reveal(with(0, 0), 7, 16, 5), with(3, 0));
    // Justo la siguiente a la última visible: baja una.
    assert_eq!(reveal(with(0, 0), 5, 16, 5), with(1, 0));
    assert_eq!(
        reveal(with(0, 11), 5, 16, 5),
        with(1, 0),
        "el resto se borra"
    );
}

#[test]
fn row_11_reveal_does_not_move_what_is_already_seen_or_touch_the_rest() {
    // Con la primera en 3 se ven 3..=7: las dos puntas y el medio.
    for row_2 in 3..=7 {
        assert_eq!(reveal(with(3, 9), row_2, 16, 5), with(3, 9), "fila {row_2}");
    }
}

#[test]
fn row_11_reveal_is_clipped_to_the_maximum() {
    // La última fila deja la primera en `total − visible`.
    assert_eq!(reveal(with(0, 0), 15, 16, 5), with(11, 0));
    // Con más filas visibles que totales no hay a dónde moverse.
    assert_eq!(reveal(with(0, 0), 3, 16, 20), with(0, 0));
    // Fila 0 desde abajo.
    assert_eq!(reveal(with(11, 0), 0, 16, 5), with(0, 0));
}

#[test]
fn row_11_reveal_with_a_visible_row_puts_it_on_top() {
    assert_eq!(reveal(with(0, 0), 7, 16, 1), with(7, 0));
    assert_eq!(reveal(with(9, 0), 2, 16, 1), with(2, 0));
    assert_eq!(
        reveal(with(7, 3), 7, 16, 1),
        with(7, 3),
        "ya visible: no toca el resto"
    );
    assert_eq!(reveal(with(0, 0), 15, 16, 1), with(15, 0));
}

#[test]
fn row_11_reveal_with_zero_visible_does_not_panic_or_leave_the_range() {
    for row in [0usize, 3, 15] {
        let s = reveal(with(2, 0), row, 16, 0);
        assert!(s.first_row <= max_first_row(16, 0), "fila {row}: {s:?}");
    }
}

#[test]
fn row_11_reveal_with_a_row_outside_the_palette_does_nothing() {
    for (row, total) in [(16usize, 16usize), (17, 16), (usize::MAX, 16), (0, 0)] {
        let initial = with(2, 5);
        assert_eq!(
            reveal(initial, row, total, 5),
            initial,
            "fila {row}, total {total}"
        );
    }
}

proptest! {
    #[test]
    fn reveal_leaves_the_row_visible_with_the_minimum_movement(
        total in 1usize..300,
        visible in 1usize..40,
        first in 0usize..300,
        row_2 in 0usize..300,
        rest_2 in 0i64..13,
    ) {
        prop_assume!(row_2 < total);
        let max = max_first_row(total, visible);
        prop_assume!(first <= max);
        let before = with(first, rest_2);
        let after_2 = reveal(before, row_2, total, visible);
        // La fila queda dentro de la ventana de filas visibles.
        prop_assert!(after_2.first_row <= row_2 && row_2 < after_2.first_row + visible);
        prop_assert!(after_2.first_row <= max);
        // Mínimo movimiento: si ya se veía, no se mueve; si no, la distancia es la justa.
        let already_visible = before.first_row <= row_2 && row_2 < before.first_row + visible;
        if already_visible {
            prop_assert_eq!(after_2, before);
        } else if row_2 < before.first_row {
            prop_assert_eq!(after_2.first_row, row_2);
            prop_assert_eq!(after_2.rest, 0);
        } else {
            prop_assert_eq!(after_2.first_row, row_2 + 1 - visible);
            prop_assert_eq!(after_2.rest, 0);
        }
    }
}
