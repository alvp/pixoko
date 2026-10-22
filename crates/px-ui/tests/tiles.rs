//! Baldosas, región sucia y baldosa desde las filas (specs/hito-2-zoom-baldosas.md, unidad 14a-2:
//! filas B1, B2 y B4 y propiedades Q1 y Q3; contrato de API en «Contrato de API de la 14a-2»).
//! Todo es lógica pura de `px-ui`, sin egui más que `Color32` y `ColorImage`.
//!
//! - B1: `tile_side`, `Tiling` (`rect`, `screen_rect`, `uv`, `visible`): conteo y recorte en el
//!   borde, cobertura exacta sin solapes ni huecos en imagen y en pantalla a varios zooms (también
//!   1/K), uv de 0 a 1 con zoom ≥ ×1 y, a 1/K, el centro de cada píxel de pantalla muestrea el
//!   texel `s·K − a` de su baldosa.
//! - B2: `BlockSet` frente a un modelo de conjunto de coordenadas escrito aquí.
//! - B4: `tile_pixels` frente a `ColorImage::from_rgba_unmultiplied(to_rgba8(image_2))` recortado.
//! - Q1 (geometría de baldosas) y Q3 (región: `dirty_of` y el registro por `state_id` contienen todo
//!   píxel que cambia, con `undo` y `redo`).
//!
//! Supuestos que la spec no fija del todo:
//! - `cols()`, `rows()`, `len()`, `side()`, `BlockSet::len()` y los índices de baldosa son `usize`
//!   (como `tile_side`, `Tiling::new(.., side: usize)` y `ChangeLog::LIMIT_BYTES`); `contains(bx, by)`
//!   recibe `u32` (coordenadas de bloque, como `add_pixel`).
//! - Un rectángulo de `tile_pixels` que se sale de la imagen devuelve los píxeles del recorte
//!   (y vacío si no corta la imagen), como dice el contrato («se recorta»).
//! - El uv de B1 se comprueba por **comportamiento** (el texel que muestrea el centro de cada
//!   píxel de pantalla), no por la fórmula escrita: el contrato la da con una errata (el
//!   desplazamiento `0,5 − K/2` va sobre la posición medida desde el borde del cuadrilátero).
//! - Las pruebas de uv usan lados de baldosa múltiplos de 64 (lo único que construye el lienzo);
//!   `screen_rect` y `visible` también con lados pequeños (16, 64) y a 1/64 (cuadriláteros de ancho
//!   cero que no se ven).

use px_core::ImageId;
use std::collections::HashSet;

use egui::{Color32, ColorImage};
use proptest::prelude::*;
use px_core::{Command, Document, Image, Mode, Pixel, PixelEdit, Rgba, Span};
use px_ui::canvas::CanvasError;
use px_ui::dirty::{BlockSet, ChangeLog, dirty_of};
use px_ui::texture::{
    tile_pixels, tile_pixels_with_entry, to_rgba8, to_rgba8_with_entry, try_tile_pixels,
};
use px_ui::tiles::{BLOCK, TILE_SIDE, TileAlloc, TileRect, Tiling, tile_side};
use px_ui::view::{ScreenRect, Size, View, Zoom};

mod tiles_support;
use tiles_support::{
    B, Model, Step, TEST_MODES, clip_2, expected_2, image_of, model_full, model_of_rect,
    runs_expected, seed, step_of,
};

// ---------------------------------------------------------------------------------------------
// Oráculo de zoom: fracciones con i128
// ---------------------------------------------------------------------------------------------

/// Un nivel de la escalera y su fracción `num/den`.
#[derive(Clone, Copy)]
struct Level {
    zoom: Zoom,
    num: i128,
    den: i128,
}

/// Los 70 niveles, de menor a mayor.
fn levels() -> Vec<Level> {
    let mut v: Vec<Level> = [64u32, 32, 16, 8, 4, 2]
        .iter()
        .map(|&k| Level {
            zoom: Zoom::shrink(k).expect("nivel 1/K"),
            num: 1,
            den: i128::from(k),
        })
        .collect();
    v.extend((1..=64u32).map(|n| Level {
        zoom: Zoom::from_int(n).expect("nivel entero"),
        num: i128::from(n),
        den: 1,
    }));
    v
}

fn level(num: u32, den: u32) -> Level {
    levels()
        .into_iter()
        .find(|n| n.num == i128::from(num) && n.den == i128::from(den))
        .expect("nivel de la escalera")
}

/// `⌈n·num/den⌉`: lo que ocupan `n` píxeles de imagen en pantalla.
fn ext(level: Level, n: u32) -> i64 {
    let q = i128::from(n) * level.num;
    (-((-q).div_euclid(level.den))) as i64
}

/// Una vista en el nivel dado y con el origen que sale de anclar en `anchor` (sin `clamp`).
fn vista(viewport: Size, image_2: Size, level: Level, anchor: (i64, i64)) -> View {
    let mut v = View::fit(viewport, image_2);
    v.set_zoom(level.zoom, anchor);
    v
}

/// El rectángulo de pantalla de una baldosa según la fórmula del contrato, con enteros.
fn screen_expected(vista: &View, level: Level, r: TileRect) -> ScreenRect {
    let (ox, oy) = vista.origin();
    ScreenRect {
        x: ox + ext(level, r.x),
        y: oy + ext(level, r.y),
        width: ext(level, r.x + r.width) - ext(level, r.x),
        height: ext(level, r.y + r.height) - ext(level, r.y),
    }
}

fn short_the_canvas(r: ScreenRect, viewport: Size) -> bool {
    let x0 = r.x.max(0);
    let x1 = (r.x + r.width).min(i64::from(viewport.width));
    let y0 = r.y.max(0);
    let y1 = (r.y + r.height).min(i64::from(viewport.height));
    x1 > x0 && y1 > y0
}

fn rect_de(t: &Tiling, i: usize) -> TileRect {
    t.rect(i)
}

fn screen_of(t: &Tiling, i: usize, v: &View) -> ScreenRect {
    t.screen_rect(i, v)
}

fn number(t: &Tiling) -> usize {
    t.len()
}

fn columns_2(t: &Tiling) -> usize {
    t.cols()
}

fn rows_2(t: &Tiling) -> usize {
    t.rows()
}

// ---------------------------------------------------------------------------------------------
// B1 · Geometría de baldosas
// ---------------------------------------------------------------------------------------------

/// Imágenes, lado de baldosa y número de baldosas de la tabla de B1.
const CASES_B_1: [((u32, u32), usize, usize); 7] = [
    ((1, 1), 1024, 1),
    ((2048, 2048), 1024, 4),
    ((2049, 1), 1024, 3),
    ((1025, 2), 1024, 2),
    ((8192, 8192), 2048, 16),
    ((6000, 6000), 2048, 9),
    ((16384, 4096), 2048, 16),
];

#[test]
fn row_b1_the_constants_are_those_of_the_spec() {
    assert_eq!(TILE_SIDE, 2048);
    assert_eq!(BLOCK, 64);
}

#[test]
fn row_b1_tile_side_gives_the_largest_bounded_power_of_two_with_minimum_64() {
    let cases: [(usize, usize); 6] = [
        (32768, 2048),
        (4096, 2048),
        (2048, 2048),
        (1024, 1024),
        (1000, 512),
        (64, 64),
    ];
    for (max, expected_2) in cases {
        assert_eq!(tile_side(max), expected_2, "tile_side({max})");
    }
    // Alrededor de las potencias de dos, por debajo del mínimo y en los extremos.
    for (max, expected_2) in [
        (2047usize, 1024usize),
        (1025, 1024),
        (1023, 512),
        (129, 128),
        (128, 128),
        (127, 64),
        (65, 64),
        (63, 64),
        (1, 64),
        (0, 64),
        (usize::MAX, 2048),
    ] {
        assert_eq!(tile_side(max), expected_2, "tile_side({max})");
    }
    // Siempre potencia de dos, múltiplo de 64 y ≤ min(2048, max) (salvo el mínimo).
    for max in 1..=4200usize {
        let side = tile_side(max);
        assert!(side.is_power_of_two(), "{max} → {side}");
        assert_eq!(side % 64, 0, "{max} → {side}");
        assert!(side <= 2048, "{max} → {side}");
        assert!(side <= max.max(64), "{max} → {side}");
        // Y es la mayor: el doble ya no cabe (o pasa de 2048).
        assert!(side * 2 > max.min(2048) || side == 64, "{max} → {side}");
    }
}

#[test]
fn row_b1_number_of_tiles_and_clip_at_the_edge() {
    for ((w, h), side, how_many) in CASES_B_1 {
        let t = Tiling::new(Size::new(w, h), side);
        let label_2 = format!("{w}×{h} con T = {side}");
        assert_eq!(t.side(), side, "{label_2}");
        assert_eq!(number(&t), how_many, "{label_2}");
        let (cols, rows) = ((w as usize).div_ceil(side), (h as usize).div_ceil(side));
        assert_eq!(columns_2(&t), cols, "{label_2}");
        assert_eq!(rows_2(&t), rows, "{label_2}");
        assert_eq!(cols * rows, how_many, "{label_2}");
        // Orden por filas: `índice = row_2 · cols + col`; las del borde, recortadas.
        for i in 0..how_many {
            let (col, row) = (i % cols, i / cols);
            let x = (col * side) as u32;
            let y = (row * side) as u32;
            let expected_2 = TileRect {
                x,
                y,
                width: (w - x).min(side as u32),
                height: (h - y).min(side as u32),
            };
            assert_eq!(rect_de(&t, i), expected_2, "{label_2}: baldosa {i}");
        }
    }
    // Los casos que el texto de la spec nombra.
    let t = Tiling::new(Size::new(1025, 2), 1024);
    assert_eq!(
        rect_de(&t, 0),
        TileRect {
            x: 0,
            y: 0,
            width: 1024,
            height: 2
        }
    );
    assert_eq!(
        rect_de(&t, 1),
        TileRect {
            x: 1024,
            y: 0,
            width: 1,
            height: 2
        }
    );
    let t = Tiling::new(Size::new(6000, 6000), 2048);
    assert_eq!(rect_de(&t, 8).width, 6000 - 2 * 2048);
    assert_eq!(rect_de(&t, 8).height, 6000 - 2 * 2048);
}

#[test]
fn row_b1_the_tiles_cover_the_image_without_overlaps_or_gaps() {
    for ((w, h), side, _) in CASES_B_1 {
        let t = Tiling::new(Size::new(w, h), side);
        let rects: Vec<TileRect> = (0..number(&t)).map(|i| rect_de(&t, i)).collect();
        let area: u64 = rects
            .iter()
            .map(|r| u64::from(r.width) * u64::from(r.height))
            .sum();
        assert_eq!(area, u64::from(w) * u64::from(h), "{w}×{h}: área total");
        for (i, a) in rects.iter().enumerate() {
            assert!(a.width >= 1 && a.height >= 1, "{w}×{h}: baldosa {i} vacía");
            assert!(a.x + a.width <= w && a.y + a.height <= h, "{w}×{h}: {i}");
            for (j, b) in rects.iter().enumerate().skip(i + 1) {
                let overlap = a.x < b.x + b.width
                    && b.x < a.x + a.width
                    && a.y < b.y + b.height
                    && b.y < a.y + a.height;
                assert!(!overlap, "{w}×{h}: las baldosas {i} y {j} se solapan");
            }
        }
    }
}

/// Zooms de B1: ×1, ×5 y 1/64 (los de la tabla) y otros fraccionarios y el máximo.
fn zooms_b1() -> Vec<Level> {
    vec![
        level(1, 1),
        level(5, 1),
        level(1, 64),
        level(1, 2),
        level(1, 4),
        level(1, 8),
        level(1, 32),
        level(3, 1),
        level(64, 1),
    ]
}

#[test]
fn row_b1_screen_rect_covers_exactly_image_rect_without_overlaps_or_gaps() {
    let viewport = Size::new(620, 684);
    for ((w, h), side, _) in CASES_B_1 {
        let image_2 = Size::new(w, h);
        let t = Tiling::new(image_2, side);
        for z in zooms_b1() {
            for anchor in [(0, 0), (310, 342), (-5000, 77), (900, -1234)] {
                let v = vista(viewport, image_2, z, anchor);
                let label_2 = format!("{w}×{h} T={side} ×{}/{} ancla {anchor:?}", z.num, z.den);
                let frame = v.image_rect(image_2);
                let screens: Vec<ScreenRect> =
                    (0..number(&t)).map(|i| screen_of(&t, i, &v)).collect();
                for (i, p) in screens.iter().enumerate() {
                    assert_eq!(
                        *p,
                        screen_expected(&v, z, rect_de(&t, i)),
                        "{label_2}: baldosa {i}"
                    );
                }
                // Por columnas y por filas: contiguas, empiezan en el origen y acaban en el final.
                let (cols, rows) = (columns_2(&t), rows_2(&t));
                assert_eq!(screens[0].x, frame.x, "{label_2}");
                assert_eq!(screens[0].y, frame.y, "{label_2}");
                let last = screens[cols * rows - 1];
                assert_eq!(last.x + last.width, frame.x + frame.width, "{label_2}");
                assert_eq!(last.y + last.height, frame.y + frame.height, "{label_2}");
                for c in 0..cols - 1 {
                    let (a, b) = (screens[c], screens[c + 1]);
                    assert_eq!(
                        a.x + a.width,
                        b.x,
                        "{label_2}: hueco o solape entre columnas {c}"
                    );
                }
                for r in 0..rows - 1 {
                    let (a, b) = (screens[r * cols], screens[(r + 1) * cols]);
                    assert_eq!(
                        a.y + a.height,
                        b.y,
                        "{label_2}: hueco o solape entre filas {r}"
                    );
                }
                // Cada baldosa comparte x y ancho con su columna, y y alto con su fila.
                for (i, p) in screens.iter().enumerate() {
                    let (col, row) = (i % cols, i / cols);
                    assert_eq!((p.x, p.width), (screens[col].x, screens[col].width));
                    assert_eq!(
                        (p.y, p.height),
                        (screens[row * cols].y, screens[row * cols].height)
                    );
                }
                // El área sumada es la de `image_rect`.
                let area: i128 = screens
                    .iter()
                    .map(|p| i128::from(p.width) * i128::from(p.height))
                    .sum();
                assert_eq!(
                    area,
                    i128::from(frame.width) * i128::from(frame.height),
                    "{label_2}"
                );
            }
        }
    }
}

#[test]
fn row_b1_at_1_k_the_quad_goes_from_a_k_to_the_ceiling_of_b_k_from_the_origin() {
    // A 1/K el lado (múltiplo de 64, y de K) da `[o + a/K, o + ⌈b/K⌉)`: la fórmula escrita sin
    // `extent`, para la baldosa del borde con recorte incluido.
    let image_2 = Size::new(6000, 6000);
    let t = Tiling::new(image_2, 2048);
    for k in [2u32, 4, 8, 16, 32, 64] {
        let z = level(1, k);
        let v = vista(Size::new(800, 600), image_2, z, (400, 300));
        let (ox, oy) = v.origin();
        for i in 0..number(&t) {
            let r = rect_de(&t, i);
            let p = screen_of(&t, i, &v);
            assert_eq!(p.x, ox + i64::from(r.x / k), "K={k} baldosa {i}");
            assert_eq!(p.y, oy + i64::from(r.y / k), "K={k} baldosa {i}");
            assert_eq!(
                p.x + p.width,
                ox + i64::from((r.x + r.width).div_ceil(k)),
                "K={k} baldosa {i}"
            );
            assert_eq!(
                p.y + p.height,
                oy + i64::from((r.y + r.height).div_ceil(k)),
                "K={k} baldosa {i}"
            );
        }
    }
}

#[test]
fn row_b1_uv_from_0_to_1_with_integer_zoom() {
    let viewport = Size::new(620, 684);
    for ((w, h), side, _) in CASES_B_1 {
        let image_2 = Size::new(w, h);
        let t = Tiling::new(image_2, side);
        for z in [level(1, 1), level(2, 1), level(5, 1), level(64, 1)] {
            let v = vista(viewport, image_2, z, (100, 100));
            for i in 0..number(&t) {
                assert_eq!(
                    t.uv(i, &v),
                    [0.0, 0.0, 1.0, 1.0],
                    "{w}×{h} ×{}: baldosa {i}",
                    z.num
                );
            }
        }
    }
}

/// El texel (por eje) que muestrea el centro de cada píxel de pantalla de una baldosa a 1/K:
/// `u0`, `u1` son el uv en los bordes del cuadrilátero de `screen` píxeles y `texels` el ancho
/// (o alto) de la textura de la baldosa; el uv se interpola linealmente.
fn texel_per_pixel(u0: f32, u1: f32, screen: i64, texels: u32) -> Vec<i64> {
    (0..screen)
        .map(|j| {
            let t = (j as f64 + 0.5) / screen as f64;
            let u = f64::from(u0) + (f64::from(u1) - f64::from(u0)) * t;
            (u * f64::from(texels)).floor() as i64
        })
        .collect()
}

#[test]
fn row_b1_at_1_k_the_center_of_each_screen_pixel_samples_the_texel_s_by_k_minus_a() {
    let viewport = Size::new(900, 700);
    for ((w, h), side, _) in CASES_B_1 {
        let image_2 = Size::new(w, h);
        let t = Tiling::new(image_2, side);
        for k in [2u32, 4, 8, 16, 32, 64] {
            let z = level(1, k);
            let v = vista(viewport, image_2, z, (450, 350));
            let (ox, oy) = v.origin();
            for i in 0..number(&t) {
                let r = rect_de(&t, i);
                let p = screen_of(&t, i, &v);
                let [u0, v0, u1, v1] = t.uv(i, &v);
                let label_2 = format!("{w}×{h} T={side} 1/{k} baldosa {i}");
                // Eje x: el píxel de pantalla `s` (desde el origen de la imagen) muestra `s·K − a`.
                let xs = texel_per_pixel(u0, u1, p.width, r.width);
                for (j, &texel) in xs.iter().enumerate() {
                    let s = (p.x - ox) + j as i64;
                    assert_eq!(
                        texel,
                        s * i64::from(k) - i64::from(r.x),
                        "{label_2}: x, píxel {s}"
                    );
                    assert!(
                        (0..i64::from(r.width)).contains(&texel),
                        "{label_2}: x fuera"
                    );
                }
                let ys = texel_per_pixel(v0, v1, p.height, r.height);
                for (j, &texel) in ys.iter().enumerate() {
                    let s = (p.y - oy) + j as i64;
                    assert_eq!(
                        texel,
                        s * i64::from(k) - i64::from(r.y),
                        "{label_2}: y, píxel {s}"
                    );
                    assert!(
                        (0..i64::from(r.height)).contains(&texel),
                        "{label_2}: y fuera"
                    );
                }
            }
        }
    }
}

#[test]
fn row_b1_visible_is_that_the_quad_cuts_the_canvas_with_positive_area() {
    let image_2 = Size::new(300, 200);
    for side in [16usize, 64, 128] {
        let t = Tiling::new(image_2, side);
        for z in levels().into_iter().step_by(4) {
            for viewport in [Size::new(160, 120), Size::new(97, 31), Size::new(1, 1)] {
                for ax in (-400..=400).step_by(53) {
                    for ay in (-300..=300).step_by(71) {
                        let v = vista(viewport, image_2, z, (ax, ay));
                        for i in 0..number(&t) {
                            let p = screen_of(&t, i, &v);
                            assert_eq!(
                                t.visible(i, &v, viewport),
                                short_the_canvas(p, viewport),
                                "T={side} ×{}/{} ancla ({ax}, {ay}) {viewport:?}: baldosa {i} en {p:?}",
                                z.num,
                                z.den
                            );
                        }
                    }
                }
            }
        }
    }
}

#[test]
fn row_b1_visible_with_an_empty_canvas_never_and_at_the_edge_only_with_area() {
    let image_2 = Size::new(300, 200);
    let t = Tiling::new(image_2, 128);
    let v = View::fit(Size::new(160, 120), image_2);
    for i in 0..number(&t) {
        assert!(!t.visible(i, &v, Size::new(0, 0)), "baldosa {i}");
        assert!(!t.visible(i, &v, Size::new(0, 120)), "baldosa {i}");
        assert!(!t.visible(i, &v, Size::new(160, 0)), "baldosa {i}");
    }
    // Con el origen en (−128, 0) la columna 0 acaba justo en el borde izquierdo (no se ve) y la
    // columna 1 empieza en 0 (se ve); la fila 1 empieza en y = 128, debajo de un lienzo de 120.
    let mut v = View::fit(Size::new(160, 120), image_2);
    v.set_zoom(Zoom::ONE, (138, 20));
    assert_eq!(v.origin(), (-128, 0), "preparación del banco");
    let visible_2: Vec<bool> = (0..number(&t))
        .map(|i| t.visible(i, &v, Size::new(160, 120)))
        .collect();
    assert_eq!(visible_2, [false, true, true, false, false, false]);
}

// ---------------------------------------------------------------------------------------------
// B2 · BlockSet
// ---------------------------------------------------------------------------------------------

fn has(b: &BlockSet, bx: u32, by: u32) -> bool {
    b.contains(bx, by)
}

/// Los bloques de un `BlockSet` como modelo, mirando solo `contains`.
fn model_of(b: &BlockSet) -> Model {
    let size = b.size();
    let mut m = Model::new();
    for by in 0..size.height.div_ceil(B) {
        for bx in 0..size.width.div_ceil(B) {
            if has(b, bx, by) {
                m.insert((bx, by));
            }
        }
    }
    m
}

fn with_pixels(width_2: u32, height_2: u32, pixels_2: &[(u32, u32)]) -> BlockSet {
    let mut b = BlockSet::new(width_2, height_2);
    for &(x, y) in pixels_2 {
        b.add_pixel(x, y);
    }
    b
}

fn checks(b: &BlockSet, expected_2: &Model, label_2: &str) {
    assert_eq!(&model_of(b), expected_2, "{label_2}");
    assert_eq!(b.len(), expected_2.len(), "{label_2}: len");
    assert_eq!(b.is_empty(), expected_2.is_empty(), "{label_2}: is_empty");
}

#[test]
fn row_b2_new_empty_and_full() {
    let b = BlockSet::new(200, 130);
    assert!(b.is_empty());
    assert_eq!(b.len(), 0);
    assert_eq!(b.size(), Size::new(200, 130));
    let full = BlockSet::full(200, 130);
    assert!(!full.is_empty());
    assert_eq!(full.len(), 4 * 3);
    assert_eq!(full.size(), Size::new(200, 130));
    checks(&full, &model_full(200, 130), "lleno");
    assert_ne!(b, full);
    assert_eq!(b.clone(), b);
}

#[test]
fn row_b2_add_pixel_at_the_edges_63_and_64_of_each_block() {
    // 200×130 → rejilla de 4×3 bloques.
    let cases: [((u32, u32), (u32, u32)); 8] = [
        ((0, 0), (0, 0)),
        ((63, 0), (0, 0)),
        ((64, 0), (1, 0)),
        ((63, 63), (0, 0)),
        ((64, 64), (1, 1)),
        ((127, 128), (1, 2)),
        ((128, 129), (2, 2)),
        ((199, 129), (3, 2)),
    ];
    for ((x, y), block) in cases {
        let b = with_pixels(200, 130, &[(x, y)]);
        let expected_2: Model = [block].into_iter().collect();
        checks(&b, &expected_2, &format!("píxel ({x}, {y})"));
        assert!(has(&b, block.0, block.1));
    }
    // Varios a la vez, repetidos: los bloques sin repetir.
    let b = with_pixels(200, 130, &[(63, 0), (64, 0), (64, 63), (63, 64), (0, 0)]);
    let expected_2: Model = [(0, 0), (1, 0), (0, 1)].into_iter().collect();
    checks(&b, &expected_2, "varios");
}

#[test]
fn row_b2_add_pixel_outside_the_image_is_ignored() {
    for (x, y) in [(200, 0), (0, 130), (200, 130), (u32::MAX, 0), (0, u32::MAX)] {
        let b = with_pixels(200, 130, &[(x, y)]);
        assert!(b.is_empty(), "píxel ({x}, {y}) fuera");
        assert_eq!(b.len(), 0);
    }
    // Un píxel fuera no quita ni cambia los de dentro.
    let b = with_pixels(200, 130, &[(5, 5), (200, 5), (5, 130)]);
    let expected_2: Model = [(0, 0)].into_iter().collect();
    checks(&b, &expected_2, "dentro y fuera");
}

#[test]
fn row_b2_add_rect_gives_the_blocks_that_cut_the_rectangle() {
    let rects: [(i64, i64, u64, u64); 16] = [
        (0, 0, 64, 64),
        (0, 0, 65, 64),
        (0, 0, 64, 65),
        (63, 63, 2, 2),
        (63, 63, 1, 1),
        (64, 64, 1, 1),
        (10, 10, 100, 40),
        (0, 0, 200, 130),
        (-10, -10, 20, 20),
        (-100, -100, 50, 50),
        (200, 0, 10, 10),
        (0, 130, 10, 10),
        (190, 120, 500, 500),
        (5, 5, 0, 10),
        (5, 5, 10, 0),
        (-50, 60, 400, 10),
    ];
    for (x, y, w, h) in rects {
        let mut b = BlockSet::new(200, 130);
        b.add_rect(x, y, w, h);
        checks(
            &b,
            &model_of_rect(x, y, w, h, 200, 130),
            &format!("add_rect({x}, {y}, {w}, {h})"),
        );
    }
}

#[test]
fn row_b2_add_rect_with_extreme_values_does_not_overflow() {
    let (iw, ih) = (200, 130);
    let ends: [(i64, i64, u64, u64); 6] = [
        (i64::MIN, i64::MIN, u64::MAX, u64::MAX),
        (i64::MAX, i64::MAX, u64::MAX, u64::MAX),
        (0, 0, u64::MAX, u64::MAX),
        (i64::MIN, 0, 1, 1),
        (i64::MAX, 0, 1, 1),
        (-1, -1, u64::MAX, 3),
    ];
    for (x, y, w, h) in ends {
        let mut b = BlockSet::new(iw, ih);
        b.add_rect(x, y, w, h);
        checks(
            &b,
            &model_of_rect(x, y, w, h, iw, ih),
            &format!("add_rect({x}, {y}, {w}, {h})"),
        );
    }
}

#[test]
fn row_b2_a_1x1_image() {
    let mut b = BlockSet::new(1, 1);
    assert!(b.is_empty());
    b.add_pixel(1, 0);
    b.add_pixel(0, 1);
    assert!(b.is_empty(), "fuera de la imagen");
    b.add_pixel(0, 0);
    assert_eq!(b.len(), 1);
    assert!(has(&b, 0, 0));
    assert!(!has(&b, 1, 0));
    assert_eq!(b, BlockSet::full(1, 1));
    let mut c = BlockSet::new(1, 1);
    c.add_rect(-5, -5, 10, 10);
    assert_eq!(c, BlockSet::full(1, 1));
    let mut d = BlockSet::new(1, 1);
    d.add_rect(1, 0, 10, 10);
    assert!(d.is_empty());
    // `runs` de la única baldosa de 1×1.
    let runs = b.runs(TileRect {
        x: 0,
        y: 0,
        width: 1,
        height: 1,
    });
    assert_eq!(
        runs,
        vec![TileRect {
            x: 0,
            y: 0,
            width: 1,
            height: 1
        }]
    );
}

#[test]
fn row_b2_an_8192_by_8192_image() {
    let mut b = BlockSet::new(8192, 8192);
    assert!(b.is_empty());
    assert_eq!(BlockSet::full(8192, 8192).len(), 128 * 128);
    b.add_pixel(8191, 8191);
    b.add_pixel(0, 0);
    b.add_pixel(8192, 8191);
    assert_eq!(b.len(), 2);
    assert!(has(&b, 127, 127) && has(&b, 0, 0));
    assert!(!has(&b, 126, 127));
    // La imagen entera como rectángulo es la imagen llena.
    let mut c = BlockSet::new(8192, 8192);
    c.add_rect(0, 0, 8192, 8192);
    assert_eq!(c, BlockSet::full(8192, 8192));
    // `runs` por baldosa de 2048: una baldosa llena da una serie por fila de bloques.
    let tile = TileRect {
        x: 2048,
        y: 4096,
        width: 2048,
        height: 2048,
    };
    let series = c.runs(tile);
    assert_eq!(series.len(), 32, "32 filas de bloques");
    for (i, s) in series.iter().enumerate() {
        assert_eq!(
            *s,
            TileRect {
                x: 2048,
                y: 4096 + 64 * i as u32,
                width: 2048,
                height: 64
            }
        );
    }
    // Un bloque suelto en otra baldosa no aparece en esta.
    assert!(b.runs(tile).is_empty());
    let last = TileRect {
        x: 6144,
        y: 6144,
        width: 2048,
        height: 2048,
    };
    assert_eq!(
        b.runs(last),
        vec![TileRect {
            x: 8128,
            y: 8128,
            width: 64,
            height: 64
        }]
    );
}

/// Conjuntos de prueba sobre una imagen de 300×200 (5×4 bloques).
fn sets_of_test() -> Vec<BlockSet> {
    let (w, h) = (300, 200);
    let mut v = vec![
        BlockSet::new(w, h),
        BlockSet::full(w, h),
        with_pixels(w, h, &[(0, 0)]),
        with_pixels(w, h, &[(70, 5), (130, 5), (299, 199)]),
        with_pixels(w, h, &[(63, 63), (64, 64), (64, 63), (63, 64)]),
        with_pixels(
            w,
            h,
            &[(5, 130), (70, 130), (140, 130), (200, 130), (270, 130)],
        ),
    ];
    let mut r = BlockSet::new(w, h);
    r.add_rect(60, 60, 100, 100);
    v.push(r);
    v
}

#[test]
fn row_b2_union_is_commutative_associative_idempotent_and_with_a_neutral() {
    let sets = sets_of_test();
    let union = |a: &BlockSet, b: &BlockSet| {
        let mut u = a.clone();
        u.union(b);
        u
    };
    for a in &sets {
        // Idempotente y con neutro.
        assert_eq!(union(a, a), *a);
        assert_eq!(union(a, &BlockSet::new(300, 200)), *a);
        assert_eq!(union(&BlockSet::new(300, 200), a), *a);
        assert_eq!(
            union(a, &BlockSet::full(300, 200)),
            BlockSet::full(300, 200)
        );
        for b in &sets {
            assert_eq!(union(a, b), union(b, a), "conmutativa");
            // Es la unión de los modelos.
            let expected_2: Model = model_of(a).union(&model_of(b)).copied().collect();
            checks(&union(a, b), &expected_2, "unión de modelos");
            for c in &sets {
                assert_eq!(union(&union(a, b), c), union(a, &union(b, c)), "asociativa");
            }
        }
    }
}

#[test]
fn row_b2_union_with_another_size_does_nothing() {
    let mut a = with_pixels(300, 200, &[(5, 5)]);
    let antes = a.clone();
    a.union(&BlockSet::full(301, 200));
    assert_eq!(a, antes);
    a.union(&BlockSet::full(300, 201));
    assert_eq!(a, antes);
    a.union(&BlockSet::full(1, 1));
    assert_eq!(a, antes);
    assert_eq!(a.size(), Size::new(300, 200));
}

#[test]
fn row_b2_clip_leaves_the_blocks_that_cut_the_rectangle_and_is_idempotent() {
    let rects = [
        TileRect {
            x: 0,
            y: 0,
            width: 64,
            height: 64,
        },
        TileRect {
            x: 64,
            y: 0,
            width: 128,
            height: 128,
        },
        TileRect {
            x: 63,
            y: 63,
            width: 2,
            height: 2,
        },
        TileRect {
            x: 128,
            y: 128,
            width: 172,
            height: 72,
        },
        TileRect {
            x: 100,
            y: 50,
            width: 1000,
            height: 1000,
        },
        TileRect {
            x: 299,
            y: 199,
            width: 1,
            height: 1,
        },
    ];
    for original in sets_of_test() {
        for r in rects {
            let mut b = original.clone();
            b.clip(r);
            let expected_2: Model = model_of(&original)
                .into_iter()
                .filter(|&(bx, by)| tiles_support::short(bx, by, r))
                .collect();
            checks(&b, &expected_2, &format!("clip {r:?}"));
            assert_eq!(b.size(), original.size(), "clip no cambia el tamaño");
            let a_time = b.clone();
            b.clip(r);
            assert_eq!(b, a_time, "clip es idempotente: {r:?}");
        }
    }
}

#[test]
fn row_b2_runs_merges_the_contiguous_blocks_of_a_row() {
    // Bloques (0,0), (1,0), (3,0) y (1,1) de una imagen de 300×200.
    let b = with_pixels(300, 200, &[(0, 0), (64, 0), (192, 0), (64, 64)]);
    let all = TileRect {
        x: 0,
        y: 0,
        width: 300,
        height: 200,
    };
    assert_eq!(
        b.runs(all),
        vec![
            TileRect {
                x: 0,
                y: 0,
                width: 128,
                height: 64
            },
            TileRect {
                x: 192,
                y: 0,
                width: 64,
                height: 64
            },
            TileRect {
                x: 64,
                y: 64,
                width: 64,
                height: 64
            },
        ]
    );
    // El último bloque de una fila y de una columna se recorta al rectángulo (y a la imagen).
    let mut c = BlockSet::new(300, 200);
    c.add_rect(256, 192, 44, 8);
    assert_eq!(
        c.runs(all),
        vec![TileRect {
            x: 256,
            y: 192,
            width: 44,
            height: 8
        }]
    );
    // Sin ningún bloque, nada; con todos, una serie por fila.
    assert!(BlockSet::new(300, 200).runs(all).is_empty());
    assert_eq!(BlockSet::full(300, 200).runs(all).len(), 4);
}

#[test]
fn row_b2_runs_clips_to_an_inner_rectangle_and_covers_exactly_the_dirty_blocks() {
    for original in sets_of_test() {
        let model = model_of(&original);
        for r in [
            TileRect {
                x: 0,
                y: 0,
                width: 300,
                height: 200,
            },
            TileRect {
                x: 32,
                y: 32,
                width: 100,
                height: 60,
            },
            TileRect {
                x: 128,
                y: 0,
                width: 128,
                height: 128,
            },
            TileRect {
                x: 256,
                y: 128,
                width: 44,
                height: 72,
            },
            TileRect {
                x: 63,
                y: 63,
                width: 2,
                height: 2,
            },
            TileRect {
                x: 64,
                y: 64,
                width: 1,
                height: 1,
            },
        ] {
            let series = original.runs(r);
            assert_eq!(series, runs_expected(&model, r), "runs de {r:?}");
            // Cada serie está dentro del rectángulo.
            for s in &series {
                assert!(s.x >= r.x && s.y >= r.y, "{s:?} fuera de {r:?}");
                assert!(s.x + s.width <= r.x + r.width && s.y + s.height <= r.y + r.height);
                assert!(s.width >= 1 && s.height >= 1);
            }
            // Y cubren exactamente los píxeles de los bloques sucios que cortan r, recortados a r.
            let mut covered = HashSet::new();
            for s in &series {
                for y in s.y..s.y + s.height {
                    for x in s.x..s.x + s.width {
                        assert!(covered.insert((x, y)), "serie solapada en ({x}, {y})");
                    }
                }
            }
            let mut expected_2_2 = HashSet::new();
            for y in r.y..r.y + r.height {
                for x in r.x..r.x + r.width {
                    if model.contains(&(x / B, y / B)) {
                        expected_2_2.insert((x, y));
                    }
                }
            }
            assert_eq!(covered, expected_2_2, "cobertura de {r:?}");
        }
    }
}

// ---------------------------------------------------------------------------------------------
// B4 · Baldosa desde las filas
// ---------------------------------------------------------------------------------------------

const WIDTH_B_4: u32 = 70;
const HEIGHT_B_4: u32 = 66;

fn rect(x: u32, y: u32, width: u32, height: u32) -> TileRect {
    TileRect {
        x,
        y,
        width,
        height,
    }
}

/// El rectángulo recortado a la imagen (`None` si no la corta).
fn clipped(r: TileRect, width_2: u32, height_2: u32) -> Option<TileRect> {
    let (x1, y1) = (
        (r.x.saturating_add(r.width)).min(width_2),
        (r.y.saturating_add(r.height)).min(height_2),
    );
    (r.x < x1 && r.y < y1).then(|| rect(r.x, r.y, x1 - r.x, y1 - r.y))
}

fn rectangles_b_4() -> Vec<TileRect> {
    vec![
        rect(5, 7, 20, 13),
        rect(50, 50, 20, 16),
        rect(0, 0, 1, 1),
        rect(69, 65, 1, 1),
        rect(69, 0, 1, 1),
        rect(0, 65, 1, 1),
        rect(33, 20, 1, 1),
        rect(0, 0, 70, 66),
        rect(0, 10, 70, 1),
        rect(10, 0, 1, 66),
        rect(64, 0, 6, 66),
        rect(0, 64, 70, 2),
    ]
}

fn image_b_4(mode_2: Mode) -> Image {
    image_of(mode_2, WIDTH_B_4, HEIGHT_B_4, 3)
}

#[test]
fn row_b4_tile_pixels_is_the_clip_of_color_image_from_rgba_unmultiplied_in_the_four_modes() {
    for mode_2 in TEST_MODES {
        let image_2 = image_b_4(mode_2);
        let reference = ColorImage::from_rgba_unmultiplied(
            [WIDTH_B_4 as usize, HEIGHT_B_4 as usize],
            &to_rgba8(&image_2),
        );
        for r in rectangles_b_4() {
            let obtained = tile_pixels(&image_2, r);
            let expected_2 = clip_2(&reference.pixels, WIDTH_B_4 as usize, r);
            assert_eq!(
                obtained.len(),
                (r.width * r.height) as usize,
                "{mode_2:?} {r:?}"
            );
            assert_eq!(obtained, expected_2, "{mode_2:?} {r:?}");
        }
    }
}

#[test]
fn row_b4_the_four_alphas_and_the_indexed_one_with_transparent_and_duplicates_appear_and_work() {
    // La imagen de prueba lleva los alfas 0, 1, 128 y 255 en RGBA, indexado y gris con alfa.
    for mode_2 in [Mode::Rgba, Mode::GrayAlpha, Mode::Indexed] {
        let image_2 = image_b_4(mode_2);
        let bytes = to_rgba8(&image_2);
        let alphas: HashSet<u8> = bytes.as_chunks::<4>().0.iter().map(|c| c[3]).collect();
        for alpha in [0u8, 1, 128, 255] {
            assert!(alphas.contains(&alpha), "{mode_2:?}: falta el alfa {alpha}");
        }
        let obtained = tile_pixels(&image_2, rect(0, 0, WIDTH_B_4, HEIGHT_B_4));
        assert_eq!(obtained, expected_2(&image_2), "{mode_2:?}");
        // Alfa 0 → transparente (premultiplicado).
        for (c, t) in bytes.as_chunks::<4>().0.iter().zip(&obtained) {
            if c[3] == 0 {
                assert_eq!(*t, Color32::TRANSPARENT, "{mode_2:?}: alfa 0");
            }
        }
    }
    // Indexado: el índice transparente (7) guarda blanco opaco pero se ve con alfa 0, y las entradas
    // 2 y 4 (duplicadas) dan el mismo texel.
    let image_2 = image_b_4(Mode::Indexed);
    let pixels_2 = tile_pixels(&image_2, rect(0, 0, WIDTH_B_4, HEIGHT_B_4));
    let mut seen_2 = [None; 8];
    for y in 0..HEIGHT_B_4 {
        for x in 0..WIDTH_B_4 {
            if let Ok(Pixel::Indexed(i)) = image_2.get_pixel(x, y) {
                let t = pixels_2[(y * WIDTH_B_4 + x) as usize];
                match seen_2[usize::from(i)] {
                    None => seen_2[usize::from(i)] = Some(t),
                    Some(antes) => assert_eq!(antes, t, "la entrada {i} siempre da el mismo texel"),
                }
            }
        }
    }
    assert_eq!(seen_2[7], Some(Color32::TRANSPARENT), "índice transparente");
    assert_eq!(seen_2[2], seen_2[4], "entradas duplicadas");
    assert_eq!(seen_2[5], Some(Color32::TRANSPARENT), "entrada de alfa 0");
}

#[test]
fn row_b4_a_rectangle_that_sticks_out_is_clipped() {
    for mode_2 in TEST_MODES {
        let image_2 = image_b_4(mode_2);
        let complete = expected_2(&image_2);
        for r in [
            rect(60, 60, 30, 30),
            rect(69, 0, 100, 3),
            rect(0, 65, 3, 100),
            rect(0, 0, 1000, 1000),
        ] {
            let c = clipped(r, WIDTH_B_4, HEIGHT_B_4).expect("corta la imagen");
            assert_eq!(
                tile_pixels(&image_2, r),
                clip_2(&complete, WIDTH_B_4 as usize, c),
                "{mode_2:?} {r:?}"
            );
        }
        for r in [rect(70, 0, 5, 5), rect(0, 66, 5, 5), rect(500, 500, 3, 3)] {
            assert!(tile_pixels(&image_2, r).is_empty(), "{mode_2:?} {r:?}");
        }
    }
}

#[test]
fn row_b4_tile_pixels_with_entry_is_the_clip_of_to_rgba8_with_entry() {
    let red = Rgba::opaque(250, 10, 10);
    let semi = Rgba::new(10, 250, 10, 128);
    for mode_2 in TEST_MODES {
        let image_2 = image_b_4(mode_2);
        // Entradas: una normal, la transparente (7), una duplicada (4), una de alfa 0 (5) y una fuera
        // de la paleta (200); en los modos sin paleta no cambia nada.
        for (index, color) in [
            (2u8, red),
            (7, red),
            (4, semi),
            (5, red),
            (200, red),
            (1, Rgba::TRANSPARENT),
        ] {
            let reference = ColorImage::from_rgba_unmultiplied(
                [WIDTH_B_4 as usize, HEIGHT_B_4 as usize],
                &to_rgba8_with_entry(&image_2, index, color),
            );
            for r in rectangles_b_4() {
                assert_eq!(
                    tile_pixels_with_entry(&image_2, r, index, color),
                    clip_2(&reference.pixels, WIDTH_B_4 as usize, r),
                    "{mode_2:?} entrada {index} {color:?} {r:?}"
                );
            }
        }
    }
    // En indexado la entrada transparente conserva alfa 0 con el RGB nuevo (a efectos del texel:
    // transparente) y las duplicadas no se tocan.
    let image_2 = image_b_4(Mode::Indexed);
    let all_2 = tile_pixels_with_entry(&image_2, rect(0, 0, WIDTH_B_4, HEIGHT_B_4), 7, red);
    for y in 0..HEIGHT_B_4 {
        for x in 0..WIDTH_B_4 {
            if image_2.get_pixel(x, y) == Ok(Pixel::Indexed(7)) {
                assert_eq!(all_2[(y * WIDTH_B_4 + x) as usize].a(), 0, "({x}, {y})");
            }
        }
    }
}

/// Asignador falso: da memoria las primeras `good` veces y después falla.
struct Fail {
    good: usize,
    calls: usize,
}

impl TileAlloc for Fail {
    fn pixels(&mut self, len: usize) -> Option<Vec<Color32>> {
        self.calls += 1;
        (self.calls <= self.good).then(|| Vec::with_capacity(len))
    }
}

#[test]
fn row_b4_try_tile_pixels_gives_the_same_as_tile_pixels_and_with_a_failure_out_of_memory() {
    let image_2 = image_b_4(Mode::Rgba);
    let r = rect(5, 7, 20, 13);
    let mut well = Fail {
        good: usize::MAX,
        calls: 0,
    };
    let with_allocator = try_tile_pixels(&image_2, r, &mut well).expect("hay memoria");
    assert_eq!(with_allocator, tile_pixels(&image_2, r));
    assert_eq!(well.calls, 1, "una reserva por baldosa");
    let mut badly = Fail { good: 0, calls: 0 };
    assert_eq!(
        try_tile_pixels(&image_2, r, &mut badly),
        Err(CanvasError::OutOfMemory)
    );
}

// ---------------------------------------------------------------------------------------------
// Q1 · Geometría de baldosas
// ---------------------------------------------------------------------------------------------

proptest! {
    #![proptest_config(ProptestConfig::with_cases(256))]

    /// Q1: las baldosas cubren exactamente la imagen sin solapes, cada una ≤ T y solo las del borde
    /// son menores; sus rectángulos en pantalla, a cualquier zoom y origen entero, cubren
    /// exactamente los `extent` de `image_rect`.
    #[test]
    fn property_q1_the_tiles_cover_the_image_and_the_screen_covers_image_rect(
        width_2 in 1u32..=20_000,
        height_2 in 1u32..=20_000,
        side in (1usize..=32).prop_map(|k| k * 64),
        index_level in 0usize..70,
        ax in -4000i64..4000,
        ay in -4000i64..4000,
        vw in 0u32..2000,
        vh in 0u32..2000,
        swatches in proptest::collection::vec(any::<u32>(), 8),
    ) {
        let image_2 = Size::new(width_2, height_2);
        let t = Tiling::new(image_2, side);
        let cols = (width_2 as usize).div_ceil(side);
        let rows = (height_2 as usize).div_ceil(side);
        prop_assert_eq!(columns_2(&t), cols);
        prop_assert_eq!(rows_2(&t), rows);
        prop_assert_eq!(number(&t), cols * rows);

        // Columnas (la fila 0) y filas (la columna 0): empiezan donde acaba la anterior, miden T
        // salvo la última, que mide lo que queda.
        let mut x_expected = 0u32;
        for c in 0..cols {
            let r = rect_de(&t, c);
            prop_assert_eq!(r.x, x_expected);
            prop_assert!(r.width >= 1 && r.width as usize <= side);
            if c + 1 < cols {
                prop_assert_eq!(r.width as usize, side, "solo las del borde son menores");
            }
            x_expected += r.width;
        }
        prop_assert_eq!(x_expected, width_2);
        let mut y_expected = 0u32;
        for f in 0..rows {
            let r = rect_de(&t, f * cols);
            prop_assert_eq!(r.y, y_expected);
            prop_assert!(r.height >= 1 && r.height as usize <= side);
            if f + 1 < rows {
                prop_assert_eq!(r.height as usize, side);
            }
            y_expected += r.height;
        }
        prop_assert_eq!(y_expected, height_2);

        // Algunas baldosas sueltas: el rectángulo es el de su columna y su fila.
        for m in &swatches {
            let i = *m as usize % (cols * rows);
            let (col, row) = (i % cols, i / cols);
            let (c, f) = (rect_de(&t, col), rect_de(&t, row * cols));
            prop_assert_eq!(rect_de(&t, i), rect(c.x, f.y, c.width, f.height));
        }

        // Pantalla: a cualquier zoom y origen entero.
        let z = levels()[index_level];
        let v = vista(Size::new(vw, vh), image_2, z, (ax, ay));
        let frame = v.image_rect(image_2);
        prop_assert_eq!(frame.width, ext(z, width_2));
        prop_assert_eq!(frame.height, ext(z, height_2));
        let mut x_screen = frame.x;
        for c in 0..cols {
            let p = screen_of(&t, c, &v);
            prop_assert_eq!(p.x, x_screen);
            prop_assert_eq!(p, screen_expected(&v, z, rect_de(&t, c)));
            prop_assert!(p.width >= 0);
            x_screen += p.width;
        }
        prop_assert_eq!(x_screen, frame.x + frame.width, "sin huecos ni sobrantes en x");
        let mut y_screen = frame.y;
        for f in 0..rows {
            let p = screen_of(&t, f * cols, &v);
            prop_assert_eq!(p.y, y_screen);
            prop_assert!(p.height >= 0);
            y_screen += p.height;
        }
        prop_assert_eq!(y_screen, frame.y + frame.height, "sin huecos ni sobrantes en y");
        for m in &swatches {
            let i = *m as usize % (cols * rows);
            let (col, row) = (i % cols, i / cols);
            let (a, b) = (screen_of(&t, col, &v), screen_of(&t, row * cols, &v));
            prop_assert_eq!(
                screen_of(&t, i, &v),
                ScreenRect { x: a.x, y: b.y, width: a.width, height: b.height }
            );
        }
    }
}

// ---------------------------------------------------------------------------------------------
// Q3 · Región
// ---------------------------------------------------------------------------------------------

/// Los píxeles cuyo `to_rgba8` cambia entre `antes` y `after_2` están todos en `dirty_2`.
fn contains_what_changes(antes: &Image, after_2: &Image, dirty_2: &BlockSet) -> Result<(), String> {
    if (antes.width(), antes.height()) != (after_2.width(), after_2.height()) {
        return Err("el tamaño cambió".to_string());
    }
    let (a, d) = (to_rgba8(antes), to_rgba8(after_2));
    let width_2 = antes.width() as usize;
    for (i, (pa, pd)) in a
        .as_chunks::<4>()
        .0
        .iter()
        .zip(d.as_chunks::<4>().0.iter())
        .enumerate()
    {
        if pa != pd {
            let (x, y) = ((i % width_2) as u32, (i / width_2) as u32);
            if !dirty_2.contains(x / B, y / B) {
                return Err(format!(
                    "el píxel ({x}, {y}) cambió ({pa:?} → {pd:?}) y su bloque no está en la región"
                ));
            }
        }
    }
    Ok(())
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(256))]

    /// Q3: para cualquier comando válido sobre cualquier imagen, los bloques de `dirty_of` contienen
    /// todo píxel cuyo `to_rgba8` cambia, y lo mismo con `undo` y `redo` a través del registro por
    /// `state_id` (clave ausente → toda la imagen).
    #[test]
    fn property_q3_dirty_of_and_the_log_contain_everything_that_changes(
        mode_2 in 0u8..4,
        width_2 in 1u32..=140,
        height_2 in 1u32..=140,
        cel in 1u32..=9,
        steps_2 in proptest::collection::vec(seed(), 1..=8),
    ) {
        let image_2 = image_of(TEST_MODES[usize::from(mode_2)], width_2, height_2, cel);
        let size_2 = Size::new(width_2, height_2);
        let mut doc = Document::new(image_2);
        let mut log = ChangeLog::new();
        for (n, s) in steps_2.into_iter().enumerate() {
            let id = ImageId::from_raw(0);
            let antes = doc.image(id).expect("imagen").clone();
            match step_of(&doc, s) {
                Step::Nothing => {}
                Step::Applies(command) => {
                    let region = dirty_of(&command, size_2);
                    if let Ok(true) = doc.apply(command.clone()) {
                        log.record(doc.state_id(), region.clone());
                        let after_2 = doc.image(id).expect("imagen");
                        let ok = contains_what_changes(&antes, after_2, &region);
                        prop_assert!(ok.is_ok(), "paso {n} {command:?}: {:?}", ok.err());
                    }
                }
                Step::Undo => {
                    let from_2 = doc.state_id();
                    if doc.undo().is_ok() {
                        let region = log.get(from_2).cloned().unwrap_or_else(|| BlockSet::full(width_2, height_2));
                        let after_2 = doc.image(id).expect("imagen");
                        let ok = contains_what_changes(&antes, after_2, &region);
                        prop_assert!(ok.is_ok(), "paso {n} deshacer: {:?}", ok.err());
                    }
                }
                Step::Redo => {
                    if doc.redo().is_ok() {
                        let towards = doc.state_id();
                        let region = log.get(towards).cloned().unwrap_or_else(|| BlockSet::full(width_2, height_2));
                        let after_2 = doc.image(id).expect("imagen");
                        let ok = contains_what_changes(&antes, after_2, &region);
                        prop_assert!(ok.is_ok(), "paso {n} rehacer: {:?}", ok.err());
                    }
                }
            }
        }
    }
}

/// Q3 con los comandos sueltos más delicados, sin azar: cada uno sobre una imagen que cruza varios
/// bloques, con la región escrita a mano.
#[test]
fn property_q3_fixed_cases_of_the_block_edges() {
    let id = |_: &Document| ImageId::from_raw(0);
    let mut doc = Document::new(image_of(Mode::Rgba, 200, 130, 5));
    let size_2 = Size::new(200, 130);
    let pixel = Pixel::Rgba(Rgba::opaque(1, 2, 3));
    let commands = vec![
        Command::SetPixels {
            image: id(&doc),
            pixels: vec![
                PixelEdit { x: 63, y: 0, pixel },
                PixelEdit { x: 64, y: 0, pixel },
                PixelEdit {
                    x: 199,
                    y: 129,
                    pixel,
                },
            ],
        },
        Command::FillSpans {
            image: id(&doc),
            spans: vec![
                Span {
                    y: 5,
                    x_start: 0,
                    x_end: 64,
                },
                Span {
                    y: 70,
                    x_start: 60,
                    x_end: 70,
                },
            ],
            pixel: Pixel::Rgba(Rgba::opaque(9, 9, 9)),
        },
    ];
    for command in commands {
        let antes = doc.image(id(&doc)).expect("imagen").clone();
        let region = dirty_of(&command, size_2);
        assert_eq!(doc.apply(command), Ok(true));
        let after_2 = doc.image(id(&doc)).expect("imagen");
        assert_eq!(contains_what_changes(&antes, after_2, &region), Ok(()));
    }
}
