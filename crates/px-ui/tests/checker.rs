//! Tests de la lógica pura del damero de transparencia y del fondo elegible (specs/hito-2-damero-
//! lienzo.md, «Casos» filas 1 a 4, 9, 10 y la parte pura de la 15, y propiedad 1): `px_ui::checker`
//! y `px_ui::background` no usan tipos de egui, así que aquí no hay ventana ni banco. Las filas de
//! lienzo (5 a 8, 14 y la parte de lienzo de la 15) están en `checker_canvas.rs`; el menú y los
//! bloqueos (12 y 13), en `background_menu.rs`.
//!
//! - Fila 1: `CELL_U`, `cell_px`, `Shade::color`, `texels` y `BACKGROUND`.
//! - Fila 2: `shade_at` con la tabla de la spec, con negativos, con origen desplazado y con i64
//!   enormes contra la definición por `rem_euclid` con período `2·cell`.
//! - Fila 3: `quad` (a, b, c, d) y `clip` (e).
//! - Fila 4: `cells` (a a e).
//! - Fila 9: `CanvasBackground`, `Fill`, `BLACK`, `WHITE` y `background_fill`.
//! - Fila 10: coherencia con `swatch_parts` y con las constantes `CHECK_A` y `CHECK_B`.
//! - Fila 15 (parte pura): `ENV_MESH` y `mesh_fallback_enabled`.
//! - Propiedad 1: `quad`, `shade_at` y `cells` son la misma función (512 casos).
//!
//! Suposiciones sobre las firmas de «Piezas»: `Cell` y `CheckerQuad` comparan con `==` (derives
//! `Debug` y `PartialEq`), `Shade`, `CanvasBackground` y `Fill` son `Clone, Copy, Debug, PartialEq,
//! Eq` y `Rgba` es `Debug` y `PartialEq`. Los uv de `CheckerQuad` se comparan **exactos**: all_2 los
//! valores de la spec son múltiplos de 1/16 y se representan sin error en `f32`.
//!
//! Unidad 19b (specs/hito-2-fondo-color.md): la fila 1 (`CanvasBackground::Custom`, `custom`,
//! `solid_color`, `background_value_text` y `background_fill` de un color propio) y la parte pura de
//! la propiedad 1 (`custom(c)` es `Some` si y solo si el alfa es 255, `background_value_text` se lee
//! de vuelta con `parse_hex` y `background_fill` nunca lleva alfa < 255) están al final del fichero.
//!
//! Los oráculos (paridad de las casillas, fase, número de casillas por eje) están escritos aquí con
//! `rem_euclid` y `div_euclid`, sin llamar a las funciones nuevas, salvo donde la propiedad 1 las
//! compara entre sí.

use proptest::prelude::*;
use px_core::{Mode, Rgba};
use px_ui::background::{
    BLACK, CanvasBackground, Fill, WHITE, background_fill, background_value_text,
};
use px_ui::canvas::BACKGROUND;
use px_ui::checker::{
    CELL_U, Cell, CheckerQuad, ENV_MESH, Shade, cell_px, cells, clip, mesh_fallback_enabled, quad,
    shade_at, texels,
};
use px_ui::color_edit::parse_hex;
use px_ui::palette_grid::{CHECK_A, CHECK_B, swatch_parts};
use px_ui::text::text_scale;
use px_ui::view::{ScreenRect, Size};

mod common;
use common::PPPS;

fn r(x: i64, y: i64, width: i64, height: i64) -> ScreenRect {
    ScreenRect {
        x,
        y,
        width,
        height,
    }
}

fn assert_plane<T: Clone + Copy + std::fmt::Debug + PartialEq + Eq>() {}

const GRAY_A: Rgba = Rgba::opaque(0x4a, 0x4a, 0x4a);
const GRAY_B: Rgba = Rgba::opaque(0x6e, 0x6e, 0x6e);

/// Tono según la definición por fase, escrita aparte de `shade_at`: la casilla `(i, j)` es `A` si
/// `i + j` es par; `floor(d / c)` es impar justo cuando `d mod 2c >= c`. Con `i128` no desborda con
/// ningún `i64`.
fn tone_oracle(x: i64, y: i64, source_2: (i64, i64), cell: i64) -> Shade {
    let period = 2 * i128::from(cell);
    let middle = i128::from(cell);
    let odd_x = (i128::from(x) - i128::from(source_2.0)).rem_euclid(period) >= middle;
    let odd_y = (i128::from(y) - i128::from(source_2.1)).rem_euclid(period) >= middle;
    if odd_x == odd_y { Shade::A } else { Shade::B }
}

// ---------------------------------------------------------------------------------------------
// Fila 1: constantes y tamaño
// ---------------------------------------------------------------------------------------------

#[test]
fn row_1_cell_u_is_worth_8() {
    assert_eq!(CELL_U, 8);
}

#[test]
fn row_1_cell_px_is_8_times_n_from_1_to_8() {
    for n in 1..=8u32 {
        assert_eq!(cell_px(n), 8 * i64::from(n), "n = {n}");
    }
}

#[test]
fn row_1_cell_px_with_n_zero_is_treated_as_1() {
    assert_eq!(cell_px(0), 8);
}

#[test]
fn row_1_the_box_at_each_screen_scale_is_8_8_16_16_and_24() {
    let expected_2 = [8, 8, 16, 16, 24];
    for (ppp, cell) in PPPS.into_iter().zip(expected_2) {
        assert_eq!(cell_px(text_scale(ppp)), cell, "ppp {ppp}");
    }
}

#[test]
fn row_1_the_two_tones_are_check_a_and_check_b_with_their_values() {
    assert_eq!(Shade::A.color(), CHECK_A);
    assert_eq!(Shade::B.color(), CHECK_B);
    // Los mismos valores que prueba `palette_grid.rs`: `#4a4a4a` y `#6e6e6e`.
    assert_eq!(Shade::A.color(), GRAY_A);
    assert_eq!(Shade::B.color(), GRAY_B);
    assert_eq!(CHECK_A, GRAY_A);
    assert_eq!(CHECK_B, GRAY_B);
    assert_ne!(Shade::A, Shade::B);
    assert_ne!(Shade::A.color(), Shade::B.color());
}

#[test]
fn row_1_texels_are_a_b_b_a_by_rows() {
    assert_eq!(texels(), [GRAY_A, GRAY_B, GRAY_B, GRAY_A]);
    assert_eq!(
        texels(),
        [
            Shade::A.color(),
            Shade::B.color(),
            Shade::B.color(),
            Shade::A.color()
        ]
    );
}

#[test]
fn row_1_the_background_is_still_gray_40_and_different_from_the_two_tones() {
    assert_eq!(BACKGROUND, egui::Color32::from_gray(40));
    let background_2 = (BACKGROUND.r(), BACKGROUND.g(), BACKGROUND.b());
    assert_eq!(background_2, (40, 40, 40));
    for tone in [CHECK_A, CHECK_B] {
        assert_ne!(background_2, (tone.r, tone.g, tone.b));
    }
}

#[test]
fn row_1_shade_is_a_plain_type() {
    assert_plane::<Shade>();
}

// ---------------------------------------------------------------------------------------------
// Fila 2: shade_at
// ---------------------------------------------------------------------------------------------

#[test]
fn row_2_origin_zero_cell_8_table_of_the_spec() {
    let cases = [
        ((0, 0), Shade::A),
        ((7, 7), Shade::A),
        ((8, 0), Shade::B),
        ((0, 8), Shade::B),
        ((8, 8), Shade::A),
        ((15, 15), Shade::A),
        ((16, 0), Shade::A),
    ];
    for ((x, y), tone) in cases {
        assert_eq!(shade_at(x, y, (0, 0), 8), tone, "({x}, {y})");
    }
}

#[test]
fn row_2_with_negative_coordinates_the_floor_applies() {
    let cases = [
        ((-1, 0), Shade::B),
        ((-1, -1), Shade::A),
        ((-8, -8), Shade::A),
        ((-9, 0), Shade::A),
    ];
    for ((x, y), tone) in cases {
        assert_eq!(shade_at(x, y, (0, 0), 8), tone, "({x}, {y})");
    }
}

#[test]
fn row_2_with_origin_10_20() {
    let cases = [
        ((10, 20), Shade::A),
        ((18, 20), Shade::B),
        ((9, 20), Shade::B),
    ];
    for ((x, y), tone) in cases {
        assert_eq!(shade_at(x, y, (10, 20), 8), tone, "({x}, {y})");
    }
}

#[test]
fn row_2_with_origin_and_points_at_plus_minus_2_to_the_40_it_matches_rem_euclid() {
    let g = 1_i64 << 40;
    let source_2 = (-g + 3, g);
    let values = [-g, -g + 3, -g + 4, -1, 0, 1, g - 1, g, g + 1];
    for cell in [8, 16, 24, 40, 64] {
        for &x in &values {
            for &y in &values {
                assert_eq!(
                    shade_at(x, y, source_2, cell),
                    tone_oracle(x, y, source_2, cell),
                    "({x}, {y}) cell {cell}"
                );
            }
        }
    }
    // Y la esquina del origen es siempre la casilla (0, 0): `A`.
    assert_eq!(shade_at(source_2.0, source_2.1, source_2, 8), Shade::A);
}

#[test]
fn row_2_with_any_i64_it_does_not_overflow_or_panic() {
    // «Vale con cualquier i64»: la resta `x − ox` desbordaría un i64 (se hace en i128).
    let ends = [i64::MIN, i64::MIN + 1, -1, 0, 1, i64::MAX - 1, i64::MAX];
    for cell in [1, 8, 24, 64] {
        for &ox in &ends {
            for &oy in &ends {
                for &x in &ends {
                    for &y in &ends {
                        assert_eq!(
                            shade_at(x, y, (ox, oy), cell),
                            tone_oracle(x, y, (ox, oy), cell),
                            "({x}, {y}) origen ({ox}, {oy}) cell {cell}"
                        );
                    }
                }
            }
        }
    }
}

#[test]
fn row_2_toggles_box_by_box_in_both_directions() {
    // Vecinas por la derecha y por abajo siempre de tono distinto, con cualquier origen.
    for source_2 in [(0, 0), (5, -3), (-100, 77)] {
        for cell in [8, 16, 24] {
            for i in -3..=3 {
                for j in -3..=3 {
                    let (x, y) = (source_2.0 + i * cell, source_2.1 + j * cell);
                    let here = shade_at(x, y, source_2, cell);
                    assert_ne!(here, shade_at(x + cell, y, source_2, cell));
                    assert_ne!(here, shade_at(x, y + cell, source_2, cell));
                    assert_eq!(here, shade_at(x + cell, y + cell, source_2, cell));
                    // Dentro de la casilla, el tono no cambia.
                    assert_eq!(here, shade_at(x + cell - 1, y + cell - 1, source_2, cell));
                }
            }
        }
    }
}

// ---------------------------------------------------------------------------------------------
// Fila 3: quad y clip
// ---------------------------------------------------------------------------------------------

#[test]
fn row_3a_whole_image_visible() {
    let area = r(10, 20, 100, 50);
    assert_eq!(
        quad(area, (10, 20), 8),
        Some(CheckerQuad {
            rect: area,
            uv: [0.0, 0.0, 6.25, 3.125]
        })
    );
}

#[test]
fn row_3b_image_with_negative_origin_and_area_the_visible_part() {
    let area = r(0, 0, 200, 150);
    let u0 = 5.0_f32 / 16.0; // (21 mod 16) / 16
    let v0 = 3.0_f32 / 16.0; // (35 mod 16) / 16
    let q = quad(area, (-21, -35), 8).expect("área no vacía");
    assert_eq!(q.rect, area);
    assert_eq!(q.uv, [u0, v0, u0 + 200.0 / 16.0, v0 + 150.0 / 16.0]);
}

#[test]
fn row_3c_one_pixel_of_zoom_x1() {
    let area = r(37, 11, 1, 1);
    let u0 = 5.0_f32 / 16.0; // (37 mod 16) / 16
    let v0 = 11.0_f32 / 16.0; // (11 mod 16) / 16
    let q = quad(area, (0, 0), 8).expect("área de 1×1");
    assert_eq!(q.rect, area);
    assert_eq!(q.uv, [u0, v0, u0 + 1.0 / 16.0, v0 + 1.0 / 16.0]);
}

#[test]
fn row_3d_area_of_width_0_negative_height_and_cell_0_give_none() {
    assert_eq!(quad(r(5, 5, 0, 10), (0, 0), 8), None, "ancho 0");
    assert_eq!(quad(r(5, 5, 10, -3), (0, 0), 8), None, "alto negativo");
    assert_eq!(quad(r(5, 5, 10, 10), (0, 0), 0), None, "cell 0");
}

#[test]
fn row_3d_also_negative_width_height_0_and_negative_cell_give_none() {
    // «None si area está vacía (ancho o alto <= 0) o cell < 1» (sección «Piezas»).
    assert_eq!(quad(r(5, 5, -2, 10), (0, 0), 8), None, "ancho negativo");
    assert_eq!(quad(r(5, 5, 10, 0), (0, 0), 8), None, "alto 0");
    assert_eq!(quad(r(5, 5, 0, 0), (0, 0), 8), None, "vacía del todo");
    assert_eq!(quad(r(5, 5, 10, 10), (0, 0), -8), None, "cell negativa");
}

#[test]
fn row_3e_clip_of_an_image_that_sticks_out_on_one_side_and_the_other() {
    let canvas_2 = Size::new(100, 80);
    assert_eq!(
        clip(r(-50, -40, 300, 200), canvas_2),
        Some(r(0, 0, 100, 80)),
        "la imagen cubre el lienzo y sobresale"
    );
}

#[test]
fn row_3e_clip_of_an_image_outside_the_canvas_is_none() {
    let canvas_2 = Size::new(100, 80);
    assert_eq!(clip(r(200, 10, 50, 50), canvas_2), None, "a la derecha");
    assert_eq!(clip(r(10, 200, 50, 50), canvas_2), None, "debajo");
    assert_eq!(
        clip(r(-100, -100, 50, 50), canvas_2),
        None,
        "arriba a la izquierda"
    );
    assert_eq!(
        clip(r(100, 0, 50, 50), canvas_2),
        None,
        "pegada al borde derecho"
    );
}

#[test]
fn row_3e_clip_of_an_image_that_covers_the_canvas() {
    let canvas_2 = Size::new(100, 80);
    assert_eq!(
        clip(r(-10, -10, 500, 500), canvas_2),
        Some(r(0, 0, 100, 80))
    );
    assert_eq!(clip(r(0, 0, 100, 80), canvas_2), Some(r(0, 0, 100, 80)));
}

#[test]
fn row_3e_clip_of_an_image_that_is_only_partly_inside() {
    // `image ∩ (0, 0, width_2, height_2)`.
    let canvas_2 = Size::new(100, 80);
    assert_eq!(clip(r(30, 20, 200, 200), canvas_2), Some(r(30, 20, 70, 60)));
    assert_eq!(clip(r(-5, 10, 20, 20), canvas_2), Some(r(0, 10, 15, 20)));
}

#[test]
fn row_3_clip_with_an_empty_canvas_is_none() {
    assert_eq!(clip(r(0, 0, 10, 10), Size::new(0, 0)), None);
    assert_eq!(clip(r(0, 0, 10, 10), Size::new(0, 5)), None);
}

#[test]
fn row_3_clip_with_a_1x1_canvas_gives_none_or_1x1() {
    let canvas_2 = Size::new(1, 1);
    for image_2 in [
        r(-30, -20, 64, 48),
        r(0, 0, 64, 48),
        r(-63, -47, 64, 48),
        r(1, 0, 64, 48),
    ] {
        let clip_2 = clip(image_2, canvas_2);
        assert!(
            matches!(
                clip_2,
                None | Some(ScreenRect {
                    width: 1,
                    height: 1,
                    ..
                })
            ),
            "{image_2:?} → {clip_2:?}"
        );
    }
}

// ---------------------------------------------------------------------------------------------
// Fila 4: cells
// ---------------------------------------------------------------------------------------------

fn cel(x: i64, y: i64, w: i64, h: i64, shade: Shade) -> Cell {
    Cell {
        rect: r(x, y, w, h),
        shade,
    }
}

fn area_total(list: &[Cell]) -> i64 {
    list.iter().map(|c| c.rect.width * c.rect.height).sum()
}

/// Dos rectángulos se solapan si comparten algún píxel.
fn overlap_2(a: ScreenRect, b: ScreenRect) -> bool {
    a.x < b.x + b.width && b.x < a.x + a.width && a.y < b.y + b.height && b.y < a.y + a.height
}

fn without_overlaps(list: &[Cell]) -> bool {
    for (i, a) in list.iter().enumerate() {
        for b in &list[i + 1..] {
            if overlap_2(a.rect, b.rect) {
                return false;
            }
        }
    }
    true
}

#[test]
fn row_4a_area_of_20_by_12_with_origin_zero() {
    let list: Vec<Cell> = cells(r(0, 0, 20, 12), (0, 0), 8).collect();
    assert_eq!(
        list,
        [
            cel(0, 0, 8, 8, Shade::A),
            cel(8, 0, 8, 8, Shade::B),
            cel(16, 0, 4, 8, Shade::A),
            cel(0, 8, 8, 4, Shade::B),
            cel(8, 8, 8, 4, Shade::A),
            cel(16, 8, 4, 4, Shade::B),
        ]
    );
    assert_eq!(list.len(), 6);
    assert_eq!(area_total(&list), 240);
    assert!(without_overlaps(&list));
}

#[test]
fn row_4b_area_shifted_5_3() {
    let list: Vec<Cell> = cells(r(5, 3, 20, 12), (0, 0), 8).collect();
    assert_eq!(list.len(), 8);
    assert_eq!(list[0], cel(5, 3, 3, 5, Shade::A));
    // Columnas de ancho 3, 8, 8 y 1; filas de alto 5 y 7; por filas.
    let widths: Vec<i64> = list.iter().map(|c| c.rect.width).collect();
    let altos: Vec<i64> = list.iter().map(|c| c.rect.height).collect();
    assert_eq!(widths, [3, 8, 8, 1, 3, 8, 8, 1]);
    assert_eq!(altos, [5, 5, 5, 5, 7, 7, 7, 7]);
    let tones: Vec<Shade> = list.iter().map(|c| c.shade).collect();
    let (ta, tb) = (Shade::A, Shade::B);
    assert_eq!(tones, [ta, tb, ta, tb, tb, ta, tb, ta]);
    assert_eq!(
        list,
        [
            cel(5, 3, 3, 5, Shade::A),
            cel(8, 3, 8, 5, Shade::B),
            cel(16, 3, 8, 5, Shade::A),
            cel(24, 3, 1, 5, Shade::B),
            cel(5, 8, 3, 7, Shade::B),
            cel(8, 8, 8, 7, Shade::A),
            cel(16, 8, 8, 7, Shade::B),
            cel(24, 8, 1, 7, Shade::A),
        ]
    );
    assert_eq!(area_total(&list), 240);
    assert!(without_overlaps(&list));
}

#[test]
fn row_4c_area_smaller_than_a_box_is_a_box_equal_to_the_area() {
    let list: Vec<Cell> = cells(r(3, 3, 2, 2), (0, 0), 8).collect();
    assert_eq!(list, [cel(3, 3, 2, 2, Shade::A)]);
}

#[test]
fn row_4d_area_of_1x1_one_box_and_empty_area_none() {
    let a: Vec<Cell> = cells(r(9, 0, 1, 1), (0, 0), 8).collect();
    assert_eq!(a, [cel(9, 0, 1, 1, Shade::B)]);
    assert_eq!(cells(r(0, 0, 0, 5), (0, 0), 8).count(), 0, "ancho 0");
    assert_eq!(cells(r(0, 0, 5, 0), (0, 0), 8).count(), 0, "alto 0");
    assert_eq!(
        cells(r(0, 0, -5, 5), (0, 0), 8).count(),
        0,
        "ancho negativo"
    );
    assert_eq!(cells(r(0, 0, 0, 0), (0, 0), 8).count(), 0, "vacía del todo");
}

#[test]
fn row_4d_with_cell_less_than_1_there_are_no_boxes() {
    // «Vacío si area o cell lo son».
    assert_eq!(cells(r(0, 0, 20, 12), (0, 0), 0).count(), 0);
    assert_eq!(cells(r(0, 0, 20, 12), (0, 0), -8).count(), 0);
}

#[test]
fn row_4e_a_4k_canvas_has_129600_boxes_without_reserving_the_list() {
    // Solo se cuenta: `cells` es un iterador, no una lista.
    assert_eq!(cells(r(0, 0, 3840, 2160), (0, 0), 8).count(), 129_600);
    // Con un origen cualquiera el número depende de la fase, pero el iterador sigue siendo
    // perezoso: un área de 2³⁰ píxeles por lado da sus primeras casillas sin recorrerla entera.
    let huge = r(0, 0, 1 << 30, 1 << 30);
    let first: Vec<Cell> = cells(huge, (0, 0), 8).take(3).collect();
    assert_eq!(
        first,
        [
            cel(0, 0, 8, 8, Shade::A),
            cel(8, 0, 8, 8, Shade::B),
            cel(16, 0, 8, 8, Shade::A),
        ]
    );
}

#[test]
fn row_4_the_tone_of_each_box_is_that_of_shade_at() {
    let source_2 = (-7, 13);
    for c in cells(r(-20, -20, 70, 60), source_2, 8) {
        assert_eq!(
            c.shade,
            shade_at(c.rect.x, c.rect.y, source_2, 8),
            "{:?}",
            c.rect
        );
    }
}

// ---------------------------------------------------------------------------------------------
// Fila 9: fondo elegible
// ---------------------------------------------------------------------------------------------

const BACKGROUNDS: [CanvasBackground; 4] = [
    CanvasBackground::Checker,
    CanvasBackground::Black,
    CanvasBackground::Gray,
    CanvasBackground::White,
];

#[test]
fn row_9_by_default_it_is_the_checker() {
    assert_eq!(CanvasBackground::default(), CanvasBackground::Checker);
}

#[test]
fn row_9_background_fill_of_the_four_options() {
    assert_eq!(background_fill(CanvasBackground::Checker), Fill::Checker);
    assert_eq!(
        background_fill(CanvasBackground::Black),
        Fill::Solid(Rgba::opaque(0, 0, 0))
    );
    assert_eq!(
        background_fill(CanvasBackground::White),
        Fill::Solid(Rgba::opaque(255, 255, 255))
    );
    // Gris: los canales de `BACKGROUND` (gris 40).
    assert_eq!(
        background_fill(CanvasBackground::Gray),
        Fill::Solid(Rgba::opaque(BACKGROUND.r(), BACKGROUND.g(), BACKGROUND.b()))
    );
    assert_eq!(
        background_fill(CanvasBackground::Gray),
        Fill::Solid(Rgba::opaque(40, 40, 40))
    );
}

#[test]
fn row_9_black_and_white_are_opaque_and_different_from_the_background_and_the_two_tones() {
    assert_eq!(BLACK, Rgba::opaque(0, 0, 0));
    assert_eq!(WHITE, Rgba::opaque(255, 255, 255));
    assert_eq!(BLACK.a, 255);
    assert_eq!(WHITE.a, 255);
    let background_2 = Rgba::opaque(BACKGROUND.r(), BACKGROUND.g(), BACKGROUND.b());
    for color in [BLACK, WHITE] {
        assert_ne!(color, background_2);
        assert_ne!(color, CHECK_A);
        assert_ne!(color, CHECK_B);
    }
    assert_ne!(BLACK, WHITE);
}

#[test]
fn row_9_the_four_options_are_different_from_each_other_and_their_fills_too() {
    for (i, a) in BACKGROUNDS.iter().enumerate() {
        for b in &BACKGROUNDS[i + 1..] {
            assert_ne!(a, b);
            assert_ne!(background_fill(*a), background_fill(*b), "{a:?} y {b:?}");
        }
    }
}

#[test]
fn row_9_the_background_types_are_plain_and_with_a_default() {
    assert_plane::<CanvasBackground>();
    assert_plane::<Fill>();
    let _: CanvasBackground = Default::default();
}

// ---------------------------------------------------------------------------------------------
// Fila 10: coherencia con el panel de paletas
// ---------------------------------------------------------------------------------------------

#[test]
fn row_10_the_two_boxes_of_the_canvas_are_the_constants_of_the_panel() {
    assert_eq!(Shade::A.color(), CHECK_A);
    assert_eq!(Shade::B.color(), CHECK_B);
    assert_eq!(texels(), [CHECK_A, CHECK_B, CHECK_B, CHECK_A]);
}

#[test]
fn row_10_the_transparent_swatch_of_the_panel_uses_the_same_pair_in_the_same_order() {
    for n in 1..=3u32 {
        let parts = swatch_parts(n, Rgba::new(0, 0, 0, 0));
        assert_eq!(parts.len(), 4, "n = {n}: cuatro cuadrados y nada más");
        let tones: Vec<Rgba> = parts.iter().map(|(_, tone)| *tone).collect();
        // Los cuatro cuadrados, por filas, son los cuatro texels de la textura del lienzo.
        assert_eq!(tones, texels(), "n = {n}");
        // Y no se pinta con ningún otro gris.
        for tone in &tones {
            assert!(
                *tone == Shade::A.color() || *tone == Shade::B.color(),
                "n = {n}: {tone:?}"
            );
        }
    }
}

#[test]
fn row_10_in_the_swatch_the_squares_follow_the_same_pattern_as_shade_at() {
    // Los cuatro cuadrados de la muestra (origen en su esquina, casilla de `5·n`) alternan igual
    // que las casillas del lienzo: la casilla (0, 0) es `A`.
    for n in 1..=3u32 {
        let parts = swatch_parts(n, Rgba::new(0, 0, 0, 0));
        for (rect, tone) in parts {
            let cell = rect.width;
            assert_eq!(
                tone,
                shade_at(rect.x, rect.y, (0, 0), cell).color(),
                "n = {n}: {rect:?}"
            );
        }
    }
}

// ---------------------------------------------------------------------------------------------
// Fila 15, parte pura: la variable de entorno
// ---------------------------------------------------------------------------------------------

#[test]
fn row_15_the_environment_variable_is_called_pixoko_checker_mesh() {
    assert_eq!(ENV_MESH, "PIXOKO_CHECKER_MESH");
}

#[test]
fn row_15_only_the_value_1_activates_the_mesh_fallback() {
    assert!(!mesh_fallback_enabled(None));
    assert!(mesh_fallback_enabled(Some("1")));
    assert!(!mesh_fallback_enabled(Some("0")));
    assert!(!mesh_fallback_enabled(Some("")));
    assert!(!mesh_fallback_enabled(Some("true")));
}

#[test]
fn row_15_no_other_value_similar_to_1_activates_it() {
    for value in [" 1", "1 ", "11", "01", "yes", "on", "TRUE", "-1"] {
        assert!(!mesh_fallback_enabled(Some(value)), "{value:?}");
    }
}

// ---------------------------------------------------------------------------------------------
// Propiedad 1: quad, shade_at y cells son la misma función
// ---------------------------------------------------------------------------------------------

/// Muestra la textura 2×2 con `NEAREST` y envoltura en el centro del píxel `(i, j)` (de 0) del
/// cuadrilátero, interpolando los uv `f32` en `f64`.
fn swatch(q: &CheckerQuad, tex: &[Rgba; 4], i: i64, j: i64) -> Rgba {
    let t = (i as f64 + 0.5) / q.rect.width as f64;
    let s = (j as f64 + 0.5) / q.rect.height as f64;
    let (u0, v0) = (f64::from(q.uv[0]), f64::from(q.uv[1]));
    let (u1, v1) = (f64::from(q.uv[2]), f64::from(q.uv[3]));
    let u = u0 + (u1 - u0) * t;
    let v = v0 + (v1 - v0) * s;
    let tx = ((u * 2.0).floor() as i64).rem_euclid(2);
    let ty = ((v * 2.0).floor() as i64).rem_euclid(2);
    tex[(ty * 2 + tx) as usize]
}

/// Tono por fase en `i64` (los datos de la propiedad caben de sobra): más barato que `i128` y
/// escrito aparte de `shade_at`.
fn color_per_phase(x: i64, y: i64, source_2: (i64, i64), cell: i64) -> Rgba {
    let odd_x = (x - source_2.0).rem_euclid(2 * cell) >= cell;
    let odd_y = (y - source_2.1).rem_euclid(2 * cell) >= cell;
    if odd_x == odd_y { GRAY_A } else { GRAY_B }
}

/// Bordes de las casillas del eje: el principio del área, cada borde de casilla estrictamente
/// dentro y el final.
fn borders(start_2: i64, length_2: i64, source_2: i64, cell: i64) -> Vec<i64> {
    let fin = start_2 + length_2;
    let mut v = vec![start_2];
    let mut k = (start_2 - source_2).div_euclid(cell) + 1;
    while source_2 + k * cell < fin {
        v.push(source_2 + k * cell);
        k += 1;
    }
    v.push(fin);
    v
}

/// Número de casillas que corta el eje: `⌈(start_2 + length_2 − source_2) / cell⌉ − ⌊(start_2 − source_2) /
/// cell⌋`.
fn boxes_of_the_axis(start_2: i64, length_2: i64, source_2: i64, cell: i64) -> i64 {
    let ceiling = -(-(start_2 + length_2 - source_2)).div_euclid(cell);
    ceiling - (start_2 - source_2).div_euclid(cell)
}

const LARGE: i64 = 1 << 40;

/// Datos de un caso: origen, `cell`, área y una subárea (como porcentajes sobre el área).
#[derive(Clone, Debug)]
struct Case {
    source_2: (i64, i64),
    cell: i64,
    area: ScreenRect,
    sub: ScreenRect,
}

fn case() -> impl Strategy<Value = Case> {
    (
        (-LARGE..=LARGE, -LARGE..=LARGE),
        1i64..=8,
        any::<bool>(),
        (-LARGE..=LARGE, -LARGE..=LARGE),
        (-3000i64..=3000, -3000i64..=3000),
        (1i64..=600, 1i64..=600),
        (any::<u32>(), any::<u32>(), any::<u32>(), any::<u32>()),
    )
        .prop_map(|(source_2, k, near, far, desplazo, (w, h), (a, b, c, d))| {
            let cell = 8 * k;
            // Mitad de los casos con el área junto al origen (casillas a ambos lados del
            // cero de la fase) y mitad en cualquier posición, también negativa.
            let (x, y) = if near {
                (source_2.0 + desplazo.0, source_2.1 + desplazo.1)
            } else {
                far
            };
            let sx = i64::from(a) % w;
            let sy = i64::from(b) % h;
            let sw = 1 + i64::from(c) % (w - sx);
            let sh = 1 + i64::from(d) % (h - sy);
            Case {
                source_2,
                cell,
                area: r(x, y, w, h),
                sub: r(x + sx, y + sy, sw, sh),
            }
        })
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(512))]

    /// (a) El cuadrilátero del área, muestreado como la GPU (`NEAREST` con envoltura) en el centro
    /// de cada píxel físico, da el tono de `shade_at`, que a su vez coincide con la fase calculada
    /// aparte.
    #[test]
    fn property_1a_the_quad_sampling_is_shade_at(c in case()) {
        let q = quad(c.area, c.source_2, c.cell).expect("área no vacía");
        prop_assert_eq!(q.rect, c.area);
        let tex = texels();
        for j in 0..c.area.height {
            for i in 0..c.area.width {
                let (x, y) = (c.area.x + i, c.area.y + j);
                let expected_2 = color_per_phase(x, y, c.source_2, c.cell);
                let seen = swatch(&q, &tex, i, j);
                prop_assert_eq!(seen, expected_2, "el muestreo en ({}, {})", x, y);
                prop_assert_eq!(
                    shade_at(x, y, c.source_2, c.cell).color(),
                    expected_2,
                    "shade_at en ({}, {})", x, y
                );
            }
        }
    }

    /// (b) Para cualquier subárea, `quad(subárea)` muestrea lo mismo que `quad(área)` en esos
    /// píxeles: el recorte no cambia el color.
    #[test]
    fn property_1b_the_clip_does_not_change_the_color(c in case()) {
        let large = quad(c.area, c.source_2, c.cell).expect("área no vacía");
        let small = quad(c.sub, c.source_2, c.cell).expect("subárea no vacía");
        prop_assert_eq!(small.rect, c.sub);
        let tex = texels();
        for j in 0..c.sub.height {
            for i in 0..c.sub.width {
                let (x, y) = (c.sub.x + i, c.sub.y + j);
                let with_the_small = swatch(&small, &tex, i, j);
                let with_the_large = swatch(&large, &tex, x - c.area.x, y - c.area.y);
                prop_assert_eq!(with_the_small, with_the_large, "en ({}, {})", x, y);
                prop_assert_eq!(with_the_small, color_per_phase(x, y, c.source_2, c.cell));
            }
        }
    }

    /// (c) `cells(área)` cubre el área exactamente: sin solapes ni huecos, cada casilla dentro de
    /// una sola casilla del damero, con el tono de `shade_at` de cualquiera de sus píxeles,
    /// alternado con sus vecinas, y su número es el calculado aparte por eje.
    #[test]
    fn property_1c_the_boxes_cover_the_area_exactly(c in case()) {
        let list: Vec<Cell> = cells(c.area, c.source_2, c.cell).collect();
        let xs = borders(c.area.x, c.area.width, c.source_2.0, c.cell);
        let ys = borders(c.area.y, c.area.height, c.source_2.1, c.cell);
        let columns_2 = boxes_of_the_axis(c.area.x, c.area.width, c.source_2.0, c.cell);
        let rows_2 = boxes_of_the_axis(c.area.y, c.area.height, c.source_2.1, c.cell);
        prop_assert_eq!(xs.len() as i64 - 1, columns_2);
        prop_assert_eq!(ys.len() as i64 - 1, rows_2);
        prop_assert_eq!(list.len() as i64, columns_2 * rows_2, "número de casillas");
        // Suma de áreas: sin huecos ni solapes (junto con que van contiguas, por filas).
        prop_assert_eq!(area_total(&list), c.area.width * c.area.height);
        let columns_2 = columns_2 as usize;
        for (k, box_value) in list.iter().enumerate() {
            let (row_2, column) = (k / columns_2, k % columns_2);
            // Por filas y contiguas: la que toca por posición, recortada al área.
            let rect = r(
                xs[column],
                ys[row_2],
                xs[column + 1] - xs[column],
                ys[row_2 + 1] - ys[row_2],
            );
            prop_assert_eq!(box_value.rect, rect, "casilla {}", k);
            // Dentro del área.
            prop_assert!(box_value.rect.x >= c.area.x && box_value.rect.y >= c.area.y);
            prop_assert!(box_value.rect.x + box_value.rect.width <= c.area.x + c.area.width);
            prop_assert!(box_value.rect.y + box_value.rect.height <= c.area.y + c.area.height);
            // Dentro de una sola casilla del damero: el primer y el último píxel caen en la misma.
            let (x1, y1) = (rect.x + rect.width - 1, rect.y + rect.height - 1);
            prop_assert_eq!(
                (rect.x - c.source_2.0).div_euclid(c.cell),
                (x1 - c.source_2.0).div_euclid(c.cell)
            );
            prop_assert_eq!(
                (rect.y - c.source_2.1).div_euclid(c.cell),
                (y1 - c.source_2.1).div_euclid(c.cell)
            );
            // Tono: el de `shade_at` de sus esquinas y el de la fase calculada aparte.
            prop_assert_eq!(box_value.shade, shade_at(rect.x, rect.y, c.source_2, c.cell));
            prop_assert_eq!(box_value.shade, shade_at(x1, y1, c.source_2, c.cell));
            prop_assert_eq!(
                box_value.shade.color(),
                color_per_phase(rect.x, rect.y, c.source_2, c.cell)
            );
            // Alternado con la vecina de la derecha y con la de abajo.
            if column + 1 < columns_2 {
                prop_assert_ne!(box_value.shade, list[k + 1].shade);
            }
            if k + columns_2 < list.len() {
                prop_assert_ne!(box_value.shade, list[k + columns_2].shade);
            }
        }
    }
}

// ---------------------------------------------------------------------------------------------
// Unidad 19b, fila 1: tipo y relleno de `CanvasBackground::Custom`
// ---------------------------------------------------------------------------------------------
//
// Suposiciones: `CanvasBackground::custom(Rgba) -> Option<CanvasBackground>`, `solid_color(self) ->
// Option<Rgba>` y `background_value_text(CanvasBackground) -> Option<String>` son las de «Piezas».
// Con un `Custom` construido a mano con alfa < 255 (la variante es pública) la spec solo fija
// `background_fill` (alfa 255 forzado): su `solid_color` y su valor escrito no se prueban.
// NO PROBADO: `solid_color` y `background_value_text` de un `Custom(c)` con `c.a < 255` (la spec no
// dice si devuelven `c` tal cual, con el alfa forzado o `#rrggbbaa`).

const BLUE_STEEL: Rgba = Rgba::opaque(0x33, 0x66, 0x99);
const GRAY_40: Rgba = Rgba::opaque(40, 40, 40);

fn own(c: Rgba) -> CanvasBackground {
    CanvasBackground::Custom(c)
}

/// Los cinco fondos, con un color propio de canales distintos.
fn the_five() -> [CanvasBackground; 5] {
    [
        CanvasBackground::Checker,
        CanvasBackground::Black,
        CanvasBackground::Gray,
        CanvasBackground::White,
        own(BLUE_STEEL),
    ]
}

#[test]
fn row_1_custom_gives_some_only_with_alpha_255_and_keeps_the_color() {
    assert_eq!(
        CanvasBackground::custom(BLUE_STEEL),
        Some(CanvasBackground::Custom(BLUE_STEEL))
    );
    assert_eq!(
        CanvasBackground::custom(Rgba::new(0x33, 0x66, 0x99, 0xfe)),
        None
    );
    assert_eq!(CanvasBackground::custom(Rgba::new(0, 0, 0, 0)), None);
    // `#ffffffff` es el blanco opaco: vale y no se confunde con `White`.
    assert_eq!(
        CanvasBackground::custom(Rgba::new(0xff, 0xff, 0xff, 0xff)),
        Some(CanvasBackground::Custom(WHITE))
    );
    assert_ne!(CanvasBackground::Custom(WHITE), CanvasBackground::White);
    // Los 256 alfas: solo el último vale.
    for a in 0..=255u8 {
        let c = Rgba::new(0x33, 0x66, 0x99, a);
        assert_eq!(
            CanvasBackground::custom(c).is_some(),
            a == 255,
            "alfa {a:#04x}"
        );
    }
}

#[test]
fn row_1_background_fill_of_custom_is_always_solid_with_alpha_255() {
    assert_eq!(
        background_fill(own(BLUE_STEEL)),
        Fill::Solid(Rgba::new(0x33, 0x66, 0x99, 255))
    );
    // Construido a mano con alfa < 255: el relleno fuerza el alfa a 255 y conserva r, g, b.
    for a in [0u8, 1, 0x7f, 0x80, 0xfe] {
        assert_eq!(
            background_fill(own(Rgba::new(0x33, 0x66, 0x99, a))),
            Fill::Solid(Rgba::new(0x33, 0x66, 0x99, 255)),
            "alfa {a:#04x}"
        );
    }
    for c in [
        Rgba::opaque(0xff, 0x80, 0x00),
        Rgba::opaque(0x01, 0x02, 0x03),
        Rgba::new(0xff, 0x80, 0x00, 0),
    ] {
        assert_eq!(
            background_fill(own(c)),
            Fill::Solid(Rgba::opaque(c.r, c.g, c.b))
        );
    }
}

#[test]
fn row_1_the_fill_of_the_other_four_is_that_of_the_17b() {
    assert_eq!(background_fill(CanvasBackground::Checker), Fill::Checker);
    assert_eq!(background_fill(CanvasBackground::Black), Fill::Solid(BLACK));
    assert_eq!(background_fill(CanvasBackground::White), Fill::Solid(WHITE));
    assert_eq!(
        background_fill(CanvasBackground::Gray),
        Fill::Solid(GRAY_40)
    );
    assert_eq!(
        background_fill(CanvasBackground::Gray),
        Fill::Solid(Rgba::opaque(BACKGROUND.r(), BACKGROUND.g(), BACKGROUND.b()))
    );
}

#[test]
fn row_1_the_default_background_is_still_the_checker() {
    assert_eq!(CanvasBackground::default(), CanvasBackground::Checker);
    assert_eq!(background_fill(CanvasBackground::default()), Fill::Checker);
}

#[test]
fn row_1_solid_color_is_some_in_the_four_solids_and_none_with_the_checker() {
    assert_eq!(CanvasBackground::Checker.solid_color(), None);
    assert_eq!(CanvasBackground::Black.solid_color(), Some(BLACK));
    assert_eq!(CanvasBackground::White.solid_color(), Some(WHITE));
    assert_eq!(CanvasBackground::Gray.solid_color(), Some(GRAY_40));
    assert_eq!(own(BLUE_STEEL).solid_color(), Some(BLUE_STEEL));
    assert_eq!(
        own(Rgba::opaque(0xff, 0x80, 0x00)).solid_color(),
        Some(Rgba::opaque(0xff, 0x80, 0x00))
    );
    // `Custom(#000000)` no pasa a `Black` ni a otro valor: su color es el elegido.
    assert_eq!(own(BLACK).solid_color(), Some(BLACK));
}

#[test]
fn row_1_background_value_text_is_some_only_with_custom_and_measures_7_characters() {
    assert_eq!(
        background_value_text(own(BLUE_STEEL)),
        Some("#336699".to_string())
    );
    assert_eq!(
        background_value_text(own(Rgba::opaque(0xff, 0x80, 0x00))),
        Some("#ff8000".to_string())
    );
    assert_eq!(
        background_value_text(own(Rgba::opaque(0x01, 0x02, 0x03))),
        Some("#010203".to_string())
    );
    assert_eq!(
        background_value_text(own(BLACK)),
        Some("#000000".to_string())
    );
    assert_eq!(
        background_value_text(own(BLUE_STEEL)).map(|t| t.chars().count()),
        Some(7)
    );
    for other_2 in [
        CanvasBackground::Checker,
        CanvasBackground::Black,
        CanvasBackground::Gray,
        CanvasBackground::White,
    ] {
        assert_eq!(background_value_text(other_2), None, "{other_2:?}");
    }
}

#[test]
fn row_1_custom_is_not_normalized_to_another_variant_even_if_its_color_matches() {
    // El usuario eligió «Color…»: la marca y el valor no saltan a otra entrada.
    assert_ne!(own(BLACK), CanvasBackground::Black);
    assert_ne!(own(WHITE), CanvasBackground::White);
    assert_ne!(own(GRAY_40), CanvasBackground::Gray);
    assert_eq!(CanvasBackground::custom(BLACK), Some(own(BLACK)));
    // Pero se ve igual: el mismo relleno.
    assert_eq!(
        background_fill(own(BLACK)),
        background_fill(CanvasBackground::Black)
    );
    assert_eq!(
        background_fill(own(WHITE)),
        background_fill(CanvasBackground::White)
    );
    assert_eq!(
        background_fill(own(GRAY_40)),
        background_fill(CanvasBackground::Gray)
    );
}

#[test]
fn row_1_the_five_variants_are_different_from_each_other_and_the_type_is_still_plain() {
    let five = the_five();
    for (i, a) in five.iter().enumerate() {
        for b in &five[i + 1..] {
            assert_ne!(a, b);
        }
    }
    // Dos colores propios distintos son fondos distintos y con rellenos distintos.
    assert_ne!(own(BLUE_STEEL), own(Rgba::opaque(0xff, 0x80, 0)));
    assert_ne!(
        background_fill(own(BLUE_STEEL)),
        background_fill(own(Rgba::opaque(0xff, 0x80, 0)))
    );
    assert_eq!(own(BLUE_STEEL), own(BLUE_STEEL));
    assert_plane::<CanvasBackground>();
    assert_plane::<Fill>();
}

// ---------------------------------------------------------------------------------------------
// Unidad 19b, propiedad 1 (parte pura): `custom`, `background_value_text` y `background_fill`
// ---------------------------------------------------------------------------------------------

proptest! {
    #![proptest_config(ProptestConfig::with_cases(256))]

    /// `custom(c)` es `Some` si y solo si `c.a == 255`; entonces es `Custom(c)`. El valor escrito de
    /// un color propio opaco se lee de vuelta (`parse_hex`) como el mismo color y mide 7
    /// caracteres. `background_fill(Custom(c))`, con cualquier alfa, es un sólido opaco con los
    /// mismos r, g, b y nunca lleva alfa < 255.
    #[test]
    fn property_1_custom_only_opaque_round_trip_value_and_always_opaque_fill(
        r in any::<u8>(),
        g in any::<u8>(),
        b in any::<u8>(),
        a in any::<u8>(),
    ) {
        let c = Rgba::new(r, g, b, a);
        let done = CanvasBackground::custom(c);
        prop_assert_eq!(done.is_some(), a == 255);
        if let Some(background_2) = done {
            prop_assert_eq!(background_2, CanvasBackground::Custom(c));
            prop_assert_eq!(background_2.solid_color(), Some(c));
            let text_2 = background_value_text(background_2).expect("un color propio tiene valor");
            prop_assert_eq!(text_2.chars().count(), 7);
            // Se lee de vuelta con `parse_hex`: con seis dígitos el alfa es el del color actual
            // (aquí, opaco).
            prop_assert_eq!(parse_hex(&text_2, Mode::Rgba, Rgba::opaque(0, 0, 0)), Ok(c));
        }
        // El relleno de un `Custom` construido a mano, con cualquier alfa.
        match background_fill(CanvasBackground::Custom(c)) {
            Fill::Solid(s) => {
                prop_assert_eq!(s.a, 255);
                prop_assert_eq!((s.r, s.g, s.b), (r, g, b));
            }
            Fill::Checker => prop_assert!(false, "un color propio nunca es el damero"),
        }
    }
}
