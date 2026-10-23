//! Tests de `palette_grid` (specs/hito-2-panel-paletas.md, unidad 11b): la maqueta del panel de
//! paletas, sin tipos de `egui`.
//!
//! Cubre las filas 1 a 4 y 10 de «Casos límite», la parte geométrica de la fila 11 (paletas de 1
//! y 256 entradas y las 9 ranuras de los modos sin paleta) y las propiedades 1 (geometría) y 2
//! (desplazamiento). Las medidas están en «píxeles de interfaz» `u` = `n` píxeles físicos, con
//! `n` = 1, 2 y 3 donde la spec lo dice.
//!
//! Los números esperados salen de la spec. Se usan las constantes de `palette_grid` cuando es
//! natural, pero `the_constants_are_worth_what_the_spec_says` fija los valores literales para que
//! cambiar una constante rompa un test. El modelo del desplazamiento (propiedad 2) usa `i128`
//! y `div_euclid`, sin las fórmulas de la implementación.

use proptest::collection::vec;
use proptest::prelude::*;
use px_core::{Image, Palette, Pixel, Rgba};
use px_ui::palette_grid::{
    CELL_U, CHECK_A, CHECK_B, CHECK_U, COLUMNS, GAP_U, GRID_X_U, GRID_Y_U, Grid, PANEL_WIDTH_U,
    PITCH_U, RIGHT_LINE_X_U, RING_ACTIVE_U, RING_HOVER_U, Scroll, TEXT_WIDTH_U, TEXT_X_U,
    TRACK_W_U, TRACK_X_U, edges_u, max_first_row, ring_rects, swatch_parts, thumb, total_rows,
    visible_rows,
};
use px_ui::texture::pixel_rgba;
use px_ui::theme::{self, contrast_ratio};
use px_ui::view::ScreenRect;

const SCALES: [u32; 3] = [1, 2, 3];

fn rect(x: i64, y: i64, width: i64, height: i64) -> ScreenRect {
    ScreenRect {
        x,
        y,
        width,
        height,
    }
}

fn contains(r: ScreenRect, x: i64, y: i64) -> bool {
    x >= r.x && x < r.x + r.width && y >= r.y && y < r.y + r.height
}

// Constantes.

#[test]
fn the_constants_are_worth_what_the_spec_says() {
    assert_eq!(COLUMNS, 16usize);
    assert_eq!(CELL_U, 12);
    assert_eq!(GAP_U, 2);
    assert_eq!(PITCH_U, 14);
    assert_eq!(CHECK_U, 6);
    assert_eq!(PANEL_WIDTH_U, 233);
    assert_eq!(GRID_X_U, 4);
    assert_eq!(GRID_Y_U, 4);
    assert_eq!(TEXT_X_U, 4);
    assert_eq!(TEXT_WIDTH_U, 225);
    assert_eq!(TRACK_X_U, 230);
    assert_eq!(TRACK_W_U, 2);
    assert_eq!(RIGHT_LINE_X_U, 232);
    assert_eq!(RING_ACTIVE_U, 2);
    assert_eq!(RING_HOVER_U, 1);
    assert_eq!(CHECK_A, Rgba::opaque(0x4a, 0x4a, 0x4a));
    assert_eq!(CHECK_B, Rgba::opaque(0x6e, 0x6e, 0x6e));
}

#[test]
fn the_constants_are_consistent_with_each_other() {
    assert_eq!(PITCH_U, CELL_U + GAP_U);
    assert_eq!(CHECK_U * 2, CELL_U);
    // 4 (margen) + 224 (16 pasos) + 2 + 2 (pista) + 1 (trazo derecho).
    assert_eq!(
        PANEL_WIDTH_U,
        GRID_X_U + COLUMNS as i64 * PITCH_U + 2 + TRACK_W_U + 1
    );
    assert_eq!(RIGHT_LINE_X_U, PANEL_WIDTH_U - 1);
    assert_eq!(TRACK_X_U + TRACK_W_U, RIGHT_LINE_X_U);
}

#[test]
fn edges_u_contains_the_edges_of_the_mockup() {
    let borders = edges_u();
    assert!(!borders.is_empty());
    // Los únicos negativos posibles son los de los anillos (-2).
    for &border in &borders {
        assert!(
            border >= 0 || border == -2,
            "borde {border}: solo se admite -2 como negativo"
        );
    }
    for (name_2, expected_2) in [
        ("PANEL_WIDTH_U", PANEL_WIDTH_U),
        ("GRID_X_U", GRID_X_U),
        ("GRID_Y_U", GRID_Y_U),
    ] {
        assert!(
            borders.contains(&expected_2),
            "falta {name_2} = {expected_2} en {borders:?}"
        );
    }
    // Y con los valores literales de la spec.
    for expected_2 in [233, 4, 74, 72] {
        assert!(borders.contains(&expected_2), "falta {expected_2}");
    }
}

// Fila 1: `cell_rect`.

#[test]
fn row_1_cell_rect_with_256_entries_and_16_rows() {
    for n in SCALES {
        let k = i64::from(n);
        let grid = Grid::new(n, 256, 16, 0);
        assert_eq!(grid.cell_rect(0), Some(rect(0, 0, 12 * k, 12 * k)), "n={n}");
        assert_eq!(
            grid.cell_rect(1),
            Some(rect(14 * k, 0, 12 * k, 12 * k)),
            "n={n}"
        );
        assert_eq!(
            grid.cell_rect(15),
            Some(rect(210 * k, 0, 12 * k, 12 * k)),
            "n={n}"
        );
        assert_eq!(
            grid.cell_rect(16),
            Some(rect(0, 14 * k, 12 * k, 12 * k)),
            "n={n}"
        );
        assert_eq!(
            grid.cell_rect(255),
            Some(rect(210 * k, 210 * k, 12 * k, 12 * k)),
            "n={n}"
        );
        assert_eq!(grid.cell_rect(256), None, "n={n}");
    }
}

#[test]
fn row_1_cell_rect_with_a_scroll_of_3_rows_and_5_visible() {
    for n in SCALES {
        let k = i64::from(n);
        let grid = Grid::new(n, 256, 5, 3);
        assert_eq!(grid.cell_rect(47), None, "n={n}: fila 2, por arriba");
        assert_eq!(
            grid.cell_rect(48),
            Some(rect(0, 0, 12 * k, 12 * k)),
            "n={n}"
        );
        assert_eq!(
            grid.cell_rect(111),
            Some(rect(210 * k, 42 * k, 12 * k, 12 * k)),
            "n={n}"
        );
        assert_eq!(
            grid.cell_rect(112),
            Some(rect(0, 56 * k, 12 * k, 12 * k)),
            "n={n}"
        );
        assert_eq!(grid.cell_rect(128), None, "n={n}: fila 8, por abajo");
    }
}

#[test]
fn row_1_with_n_1_the_literal_numbers_of_the_spec() {
    let grid = Grid::new(1, 256, 16, 0);
    assert_eq!(grid.cell_rect(0), Some(rect(0, 0, 12, 12)));
    assert_eq!(grid.cell_rect(1), Some(rect(14, 0, 12, 12)));
    assert_eq!(grid.cell_rect(15), Some(rect(210, 0, 12, 12)));
    assert_eq!(grid.cell_rect(16), Some(rect(0, 14, 12, 12)));
    assert_eq!(grid.cell_rect(255), Some(rect(210, 210, 12, 12)));

    let grid = Grid::new(1, 256, 5, 3);
    assert_eq!(grid.cell_rect(48), Some(rect(0, 0, 12, 12)));
    assert_eq!(grid.cell_rect(111), Some(rect(210, 42, 12, 12)));
    assert_eq!(grid.cell_rect(112), Some(rect(0, 56, 12, 12)));
}

#[test]
fn cell_rect_uses_the_constants() {
    for n in SCALES {
        let k = i64::from(n);
        let grid = Grid::new(n, 256, 16, 0);
        for i in [0usize, 1, 15, 16, 17, 100, 255] {
            let (col, row_2) = ((i % COLUMNS) as i64, (i / COLUMNS) as i64);
            assert_eq!(
                grid.cell_rect(i),
                Some(rect(
                    col * PITCH_U * k,
                    row_2 * PITCH_U * k,
                    CELL_U * k,
                    CELL_U * k
                )),
                "n={n}, i={i}"
            );
        }
    }
}

// Fila 2: `index_at`.

#[test]
fn row_2_index_at_with_256_entries_and_16_rows() {
    for n in SCALES {
        let k = i64::from(n);
        let grid = Grid::new(n, 256, 16, 0);
        let cases: [(i64, i64, Option<usize>, &str); 11] = [
            (0, 0, Some(0), "esquina de la muestra 0"),
            (
                12 * k - 1,
                12 * k - 1,
                Some(0),
                "último píxel de la muestra 0",
            ),
            (12 * k, 0, None, "hueco a la derecha de la 0"),
            (14 * k - 1, 0, None, "último píxel del hueco"),
            (14 * k, 0, Some(1), "primer píxel de la muestra 1"),
            (12 * k, 12 * k, None, "esquina de huecos"),
            (-1, 0, None, "fuera por la izquierda"),
            (0, -1, None, "fuera por arriba"),
            (222 * k, 0, None, "la columna 15 acaba en 222n"),
            (224 * k, 0, None, "no hay columna 16"),
            (0, 224 * k, None, "no hay fila 16"),
        ];
        for (x, y, expected_2, that) in cases {
            assert_eq!(grid.index_at(x, y), expected_2, "n={n}, ({x}, {y}): {that}");
        }
    }
}

#[test]
fn row_2_index_at_with_9_and_with_1_entries() {
    for n in SCALES {
        let k = i64::from(n);
        let nine = Grid::new(n, 9, 16, 0);
        assert_eq!(nine.index_at(126 * k, 0), None, "n={n}: índice 9 >= 9");
        assert_eq!(nine.index_at(0, 14 * k), None, "n={n}: fila 1");
        // La última existente sí.
        assert_eq!(nine.index_at(112 * k, 0), Some(8), "n={n}");

        let a = Grid::new(n, 1, 16, 0);
        assert_eq!(a.index_at(0, 0), Some(0), "n={n}");
        assert_eq!(a.index_at(14 * k, 0), None, "n={n}");
    }
}

#[test]
fn row_2_with_n_1_the_literal_numbers_of_the_spec() {
    let grid = Grid::new(1, 256, 16, 0);
    assert_eq!(grid.index_at(0, 0), Some(0));
    assert_eq!(grid.index_at(11, 11), Some(0));
    assert_eq!(grid.index_at(12, 0), None);
    assert_eq!(grid.index_at(13, 0), None);
    assert_eq!(grid.index_at(14, 0), Some(1));
    assert_eq!(grid.index_at(12, 12), None);
    assert_eq!(grid.index_at(222, 0), None);
    assert_eq!(grid.index_at(224, 0), None);
    assert_eq!(grid.index_at(0, 224), None);
    assert_eq!(grid.index_at(210, 210), Some(255));
    assert_eq!(Grid::new(1, 9, 16, 0).index_at(126, 0), None);
    assert_eq!(Grid::new(1, 1, 16, 0).index_at(14, 0), None);
}

#[test]
fn index_at_and_cell_rect_respect_the_visible_rows() {
    for n in SCALES {
        let k = i64::from(n);
        // Con 3 filas visibles, la fila 5 no se ve (fila 7 de la spec, «la fila 5 con solo 3»).
        let grid = Grid::new(n, 256, 3, 0);
        assert_eq!(grid.cell_rect(5 * 16), None, "n={n}");
        assert_eq!(grid.index_at(0, 5 * 14 * k), None, "n={n}");
        assert_eq!(
            grid.index_at(0, 2 * 14 * k),
            Some(32),
            "n={n}: última fila visible"
        );
        assert_eq!(
            grid.index_at(0, 3 * 14 * k),
            None,
            "n={n}: primera fila oculta"
        );
        // Sin filas visibles no hay muestras.
        let nothing = Grid::new(n, 256, 0, 0);
        assert_eq!(nothing.cell_rect(0), None, "n={n}");
        assert_eq!(nothing.index_at(0, 0), None, "n={n}");
    }
}

// Fila 3: filas visibles y totales.

#[test]
fn row_3_visible_rows() {
    let n1 = [
        (0, 0usize),
        (-5, 0),
        (17, 0),
        (18, 1),
        (31, 1),
        (32, 2),
        (227, 15),
        (228, 16),
    ];
    for (height_2, expected_2) in n1 {
        assert_eq!(
            visible_rows(height_2, 1),
            expected_2,
            "n=1, alto {height_2}"
        );
    }
    for (height_2, expected_2) in [(35, 0usize), (36, 1)] {
        assert_eq!(
            visible_rows(height_2, 2),
            expected_2,
            "n=2, alto {height_2}"
        );
    }
    for (height_2, expected_2) in [(53, 0usize), (54, 1)] {
        assert_eq!(
            visible_rows(height_2, 3),
            expected_2,
            "n=3, alto {height_2}"
        );
    }
}

#[test]
fn row_3_visible_rows_comes_from_the_constants() {
    for n in SCALES {
        let k = i64::from(n);
        let source_2 = GRID_Y_U * k;
        let step = PITCH_U * k;
        for rows_2 in [0i64, 1, 2, 5, 16] {
            let just = source_2 + rows_2 * step;
            assert_eq!(visible_rows(just, n), rows_2 as usize, "n={n}");
            if rows_2 > 0 {
                assert_eq!(visible_rows(just - 1, n), rows_2 as usize - 1, "n={n}");
            }
        }
    }
}

#[test]
fn row_3_total_rows() {
    for (count, expected_2) in [
        (0usize, 0usize),
        (1, 1),
        (9, 1),
        (16, 1),
        (17, 2),
        (256, 16),
    ] {
        assert_eq!(total_rows(count), expected_2, "count {count}");
    }
}

#[test]
fn row_3_max_first_row() {
    for ((total, visible), expected_2) in [
        ((16usize, 5usize), 11usize),
        ((16, 16), 0),
        ((16, 20), 0),
        ((16, 0), 15),
        ((0, 5), 0),
    ] {
        assert_eq!(
            max_first_row(total, visible),
            expected_2,
            "total {total}, visibles {visible}"
        );
    }
}

// Fila 4: desplazamiento.

fn by(scroll: &mut Scroll, delta: i64, n: u32, total: usize, visible: usize) -> (usize, i64) {
    scroll.by(delta, n, total, visible);
    (scroll.first_row, scroll.rest)
}

#[test]
fn row_4_sequence_of_scrolls_with_n_1() {
    let mut s = Scroll::default();
    assert_eq!((s.first_row, s.rest), (0, 0));
    assert_eq!(by(&mut s, 7, 1, 16, 5), (0, 7));
    assert_eq!(by(&mut s, 7, 1, 16, 5), (1, 0));
    assert_eq!(by(&mut s, 21, 1, 16, 5), (2, 7));
    assert_eq!(by(&mut s, -8, 1, 16, 5), (1, 13));
    assert_eq!(by(&mut s, -100, 1, 16, 5), (0, 0));
    // Hacia abajo del todo: se limita y no queda resto.
    assert_eq!(by(&mut s, 1_000_000_000_000, 1, 16, 5), (11, 0));
    assert_eq!(by(&mut s, 1, 1, 16, 5), (11, 0));
    let before = s;
    assert_eq!(by(&mut s, 0, 1, 16, 5), (11, 0));
    assert_eq!(s, before, "delta 0: sin cambio");
    // Extremos: sin pánico ni desbordamiento.
    assert_eq!(by(&mut s, i64::MIN, 1, 16, 5), (0, 0));
    assert_eq!(by(&mut s, i64::MAX, 1, 16, 5), (11, 0));
}

#[test]
fn row_4_with_n_2_28_pixels_are_one_row() {
    let mut s = Scroll::default();
    assert_eq!(by(&mut s, 28, 2, 16, 5), (1, 0));
    let mut s = Scroll::default();
    assert_eq!(by(&mut s, 27, 2, 16, 5), (0, 27));
}

#[test]
fn row_4_clamp() {
    let mut s = Scroll {
        first_row: 11,
        rest: 0,
    };
    s.clamp(16, 12);
    assert_eq!(s.first_row, 4);

    // Lo que ya cumple no se mueve.
    let mut s = Scroll {
        first_row: 4,
        rest: 0,
    };
    s.clamp(16, 12);
    assert_eq!(s.first_row, 4);
    let mut s = Scroll {
        first_row: 3,
        rest: 0,
    };
    s.clamp(16, 12);
    assert_eq!(s.first_row, 3);
}

#[test]
fn row_4_thumb() {
    assert_eq!(thumb(0, 5, 16, 1), Some((0, 21)));
    assert_eq!(thumb(11, 5, 16, 1), Some((48, 21)));
    assert_eq!(thumb(0, 5, 16, 2), Some((0, 43)));
    assert_eq!(thumb(11, 5, 16, 2), Some((96, 43)));
    // Todo visible, menos total que visibles o sin filas visibles: no hay indicador.
    assert_eq!(thumb(0, 16, 16, 1), None);
    assert_eq!(thumb(0, 5, 4, 1), None);
    assert_eq!(thumb(0, 0, 16, 1), None);
    // Alto mínimo de 2n.
    assert_eq!(thumb(15, 1, 16, 1), Some((12, 2)));
}

// Fila 10: muestras y anillos.

fn colors_of_the_checker() -> [Rgba; 4] {
    [CHECK_A, CHECK_B, CHECK_B, CHECK_A]
}

/// Los cuatro cuadrados del damero de una muestra, en orden de lectura (por filas), sin
/// depender del orden de pintado que escoja la implementación.
fn squares_of_the_checker(parts: &[(ScreenRect, Rgba)], n: u32) -> Vec<(ScreenRect, Rgba)> {
    let k = i64::from(n);
    let mut squares: Vec<(ScreenRect, Rgba)> = parts[..4].to_vec();
    squares.sort_by_key(|(r, _)| (r.y, r.x));
    assert_eq!(squares.len(), 4);
    for (r, _) in &squares {
        assert_eq!((r.width, r.height), (6 * k, 6 * k), "n={n}");
    }
    squares
}

#[test]
fn row_10_opaque_swatch_is_a_single_rectangle() {
    for n in SCALES {
        let k = i64::from(n);
        let red = Rgba::opaque(0xff, 0, 0);
        assert_eq!(
            swatch_parts(n, red),
            vec![(rect(0, 0, 12 * k, 12 * k), red)],
            "n={n}"
        );
    }
}

#[test]
fn row_10_transparent_swatch_is_four_squares_without_color() {
    for n in SCALES {
        let k = i64::from(n);
        let parts = swatch_parts(n, Rgba::TRANSPARENT);
        assert_eq!(parts.len(), 4, "n={n}");
        let squares = squares_of_the_checker(&parts, n);
        let positions: Vec<(i64, i64)> = squares.iter().map(|(r, _)| (r.x, r.y)).collect();
        assert_eq!(
            positions,
            [(0, 0), (6 * k, 0), (0, 6 * k), (6 * k, 6 * k)],
            "n={n}"
        );
        let colors_2: Vec<Rgba> = squares.iter().map(|&(_, c)| c).collect();
        assert_eq!(colors_2, colors_of_the_checker(), "n={n}");
        // El orden de las partes, tal como lo devuelve: A, B, B, A.
        let in_order: Vec<Rgba> = parts.iter().map(|&(_, c)| c).collect();
        assert_eq!(in_order, colors_of_the_checker(), "n={n}");
    }
}

#[test]
fn row_10_swatch_with_partial_alpha_is_checker_and_the_color_on_top() {
    for n in SCALES {
        let k = i64::from(n);
        let base = swatch_parts(n, Rgba::TRANSPARENT);
        for alpha in [1u8, 128, 254] {
            let color = Rgba::new(0xff, 0, 0, alpha);
            let parts = swatch_parts(n, color);
            assert_eq!(parts.len(), 5, "n={n}, alfa {alpha}");
            assert_eq!(
                squares_of_the_checker(&parts, n),
                squares_of_the_checker(&base, n),
                "n={n}, alfa {alpha}: el mismo damero"
            );
            assert_eq!(
                parts[4],
                (rect(0, 0, 12 * k, 12 * k), color),
                "n={n}, alfa {alpha}: el color con su alfa encima"
            );
        }
    }
}

#[test]
fn row_10_swatch_with_the_alpha_of_the_spec_in_hexadecimal() {
    // `#ff000001`, `#ff000080` y `#ff0000fe`.
    for (hex, alpha) in [("#ff000001", 1u8), ("#ff000080", 128), ("#ff0000fe", 254)] {
        let color: Rgba = hex.parse().unwrap();
        assert_eq!(color.a, alpha);
        let parts = swatch_parts(1, color);
        assert_eq!(parts.len(), 5, "{hex}");
        assert_eq!(parts[4], (rect(0, 0, 12, 12), color), "{hex}");
    }
}

#[test]
fn row_10_the_transparent_entry_saved_opaque_is_painted_as_transparent() {
    let mut palette =
        Palette::from_colors([Rgba::opaque(0xff, 0, 0), Rgba::opaque(0, 0, 0xff)]).unwrap();
    palette.set_transparent_index(Some(0)).unwrap();
    let image = Image::new_indexed(2, 2, palette).unwrap();
    let color = pixel_rgba(&image, Pixel::Indexed(0)).unwrap();
    assert_eq!(color.a, 0, "el color resuelto de la transparente");
    for n in SCALES {
        assert_eq!(
            swatch_parts(n, color),
            swatch_parts(n, Rgba::TRANSPARENT),
            "n={n}: se ignora su RGB"
        );
    }
}

#[test]
fn row_10_ring_rects() {
    for n in SCALES {
        let k = i64::from(n);
        let cel = rect(0, 0, 10 * k, 10 * k);
        assert_eq!(
            ring_rects(cel, 2 * k),
            [
                rect(-2 * k, -2 * k, 14 * k, 2 * k),
                rect(-2 * k, 10 * k, 14 * k, 2 * k),
                rect(-2 * k, 0, 2 * k, 10 * k),
                rect(10 * k, 0, 2 * k, 10 * k),
            ],
            "n={n}, grosor 2n"
        );
        assert_eq!(
            ring_rects(cel, k),
            [
                rect(-k, -k, 12 * k, k),
                rect(-k, 10 * k, 12 * k, k),
                rect(-k, 0, k, 10 * k),
                rect(10 * k, 0, k, 10 * k),
            ],
            "n={n}, grosor n"
        );
    }
}

#[test]
fn row_10_ring_rects_accompanies_the_cell_wherever_it_is() {
    // Relativo a la esquina de la celda, esté donde esté.
    let cel = rect(28, 42, 10, 10);
    assert_eq!(
        ring_rects(cel, 2),
        [
            rect(26, 40, 14, 2),
            rect(26, 52, 14, 2),
            rect(26, 42, 2, 10),
            rect(38, 42, 2, 10),
        ]
    );
}

#[test]
fn row_10_the_rings_contrast_at_least_3_to_1_with_the_panel_background() {
    assert!(
        contrast_ratio(theme::RING_ACTIVE, theme::SURFACE_1) >= 3.0,
        "RING_ACTIVE: {}",
        contrast_ratio(theme::RING_ACTIVE, theme::SURFACE_1)
    );
    assert!(
        contrast_ratio(theme::RING_HOVER, theme::SURFACE_1) >= 3.0,
        "RING_HOVER: {}",
        contrast_ratio(theme::RING_HOVER, theme::SURFACE_1)
    );
}

// Fila 11 (parte geométrica): paletas límite.

#[test]
fn row_11_palette_of_one_entry() {
    assert_eq!(total_rows(1), 1);
    for n in SCALES {
        let k = i64::from(n);
        let grid = Grid::new(n, 1, 1, 0);
        assert_eq!(grid.cell_rect(0), Some(rect(0, 0, 12 * k, 12 * k)), "n={n}");
        assert_eq!(grid.cell_rect(1), None, "n={n}");
        // Una fila, sin pista (con cualquier número de filas visibles).
        for visible_2 in [1usize, 2, 16] {
            assert_eq!(
                thumb(0, visible_2, 1, n),
                None,
                "n={n}, visibles {visible_2}"
            );
        }
        assert_eq!(max_first_row(1, 1), 0);
    }
}

#[test]
fn row_11_palette_of_256_entries() {
    assert_eq!(total_rows(256), 16);
    for n in SCALES {
        let k = i64::from(n);
        let grid = Grid::new(n, 256, 16, 0);
        // La 255 está en la columna 15 y la fila 15.
        assert_eq!(
            grid.cell_rect(255),
            Some(rect(15 * 14 * k, 15 * 14 * k, 12 * k, 12 * k)),
            "n={n}"
        );
        assert_eq!(grid.index_at(15 * 14 * k, 15 * 14 * k), Some(255), "n={n}");
        assert_eq!(grid.cell_rect(256), None, "n={n}");
        // Con las 16 filas visibles no hay pista; con menos, sí.
        assert_eq!(thumb(0, 16, 16, n), None, "n={n}");
        assert!(thumb(0, 15, 16, n).is_some(), "n={n}");
    }
}

#[test]
fn row_11_the_nine_slots_of_the_modes_without_a_palette() {
    // RGBA, Gray y GrayAlpha: 9 muestras en una fila; las 9+ no existen.
    assert_eq!(total_rows(9), 1);
    for n in SCALES {
        let k = i64::from(n);
        let grid = Grid::new(n, 9, 16, 0);
        for i in 0..9usize {
            assert_eq!(
                grid.cell_rect(i),
                Some(rect(i as i64 * 14 * k, 0, 12 * k, 12 * k)),
                "n={n}, i={i}"
            );
            assert_eq!(grid.index_at(i as i64 * 14 * k, 0), Some(i), "n={n}, i={i}");
        }
        assert_eq!(grid.cell_rect(9), None, "n={n}");
        assert_eq!(grid.index_at(9 * 14 * k, 0), None, "n={n}");
        assert_eq!(grid.index_at(10 * 14 * k, 0), None, "n={n}");
        assert_eq!(grid.index_at(0, 14 * k), None, "n={n}: no hay fila 1");
        // Una sola fila: nada que desplazar.
        assert_eq!(thumb(0, 1, 1, n), None, "n={n}");
    }
}

// Propiedad 1: geometría.

fn coordinate(raw_2: i64, k: i64, width_2: i64, delta: i64, of_border: bool, n: i64) -> i64 {
    if of_border {
        // Alrededor de los bordes de las muestras y de los huecos.
        k * 14 * n + width_2 * 12 * n + delta
    } else {
        raw_2
    }
}

proptest! {
    #[test]
    fn property_1_index_at_and_cell_rect_are_inverses(
        n in 1u32..=8,
        count in 0usize..=256,
        visible_2 in 0usize..=20,
        row_raw in any::<usize>(),
        x_raw in -30i64..2000,
        y_raw in -30i64..2000,
        (kx, wx, dx, border_x) in (-1i64..=17, 0i64..=1, -2i64..=1, any::<bool>()),
        (ky, wy, dy, border_y) in (-1i64..=17, 0i64..=1, -2i64..=1, any::<bool>()),
    ) {
        let total = total_rows(count);
        let first_row = row_raw % (max_first_row(total, visible_2) + 1);
        let grid = Grid::new(n, count, visible_2, first_row);
        let k = i64::from(n);
        let x = coordinate(x_raw, kx, wx, dx, border_x, k);
        let y = coordinate(y_raw, ky, wy, dy, border_y, k);

        // Qué muestras contienen el punto (según `cell_rect`).
        let mut contain = Vec::new();
        for i in 0..count {
            if let Some(r) = grid.cell_rect(i) {
                // Rectangle of 12n x 12n inside the visible rows.
                prop_assert_eq!((r.width, r.height), (12 * k, 12 * k));
                let row_2 = i / COLUMNS;
                prop_assert!(row_2 >= first_row && row_2 < first_row + visible_2);
                if contains(r, x, y) {
                    contain.push(i);
                }
            } else {
                let row_2 = i / COLUMNS;
                prop_assert!(
                    row_2 < first_row || row_2 >= first_row + visible_2,
                    "la muestra {} está en una fila visible y no tiene rectángulo", i
                );
            }
        }
        // Ningún punto está en dos muestras.
        prop_assert!(contain.len() <= 1, "el punto ({}, {}) está en {:?}", x, y, contain);
        // `index_at` da `Some(i)` si y solo si el punto está en `cell_rect(i)`.
        prop_assert_eq!(grid.index_at(x, y), contain.first().copied());

        // Los huecos nunca dan muestra.
        let gap = x.rem_euclid(14 * k) >= 12 * k || y.rem_euclid(14 * k) >= 12 * k;
        if gap {
            prop_assert_eq!(grid.index_at(x, y), None);
        }

        // Las muestras que no existen no tienen rectángulo.
        prop_assert_eq!(grid.cell_rect(count), None);
    }

    #[test]
    fn property_1_each_visible_swatch_is_found_by_its_four_corners(
        n in 1u32..=8,
        count in 1usize..=256,
        visible_2 in 1usize..=20,
        row_raw in any::<usize>(),
    ) {
        let total = total_rows(count);
        let first_row = row_raw % (max_first_row(total, visible_2) + 1);
        let grid = Grid::new(n, count, visible_2, first_row);
        for i in 0..count {
            if let Some(r) = grid.cell_rect(i) {
                for (x, y) in [
                    (r.x, r.y),
                    (r.x + r.width - 1, r.y),
                    (r.x, r.y + r.height - 1),
                    (r.x + r.width - 1, r.y + r.height - 1),
                ] {
                    prop_assert_eq!(grid.index_at(x, y), Some(i));
                }
                // Justo fuera por cada lado: no es ella.
                for (x, y) in [
                    (r.x - 1, r.y),
                    (r.x + r.width, r.y),
                    (r.x, r.y - 1),
                    (r.x, r.y + r.height),
                ] {
                    prop_assert_ne!(grid.index_at(x, y), Some(i));
                }
            }
        }
    }
}

// Propiedad 2: desplazamiento.

fn delta() -> impl Strategy<Value = i64> {
    prop_oneof![
        any::<i64>(),
        Just(i64::MIN),
        Just(i64::MAX),
        Just(0i64),
        -300i64..300,
        -5000i64..5000,
    ]
}

proptest! {
    #[test]
    fn property_2_scroll_without_panic_and_within_the_limits(
        n in 1u32..=8,
        total in 0usize..=300,
        visible_2 in 0usize..=20,
        deltas in vec(delta(), 0..30),
    ) {
        let step = 14 * i128::from(n);
        let maximum_2 = max_first_row(total, visible_2);
        let mut scroll = Scroll::default();
        for d in deltas {
            let before = scroll;
            scroll.by(d, n, total, visible_2);

            prop_assert!(scroll.first_row <= maximum_2, "{:?} con máximo {}", scroll, maximum_2);
            prop_assert!(
                scroll.rest >= 0 && i128::from(scroll.rest) < step,
                "rest {} fuera de 0..{}", scroll.rest, step
            );
            if d >= 0 {
                prop_assert!(scroll.first_row >= before.first_row);
            }
            if d <= 0 {
                prop_assert!(scroll.first_row <= before.first_row);
            }
            // Sin nada más que acumular en la última fila.
            if scroll.first_row == maximum_2 {
                prop_assert_eq!(scroll.rest, 0);
            }

            // Modelo con `i128`.
            let sum = i128::from(before.rest) + i128::from(d);
            let rows_2 = before.first_row as i128 + sum.div_euclid(step);
            let (row_2, rest_2) = if rows_2 < 0 || rows_2 > maximum_2 as i128 {
                (rows_2.clamp(0, maximum_2 as i128), 0)
            } else if rows_2 == maximum_2 as i128 {
                (rows_2, 0)
            } else {
                (rows_2, sum.rem_euclid(step))
            };
            prop_assert_eq!(scroll.first_row as i128, row_2);
            prop_assert_eq!(i128::from(scroll.rest), rest_2);

            // El indicador queda dentro de la pista.
            if let Some((y, height_2)) = thumb(scroll.first_row, visible_2, total, n) {
                prop_assert!(y >= 0);
                prop_assert!(height_2 >= 2 * i64::from(n));
                prop_assert!(
                    i128::from(y) + i128::from(height_2) <= visible_2 as i128 * step,
                    "y {} + alto {} sobrepasa la pista de {}", y, height_2, visible_2 as i128 * step
                );
            } else {
                prop_assert!(total <= visible_2 || visible_2 == 0);
            }
        }
    }

    #[test]
    fn property_2_clamp_leaves_the_first_row_inside_and_is_idempotent(
        total in 0usize..=300,
        visible_2 in 0usize..=20,
        first in 0usize..=400,
        rest_2 in 0i64..14,
    ) {
        let mut scroll = Scroll { first_row: first, rest: rest_2 };
        scroll.clamp(total, visible_2);
        prop_assert!(scroll.first_row <= max_first_row(total, visible_2));
        if first <= max_first_row(total, visible_2) {
            prop_assert_eq!(scroll.first_row, first);
        }
        let a_time = scroll;
        scroll.clamp(total, visible_2);
        prop_assert_eq!(scroll, a_time);
    }

    #[test]
    fn property_2_thumb_inside_the_track_for_any_valid_row(
        n in 1u32..=8,
        total in 0usize..=300,
        visible_2 in 0usize..=20,
        first_raw in any::<usize>(),
    ) {
        let first = first_raw % (max_first_row(total, visible_2) + 1);
        let step = 14 * i64::from(n);
        match thumb(first, visible_2, total, n) {
            None => prop_assert!(total <= visible_2 || visible_2 == 0),
            Some((y, height_2)) => {
                prop_assert!(total > visible_2 && visible_2 > 0);
                prop_assert!(y >= 0 && height_2 >= 2 * i64::from(n));
                prop_assert!(y + height_2 <= visible_2 as i64 * step);
            }
        }
    }
}
