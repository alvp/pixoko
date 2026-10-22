//! Tests de `edit_layout`: la maqueta del editor de color, sin tipos de `egui`, y
//! `visible_rows_reserved` (specs/hito-2-editar-color.md, unidad 11c) y la parte pura de la
//! spec del panel con la ventana baja (specs/hito-2-panel-ventana-baja.md).
//!
//! Cubre la fila 3 de «Casos límite» de la spec del editor en su parte de maqueta
//! (`editor_height_u`, `visible_rows_reserved == visible_rows` con reserva 0, `slider_hit`,
//! `control_row_u`, `field_y_u`, `notice_y_u` y `edges_u`: todo borde en puntos × `ppp` es un
//! entero, ±0,001, a `ppp` 1,0, 1,25, 1,5, 2,0 y 3,0 con `n = text_scale(ppp)`). De la spec de la
//! ventana baja, en su parte pura: las filas 1 (`min_panel_height_px`, `min_window_height_pt`),
//! 2 (una sola función para la altura del editor: `editor_height_u`, `editor_height_u_max`,
//! `block_height_px`) y 3 (`panel_layout` con 305·n, 304·n, 200·n y 100·n). `editor_layout`,
//! `EditorLayout` y `EDITOR_HIDDEN_NOTICE` desaparecieron: la fila 18 de la spec del editor la
//! sustituye la spec de la ventana baja.
//!
//! Los números de la tabla «Geometría» están escritos como literales en un solo test
//! (`the_constants_are_worth_what_the_spec_says`); el resto usa las constantes de `edit_layout`,
//! que son el único sitio donde viven.

use proptest::prelude::*;
use px_core::Mode;
use px_ui::chrome::{MENU_H_U, toolbar_width_u};
use px_ui::color_edit::{Control, controls};
use px_ui::edit_layout::{
    BLOCK_GAP_U, BUTTON_COL_GAP_U, BUTTON_FULL_W_U, BUTTON_H_U, BUTTON_HALF_W_U, BUTTON_ROWS,
    BUTTON_TEXT_X_U, BUTTON_TEXT_Y_U, BUTTON_X_U, FIELD_H_U, FIELD_PAD_U, FIELD_W_U, FIELD_X_U,
    FIRST_ROW_Y_U, GROUP_GAP_U, HEADER_X_U, HEADER_Y_U, HIT_X0_U, HIT_X1_U, LABEL_W_U, LABEL_X_U,
    MIN_CANVAS_WIDTH_U, NOTICE_LINES, PaletteButton, PanelLayout, ROW_H_U, SEPARATOR_Y_U,
    THUMB_H_U, THUMB_W_U, TRACK_H_U, TRACK_W_U, TRACK_X_U, VALUE_W_U, VALUE_X_U, block_height_px,
    button_hit, button_rect_u, buttons_height_u, control_row_u, edges_u, editor_height_u,
    editor_height_u_max, field_y_u, icons_height_u, min_panel_height_px, min_window_height_pt,
    min_window_width_pt, min_window_width_u, notice_y_u, panel_layout, slider_hit,
};
use px_ui::palette_grid::{GRID_Y_U, PANEL_WIDTH_U, PITCH_U, visible_rows, visible_rows_reserved};
use px_ui::status::bar_height_px;
use px_ui::text::text_scale;

const TEST_MODES: [Mode; 4] = [Mode::Rgba, Mode::Indexed, Mode::Gray, Mode::GrayAlpha];
const SCALES: [u32; 3] = [1, 2, 3];
const PPPS: [f32; 5] = [1.0, 1.25, 1.5, 2.0, 3.0];

// Constantes.

#[test]
fn the_constants_are_worth_what_the_spec_says() {
    assert_eq!(SEPARATOR_Y_U, 0);
    assert_eq!(HEADER_X_U, 4);
    assert_eq!(HEADER_Y_U, 4);
    assert_eq!(ROW_H_U, 14);
    assert_eq!(FIRST_ROW_Y_U, 22);
    assert_eq!(GROUP_GAP_U, 4);
    assert_eq!(LABEL_X_U, 4);
    assert_eq!(LABEL_W_U, 7);
    assert_eq!(TRACK_X_U, 15);
    assert_eq!(TRACK_W_U, 184);
    assert_eq!(TRACK_H_U, 4);
    assert_eq!(THUMB_W_U, 3);
    assert_eq!(THUMB_H_U, 12);
    assert_eq!(VALUE_X_U, 203);
    assert_eq!(VALUE_W_U, 21);
    assert_eq!(HIT_X0_U, 13);
    assert_eq!(HIT_X1_U, 201);
    assert_eq!(FIELD_X_U, 4);
    assert_eq!(FIELD_W_U, 69);
    assert_eq!(FIELD_H_U, 18);
    assert_eq!(FIELD_PAD_U, 3);
    assert_eq!(BLOCK_GAP_U, 4);
    assert_eq!(NOTICE_LINES, 2);
}

#[test]
fn the_constants_are_consistent_with_each_other() {
    // `#` + 8 dígitos = 9 columnas de 7 u, más 3 de relleno a cada lado.
    assert_eq!(FIELD_W_U, 9 * 7 + 2 * FIELD_PAD_U);
    const {
        // La zona activa envuelve la pista (con 2 u de margen) y deja fuera la letra y el valor.
        assert!(HIT_X0_U < TRACK_X_U);
        assert!(HIT_X1_U > TRACK_X_U + TRACK_W_U);
        assert!(HIT_X1_U <= VALUE_X_U);
        // La pista y la marca caben en la fila.
        assert!(TRACK_H_U < ROW_H_U && THUMB_H_U < ROW_H_U);
        // Letra, pista y valor van de izquierda a derecha sin pisarse.
        assert!(LABEL_X_U + LABEL_W_U <= TRACK_X_U);
        assert!(TRACK_X_U + TRACK_W_U <= VALUE_X_U);
    }
}

// Alturas y filas.

#[test]
fn the_editor_height_depends_on_the_mode() {
    // Indexed: 186 as well; the icon row left the editor for the space under the grid (C1).
    assert_eq!(editor_height_u(Mode::Indexed), 186);
    assert_eq!(editor_height_u(Mode::Rgba), 186);
    assert_eq!(editor_height_u(Mode::GrayAlpha), 112);
    assert_eq!(editor_height_u(Mode::Gray), 94);
}

#[test]
fn the_slider_rows_of_indexed_and_rgba() {
    for mode_2 in [Mode::Indexed, Mode::Rgba] {
        let expected_2 = [
            (Control::Red, 22),
            (Control::Green, 36),
            (Control::Blue, 50),
            (Control::Hue, 68),
            (Control::Saturation, 82),
            (Control::Lightness, 96),
            (Control::Alpha, 114),
        ];
        for (control, row_2) in expected_2 {
            assert_eq!(
                control_row_u(mode_2, control),
                Some(row_2),
                "{mode_2:?} {control:?}"
            );
        }
        assert_eq!(control_row_u(mode_2, Control::Gray), None);
    }
}

#[test]
fn the_rows_are_compacted_without_gaps_in_gray_and_gray_alpha() {
    // Gray: solo «G», en la primera fila.
    assert_eq!(
        control_row_u(Mode::Gray, Control::Gray),
        Some(FIRST_ROW_Y_U)
    );
    for absent in [
        Control::Red,
        Control::Green,
        Control::Blue,
        Control::Hue,
        Control::Saturation,
        Control::Lightness,
        Control::Alpha,
    ] {
        assert_eq!(control_row_u(Mode::Gray, absent), None, "{absent:?}");
    }
    // GrayAlpha: «G» y «A» con el hueco de 4 u entre grupos; la altura 112 lo exige (94 más una
    // fila de 14 más el hueco de 4).
    assert_eq!(
        control_row_u(Mode::GrayAlpha, Control::Gray),
        Some(FIRST_ROW_Y_U)
    );
    assert_eq!(
        control_row_u(Mode::GrayAlpha, Control::Alpha),
        Some(FIRST_ROW_Y_U + ROW_H_U + GROUP_GAP_U)
    );
    for absent in [
        Control::Red,
        Control::Green,
        Control::Blue,
        Control::Hue,
        Control::Saturation,
        Control::Lightness,
    ] {
        assert_eq!(control_row_u(Mode::GrayAlpha, absent), None, "{absent:?}");
    }
}

#[test]
fn the_controls_of_each_mode_have_a_row_and_go_from_top_to_bottom() {
    for mode_2 in TEST_MODES {
        let mut previous = None;
        for control in controls(mode_2) {
            let row_2 = control_row_u(mode_2, *control).expect("cada control tiene fila");
            if let Some(previous) = previous {
                assert!(row_2 >= previous + ROW_H_U, "{mode_2:?} {control:?}");
            }
            previous = Some(row_2);
        }
    }
}

#[test]
fn the_field_and_the_notice_go_after_the_controls() {
    // Tras el último control, 4 u; tras el campo, 4 u; el aviso son dos líneas y 4 u de margen.
    let expected_2 = [
        (Mode::Indexed, 132, 154),
        (Mode::Rgba, 132, 154),
        (Mode::GrayAlpha, 58, 80),
        (Mode::Gray, 40, 62),
    ];
    for (mode_2, field_2, notice) in expected_2 {
        assert_eq!(field_y_u(mode_2), field_2, "{mode_2:?}");
        assert_eq!(notice_y_u(mode_2), notice, "{mode_2:?}");
    }
}

#[test]
fn the_mockup_is_deduced_from_the_constants() {
    for mode_2 in TEST_MODES {
        let last_2 = controls(mode_2)
            .iter()
            .filter_map(|c| control_row_u(mode_2, *c))
            .max()
            .expect("hay controles");
        assert_eq!(
            field_y_u(mode_2),
            last_2 + ROW_H_U + BLOCK_GAP_U,
            "{mode_2:?}"
        );
        assert_eq!(
            notice_y_u(mode_2),
            field_y_u(mode_2) + FIELD_H_U + BLOCK_GAP_U,
            "{mode_2:?}"
        );
        assert_eq!(
            editor_height_u(mode_2),
            notice_y_u(mode_2) + NOTICE_LINES * ROW_H_U + BLOCK_GAP_U,
            "{mode_2:?}"
        );
    }
}

// `editor_height_u_max`, `block_height_px`, `min_panel_height_px` y `min_window_height_pt`.

/// The block `[band · grid of 1 row · icons · editor]` worked out by hand from the constants.
fn block_px(mode_2: Mode, n: i64) -> i64 {
    (GRID_Y_U + PITCH_U + icons_height_u(mode_2) + editor_height_u(mode_2)) * n
}

#[test]
fn editor_height_u_max_is_the_largest_of_the_modes() {
    let greater = TEST_MODES
        .iter()
        .map(|m| editor_height_u(*m))
        .max()
        .expect("hay modos");
    assert_eq!(editor_height_u_max(), greater);
    assert_eq!(editor_height_u_max(), 186);
}

#[test]
fn block_height_px_is_text_one_row_and_editor() {
    for n in SCALES {
        let n_i = i64::from(n);
        for mode_2 in TEST_MODES {
            assert_eq!(
                block_height_px(mode_2, n),
                block_px(mode_2, n_i),
                "{mode_2:?}, n = {n}"
            );
        }
        // With the 44 u band and 5 u of separator and gap (C1) and the icon row (28 u) in indexed
        // images: 277, 249, 175 and 157 u.
        assert_eq!(block_height_px(Mode::Indexed, n), 277 * n_i);
        assert_eq!(block_height_px(Mode::Rgba, n), 249 * n_i);
        assert_eq!(block_height_px(Mode::GrayAlpha, n), 175 * n_i);
        assert_eq!(block_height_px(Mode::Gray, n), 157 * n_i);
    }
}

// Fila 1 (parte pura de `edit_layout`).

#[test]
fn row_1_min_panel_height_px_is_277n() {
    for n in [1u32, 2, 3] {
        let n_i = i64::from(n);
        assert_eq!(min_panel_height_px(n), 277 * n_i, "n = {n}");
        // Sin literales: una fila, el texto y el editor del modo de mayor editor.
        assert_eq!(
            min_panel_height_px(n),
            (GRID_Y_U + PITCH_U + icons_height_u(Mode::Indexed) + editor_height_u_max()) * n_i
        );
    }
}

#[test]
fn row_1_the_minimum_covers_the_block_of_all_the_modes() {
    // Abrir una imagen Gray no cambia el mínimo: es el del mayor editor, no el del modo actual.
    for n in SCALES {
        for mode_2 in TEST_MODES {
            assert!(
                block_height_px(mode_2, n) <= min_panel_height_px(n),
                "{mode_2:?}, n = {n}"
            );
        }
        assert_eq!(
            block_height_px(Mode::Indexed, n),
            min_panel_height_px(n),
            "n = {n}"
        );
        assert!(block_height_px(Mode::Gray, n) < min_panel_height_px(n));
    }
}

#[test]
fn row_1_min_window_height_pt_at_the_five_ppp() {
    let expected_2 = [
        (1.0f32, 313.0f32),
        (1.25, 250.4),
        (1.5, 417.33),
        (2.0, 313.0),
        (3.0, 313.0),
    ];
    for (ppp, points) in expected_2 {
        let measured = min_window_height_pt(ppp);
        assert!(
            (measured - points).abs() <= 0.01,
            "ppp {ppp}: {measured} pt y se esperaban {points}"
        );
        // Barra de estado, menú y panel mínimo, en píxeles físicos, entre `ppp`.
        let n = text_scale(ppp);
        let formula =
            (i64::from(bar_height_px(n)) + MENU_H_U * i64::from(n) + min_panel_height_px(n)) as f32
                / ppp;
        assert!(
            (measured - formula).abs() <= 0.001,
            "ppp {ppp}: {measured} pt y la fórmula da {formula}"
        );
    }
}

#[test]
fn row_1_the_minimum_does_not_pass_60_percent_of_720_and_leaves_one_row_with_the_editor() {
    let mut greater = 0.0f32;
    for ppp in PPPS {
        let n = text_scale(ppp);
        let minimum_2 = min_window_height_pt(ppp);
        greater = greater.max(minimum_2);
        assert!(
            minimum_2 >= min_panel_height_px(n) as f32 / ppp,
            "ppp {ppp}: {minimum_2} pt no deja sitio a una fila y al editor"
        );
        // Y la barra de estado.
        assert!(minimum_2 > min_panel_height_px(n) as f32 / ppp, "ppp {ppp}");
    }
    // The 60 % of 720 points (C1; it was 65 % since I.4 withdrew the 72 % exception of I.2a).
    assert!(greater <= 432.0, "el mayor de los cinco mide {greater} pt");
}

// Fila 1b: ancho mínimo (columna de herramientas, ancho del panel y 128 u de lienzo).

#[test]
fn row_1b_min_window_width_u_is_the_column_plus_the_panel_plus_128() {
    assert_eq!(MIN_CANVAS_WIDTH_U, 128);
    assert_eq!(PANEL_WIDTH_U, 233);
    assert_eq!(toolbar_width_u(), 107);
    assert_eq!(min_window_width_u(), 468);
    assert_eq!(
        min_window_width_u(),
        toolbar_width_u() + PANEL_WIDTH_U + MIN_CANVAS_WIDTH_U
    );
}

#[test]
fn row_1b_min_window_width_pt_at_the_five_ppp() {
    let expected_2 = [
        (1.0f32, 468.0f32),
        (1.25, 374.4),
        (1.5, 624.0),
        (2.0, 468.0),
        (3.0, 468.0),
    ];
    for (ppp, points) in expected_2 {
        let measured = min_window_width_pt(ppp);
        assert!(
            (measured - points).abs() <= 0.01,
            "ppp {ppp}: {measured} pt y se esperaban {points}"
        );
        let n = text_scale(ppp) as f32;
        let formula = min_window_width_u() as f32 * n / ppp;
        assert!(
            (measured - formula).abs() <= 0.001,
            "ppp {ppp}: {measured} pt y la fórmula da {formula}"
        );
    }
}

#[test]
fn row_1b_the_minimum_width_leaves_128_u_of_canvas_and_fits_in_half_of_1366() {
    for ppp in PPPS {
        let n = text_scale(ppp) as f32;
        let minimum_2 = min_window_width_pt(ppp);
        let panel = PANEL_WIDTH_U as f32 * n / ppp;
        let column = toolbar_width_u() as f32 * n / ppp;
        assert!(
            minimum_2 > panel + column,
            "ppp {ppp}: {minimum_2} pt no deja lienzo"
        );
        assert!(
            (minimum_2 - column - panel - MIN_CANVAS_WIDTH_U as f32 * n / ppp).abs() <= 0.001,
            "ppp {ppp}: el lienzo mínimo no mide 128 u"
        );
        assert!(minimum_2 <= 683.0, "ppp {ppp}: {minimum_2} pt");
    }
}

#[test]
fn row_1b_min_window_width_pt_with_an_invalid_ppp_is_that_of_1() {
    for ppp in [0.0f32, -1.0, f32::NAN, f32::INFINITY, f32::NEG_INFINITY] {
        let measured = min_window_width_pt(ppp);
        assert!(
            (measured - 468.0).abs() <= 0.001,
            "ppp {ppp}: {measured} pt y se esperaban 468"
        );
    }
}

// Fila 2: una sola función para la altura del editor.

#[test]
fn row_2_editor_height_u_matches_the_derivation_of_the_mockup() {
    let expected_2 = [
        (Mode::Indexed, 186),
        (Mode::Rgba, 186),
        (Mode::GrayAlpha, 112),
        (Mode::Gray, 94),
    ];
    for (mode_2, height_2_2) in expected_2 {
        let derived = notice_y_u(mode_2) + NOTICE_LINES * ROW_H_U + BLOCK_GAP_U;
        assert_eq!(editor_height_u(mode_2), derived, "{mode_2:?}");
        assert_eq!(editor_height_u(mode_2), height_2_2, "{mode_2:?}");
    }
}

/// No se puede simular de verdad una altura de botones de 20 u sin tocar la maqueta; lo que sí se
/// comprueba es que todo lo que depende de la altura del editor es función **solo** de
/// `editor_height_u`: dos modos con la misma altura dan lo mismo, y entre dos modos con alturas
/// distintas la diferencia de cada cantidad derivada es exactamente la diferencia de alturas por
/// `n`. Así, 20 u más en `editor_height_u` mueven `block_height_px`, el mínimo y `panel_layout` en
/// la misma cuantía y sin tocar nada más.
#[test]
fn row_2_what_is_derived_depends_only_on_editor_height_u() {
    for n in SCALES {
        let n_i = i64::from(n);
        // Everything derives from `editor_height_u` and `icons_height_u`: indexed and RGBA editors
        // are equal and indexed only adds the icon row, so the block and the minimum differ in
        // `icons · n` and the `panel_layout` of indexed with `icons · n` more pixels of height is
        // the one of RGBA with the editor `icons · n` lower.
        let buttons = icons_height_u(Mode::Indexed);
        assert_eq!(icons_height_u(Mode::Rgba), 0);
        assert_eq!(
            editor_height_u(Mode::Indexed) - editor_height_u(Mode::Rgba),
            0
        );
        assert_eq!(
            block_height_px(Mode::Indexed, n) - block_height_px(Mode::Rgba, n),
            buttons * n_i
        );
        for height_2 in [0, 100 * n_i, 200 * n_i, 241 * n_i, 700 * n_i] {
            for requested in [0, n_i, 50 * n_i, 10_000] {
                let indexed = panel_layout(Mode::Indexed, height_2 + buttons * n_i, n, requested);
                let rgba = panel_layout(Mode::Rgba, height_2, n, requested);
                assert_eq!(
                    PanelLayout {
                        top_px: indexed.top_px - buttons * n_i,
                        ..indexed
                    },
                    rgba,
                    "alto {height_2}, pedido {requested}"
                );
            }
        }
        // Different heights: the difference is `Δh · n` (icon row and editor).
        let tail = |m: Mode| icons_height_u(m) + editor_height_u(m);
        for a in TEST_MODES {
            for b in TEST_MODES {
                let delta = (tail(a) - tail(b)) * n_i;
                assert_eq!(
                    block_height_px(a, n) - block_height_px(b, n),
                    delta,
                    "{a:?} frente a {b:?}"
                );
            }
            // The minimum is the block of the tallest icon row plus editor: its difference with the
            // block of `a` is what `a` lacks to reach it.
            assert_eq!(
                min_panel_height_px(n) - block_height_px(a, n),
                (tail(Mode::Indexed) - tail(a)) * n_i,
                "{a:?}"
            );
            // `panel_layout` cambia de régimen exactamente en el bloque de cada modo.
            let block = block_height_px(a, n);
            assert!(!panel_layout(a, block, n, 0).scrolling, "{a:?}");
            assert!(panel_layout(a, block - 1, n, 0).scrolling, "{a:?}");
            // And the editor, anchored to the bottom, always starts `editor_height_u · n` above.
            let footer = panel_layout(a, block, n, 0);
            assert_eq!(footer.top_px, block - editor_height_u(a) * n_i, "{a:?}");
        }
    }
}

// `panel_layout`.

/// `PanelLayout` del diseño normal (sin desplazamiento) con `height_2` ≥ bloque.
fn normal(mode_2: Mode, height_2: i64, n: u32) -> PanelLayout {
    let n_i = i64::from(n);
    let top = (height_2 - editor_height_u(mode_2) * n_i).div_euclid(n_i) * n_i;
    let icons_top = top - icons_height_u(mode_2) * n_i;
    PanelLayout {
        scrolling: false,
        offset_px: 0,
        max_offset_px: 0,
        top_px: top,
        icons_top_px: icons_top,
        visible_rows: visible_rows_reserved(height_2, height_2 - icons_top, n),
    }
}

#[test]
fn with_height_700n_the_editor_anchors_to_the_bottom() {
    for n in SCALES {
        let n_i = i64::from(n);
        let expected_2 = [
            (Mode::Indexed, 514),
            (Mode::Rgba, 514),
            (Mode::GrayAlpha, 588),
            (Mode::Gray, 606),
        ];
        for (mode_2, top) in expected_2 {
            let layout = panel_layout(mode_2, 700 * n_i, n, 0);
            assert!(!layout.scrolling, "{mode_2:?}, n = {n}");
            assert_eq!(layout.offset_px, 0, "{mode_2:?}, n = {n}");
            assert_eq!(layout.top_px, top * n_i, "{mode_2:?}, n = {n}");
            assert_eq!(
                layout.visible_rows,
                visible_rows_reserved(700 * n_i, 700 * n_i - layout.icons_top_px, n),
                "{mode_2:?}, n = {n}"
            );
        }
        // The icon row (28 u) is right above the editor, in indexed images only.
        assert_eq!(
            panel_layout(Mode::Indexed, 700 * n_i, n, 0).icons_top_px,
            486 * n_i
        );
        assert_eq!(
            panel_layout(Mode::Rgba, 700 * n_i, n, 0).icons_top_px,
            514 * n_i
        );
        // 700n with 214n reserved (28 of icons and 186 of editor): (700 − 214 − 49) / 14 = 31 rows.
        assert_eq!(
            panel_layout(Mode::Indexed, 700 * n_i, n, 0).visible_rows,
            31
        );
        assert_eq!(visible_rows_reserved(700 * n_i, 214 * n_i, n), 31);
    }
}

#[test]
fn in_the_normal_layout_the_requested_scroll_is_ignored() {
    for n in SCALES {
        let n_i = i64::from(n);
        for mode_2 in TEST_MODES {
            let height_2 = 700 * n_i;
            for requested in [0, 1, n_i, 1000, -5] {
                let layout = panel_layout(mode_2, height_2, n, requested);
                assert!(!layout.scrolling, "{mode_2:?}, pedido {requested}");
                assert_eq!(layout.offset_px, 0, "{mode_2:?}, pedido {requested}");
                assert_eq!(layout.top_px, normal(mode_2, height_2, n).top_px);
            }
        }
    }
}

#[test]
fn top_px_is_always_a_multiple_of_n_even_if_the_height_is_not() {
    // La spec propone `700n + 3` con n = 3, que sí es múltiplo de 3 (2103): se prueban los
    // restos 1 y 2 y también el 3 de la spec.
    let n = 3u32;
    let n_i = i64::from(n);
    for mode_2 in TEST_MODES {
        for rest_2 in 0..=5 {
            let height_2 = 700 * n_i + rest_2;
            let layout = panel_layout(mode_2, height_2, n, 0);
            assert!(!layout.scrolling, "{mode_2:?}, resto {rest_2}");
            assert_eq!(layout.top_px % n_i, 0, "{mode_2:?}, resto {rest_2}");
            // `((height_2 − H·n) div n)·n`, con el alto redondeado a píxeles físicos.
            let h = editor_height_u(mode_2) * n_i;
            assert_eq!(layout.top_px, (height_2 - h).div_euclid(n_i) * n_i);
            assert!(height_2 - layout.top_px >= h);
            assert_eq!(
                layout.visible_rows,
                visible_rows_reserved(height_2, height_2 - layout.icons_top_px, n)
            );
        }
    }
}

#[test]
fn one_more_pixel_of_height_does_not_move_the_editor_until_completing_n() {
    // `top_px` es múltiplo de `n`: un píxel físico más de alto no lo mueve; al completar `n`
    // píxeles baja exactamente `n`.
    let n = 2u32;
    let n_i = i64::from(n);
    let mode_2 = Mode::Rgba;
    let a = panel_layout(mode_2, 700 * n_i, n, 0);
    let b = panel_layout(mode_2, 700 * n_i + 1, n, 0);
    let c = panel_layout(mode_2, 701 * n_i, n, 0);
    assert_eq!(a.top_px, b.top_px);
    assert_eq!(c.top_px, a.top_px + n_i);
}

#[test]
fn the_regime_change_is_the_block_of_each_mode() {
    for n in SCALES {
        let n_i = i64::from(n);
        for mode_2 in TEST_MODES {
            let limit = block_px(mode_2, n_i);
            assert_eq!(limit, block_height_px(mode_2, n));
            let just = panel_layout(mode_2, limit, n, 0);
            assert!(!just.scrolling, "{mode_2:?}, n = {n}");
            assert_eq!(just.visible_rows, 1, "{mode_2:?}, n = {n}");
            let below_2 = panel_layout(mode_2, limit - 1, n, 0);
            assert!(below_2.scrolling, "{mode_2:?}, n = {n}");
            assert_eq!(below_2.visible_rows, 1, "{mode_2:?}, n = {n}");
            // Sin salto: con `offset = 0` el `top_px` es el mismo a un lado y otro.
            assert_eq!(below_2.top_px, just.top_px, "{mode_2:?}, n = {n}");
            assert_eq!(
                below_2.top_px,
                (GRID_Y_U + PITCH_U + icons_height_u(mode_2)) * n_i
            );
        }
    }
}

#[test]
fn height_zero_or_negative_does_not_panic_and_scrolls() {
    for n in SCALES {
        for mode_2 in TEST_MODES {
            for height_2 in [0, -5] {
                let layout = panel_layout(mode_2, height_2, n, 0);
                assert!(layout.scrolling, "{mode_2:?}, alto {height_2}");
                assert_eq!(layout.visible_rows, 1);
                assert_eq!(layout.offset_px, 0);
                // Con cualquier desplazamiento pedido, sigue en su rango.
                let huge = panel_layout(mode_2, height_2, n, i64::MAX / 4);
                assert_eq!(huge.offset_px, huge.max_offset_px);
            }
        }
    }
}

#[test]
fn with_the_editor_visible_one_grid_row_always_remains() {
    for n in SCALES {
        let n_i = i64::from(n);
        for mode_2 in TEST_MODES {
            for height_2 in 0..(900 * n_i) {
                let layout = panel_layout(mode_2, height_2, n, 0);
                assert!(
                    layout.visible_rows >= 1,
                    "{mode_2:?}, n = {n}, alto {height_2}"
                );
                assert_eq!(layout.top_px % n_i, 0);
                assert!(layout.top_px >= (GRID_Y_U + PITCH_U + icons_height_u(mode_2)) * n_i);
                assert_eq!(layout.scrolling, height_2 < block_height_px(mode_2, n));
            }
        }
    }
}

// Fila 3: alto del panel 241n, 240n, 200n y 100n.

#[test]
fn row_3_with_the_exact_block_the_editor_fits_and_the_grid_has_one_row() {
    for n in SCALES {
        let n_i = i64::from(n);
        for (mode_2, block) in [(Mode::Indexed, 277), (Mode::Rgba, 249)] {
            let layout = panel_layout(mode_2, block * n_i, n, 0);
            assert!(!layout.scrolling, "{mode_2:?}, n = {n}");
            assert_eq!(layout.offset_px, 0);
            assert_eq!(
                layout.top_px,
                (GRID_Y_U + PITCH_U + icons_height_u(mode_2)) * n_i,
                "{mode_2:?}, n = {n}"
            );
            assert_eq!(layout.visible_rows, 1, "{mode_2:?}, n = {n}");
        }
    }
}

#[test]
fn row_3_with_one_pixel_less_and_offset_0_there_is_no_jump_with_respect_to_the_exact_block() {
    for n in SCALES {
        let n_i = i64::from(n);
        for (mode_2, block) in [(Mode::Indexed, 277), (Mode::Rgba, 249)] {
            let just = panel_layout(mode_2, block * n_i, n, 0);
            let below_2 = panel_layout(mode_2, (block - 1) * n_i, n, 0);
            assert!(below_2.scrolling, "{mode_2:?}, n = {n}");
            assert_eq!(below_2.offset_px, 0);
            assert_eq!(
                below_2.top_px,
                (GRID_Y_U + PITCH_U + icons_height_u(mode_2)) * n_i,
                "{mode_2:?}, n = {n}"
            );
            assert_eq!(below_2.top_px, just.top_px);
            assert_eq!(below_2.visible_rows, 1);
            // Un solo `n` de desplazamiento posible: el bloque excede el panel en `n`.
            assert_eq!(below_2.max_offset_px, n_i, "{mode_2:?}, n = {n}");
        }
    }
}

#[test]
fn row_3_max_offset_px_and_top_with_maximum_offset() {
    for n in SCALES {
        let n_i = i64::from(n);
        for mode_2 in TEST_MODES {
            let block = block_height_px(mode_2, n);
            for height_2 in [241 * n_i, 240 * n_i, 200 * n_i, 100 * n_i] {
                let without = panel_layout(mode_2, height_2, n, 0);
                if height_2 >= block {
                    assert!(!without.scrolling, "{mode_2:?}, n = {n}, alto {height_2}");
                    continue;
                }
                let max = (block - height_2).div_euclid(n_i) * n_i;
                assert!(without.scrolling, "{mode_2:?}, n = {n}, alto {height_2}");
                assert_eq!(
                    without.max_offset_px, max,
                    "{mode_2:?}, n = {n}, alto {height_2}"
                );
                assert_eq!(without.visible_rows, 1);
                assert_eq!(
                    without.top_px,
                    (GRID_Y_U + PITCH_U + icons_height_u(mode_2)) * n_i
                );

                let below = panel_layout(mode_2, height_2, n, max);
                assert_eq!(below.offset_px, max);
                assert_eq!(below.max_offset_px, max);
                assert_eq!(
                    below.top_px,
                    (GRID_Y_U + PITCH_U + icons_height_u(mode_2)) * n_i - max
                );
                // Con el alto múltiplo de `n`, coincide con el editor anclado al fondo.
                assert_eq!(
                    below.top_px,
                    height_2 - editor_height_u(mode_2) * n_i,
                    "{mode_2:?}, n = {n}, alto {height_2}"
                );
                assert_eq!(below.visible_rows, 1);
            }
        }
    }
}

#[test]
fn row_3_the_scroll_is_clipped_and_lowered_to_a_multiple_of_n() {
    let n = 3u32;
    let n_i = i64::from(n);
    let mode_2 = Mode::Indexed;
    let height_2 = 200 * n_i;
    let max = panel_layout(mode_2, height_2, n, 0).max_offset_px;
    assert_eq!(max, 77 * n_i);
    for (requested, expected_2) in [
        (-100, 0),
        (0, 0),
        (1, 0),
        (n_i - 1, 0),
        (n_i, n_i),
        (n_i + 1, n_i),
        (2 * n_i + 2, 2 * n_i),
        (max - 1, max - n_i),
        (max, max),
        (max + 1, max),
        (10_000, max),
    ] {
        let layout = panel_layout(mode_2, height_2, n, requested);
        assert_eq!(layout.offset_px, expected_2, "pedido {requested}");
        assert_eq!(
            layout.top_px,
            (GRID_Y_U + PITCH_U + icons_height_u(mode_2)) * n_i - expected_2,
            "pedido {requested}"
        );
    }
}

#[test]
fn row_3_the_editor_always_has_room_with_maximum_offset() {
    // With `offset = max` the whole editor falls inside the panel (height multiple of `n`).
    for n in SCALES {
        let n_i = i64::from(n);
        for mode_2 in TEST_MODES {
            for height_n in [60, 100, 120, 200, 240] {
                let height_2 = height_n * n_i;
                let without = panel_layout(mode_2, height_2, n, 0);
                let layout = panel_layout(mode_2, height_2, n, without.max_offset_px.max(0));
                if layout.scrolling {
                    let fin = layout.top_px + editor_height_u(mode_2) * n_i;
                    assert_eq!(fin, height_2, "{mode_2:?}, n = {n}, alto {height_2}");
                }
            }
        }
    }
}

proptest! {
    #[test]
    fn panel_layout_meets_its_invariants(
        mode_2 in prop::sample::select(TEST_MODES.to_vec()),
        height_2 in -50i64..6000,
        n in 1u32..=8,
        requested in -200i64..9000,
    ) {
        let n_i = i64::from(n);
        let block = block_height_px(mode_2, n);
        let layout = panel_layout(mode_2, height_2, n, requested);
        if height_2 >= block {
            prop_assert!(!layout.scrolling);
            prop_assert_eq!(layout.offset_px, 0);
            prop_assert_eq!(layout.top_px, normal(mode_2, height_2, n).top_px);
            prop_assert_eq!(layout.visible_rows, normal(mode_2, height_2, n).visible_rows);
        } else {
            let max = (block - height_2).div_euclid(n_i) * n_i;
            prop_assert!(layout.scrolling);
            prop_assert_eq!(layout.max_offset_px, max);
            prop_assert!(layout.offset_px >= 0 && layout.offset_px <= max);
            prop_assert_eq!(layout.offset_px % n_i, 0);
            prop_assert_eq!(layout.offset_px, requested.clamp(0, max).div_euclid(n_i) * n_i);
            prop_assert_eq!(
                layout.top_px,
                (GRID_Y_U + PITCH_U + icons_height_u(mode_2)) * n_i - layout.offset_px
            );
            prop_assert_eq!(layout.visible_rows, 1);
        }
        prop_assert_eq!(layout.top_px % n_i, 0);
    }

    #[test]
    fn the_editor_does_not_jump_when_crossing_the_block(
        mode_2 in prop::sample::select(TEST_MODES.to_vec()),
        n in 1u32..=8,
        extra in 0i64..40,
    ) {
        // Un píxel por encima o por debajo del bloque, con offset 0, el top es el mismo.
        let block = block_height_px(mode_2, n);
        let a = panel_layout(mode_2, block - 1 - extra.min(i64::from(n) - 1), n, 0);
        let b = panel_layout(mode_2, block, n, 0);
        prop_assert_eq!(a.top_px, b.top_px);
        prop_assert_eq!(a.visible_rows, b.visible_rows);
    }
}

// `visible_rows_reserved`.

#[test]
fn visible_rows_reserved_with_zero_reserve_is_visible_rows() {
    for n in [1u32, 2, 3, 4] {
        for height_2 in -50i64..3000 {
            assert_eq!(
                visible_rows_reserved(height_2, 0, n),
                visible_rows(height_2, n),
                "alto {height_2}, n = {n}"
            );
        }
    }
}

#[test]
fn visible_rows_reserved_uses_the_remaining_height() {
    for n in SCALES {
        let n_i = i64::from(n);
        // 700n with 186n reserved: (700 − 186 − 49) / 14 = 33.
        assert_eq!(visible_rows_reserved(700 * n_i, 186 * n_i, n), 33);
        // Una reserva mayor que el alto no da pánico ni filas.
        assert_eq!(visible_rows_reserved(100 * n_i, 500 * n_i, n), 0);
        assert_eq!(visible_rows_reserved(0, 0, n), 0);
    }
}

proptest! {
    #[test]
    fn visible_rows_reserved_is_visible_rows_of_what_is_left(
        height_2 in 0i64..6000,
        reserved in 0i64..6000,
        n in 1u32..=8,
    ) {
        prop_assert_eq!(
            visible_rows_reserved(height_2, reserved, n),
            visible_rows(height_2 - reserved.min(height_2), n)
        );
    }

    #[test]
    fn reserving_more_never_gives_more_rows(
        height_2 in 0i64..6000,
        reserved in 0i64..6000,
        extra in 0i64..500,
        n in 1u32..=8,
    ) {
        prop_assert!(
            visible_rows_reserved(height_2, reserved + extra, n)
                <= visible_rows_reserved(height_2, reserved, n)
        );
    }
}

// `slider_hit`.

/// Control cuya zona activa contiene el punto, calculado con `control_row_u` (probado arriba).
fn control_expected(mode_2: Mode, x: i64, y: i64, n: i64) -> Option<Control> {
    if x < HIT_X0_U * n || x >= HIT_X1_U * n {
        return None;
    }
    controls(mode_2).iter().copied().find(|control| {
        let row_2 = control_row_u(mode_2, *control).expect("tiene fila");
        y >= row_2 * n && y < (row_2 + ROW_H_U) * n
    })
}

#[test]
fn slider_hit_walks_the_whole_area_of_each_mode() {
    for n in SCALES {
        let n_i = i64::from(n);
        for mode_2 in TEST_MODES {
            let xs = [
                -1,
                0,
                HIT_X0_U * n_i - 1,
                HIT_X0_U * n_i,
                HIT_X0_U * n_i + 1,
                100 * n_i,
                HIT_X1_U * n_i - 1,
                HIT_X1_U * n_i,
                233 * n_i,
            ];
            for y in -2..(editor_height_u(mode_2) * n_i + 2) {
                for x in xs {
                    assert_eq!(
                        slider_hit(mode_2, x, y, n),
                        control_expected(mode_2, x, y, n_i),
                        "{mode_2:?}, n = {n}, ({x}, {y})"
                    );
                }
            }
        }
    }
}

#[test]
fn slider_hit_at_the_corners_of_each_row() {
    for n in SCALES {
        let n_i = i64::from(n);
        for mode_2 in TEST_MODES {
            for control in controls(mode_2) {
                let row_2 = control_row_u(mode_2, *control).expect("tiene fila");
                let (x0, x1) = (HIT_X0_U * n_i, HIT_X1_U * n_i);
                let (y0, y1) = (row_2 * n_i, (row_2 + ROW_H_U) * n_i);
                // Dentro: las cuatro esquinas incluidas y las excluidas.
                assert_eq!(slider_hit(mode_2, x0, y0, n), Some(*control));
                assert_eq!(slider_hit(mode_2, x1 - 1, y0, n), Some(*control));
                assert_eq!(slider_hit(mode_2, x0, y1 - 1, n), Some(*control));
                assert_eq!(slider_hit(mode_2, x1 - 1, y1 - 1, n), Some(*control));
                // Fuera por la x.
                assert_eq!(slider_hit(mode_2, x0 - 1, y0, n), None);
                assert_eq!(slider_hit(mode_2, x1, y0, n), None);
            }
        }
    }
}

#[test]
fn slider_hit_does_not_hit_in_the_header_the_field_or_the_notice() {
    for n in SCALES {
        let n_i = i64::from(n);
        for mode_2 in TEST_MODES {
            let x = 100 * n_i;
            assert_eq!(slider_hit(mode_2, x, 0, n), None, "separador");
            assert_eq!(slider_hit(mode_2, x, HEADER_Y_U * n_i, n), None, "cabecera");
            assert_eq!(
                slider_hit(mode_2, x, field_y_u(mode_2) * n_i, n),
                None,
                "campo"
            );
            assert_eq!(
                slider_hit(mode_2, x, notice_y_u(mode_2) * n_i, n),
                None,
                "aviso"
            );
            assert_eq!(
                slider_hit(mode_2, x, editor_height_u(mode_2) * n_i, n),
                None,
                "bajo el editor"
            );
        }
    }
}

#[test]
fn slider_hit_in_the_gap_between_groups_does_not_hit() {
    for n in SCALES {
        let n_i = i64::from(n);
        let x = 100 * n_i;
        for mode_2 in [Mode::Indexed, Mode::Rgba] {
            // Entre B (50..64) y H (68..82), y entre L (96..110) y A (114..128).
            let after_b = control_row_u(mode_2, Control::Blue).expect("fila") + ROW_H_U;
            let after_l = control_row_u(mode_2, Control::Lightness).expect("fila") + ROW_H_U;
            for y in [after_b * n_i, (after_b + GROUP_GAP_U) * n_i - 1] {
                assert_eq!(slider_hit(mode_2, x, y, n), None, "{mode_2:?}, y = {y}");
            }
            for y in [after_l * n_i, (after_l + GROUP_GAP_U) * n_i - 1] {
                assert_eq!(slider_hit(mode_2, x, y, n), None, "{mode_2:?}, y = {y}");
            }
            // Las tres filas de un grupo son contiguas.
            let green = control_row_u(mode_2, Control::Green).expect("fila") * n_i;
            assert_eq!(slider_hit(mode_2, x, green - 1, n), Some(Control::Red));
            assert_eq!(slider_hit(mode_2, x, green, n), Some(Control::Green));
        }
    }
}

#[test]
fn slider_hit_does_not_see_controls_that_the_mode_does_not_have() {
    let n = 2i64;
    let x = 100 * n;
    // Gray no tiene segunda fila de deslizadores.
    let second = (FIRST_ROW_Y_U + ROW_H_U) * n;
    assert_eq!(slider_hit(Mode::Gray, x, second, 2), None);
    assert_eq!(
        slider_hit(Mode::Gray, x, FIRST_ROW_Y_U * n, 2),
        Some(Control::Gray)
    );
}

// Bordes en píxeles físicos enteros.

#[test]
fn every_edge_in_points_per_ppp_is_an_integer() {
    for ppp in PPPS {
        let n = text_scale(ppp);
        let n_i = i64::from(n);
        for mode_2 in TEST_MODES {
            let borders = edges_u(mode_2);
            assert!(!borders.is_empty());
            for border in borders {
                let physical = (border * n_i) as f32;
                let points = physical / ppp;
                let of_return = points * ppp;
                assert!(
                    (of_return - of_return.round()).abs() <= 0.001,
                    "{mode_2:?}, ppp {ppp}: borde {border} u = {points} pt = {of_return} px"
                );
                assert!(
                    (of_return - physical).abs() <= 0.001,
                    "{mode_2:?}, ppp {ppp}: borde {border} u cae en {of_return} px, no en {physical}"
                );
            }
        }
    }
}

#[test]
fn the_editor_anchoring_falls_on_whole_physical_pixels() {
    for ppp in PPPS {
        let n = text_scale(ppp);
        for mode_2 in TEST_MODES {
            for height_2 in [700, 701, 777, 1080, 1441] {
                let height_2 = height_2 * i64::from(n) + i64::from(n) / 2;
                let layout = panel_layout(mode_2, height_2, n, 0);
                assert!(!layout.scrolling, "{mode_2:?}, ppp {ppp}, alto {height_2}");
                let points = layout.top_px as f32 / ppp;
                let physical = points * ppp;
                assert!(
                    (physical - layout.top_px as f32).abs() <= 0.001,
                    "{mode_2:?}, ppp {ppp}, alto {height_2}"
                );
            }
        }
    }
}

#[test]
fn edges_u_contains_the_edges_of_the_mockup() {
    for mode_2 in TEST_MODES {
        let borders = edges_u(mode_2);
        assert!(borders.iter().all(|b| *b >= 0), "{mode_2:?}: {borders:?}");
        let mut expected_2_2 = vec![
            SEPARATOR_Y_U,
            SEPARATOR_Y_U + 1,
            HEADER_Y_U,
            HEADER_Y_U + ROW_H_U,
            TRACK_X_U,
            TRACK_X_U + TRACK_W_U,
            VALUE_X_U,
            VALUE_X_U + VALUE_W_U,
            FIELD_X_U,
            FIELD_X_U + FIELD_W_U,
            field_y_u(mode_2),
            field_y_u(mode_2) + FIELD_H_U,
            notice_y_u(mode_2),
            notice_y_u(mode_2) + ROW_H_U,
            notice_y_u(mode_2) + NOTICE_LINES * ROW_H_U,
            editor_height_u(mode_2),
        ];
        for control in controls(mode_2) {
            let row_2 = control_row_u(mode_2, *control).expect("tiene fila");
            expected_2_2.extend([
                row_2,
                row_2 + ROW_H_U,
                // La pista, centrada en la fila.
                row_2 + (ROW_H_U - TRACK_H_U) / 2,
                row_2 + (ROW_H_U - TRACK_H_U) / 2 + TRACK_H_U,
                // La marca, centrada en la fila.
                row_2 + (ROW_H_U - THUMB_H_U) / 2,
                row_2 + (ROW_H_U - THUMB_H_U) / 2 + THUMB_H_U,
            ]);
        }
        for expected_2 in expected_2_2 {
            assert!(
                borders.contains(&expected_2),
                "{mode_2:?}: falta el borde {expected_2} u en {borders:?}"
            );
        }
    }
}

#[test]
fn edges_u_does_not_include_the_rows_the_mode_does_not_have() {
    let gray = edges_u(Mode::Gray);
    let complete = edges_u(Mode::Rgba);
    // La fila de B de RGBA (y = 50) no existe en Gray.
    let third_row = FIRST_ROW_Y_U + 2 * ROW_H_U;
    assert!(complete.contains(&third_row));
    assert!(!gray.contains(&third_row));
}

// Botones de la paleta: una fila de cinco iconos de 24 u y, con una quita pendiente, `Confirmar` y
// `Cancel` en texto en su lugar (specs/interfaz-iconos.md, decisiones 7 y 8). Antes: cuatro filas
// de botones de texto (specs/hito-2-paleta-operaciones.md y hito-2-fijar-transparente.md, filas 1
// y 2); las comprobaciones son las mismas con los rectángulos nuevos.

const ALL_THE_BUTTONS: [PaletteButton; 7] = [
    PaletteButton::Add,
    PaletteButton::Remove,
    PaletteButton::Confirm,
    PaletteButton::Cancel,
    PaletteButton::RotateLeft,
    PaletteButton::RotateRight,
    PaletteButton::Transparent,
];

#[test]
fn the_buttons_measure_24_u_and_sit_2_u_into_the_icon_row_only_in_indexed() {
    assert_eq!(BUTTON_ROWS, 1);
    assert_eq!(BUTTON_H_U, 24);
    assert_eq!(BUTTON_ROWS * BUTTON_H_U, 24);
    assert_eq!(buttons_height_u(Mode::Indexed), 24);
    assert_eq!(icons_height_u(Mode::Indexed), 28);
    for mode_2 in [Mode::Rgba, Mode::Gray, Mode::GrayAlpha] {
        assert_eq!(buttons_height_u(mode_2), 0, "{mode_2:?}");
        // Without buttons there is no icon row.
        assert_eq!(icons_height_u(mode_2), 0, "{mode_2:?}");
        for button_2 in ALL_THE_BUTTONS {
            assert_eq!(
                button_rect_u(mode_2, button_2),
                None,
                "{mode_2:?}, {button_2:?}"
            );
        }
    }
    // `Confirmar` y `Cancel` y su hueco ocupan el ancho útil del texto.
    assert_eq!(2 * BUTTON_HALF_W_U + BUTTON_COL_GAP_U, BUTTON_FULL_W_U);
    assert_eq!(BUTTON_FULL_W_U, 225);
}

#[test]
fn button_rect_u_follows_the_table_of_the_mockup() {
    let m = Mode::Indexed;
    assert_eq!(BUTTON_X_U, 4);
    // Los cinco iconos, en una fila: hueco de 1 u en un grupo y de 8 u entre grupos.
    for (button_2, x) in [
        (PaletteButton::Add, 4),
        (PaletteButton::Remove, 29),
        (PaletteButton::RotateLeft, 61),
        (PaletteButton::RotateRight, 86),
        (PaletteButton::Transparent, 118),
    ] {
        assert_eq!(
            button_rect_u(m, button_2),
            Some((x, 2, 24, 24)),
            "{button_2:?}"
        );
    }
    // `Confirmar` y `Cancel`: texto, de 110 × 24 u.
    assert_eq!(
        button_rect_u(m, PaletteButton::Confirm),
        Some((4, 2, 110, 24))
    );
    assert_eq!(
        button_rect_u(m, PaletteButton::Cancel),
        Some((119, 2, 110, 24))
    );
    // The last button ends 2 u (the margin) before the end of the 28 u icon row.
    assert_eq!(2 + 24 + 2, icons_height_u(m));
    // None leaves the row or the panel.
    for button_2 in ALL_THE_BUTTONS {
        let (x, y, w, h) = button_rect_u(m, button_2).expect("indexadas tienen botones");
        assert!(y >= 0 && y + h <= icons_height_u(m), "{button_2:?}");
        assert!(x >= 0 && x + w < PANEL_WIDTH_U, "{button_2:?}");
    }
}

#[test]
fn button_hit_at_the_points_of_the_spec() {
    use PaletteButton::*;
    for n in SCALES {
        let k = i64::from(n);
        let m = Mode::Indexed;
        // (x, y, con los iconos, con una quita pendiente)
        let cases = [
            (4 * k, 2 * k, Some(Add), Some(Confirm)),
            (28 * k - 1, 2 * k, Some(Add), Some(Confirm)),
            (28 * k, 2 * k, None, Some(Confirm)),
            (29 * k, 2 * k, Some(Remove), Some(Confirm)),
            (53 * k - 1, 26 * k - 1, Some(Remove), Some(Confirm)),
            (53 * k, 2 * k, None, Some(Confirm)),
            (60 * k, 2 * k, None, Some(Confirm)),
            (61 * k, 2 * k, Some(RotateLeft), Some(Confirm)),
            (86 * k, 2 * k, Some(RotateRight), Some(Confirm)),
            (110 * k, 2 * k, None, Some(Confirm)),
            (114 * k, 2 * k, None, None),
            (118 * k, 2 * k, Some(Transparent), None),
            (119 * k, 2 * k, Some(Transparent), Some(Cancel)),
            (142 * k - 1, 26 * k - 1, Some(Transparent), Some(Cancel)),
            (142 * k, 2 * k, None, Some(Cancel)),
            (229 * k - 1, 25 * k, None, Some(Cancel)),
            (229 * k, 2 * k, None, None),
            (10 * k, 185 * k, None, None),
            (10 * k, 26 * k, None, None),
            (-1, -1, None, None),
        ];
        for (x, y, icons_2, removes) in cases {
            assert_eq!(
                button_hit(m, false, x, y, n),
                icons_2,
                "n = {n}, ({x}, {y})"
            );
            assert_eq!(
                button_hit(m, true, x, y, n),
                removes,
                "n = {n}, ({x}, {y}) con quita"
            );
        }
        for mode_2 in [Mode::Rgba, Mode::Gray, Mode::GrayAlpha] {
            for removing in [false, true] {
                assert_eq!(button_hit(mode_2, removing, 10 * k, 6 * k, n), None);
            }
        }
    }
}

#[test]
fn the_button_edges_fall_on_whole_physical_pixels() {
    for ppp in PPPS {
        let n = text_scale(ppp);
        let k = i64::from(n);
        for button_2 in ALL_THE_BUTTONS {
            let (x, y, w, h) = button_rect_u(Mode::Indexed, button_2).expect("botones");
            for u in [
                x,
                x + 1,
                x + 4,
                x + w - 4,
                x + w - 1,
                x + w,
                x + BUTTON_TEXT_X_U,
                y,
                y + 1,
                y + 4,
                y + h - 4,
                y + h - 1,
                y + h,
                y + BUTTON_TEXT_Y_U,
            ] {
                let physical = (u * k) as f32 / ppp * ppp;
                assert!(
                    (physical - physical.round()).abs() <= 0.001 && (u * k) % k == 0,
                    "ppp {ppp}, {button_2:?}, {u} u"
                );
            }
        }
        // Y están en `edges_u`.
        let borders = edges_u(Mode::Indexed);
        for button_2 in [PaletteButton::Add, PaletteButton::RotateRight] {
            let (x, y, w, h) = button_rect_u(Mode::Indexed, button_2).expect("botones");
            for u in [x, x + w, y, y + h] {
                assert!(borders.contains(&u), "{button_2:?}: {u}");
            }
        }
    }
}

// Unidad 21 (specs/hito-2-fijar-transparente.md, filas 1 y 2) con la fila de iconos de I.4: el
// botón `Transparent` es el quinto icono; las cifras de alturas, ventana y `panel_layout` están en
// las pruebas adaptadas de arriba.

const BUTTONS_U_21: [PaletteButton; 7] = ALL_THE_BUTTONS;

#[test]
fn row_1_u21_the_icon_row_takes_28_u_under_the_grid_and_leaves_the_editor_at_186() {
    assert_eq!(buttons_height_u(Mode::Indexed), 24);
    // The derivation of the layout, with literals: 24 of buttons and 2 u of margin on each side.
    assert_eq!(icons_height_u(Mode::Indexed), 2 + 24 + 2);
    assert_eq!(editor_height_u(Mode::Indexed), 186);
    // One single function: the block, the minimum and the last button hang from it.
    let (_, y, _, h) = button_rect_u(Mode::Indexed, PaletteButton::Transparent).expect("botón");
    assert_eq!(y + h + 2, icons_height_u(Mode::Indexed));
    for n in SCALES {
        let k = i64::from(n);
        assert_eq!(block_height_px(Mode::Indexed, n), 277 * k);
        assert_eq!(min_panel_height_px(n), 277 * k);
    }
    // Solo indexadas llevan botones: los demás modos no cambian (2, 94 y 112 u).
    for (mode_2, height_2_2) in [(Mode::Rgba, 186), (Mode::Gray, 94), (Mode::GrayAlpha, 112)] {
        assert_eq!(buttons_height_u(mode_2), 0, "{mode_2:?}");
        assert_eq!(editor_height_u(mode_2), height_2_2, "{mode_2:?}");
    }
}

#[test]
fn row_2_u21_button_rect_u_of_the_transparent_button() {
    let m = Mode::Indexed;
    assert_eq!(
        button_rect_u(m, PaletteButton::Transparent),
        Some((118, 2, 24, 24))
    );
    let (x, y, w, h) = button_rect_u(m, PaletteButton::Transparent).expect("botón");
    // Quinto icono de la fila: mismo `y` y mismo tamaño que los otros cuatro, 8 u tras el grupo
    // de rotar.
    let (xd, yd, wd, _) = button_rect_u(m, PaletteButton::RotateRight).expect("botón");
    assert_eq!((y, w, h), (yd, BUTTON_H_U, BUTTON_H_U));
    assert_eq!(x, xd + wd + 8);
    // It does not leave the row or the panel nor overlap another button of the same row.
    assert!(y >= 0 && y + h <= icons_height_u(m));
    assert!(x >= 0 && x + w < PANEL_WIDTH_U);
    for other_2 in BUTTONS_U_21 {
        if matches!(
            other_2,
            PaletteButton::Transparent | PaletteButton::Confirm | PaletteButton::Cancel
        ) {
            continue;
        }
        let (ox, oy, ow, oh) = button_rect_u(m, other_2).expect("botón");
        let overlaps = x < ox + ow && ox < x + w && y < oy + oh && oy < y + h;
        assert!(!overlaps, "solapa con {other_2:?}");
    }
}

#[test]
fn row_2_u21_the_modes_without_buttons_do_not_have_the_transparent_one() {
    for mode_2 in [Mode::Rgba, Mode::Gray, Mode::GrayAlpha] {
        assert_eq!(
            button_rect_u(mode_2, PaletteButton::Transparent),
            None,
            "{mode_2:?}"
        );
        for n in SCALES {
            let k = i64::from(n);
            for removing in [false, true] {
                // El punto donde está el botón en indexadas y todo el alto del editor.
                for y in [0, 6 * k, 16 * k, 26 * k, editor_height_u(mode_2) * k - 1] {
                    assert_eq!(
                        button_hit(mode_2, removing, 120 * k, y, n),
                        None,
                        "{mode_2:?}, n = {n}, y = {y}"
                    );
                }
            }
        }
    }
}

#[test]
fn row_2_u21_button_hit_at_the_points_of_the_transparent_button() {
    use PaletteButton::*;
    for n in SCALES {
        let k = i64::from(n);
        let m = Mode::Indexed;
        let cases = [
            // Esquina superior izquierda y última esquina dentro del botón.
            (118 * k, 2 * k, Some(Transparent)),
            (141 * k, 25 * k, Some(Transparent)),
            (142 * k - 1, 26 * k - 1, Some(Transparent)),
            (130 * k, 14 * k, Some(Transparent)),
            // A la izquierda (hueco de grupo), a la derecha, 1 u sobre el botón y bajo él.
            (117 * k, 14 * k, None),
            (142 * k, 14 * k, None),
            (130 * k, 185 * k, None),
            (130 * k, 26 * k, None),
            // Un píxel físico fuera por cada lado.
            (118 * k - 1, 2 * k, None),
            (142 * k, 25 * k, None),
            (130 * k, 2 * k - 1, None),
        ];
        for (x, y, expected_2) in cases {
            assert_eq!(
                button_hit(m, false, x, y, n),
                expected_2,
                "n = {n}, ({x}, {y})"
            );
        }
        // Con una quita pendiente no hay iconos: solo `Confirmar` y `Cancel` (decisión 8).
        assert_eq!(button_hit(m, true, 130 * k, 14 * k, n), Some(Cancel));
        assert_eq!(button_hit(m, true, 118 * k, 14 * k, n), None);
        // Los demás iconos siguen en sus puntos.
        assert_eq!(button_hit(m, false, 4 * k, 2 * k, n), Some(Add));
        assert_eq!(button_hit(m, true, 4 * k, 2 * k, n), Some(Confirm));
        assert_eq!(button_hit(m, false, 70 * k, 14 * k, n), Some(RotateLeft));
        assert_eq!(button_hit(m, false, 95 * k, 14 * k, n), Some(RotateRight));
        assert_eq!(button_hit(m, true, 95 * k, 14 * k, n), Some(Confirm));
    }
}

#[test]
fn row_2_u21_button_hit_walks_the_whole_rectangle_of_the_button() {
    let n = 2u32;
    let k = i64::from(n);
    let m = Mode::Indexed;
    let (x, y, w, h) = button_rect_u(m, PaletteButton::Transparent).expect("botón");
    for py in (y - 2) * k..(y + h + 2) * k {
        for px in ((x - 2) * k..(x + w + 2) * k).step_by(3) {
            let inside = px >= x * k && px < (x + w) * k && py >= y * k && py < (y + h) * k;
            let expected_2 = inside.then_some(PaletteButton::Transparent);
            assert_eq!(button_hit(m, false, px, py, n), expected_2, "({px}, {py})");
            // Con una quita pendiente el icono no golpea nunca.
            assert_ne!(
                button_hit(m, true, px, py, n),
                Some(PaletteButton::Transparent),
                "({px}, {py}) con quita"
            );
        }
    }
}

#[test]
fn row_2_u21_the_button_edges_are_in_edges_u_and_are_integers_at_the_five_ppp() {
    let (x, y, w, h) = button_rect_u(Mode::Indexed, PaletteButton::Transparent).expect("botón");
    let borders = edges_u(Mode::Indexed);
    for u in [
        x,
        x + 1,
        x + 4,
        x + w - 4,
        x + w - 1,
        x + w,
        y,
        y + 1,
        y + 4,
        y + h - 4,
        y + h - 1,
        y + h,
    ] {
        assert!(borders.contains(&u), "falta el borde {u} en edges_u");
    }
    assert!(borders.contains(&2) && borders.contains(&26));
    assert!(borders.contains(&editor_height_u(Mode::Indexed)));
    for ppp in PPPS {
        let n = text_scale(ppp);
        let k = i64::from(n);
        for u in edges_u(Mode::Indexed) {
            let physical = (u * k) as f32 / ppp * ppp;
            assert!(
                (physical - physical.round()).abs() <= 0.001 && (u * k) % k == 0,
                "ppp {ppp}: el borde {u} u no cae en píxel físico entero"
            );
        }
    }
}

// ---------------------------------------------------------------------------------------------
// Unidad I.4 (specs/interfaz-iconos.md): fila 4 y propiedad 3 de la fila de iconos del editor
// ---------------------------------------------------------------------------------------------

mod icons {
    use super::*;

    type RectU = (i64, i64, i64, i64);

    /// Los cinco iconos con los literales de la spec y los dos botones de texto de la quita
    /// pendiente.
    const ICONS: [(PaletteButton, RectU); 5] = [
        (PaletteButton::Add, (4, 2, 24, 24)),
        (PaletteButton::Remove, (29, 2, 24, 24)),
        (PaletteButton::RotateLeft, (61, 2, 24, 24)),
        (PaletteButton::RotateRight, (86, 2, 24, 24)),
        (PaletteButton::Transparent, (118, 2, 24, 24)),
    ];
    const TEST_TEXTS: [(PaletteButton, RectU); 2] = [
        (PaletteButton::Confirm, (4, 2, 110, 24)),
        (PaletteButton::Cancel, (119, 2, 110, 24)),
    ];

    #[test]
    fn row_4_the_editor_icon_row_has_the_geometry_of_the_spec_at_the_five_scales() {
        for (button_2, rect) in ICONS.into_iter().chain(TEST_TEXTS) {
            assert_eq!(
                button_rect_u(Mode::Indexed, button_2),
                Some(rect),
                "{button_2:?}"
            );
        }
        for (mode_2, height_2) in [
            (Mode::Indexed, 186),
            (Mode::Rgba, 186),
            (Mode::GrayAlpha, 112),
            (Mode::Gray, 94),
        ] {
            assert_eq!(editor_height_u(mode_2), height_2, "{mode_2:?}");
        }
        // Rgba, Gray y GrayAlpha: sin botones.
        for mode_2 in [Mode::Rgba, Mode::Gray, Mode::GrayAlpha] {
            assert_eq!(buttons_height_u(mode_2), 0, "{mode_2:?}");
            for (button_2, _) in ICONS.into_iter().chain(TEST_TEXTS) {
                assert_eq!(button_rect_u(mode_2, button_2), None);
            }
        }
        // Nothing leaves the row; the 1 u and 8 u gaps do not hit.
        for (_, (x, y, w, h)) in ICONS.into_iter().chain(TEST_TEXTS) {
            assert!(y >= 0 && y + h + 2 == icons_height_u(Mode::Indexed));
            assert!(x >= 0 && x + w < PANEL_WIDTH_U);
        }
        for n in SCALES {
            let k = i64::from(n);
            for x_u in [28, 53, 57, 60, 85, 110, 117, 142] {
                assert_eq!(
                    button_hit(Mode::Indexed, false, x_u * k, 198 * k, n),
                    None,
                    "n = {n}, x = {x_u} u: es un hueco"
                );
            }
        }
        // Todo borde en puntos × `ppp` es entero (±0,001) y múltiplo de `n`.
        for ppp in PPPS {
            let k = i64::from(text_scale(ppp));
            for (_, (x, y, w, h)) in ICONS.into_iter().chain(TEST_TEXTS) {
                for u in [x, x + 1, x + 4, x + w - 4, x + w, y, y + 4, y + h] {
                    let physical = (u * k) as f32 / ppp * ppp;
                    assert!(
                        (physical - physical.round()).abs() <= 0.001,
                        "ppp {ppp}: {u} u"
                    );
                }
            }
        }
    }

    proptest! {
        #![proptest_config(ProptestConfig::with_cases(256))]

        /// Propiedad 3 (el editor): un punto golpea como mucho un botón y exactamente el de su
        /// rectángulo; con una quita pendiente solo `Confirmar` y `Cancel`; es total.
        #[test]
        fn property_3_button_hit_gives_the_button_of_its_rectangle_and_nothing_in_the_gaps(
            n in 1u32..=8,
            x in -60i64..=2000,
            y in -60i64..=2200,
            removing in any::<bool>(),
        ) {
            let k = i64::from(n);
            let table: &[(PaletteButton, RectU)] = if removing { &TEST_TEXTS } else { &ICONS };
            let expected_2 = table
                .iter()
                .find(|(_, r)| x >= r.0 * k && x < (r.0 + r.2) * k && y >= r.1 * k && y < (r.1 + r.3) * k)
                .map(|(b, _)| *b);
            prop_assert_eq!(button_hit(Mode::Indexed, removing, x, y, n), expected_2);
            for mode_2 in [Mode::Rgba, Mode::Gray, Mode::GrayAlpha] {
                prop_assert_eq!(button_hit(mode_2, removing, x, y, n), None);
            }
        }
    }
}
