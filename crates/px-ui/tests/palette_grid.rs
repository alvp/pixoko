//! Tests de `palette_grid` (specs/hito-2-panel-paletas.md, unidad 11b): la maqueta del panel de
//! paletas, sin tipos de `egui`.
//!
//! Cubre las filas 1 a 4 y 10 de «Casos límite», la parte geométrica de la fila 11 (paletas de 1
//! y 256 entradas y las 9 ranuras de los modos sin paleta) y las propiedades 1 (geometría) y 2
//! (desplazamiento). Las medidas están en «píxeles de interfaz» `u` = `n` píxeles físicos, con
//! `n` = 1, 2 y 3 donde la spec lo dice.
//!
//! Los números esperados salen de la spec. Se usan las constantes de `palette_grid` cuando es
//! natural, pero `las_constantes_valen_lo_que_dice_la_spec` fija los valores literales para que
//! cambiar una constante rompa un test. El modelo del desplazamiento (propiedad 2) usa `i128`
//! y `div_euclid`, sin las fórmulas de la implementación.

use proptest::collection::vec;
use proptest::prelude::*;
use px_core::{Image, Palette, Pixel, Rgba};
use px_ui::palette_grid::{
    CELL_U, CHECK_A, CHECK_B, CHECK_U, COLUMNS, GAP_U, GRID_X_U, GRID_Y_U, Grid, LINE_H_U,
    LINE1_Y_U, LINE2_Y_U, PANEL_WIDTH_U, PITCH_U, RIGHT_LINE_X_U, RING_ACTIVE_U, RING_HOVER_U,
    SEPARATOR_Y_U, Scroll, TEXT_WIDTH_U, TEXT_X_U, TRACK_W_U, TRACK_X_U, edges_u, max_first_row,
    ring_rects, swatch_parts, thumb, total_rows, visible_rows,
};
use px_ui::texture::pixel_rgba;
use px_ui::theme::{self, contrast_ratio};
use px_ui::view::ScreenRect;

const ESCALAS: [u32; 3] = [1, 2, 3];

fn rect(x: i64, y: i64, width: i64, height: i64) -> ScreenRect {
    ScreenRect {
        x,
        y,
        width,
        height,
    }
}

fn contiene(r: ScreenRect, x: i64, y: i64) -> bool {
    x >= r.x && x < r.x + r.width && y >= r.y && y < r.y + r.height
}

// Constantes.

#[test]
fn las_constantes_valen_lo_que_dice_la_spec() {
    assert_eq!(COLUMNS, 16usize);
    assert_eq!(CELL_U, 10);
    assert_eq!(GAP_U, 4);
    assert_eq!(PITCH_U, 14);
    assert_eq!(CHECK_U, 5);
    assert_eq!(PANEL_WIDTH_U, 233);
    assert_eq!(GRID_X_U, 4);
    assert_eq!(GRID_Y_U, 77);
    assert_eq!(TEXT_X_U, 4);
    assert_eq!(TEXT_WIDTH_U, 225);
    assert_eq!(LINE_H_U, 14);
    assert_eq!(LINE1_Y_U, 40);
    assert_eq!(LINE2_Y_U, 54);
    assert_eq!(SEPARATOR_Y_U, 72);
    assert_eq!(TRACK_X_U, 230);
    assert_eq!(TRACK_W_U, 2);
    assert_eq!(RIGHT_LINE_X_U, 232);
    assert_eq!(RING_ACTIVE_U, 2);
    assert_eq!(RING_HOVER_U, 1);
    assert_eq!(CHECK_A, Rgba::opaque(0x4a, 0x4a, 0x4a));
    assert_eq!(CHECK_B, Rgba::opaque(0x6e, 0x6e, 0x6e));
}

#[test]
fn las_constantes_son_coherentes_entre_si() {
    assert_eq!(PITCH_U, CELL_U + GAP_U);
    assert_eq!(CHECK_U * 2, CELL_U);
    assert_eq!(
        LINE_H_U, PITCH_U,
        "el paso es el alto de una línea de texto"
    );
    // 4 (margen) + 224 (16 pasos) + 2 + 2 (pista) + 1 (trazo derecho).
    assert_eq!(
        PANEL_WIDTH_U,
        GRID_X_U + COLUMNS as i64 * PITCH_U + 2 + TRACK_W_U + 1
    );
    assert_eq!(RIGHT_LINE_X_U, PANEL_WIDTH_U - 1);
    assert_eq!(TRACK_X_U + TRACK_W_U, RIGHT_LINE_X_U);
}

#[test]
fn edges_u_contiene_los_bordes_de_la_maqueta() {
    let bordes = edges_u();
    assert!(!bordes.is_empty());
    // Los únicos negativos posibles son los de los anillos (-2).
    for &borde in &bordes {
        assert!(
            borde >= 0 || borde == -2,
            "borde {borde}: solo se admite -2 como negativo"
        );
    }
    for (nombre, esperado) in [
        ("PANEL_WIDTH_U", PANEL_WIDTH_U),
        ("GRID_X_U", GRID_X_U),
        ("GRID_Y_U", GRID_Y_U),
        ("SEPARATOR_Y_U", SEPARATOR_Y_U),
    ] {
        assert!(
            bordes.contains(&esperado),
            "falta {nombre} = {esperado} en {bordes:?}"
        );
    }
    // Y con los valores literales de la spec.
    for esperado in [233, 4, 77, 72] {
        assert!(bordes.contains(&esperado), "falta {esperado}");
    }
}

// Fila 1: `cell_rect`.

#[test]
fn fila_1_cell_rect_con_256_entradas_y_16_filas() {
    for n in ESCALAS {
        let k = i64::from(n);
        let grid = Grid::new(n, 256, 16, 0);
        assert_eq!(grid.cell_rect(0), Some(rect(0, 0, 10 * k, 10 * k)), "n={n}");
        assert_eq!(
            grid.cell_rect(1),
            Some(rect(14 * k, 0, 10 * k, 10 * k)),
            "n={n}"
        );
        assert_eq!(
            grid.cell_rect(15),
            Some(rect(210 * k, 0, 10 * k, 10 * k)),
            "n={n}"
        );
        assert_eq!(
            grid.cell_rect(16),
            Some(rect(0, 14 * k, 10 * k, 10 * k)),
            "n={n}"
        );
        assert_eq!(
            grid.cell_rect(255),
            Some(rect(210 * k, 210 * k, 10 * k, 10 * k)),
            "n={n}"
        );
        assert_eq!(grid.cell_rect(256), None, "n={n}");
    }
}

#[test]
fn fila_1_cell_rect_con_desplazamiento_de_3_filas_y_5_visibles() {
    for n in ESCALAS {
        let k = i64::from(n);
        let grid = Grid::new(n, 256, 5, 3);
        assert_eq!(grid.cell_rect(47), None, "n={n}: fila 2, por arriba");
        assert_eq!(
            grid.cell_rect(48),
            Some(rect(0, 0, 10 * k, 10 * k)),
            "n={n}"
        );
        assert_eq!(
            grid.cell_rect(111),
            Some(rect(210 * k, 42 * k, 10 * k, 10 * k)),
            "n={n}"
        );
        assert_eq!(
            grid.cell_rect(112),
            Some(rect(0, 56 * k, 10 * k, 10 * k)),
            "n={n}"
        );
        assert_eq!(grid.cell_rect(128), None, "n={n}: fila 8, por abajo");
    }
}

#[test]
fn fila_1_con_n_1_los_numeros_literales_de_la_spec() {
    let grid = Grid::new(1, 256, 16, 0);
    assert_eq!(grid.cell_rect(0), Some(rect(0, 0, 10, 10)));
    assert_eq!(grid.cell_rect(1), Some(rect(14, 0, 10, 10)));
    assert_eq!(grid.cell_rect(15), Some(rect(210, 0, 10, 10)));
    assert_eq!(grid.cell_rect(16), Some(rect(0, 14, 10, 10)));
    assert_eq!(grid.cell_rect(255), Some(rect(210, 210, 10, 10)));

    let grid = Grid::new(1, 256, 5, 3);
    assert_eq!(grid.cell_rect(48), Some(rect(0, 0, 10, 10)));
    assert_eq!(grid.cell_rect(111), Some(rect(210, 42, 10, 10)));
    assert_eq!(grid.cell_rect(112), Some(rect(0, 56, 10, 10)));
}

#[test]
fn cell_rect_usa_las_constantes() {
    for n in ESCALAS {
        let k = i64::from(n);
        let grid = Grid::new(n, 256, 16, 0);
        for i in [0usize, 1, 15, 16, 17, 100, 255] {
            let (col, fila) = ((i % COLUMNS) as i64, (i / COLUMNS) as i64);
            assert_eq!(
                grid.cell_rect(i),
                Some(rect(
                    col * PITCH_U * k,
                    fila * PITCH_U * k,
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
fn fila_2_index_at_con_256_entradas_y_16_filas() {
    for n in ESCALAS {
        let k = i64::from(n);
        let grid = Grid::new(n, 256, 16, 0);
        let casos: [(i64, i64, Option<usize>, &str); 11] = [
            (0, 0, Some(0), "esquina de la muestra 0"),
            (
                10 * k - 1,
                10 * k - 1,
                Some(0),
                "último píxel de la muestra 0",
            ),
            (10 * k, 0, None, "hueco a la derecha de la 0"),
            (14 * k - 1, 0, None, "último píxel del hueco"),
            (14 * k, 0, Some(1), "primer píxel de la muestra 1"),
            (10 * k, 10 * k, None, "esquina de huecos"),
            (-1, 0, None, "fuera por la izquierda"),
            (0, -1, None, "fuera por arriba"),
            (220 * k, 0, None, "la columna 15 acaba en 220n"),
            (224 * k, 0, None, "no hay columna 16"),
            (0, 224 * k, None, "no hay fila 16"),
        ];
        for (x, y, esperado, que) in casos {
            assert_eq!(grid.index_at(x, y), esperado, "n={n}, ({x}, {y}): {que}");
        }
    }
}

#[test]
fn fila_2_index_at_con_9_y_con_1_entradas() {
    for n in ESCALAS {
        let k = i64::from(n);
        let nueve = Grid::new(n, 9, 16, 0);
        assert_eq!(nueve.index_at(126 * k, 0), None, "n={n}: índice 9 >= 9");
        assert_eq!(nueve.index_at(0, 14 * k), None, "n={n}: fila 1");
        // La última existente sí.
        assert_eq!(nueve.index_at(112 * k, 0), Some(8), "n={n}");

        let una = Grid::new(n, 1, 16, 0);
        assert_eq!(una.index_at(0, 0), Some(0), "n={n}");
        assert_eq!(una.index_at(14 * k, 0), None, "n={n}");
    }
}

#[test]
fn fila_2_con_n_1_los_numeros_literales_de_la_spec() {
    let grid = Grid::new(1, 256, 16, 0);
    assert_eq!(grid.index_at(0, 0), Some(0));
    assert_eq!(grid.index_at(9, 9), Some(0));
    assert_eq!(grid.index_at(10, 0), None);
    assert_eq!(grid.index_at(13, 0), None);
    assert_eq!(grid.index_at(14, 0), Some(1));
    assert_eq!(grid.index_at(10, 10), None);
    assert_eq!(grid.index_at(220, 0), None);
    assert_eq!(grid.index_at(224, 0), None);
    assert_eq!(grid.index_at(0, 224), None);
    assert_eq!(grid.index_at(210, 210), Some(255));
    assert_eq!(Grid::new(1, 9, 16, 0).index_at(126, 0), None);
    assert_eq!(Grid::new(1, 1, 16, 0).index_at(14, 0), None);
}

#[test]
fn index_at_y_cell_rect_respetan_las_filas_visibles() {
    for n in ESCALAS {
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
        let nada = Grid::new(n, 256, 0, 0);
        assert_eq!(nada.cell_rect(0), None, "n={n}");
        assert_eq!(nada.index_at(0, 0), None, "n={n}");
    }
}

// Fila 3: filas visibles y totales.

#[test]
fn fila_3_visible_rows() {
    let n1 = [
        (0, 0usize),
        (-5, 0),
        (90, 0),
        (91, 1),
        (104, 1),
        (105, 2),
        (300, 15),
        (301, 16),
    ];
    for (alto, esperado) in n1 {
        assert_eq!(visible_rows(alto, 1), esperado, "n=1, alto {alto}");
    }
    for (alto, esperado) in [(181, 0usize), (182, 1)] {
        assert_eq!(visible_rows(alto, 2), esperado, "n=2, alto {alto}");
    }
    for (alto, esperado) in [(272, 0usize), (273, 1)] {
        assert_eq!(visible_rows(alto, 3), esperado, "n=3, alto {alto}");
    }
}

#[test]
fn fila_3_visible_rows_sale_de_las_constantes() {
    for n in ESCALAS {
        let k = i64::from(n);
        let origen = GRID_Y_U * k;
        let paso = PITCH_U * k;
        for filas in [0i64, 1, 2, 5, 16] {
            let justo = origen + filas * paso;
            assert_eq!(visible_rows(justo, n), filas as usize, "n={n}");
            if filas > 0 {
                assert_eq!(visible_rows(justo - 1, n), filas as usize - 1, "n={n}");
            }
        }
    }
}

#[test]
fn fila_3_total_rows() {
    for (count, esperado) in [
        (0usize, 0usize),
        (1, 1),
        (9, 1),
        (16, 1),
        (17, 2),
        (256, 16),
    ] {
        assert_eq!(total_rows(count), esperado, "count {count}");
    }
}

#[test]
fn fila_3_max_first_row() {
    for ((total, visible), esperado) in [
        ((16usize, 5usize), 11usize),
        ((16, 16), 0),
        ((16, 20), 0),
        ((16, 0), 15),
        ((0, 5), 0),
    ] {
        assert_eq!(
            max_first_row(total, visible),
            esperado,
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
fn fila_4_secuencia_de_desplazamientos_con_n_1() {
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
    let antes = s;
    assert_eq!(by(&mut s, 0, 1, 16, 5), (11, 0));
    assert_eq!(s, antes, "delta 0: sin cambio");
    // Extremos: sin pánico ni desbordamiento.
    assert_eq!(by(&mut s, i64::MIN, 1, 16, 5), (0, 0));
    assert_eq!(by(&mut s, i64::MAX, 1, 16, 5), (11, 0));
}

#[test]
fn fila_4_con_n_2_28_pixeles_son_una_fila() {
    let mut s = Scroll::default();
    assert_eq!(by(&mut s, 28, 2, 16, 5), (1, 0));
    let mut s = Scroll::default();
    assert_eq!(by(&mut s, 27, 2, 16, 5), (0, 27));
}

#[test]
fn fila_4_clamp() {
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
fn fila_4_thumb() {
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

fn colores_del_damero() -> [Rgba; 4] {
    [CHECK_A, CHECK_B, CHECK_B, CHECK_A]
}

/// Los cuatro cuadrados del damero de una muestra, en orden de lectura (por filas), sin
/// depender del orden de pintado que escoja la implementación.
fn cuadrados_del_damero(partes: &[(ScreenRect, Rgba)], n: u32) -> Vec<(ScreenRect, Rgba)> {
    let k = i64::from(n);
    let mut cuadrados: Vec<(ScreenRect, Rgba)> = partes[..4].to_vec();
    cuadrados.sort_by_key(|(r, _)| (r.y, r.x));
    assert_eq!(cuadrados.len(), 4);
    for (r, _) in &cuadrados {
        assert_eq!((r.width, r.height), (5 * k, 5 * k), "n={n}");
    }
    cuadrados
}

#[test]
fn fila_10_swatch_opaco_es_un_solo_rectangulo() {
    for n in ESCALAS {
        let k = i64::from(n);
        let rojo = Rgba::opaque(0xff, 0, 0);
        assert_eq!(
            swatch_parts(n, rojo),
            vec![(rect(0, 0, 10 * k, 10 * k), rojo)],
            "n={n}"
        );
    }
}

#[test]
fn fila_10_swatch_transparente_son_cuatro_cuadrados_sin_color() {
    for n in ESCALAS {
        let k = i64::from(n);
        let partes = swatch_parts(n, Rgba::TRANSPARENT);
        assert_eq!(partes.len(), 4, "n={n}");
        let cuadrados = cuadrados_del_damero(&partes, n);
        let posiciones: Vec<(i64, i64)> = cuadrados.iter().map(|(r, _)| (r.x, r.y)).collect();
        assert_eq!(
            posiciones,
            [(0, 0), (5 * k, 0), (0, 5 * k), (5 * k, 5 * k)],
            "n={n}"
        );
        let colores: Vec<Rgba> = cuadrados.iter().map(|&(_, c)| c).collect();
        assert_eq!(colores, colores_del_damero(), "n={n}");
        // El orden de las partes, tal como lo devuelve: A, B, B, A.
        let en_orden: Vec<Rgba> = partes.iter().map(|&(_, c)| c).collect();
        assert_eq!(en_orden, colores_del_damero(), "n={n}");
    }
}

#[test]
fn fila_10_swatch_con_alfa_parcial_es_damero_y_encima_el_color() {
    for n in ESCALAS {
        let k = i64::from(n);
        let base = swatch_parts(n, Rgba::TRANSPARENT);
        for alfa in [1u8, 128, 254] {
            let color = Rgba::new(0xff, 0, 0, alfa);
            let partes = swatch_parts(n, color);
            assert_eq!(partes.len(), 5, "n={n}, alfa {alfa}");
            assert_eq!(
                cuadrados_del_damero(&partes, n),
                cuadrados_del_damero(&base, n),
                "n={n}, alfa {alfa}: el mismo damero"
            );
            assert_eq!(
                partes[4],
                (rect(0, 0, 10 * k, 10 * k), color),
                "n={n}, alfa {alfa}: el color con su alfa encima"
            );
        }
    }
}

#[test]
fn fila_10_swatch_con_el_alfa_de_la_spec_en_hexadecimal() {
    // `#ff000001`, `#ff000080` y `#ff0000fe`.
    for (hex, alfa) in [("#ff000001", 1u8), ("#ff000080", 128), ("#ff0000fe", 254)] {
        let color: Rgba = hex.parse().unwrap();
        assert_eq!(color.a, alfa);
        let partes = swatch_parts(1, color);
        assert_eq!(partes.len(), 5, "{hex}");
        assert_eq!(partes[4], (rect(0, 0, 10, 10), color), "{hex}");
    }
}

#[test]
fn fila_10_la_entrada_transparente_guardada_opaca_se_pinta_como_transparente() {
    let mut paleta =
        Palette::from_colors([Rgba::opaque(0xff, 0, 0), Rgba::opaque(0, 0, 0xff)]).unwrap();
    paleta.set_transparent_index(Some(0)).unwrap();
    let image = Image::new_indexed(2, 2, paleta).unwrap();
    let color = pixel_rgba(&image, Pixel::Indexed(0)).unwrap();
    assert_eq!(color.a, 0, "el color resuelto de la transparente");
    for n in ESCALAS {
        assert_eq!(
            swatch_parts(n, color),
            swatch_parts(n, Rgba::TRANSPARENT),
            "n={n}: se ignora su RGB"
        );
    }
}

#[test]
fn fila_10_ring_rects() {
    for n in ESCALAS {
        let k = i64::from(n);
        let celda = rect(0, 0, 10 * k, 10 * k);
        assert_eq!(
            ring_rects(celda, 2 * k),
            [
                rect(-2 * k, -2 * k, 14 * k, 2 * k),
                rect(-2 * k, 10 * k, 14 * k, 2 * k),
                rect(-2 * k, 0, 2 * k, 10 * k),
                rect(10 * k, 0, 2 * k, 10 * k),
            ],
            "n={n}, grosor 2n"
        );
        assert_eq!(
            ring_rects(celda, k),
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
fn fila_10_ring_rects_acompana_a_la_celda_donde_este() {
    // Relativo a la esquina de la celda, esté donde esté.
    let celda = rect(28, 42, 10, 10);
    assert_eq!(
        ring_rects(celda, 2),
        [
            rect(26, 40, 14, 2),
            rect(26, 52, 14, 2),
            rect(26, 42, 2, 10),
            rect(38, 42, 2, 10),
        ]
    );
}

#[test]
fn fila_10_los_anillos_contrastan_al_menos_3_a_1_con_el_fondo_del_panel() {
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
fn fila_11_paleta_de_una_entrada() {
    assert_eq!(total_rows(1), 1);
    for n in ESCALAS {
        let k = i64::from(n);
        let grid = Grid::new(n, 1, 1, 0);
        assert_eq!(grid.cell_rect(0), Some(rect(0, 0, 10 * k, 10 * k)), "n={n}");
        assert_eq!(grid.cell_rect(1), None, "n={n}");
        // Una fila, sin pista (con cualquier número de filas visibles).
        for visibles in [1usize, 2, 16] {
            assert_eq!(thumb(0, visibles, 1, n), None, "n={n}, visibles {visibles}");
        }
        assert_eq!(max_first_row(1, 1), 0);
    }
}

#[test]
fn fila_11_paleta_de_256_entradas() {
    assert_eq!(total_rows(256), 16);
    for n in ESCALAS {
        let k = i64::from(n);
        let grid = Grid::new(n, 256, 16, 0);
        // La 255 está en la columna 15 y la fila 15.
        assert_eq!(
            grid.cell_rect(255),
            Some(rect(15 * 14 * k, 15 * 14 * k, 10 * k, 10 * k)),
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
fn fila_11_las_nueve_ranuras_de_los_modos_sin_paleta() {
    // RGBA, Gray y GrayAlpha: 9 muestras en una fila; las 9+ no existen.
    assert_eq!(total_rows(9), 1);
    for n in ESCALAS {
        let k = i64::from(n);
        let grid = Grid::new(n, 9, 16, 0);
        for i in 0..9usize {
            assert_eq!(
                grid.cell_rect(i),
                Some(rect(i as i64 * 14 * k, 0, 10 * k, 10 * k)),
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

fn coordenada(crudo: i64, k: i64, ancho: i64, delta: i64, de_borde: bool, n: i64) -> i64 {
    if de_borde {
        // Alrededor de los bordes de las muestras y de los huecos.
        k * 14 * n + ancho * 10 * n + delta
    } else {
        crudo
    }
}

proptest! {
    #[test]
    fn propiedad_1_index_at_y_cell_rect_son_inversas(
        n in 1u32..=8,
        count in 0usize..=256,
        visibles in 0usize..=20,
        fila_cruda in any::<usize>(),
        x_crudo in -30i64..2000,
        y_crudo in -30i64..2000,
        (kx, wx, dx, borde_x) in (-1i64..=17, 0i64..=1, -2i64..=1, any::<bool>()),
        (ky, wy, dy, borde_y) in (-1i64..=17, 0i64..=1, -2i64..=1, any::<bool>()),
    ) {
        let total = total_rows(count);
        let first_row = fila_cruda % (max_first_row(total, visibles) + 1);
        let grid = Grid::new(n, count, visibles, first_row);
        let k = i64::from(n);
        let x = coordenada(x_crudo, kx, wx, dx, borde_x, k);
        let y = coordenada(y_crudo, ky, wy, dy, borde_y, k);

        // Qué muestras contienen el punto (según `cell_rect`).
        let mut contienen = Vec::new();
        for i in 0..count {
            if let Some(r) = grid.cell_rect(i) {
                // Rectángulo de 10n x 10n dentro de las filas visibles.
                prop_assert_eq!((r.width, r.height), (10 * k, 10 * k));
                let fila = i / COLUMNS;
                prop_assert!(fila >= first_row && fila < first_row + visibles);
                if contiene(r, x, y) {
                    contienen.push(i);
                }
            } else {
                let fila = i / COLUMNS;
                prop_assert!(
                    fila < first_row || fila >= first_row + visibles,
                    "la muestra {} está en una fila visible y no tiene rectángulo", i
                );
            }
        }
        // Ningún punto está en dos muestras.
        prop_assert!(contienen.len() <= 1, "el punto ({}, {}) está en {:?}", x, y, contienen);
        // `index_at` da `Some(i)` si y solo si el punto está en `cell_rect(i)`.
        prop_assert_eq!(grid.index_at(x, y), contienen.first().copied());

        // Los huecos nunca dan muestra.
        let hueco = x.rem_euclid(14 * k) >= 10 * k || y.rem_euclid(14 * k) >= 10 * k;
        if hueco {
            prop_assert_eq!(grid.index_at(x, y), None);
        }

        // Las muestras que no existen no tienen rectángulo.
        prop_assert_eq!(grid.cell_rect(count), None);
    }

    #[test]
    fn propiedad_1_cada_muestra_visible_se_encuentra_por_sus_cuatro_esquinas(
        n in 1u32..=8,
        count in 1usize..=256,
        visibles in 1usize..=20,
        fila_cruda in any::<usize>(),
    ) {
        let total = total_rows(count);
        let first_row = fila_cruda % (max_first_row(total, visibles) + 1);
        let grid = Grid::new(n, count, visibles, first_row);
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
    fn propiedad_2_desplazamiento_sin_panico_y_dentro_de_los_limites(
        n in 1u32..=8,
        total in 0usize..=300,
        visibles in 0usize..=20,
        deltas in vec(delta(), 0..30),
    ) {
        let paso = 14 * i128::from(n);
        let maximo = max_first_row(total, visibles);
        let mut scroll = Scroll::default();
        for d in deltas {
            let antes = scroll;
            scroll.by(d, n, total, visibles);

            prop_assert!(scroll.first_row <= maximo, "{:?} con máximo {}", scroll, maximo);
            prop_assert!(
                scroll.rest >= 0 && i128::from(scroll.rest) < paso,
                "rest {} fuera de 0..{}", scroll.rest, paso
            );
            if d >= 0 {
                prop_assert!(scroll.first_row >= antes.first_row);
            }
            if d <= 0 {
                prop_assert!(scroll.first_row <= antes.first_row);
            }
            // Sin nada más que acumular en la última fila.
            if scroll.first_row == maximo {
                prop_assert_eq!(scroll.rest, 0);
            }

            // Modelo con `i128`.
            let suma = i128::from(antes.rest) + i128::from(d);
            let filas = antes.first_row as i128 + suma.div_euclid(paso);
            let (fila, resto) = if filas < 0 || filas > maximo as i128 {
                (filas.clamp(0, maximo as i128), 0)
            } else if filas == maximo as i128 {
                (filas, 0)
            } else {
                (filas, suma.rem_euclid(paso))
            };
            prop_assert_eq!(scroll.first_row as i128, fila);
            prop_assert_eq!(i128::from(scroll.rest), resto);

            // El indicador queda dentro de la pista.
            if let Some((y, alto)) = thumb(scroll.first_row, visibles, total, n) {
                prop_assert!(y >= 0);
                prop_assert!(alto >= 2 * i64::from(n));
                prop_assert!(
                    i128::from(y) + i128::from(alto) <= visibles as i128 * paso,
                    "y {} + alto {} sobrepasa la pista de {}", y, alto, visibles as i128 * paso
                );
            } else {
                prop_assert!(total <= visibles || visibles == 0);
            }
        }
    }

    #[test]
    fn propiedad_2_clamp_deja_la_primera_fila_dentro_y_es_idempotente(
        total in 0usize..=300,
        visibles in 0usize..=20,
        primera in 0usize..=400,
        resto in 0i64..14,
    ) {
        let mut scroll = Scroll { first_row: primera, rest: resto };
        scroll.clamp(total, visibles);
        prop_assert!(scroll.first_row <= max_first_row(total, visibles));
        if primera <= max_first_row(total, visibles) {
            prop_assert_eq!(scroll.first_row, primera);
        }
        let una_vez = scroll;
        scroll.clamp(total, visibles);
        prop_assert_eq!(scroll, una_vez);
    }

    #[test]
    fn propiedad_2_thumb_dentro_de_la_pista_para_cualquier_fila_valida(
        n in 1u32..=8,
        total in 0usize..=300,
        visibles in 0usize..=20,
        primera_cruda in any::<usize>(),
    ) {
        let primera = primera_cruda % (max_first_row(total, visibles) + 1);
        let paso = 14 * i64::from(n);
        match thumb(primera, visibles, total, n) {
            None => prop_assert!(total <= visibles || visibles == 0),
            Some((y, alto)) => {
                prop_assert!(total > visibles && visibles > 0);
                prop_assert!(y >= 0 && alto >= 2 * i64::from(n));
                prop_assert!(y + alto <= visibles as i64 * paso);
            }
        }
    }
}
