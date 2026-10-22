//! Baldosas, región sucia y baldosa desde las filas (specs/hito-2-zoom-baldosas.md, unidad 14a-2:
//! filas B1, B2 y B4 y propiedades Q1 y Q3; contrato de API en «Contrato de API de la 14a-2»).
//! Todo es lógica pura de `px-ui`, sin egui más que `Color32` y `ColorImage`.
//!
//! - B1: `tile_side`, `Tiling` (`rect`, `screen_rect`, `uv`, `visible`): conteo y recorte en el
//!   borde, cobertura exacta sin solapes ni huecos en imagen y en pantalla a varios zooms (también
//!   1/K), uv de 0 a 1 con zoom ≥ ×1 y, a 1/K, el centro de cada píxel de pantalla muestrea el
//!   texel `s·K − a` de su baldosa.
//! - B2: `BlockSet` frente a un modelo de conjunto de coordenadas escrito aquí.
//! - B4: `tile_pixels` frente a `ColorImage::from_rgba_unmultiplied(to_rgba8(imagen))` recortado.
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
    B, MODOS, Modelo, Paso, esperado, imagen_de, modelo_de_rect, modelo_lleno, paso_de, recorte,
    runs_esperados, semilla,
};

// ---------------------------------------------------------------------------------------------
// Oráculo de zoom: fracciones con i128
// ---------------------------------------------------------------------------------------------

/// Un nivel de la escalera y su fracción `num/den`.
#[derive(Clone, Copy)]
struct Nivel {
    zoom: Zoom,
    num: i128,
    den: i128,
}

/// Los 70 niveles, de menor a mayor.
fn niveles() -> Vec<Nivel> {
    let mut v: Vec<Nivel> = [64u32, 32, 16, 8, 4, 2]
        .iter()
        .map(|&k| Nivel {
            zoom: Zoom::shrink(k).expect("nivel 1/K"),
            num: 1,
            den: i128::from(k),
        })
        .collect();
    v.extend((1..=64u32).map(|n| Nivel {
        zoom: Zoom::from_int(n).expect("nivel entero"),
        num: i128::from(n),
        den: 1,
    }));
    v
}

fn nivel(num: u32, den: u32) -> Nivel {
    niveles()
        .into_iter()
        .find(|n| n.num == i128::from(num) && n.den == i128::from(den))
        .expect("nivel de la escalera")
}

/// `⌈n·num/den⌉`: lo que ocupan `n` píxeles de imagen en pantalla.
fn ext(nivel: Nivel, n: u32) -> i64 {
    let q = i128::from(n) * nivel.num;
    (-((-q).div_euclid(nivel.den))) as i64
}

/// Una vista en el nivel dado y con el origen que sale de anclar en `ancla` (sin `clamp`).
fn vista(viewport: Size, imagen: Size, nivel: Nivel, ancla: (i64, i64)) -> View {
    let mut v = View::fit(viewport, imagen);
    v.set_zoom(nivel.zoom, ancla);
    v
}

/// El rectángulo de pantalla de una baldosa según la fórmula del contrato, con enteros.
fn pantalla_esperada(vista: &View, nivel: Nivel, r: TileRect) -> ScreenRect {
    let (ox, oy) = vista.origin();
    ScreenRect {
        x: ox + ext(nivel, r.x),
        y: oy + ext(nivel, r.y),
        width: ext(nivel, r.x + r.width) - ext(nivel, r.x),
        height: ext(nivel, r.y + r.height) - ext(nivel, r.y),
    }
}

fn corta_el_lienzo(r: ScreenRect, viewport: Size) -> bool {
    let x0 = r.x.max(0);
    let x1 = (r.x + r.width).min(i64::from(viewport.width));
    let y0 = r.y.max(0);
    let y1 = (r.y + r.height).min(i64::from(viewport.height));
    x1 > x0 && y1 > y0
}

fn rect_de(t: &Tiling, i: usize) -> TileRect {
    t.rect(i)
}

fn pantalla_de(t: &Tiling, i: usize, v: &View) -> ScreenRect {
    t.screen_rect(i, v)
}

fn numero(t: &Tiling) -> usize {
    t.len()
}

fn columnas(t: &Tiling) -> usize {
    t.cols()
}

fn filas(t: &Tiling) -> usize {
    t.rows()
}

// ---------------------------------------------------------------------------------------------
// B1 · Geometría de baldosas
// ---------------------------------------------------------------------------------------------

/// Imágenes, lado de baldosa y número de baldosas de la tabla de B1.
const CASOS_B1: [((u32, u32), usize, usize); 7] = [
    ((1, 1), 1024, 1),
    ((2048, 2048), 1024, 4),
    ((2049, 1), 1024, 3),
    ((1025, 2), 1024, 2),
    ((8192, 8192), 2048, 16),
    ((6000, 6000), 2048, 9),
    ((16384, 4096), 2048, 16),
];

#[test]
fn fila_b1_las_constantes_son_las_de_la_spec() {
    assert_eq!(TILE_SIDE, 2048);
    assert_eq!(BLOCK, 64);
}

#[test]
fn fila_b1_tile_side_da_la_mayor_potencia_de_dos_acotada_con_minimo_64() {
    let casos: [(usize, usize); 6] = [
        (32768, 2048),
        (4096, 2048),
        (2048, 2048),
        (1024, 1024),
        (1000, 512),
        (64, 64),
    ];
    for (max, esperado) in casos {
        assert_eq!(tile_side(max), esperado, "tile_side({max})");
    }
    // Alrededor de las potencias de dos, por debajo del mínimo y en los extremos.
    for (max, esperado) in [
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
        assert_eq!(tile_side(max), esperado, "tile_side({max})");
    }
    // Siempre potencia de dos, múltiplo de 64 y ≤ min(2048, max) (salvo el mínimo).
    for max in 1..=4200usize {
        let lado = tile_side(max);
        assert!(lado.is_power_of_two(), "{max} → {lado}");
        assert_eq!(lado % 64, 0, "{max} → {lado}");
        assert!(lado <= 2048, "{max} → {lado}");
        assert!(lado <= max.max(64), "{max} → {lado}");
        // Y es la mayor: el doble ya no cabe (o pasa de 2048).
        assert!(lado * 2 > max.min(2048) || lado == 64, "{max} → {lado}");
    }
}

#[test]
fn fila_b1_numero_de_baldosas_y_recorte_en_el_borde() {
    for ((w, h), lado, cuantas) in CASOS_B1 {
        let t = Tiling::new(Size::new(w, h), lado);
        let etiqueta = format!("{w}×{h} con T = {lado}");
        assert_eq!(t.side(), lado, "{etiqueta}");
        assert_eq!(numero(&t), cuantas, "{etiqueta}");
        let (cols, rows) = ((w as usize).div_ceil(lado), (h as usize).div_ceil(lado));
        assert_eq!(columnas(&t), cols, "{etiqueta}");
        assert_eq!(filas(&t), rows, "{etiqueta}");
        assert_eq!(cols * rows, cuantas, "{etiqueta}");
        // Orden por filas: `índice = fila · cols + col`; las del borde, recortadas.
        for i in 0..cuantas {
            let (col, row) = (i % cols, i / cols);
            let x = (col * lado) as u32;
            let y = (row * lado) as u32;
            let esperado = TileRect {
                x,
                y,
                width: (w - x).min(lado as u32),
                height: (h - y).min(lado as u32),
            };
            assert_eq!(rect_de(&t, i), esperado, "{etiqueta}: baldosa {i}");
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
fn fila_b1_las_baldosas_cubren_la_imagen_sin_solapes_ni_huecos() {
    for ((w, h), lado, _) in CASOS_B1 {
        let t = Tiling::new(Size::new(w, h), lado);
        let rects: Vec<TileRect> = (0..numero(&t)).map(|i| rect_de(&t, i)).collect();
        let area: u64 = rects
            .iter()
            .map(|r| u64::from(r.width) * u64::from(r.height))
            .sum();
        assert_eq!(area, u64::from(w) * u64::from(h), "{w}×{h}: área total");
        for (i, a) in rects.iter().enumerate() {
            assert!(a.width >= 1 && a.height >= 1, "{w}×{h}: baldosa {i} vacía");
            assert!(a.x + a.width <= w && a.y + a.height <= h, "{w}×{h}: {i}");
            for (j, b) in rects.iter().enumerate().skip(i + 1) {
                let solapan = a.x < b.x + b.width
                    && b.x < a.x + a.width
                    && a.y < b.y + b.height
                    && b.y < a.y + a.height;
                assert!(!solapan, "{w}×{h}: las baldosas {i} y {j} se solapan");
            }
        }
    }
}

/// Zooms de B1: ×1, ×5 y 1/64 (los de la tabla) y otros fraccionarios y el máximo.
fn zooms_b1() -> Vec<Nivel> {
    vec![
        nivel(1, 1),
        nivel(5, 1),
        nivel(1, 64),
        nivel(1, 2),
        nivel(1, 4),
        nivel(1, 8),
        nivel(1, 32),
        nivel(3, 1),
        nivel(64, 1),
    ]
}

#[test]
fn fila_b1_screen_rect_cubre_exactamente_image_rect_sin_solapes_ni_huecos() {
    let viewport = Size::new(620, 684);
    for ((w, h), lado, _) in CASOS_B1 {
        let imagen = Size::new(w, h);
        let t = Tiling::new(imagen, lado);
        for z in zooms_b1() {
            for ancla in [(0, 0), (310, 342), (-5000, 77), (900, -1234)] {
                let v = vista(viewport, imagen, z, ancla);
                let etiqueta = format!("{w}×{h} T={lado} ×{}/{} ancla {ancla:?}", z.num, z.den);
                let marco = v.image_rect(imagen);
                let pantallas: Vec<ScreenRect> =
                    (0..numero(&t)).map(|i| pantalla_de(&t, i, &v)).collect();
                for (i, p) in pantallas.iter().enumerate() {
                    assert_eq!(
                        *p,
                        pantalla_esperada(&v, z, rect_de(&t, i)),
                        "{etiqueta}: baldosa {i}"
                    );
                }
                // Por columnas y por filas: contiguas, empiezan en el origen y acaban en el final.
                let (cols, rows) = (columnas(&t), filas(&t));
                assert_eq!(pantallas[0].x, marco.x, "{etiqueta}");
                assert_eq!(pantallas[0].y, marco.y, "{etiqueta}");
                let ultima = pantallas[cols * rows - 1];
                assert_eq!(ultima.x + ultima.width, marco.x + marco.width, "{etiqueta}");
                assert_eq!(
                    ultima.y + ultima.height,
                    marco.y + marco.height,
                    "{etiqueta}"
                );
                for c in 0..cols - 1 {
                    let (a, b) = (pantallas[c], pantallas[c + 1]);
                    assert_eq!(
                        a.x + a.width,
                        b.x,
                        "{etiqueta}: hueco o solape entre columnas {c}"
                    );
                }
                for r in 0..rows - 1 {
                    let (a, b) = (pantallas[r * cols], pantallas[(r + 1) * cols]);
                    assert_eq!(
                        a.y + a.height,
                        b.y,
                        "{etiqueta}: hueco o solape entre filas {r}"
                    );
                }
                // Cada baldosa comparte x y ancho con su columna, y y alto con su fila.
                for (i, p) in pantallas.iter().enumerate() {
                    let (col, row) = (i % cols, i / cols);
                    assert_eq!((p.x, p.width), (pantallas[col].x, pantallas[col].width));
                    assert_eq!(
                        (p.y, p.height),
                        (pantallas[row * cols].y, pantallas[row * cols].height)
                    );
                }
                // El área sumada es la de `image_rect`.
                let area: i128 = pantallas
                    .iter()
                    .map(|p| i128::from(p.width) * i128::from(p.height))
                    .sum();
                assert_eq!(
                    area,
                    i128::from(marco.width) * i128::from(marco.height),
                    "{etiqueta}"
                );
            }
        }
    }
}

#[test]
fn fila_b1_a_1_k_el_cuadrilatero_va_de_a_k_a_techo_de_b_k_desde_el_origen() {
    // A 1/K el lado (múltiplo de 64, y de K) da `[o + a/K, o + ⌈b/K⌉)`: la fórmula escrita sin
    // `extent`, para la baldosa del borde con recorte incluido.
    let imagen = Size::new(6000, 6000);
    let t = Tiling::new(imagen, 2048);
    for k in [2u32, 4, 8, 16, 32, 64] {
        let z = nivel(1, k);
        let v = vista(Size::new(800, 600), imagen, z, (400, 300));
        let (ox, oy) = v.origin();
        for i in 0..numero(&t) {
            let r = rect_de(&t, i);
            let p = pantalla_de(&t, i, &v);
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
fn fila_b1_uv_de_0_a_1_con_zoom_entero() {
    let viewport = Size::new(620, 684);
    for ((w, h), lado, _) in CASOS_B1 {
        let imagen = Size::new(w, h);
        let t = Tiling::new(imagen, lado);
        for z in [nivel(1, 1), nivel(2, 1), nivel(5, 1), nivel(64, 1)] {
            let v = vista(viewport, imagen, z, (100, 100));
            for i in 0..numero(&t) {
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
/// `u0`, `u1` son el uv en los bordes del cuadrilátero de `pantalla` píxeles y `texels` el ancho
/// (o alto) de la textura de la baldosa; el uv se interpola linealmente.
fn texel_por_pixel(u0: f32, u1: f32, pantalla: i64, texels: u32) -> Vec<i64> {
    (0..pantalla)
        .map(|j| {
            let t = (j as f64 + 0.5) / pantalla as f64;
            let u = f64::from(u0) + (f64::from(u1) - f64::from(u0)) * t;
            (u * f64::from(texels)).floor() as i64
        })
        .collect()
}

#[test]
fn fila_b1_a_1_k_el_centro_de_cada_pixel_de_pantalla_muestrea_el_texel_s_por_k_menos_a() {
    let viewport = Size::new(900, 700);
    for ((w, h), lado, _) in CASOS_B1 {
        let imagen = Size::new(w, h);
        let t = Tiling::new(imagen, lado);
        for k in [2u32, 4, 8, 16, 32, 64] {
            let z = nivel(1, k);
            let v = vista(viewport, imagen, z, (450, 350));
            let (ox, oy) = v.origin();
            for i in 0..numero(&t) {
                let r = rect_de(&t, i);
                let p = pantalla_de(&t, i, &v);
                let [u0, v0, u1, v1] = t.uv(i, &v);
                let etiqueta = format!("{w}×{h} T={lado} 1/{k} baldosa {i}");
                // Eje x: el píxel de pantalla `s` (desde el origen de la imagen) muestra `s·K − a`.
                let xs = texel_por_pixel(u0, u1, p.width, r.width);
                for (j, &texel) in xs.iter().enumerate() {
                    let s = (p.x - ox) + j as i64;
                    assert_eq!(
                        texel,
                        s * i64::from(k) - i64::from(r.x),
                        "{etiqueta}: x, píxel {s}"
                    );
                    assert!(
                        (0..i64::from(r.width)).contains(&texel),
                        "{etiqueta}: x fuera"
                    );
                }
                let ys = texel_por_pixel(v0, v1, p.height, r.height);
                for (j, &texel) in ys.iter().enumerate() {
                    let s = (p.y - oy) + j as i64;
                    assert_eq!(
                        texel,
                        s * i64::from(k) - i64::from(r.y),
                        "{etiqueta}: y, píxel {s}"
                    );
                    assert!(
                        (0..i64::from(r.height)).contains(&texel),
                        "{etiqueta}: y fuera"
                    );
                }
            }
        }
    }
}

#[test]
fn fila_b1_visible_es_que_el_cuadrilatero_corta_el_lienzo_con_area_positiva() {
    let imagen = Size::new(300, 200);
    for lado in [16usize, 64, 128] {
        let t = Tiling::new(imagen, lado);
        for z in niveles().into_iter().step_by(4) {
            for viewport in [Size::new(160, 120), Size::new(97, 31), Size::new(1, 1)] {
                for ax in (-400..=400).step_by(53) {
                    for ay in (-300..=300).step_by(71) {
                        let v = vista(viewport, imagen, z, (ax, ay));
                        for i in 0..numero(&t) {
                            let p = pantalla_de(&t, i, &v);
                            assert_eq!(
                                t.visible(i, &v, viewport),
                                corta_el_lienzo(p, viewport),
                                "T={lado} ×{}/{} ancla ({ax}, {ay}) {viewport:?}: baldosa {i} en {p:?}",
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
fn fila_b1_visible_con_un_lienzo_vacio_nunca_y_en_el_borde_solo_con_area() {
    let imagen = Size::new(300, 200);
    let t = Tiling::new(imagen, 128);
    let v = View::fit(Size::new(160, 120), imagen);
    for i in 0..numero(&t) {
        assert!(!t.visible(i, &v, Size::new(0, 0)), "baldosa {i}");
        assert!(!t.visible(i, &v, Size::new(0, 120)), "baldosa {i}");
        assert!(!t.visible(i, &v, Size::new(160, 0)), "baldosa {i}");
    }
    // Con el origen en (−128, 0) la columna 0 acaba justo en el borde izquierdo (no se ve) y la
    // columna 1 empieza en 0 (se ve); la fila 1 empieza en y = 128, debajo de un lienzo de 120.
    let mut v = View::fit(Size::new(160, 120), imagen);
    v.set_zoom(Zoom::ONE, (138, 20));
    assert_eq!(v.origin(), (-128, 0), "preparación del banco");
    let visibles: Vec<bool> = (0..numero(&t))
        .map(|i| t.visible(i, &v, Size::new(160, 120)))
        .collect();
    assert_eq!(visibles, [false, true, true, false, false, false]);
}

// ---------------------------------------------------------------------------------------------
// B2 · BlockSet
// ---------------------------------------------------------------------------------------------

fn tiene(b: &BlockSet, bx: u32, by: u32) -> bool {
    b.contains(bx, by)
}

/// Los bloques de un `BlockSet` como modelo, mirando solo `contains`.
fn modelo_de(b: &BlockSet) -> Modelo {
    let size = b.size();
    let mut m = Modelo::new();
    for by in 0..size.height.div_ceil(B) {
        for bx in 0..size.width.div_ceil(B) {
            if tiene(b, bx, by) {
                m.insert((bx, by));
            }
        }
    }
    m
}

fn con_pixeles(ancho: u32, alto: u32, pixeles: &[(u32, u32)]) -> BlockSet {
    let mut b = BlockSet::new(ancho, alto);
    for &(x, y) in pixeles {
        b.add_pixel(x, y);
    }
    b
}

fn comprueba(b: &BlockSet, esperado: &Modelo, etiqueta: &str) {
    assert_eq!(&modelo_de(b), esperado, "{etiqueta}");
    assert_eq!(b.len(), esperado.len(), "{etiqueta}: len");
    assert_eq!(b.is_empty(), esperado.is_empty(), "{etiqueta}: is_empty");
}

#[test]
fn fila_b2_nuevo_vacio_y_lleno() {
    let b = BlockSet::new(200, 130);
    assert!(b.is_empty());
    assert_eq!(b.len(), 0);
    assert_eq!(b.size(), Size::new(200, 130));
    let lleno = BlockSet::full(200, 130);
    assert!(!lleno.is_empty());
    assert_eq!(lleno.len(), 4 * 3);
    assert_eq!(lleno.size(), Size::new(200, 130));
    comprueba(&lleno, &modelo_lleno(200, 130), "lleno");
    assert_ne!(b, lleno);
    assert_eq!(b.clone(), b);
}

#[test]
fn fila_b2_add_pixel_en_los_bordes_63_y_64_de_cada_bloque() {
    // 200×130 → rejilla de 4×3 bloques.
    let casos: [((u32, u32), (u32, u32)); 8] = [
        ((0, 0), (0, 0)),
        ((63, 0), (0, 0)),
        ((64, 0), (1, 0)),
        ((63, 63), (0, 0)),
        ((64, 64), (1, 1)),
        ((127, 128), (1, 2)),
        ((128, 129), (2, 2)),
        ((199, 129), (3, 2)),
    ];
    for ((x, y), bloque) in casos {
        let b = con_pixeles(200, 130, &[(x, y)]);
        let esperado: Modelo = [bloque].into_iter().collect();
        comprueba(&b, &esperado, &format!("píxel ({x}, {y})"));
        assert!(tiene(&b, bloque.0, bloque.1));
    }
    // Varios a la vez, repetidos: los bloques sin repetir.
    let b = con_pixeles(200, 130, &[(63, 0), (64, 0), (64, 63), (63, 64), (0, 0)]);
    let esperado: Modelo = [(0, 0), (1, 0), (0, 1)].into_iter().collect();
    comprueba(&b, &esperado, "varios");
}

#[test]
fn fila_b2_add_pixel_fuera_de_la_imagen_se_ignora() {
    for (x, y) in [(200, 0), (0, 130), (200, 130), (u32::MAX, 0), (0, u32::MAX)] {
        let b = con_pixeles(200, 130, &[(x, y)]);
        assert!(b.is_empty(), "píxel ({x}, {y}) fuera");
        assert_eq!(b.len(), 0);
    }
    // Un píxel fuera no quita ni cambia los de dentro.
    let b = con_pixeles(200, 130, &[(5, 5), (200, 5), (5, 130)]);
    let esperado: Modelo = [(0, 0)].into_iter().collect();
    comprueba(&b, &esperado, "dentro y fuera");
}

#[test]
fn fila_b2_add_rect_da_los_bloques_que_cortan_el_rectangulo() {
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
        comprueba(
            &b,
            &modelo_de_rect(x, y, w, h, 200, 130),
            &format!("add_rect({x}, {y}, {w}, {h})"),
        );
    }
}

#[test]
fn fila_b2_add_rect_con_valores_extremos_no_desborda() {
    let (iw, ih) = (200, 130);
    let extremos: [(i64, i64, u64, u64); 6] = [
        (i64::MIN, i64::MIN, u64::MAX, u64::MAX),
        (i64::MAX, i64::MAX, u64::MAX, u64::MAX),
        (0, 0, u64::MAX, u64::MAX),
        (i64::MIN, 0, 1, 1),
        (i64::MAX, 0, 1, 1),
        (-1, -1, u64::MAX, 3),
    ];
    for (x, y, w, h) in extremos {
        let mut b = BlockSet::new(iw, ih);
        b.add_rect(x, y, w, h);
        comprueba(
            &b,
            &modelo_de_rect(x, y, w, h, iw, ih),
            &format!("add_rect({x}, {y}, {w}, {h})"),
        );
    }
}

#[test]
fn fila_b2_una_imagen_de_1x1() {
    let mut b = BlockSet::new(1, 1);
    assert!(b.is_empty());
    b.add_pixel(1, 0);
    b.add_pixel(0, 1);
    assert!(b.is_empty(), "fuera de la imagen");
    b.add_pixel(0, 0);
    assert_eq!(b.len(), 1);
    assert!(tiene(&b, 0, 0));
    assert!(!tiene(&b, 1, 0));
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
fn fila_b2_una_imagen_de_8192_por_8192() {
    let mut b = BlockSet::new(8192, 8192);
    assert!(b.is_empty());
    assert_eq!(BlockSet::full(8192, 8192).len(), 128 * 128);
    b.add_pixel(8191, 8191);
    b.add_pixel(0, 0);
    b.add_pixel(8192, 8191);
    assert_eq!(b.len(), 2);
    assert!(tiene(&b, 127, 127) && tiene(&b, 0, 0));
    assert!(!tiene(&b, 126, 127));
    // La imagen entera como rectángulo es la imagen llena.
    let mut c = BlockSet::new(8192, 8192);
    c.add_rect(0, 0, 8192, 8192);
    assert_eq!(c, BlockSet::full(8192, 8192));
    // `runs` por baldosa de 2048: una baldosa llena da una serie por fila de bloques.
    let baldosa = TileRect {
        x: 2048,
        y: 4096,
        width: 2048,
        height: 2048,
    };
    let series = c.runs(baldosa);
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
    assert!(b.runs(baldosa).is_empty());
    let ultima = TileRect {
        x: 6144,
        y: 6144,
        width: 2048,
        height: 2048,
    };
    assert_eq!(
        b.runs(ultima),
        vec![TileRect {
            x: 8128,
            y: 8128,
            width: 64,
            height: 64
        }]
    );
}

/// Conjuntos de prueba sobre una imagen de 300×200 (5×4 bloques).
fn conjuntos_de_prueba() -> Vec<BlockSet> {
    let (w, h) = (300, 200);
    let mut v = vec![
        BlockSet::new(w, h),
        BlockSet::full(w, h),
        con_pixeles(w, h, &[(0, 0)]),
        con_pixeles(w, h, &[(70, 5), (130, 5), (299, 199)]),
        con_pixeles(w, h, &[(63, 63), (64, 64), (64, 63), (63, 64)]),
        con_pixeles(
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
fn fila_b2_union_es_conmutativa_asociativa_idempotente_y_con_neutro() {
    let conjuntos = conjuntos_de_prueba();
    let union = |a: &BlockSet, b: &BlockSet| {
        let mut u = a.clone();
        u.union(b);
        u
    };
    for a in &conjuntos {
        // Idempotente y con neutro.
        assert_eq!(union(a, a), *a);
        assert_eq!(union(a, &BlockSet::new(300, 200)), *a);
        assert_eq!(union(&BlockSet::new(300, 200), a), *a);
        assert_eq!(
            union(a, &BlockSet::full(300, 200)),
            BlockSet::full(300, 200)
        );
        for b in &conjuntos {
            assert_eq!(union(a, b), union(b, a), "conmutativa");
            // Es la unión de los modelos.
            let esperado: Modelo = modelo_de(a).union(&modelo_de(b)).copied().collect();
            comprueba(&union(a, b), &esperado, "unión de modelos");
            for c in &conjuntos {
                assert_eq!(union(&union(a, b), c), union(a, &union(b, c)), "asociativa");
            }
        }
    }
}

#[test]
fn fila_b2_union_con_otro_tamano_no_hace_nada() {
    let mut a = con_pixeles(300, 200, &[(5, 5)]);
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
fn fila_b2_clip_deja_los_bloques_que_cortan_el_rectangulo_y_es_idempotente() {
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
    for original in conjuntos_de_prueba() {
        for r in rects {
            let mut b = original.clone();
            b.clip(r);
            let esperado: Modelo = modelo_de(&original)
                .into_iter()
                .filter(|&(bx, by)| tiles_support::corta(bx, by, r))
                .collect();
            comprueba(&b, &esperado, &format!("clip {r:?}"));
            assert_eq!(b.size(), original.size(), "clip no cambia el tamaño");
            let una_vez = b.clone();
            b.clip(r);
            assert_eq!(b, una_vez, "clip es idempotente: {r:?}");
        }
    }
}

#[test]
fn fila_b2_runs_fusiona_los_bloques_contiguos_de_una_fila() {
    // Bloques (0,0), (1,0), (3,0) y (1,1) de una imagen de 300×200.
    let b = con_pixeles(300, 200, &[(0, 0), (64, 0), (192, 0), (64, 64)]);
    let toda = TileRect {
        x: 0,
        y: 0,
        width: 300,
        height: 200,
    };
    assert_eq!(
        b.runs(toda),
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
        c.runs(toda),
        vec![TileRect {
            x: 256,
            y: 192,
            width: 44,
            height: 8
        }]
    );
    // Sin ningún bloque, nada; con todos, una serie por fila.
    assert!(BlockSet::new(300, 200).runs(toda).is_empty());
    assert_eq!(BlockSet::full(300, 200).runs(toda).len(), 4);
}

#[test]
fn fila_b2_runs_recorta_a_un_rectangulo_interior_y_cubre_exactamente_los_bloques_sucios() {
    for original in conjuntos_de_prueba() {
        let modelo = modelo_de(&original);
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
            assert_eq!(series, runs_esperados(&modelo, r), "runs de {r:?}");
            // Cada serie está dentro del rectángulo.
            for s in &series {
                assert!(s.x >= r.x && s.y >= r.y, "{s:?} fuera de {r:?}");
                assert!(s.x + s.width <= r.x + r.width && s.y + s.height <= r.y + r.height);
                assert!(s.width >= 1 && s.height >= 1);
            }
            // Y cubren exactamente los píxeles de los bloques sucios que cortan r, recortados a r.
            let mut cubiertos = HashSet::new();
            for s in &series {
                for y in s.y..s.y + s.height {
                    for x in s.x..s.x + s.width {
                        assert!(cubiertos.insert((x, y)), "serie solapada en ({x}, {y})");
                    }
                }
            }
            let mut esperados = HashSet::new();
            for y in r.y..r.y + r.height {
                for x in r.x..r.x + r.width {
                    if modelo.contains(&(x / B, y / B)) {
                        esperados.insert((x, y));
                    }
                }
            }
            assert_eq!(cubiertos, esperados, "cobertura de {r:?}");
        }
    }
}

// ---------------------------------------------------------------------------------------------
// B4 · Baldosa desde las filas
// ---------------------------------------------------------------------------------------------

const ANCHO_B4: u32 = 70;
const ALTO_B4: u32 = 66;

fn rect(x: u32, y: u32, width: u32, height: u32) -> TileRect {
    TileRect {
        x,
        y,
        width,
        height,
    }
}

/// El rectángulo recortado a la imagen (`None` si no la corta).
fn recortado(r: TileRect, ancho: u32, alto: u32) -> Option<TileRect> {
    let (x1, y1) = (
        (r.x.saturating_add(r.width)).min(ancho),
        (r.y.saturating_add(r.height)).min(alto),
    );
    (r.x < x1 && r.y < y1).then(|| rect(r.x, r.y, x1 - r.x, y1 - r.y))
}

fn rectangulos_b4() -> Vec<TileRect> {
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

fn imagen_b4(modo: Mode) -> Image {
    imagen_de(modo, ANCHO_B4, ALTO_B4, 3)
}

#[test]
fn fila_b4_tile_pixels_es_el_recorte_de_color_image_from_rgba_unmultiplied_en_los_cuatro_modos() {
    for modo in MODOS {
        let imagen = imagen_b4(modo);
        let referencia = ColorImage::from_rgba_unmultiplied(
            [ANCHO_B4 as usize, ALTO_B4 as usize],
            &to_rgba8(&imagen),
        );
        for r in rectangulos_b4() {
            let obtenido = tile_pixels(&imagen, r);
            let esperado = recorte(&referencia.pixels, ANCHO_B4 as usize, r);
            assert_eq!(
                obtenido.len(),
                (r.width * r.height) as usize,
                "{modo:?} {r:?}"
            );
            assert_eq!(obtenido, esperado, "{modo:?} {r:?}");
        }
    }
}

#[test]
fn fila_b4_los_cuatro_alfas_y_el_indexado_con_transparente_y_duplicadas_aparecen_y_valen() {
    // La imagen de prueba lleva los alfas 0, 1, 128 y 255 en RGBA, indexado y gris con alfa.
    for modo in [Mode::Rgba, Mode::GrayAlpha, Mode::Indexed] {
        let imagen = imagen_b4(modo);
        let bytes = to_rgba8(&imagen);
        let alfas: HashSet<u8> = bytes.as_chunks::<4>().0.iter().map(|c| c[3]).collect();
        for alfa in [0u8, 1, 128, 255] {
            assert!(alfas.contains(&alfa), "{modo:?}: falta el alfa {alfa}");
        }
        let obtenido = tile_pixels(&imagen, rect(0, 0, ANCHO_B4, ALTO_B4));
        assert_eq!(obtenido, esperado(&imagen), "{modo:?}");
        // Alfa 0 → transparente (premultiplicado).
        for (c, t) in bytes.as_chunks::<4>().0.iter().zip(&obtenido) {
            if c[3] == 0 {
                assert_eq!(*t, Color32::TRANSPARENT, "{modo:?}: alfa 0");
            }
        }
    }
    // Indexado: el índice transparente (7) guarda blanco opaco pero se ve con alfa 0, y las entradas
    // 2 y 4 (duplicadas) dan el mismo texel.
    let imagen = imagen_b4(Mode::Indexed);
    let pixeles = tile_pixels(&imagen, rect(0, 0, ANCHO_B4, ALTO_B4));
    let mut vistos = [None; 8];
    for y in 0..ALTO_B4 {
        for x in 0..ANCHO_B4 {
            if let Ok(Pixel::Indexed(i)) = imagen.get_pixel(x, y) {
                let t = pixeles[(y * ANCHO_B4 + x) as usize];
                match vistos[usize::from(i)] {
                    None => vistos[usize::from(i)] = Some(t),
                    Some(antes) => assert_eq!(antes, t, "la entrada {i} siempre da el mismo texel"),
                }
            }
        }
    }
    assert_eq!(vistos[7], Some(Color32::TRANSPARENT), "índice transparente");
    assert_eq!(vistos[2], vistos[4], "entradas duplicadas");
    assert_eq!(vistos[5], Some(Color32::TRANSPARENT), "entrada de alfa 0");
}

#[test]
fn fila_b4_un_rectangulo_que_se_sale_se_recorta() {
    for modo in MODOS {
        let imagen = imagen_b4(modo);
        let completa = esperado(&imagen);
        for r in [
            rect(60, 60, 30, 30),
            rect(69, 0, 100, 3),
            rect(0, 65, 3, 100),
            rect(0, 0, 1000, 1000),
        ] {
            let c = recortado(r, ANCHO_B4, ALTO_B4).expect("corta la imagen");
            assert_eq!(
                tile_pixels(&imagen, r),
                recorte(&completa, ANCHO_B4 as usize, c),
                "{modo:?} {r:?}"
            );
        }
        for r in [rect(70, 0, 5, 5), rect(0, 66, 5, 5), rect(500, 500, 3, 3)] {
            assert!(tile_pixels(&imagen, r).is_empty(), "{modo:?} {r:?}");
        }
    }
}

#[test]
fn fila_b4_tile_pixels_with_entry_es_el_recorte_de_to_rgba8_with_entry() {
    let rojo = Rgba::opaque(250, 10, 10);
    let semi = Rgba::new(10, 250, 10, 128);
    for modo in MODOS {
        let imagen = imagen_b4(modo);
        // Entradas: una normal, la transparente (7), una duplicada (4), una de alfa 0 (5) y una fuera
        // de la paleta (200); en los modos sin paleta no cambia nada.
        for (index, color) in [
            (2u8, rojo),
            (7, rojo),
            (4, semi),
            (5, rojo),
            (200, rojo),
            (1, Rgba::TRANSPARENT),
        ] {
            let referencia = ColorImage::from_rgba_unmultiplied(
                [ANCHO_B4 as usize, ALTO_B4 as usize],
                &to_rgba8_with_entry(&imagen, index, color),
            );
            for r in rectangulos_b4() {
                assert_eq!(
                    tile_pixels_with_entry(&imagen, r, index, color),
                    recorte(&referencia.pixels, ANCHO_B4 as usize, r),
                    "{modo:?} entrada {index} {color:?} {r:?}"
                );
            }
        }
    }
    // En indexado la entrada transparente conserva alfa 0 con el RGB nuevo (a efectos del texel:
    // transparente) y las duplicadas no se tocan.
    let imagen = imagen_b4(Mode::Indexed);
    let todos = tile_pixels_with_entry(&imagen, rect(0, 0, ANCHO_B4, ALTO_B4), 7, rojo);
    for y in 0..ALTO_B4 {
        for x in 0..ANCHO_B4 {
            if imagen.get_pixel(x, y) == Ok(Pixel::Indexed(7)) {
                assert_eq!(todos[(y * ANCHO_B4 + x) as usize].a(), 0, "({x}, {y})");
            }
        }
    }
}

/// Asignador falso: da memoria las primeras `buenas` veces y después falla.
struct Falla {
    buenas: usize,
    llamadas: usize,
}

impl TileAlloc for Falla {
    fn pixels(&mut self, len: usize) -> Option<Vec<Color32>> {
        self.llamadas += 1;
        (self.llamadas <= self.buenas).then(|| Vec::with_capacity(len))
    }
}

#[test]
fn fila_b4_try_tile_pixels_da_lo_mismo_que_tile_pixels_y_con_un_fallo_out_of_memory() {
    let imagen = imagen_b4(Mode::Rgba);
    let r = rect(5, 7, 20, 13);
    let mut bien = Falla {
        buenas: usize::MAX,
        llamadas: 0,
    };
    let con_asignador = try_tile_pixels(&imagen, r, &mut bien).expect("hay memoria");
    assert_eq!(con_asignador, tile_pixels(&imagen, r));
    assert_eq!(bien.llamadas, 1, "una reserva por baldosa");
    let mut mal = Falla {
        buenas: 0,
        llamadas: 0,
    };
    assert_eq!(
        try_tile_pixels(&imagen, r, &mut mal),
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
    fn propiedad_q1_las_baldosas_cubren_la_imagen_y_la_pantalla_cubre_image_rect(
        ancho in 1u32..=20_000,
        alto in 1u32..=20_000,
        lado in (1usize..=32).prop_map(|k| k * 64),
        indice_nivel in 0usize..70,
        ax in -4000i64..4000,
        ay in -4000i64..4000,
        vw in 0u32..2000,
        vh in 0u32..2000,
        muestras in proptest::collection::vec(any::<u32>(), 8),
    ) {
        let imagen = Size::new(ancho, alto);
        let t = Tiling::new(imagen, lado);
        let cols = (ancho as usize).div_ceil(lado);
        let rows = (alto as usize).div_ceil(lado);
        prop_assert_eq!(columnas(&t), cols);
        prop_assert_eq!(filas(&t), rows);
        prop_assert_eq!(numero(&t), cols * rows);

        // Columnas (la fila 0) y filas (la columna 0): empiezan donde acaba la anterior, miden T
        // salvo la última, que mide lo que queda.
        let mut x_esperada = 0u32;
        for c in 0..cols {
            let r = rect_de(&t, c);
            prop_assert_eq!(r.x, x_esperada);
            prop_assert!(r.width >= 1 && r.width as usize <= lado);
            if c + 1 < cols {
                prop_assert_eq!(r.width as usize, lado, "solo las del borde son menores");
            }
            x_esperada += r.width;
        }
        prop_assert_eq!(x_esperada, ancho);
        let mut y_esperada = 0u32;
        for f in 0..rows {
            let r = rect_de(&t, f * cols);
            prop_assert_eq!(r.y, y_esperada);
            prop_assert!(r.height >= 1 && r.height as usize <= lado);
            if f + 1 < rows {
                prop_assert_eq!(r.height as usize, lado);
            }
            y_esperada += r.height;
        }
        prop_assert_eq!(y_esperada, alto);

        // Algunas baldosas sueltas: el rectángulo es el de su columna y su fila.
        for m in &muestras {
            let i = *m as usize % (cols * rows);
            let (col, row) = (i % cols, i / cols);
            let (c, f) = (rect_de(&t, col), rect_de(&t, row * cols));
            prop_assert_eq!(rect_de(&t, i), rect(c.x, f.y, c.width, f.height));
        }

        // Pantalla: a cualquier zoom y origen entero.
        let z = niveles()[indice_nivel];
        let v = vista(Size::new(vw, vh), imagen, z, (ax, ay));
        let marco = v.image_rect(imagen);
        prop_assert_eq!(marco.width, ext(z, ancho));
        prop_assert_eq!(marco.height, ext(z, alto));
        let mut x_pantalla = marco.x;
        for c in 0..cols {
            let p = pantalla_de(&t, c, &v);
            prop_assert_eq!(p.x, x_pantalla);
            prop_assert_eq!(p, pantalla_esperada(&v, z, rect_de(&t, c)));
            prop_assert!(p.width >= 0);
            x_pantalla += p.width;
        }
        prop_assert_eq!(x_pantalla, marco.x + marco.width, "sin huecos ni sobrantes en x");
        let mut y_pantalla = marco.y;
        for f in 0..rows {
            let p = pantalla_de(&t, f * cols, &v);
            prop_assert_eq!(p.y, y_pantalla);
            prop_assert!(p.height >= 0);
            y_pantalla += p.height;
        }
        prop_assert_eq!(y_pantalla, marco.y + marco.height, "sin huecos ni sobrantes en y");
        for m in &muestras {
            let i = *m as usize % (cols * rows);
            let (col, row) = (i % cols, i / cols);
            let (a, b) = (pantalla_de(&t, col, &v), pantalla_de(&t, row * cols, &v));
            prop_assert_eq!(
                pantalla_de(&t, i, &v),
                ScreenRect { x: a.x, y: b.y, width: a.width, height: b.height }
            );
        }
    }
}

// ---------------------------------------------------------------------------------------------
// Q3 · Región
// ---------------------------------------------------------------------------------------------

/// Los píxeles cuyo `to_rgba8` cambia entre `antes` y `despues` están todos en `sucios`.
fn contiene_lo_que_cambia(antes: &Image, despues: &Image, sucios: &BlockSet) -> Result<(), String> {
    if (antes.width(), antes.height()) != (despues.width(), despues.height()) {
        return Err("el tamaño cambió".to_string());
    }
    let (a, d) = (to_rgba8(antes), to_rgba8(despues));
    let ancho = antes.width() as usize;
    for (i, (pa, pd)) in a
        .as_chunks::<4>()
        .0
        .iter()
        .zip(d.as_chunks::<4>().0.iter())
        .enumerate()
    {
        if pa != pd {
            let (x, y) = ((i % ancho) as u32, (i / ancho) as u32);
            if !sucios.contains(x / B, y / B) {
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
    fn propiedad_q3_dirty_of_y_el_registro_contienen_todo_lo_que_cambia(
        modo in 0u8..4,
        ancho in 1u32..=140,
        alto in 1u32..=140,
        celda in 1u32..=9,
        pasos in proptest::collection::vec(semilla(), 1..=8),
    ) {
        let imagen = imagen_de(MODOS[usize::from(modo)], ancho, alto, celda);
        let tamano = Size::new(ancho, alto);
        let mut doc = Document::new(imagen);
        let mut registro = ChangeLog::new();
        for (n, s) in pasos.into_iter().enumerate() {
            let id = ImageId::from_raw(0);
            let antes = doc.image(id).expect("imagen").clone();
            match paso_de(&doc, s) {
                Paso::Nada => {}
                Paso::Aplica(comando) => {
                    let region = dirty_of(&comando, tamano);
                    if let Ok(true) = doc.apply(comando.clone()) {
                        registro.record(doc.state_id(), region.clone());
                        let despues = doc.image(id).expect("imagen");
                        let ok = contiene_lo_que_cambia(&antes, despues, &region);
                        prop_assert!(ok.is_ok(), "paso {n} {comando:?}: {:?}", ok.err());
                    }
                }
                Paso::Deshacer => {
                    let desde = doc.state_id();
                    if doc.undo().is_ok() {
                        let region = registro.get(desde).cloned().unwrap_or_else(|| BlockSet::full(ancho, alto));
                        let despues = doc.image(id).expect("imagen");
                        let ok = contiene_lo_que_cambia(&antes, despues, &region);
                        prop_assert!(ok.is_ok(), "paso {n} deshacer: {:?}", ok.err());
                    }
                }
                Paso::Rehacer => {
                    if doc.redo().is_ok() {
                        let hacia = doc.state_id();
                        let region = registro.get(hacia).cloned().unwrap_or_else(|| BlockSet::full(ancho, alto));
                        let despues = doc.image(id).expect("imagen");
                        let ok = contiene_lo_que_cambia(&antes, despues, &region);
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
fn propiedad_q3_casos_fijos_de_los_bordes_de_bloque() {
    let id = |_: &Document| ImageId::from_raw(0);
    let mut doc = Document::new(imagen_de(Mode::Rgba, 200, 130, 5));
    let tamano = Size::new(200, 130);
    let pixel = Pixel::Rgba(Rgba::opaque(1, 2, 3));
    let comandos = vec![
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
    for comando in comandos {
        let antes = doc.image(id(&doc)).expect("imagen").clone();
        let region = dirty_of(&comando, tamano);
        assert_eq!(doc.apply(comando), Ok(true));
        let despues = doc.image(id(&doc)).expect("imagen");
        assert_eq!(contiene_lo_que_cambia(&antes, despues, &region), Ok(()));
    }
}
