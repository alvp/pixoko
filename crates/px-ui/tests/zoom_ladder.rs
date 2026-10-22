//! Escalera de zoom, parte pura (specs/hito-2-zoom-baldosas.md, decisiones 2, 3 y 4; unidad 14a-1,
//! filas A1 a A5 y propiedades P1 a P4 sin egui).
//!
//! Sin egui y sin GPU: solo `px_ui::view`. Los oráculos están escritos aquí, con `i128` y
//! fracciones `(num, den)` y un `floor`/`ceil` propio, **sin llamar** a las funciones bajo prueba:
//!
//! - A1: los 70 niveles (`Zoom::levels`), `MIN`/`MAX`, `zoom_in`/`zoom_out`, `shrink`,
//!   `shrink_factor`, `from_int`, orden, `Display` y `Hash`.
//! - A2: `scale`, `unscale`, `rescale` y `extent` a 1/2, 1/4 y 1/64, con negativos y `i64` extremos.
//! - A3: pantalla ↔ imagen a 1/4 (imagen 10×10, origen (10, 20)), `image_rect`, `image_to_screen`,
//!   `pixel_block` e ida y vuelta.
//! - A4: `View::fit` con la tabla de la decisión 2 y los casos límite.
//! - A5: el ancla de `set_zoom` (el píxel bajo el ancla sube a la rejilla del nivel nuevo).
//! - P1 a P4: conversión exacta, ancla, Ajustar y cobertura/visibilidad (256 casos).
//!
//! Supuestos que la spec no fija del todo (anotados al escribirlos):
//! - `View::pixel_block` devuelve `((x0, y0), (x1, y1))` con el máximo **exclusivo** (`x1 = x0 +
//!   ancho`): `zoom × zoom` with zoom ≥ ×1 y 1×1 a 1/K.
//! - La retracción `scale(unscale(s)) = s` y `unscale(scale(x)) = x − x mod K` es la de los niveles
//!   1/K (decisión 4); con zoom entero ≥ ×1 es la de hoy: `unscale(scale(x)) = x` y
//!   `scale(unscale(s)) = s − s mod z`.
//! - Una imagen de ancho 0 no se genera en P3 (la spec no dice qué nivel le toca).

use std::collections::HashSet;
use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};

use proptest::prelude::*;
use px_ui::view::{MIN_VISIBLE, Size, View, Zoom};

// ---------------------------------------------------------------------------------------------
// Oráculo
// ---------------------------------------------------------------------------------------------

/// Fracción `(num, den)` de un nivel.
type Fraction = (i128, i128);

/// Punto de pantalla.
type Point = (i64, i64);

/// Los 70 niveles de la decisión 2, de menor a mayor: 1/64, 1/32, …, 1/2, ×1, ×2, …, ×64.
fn ladder() -> Vec<Fraction> {
    let mut levels: Vec<Fraction> = [64, 32, 16, 8, 4, 2].iter().map(|&k| (1, k)).collect();
    levels.extend((1..=64).map(|n| (n, 1)));
    levels
}

/// Texto de un nivel: `×12` o `×1/8`.
fn label((num, den): Fraction) -> String {
    if den == 1 {
        format!("×{num}")
    } else {
        format!("×{num}/{den}")
    }
}

/// `floor(n / d)` con `d > 0`.
fn floor(n: i128, d: i128) -> i128 {
    let q = n / d;
    if n % d != 0 && n < 0 { q - 1 } else { q }
}

/// `ceil(n / d)` con `d > 0`.
fn ceiling(n: i128, d: i128) -> i128 {
    -floor(-n, d)
}

fn saturates(v: i128) -> i64 {
    v.clamp(i128::from(i64::MIN), i128::from(i64::MAX)) as i64
}

fn saturates_i_32(v: i128) -> i32 {
    v.clamp(i128::from(i32::MIN), i128::from(i32::MAX)) as i32
}

fn o_scale(f: Fraction, n: i64) -> i64 {
    saturates(floor(i128::from(n) * f.0, f.1))
}

fn o_unscale(f: Fraction, n: i64) -> i64 {
    saturates(floor(i128::from(n) * f.1, f.0))
}

fn o_extent(f: Fraction, n: i64) -> i64 {
    saturates(ceiling(i128::from(n) * f.0, f.1))
}

/// `floor(n · to / self)`.
fn o_rescale(from_2: Fraction, n: i64, until: Fraction) -> i64 {
    saturates(floor(
        i128::from(n) * until.0 * from_2.1,
        until.1 * from_2.0,
    ))
}

/// Píxel de imagen bajo el punto `p` de un eje (origen `source_2`, `dim` píxeles), o `None`.
fn o_axis(f: Fraction, p: i64, source_2: i64, dim: u32) -> Option<u32> {
    let d = i128::from(p) - i128::from(source_2);
    if d < 0 {
        return None;
    }
    let index_2 = floor(d * f.1, f.0);
    (index_2 < i128::from(dim)).then_some(index_2 as u32)
}

fn or_axis_i32(f: Fraction, p: i64, source_2: i64) -> i32 {
    let d = i128::from(p) - i128::from(source_2);
    saturates_i_32(floor(d * f.1, f.0))
}

/// Índice del mayor nivel con `extent ≤ canvas_2` en los dos ejes (0, el mínimo, si ninguno cabe).
fn o_better_level(viewport: Size, image: Size) -> usize {
    let levels = ladder();
    (0..levels.len())
        .rev()
        .find(|&i| {
            o_extent(levels[i], i64::from(image.width)) <= i64::from(viewport.width)
                && o_extent(levels[i], i64::from(image.height)) <= i64::from(viewport.height)
        })
        .unwrap_or(0)
}

fn levels() -> Vec<Zoom> {
    Zoom::levels().collect()
}

/// La fracción del oráculo de un zoom de la escalera.
fn fraction_of(zoom: Zoom) -> Fraction {
    let index_2 = levels()
        .iter()
        .position(|n| *n == zoom)
        .unwrap_or_else(|| panic!("{zoom} no es un nivel de la escalera"));
    ladder()[index_2]
}

fn hash_de(zoom: Zoom) -> u64 {
    let mut h = DefaultHasher::new();
    zoom.hash(&mut h);
    h.finish()
}

fn zoom_integer(n: u32) -> Zoom {
    Zoom::from_int(n).unwrap_or_else(|| panic!("×{n} debería existir"))
}

fn fractional(k: u32) -> Zoom {
    Zoom::shrink(k).unwrap_or_else(|| panic!("1/{k} debería existir"))
}

/// Vista con `zoom` y exactamente ese `origin` (con `pan`, que hace `clamp`): falla si `clamp` lo
/// habría movido.
fn vista_en(zoom: Zoom, origin: (i64, i64), viewport: Size, image: Size) -> View {
    let mut v = View::fit(viewport, image);
    v.set_zoom(zoom, (0, 0));
    let (ox, oy) = v.origin();
    v.pan(origin.0 - ox, origin.1 - oy, viewport, image);
    assert_eq!(
        v.origin(),
        origin,
        "el origen pedido queda fuera de `clamp`"
    );
    assert_eq!(v.zoom(), zoom);
    v
}

// ---------------------------------------------------------------------------------------------
// A1 · Escalera
// ---------------------------------------------------------------------------------------------

#[test]
fn row_a1_there_are_70_levels_in_increasing_order_with_the_expected_text() {
    let obtained: Vec<Zoom> = Zoom::levels().collect();
    assert_eq!(obtained.len(), 70);
    let expected_2_2: Vec<String> = ladder().into_iter().map(label).collect();
    let texts: Vec<String> = obtained.iter().map(Zoom::to_string).collect();
    assert_eq!(texts, expected_2_2);
    assert!(
        obtained.windows(2).all(|w| w[0] < w[1]),
        "estrictamente creciente"
    );
    assert_eq!(expected_2_2[0], "×1/64");
    assert_eq!(expected_2_2[5], "×1/2");
    assert_eq!(expected_2_2[6], "×1");
    assert_eq!(expected_2_2[69], "×64");
}

#[test]
fn row_a1_levels_is_double_and_cloneable() {
    let walk = Zoom::levels();
    let copy = walk.clone();
    assert_eq!(walk.count(), 70);
    assert_eq!(copy.count(), 70);
    let mut reversed: Vec<Zoom> = Zoom::levels().rev().collect();
    reversed.reverse();
    assert_eq!(reversed, levels());
    assert_eq!(Zoom::levels().next_back(), Some(Zoom::MAX));
    assert_eq!(Zoom::levels().next(), Some(Zoom::MIN));
    // Se puede avanzar por los dos extremos a la vez sin repetir ni perder ninguno.
    let mut double = Zoom::levels();
    let mut seen_2 = Vec::new();
    while let (Some(a), Some(b)) = (double.next(), double.next_back()) {
        seen_2.push(a);
        seen_2.push(b);
    }
    assert_eq!(seen_2.len(), 70);
    assert_eq!(seen_2.iter().copied().collect::<HashSet<_>>().len(), 70);
}

#[test]
fn row_a1_min_is_1_64_and_max_is_x64() {
    assert_eq!(Zoom::MIN.to_string(), "×1/64");
    assert_eq!(Zoom::MIN, fractional(64));
    assert_eq!(Zoom::MAX.to_string(), "×64");
    assert_eq!(Zoom::MAX, zoom_integer(64));
    assert_eq!(Zoom::ONE.to_string(), "×1");
    assert_eq!(Zoom::default(), Zoom::ONE);
    assert!(Zoom::MIN < Zoom::ONE && Zoom::ONE < Zoom::MAX);
    assert_eq!(Zoom::levels().next(), Some(Zoom::MIN));
    assert_eq!(Zoom::levels().last(), Some(Zoom::MAX));
}

#[test]
fn row_a1_zoom_in_and_zoom_out_go_up_and_down_a_level_and_saturate_at_the_extremes() {
    let all_2 = levels();
    for (i, z) in all_2.iter().enumerate() {
        let top = all_2[(i + 1).min(69)];
        let below = all_2[i.saturating_sub(1)];
        assert_eq!(z.zoom_in(), top, "zoom_in de {z}");
        assert_eq!(z.zoom_out(), below, "zoom_out de {z}");
    }
    assert_eq!(Zoom::ONE.zoom_out(), fractional(2));
    assert_eq!(fractional(2).zoom_in(), Zoom::ONE);
    assert_eq!(Zoom::MIN.zoom_out(), Zoom::MIN);
    assert_eq!(Zoom::MAX.zoom_in(), Zoom::MAX);
    assert_eq!(fractional(64).zoom_in(), fractional(32));
    assert_eq!(Zoom::ONE.zoom_in(), zoom_integer(2));
    assert_eq!(zoom_integer(2).zoom_out(), Zoom::ONE);
}

#[test]
fn row_a1_walking_the_ladder_from_end_to_end_gives_69_steps() {
    let mut z = Zoom::MIN;
    for step in 0..69 {
        let next = z.zoom_in();
        assert_ne!(next, z, "paso {step}");
        z = next;
    }
    assert_eq!(z, Zoom::MAX);
    assert_eq!(z.zoom_in(), Zoom::MAX);
    for _ in 0..69 {
        z = z.zoom_out();
    }
    assert_eq!(z, Zoom::MIN);
    assert_eq!(z.zoom_out(), Zoom::MIN);
}

#[test]
fn row_a1_shrink_gives_the_levels_of_the_powers_of_two_and_none_for_the_rest() {
    for k in [2u32, 4, 8, 16, 32, 64] {
        let z = Zoom::shrink(k).unwrap_or_else(|| panic!("shrink({k})"));
        assert_eq!(z.to_string(), format!("×1/{k}"));
        assert_eq!(z.shrink_factor(), Some(k));
        assert!(z < Zoom::ONE);
    }
    for k in [
        0u32,
        1,
        3,
        5,
        6,
        7,
        9,
        12,
        48,
        62,
        63,
        65,
        100,
        128,
        256,
        u32::MAX,
    ] {
        assert_eq!(Zoom::shrink(k), None, "shrink({k})");
    }
    // Los niveles que da shrink son los de la escalera, y en el orden esperado.
    let per_shrink: Vec<Zoom> = [64u32, 32, 16, 8, 4, 2]
        .into_iter()
        .map(fractional)
        .collect();
    assert_eq!(per_shrink, levels()[..6]);
}

#[test]
fn row_a1_shrink_factor_only_exists_at_the_fractional_levels() {
    for (i, z) in levels().iter().enumerate() {
        let expected_2 = (i < 6).then(|| 64u32 >> i);
        assert_eq!(z.shrink_factor(), expected_2, "{z}");
    }
    assert_eq!(Zoom::ONE.shrink_factor(), None);
    assert_eq!(Zoom::MAX.shrink_factor(), None);
    assert_eq!(Zoom::MIN.shrink_factor(), Some(64));
}

#[test]
fn row_a1_from_int_as_today() {
    assert_eq!(Zoom::from_int(0), None);
    assert_eq!(Zoom::from_int(1), Some(Zoom::ONE));
    assert_eq!(Zoom::from_int(64), Some(Zoom::MAX));
    assert_eq!(Zoom::from_int(65), None);
    assert_eq!(Zoom::from_int(u32::MAX), None);
    let all_2 = levels();
    for n in 1..=64u32 {
        let z = zoom_integer(n);
        assert_eq!(z, all_2[5 + n as usize], "×{n} es el nivel {}", 5 + n);
        assert_eq!(z.to_string(), format!("×{n}"));
        assert_eq!(z.shrink_factor(), None);
    }
}

#[test]
fn row_a1_display_order_eq_and_hash_are_consistent() {
    assert_eq!(fractional(8).to_string(), "×1/8");
    assert_eq!(fractional(64).to_string(), "×1/64");
    assert_eq!(zoom_integer(12).to_string(), "×12");

    let all_2 = levels();
    // Eq y Ord coherentes: `cmp` es `Equal` exactamente cuando son iguales, y el orden coincide con
    // la posición en la escalera.
    for (i, a) in all_2.iter().enumerate() {
        for (j, b) in all_2.iter().enumerate() {
            assert_eq!(a == b, i == j);
            assert_eq!(a.cmp(b), i.cmp(&j), "{a} frente a {b}");
            assert_eq!(a.partial_cmp(b), Some(i.cmp(&j)));
        }
    }
    let mut unordered: Vec<Zoom> = all_2.iter().rev().copied().collect();
    unordered.sort();
    assert_eq!(unordered, all_2);

    // Hash: iguales (aunque se construyan por caminos distintos) dan el mismo hash, y los 70 son
    // distintos en un conjunto.
    let different: HashSet<Zoom> = all_2.iter().copied().collect();
    assert_eq!(different.len(), 70);
    let pares = [
        (fractional(2), all_2[5]),
        (Zoom::ONE, zoom_integer(1)),
        (Zoom::ONE, fractional(2).zoom_in()),
        (zoom_integer(2), Zoom::ONE.zoom_in()),
        (Zoom::MIN, Zoom::MIN.zoom_out()),
        (Zoom::MAX, zoom_integer(64)),
    ];
    for (a, b) in pares {
        assert_eq!(a, b);
        assert_eq!(hash_de(a), hash_de(b), "{a}");
    }
}

// ---------------------------------------------------------------------------------------------
// A2 · Aritmética
// ---------------------------------------------------------------------------------------------

#[test]
fn row_a2_at_1_2_the_examples_of_the_spec() {
    let z = fractional(2);
    assert_eq!(z.scale(5), 2);
    assert_eq!(z.unscale(3), 6);
    assert_eq!(z.extent(5), 3);
    assert_eq!(z.extent(4), 2);
    assert_eq!(z.scale(-1), -1, "floor de -0,5");
    assert_eq!(z.extent(-1), 0, "ceil de -0,5");
}

#[test]
fn row_a2_at_1_4_the_examples_of_the_spec() {
    let z = fractional(4);
    assert_eq!(z.scale(-1), -1);
    assert_eq!(z.unscale(-1), -4);
    assert_eq!(z.extent(7), 2);
    assert_eq!(z.extent(8), 2);
    assert_eq!(z.extent(9), 3);
    assert_eq!(z.extent(0), 0);
}

#[test]
fn row_a2_at_1_64_the_examples_of_the_spec() {
    let z = fractional(64);
    assert_eq!(z.scale(63), 0);
    assert_eq!(z.scale(64), 1);
    assert_eq!(z.unscale(2), 128);
    assert_eq!(z.extent(8192), 128);
    assert_eq!(z.extent(6000), 94);
    assert_eq!(z.extent(1), 1);
    assert_eq!(z.extent(64), 1);
    assert_eq!(z.extent(65), 2);
}

#[test]
fn row_a2_rescale_between_levels() {
    // `rescale(n, a)` = floor(n · a / self): lleva una longitud de pantalla de este zoom al de `a`.
    assert_eq!(fractional(2).rescale(3, Zoom::ONE), 6);
    assert_eq!(Zoom::ONE.rescale(5, fractional(2)), 2);
    assert_eq!(fractional(4).rescale(10, fractional(64)), 0);
    assert_eq!(fractional(64).rescale(10, fractional(4)), 160);
    assert_eq!(
        fractional(2).rescale(-3, fractional(4)),
        -2,
        "floor de -1,5"
    );
    assert_eq!(fractional(4).rescale(-3, fractional(2)), -6);
    assert_eq!(
        zoom_integer(3).rescale(7, fractional(2)),
        1,
        "floor(7 · 1/2 / 3)"
    );
    assert_eq!(fractional(2).rescale(100, zoom_integer(3)), 600);
    assert_eq!(Zoom::ONE.rescale(0, Zoom::MAX), 0);
}

#[test]
fn row_a2_with_zoom_x1_or_more_extent_is_scale_and_todays_does_not_change() {
    for n in 1..=64u32 {
        let z = zoom_integer(n);
        for v in [-1000i64, -7, -1, 0, 1, 2, 63, 64, 1000, 8192, 123_456] {
            assert_eq!(z.extent(v), z.scale(v), "{z} extent({v})");
            assert_eq!(z.scale(v), v * i64::from(n), "{z} scale({v})");
            assert_eq!(z.unscale(v), v.div_euclid(i64::from(n)), "{z} unscale({v})");
        }
    }
    assert_eq!(Zoom::ONE.scale(5), 5);
    assert_eq!(zoom_integer(3).unscale(7), 2);
    assert_eq!(zoom_integer(3).unscale(-1), -1);
    assert_eq!(Zoom::ONE.rescale(10, zoom_integer(4)), 40);
}

#[test]
fn row_a2_with_i64_extremes_it_does_not_overflow_and_saturates() {
    for z in levels() {
        let f = fraction_of(z);
        for n in [i64::MIN, i64::MIN + 1, -1, 0, 1, i64::MAX - 1, i64::MAX] {
            assert_eq!(z.scale(n), o_scale(f, n), "{z} scale({n})");
            assert_eq!(z.unscale(n), o_unscale(f, n), "{z} unscale({n})");
            assert_eq!(z.extent(n), o_extent(f, n), "{z} extent({n})");
            for a in [Zoom::MIN, Zoom::ONE, Zoom::MAX] {
                assert_eq!(
                    z.rescale(n, a),
                    o_rescale(f, n, fraction_of(a)),
                    "{z} rescale({n}, {a})"
                );
            }
        }
    }
    assert_eq!(fractional(2).extent(i64::MAX), 1 << 62);
    assert_eq!(fractional(2).extent(i64::MIN), i64::MIN / 2);
    assert_eq!(Zoom::MAX.extent(i64::MAX), i64::MAX);
    assert_eq!(Zoom::MAX.extent(i64::MIN), i64::MIN);
    assert_eq!(fractional(64).unscale(i64::MAX), i64::MAX);
    assert_eq!(fractional(64).unscale(i64::MIN), i64::MIN);
}

// ---------------------------------------------------------------------------------------------
// A3 · Pantalla ↔ imagen a 1/4
// ---------------------------------------------------------------------------------------------

const IMAGE_10: Size = Size {
    width: 10,
    height: 10,
};
const CANVAS_100: Size = Size {
    width: 100,
    height: 100,
};

fn vista_a_1_4() -> View {
    vista_en(fractional(4), (10, 20), CANVAS_100, IMAGE_10)
}

#[test]
fn row_a3_screen_to_image_at_1_4_with_origin_10_20() {
    let v = vista_a_1_4();
    let cases: [(Point, Option<(u32, u32)>); 7] = [
        ((10, 20), Some((0, 0))),
        ((11, 20), Some((4, 0))),
        ((12, 21), Some((8, 4))),
        // El 13 queda fuera: unscale(3) = 12 ≥ 10.
        ((13, 20), None),
        ((9, 20), None),
        ((12, 20), Some((8, 0))),
        ((12, 22), Some((8, 8))),
    ];
    for (p, expected_2) in cases {
        assert_eq!(v.screen_to_image(p, IMAGE_10), expected_2, "{p:?}");
    }
    // La versión con signo no recorta: el 9 es -4 y el 13 es 12.
    assert_eq!(v.screen_to_image_i32((9, 20)), (-4, 0));
    assert_eq!(v.screen_to_image_i32((13, 20)), (12, 0));
    assert_eq!(v.screen_to_image_i32((12, 23)), (8, 12));
}

#[test]
fn row_a3_image_rect_measures_3_by_3_because_pixel_8_is_seen() {
    let v = vista_a_1_4();
    let r = v.image_rect(IMAGE_10);
    assert_eq!((r.x, r.y), (10, 20));
    assert_eq!((r.width, r.height), (3, 3), "⌈10/4⌉ y no ⌊10/4⌋");
    // Los píxeles de pantalla con imagen son justo esos 3×3.
    for sy in 18..26 {
        for sx in 8..16 {
            let inside = (10..13).contains(&sx) && (20..23).contains(&sy);
            assert_eq!(
                v.screen_to_image((sx, sy), IMAGE_10).is_some(),
                inside,
                "({sx}, {sy})"
            );
        }
    }
}

#[test]
fn row_a3_image_to_screen_at_1_4() {
    let v = vista_a_1_4();
    let cases = [
        ((0, 0), (10, 20)),
        ((3, 3), (10, 20)),
        ((4, 0), (11, 20)),
        ((8, 0), (12, 20)),
        ((9, 9), (12, 22)),
    ];
    for ((x, y), expected_2) in cases {
        assert_eq!(v.image_to_screen(x, y), expected_2, "({x}, {y})");
    }
}

#[test]
fn row_a3_round_trip_at_1_4() {
    let v = vista_a_1_4();
    // pantalla → imagen → pantalla es la identidad dentro de la imagen.
    for sy in 20..23 {
        for sx in 10..13 {
            let (x, y) = v
                .screen_to_image((sx, sy), IMAGE_10)
                .unwrap_or_else(|| panic!("({sx}, {sy}) está en la imagen"));
            assert_eq!(v.image_to_screen(x, y), (sx, sy));
        }
    }
    // imagen → pantalla → imagen sube a la rejilla: x − x mod 4.
    for y in 0..10u32 {
        for x in 0..10u32 {
            let (sx, sy) = v.image_to_screen(x, y);
            assert_eq!(
                v.screen_to_image((sx, sy), IMAGE_10),
                Some((x - x % 4, y - y % 4)),
                "({x}, {y})"
            );
        }
    }
    // scale(unscale(s)) = s para todo s (también negativo) y unscale(scale(x)) = x − x mod 4.
    let z = fractional(4);
    for s in -50i64..=50 {
        assert_eq!(z.scale(z.unscale(s)), s, "s = {s}");
    }
    for x in -50i64..=50 {
        assert_eq!(z.unscale(z.scale(x)), x - x.rem_euclid(4), "x = {x}");
    }
}

#[test]
fn row_a3_pixel_block_at_1_4_only_exists_on_the_grid_and_measures_1_by_1() {
    let v = vista_a_1_4();
    assert_eq!(v.pixel_block(0, 0), Some(((10, 20), (11, 21))));
    assert_eq!(v.pixel_block(4, 0), Some(((11, 20), (12, 21))));
    assert_eq!(v.pixel_block(8, 8), Some(((12, 22), (13, 23))));
    assert_eq!(v.pixel_block(0, 4), Some(((10, 21), (11, 22))));
    for (x, y) in [(1, 0), (0, 3), (3, 3), (5, 4), (4, 6), (9, 9), (2, 8)] {
        assert_eq!(v.pixel_block(x, y), None, "({x}, {y}) no es de la rejilla");
    }
}

#[test]
fn row_a3_pixel_block_at_integer_zoom_is_zoom_by_zoom_and_at_1_2_and_1_64_it_follows_the_grid() {
    let v = vista_en(zoom_integer(3), (10, 20), CANVAS_100, IMAGE_10);
    assert_eq!(v.pixel_block(2, 1), Some(((16, 23), (19, 26))));
    assert_eq!(v.pixel_block(0, 0), Some(((10, 20), (13, 23))));
    let v = vista_en(Zoom::ONE, (10, 20), CANVAS_100, IMAGE_10);
    assert_eq!(v.pixel_block(5, 5), Some(((15, 25), (16, 26))));
    let v = vista_en(fractional(2), (10, 20), CANVAS_100, IMAGE_10);
    assert_eq!(v.pixel_block(6, 2), Some(((13, 21), (14, 22))));
    assert_eq!(v.pixel_block(6, 3), None);
    assert_eq!(v.pixel_block(5, 2), None);
    let mut v = View::fit(CANVAS_100, Size::new(256, 256));
    v.set_zoom(Zoom::MIN, (0, 0));
    let (ox, oy) = v.origin();
    assert_eq!(
        v.pixel_block(64, 128),
        Some(((ox + 1, oy + 2), (ox + 2, oy + 3)))
    );
    assert_eq!(v.pixel_block(64, 127), None);
    assert_eq!(v.pixel_block(0, 0), Some(((ox, oy), (ox + 1, oy + 1))));
}

// ---------------------------------------------------------------------------------------------
// A4 · Ajustar
// ---------------------------------------------------------------------------------------------

/// `View::fit` da el nivel esperado, el mayor que cabe, y centra con diferencia ≤ 1.
fn checks_fit(viewport: Size, image: Size, expected_2: &str) {
    let v = View::fit(viewport, image);
    let label_2 = format!(
        "{}×{} en {}×{}",
        image.width, image.height, viewport.width, viewport.height
    );
    assert_eq!(v.zoom().to_string(), expected_2, "{label_2}");
    let f = fraction_of(v.zoom());
    assert_eq!(
        v.zoom(),
        levels()[o_better_level(viewport, image)],
        "{label_2}: el mayor nivel que cabe"
    );
    let (ext_x, ext_y) = (
        o_extent(f, i64::from(image.width)),
        o_extent(f, i64::from(image.height)),
    );
    assert!(
        (2 * v.origin().0 + ext_x - i64::from(viewport.width)).abs() <= 1,
        "{label_2}: centrada en x (origen {:?}, extent {ext_x})",
        v.origin()
    );
    assert!(
        (2 * v.origin().1 + ext_y - i64::from(viewport.height)).abs() <= 1,
        "{label_2}: centrada en y (origen {:?}, extent {ext_y})",
        v.origin()
    );
}

#[test]
fn row_a4_the_five_images_of_the_table_in_the_three_canvases() {
    let canvases = [
        Size::new(620, 684),
        Size::new(1600, 900),
        Size::new(128, 297),
    ];
    let tabla: [(u32, [&str; 3]); 5] = [
        (1024, ["×1/2", "×1/2", "×1/8"]),
        (2048, ["×1/4", "×1/4", "×1/16"]),
        (4096, ["×1/8", "×1/8", "×1/32"]),
        (6000, ["×1/16", "×1/8", "×1/64"]),
        (8192, ["×1/16", "×1/16", "×1/64"]),
    ];
    for (side, expected_2_2) in tabla {
        for (canvas_2, expected_2) in canvases.iter().zip(expected_2_2) {
            checks_fit(*canvas_2, Size::new(side, side), expected_2);
        }
    }
}

#[test]
fn row_a4_concrete_examples_of_origin_and_exact_fit() {
    // 4096² en 128×297: 1/32 exacto (128), centrada en alto: (297 − 128) / 2 = 84,5.
    let v = View::fit(Size::new(128, 297), Size::new(4096, 4096));
    assert_eq!(v.zoom(), fractional(32));
    assert_eq!(v.image_rect(Size::new(4096, 4096)).width, 128);
    assert_eq!(v.origin().0, 0);
    assert!(v.origin().1 == 84 || v.origin().1 == 85);
    // 64×48 en 640×480: ×10 como hoy, sin márgenes.
    let v = View::fit(Size::new(640, 480), Size::new(64, 48));
    assert_eq!(v.zoom(), zoom_integer(10));
    assert_eq!(v.origin(), (0, 0));
    // 6000² en 1600×900: 1/8 (750).
    let v = View::fit(Size::new(1600, 900), Size::new(6000, 6000));
    assert_eq!(v.zoom(), fractional(8));
    assert_eq!(v.image_rect(Size::new(6000, 6000)).width, 750);
}

#[test]
fn row_a4_16384_in_100x100_gives_1_64_and_origin_minus_78() {
    let image_2 = Size::new(16384, 16384);
    let v = View::fit(Size::new(100, 100), image_2);
    assert_eq!(v.zoom(), Zoom::MIN);
    assert_eq!(v.origin(), (-78, -78), "256 de imagen centrados en 100");
    assert_eq!(v.image_rect(image_2).width, 256);
}

#[test]
fn row_a4_3x3_in_1x1_gives_1_4() {
    let v = View::fit(Size::new(1, 1), Size::new(3, 3));
    assert_eq!(v.zoom(), fractional(4));
    assert_eq!(v.image_rect(Size::new(3, 3)).width, 1);
    assert_eq!(v.origin(), (0, 0));
}

#[test]
fn row_a4_viewport_0x0_gives_the_minimum() {
    for image_2 in [Size::new(5, 5), Size::new(64, 48), Size::new(8192, 8192)] {
        let v = View::fit(Size::new(0, 0), image_2);
        assert_eq!(v.zoom(), Zoom::MIN, "{image_2:?}");
    }
    // Y un lienzo de un solo eje vacío tampoco cabe en ningún nivel.
    assert_eq!(
        View::fit(Size::new(0, 500), Size::new(10, 10)).zoom(),
        Zoom::MIN
    );
    assert_eq!(
        View::fit(Size::new(500, 0), Size::new(10, 10)).zoom(),
        Zoom::MIN
    );
}

#[test]
fn row_a4_the_integer_zooms_stay_as_today_when_they_fit() {
    // Imágenes pequeñas: el mayor entero que cabe (hasta ×64), centrado.
    checks_fit(Size::new(700, 500), Size::new(64, 48), "×10");
    checks_fit(Size::new(640, 480), Size::new(64, 48), "×10");
    checks_fit(Size::new(1000, 1000), Size::new(1, 1), "×64");
    checks_fit(Size::new(100, 100), Size::new(100, 100), "×1");
    checks_fit(Size::new(99, 100), Size::new(100, 100), "×1/2");
    checks_fit(Size::new(150, 300), Size::new(10, 10), "×15");
}

// ---------------------------------------------------------------------------------------------
// A5 · Ancla
// ---------------------------------------------------------------------------------------------

const IMAGE_1000: Size = Size {
    width: 1000,
    height: 1000,
};
const CANVAS_800X600: Size = Size {
    width: 800,
    height: 600,
};

fn from_x_1() -> View {
    vista_en(Zoom::ONE, (0, 0), CANVAS_800X600, IMAGE_1000)
}

#[test]
fn row_a5_exact_anchor_at_1_2_at_100_50() {
    let mut v = from_x_1();
    v.set_zoom(fractional(2), (100, 50));
    assert_eq!(v.origin(), (50, 25));
    assert_eq!(v.screen_to_image((100, 50), IMAGE_1000), Some((100, 50)));
}

#[test]
fn row_a5_anchor_at_1_2_at_101_51_goes_up_to_the_grid() {
    let mut v = from_x_1();
    v.set_zoom(fractional(2), (101, 51));
    assert_eq!(v.origin(), (51, 26));
    // El 101 sube a la rejilla de 2: el píxel bajo el ancla es el 100.
    assert_eq!(v.screen_to_image((101, 51), IMAGE_1000), Some((100, 50)));
}

#[test]
fn row_a5_anchor_at_1_64_at_100_50() {
    let mut v = from_x_1();
    v.set_zoom(Zoom::MIN, (100, 50));
    assert_eq!(v.origin(), (99, 50));
    // ⌊100/64⌋·64 = 64 y ⌊50/64⌋·64 = 0.
    assert_eq!(v.screen_to_image((100, 50), IMAGE_1000), Some((64, 0)));
}

#[test]
fn row_a5_round_trip_is_not_the_identity_but_the_pixel_is_the_grid_one() {
    let mut v = from_x_1();
    v.set_zoom(fractional(2), (101, 51));
    assert_eq!(v.zoom(), fractional(2));
    v.zoom_in((101, 51));
    assert_eq!(v.zoom(), Zoom::ONE);
    assert_eq!(v.origin(), (1, 1));
    assert_eq!(v.screen_to_image((101, 51), IMAGE_1000), Some((100, 50)));
    assert_ne!(v, from_x_1(), "alejar y acercar no es la identidad");
}

#[test]
fn row_a5_zooming_in_to_an_integer_keeps_the_exact_pixel() {
    let mut v = from_x_1();
    v.set_zoom(Zoom::MIN, (100, 50));
    let pixel = v.screen_to_image_i32((100, 50));
    assert_eq!(pixel, (64, 0));
    for _ in 0..7 {
        v.zoom_in((100, 50));
        assert_eq!(v.screen_to_image_i32((100, 50)), pixel, "{}", v.zoom());
    }
    assert_eq!(v.zoom(), zoom_integer(2));
}

// ---------------------------------------------------------------------------------------------
// Estrategias
// ---------------------------------------------------------------------------------------------

/// Un `i64` cualquiera, con peso en los extremos y en los valores pequeños.
fn end() -> impl Strategy<Value = i64> {
    prop_oneof![
        4 => any::<i64>(),
        1 => Just(i64::MIN),
        1 => Just(i64::MAX),
        1 => Just(0i64),
        2 => -200i64..=200,
        1 => i64::MIN..i64::MIN + 100,
        1 => i64::MAX - 100..=i64::MAX,
        2 => -100_000i64..=100_000,
    ]
}

fn dimension() -> impl Strategy<Value = u32> {
    prop_oneof![
        3 => 1u32..=70,
        2 => 1u32..=20_000,
        1 => 1u32..=u32::MAX,
    ]
}

// ---------------------------------------------------------------------------------------------
// P1 · Conversión exacta
// ---------------------------------------------------------------------------------------------

/// Los valores sin saturar de `scale`/`unscale` caben si `|v| ≤ 2^56`.
const WITHOUT_SATURATE: i64 = 1 << 56;

proptest! {
    #![proptest_config(ProptestConfig::with_cases(256))]

    #[test]
    fn property_p1_the_conversion_matches_the_oracle_at_the_70_levels(
        previous in 0usize..70,
        anchor_preview in (end(), end()),
        anchor in (end(), end()),
        point in (end(), end()),
        n in end(),
        image_2 in (dimension(), dimension()),
        image_pixel in (any::<u32>(), any::<u32>()),
    ) {
        let all_2 = levels();
        let fractions = ladder();
        let tam = Size::new(image_2.0, image_2.1);
        // Un origen cualquiera: el que dejan dos `set_zoom` con anclas arbitrarias.
        let mut base = View::fit(Size::new(640, 480), tam);
        base.set_zoom(all_2[previous], anchor_preview);
        for (i, (&z, &f)) in all_2.iter().zip(&fractions).enumerate() {
            prop_assert_eq!(z.to_string(), label(f));
            // scale, unscale, extent.
            for v in [n, point.0, point.1] {
                prop_assert_eq!(z.scale(v), o_scale(f, v), "{} scale({})", z, v);
                prop_assert_eq!(z.unscale(v), o_unscale(f, v), "{} unscale({})", z, v);
                prop_assert_eq!(z.extent(v), o_extent(f, v), "{} extent({})", z, v);
                prop_assert_eq!(
                    all_2[previous].rescale(v, z),
                    o_rescale(fractions[previous], v, f),
                    "{} rescale({}, {})", all_2[previous], v, z
                );
            }
            // Retracción.
            let s = n.clamp(-WITHOUT_SATURATE, WITHOUT_SATURATE);
            if i < 6 {
                let k = i64::try_from(f.1).unwrap_or(1);
                prop_assert_eq!(z.scale(z.unscale(s)), s, "{} scale(unscale({}))", z, s);
                prop_assert_eq!(
                    z.unscale(z.scale(s)),
                    s - s.rem_euclid(k),
                    "{} unscale(scale({}))", z, s
                );
            } else {
                let zi = f.0 as i64;
                prop_assert_eq!(z.unscale(z.scale(s)), s, "{} unscale(scale({}))", z, s);
                prop_assert_eq!(
                    z.scale(z.unscale(s)),
                    s - s.rem_euclid(zi),
                    "{} scale(unscale({}))", z, s
                );
            }
            // Vista con origen arbitrario.
            let mut v = base;
            v.set_zoom(z, anchor);
            let source_2 = v.origin();
            let expected_2 = (
                o_axis(f, point.0, source_2.0, tam.width),
                o_axis(f, point.1, source_2.1, tam.height),
            );
            let expected_2 = match expected_2 {
                (Some(x), Some(y)) => Some((x, y)),
                _ => None,
            };
            prop_assert_eq!(v.screen_to_image(point, tam), expected_2, "{} origen {:?}", z, source_2);
            prop_assert_eq!(
                v.screen_to_image_i32(point),
                (or_axis_i32(f, point.0, source_2.0), or_axis_i32(f, point.1, source_2.1)),
                "{} origen {:?}", z, source_2
            );
            // image_to_screen y image_rect.
            let (x, y) = image_pixel;
            prop_assert_eq!(
                v.image_to_screen(x, y),
                (
                    saturates(i128::from(source_2.0) + i128::from(o_scale(f, i64::from(x)))),
                    saturates(i128::from(source_2.1) + i128::from(o_scale(f, i64::from(y)))),
                ),
                "{} image_to_screen", z
            );
            let r = v.image_rect(tam);
            prop_assert_eq!((r.x, r.y), source_2);
            prop_assert_eq!(r.width, o_extent(f, i64::from(tam.width)), "{} ancho", z);
            prop_assert_eq!(r.height, o_extent(f, i64::from(tam.height)), "{} alto", z);
            // pixel_block, con orígenes que no desbordan la suma.
            if source_2.0.unsigned_abs() < 1 << 60 && source_2.1.unsigned_abs() < 1 << 60 {
                let (sx, sy) = (
                    source_2.0 + o_scale(f, i64::from(x)),
                    source_2.1 + o_scale(f, i64::from(y)),
                );
                if i < 6 {
                    let k = f.1 as u32;
                    let aligned = (x / k * k, y / k * k);
                    prop_assert_eq!(
                        v.pixel_block(aligned.0, aligned.1).map(|b| (b.1.0 - b.0.0, b.1.1 - b.0.1)),
                        Some((1, 1)),
                        "{} 1×1 en la rejilla", z
                    );
                    prop_assert_eq!(
                        v.pixel_block(aligned.0, aligned.1).map(|b| b.0),
                        Some((
                            source_2.0 + o_scale(f, i64::from(aligned.0)),
                            source_2.1 + o_scale(f, i64::from(aligned.1)),
                        ))
                    );
                    if x % k != 0 || y % k != 0 {
                        prop_assert_eq!(v.pixel_block(x, y), None, "{} fuera de la rejilla", z);
                    }
                } else {
                    let side = f.0 as i64;
                    prop_assert_eq!(
                        v.pixel_block(x, y),
                        Some(((sx, sy), (sx + side, sy + side))),
                        "{} zoom×zoom", z
                    );
                }
            }
        }
    }
}

// ---------------------------------------------------------------------------------------------
// P2 · Ancla
// ---------------------------------------------------------------------------------------------

/// Granularidad del nivel nuevo: su `K` si es fraccionario y 1 si no.
fn grid(index_2: usize) -> i32 {
    if index_2 < 6 { 1 << (6 - index_2) } else { 1 }
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(256))]

    #[test]
    fn property_p2_the_pixel_under_the_anchor_goes_up_to_the_grid_of_the_new_level(
        from_2 in 0usize..70,
        until in 0usize..70,
        anchor_preview in (-3000i64..=3000, -3000i64..=3000),
        anchor in (-3000i64..=3000, -3000i64..=3000),
        image_2 in (1u32..=5000, 1u32..=5000),
    ) {
        let all_2 = levels();
        let tam = Size::new(image_2.0, image_2.1);
        let mut v = View::fit(Size::new(800, 600), tam);
        v.set_zoom(all_2[from_2], anchor_preview);
        prop_assert_eq!(v.zoom(), all_2[from_2]);
        let antes = v.screen_to_image_i32(anchor);
        v.set_zoom(all_2[until], anchor);
        prop_assert_eq!(v.zoom(), all_2[until]);
        let after_2 = v.screen_to_image_i32(anchor);
        let g = grid(until);
        prop_assert_eq!(
            after_2,
            (antes.0.div_euclid(g) * g, antes.1.div_euclid(g) * g),
            "{} → {}: antes {:?}", all_2[from_2], all_2[until], antes
        );
        if until >= 6 {
            prop_assert_eq!(after_2, antes, "con un nivel ≥ ×1 el píxel es exacto");
        }
        // Un paso de zoom_in / zoom_out con la misma regla.
        let mut w = View::fit(Size::new(800, 600), tam);
        w.set_zoom(all_2[from_2], anchor_preview);
        let pixel = w.screen_to_image_i32(anchor);
        w.zoom_in(anchor);
        let g = grid((from_2 + 1).min(69));
        prop_assert_eq!(
            w.screen_to_image_i32(anchor),
            (pixel.0.div_euclid(g) * g, pixel.1.div_euclid(g) * g),
            "zoom_in desde {}", all_2[from_2]
        );
        let mut w = View::fit(Size::new(800, 600), tam);
        w.set_zoom(all_2[from_2], anchor_preview);
        w.zoom_out(anchor);
        let g = grid(from_2.saturating_sub(1));
        prop_assert_eq!(
            w.screen_to_image_i32(anchor),
            (pixel.0.div_euclid(g) * g, pixel.1.div_euclid(g) * g),
            "zoom_out desde {}", all_2[from_2]
        );
    }

    #[test]
    fn property_p2_with_extreme_anchors_the_origin_does_not_overflow(
        from_2 in 0usize..70,
        until in 0usize..70,
        anchor_preview in (end(), end()),
        anchor in (end(), end()),
    ) {
        // En una compilación de depuración un desbordamiento aritmético sería un pánico.
        let all_2 = levels();
        let tam = Size::new(1000, 1000);
        let mut v = View::fit(Size::new(800, 600), tam);
        v.set_zoom(all_2[from_2], anchor_preview);
        v.set_zoom(all_2[until], anchor);
        prop_assert_eq!(v.zoom(), all_2[until]);
        v.zoom_in(anchor);
        v.zoom_out(anchor);
        let _ = v.origin();
        let _ = v.screen_to_image_i32(anchor);
        let _ = v.image_rect(tam);
    }
}

// ---------------------------------------------------------------------------------------------
// P3 · Ajustar
// ---------------------------------------------------------------------------------------------

proptest! {
    #![proptest_config(ProptestConfig::with_cases(256))]

    #[test]
    fn property_p3_fit_is_the_largest_level_that_fits_and_centers(
        canvas_2 in (0u32..=3000, 0u32..=3000),
        image_2 in (dimension(), dimension()),
    ) {
        let viewport = Size::new(canvas_2.0, canvas_2.1);
        let tam = Size::new(image_2.0, image_2.1);
        let v = View::fit(viewport, tam);
        let better = o_better_level(viewport, tam);
        let all_2 = levels();
        prop_assert_eq!(v.zoom(), all_2[better], "esperado {}", label(ladder()[better]));
        let f = ladder()[better];
        let (ext_x, ext_y) = (o_extent(f, i64::from(tam.width)), o_extent(f, i64::from(tam.height)));
        let (fits_x, fits_y) = (ext_x <= i64::from(canvas_2.0), ext_y <= i64::from(canvas_2.1));
        if better > 0 {
            prop_assert!(fits_x && fits_y, "el nivel elegido cabe");
        }
        // Es el mayor: el siguiente no cabe (salvo que ya sea ×64).
        if better < 69 {
            let g = ladder()[better + 1];
            prop_assert!(
                o_extent(g, i64::from(tam.width)) > i64::from(canvas_2.0)
                    || o_extent(g, i64::from(tam.height)) > i64::from(canvas_2.1),
                "el nivel {} también cabe", label(g)
            );
        }
        // Centrada con diferencia ≤ 1.
        prop_assert!((2 * v.origin().0 + ext_x - i64::from(canvas_2.0)).abs() <= 1);
        prop_assert!((2 * v.origin().1 + ext_y - i64::from(canvas_2.1)).abs() <= 1);
        // Y el rectángulo de la imagen mide `extent`.
        let r = v.image_rect(tam);
        prop_assert_eq!((r.width, r.height), (ext_x, ext_y));
    }
}

// ---------------------------------------------------------------------------------------------
// P4 · Cobertura y visibilidad
// ---------------------------------------------------------------------------------------------

#[derive(Clone, Debug)]
enum Op {
    SetZoom(usize, (i64, i64)),
    ZoomIn((i64, i64)),
    ZoomOut((i64, i64)),
    Pan(i64, i64),
    Clamp,
    Fit,
}

fn anchor() -> impl Strategy<Value = (i64, i64)> {
    (-3000i64..=3000, -3000i64..=3000)
}

fn op() -> impl Strategy<Value = Op> {
    prop_oneof![
        3 => (0usize..70, anchor()).prop_map(|(i, a)| Op::SetZoom(i, a)),
        2 => anchor().prop_map(Op::ZoomIn),
        3 => anchor().prop_map(Op::ZoomOut),
        3 => (-3000i64..=3000, -3000i64..=3000).prop_map(|(x, y)| Op::Pan(x, y)),
        1 => Just(Op::Clamp),
        1 => Just(Op::Fit),
    ]
}

fn applies(v: &mut View, op: &Op, viewport: Size, image: Size) {
    let all_2 = levels();
    match op {
        Op::SetZoom(i, a) => v.set_zoom(all_2[*i], *a),
        Op::ZoomIn(a) => v.zoom_in(*a),
        Op::ZoomOut(a) => v.zoom_out(*a),
        Op::Pan(x, y) => v.pan(*x, *y, viewport, image),
        Op::Clamp => v.clamp(viewport, image),
        Op::Fit => *v = View::fit(viewport, image),
    }
}

/// Posiciones de un eje que merece la pena mirar alrededor de `[source_2, source_2 + extent)`: todas si
/// el tramo es corto y los bordes y unas muestras si es largo.
fn positions(source_2: i64, extent: i64) -> Vec<i64> {
    let mut v: Vec<i64> = Vec::new();
    if extent <= 3000 {
        v.extend(source_2 - 3..source_2 + extent + 3);
    } else {
        v.extend(source_2 - 3..source_2 + 40);
        v.extend(source_2 + extent - 40..source_2 + extent + 3);
        v.extend((1..=50).map(|i| source_2 + extent * i / 51));
    }
    v
}

/// Los píxeles de pantalla con `screen_to_image = Some` son exactamente `[source_2, source_2 + extent)`
/// en cada eje.
fn checks_coverage(v: &View, image: Size) -> Result<(), TestCaseError> {
    let f = fraction_of(v.zoom());
    let r = v.image_rect(image);
    let (ext_x, ext_y) = (
        o_extent(f, i64::from(image.width)),
        o_extent(f, i64::from(image.height)),
    );
    prop_assert_eq!((r.width, r.height), (ext_x, ext_y), "image_rect usa extent");
    let (ox, oy) = v.origin();
    for x in positions(ox, ext_x) {
        let inside = ox <= x && x < ox + ext_x;
        prop_assert_eq!(
            v.screen_to_image((x, oy), image).is_some(),
            inside,
            "x = {} (origen {}, extent {}, {})",
            x,
            ox,
            ext_x,
            v.zoom()
        );
    }
    for y in positions(oy, ext_y) {
        let inside = oy <= y && y < oy + ext_y;
        prop_assert_eq!(
            v.screen_to_image((ox, y), image).is_some(),
            inside,
            "y = {} (origen {}, extent {}, {})",
            y,
            oy,
            ext_y,
            v.zoom()
        );
    }
    Ok(())
}

/// Píxeles de la imagen que caen dentro de `[0, canvas_2)` en un eje.
fn visible_2(source_2: i64, extent: i64, canvas_2: u32) -> i64 {
    (source_2 + extent).min(i64::from(canvas_2)) - source_2.max(0)
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(256))]

    #[test]
    fn property_p4_coverage_without_gaps_or_leftovers_and_clamp_leaves_the_minimum_visible(
        image_2 in (1u32..=5000, 1u32..=5000),
        canvas_2 in (1u32..=2000, 1u32..=2000),
        ops in proptest::collection::vec(op(), 0..12),
    ) {
        let tam = Size::new(image_2.0, image_2.1);
        let viewport = Size::new(canvas_2.0, canvas_2.1);
        let mut v = View::fit(viewport, tam);
        for o in &ops {
            applies(&mut v, o, viewport, tam);
        }
        checks_coverage(&v, tam)?;

        // clamp: visibles min(16, extent, lienzo) por eje, y es idempotente.
        let f = fraction_of(v.zoom());
        let mut c = v;
        c.clamp(viewport, tam);
        checks_coverage(&c, tam)?;
        let (ext_x, ext_y) = (
            o_extent(f, i64::from(tam.width)),
            o_extent(f, i64::from(tam.height)),
        );
        let minimum_x = i64::from(MIN_VISIBLE).min(ext_x).min(i64::from(viewport.width));
        let minimum_y = i64::from(MIN_VISIBLE).min(ext_y).min(i64::from(viewport.height));
        prop_assert_eq!(MIN_VISIBLE, 16);
        prop_assert!(
            visible_2(c.origin().0, ext_x, viewport.width) >= minimum_x,
            "x: {} visibles y debían ser ≥ {} (origen {:?}, extent {}, lienzo {})",
            visible_2(c.origin().0, ext_x, viewport.width), minimum_x, c.origin(), ext_x, viewport.width
        );
        prop_assert!(
            visible_2(c.origin().1, ext_y, viewport.height) >= minimum_y,
            "y: {} visibles y debían ser ≥ {} (origen {:?}, extent {}, lienzo {})",
            visible_2(c.origin().1, ext_y, viewport.height), minimum_y, c.origin(), ext_y, viewport.height
        );
        let mut other = c;
        other.clamp(viewport, tam);
        prop_assert_eq!(other, c, "clamp es idempotente");
    }
}
