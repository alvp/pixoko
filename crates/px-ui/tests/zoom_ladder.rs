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
//!   ancho`): `zoom × zoom` con zoom ≥ ×1 y 1×1 a 1/K.
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
type Fraccion = (i128, i128);

/// Punto de pantalla.
type Punto = (i64, i64);

/// Los 70 niveles de la decisión 2, de menor a mayor: 1/64, 1/32, …, 1/2, ×1, ×2, …, ×64.
fn escalera() -> Vec<Fraccion> {
    let mut niveles: Vec<Fraccion> = [64, 32, 16, 8, 4, 2].iter().map(|&k| (1, k)).collect();
    niveles.extend((1..=64).map(|n| (n, 1)));
    niveles
}

/// Texto de un nivel: `×12` o `×1/8`.
fn rotulo((num, den): Fraccion) -> String {
    if den == 1 {
        format!("×{num}")
    } else {
        format!("×{num}/{den}")
    }
}

/// `floor(n / d)` con `d > 0`.
fn suelo(n: i128, d: i128) -> i128 {
    let q = n / d;
    if n % d != 0 && n < 0 { q - 1 } else { q }
}

/// `ceil(n / d)` con `d > 0`.
fn techo(n: i128, d: i128) -> i128 {
    -suelo(-n, d)
}

fn satura(v: i128) -> i64 {
    v.clamp(i128::from(i64::MIN), i128::from(i64::MAX)) as i64
}

fn satura_i32(v: i128) -> i32 {
    v.clamp(i128::from(i32::MIN), i128::from(i32::MAX)) as i32
}

fn o_scale(f: Fraccion, n: i64) -> i64 {
    satura(suelo(i128::from(n) * f.0, f.1))
}

fn o_unscale(f: Fraccion, n: i64) -> i64 {
    satura(suelo(i128::from(n) * f.1, f.0))
}

fn o_extent(f: Fraccion, n: i64) -> i64 {
    satura(techo(i128::from(n) * f.0, f.1))
}

/// `floor(n · to / self)`.
fn o_rescale(desde: Fraccion, n: i64, hasta: Fraccion) -> i64 {
    satura(suelo(i128::from(n) * hasta.0 * desde.1, hasta.1 * desde.0))
}

/// Píxel de imagen bajo el punto `p` de un eje (origen `origen`, `dim` píxeles), o `None`.
fn o_eje(f: Fraccion, p: i64, origen: i64, dim: u32) -> Option<u32> {
    let d = i128::from(p) - i128::from(origen);
    if d < 0 {
        return None;
    }
    let indice = suelo(d * f.1, f.0);
    (indice < i128::from(dim)).then_some(indice as u32)
}

fn o_eje_i32(f: Fraccion, p: i64, origen: i64) -> i32 {
    let d = i128::from(p) - i128::from(origen);
    satura_i32(suelo(d * f.1, f.0))
}

/// Índice del mayor nivel con `extent ≤ lienzo` en los dos ejes (0, el mínimo, si ninguno cabe).
fn o_mejor_nivel(viewport: Size, image: Size) -> usize {
    let niveles = escalera();
    (0..niveles.len())
        .rev()
        .find(|&i| {
            o_extent(niveles[i], i64::from(image.width)) <= i64::from(viewport.width)
                && o_extent(niveles[i], i64::from(image.height)) <= i64::from(viewport.height)
        })
        .unwrap_or(0)
}

fn niveles() -> Vec<Zoom> {
    Zoom::levels().collect()
}

/// La fracción del oráculo de un zoom de la escalera.
fn fraccion_de(zoom: Zoom) -> Fraccion {
    let indice = niveles()
        .iter()
        .position(|n| *n == zoom)
        .unwrap_or_else(|| panic!("{zoom} no es un nivel de la escalera"));
    escalera()[indice]
}

fn hash_de(zoom: Zoom) -> u64 {
    let mut h = DefaultHasher::new();
    zoom.hash(&mut h);
    h.finish()
}

fn zoom_entero(n: u32) -> Zoom {
    Zoom::from_int(n).unwrap_or_else(|| panic!("×{n} debería existir"))
}

fn fraccionario(k: u32) -> Zoom {
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
fn fila_a1_hay_70_niveles_en_orden_creciente_con_el_texto_esperado() {
    let obtenidos: Vec<Zoom> = Zoom::levels().collect();
    assert_eq!(obtenidos.len(), 70);
    let esperados: Vec<String> = escalera().into_iter().map(rotulo).collect();
    let textos: Vec<String> = obtenidos.iter().map(Zoom::to_string).collect();
    assert_eq!(textos, esperados);
    assert!(
        obtenidos.windows(2).all(|w| w[0] < w[1]),
        "estrictamente creciente"
    );
    assert_eq!(esperados[0], "×1/64");
    assert_eq!(esperados[5], "×1/2");
    assert_eq!(esperados[6], "×1");
    assert_eq!(esperados[69], "×64");
}

#[test]
fn fila_a1_levels_es_doble_y_clonable() {
    let recorrido = Zoom::levels();
    let copia = recorrido.clone();
    assert_eq!(recorrido.count(), 70);
    assert_eq!(copia.count(), 70);
    let mut al_reves: Vec<Zoom> = Zoom::levels().rev().collect();
    al_reves.reverse();
    assert_eq!(al_reves, niveles());
    assert_eq!(Zoom::levels().next_back(), Some(Zoom::MAX));
    assert_eq!(Zoom::levels().next(), Some(Zoom::MIN));
    // Se puede avanzar por los dos extremos a la vez sin repetir ni perder ninguno.
    let mut doble = Zoom::levels();
    let mut vistos = Vec::new();
    while let (Some(a), Some(b)) = (doble.next(), doble.next_back()) {
        vistos.push(a);
        vistos.push(b);
    }
    assert_eq!(vistos.len(), 70);
    assert_eq!(vistos.iter().copied().collect::<HashSet<_>>().len(), 70);
}

#[test]
fn fila_a1_min_es_1_64_y_max_es_x64() {
    assert_eq!(Zoom::MIN.to_string(), "×1/64");
    assert_eq!(Zoom::MIN, fraccionario(64));
    assert_eq!(Zoom::MAX.to_string(), "×64");
    assert_eq!(Zoom::MAX, zoom_entero(64));
    assert_eq!(Zoom::ONE.to_string(), "×1");
    assert_eq!(Zoom::default(), Zoom::ONE);
    assert!(Zoom::MIN < Zoom::ONE && Zoom::ONE < Zoom::MAX);
    assert_eq!(Zoom::levels().next(), Some(Zoom::MIN));
    assert_eq!(Zoom::levels().last(), Some(Zoom::MAX));
}

#[test]
fn fila_a1_zoom_in_y_zoom_out_suben_y_bajan_un_nivel_y_saturan_en_los_extremos() {
    let todos = niveles();
    for (i, z) in todos.iter().enumerate() {
        let arriba = todos[(i + 1).min(69)];
        let abajo = todos[i.saturating_sub(1)];
        assert_eq!(z.zoom_in(), arriba, "zoom_in de {z}");
        assert_eq!(z.zoom_out(), abajo, "zoom_out de {z}");
    }
    assert_eq!(Zoom::ONE.zoom_out(), fraccionario(2));
    assert_eq!(fraccionario(2).zoom_in(), Zoom::ONE);
    assert_eq!(Zoom::MIN.zoom_out(), Zoom::MIN);
    assert_eq!(Zoom::MAX.zoom_in(), Zoom::MAX);
    assert_eq!(fraccionario(64).zoom_in(), fraccionario(32));
    assert_eq!(Zoom::ONE.zoom_in(), zoom_entero(2));
    assert_eq!(zoom_entero(2).zoom_out(), Zoom::ONE);
}

#[test]
fn fila_a1_recorrer_la_escalera_de_extremo_a_extremo_da_69_pasos() {
    let mut z = Zoom::MIN;
    for paso in 0..69 {
        let siguiente = z.zoom_in();
        assert_ne!(siguiente, z, "paso {paso}");
        z = siguiente;
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
fn fila_a1_shrink_da_los_niveles_de_las_potencias_de_dos_y_none_en_el_resto() {
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
    let por_shrink: Vec<Zoom> = [64u32, 32, 16, 8, 4, 2]
        .into_iter()
        .map(fraccionario)
        .collect();
    assert_eq!(por_shrink, niveles()[..6]);
}

#[test]
fn fila_a1_shrink_factor_solo_existe_en_los_niveles_fraccionarios() {
    for (i, z) in niveles().iter().enumerate() {
        let esperado = (i < 6).then(|| 64u32 >> i);
        assert_eq!(z.shrink_factor(), esperado, "{z}");
    }
    assert_eq!(Zoom::ONE.shrink_factor(), None);
    assert_eq!(Zoom::MAX.shrink_factor(), None);
    assert_eq!(Zoom::MIN.shrink_factor(), Some(64));
}

#[test]
fn fila_a1_from_int_como_hoy() {
    assert_eq!(Zoom::from_int(0), None);
    assert_eq!(Zoom::from_int(1), Some(Zoom::ONE));
    assert_eq!(Zoom::from_int(64), Some(Zoom::MAX));
    assert_eq!(Zoom::from_int(65), None);
    assert_eq!(Zoom::from_int(u32::MAX), None);
    let todos = niveles();
    for n in 1..=64u32 {
        let z = zoom_entero(n);
        assert_eq!(z, todos[5 + n as usize], "×{n} es el nivel {}", 5 + n);
        assert_eq!(z.to_string(), format!("×{n}"));
        assert_eq!(z.shrink_factor(), None);
    }
}

#[test]
fn fila_a1_display_orden_eq_y_hash_son_coherentes() {
    assert_eq!(fraccionario(8).to_string(), "×1/8");
    assert_eq!(fraccionario(64).to_string(), "×1/64");
    assert_eq!(zoom_entero(12).to_string(), "×12");

    let todos = niveles();
    // Eq y Ord coherentes: `cmp` es `Equal` exactamente cuando son iguales, y el orden coincide con
    // la posición en la escalera.
    for (i, a) in todos.iter().enumerate() {
        for (j, b) in todos.iter().enumerate() {
            assert_eq!(a == b, i == j);
            assert_eq!(a.cmp(b), i.cmp(&j), "{a} frente a {b}");
            assert_eq!(a.partial_cmp(b), Some(i.cmp(&j)));
        }
    }
    let mut desordenados: Vec<Zoom> = todos.iter().rev().copied().collect();
    desordenados.sort();
    assert_eq!(desordenados, todos);

    // Hash: iguales (aunque se construyan por caminos distintos) dan el mismo hash, y los 70 son
    // distintos en un conjunto.
    let distintos: HashSet<Zoom> = todos.iter().copied().collect();
    assert_eq!(distintos.len(), 70);
    let pares = [
        (fraccionario(2), todos[5]),
        (Zoom::ONE, zoom_entero(1)),
        (Zoom::ONE, fraccionario(2).zoom_in()),
        (zoom_entero(2), Zoom::ONE.zoom_in()),
        (Zoom::MIN, Zoom::MIN.zoom_out()),
        (Zoom::MAX, zoom_entero(64)),
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
fn fila_a2_a_1_2_los_ejemplos_de_la_spec() {
    let z = fraccionario(2);
    assert_eq!(z.scale(5), 2);
    assert_eq!(z.unscale(3), 6);
    assert_eq!(z.extent(5), 3);
    assert_eq!(z.extent(4), 2);
    assert_eq!(z.scale(-1), -1, "floor de -0,5");
    assert_eq!(z.extent(-1), 0, "ceil de -0,5");
}

#[test]
fn fila_a2_a_1_4_los_ejemplos_de_la_spec() {
    let z = fraccionario(4);
    assert_eq!(z.scale(-1), -1);
    assert_eq!(z.unscale(-1), -4);
    assert_eq!(z.extent(7), 2);
    assert_eq!(z.extent(8), 2);
    assert_eq!(z.extent(9), 3);
    assert_eq!(z.extent(0), 0);
}

#[test]
fn fila_a2_a_1_64_los_ejemplos_de_la_spec() {
    let z = fraccionario(64);
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
fn fila_a2_rescale_entre_niveles() {
    // `rescale(n, a)` = floor(n · a / self): lleva una longitud de pantalla de este zoom al de `a`.
    assert_eq!(fraccionario(2).rescale(3, Zoom::ONE), 6);
    assert_eq!(Zoom::ONE.rescale(5, fraccionario(2)), 2);
    assert_eq!(fraccionario(4).rescale(10, fraccionario(64)), 0);
    assert_eq!(fraccionario(64).rescale(10, fraccionario(4)), 160);
    assert_eq!(
        fraccionario(2).rescale(-3, fraccionario(4)),
        -2,
        "floor de -1,5"
    );
    assert_eq!(fraccionario(4).rescale(-3, fraccionario(2)), -6);
    assert_eq!(
        zoom_entero(3).rescale(7, fraccionario(2)),
        1,
        "floor(7 · 1/2 / 3)"
    );
    assert_eq!(fraccionario(2).rescale(100, zoom_entero(3)), 600);
    assert_eq!(Zoom::ONE.rescale(0, Zoom::MAX), 0);
}

#[test]
fn fila_a2_con_zoom_x1_o_mas_extent_es_scale_y_lo_de_hoy_no_cambia() {
    for n in 1..=64u32 {
        let z = zoom_entero(n);
        for v in [-1000i64, -7, -1, 0, 1, 2, 63, 64, 1000, 8192, 123_456] {
            assert_eq!(z.extent(v), z.scale(v), "{z} extent({v})");
            assert_eq!(z.scale(v), v * i64::from(n), "{z} scale({v})");
            assert_eq!(z.unscale(v), v.div_euclid(i64::from(n)), "{z} unscale({v})");
        }
    }
    assert_eq!(Zoom::ONE.scale(5), 5);
    assert_eq!(zoom_entero(3).unscale(7), 2);
    assert_eq!(zoom_entero(3).unscale(-1), -1);
    assert_eq!(Zoom::ONE.rescale(10, zoom_entero(4)), 40);
}

#[test]
fn fila_a2_con_i64_extremos_no_desborda_y_satura() {
    for z in niveles() {
        let f = fraccion_de(z);
        for n in [i64::MIN, i64::MIN + 1, -1, 0, 1, i64::MAX - 1, i64::MAX] {
            assert_eq!(z.scale(n), o_scale(f, n), "{z} scale({n})");
            assert_eq!(z.unscale(n), o_unscale(f, n), "{z} unscale({n})");
            assert_eq!(z.extent(n), o_extent(f, n), "{z} extent({n})");
            for a in [Zoom::MIN, Zoom::ONE, Zoom::MAX] {
                assert_eq!(
                    z.rescale(n, a),
                    o_rescale(f, n, fraccion_de(a)),
                    "{z} rescale({n}, {a})"
                );
            }
        }
    }
    assert_eq!(fraccionario(2).extent(i64::MAX), 1 << 62);
    assert_eq!(fraccionario(2).extent(i64::MIN), i64::MIN / 2);
    assert_eq!(Zoom::MAX.extent(i64::MAX), i64::MAX);
    assert_eq!(Zoom::MAX.extent(i64::MIN), i64::MIN);
    assert_eq!(fraccionario(64).unscale(i64::MAX), i64::MAX);
    assert_eq!(fraccionario(64).unscale(i64::MIN), i64::MIN);
}

// ---------------------------------------------------------------------------------------------
// A3 · Pantalla ↔ imagen a 1/4
// ---------------------------------------------------------------------------------------------

const IMAGEN_10: Size = Size {
    width: 10,
    height: 10,
};
const LIENZO_100: Size = Size {
    width: 100,
    height: 100,
};

fn vista_a_1_4() -> View {
    vista_en(fraccionario(4), (10, 20), LIENZO_100, IMAGEN_10)
}

#[test]
fn fila_a3_screen_to_image_a_1_4_con_origen_10_20() {
    let v = vista_a_1_4();
    let casos: [(Punto, Option<(u32, u32)>); 7] = [
        ((10, 20), Some((0, 0))),
        ((11, 20), Some((4, 0))),
        ((12, 21), Some((8, 4))),
        // El 13 queda fuera: unscale(3) = 12 ≥ 10.
        ((13, 20), None),
        ((9, 20), None),
        ((12, 20), Some((8, 0))),
        ((12, 22), Some((8, 8))),
    ];
    for (p, esperado) in casos {
        assert_eq!(v.screen_to_image(p, IMAGEN_10), esperado, "{p:?}");
    }
    // La versión con signo no recorta: el 9 es -4 y el 13 es 12.
    assert_eq!(v.screen_to_image_i32((9, 20), IMAGEN_10), (-4, 0));
    assert_eq!(v.screen_to_image_i32((13, 20), IMAGEN_10), (12, 0));
    assert_eq!(v.screen_to_image_i32((12, 23), IMAGEN_10), (8, 12));
}

#[test]
fn fila_a3_image_rect_mide_3_por_3_porque_el_pixel_8_se_ve() {
    let v = vista_a_1_4();
    let r = v.image_rect(IMAGEN_10);
    assert_eq!((r.x, r.y), (10, 20));
    assert_eq!((r.width, r.height), (3, 3), "⌈10/4⌉ y no ⌊10/4⌋");
    // Los píxeles de pantalla con imagen son justo esos 3×3.
    for sy in 18..26 {
        for sx in 8..16 {
            let dentro = (10..13).contains(&sx) && (20..23).contains(&sy);
            assert_eq!(
                v.screen_to_image((sx, sy), IMAGEN_10).is_some(),
                dentro,
                "({sx}, {sy})"
            );
        }
    }
}

#[test]
fn fila_a3_image_to_screen_a_1_4() {
    let v = vista_a_1_4();
    let casos = [
        ((0, 0), (10, 20)),
        ((3, 3), (10, 20)),
        ((4, 0), (11, 20)),
        ((8, 0), (12, 20)),
        ((9, 9), (12, 22)),
    ];
    for ((x, y), esperado) in casos {
        assert_eq!(v.image_to_screen(x, y), esperado, "({x}, {y})");
    }
}

#[test]
fn fila_a3_ida_y_vuelta_a_1_4() {
    let v = vista_a_1_4();
    // pantalla → imagen → pantalla es la identidad dentro de la imagen.
    for sy in 20..23 {
        for sx in 10..13 {
            let (x, y) = v
                .screen_to_image((sx, sy), IMAGEN_10)
                .unwrap_or_else(|| panic!("({sx}, {sy}) está en la imagen"));
            assert_eq!(v.image_to_screen(x, y), (sx, sy));
        }
    }
    // imagen → pantalla → imagen sube a la rejilla: x − x mod 4.
    for y in 0..10u32 {
        for x in 0..10u32 {
            let (sx, sy) = v.image_to_screen(x, y);
            assert_eq!(
                v.screen_to_image((sx, sy), IMAGEN_10),
                Some((x - x % 4, y - y % 4)),
                "({x}, {y})"
            );
        }
    }
    // scale(unscale(s)) = s para todo s (también negativo) y unscale(scale(x)) = x − x mod 4.
    let z = fraccionario(4);
    for s in -50i64..=50 {
        assert_eq!(z.scale(z.unscale(s)), s, "s = {s}");
    }
    for x in -50i64..=50 {
        assert_eq!(z.unscale(z.scale(x)), x - x.rem_euclid(4), "x = {x}");
    }
}

#[test]
fn fila_a3_pixel_block_a_1_4_solo_existe_en_la_rejilla_y_mide_1_por_1() {
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
fn fila_a3_pixel_block_a_zoom_entero_es_zoom_por_zoom_y_a_1_2_y_1_64_sigue_la_rejilla() {
    let v = vista_en(zoom_entero(3), (10, 20), LIENZO_100, IMAGEN_10);
    assert_eq!(v.pixel_block(2, 1), Some(((16, 23), (19, 26))));
    assert_eq!(v.pixel_block(0, 0), Some(((10, 20), (13, 23))));
    let v = vista_en(Zoom::ONE, (10, 20), LIENZO_100, IMAGEN_10);
    assert_eq!(v.pixel_block(5, 5), Some(((15, 25), (16, 26))));
    let v = vista_en(fraccionario(2), (10, 20), LIENZO_100, IMAGEN_10);
    assert_eq!(v.pixel_block(6, 2), Some(((13, 21), (14, 22))));
    assert_eq!(v.pixel_block(6, 3), None);
    assert_eq!(v.pixel_block(5, 2), None);
    let mut v = View::fit(LIENZO_100, Size::new(256, 256));
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
fn comprueba_fit(viewport: Size, image: Size, esperado: &str) {
    let v = View::fit(viewport, image);
    let etiqueta = format!(
        "{}×{} en {}×{}",
        image.width, image.height, viewport.width, viewport.height
    );
    assert_eq!(v.zoom().to_string(), esperado, "{etiqueta}");
    let f = fraccion_de(v.zoom());
    assert_eq!(
        v.zoom(),
        niveles()[o_mejor_nivel(viewport, image)],
        "{etiqueta}: el mayor nivel que cabe"
    );
    let (ext_x, ext_y) = (
        o_extent(f, i64::from(image.width)),
        o_extent(f, i64::from(image.height)),
    );
    assert!(
        (2 * v.origin().0 + ext_x - i64::from(viewport.width)).abs() <= 1,
        "{etiqueta}: centrada en x (origen {:?}, extent {ext_x})",
        v.origin()
    );
    assert!(
        (2 * v.origin().1 + ext_y - i64::from(viewport.height)).abs() <= 1,
        "{etiqueta}: centrada en y (origen {:?}, extent {ext_y})",
        v.origin()
    );
}

#[test]
fn fila_a4_las_cinco_imagenes_de_la_tabla_en_los_tres_lienzos() {
    let lienzos = [
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
    for (lado, esperados) in tabla {
        for (lienzo, esperado) in lienzos.iter().zip(esperados) {
            comprueba_fit(*lienzo, Size::new(lado, lado), esperado);
        }
    }
}

#[test]
fn fila_a4_ejemplos_concretos_de_origen_y_ajuste_exacto() {
    // 4096² en 128×297: 1/32 exacto (128), centrada en alto: (297 − 128) / 2 = 84,5.
    let v = View::fit(Size::new(128, 297), Size::new(4096, 4096));
    assert_eq!(v.zoom(), fraccionario(32));
    assert_eq!(v.image_rect(Size::new(4096, 4096)).width, 128);
    assert_eq!(v.origin().0, 0);
    assert!(v.origin().1 == 84 || v.origin().1 == 85);
    // 64×48 en 640×480: ×10 como hoy, sin márgenes.
    let v = View::fit(Size::new(640, 480), Size::new(64, 48));
    assert_eq!(v.zoom(), zoom_entero(10));
    assert_eq!(v.origin(), (0, 0));
    // 6000² en 1600×900: 1/8 (750).
    let v = View::fit(Size::new(1600, 900), Size::new(6000, 6000));
    assert_eq!(v.zoom(), fraccionario(8));
    assert_eq!(v.image_rect(Size::new(6000, 6000)).width, 750);
}

#[test]
fn fila_a4_16384_en_100x100_da_1_64_y_origen_menos_78() {
    let imagen = Size::new(16384, 16384);
    let v = View::fit(Size::new(100, 100), imagen);
    assert_eq!(v.zoom(), Zoom::MIN);
    assert_eq!(v.origin(), (-78, -78), "256 de imagen centrados en 100");
    assert_eq!(v.image_rect(imagen).width, 256);
}

#[test]
fn fila_a4_3x3_en_1x1_da_1_4() {
    let v = View::fit(Size::new(1, 1), Size::new(3, 3));
    assert_eq!(v.zoom(), fraccionario(4));
    assert_eq!(v.image_rect(Size::new(3, 3)).width, 1);
    assert_eq!(v.origin(), (0, 0));
}

#[test]
fn fila_a4_viewport_0x0_da_el_minimo() {
    for imagen in [Size::new(5, 5), Size::new(64, 48), Size::new(8192, 8192)] {
        let v = View::fit(Size::new(0, 0), imagen);
        assert_eq!(v.zoom(), Zoom::MIN, "{imagen:?}");
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
fn fila_a4_los_zooms_enteros_siguen_como_hoy_cuando_caben() {
    // Imágenes pequeñas: el mayor entero que cabe (hasta ×64), centrado.
    comprueba_fit(Size::new(700, 500), Size::new(64, 48), "×10");
    comprueba_fit(Size::new(640, 480), Size::new(64, 48), "×10");
    comprueba_fit(Size::new(1000, 1000), Size::new(1, 1), "×64");
    comprueba_fit(Size::new(100, 100), Size::new(100, 100), "×1");
    comprueba_fit(Size::new(99, 100), Size::new(100, 100), "×1/2");
    comprueba_fit(Size::new(150, 300), Size::new(10, 10), "×15");
}

// ---------------------------------------------------------------------------------------------
// A5 · Ancla
// ---------------------------------------------------------------------------------------------

const IMAGEN_1000: Size = Size {
    width: 1000,
    height: 1000,
};
const LIENZO_800X600: Size = Size {
    width: 800,
    height: 600,
};

fn desde_x1() -> View {
    vista_en(Zoom::ONE, (0, 0), LIENZO_800X600, IMAGEN_1000)
}

#[test]
fn fila_a5_ancla_exacta_a_1_2_en_100_50() {
    let mut v = desde_x1();
    v.set_zoom(fraccionario(2), (100, 50));
    assert_eq!(v.origin(), (50, 25));
    assert_eq!(v.screen_to_image((100, 50), IMAGEN_1000), Some((100, 50)));
}

#[test]
fn fila_a5_ancla_a_1_2_en_101_51_sube_a_la_rejilla() {
    let mut v = desde_x1();
    v.set_zoom(fraccionario(2), (101, 51));
    assert_eq!(v.origin(), (51, 26));
    // El 101 sube a la rejilla de 2: el píxel bajo el ancla es el 100.
    assert_eq!(v.screen_to_image((101, 51), IMAGEN_1000), Some((100, 50)));
}

#[test]
fn fila_a5_ancla_a_1_64_en_100_50() {
    let mut v = desde_x1();
    v.set_zoom(Zoom::MIN, (100, 50));
    assert_eq!(v.origin(), (99, 50));
    // ⌊100/64⌋·64 = 64 y ⌊50/64⌋·64 = 0.
    assert_eq!(v.screen_to_image((100, 50), IMAGEN_1000), Some((64, 0)));
}

#[test]
fn fila_a5_ida_y_vuelta_no_es_la_identidad_pero_el_pixel_es_el_de_la_rejilla() {
    let mut v = desde_x1();
    v.set_zoom(fraccionario(2), (101, 51));
    assert_eq!(v.zoom(), fraccionario(2));
    v.zoom_in((101, 51));
    assert_eq!(v.zoom(), Zoom::ONE);
    assert_eq!(v.origin(), (1, 1));
    assert_eq!(v.screen_to_image((101, 51), IMAGEN_1000), Some((100, 50)));
    assert_ne!(v, desde_x1(), "alejar y acercar no es la identidad");
}

#[test]
fn fila_a5_acercar_a_un_entero_conserva_el_pixel_exacto() {
    let mut v = desde_x1();
    v.set_zoom(Zoom::MIN, (100, 50));
    let pixel = v.screen_to_image_i32((100, 50), IMAGEN_1000);
    assert_eq!(pixel, (64, 0));
    for _ in 0..7 {
        v.zoom_in((100, 50));
        assert_eq!(
            v.screen_to_image_i32((100, 50), IMAGEN_1000),
            pixel,
            "{}",
            v.zoom()
        );
    }
    assert_eq!(v.zoom(), zoom_entero(2));
}

// ---------------------------------------------------------------------------------------------
// Estrategias
// ---------------------------------------------------------------------------------------------

/// Un `i64` cualquiera, con peso en los extremos y en los valores pequeños.
fn extremo() -> impl Strategy<Value = i64> {
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
const SIN_SATURAR: i64 = 1 << 56;

proptest! {
    #![proptest_config(ProptestConfig::with_cases(256))]

    #[test]
    fn propiedad_p1_la_conversion_coincide_con_el_oraculo_en_los_70_niveles(
        previo in 0usize..70,
        ancla_previa in (extremo(), extremo()),
        ancla in (extremo(), extremo()),
        punto in (extremo(), extremo()),
        n in extremo(),
        imagen in (dimension(), dimension()),
        imagen_pixel in (any::<u32>(), any::<u32>()),
    ) {
        let todos = niveles();
        let fracciones = escalera();
        let tam = Size::new(imagen.0, imagen.1);
        // Un origen cualquiera: el que dejan dos `set_zoom` con anclas arbitrarias.
        let mut base = View::fit(Size::new(640, 480), tam);
        base.set_zoom(todos[previo], ancla_previa);
        for (i, (&z, &f)) in todos.iter().zip(&fracciones).enumerate() {
            prop_assert_eq!(z.to_string(), rotulo(f));
            // scale, unscale, extent.
            for v in [n, punto.0, punto.1] {
                prop_assert_eq!(z.scale(v), o_scale(f, v), "{} scale({})", z, v);
                prop_assert_eq!(z.unscale(v), o_unscale(f, v), "{} unscale({})", z, v);
                prop_assert_eq!(z.extent(v), o_extent(f, v), "{} extent({})", z, v);
                prop_assert_eq!(
                    todos[previo].rescale(v, z),
                    o_rescale(fracciones[previo], v, f),
                    "{} rescale({}, {})", todos[previo], v, z
                );
            }
            // Retracción.
            let s = n.clamp(-SIN_SATURAR, SIN_SATURAR);
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
            v.set_zoom(z, ancla);
            let origen = v.origin();
            let esperado = (
                o_eje(f, punto.0, origen.0, tam.width),
                o_eje(f, punto.1, origen.1, tam.height),
            );
            let esperado = match esperado {
                (Some(x), Some(y)) => Some((x, y)),
                _ => None,
            };
            prop_assert_eq!(v.screen_to_image(punto, tam), esperado, "{} origen {:?}", z, origen);
            prop_assert_eq!(
                v.screen_to_image_i32(punto, tam),
                (o_eje_i32(f, punto.0, origen.0), o_eje_i32(f, punto.1, origen.1)),
                "{} origen {:?}", z, origen
            );
            // image_to_screen y image_rect.
            let (x, y) = imagen_pixel;
            prop_assert_eq!(
                v.image_to_screen(x, y),
                (
                    satura(i128::from(origen.0) + i128::from(o_scale(f, i64::from(x)))),
                    satura(i128::from(origen.1) + i128::from(o_scale(f, i64::from(y)))),
                ),
                "{} image_to_screen", z
            );
            let r = v.image_rect(tam);
            prop_assert_eq!((r.x, r.y), origen);
            prop_assert_eq!(r.width, o_extent(f, i64::from(tam.width)), "{} ancho", z);
            prop_assert_eq!(r.height, o_extent(f, i64::from(tam.height)), "{} alto", z);
            // pixel_block, con orígenes que no desbordan la suma.
            if origen.0.unsigned_abs() < 1 << 60 && origen.1.unsigned_abs() < 1 << 60 {
                let (sx, sy) = (
                    origen.0 + o_scale(f, i64::from(x)),
                    origen.1 + o_scale(f, i64::from(y)),
                );
                if i < 6 {
                    let k = f.1 as u32;
                    let alineado = (x / k * k, y / k * k);
                    prop_assert_eq!(
                        v.pixel_block(alineado.0, alineado.1).map(|b| (b.1.0 - b.0.0, b.1.1 - b.0.1)),
                        Some((1, 1)),
                        "{} 1×1 en la rejilla", z
                    );
                    prop_assert_eq!(
                        v.pixel_block(alineado.0, alineado.1).map(|b| b.0),
                        Some((
                            origen.0 + o_scale(f, i64::from(alineado.0)),
                            origen.1 + o_scale(f, i64::from(alineado.1)),
                        ))
                    );
                    if x % k != 0 || y % k != 0 {
                        prop_assert_eq!(v.pixel_block(x, y), None, "{} fuera de la rejilla", z);
                    }
                } else {
                    let lado = f.0 as i64;
                    prop_assert_eq!(
                        v.pixel_block(x, y),
                        Some(((sx, sy), (sx + lado, sy + lado))),
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
fn rejilla(indice: usize) -> i32 {
    if indice < 6 { 1 << (6 - indice) } else { 1 }
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(256))]

    #[test]
    fn propiedad_p2_el_pixel_bajo_el_ancla_sube_a_la_rejilla_del_nivel_nuevo(
        desde in 0usize..70,
        hasta in 0usize..70,
        ancla_previa in (-3000i64..=3000, -3000i64..=3000),
        ancla in (-3000i64..=3000, -3000i64..=3000),
        imagen in (1u32..=5000, 1u32..=5000),
    ) {
        let todos = niveles();
        let tam = Size::new(imagen.0, imagen.1);
        let mut v = View::fit(Size::new(800, 600), tam);
        v.set_zoom(todos[desde], ancla_previa);
        prop_assert_eq!(v.zoom(), todos[desde]);
        let antes = v.screen_to_image_i32(ancla, tam);
        v.set_zoom(todos[hasta], ancla);
        prop_assert_eq!(v.zoom(), todos[hasta]);
        let despues = v.screen_to_image_i32(ancla, tam);
        let g = rejilla(hasta);
        prop_assert_eq!(
            despues,
            (antes.0.div_euclid(g) * g, antes.1.div_euclid(g) * g),
            "{} → {}: antes {:?}", todos[desde], todos[hasta], antes
        );
        if hasta >= 6 {
            prop_assert_eq!(despues, antes, "con un nivel ≥ ×1 el píxel es exacto");
        }
        // Un paso de zoom_in / zoom_out con la misma regla.
        let mut w = View::fit(Size::new(800, 600), tam);
        w.set_zoom(todos[desde], ancla_previa);
        let pixel = w.screen_to_image_i32(ancla, tam);
        w.zoom_in(ancla);
        let g = rejilla((desde + 1).min(69));
        prop_assert_eq!(
            w.screen_to_image_i32(ancla, tam),
            (pixel.0.div_euclid(g) * g, pixel.1.div_euclid(g) * g),
            "zoom_in desde {}", todos[desde]
        );
        let mut w = View::fit(Size::new(800, 600), tam);
        w.set_zoom(todos[desde], ancla_previa);
        w.zoom_out(ancla);
        let g = rejilla(desde.saturating_sub(1));
        prop_assert_eq!(
            w.screen_to_image_i32(ancla, tam),
            (pixel.0.div_euclid(g) * g, pixel.1.div_euclid(g) * g),
            "zoom_out desde {}", todos[desde]
        );
    }

    #[test]
    fn propiedad_p2_con_anclas_extremas_el_origen_no_desborda(
        desde in 0usize..70,
        hasta in 0usize..70,
        ancla_previa in (extremo(), extremo()),
        ancla in (extremo(), extremo()),
    ) {
        // En una compilación de depuración un desbordamiento aritmético sería un pánico.
        let todos = niveles();
        let tam = Size::new(1000, 1000);
        let mut v = View::fit(Size::new(800, 600), tam);
        v.set_zoom(todos[desde], ancla_previa);
        v.set_zoom(todos[hasta], ancla);
        prop_assert_eq!(v.zoom(), todos[hasta]);
        v.zoom_in(ancla);
        v.zoom_out(ancla);
        let _ = v.origin();
        let _ = v.screen_to_image_i32(ancla, tam);
        let _ = v.image_rect(tam);
    }
}

// ---------------------------------------------------------------------------------------------
// P3 · Ajustar
// ---------------------------------------------------------------------------------------------

proptest! {
    #![proptest_config(ProptestConfig::with_cases(256))]

    #[test]
    fn propiedad_p3_fit_es_el_mayor_nivel_que_cabe_y_centra(
        lienzo in (0u32..=3000, 0u32..=3000),
        imagen in (dimension(), dimension()),
    ) {
        let viewport = Size::new(lienzo.0, lienzo.1);
        let tam = Size::new(imagen.0, imagen.1);
        let v = View::fit(viewport, tam);
        let mejor = o_mejor_nivel(viewport, tam);
        let todos = niveles();
        prop_assert_eq!(v.zoom(), todos[mejor], "esperado {}", rotulo(escalera()[mejor]));
        let f = escalera()[mejor];
        let (ext_x, ext_y) = (o_extent(f, i64::from(tam.width)), o_extent(f, i64::from(tam.height)));
        let (cabe_x, cabe_y) = (ext_x <= i64::from(lienzo.0), ext_y <= i64::from(lienzo.1));
        if mejor > 0 {
            prop_assert!(cabe_x && cabe_y, "el nivel elegido cabe");
        }
        // Es el mayor: el siguiente no cabe (salvo que ya sea ×64).
        if mejor < 69 {
            let g = escalera()[mejor + 1];
            prop_assert!(
                o_extent(g, i64::from(tam.width)) > i64::from(lienzo.0)
                    || o_extent(g, i64::from(tam.height)) > i64::from(lienzo.1),
                "el nivel {} también cabe", rotulo(g)
            );
        }
        // Centrada con diferencia ≤ 1.
        prop_assert!((2 * v.origin().0 + ext_x - i64::from(lienzo.0)).abs() <= 1);
        prop_assert!((2 * v.origin().1 + ext_y - i64::from(lienzo.1)).abs() <= 1);
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

fn ancla() -> impl Strategy<Value = (i64, i64)> {
    (-3000i64..=3000, -3000i64..=3000)
}

fn op() -> impl Strategy<Value = Op> {
    prop_oneof![
        3 => (0usize..70, ancla()).prop_map(|(i, a)| Op::SetZoom(i, a)),
        2 => ancla().prop_map(Op::ZoomIn),
        3 => ancla().prop_map(Op::ZoomOut),
        3 => (-3000i64..=3000, -3000i64..=3000).prop_map(|(x, y)| Op::Pan(x, y)),
        1 => Just(Op::Clamp),
        1 => Just(Op::Fit),
    ]
}

fn aplica(v: &mut View, op: &Op, viewport: Size, image: Size) {
    let todos = niveles();
    match op {
        Op::SetZoom(i, a) => v.set_zoom(todos[*i], *a),
        Op::ZoomIn(a) => v.zoom_in(*a),
        Op::ZoomOut(a) => v.zoom_out(*a),
        Op::Pan(x, y) => v.pan(*x, *y, viewport, image),
        Op::Clamp => v.clamp(viewport, image),
        Op::Fit => *v = View::fit(viewport, image),
    }
}

/// Posiciones de un eje que merece la pena mirar alrededor de `[origen, origen + extent)`: todas si
/// el tramo es corto y los bordes y unas muestras si es largo.
fn posiciones(origen: i64, extent: i64) -> Vec<i64> {
    let mut v: Vec<i64> = Vec::new();
    if extent <= 3000 {
        v.extend(origen - 3..origen + extent + 3);
    } else {
        v.extend(origen - 3..origen + 40);
        v.extend(origen + extent - 40..origen + extent + 3);
        v.extend((1..=50).map(|i| origen + extent * i / 51));
    }
    v
}

/// Los píxeles de pantalla con `screen_to_image = Some` son exactamente `[origen, origen + extent)`
/// en cada eje.
fn comprueba_cobertura(v: &View, image: Size) -> Result<(), TestCaseError> {
    let f = fraccion_de(v.zoom());
    let r = v.image_rect(image);
    let (ext_x, ext_y) = (
        o_extent(f, i64::from(image.width)),
        o_extent(f, i64::from(image.height)),
    );
    prop_assert_eq!((r.width, r.height), (ext_x, ext_y), "image_rect usa extent");
    let (ox, oy) = v.origin();
    for x in posiciones(ox, ext_x) {
        let dentro = ox <= x && x < ox + ext_x;
        prop_assert_eq!(
            v.screen_to_image((x, oy), image).is_some(),
            dentro,
            "x = {} (origen {}, extent {}, {})",
            x,
            ox,
            ext_x,
            v.zoom()
        );
    }
    for y in posiciones(oy, ext_y) {
        let dentro = oy <= y && y < oy + ext_y;
        prop_assert_eq!(
            v.screen_to_image((ox, y), image).is_some(),
            dentro,
            "y = {} (origen {}, extent {}, {})",
            y,
            oy,
            ext_y,
            v.zoom()
        );
    }
    Ok(())
}

/// Píxeles de la imagen que caen dentro de `[0, lienzo)` en un eje.
fn visibles(origen: i64, extent: i64, lienzo: u32) -> i64 {
    (origen + extent).min(i64::from(lienzo)) - origen.max(0)
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(256))]

    #[test]
    fn propiedad_p4_cobertura_sin_huecos_ni_sobrantes_y_clamp_deja_visible_lo_minimo(
        imagen in (1u32..=5000, 1u32..=5000),
        lienzo in (1u32..=2000, 1u32..=2000),
        ops in proptest::collection::vec(op(), 0..12),
    ) {
        let tam = Size::new(imagen.0, imagen.1);
        let viewport = Size::new(lienzo.0, lienzo.1);
        let mut v = View::fit(viewport, tam);
        for o in &ops {
            aplica(&mut v, o, viewport, tam);
        }
        comprueba_cobertura(&v, tam)?;

        // clamp: visibles min(16, extent, lienzo) por eje, y es idempotente.
        let f = fraccion_de(v.zoom());
        let mut c = v;
        c.clamp(viewport, tam);
        comprueba_cobertura(&c, tam)?;
        let (ext_x, ext_y) = (
            o_extent(f, i64::from(tam.width)),
            o_extent(f, i64::from(tam.height)),
        );
        let minimo_x = i64::from(MIN_VISIBLE).min(ext_x).min(i64::from(viewport.width));
        let minimo_y = i64::from(MIN_VISIBLE).min(ext_y).min(i64::from(viewport.height));
        prop_assert_eq!(MIN_VISIBLE, 16);
        prop_assert!(
            visibles(c.origin().0, ext_x, viewport.width) >= minimo_x,
            "x: {} visibles y debían ser ≥ {} (origen {:?}, extent {}, lienzo {})",
            visibles(c.origin().0, ext_x, viewport.width), minimo_x, c.origin(), ext_x, viewport.width
        );
        prop_assert!(
            visibles(c.origin().1, ext_y, viewport.height) >= minimo_y,
            "y: {} visibles y debían ser ≥ {} (origen {:?}, extent {}, lienzo {})",
            visibles(c.origin().1, ext_y, viewport.height), minimo_y, c.origin(), ext_y, viewport.height
        );
        let mut otra = c;
        otra.clamp(viewport, tam);
        prop_assert_eq!(otra, c, "clamp es idempotente");
    }
}
