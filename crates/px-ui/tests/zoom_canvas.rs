//! Escalera de zoom en el lienzo, menú y columna (specs/hito-2-zoom-baldosas.md, decisiones 2, 3 y 4;
//! unidad 14a-1, filas A6 a A9 y la parte de banco de la propiedad P1). La parte pura (A1 a A5 y P1
//! a P4) está en `zoom_ladder.rs`.
//!
//! Banco de egui sin ventana (`Context::run_ui` con `RawInput` sintético, sin GPU ni capturas), a
//! las cinco escalas (`ppp` 1,0, 1,25, 1,5, 2,0 y 3,0). Se afirma lo **observable**: el nivel de
//! zoom, `hover`, los eventos de trazo y el color de cada píxel físico (el lienzo teselado por egui
//! y rasterizado a mano, como en `checker_canvas.rs`), no el mecanismo interno.
//!
//! - A6: la rueda baja 1/2, 1/4, …, 1/64 y satura, sube hasta ×64 y satura, con el píxel bajo el
//!   puntero que fija la regla del ancla; `hover` y `StrokeBegin` en cada píxel de la rejilla y uno
//!   fuera; `Home`, `+`/`-` y `request_view`; el rectángulo `image_rect` mide `extent/ppp`; la malla
//!   de la imagen es un cuadrilátero de `extent` píxeles físicos con el uv desplazado de la
//!   decisión 3 (de 0 a 1 con zoom ≥ ×1).
//! - A7: un trazo a 1/4 va por la rejilla (múltiplos de 4), con un solo paso de deshacer.
//! - A8: el menú y la columna con la escalera (`enabled`, `zoom_label`) y el rótulo cabe en la
//!   columna a las cinco escalas con la fuente real y `MissingGlyphPolicy::Panic`.
//! - A9: la superposición del trazo a zoom < 1, reexpresada para la 14a-2 (la superposición es
//!   parte de la textura de la baldosa, `Canvas::set_overlay`, no hay formas): se **ven** solo los
//!   píxeles con `x` e `y` múltiplos de `K` (los demás no tienen píxel de pantalla propio), cada
//!   uno de 1×1 píxel físico y con el color del oráculo de la 17b, muestreando la **textura
//!   compuesta** de la baldosa (los `textures_delta` de cada fotograma); la superposición no añade
//!   ninguna forma al fotograma, y ninguna forma es de área cero, con los cuatro fondos y con
//!   `set_checker_mesh(true)` (lo que activa `PIXOKO_CHECKER_MESH=1` en la app).
//! - P1 (banco): `hover` y `StrokeBegin` de un puntero cualquiera son el oráculo con `round(v·ppp)`.
//!
//! Supuestos que la spec no fija del todo:
//! - A9 no usa el contenido de la imagen azul de `overlay_small_zoom.rs`, sino un degradado con un
//!   color distinto por posición: así el muestreo de la imagen (el texel `s·K` en el píxel de
//!   pantalla `s`) es observable. El oráculo es el píxel `s·K` de la imagen **que construye la
//!   prueba**, no el código bajo prueba; la imagen de A9 (256×192) cabe en una sola baldosa.
//! - Los píxeles pendientes de A9 son, por cada `K`, las celdas `K×K` de la rejilla (filas 1 y 2 y
//!   hasta 12 columnas) rellenas **enteras**: solo la esquina de cada celda se debe ver.
//! - El rótulo de la columna (A8) cabe si su texto pintado es entero (sin recorte) y termina dentro
//!   de la columna (`column_width_u(16) - TOOLBAR_MARGIN_U` `u` desde su borde izquierdo).
//! - El título de la ventana y la barra de estado son de `px-app` (usan `Zoom::to_string`): aquí solo
//!   se comprueba que `zoom_label` es `Zoom {Display}`.

use px_core::ImageId;
use std::collections::{HashMap, HashSet};

use egui::epaint::{ClippedPrimitive, ClippedShape, Primitive, Shape, Vertex};
use egui::{
    Color32, Context, Event, Key, Modifiers, Pos2, RawInput, Rect, TextureId, TexturesDelta, Vec2,
    pos2, vec2,
};
use proptest::prelude::*;
use px_core::{Document, Image, Mode, Pixel, Rgba};
use px_ui::actions::{AppAction, Locks, Origin, UiState, enabled, is_blocked};
use px_ui::background::{CanvasBackground, Fill, background_fill};
use px_ui::canvas::{BACKGROUND, Canvas, CanvasEvent, CanvasInfo, CanvasInput};
use px_ui::checker::shade_at;
use px_ui::chrome::{COLUMN_DEFAULT_COLS, TOOLBAR_MARGIN_U, column_width_u};
use px_ui::palette_grid::{CHECK_A, CHECK_B};
use px_ui::stroke::Stroke;
use px_ui::text::text_scale;
use px_ui::tool::Tool;
use px_ui::toolbar::zoom_label;
use px_ui::view::{Size, ViewOp, Zoom};

mod common;
use common::toolbox::toolbox_2;
use common::zones::toolbox_single;
use common::{
    PPPS, button_2, context, frame, in_points, key_2, move_pointer, n_of, painted,
    tolerance_of_egui, wheel_line,
};

/// Pantalla de A6, A7 y P1: la imagen de 64×48 ocupa el centro.
const TEST_SCREEN: Vec2 = vec2(800.0, 600.0);
/// Pantalla de A9: a 1/2 la imagen de 256×192 mide 128×96 píxeles físicos y cabe a `ppp` 1.
const SCREEN_A_9: Vec2 = vec2(160.0, 120.0);
const TOLERANCE: f32 = 0.02;
const DESPLAZO_X: f64 = 0.0013;
const DESPLAZO_Y: f64 = 0.0007;

const RED: Rgba = Rgba::opaque(255, 0, 0);
const TRANSPARENT: Rgba = Rgba::new(0, 0, 0, 0);
const RED_MIDDLE: Rgba = Rgba::new(255, 0, 0, 128);

const BACKGROUNDS: [CanvasBackground; 4] = [
    CanvasBackground::Checker,
    CanvasBackground::Black,
    CanvasBackground::Gray,
    CanvasBackground::White,
];

/// Un píxel pendiente: posición y color (el que tendrá en la imagen).
type TestPending = (u32, u32, Rgba);

// ---------------------------------------------------------------------------------------------
// Oráculo (fracciones e i128, sin llamar a las funciones bajo prueba)
// ---------------------------------------------------------------------------------------------

type Fraction = (i128, i128);

/// Los 70 niveles de la decisión 2, de menor a mayor.
fn ladder() -> Vec<Fraction> {
    let mut levels: Vec<Fraction> = [64, 32, 16, 8, 4, 2].iter().map(|&k| (1, k)).collect();
    levels.extend((1..=64).map(|n| (n, 1)));
    levels
}

fn label((num, den): Fraction) -> String {
    if den == 1 {
        format!("×{num}")
    } else {
        format!("×{num}/{den}")
    }
}

fn floor(n: i128, d: i128) -> i128 {
    let q = n / d;
    if n % d != 0 && n < 0 { q - 1 } else { q }
}

fn ceiling(n: i128, d: i128) -> i128 {
    -floor(-n, d)
}

fn o_extent(f: Fraction, n: i64) -> i64 {
    ceiling(i128::from(n) * f.0, f.1) as i64
}

/// Píxel de imagen bajo el punto `p` de un eje, o `None`.
fn o_axis(f: Fraction, p: i64, source_2: i64, dim: u32) -> Option<u32> {
    let d = i128::from(p) - i128::from(source_2);
    if d < 0 {
        return None;
    }
    let index_2 = floor(d * f.1, f.0);
    (index_2 < i128::from(dim)).then_some(index_2 as u32)
}

/// Lo mismo, con signo y sin recortar a la imagen (lo que emite `StrokeBegin`).
fn or_axis_i32(f: Fraction, p: i64, source_2: i64) -> i32 {
    let d = i128::from(p) - i128::from(source_2);
    floor(d * f.1, f.0).clamp(i128::from(i32::MIN), i128::from(i32::MAX)) as i32
}

fn o_hover(
    f: Fraction,
    p: (i64, i64),
    source_2: (i64, i64),
    tam: (u32, u32),
) -> Option<(u32, u32)> {
    Some((
        o_axis(f, p.0, source_2.0, tam.0)?,
        o_axis(f, p.1, source_2.1, tam.1)?,
    ))
}

fn o_stroke(f: Fraction, p: (i64, i64), source_2: (i64, i64)) -> (i32, i32) {
    (
        or_axis_i32(f, p.0, source_2.0),
        or_axis_i32(f, p.1, source_2.1),
    )
}

/// Índice del mayor nivel con `extent ≤ canvas_2` en los dos ejes (0 si ninguno cabe).
fn o_better_level(viewport: Size, image: (u32, u32)) -> usize {
    let levels = ladder();
    (0..levels.len())
        .rev()
        .find(|&i| {
            o_extent(levels[i], i64::from(image.0)) <= i64::from(viewport.width)
                && o_extent(levels[i], i64::from(image.1)) <= i64::from(viewport.height)
        })
        .unwrap_or(0)
}

/// Granularidad del nivel `index_2`: su `K` si es fraccionario y 1 si no.
fn grid(index_2: usize) -> i32 {
    if index_2 < 6 { 1 << (6 - index_2) } else { 1 }
}

/// El píxel bajo el ancla tras pasar a un nivel con la granularidad `g` (decisión 4).
fn goes_up_to_the_grid(pixel: (u32, u32), g: i32) -> (u32, u32) {
    let g = g as u32;
    (pixel.0 / g * g, pixel.1 / g * g)
}

// ---------------------------------------------------------------------------------------------
// Imagen de la prueba
// ---------------------------------------------------------------------------------------------

#[derive(Clone)]
struct TestImage {
    width_2: u32,
    height_2: u32,
    pixels_2: Vec<Rgba>,
}

impl TestImage {
    /// Cada píxel tiene un color distinto según su posición (opaco).
    fn gradient(width_2: u32, height_2: u32) -> TestImage {
        let mut pixels_2 = Vec::new();
        for y in 0..height_2 {
            for x in 0..width_2 {
                pixels_2.push(Rgba::opaque(
                    (x % 256) as u8,
                    (y % 256) as u8,
                    ((x * 5 + y * 3) % 251) as u8,
                ));
            }
        }
        TestImage {
            width_2,
            height_2,
            pixels_2,
        }
    }

    fn pixel(&self, x: u32, y: u32) -> Rgba {
        self.pixels_2[(y * self.width_2 + x) as usize]
    }

    fn image(&self) -> Image {
        let mut image = Image::new(self.width_2, self.height_2, Mode::Rgba).expect("imagen válida");
        for y in 0..self.height_2 {
            for x in 0..self.width_2 {
                image
                    .set_pixel(x, y, Pixel::Rgba(self.pixel(x, y)))
                    .expect("dentro");
            }
        }
        image
    }

    fn texels(&self) -> Vec<Color32> {
        self.pixels_2
            .iter()
            .map(|c| Color32::from_rgba_unmultiplied(c.r, c.g, c.b, c.a))
            .collect()
    }
}

fn image_64x48() -> Image {
    Image::new(64, 48, Mode::Rgba).expect("imagen válida")
}

// ---------------------------------------------------------------------------------------------
// Color
// ---------------------------------------------------------------------------------------------

/// Lo que se ve: color premultiplicado, como egui.
#[derive(Clone, Copy, Debug)]
struct Seen {
    r: f32,
    g: f32,
    b: f32,
    a: f32,
}

impl Seen {
    const EMPTY: Seen = Seen {
        r: 0.0,
        g: 0.0,
        b: 0.0,
        a: 0.0,
    };

    fn of_color(c: Color32) -> Seen {
        Seen {
            r: f32::from(c.r()) / 255.0,
            g: f32::from(c.g()) / 255.0,
            b: f32::from(c.b()) / 255.0,
            a: f32::from(c.a()) / 255.0,
        }
    }

    /// `src` por encima de `self` (alfa premultiplicado).
    fn below_of(self, src: Seen) -> Seen {
        let k = 1.0 - src.a;
        Seen {
            r: src.r + self.r * k,
            g: src.g + self.g * k,
            b: src.b + self.b * k,
            a: src.a + self.a * k,
        }
    }

    fn per(self, other_2: Seen) -> Seen {
        Seen {
            r: self.r * other_2.r,
            g: self.g * other_2.g,
            b: self.b * other_2.b,
            a: self.a * other_2.a,
        }
    }

    fn near(&self, other_2: Seen, tolerance: f32) -> bool {
        (self.r - other_2.r).abs() <= tolerance
            && (self.g - other_2.g).abs() <= tolerance
            && (self.b - other_2.b).abs() <= tolerance
            && (self.a - other_2.a).abs() <= tolerance
    }
}

fn gray_2(c: Rgba) -> Color32 {
    Color32::from_rgb(c.r, c.g, c.b)
}

/// Color del fondo elegido bajo la transparencia en el píxel físico `(px, py)`: el oráculo de la
/// 17b (`background_fill` y `shade_at`).
fn background_in(
    background_2: CanvasBackground,
    px: i64,
    py: i64,
    source_2: (i64, i64),
    cell: i64,
) -> Color32 {
    match background_fill(background_2) {
        Fill::Checker => gray_2(shade_at(px, py, source_2, cell).color()),
        Fill::Solid(color) => gray_2(color),
    }
}

/// Un color con alfa directo por encima de un fondo opaco.
fn composes(background_2: Color32, color: Rgba) -> Seen {
    let above = Color32::from_rgba_unmultiplied(color.r, color.g, color.b, color.a);
    Seen::of_color(background_2).below_of(Seen::of_color(above))
}

// ---------------------------------------------------------------------------------------------
// Rasterizador a mano (el de `checker_canvas.rs`)
// ---------------------------------------------------------------------------------------------

fn barycentric(p: (f64, f64), a: (f64, f64), b: (f64, f64), c: (f64, f64)) -> Option<[f64; 3]> {
    let area = (b.0 - a.0) * (c.1 - a.1) - (c.0 - a.0) * (b.1 - a.1);
    if area.abs() < 1e-12 {
        return None;
    }
    let w0 = ((b.0 - p.0) * (c.1 - p.1) - (c.0 - p.0) * (b.1 - p.1)) / area;
    let w1 = ((c.0 - p.0) * (a.1 - p.1) - (a.0 - p.0) * (c.1 - p.1)) / area;
    let w2 = 1.0 - w0 - w1;
    (w0 >= 0.0 && w1 >= 0.0 && w2 >= 0.0).then_some([w0, w1, w2])
}

/// Texturas conocidas para el muestreo.
struct Textures {
    image_2: TextureId,
    width_2: usize,
    height_2: usize,
    texels: Vec<Color32>,
    checker_2: Option<TextureId>,
}

/// Texel de la textura 2×2 del damero (por filas `A, B, B, A`) con `NEAREST` y envoltura `Repeat`.
fn texel_of_the_checker(u: f64, v: f64) -> Seen {
    let tx = ((u * 2.0).floor() as i64).rem_euclid(2);
    let ty = ((v * 2.0).floor() as i64).rem_euclid(2);
    let c = if (tx + ty) % 2 == 0 { CHECK_A } else { CHECK_B };
    Seen::of_color(gray_2(c))
}

struct Buffer {
    width_2: usize,
    height_2: usize,
    pixels_2: Vec<Seen>,
}

impl Buffer {
    fn at(&self, x: i64, y: i64) -> Seen {
        assert!(
            x >= 0 && y >= 0 && (x as usize) < self.width_2 && (y as usize) < self.height_2,
            "({x}, {y}) fuera del búfer de {}×{}",
            self.width_2,
            self.height_2
        );
        self.pixels_2[y as usize * self.width_2 + x as usize]
    }
}

/// Pinta `primitives`, en orden, en un búfer de `width_2 × height_2` píxeles físicos, muestreando cada
/// píxel en su centro (algo desplazado). La textura de la imagen se muestrea con `NEAREST` en su uv
/// interpolado, **sin** envoltura (se recorta al borde); la del damero, con envoltura; las mallas
/// de colores de vértice y los rellenos usan el color de sus vértices.
fn rasterizes(
    primitives: &[ClippedPrimitive],
    ppp: f32,
    width_2: usize,
    height_2: usize,
    tex: &Textures,
) -> Buffer {
    let ppp = f64::from(ppp);
    let mut pixels_2 = vec![Seen::EMPTY; width_2 * height_2];
    for primitive in primitives {
        let Primitive::Mesh(mesh) = &primitive.primitive else {
            continue;
        };
        let clip = primitive.clip_rect;
        let (cx0, cy0) = (f64::from(clip.min.x) * ppp, f64::from(clip.min.y) * ppp);
        let (cx1, cy1) = (f64::from(clip.max.x) * ppp, f64::from(clip.max.y) * ppp);
        let is_image = mesh.texture_id == tex.image_2;
        let is_checker = tex.checker_2 == Some(mesh.texture_id);
        for triangle in mesh.indices.as_chunks::<3>().0 {
            let v = [
                &mesh.vertices[triangle[0] as usize],
                &mesh.vertices[triangle[1] as usize],
                &mesh.vertices[triangle[2] as usize],
            ];
            let p = v.map(|v| (f64::from(v.pos.x) * ppp, f64::from(v.pos.y) * ppp));
            let min_x = p.iter().map(|q| q.0).fold(f64::INFINITY, f64::min);
            let max_x = p.iter().map(|q| q.0).fold(f64::NEG_INFINITY, f64::max);
            let min_y = p.iter().map(|q| q.1).fold(f64::INFINITY, f64::min);
            let max_y = p.iter().map(|q| q.1).fold(f64::NEG_INFINITY, f64::max);
            let x0 = (min_x.floor() - 1.0).max(cx0.floor()).max(0.0) as usize;
            let y0 = (min_y.floor() - 1.0).max(cy0.floor()).max(0.0) as usize;
            let x1 = (max_x.ceil() + 1.0)
                .min(cx1.ceil())
                .min(width_2 as f64)
                .max(0.0) as usize;
            let y1 = (max_y.ceil() + 1.0)
                .min(cy1.ceil())
                .min(height_2 as f64)
                .max(0.0) as usize;
            let area =
                (p[1].0 - p[0].0) * (p[2].1 - p[0].1) - (p[2].0 - p[0].0) * (p[1].1 - p[0].1);
            if area.abs() < 1e-12 {
                continue;
            }
            let tints = v.map(|v| Seen::of_color(v.color));
            let plane =
                !is_image && !is_checker && v[0].color == v[1].color && v[1].color == v[2].color;
            let edges = [(p[0], p[1]), (p[1], p[2]), (p[2], p[0])];
            for py in y0..y1 {
                let sy = py as f64 + 0.5 + DESPLAZO_Y;
                if sy < cy0 || sy > cy1 {
                    continue;
                }
                let (mut left, mut right) = (f64::INFINITY, f64::NEG_INFINITY);
                for (a, b) in edges {
                    let (under, height_) = if a.1 <= b.1 { (a, b) } else { (b, a) };
                    if sy < under.1 || sy > height_.1 {
                        continue;
                    }
                    if height_.1 == under.1 {
                        left = left.min(a.0.min(b.0));
                        right = right.max(a.0.max(b.0));
                    } else {
                        let t = (sy - under.1) / (height_.1 - under.1);
                        let x = under.0 + t * (height_.0 - under.0);
                        left = left.min(x);
                        right = right.max(x);
                    }
                }
                if left > right {
                    continue;
                }
                let from_2 = ((left.max(cx0) - 0.5 - DESPLAZO_X).ceil().max(x0 as f64)) as usize;
                let until = (right.min(cx1) - 0.5 - DESPLAZO_X)
                    .floor()
                    .min(x1 as f64 - 1.0);
                if until < 0.0 {
                    continue;
                }
                for px in from_2..=(until as usize) {
                    let s = (px as f64 + 0.5 + DESPLAZO_X, sy);
                    let color = if plane {
                        tints[0]
                    } else {
                        let Some(w) = barycentric(s, p[0], p[1], p[2]) else {
                            continue;
                        };
                        let wf = w.map(|w| w as f32);
                        let tint = Seen {
                            r: wf[0] * tints[0].r + wf[1] * tints[1].r + wf[2] * tints[2].r,
                            g: wf[0] * tints[0].g + wf[1] * tints[1].g + wf[2] * tints[2].g,
                            b: wf[0] * tints[0].b + wf[1] * tints[1].b + wf[2] * tints[2].b,
                            a: wf[0] * tints[0].a + wf[1] * tints[1].a + wf[2] * tints[2].a,
                        };
                        if is_image || is_checker {
                            let u = w[0] * f64::from(v[0].uv.x)
                                + w[1] * f64::from(v[1].uv.x)
                                + w[2] * f64::from(v[2].uv.x);
                            let t = w[0] * f64::from(v[0].uv.y)
                                + w[1] * f64::from(v[1].uv.y)
                                + w[2] * f64::from(v[2].uv.y);
                            let texel = if is_image {
                                let tx = ((u * tex.width_2 as f64).floor().max(0.0) as usize)
                                    .min(tex.width_2 - 1);
                                let ty = ((t * tex.height_2 as f64).floor().max(0.0) as usize)
                                    .min(tex.height_2 - 1);
                                Seen::of_color(tex.texels[ty * tex.width_2 + tx])
                            } else {
                                texel_of_the_checker(u, t)
                            };
                            texel.per(tint)
                        } else {
                            tint
                        }
                    };
                    let target_2 = &mut pixels_2[py * width_2 + px];
                    *target_2 = target_2.below_of(color);
                }
            }
        }
    }
    Buffer {
        width_2,
        height_2,
        pixels_2,
    }
}

// ---------------------------------------------------------------------------------------------
// Lo pintado, por capas (formas sin teselar)
// ---------------------------------------------------------------------------------------------

#[derive(Clone, Debug)]
enum Layer {
    TestFill {
        rect: Rect,
    },
    TestMesh {
        texture_2: TextureId,
        vertices: Vec<Vertex>,
        indices: Vec<u32>,
    },
}

fn collects(shape: &Shape, output_2: &mut Vec<Layer>) {
    match shape {
        Shape::Rect(r) => output_2.push(Layer::TestFill { rect: r.rect }),
        Shape::Mesh(m) => output_2.push(Layer::TestMesh {
            texture_2: m.texture_id,
            vertices: m.vertices.clone(),
            indices: m.indices.clone(),
        }),
        Shape::Vec(shapes_2) => {
            for shape_2 in shapes_2 {
                collects(shape_2, output_2);
            }
        }
        _ => {}
    }
}

fn layers_of(shapes_2: &[ClippedShape]) -> Vec<Layer> {
    let mut v = Vec::new();
    for clipped in shapes_2 {
        collects(&clipped.shape, &mut v);
    }
    v
}

fn toolbox_pos(vertices: &[Vertex]) -> Rect {
    let mut r = Rect::NOTHING;
    for v in vertices {
        r.extend_with(v.pos);
    }
    r
}

fn toolbox_uv(vertices: &[Vertex]) -> [f32; 4] {
    let mut r = Rect::NOTHING;
    for v in vertices {
        r.extend_with(v.uv);
    }
    [r.min.x, r.min.y, r.max.x, r.max.y]
}

/// Área de los triángulos de una malla, en píxeles físicos cuadrados (el menor de todos).
fn smaller_area_physical(vertices: &[Vertex], indices: &[u32], ppp: f32) -> Option<f64> {
    let k = f64::from(ppp);
    indices
        .as_chunks::<3>()
        .0
        .iter()
        .map(|t| {
            let [a, b, c] = t.map(|i| vertices[i as usize].pos);
            let cross = f64::from(b.x - a.x) * f64::from(c.y - a.y)
                - f64::from(c.x - a.x) * f64::from(b.y - a.y);
            cross.abs() / 2.0 * k * k
        })
        .reduce(f64::min)
}

// ---------------------------------------------------------------------------------------------
// El banco
// ---------------------------------------------------------------------------------------------

struct TestOutput {
    shapes_2: Vec<ClippedShape>,
    info: CanvasInfo,
    entries: Vec<CanvasInput>,
}

impl TestOutput {
    /// Solo los eventos de trazo, en orden.
    fn stroke_2(&self) -> Vec<CanvasEvent> {
        self.entries
            .iter()
            .filter_map(|e| match e {
                CanvasInput::Stroke(ev) => Some(*ev),
                CanvasInput::Escape => None,
            })
            .collect()
    }
}

/// Geometría de lo pintado, en píxeles físicos.
#[derive(Clone, Copy, Debug)]
struct Geom {
    source_2: (i64, i64),
    viewport: Size,
    cell: i64,
}

struct Bench {
    ctx: Context,
    ppp: f32,
    screen: Vec2,
    canvas: Canvas,
    /// El documento (lo que `sync` y `set_overlay` reciben): no cambia en estas pruebas.
    document: Image,
    /// Lo que la prueba sabe de la imagen (el oráculo), si la necesita.
    image_2: Option<TestImage>,
    /// El contenido de las texturas tal como egui lo entrega a la GPU: se aplican los
    /// `textures_delta` de **cada** fotograma, en orden.
    composite_2: common::textures::Textures,
}

impl Bench {
    fn with_image(ppp: f32, screen: Vec2, image: &Image) -> Bench {
        let mut canvas = Canvas::new();
        canvas.set_image(image);
        let mut bench = Bench {
            ctx: context(ppp),
            ppp,
            screen,
            canvas,
            document: image.clone(),
            image_2: None,
            composite_2: common::textures::Textures::new(),
        };
        for _ in 0..3 {
            bench.rest_2_2();
        }
        bench
    }

    fn new_2(ppp: f32, screen: Vec2, image_2: TestImage) -> Bench {
        let mut bench = Bench::with_image(ppp, screen, &image_2.image());
        bench.image_2 = Some(image_2);
        bench
    }

    fn with_background(mut self, background_2: CanvasBackground, mesh: bool) -> Bench {
        self.canvas.set_background(background_2);
        self.canvas.set_checker_mesh(mesh);
        for _ in 0..2 {
            self.rest_2_2();
        }
        self
    }

    /// Un fotograma, en el orden de la app: `interact_ordered`, `set_overlay` con la superposición
    /// dada (el trazo pendiente entero; vacío la quita), `sync` y `paint`. Aplica los cambios de
    /// textura del fotograma (completos y parciales) antes de devolver las formas.
    fn runs(&mut self, events: Vec<Event>, overlay: &[TestPending]) -> TestOutput {
        let input_2 = RawInput {
            screen_rect: Some(Rect::from_min_size(Pos2::ZERO, self.screen)),
            events,
            ..RawInput::default()
        };
        let mut result = None;
        let canvas = &mut self.canvas;
        let document = &self.document;
        let mut output_2 = self.ctx.run_ui(input_2, |ui| {
            let entries = canvas
                .interact_roles(ui)
                .into_iter()
                .map(|(input, _)| input)
                .collect::<Vec<_>>();
            canvas.set_overlay(ui.ctx(), document, overlay);
            canvas
                .sync(ui.ctx(), document)
                .expect("el documento se carga");
            let info = canvas.paint(ui);
            result = Some((info, entries));
        });
        // Sin los cambios de la fuente de egui (`Managed(0)`): el banco no pinta texto.
        let mut delta: TexturesDelta = output_2.textures_delta.clone();
        delta.set.remove(&TextureId::Managed(0));
        delta.free.remove(&TextureId::Managed(0));
        self.composite_2.applies(&delta);
        // La copia no se entrega a la GPU: egui exige vaciarla antes de soltarla.
        delta.clear();
        output_2.textures_delta.clear();
        let shapes_2 = std::mem::take(&mut output_2.shapes);
        let (info, entries) = result.expect("el lienzo se dibuja");
        TestOutput {
            shapes_2,
            info,
            entries,
        }
    }

    fn rest_2_2(&mut self) -> CanvasInfo {
        self.runs(vec![], &[]).info
    }

    fn asks(&mut self, op: ViewOp) -> CanvasInfo {
        self.canvas.request_view(op);
        self.rest_2_2()
    }

    /// Lleva el zoom al nivel `index_2` de la escalera (0 = 1/64, 6 = ×1, 69 = ×64) con peticiones de
    /// vista, empezando por ×1 anclado en el centro.
    fn go_to_level(&mut self, index_2: usize) -> CanvasInfo {
        let mut info = self.asks(ViewOp::ActualSize);
        for _ in 6..index_2 {
            info = self.asks(ViewOp::ZoomIn);
        }
        for _ in index_2..6 {
            info = self.asks(ViewOp::ZoomOut);
        }
        assert_eq!(
            info.zoom.to_string(),
            label(ladder()[index_2]),
            "ppp {}: el banco no llegó al nivel {index_2}",
            self.ppp
        );
        info
    }

    /// Tamaño del lienzo en píxeles físicos (todo el espacio de `ui`).
    fn viewport(&self) -> Size {
        Size::new(
            (self.screen.x * self.ppp).round() as u32,
            (self.screen.y * self.ppp).round() as u32,
        )
    }

    fn geom(&self, info: &CanvasInfo) -> Geom {
        let rect = info.image_rect.expect("hay imagen");
        let ppp = self.ppp;
        Geom {
            source_2: (
                (rect.min.x * ppp).round() as i64,
                (rect.min.y * ppp).round() as i64,
            ),
            viewport: self.viewport(),
            cell: 8 * i64::from(text_scale(ppp)),
        }
    }

    /// Posición en puntos de un píxel físico (más una fracción que no cambia el redondeo).
    fn pos(&self, x: i64, y: i64) -> Pos2 {
        in_points(self.ppp, x, y)
    }

    /// Las texturas para rasterizar: la **compuesta** de la baldosa 0 (la imagen de A9 cabe en una
    /// sola baldosa, y con la superposición ya escrita en ella) y la del damero.
    fn textures_2(&self) -> Textures {
        let id = self.canvas.texture_id().expect("hay textura de la imagen");
        let composite = self
            .composite_2
            .get(id)
            .expect("la baldosa se subió y sus cambios se aplicaron");
        Textures {
            image_2: id,
            width_2: composite.size_2[0],
            height_2: composite.size_2[1],
            texels: composite.texels.clone(),
            checker_2: self.canvas.checker_texture_id(),
        }
    }
}

// ---------------------------------------------------------------------------------------------
// A6 · Lienzo a las cinco escalas
// ---------------------------------------------------------------------------------------------

/// El puntero queda en el centro de la pantalla, sobre la imagen de 64×48 centrada.
fn pointer_central(ppp: f32) -> (Pos2, (i64, i64)) {
    let pos = pos2(TEST_SCREEN.x / 2.0, TEST_SCREEN.y / 2.0);
    (
        pos,
        ((pos.x * ppp).round() as i64, (pos.y * ppp).round() as i64),
    )
}

/// `image_rect` mide `extent/ppp` en cada eje (`extent` por el oráculo).
fn checks_rect(info: &CanvasInfo, ppp: f32, tam: (u32, u32), label_2: &str) {
    let f = ladder()[ladder()
        .iter()
        .position(|n| label(*n) == info.zoom.to_string())
        .expect("nivel de la escalera")];
    let rect = info.image_rect.expect("hay imagen");
    let (width_2, height_2) = (
        o_extent(f, i64::from(tam.0)) as f32,
        o_extent(f, i64::from(tam.1)) as f32,
    );
    assert!(
        (rect.width() * ppp - width_2).abs() <= 0.02
            && (rect.height() * ppp - height_2).abs() <= 0.02,
        "{label_2}: el rectángulo mide {}×{} px físicos y debía medir {width_2}×{height_2}",
        rect.width() * ppp,
        rect.height() * ppp
    );
}

#[test]
fn row_a6_the_wheel_goes_down_to_1_64_and_up_to_x64_with_the_pixel_under_the_pointer() {
    let levels = ladder();
    for ppp in PPPS {
        let mut bench = Bench::with_image(ppp, TEST_SCREEN, &image_64x48());
        let (pos, physical) = pointer_central(ppp);
        let info = bench.go_to_level(6);
        assert_eq!(info.zoom, Zoom::ONE);
        bench.runs(vec![move_pointer(pos)], &[]);
        let mut pixel = bench
            .rest_2_2()
            .hover
            .expect("el centro está sobre la imagen");
        let mut index_2 = 6usize;
        // Ocho muescas hacia fuera: 1/2, 1/4, …, 1/64 y satura (dos muescas de más).
        let mut steps_2: Vec<bool> = vec![false; 8];
        // Y 70 hacia dentro: ×2, …, ×64 y satura (la escalera tiene 69 pasos).
        steps_2.extend(vec![true; 70]);
        for (n, zooms_in) in steps_2.into_iter().enumerate() {
            let output_2 = bench.runs(vec![wheel_line(zooms_in)], &[]);
            index_2 = if zooms_in {
                (index_2 + 1).min(69)
            } else {
                index_2.saturating_sub(1)
            };
            let label_2 = format!("ppp {ppp} muesca {n} → {}", label(levels[index_2]));
            assert_eq!(
                output_2.info.zoom.to_string(),
                label(levels[index_2]),
                "{label_2}"
            );
            assert_eq!(bench.canvas.zoom(), Some(output_2.info.zoom), "{label_2}");
            // El píxel bajo el puntero sube a la rejilla del nivel nuevo (exacto con zoom ≥ ×1).
            pixel = goes_up_to_the_grid(pixel, grid(index_2));
            assert_eq!(
                output_2.info.hover,
                Some(pixel),
                "{label_2}: píxel bajo el puntero"
            );
            // Y es el del oráculo con el `physical()` de egui.
            let g = bench.geom(&output_2.info);
            assert_eq!(
                output_2.info.hover,
                o_hover(levels[index_2], physical, g.source_2, (64, 48)),
                "{label_2}: oráculo"
            );
            checks_rect(&output_2.info, ppp, (64, 48), &label_2);
        }
        assert_eq!(bench.canvas.zoom(), Some(Zoom::MAX), "ppp {ppp}");
    }
}

#[test]
fn row_a6_the_wheel_outward_saturates_at_1_64_and_half_way_goes_down_one_level_per_notch() {
    for ppp in PPPS {
        let mut bench = Bench::with_image(ppp, TEST_SCREEN, &image_64x48());
        let (pos, _) = pointer_central(ppp);
        bench.go_to_level(6);
        bench.runs(vec![move_pointer(pos)], &[]);
        let expected_2_2 = [
            "×1/2", "×1/4", "×1/8", "×1/16", "×1/32", "×1/64", "×1/64", "×1/64",
        ];
        for (n, expected_2) in expected_2_2.iter().enumerate() {
            let info = bench.runs(vec![wheel_line(false)], &[]).info;
            assert_eq!(info.zoom.to_string(), *expected_2, "ppp {ppp} muesca {n}");
        }
        assert_eq!(bench.canvas.zoom(), Some(Zoom::MIN));
        // A 1/64 la imagen de 64×48 es un solo píxel físico y sigue bajo el puntero.
        let info = bench.rest_2_2();
        assert_eq!(info.hover, Some((0, 0)), "ppp {ppp}");
        checks_rect(&info, ppp, (64, 48), "1/64");
    }
}

#[test]
fn row_a6_the_zoom_requests_at_the_extremes_do_not_change_the_level() {
    for ppp in PPPS {
        let mut bench = Bench::with_image(ppp, TEST_SCREEN, &image_64x48());
        bench.go_to_level(6);
        let info = bench.asks(ViewOp::ZoomOut);
        assert_eq!(info.zoom.to_string(), "×1/2", "ppp {ppp}");
        let info = bench.asks(ViewOp::ZoomIn);
        assert_eq!(info.zoom, Zoom::ONE, "ppp {ppp}");
        for _ in 0..8 {
            bench.asks(ViewOp::ZoomOut);
        }
        assert_eq!(bench.canvas.zoom(), Some(Zoom::MIN), "ppp {ppp}");
        let before = bench.rest_2_2().image_rect;
        let info = bench.asks(ViewOp::ZoomOut);
        assert_eq!(info.zoom, Zoom::MIN, "ppp {ppp}: `-` satura");
        assert_eq!(info.image_rect, before, "ppp {ppp}");
        for _ in 0..80 {
            bench.asks(ViewOp::ZoomIn);
        }
        assert_eq!(bench.canvas.zoom(), Some(Zoom::MAX), "ppp {ppp}");
        let before = bench.rest_2_2().image_rect;
        let info = bench.asks(ViewOp::ZoomIn);
        assert_eq!(info.zoom, Zoom::MAX, "ppp {ppp}: `+` satura");
        assert_eq!(info.image_rect, before, "ppp {ppp}");
    }
}

/// Puntos de un eje que se prueban: el píxel físico anterior a la imagen, el siguiente, y, por cada
/// píxel de la rejilla, su primer y su último píxel físico (a 1/K son el mismo y se miran todos; con
/// zoom ≥ ×1 se miran los tres primeros y los tres últimos píxeles de la imagen).
fn points_of_the_axis(f: Fraction, source_2: i64, dim: u32) -> Vec<i64> {
    let extent = o_extent(f, i64::from(dim));
    let mut v = vec![source_2 - 1, source_2 + extent];
    if f.1 > 1 {
        v.extend(source_2..source_2 + extent);
        return v;
    }
    let side = f.0 as i64;
    let dim = i64::from(dim);
    for i in [0, 1, 2, dim - 3, dim - 2, dim - 1] {
        v.push(source_2 + i * side);
        v.push(source_2 + (i + 1) * side - 1);
    }
    v
}

#[test]
fn row_a6_hover_and_stroke_begin_at_each_pixel_of_the_grid_and_one_outside() {
    let levels = ladder();
    // 1/64, 1/8, 1/4, 1/2, ×1 y ×3.
    for ppp in PPPS {
        for index_2 in [0usize, 3, 4, 5, 6, 8] {
            let f = levels[index_2];
            let mut bench = Bench::with_image(ppp, TEST_SCREEN, &image_64x48());
            let info = bench.go_to_level(index_2);
            let g = bench.geom(&info);
            let (ext_x, ext_y) = (o_extent(f, 64), o_extent(f, 48));
            let row_2 = g.source_2.1 + ext_y / 2;
            let column = g.source_2.0 + ext_x / 2;
            let mut points: Vec<(i64, i64)> = points_of_the_axis(f, g.source_2.0, 64)
                .into_iter()
                .map(|x| (x, row_2))
                .collect();
            points.extend(
                points_of_the_axis(f, g.source_2.1, 48)
                    .into_iter()
                    .map(|y| (column, y)),
            );
            for p in points {
                let label_2 = format!(
                    "ppp {ppp} {} punto físico {p:?} (origen {:?})",
                    label(f),
                    g.source_2
                );
                let pos = bench.pos(p.0, p.1);
                // hover
                let rest_2_2 = bench.runs(vec![move_pointer(pos)], &[]);
                assert_eq!(
                    rest_2_2.info.hover,
                    o_hover(f, p, g.source_2, (64, 48)),
                    "{label_2}: hover"
                );
                // StrokeBegin (con signo, sin recortar a la imagen).
                let presses_2 = bench.runs(vec![button_2(pos, true)], &[]);
                assert_eq!(
                    presses_2.stroke_2(),
                    vec![CanvasEvent::StrokeBegin(o_stroke(f, p, g.source_2))],
                    "{label_2}: StrokeBegin"
                );
                // Se suelta para el siguiente punto.
                let releases = bench.runs(vec![button_2(pos, false)], &[]);
                assert_eq!(
                    releases.stroke_2().last(),
                    Some(&CanvasEvent::StrokeEnd),
                    "{label_2}"
                );
            }
        }
    }
}

#[test]
fn row_a6_at_1_k_hover_and_stroke_begin_are_multiples_of_k_and_outside_hover_is_none() {
    for ppp in PPPS {
        for (index_2, k) in [(5usize, 2i32), (4, 4), (0, 64)] {
            let f = ladder()[index_2];
            let mut bench = Bench::with_image(ppp, TEST_SCREEN, &image_64x48());
            let info = bench.go_to_level(index_2);
            let g = bench.geom(&info);
            let (ext_x, ext_y) = (o_extent(f, 64), o_extent(f, 48));
            for (dx, dy) in [
                (0, 0),
                (ext_x - 1, ext_y - 1),
                (-1, 0),
                (0, -1),
                (ext_x, 0),
                (0, ext_y),
            ] {
                let p = (g.source_2.0 + dx, g.source_2.1 + dy);
                let pos = bench.pos(p.0, p.1);
                let inside = (0..ext_x).contains(&dx) && (0..ext_y).contains(&dy);
                let hover = bench.runs(vec![move_pointer(pos)], &[]).info.hover;
                let begin = bench.runs(vec![button_2(pos, true)], &[]).stroke_2();
                bench.runs(vec![button_2(pos, false)], &[]);
                let expected_2 = (dx as i32 * k, dy as i32 * k);
                assert_eq!(
                    begin,
                    vec![CanvasEvent::StrokeBegin(expected_2)],
                    "ppp {ppp} 1/{k} ({dx}, {dy})"
                );
                if inside {
                    assert_eq!(hover, Some((expected_2.0 as u32, expected_2.1 as u32)));
                    assert_eq!(expected_2.0 % k, 0);
                    assert_eq!(expected_2.1 % k, 0);
                } else {
                    assert_eq!(hover, None, "ppp {ppp} 1/{k} ({dx}, {dy}) está fuera");
                }
            }
        }
    }
}

/// Imagen de 2048×1536 (cabe en la textura de 2048 de egui) para que Ajustar caiga en fracciones.
fn image_large() -> Image {
    Image::new(2048, 1536, Mode::Rgba).expect("imagen válida")
}

#[test]
fn row_a6_home_fit_and_request_view_give_that_of_a4() {
    let levels = ladder();
    for ppp in PPPS {
        let mut bench = Bench::with_image(ppp, TEST_SCREEN, &image_large());
        let viewport = bench.viewport();
        let better = o_better_level(viewport, (2048, 1536));
        let expected_2 = label(levels[better]);
        // El ajuste inicial.
        let info = bench.rest_2_2();
        assert_eq!(
            info.zoom.to_string(),
            expected_2,
            "ppp {ppp}: ajuste inicial"
        );
        checks_rect(&info, ppp, (2048, 1536), "ajuste inicial");
        let g = bench.geom(&info);
        let f = levels[better];
        for (axis, source_2, view, dim) in [
            ("x", g.source_2.0, viewport.width, 2048),
            ("y", g.source_2.1, viewport.height, 1536),
        ] {
            let extent = o_extent(f, dim);
            assert!(
                (2 * source_2 + extent - i64::from(view)).abs() <= 1,
                "ppp {ppp}: centrada en {axis} (origen {source_2}, extent {extent}, lienzo {view})"
            );
        }
        // Home.
        bench.asks(ViewOp::ActualSize);
        assert_eq!(bench.canvas.zoom(), Some(Zoom::ONE));
        let info = bench
            .runs(vec![key_2(Key::Home, Modifiers::NONE)], &[])
            .info;
        assert_eq!(info.zoom.to_string(), expected_2, "ppp {ppp}: Home");
        // request_view(Fit) desde otro nivel.
        bench.asks(ViewOp::ActualSize);
        let info = bench.asks(ViewOp::Fit);
        assert_eq!(info.zoom.to_string(), expected_2, "ppp {ppp}: Fit");
        assert_eq!(
            bench.geom(&info).source_2,
            g.source_2,
            "ppp {ppp}: Fit centra igual"
        );
        // request_view(ZoomOut) y (ZoomIn) desde el ajuste: un nivel.
        let info = bench.asks(ViewOp::ZoomOut);
        assert_eq!(
            info.zoom.to_string(),
            label(levels[better.saturating_sub(1)]),
            "ppp {ppp}: ZoomOut"
        );
        bench.asks(ViewOp::Fit);
        let info = bench.asks(ViewOp::ZoomIn);
        assert_eq!(
            info.zoom.to_string(),
            label(levels[(better + 1).min(69)]),
            "ppp {ppp}: ZoomIn"
        );
        checks_rect(&info, ppp, (2048, 1536), "tras ZoomIn");
    }
}

/// La malla de la textura de la imagen (la de la única baldosa de una imagen pequeña).
fn image_mesh(bench: &Bench, shapes_2: &[ClippedShape]) -> (Vec<Vertex>, Vec<u32>) {
    let id = bench.canvas.texture_id().expect("hay textura de la imagen");
    let mut meshes: Vec<(Vec<Vertex>, Vec<u32>)> = layers_of(shapes_2)
        .into_iter()
        .filter_map(|layer_2| match layer_2 {
            Layer::TestMesh {
                texture_2,
                vertices,
                indices,
            } if texture_2 == id => Some((vertices, indices)),
            _ => None,
        })
        .collect();
    assert_eq!(
        meshes.len(),
        1,
        "con una sola baldosa, una sola malla de la imagen"
    );
    meshes.remove(0)
}

#[test]
fn row_a6_the_image_mesh_is_a_quad_of_extent_pixels_with_the_shifted_uv() {
    let levels = ladder();
    // 1/64, 1/32, 1/8, 1/2, ×1, ×2 y ×6.
    for ppp in PPPS {
        for index_2 in [0usize, 1, 3, 5, 6, 7, 11] {
            let f = levels[index_2];
            let mut bench = Bench::with_image(ppp, TEST_SCREEN, &image_64x48());
            bench.go_to_level(index_2);
            let output_2 = bench.runs(vec![], &[]);
            let g = bench.geom(&output_2.info);
            let (vertices, indices) = image_mesh(&bench, &output_2.shapes_2);
            let label_2 = format!("ppp {ppp} {}", label(f));
            assert_eq!(vertices.len(), 4, "{label_2}: un cuadrilátero");
            assert_eq!(indices.len(), 6, "{label_2}");
            // Ocupa `extent` píxeles físicos desde el origen.
            let (ext_x, ext_y) = (o_extent(f, 64) as f32, o_extent(f, 48) as f32);
            let toolbox_2 = toolbox_pos(&vertices);
            let expected = Rect::from_min_max(
                pos2(g.source_2.0 as f32 / ppp, g.source_2.1 as f32 / ppp),
                pos2(
                    (g.source_2.0 as f32 + ext_x) / ppp,
                    (g.source_2.1 as f32 + ext_y) / ppp,
                ),
            );
            assert!(
                (toolbox_2.min.x - expected.min.x).abs() <= 2e-3
                    && (toolbox_2.min.y - expected.min.y).abs() <= 2e-3
                    && (toolbox_2.max.x - expected.max.x).abs() <= 2e-3
                    && (toolbox_2.max.y - expected.max.y).abs() <= 2e-3,
                "{label_2}: la malla ocupa {toolbox_2:?} y debía ocupar {expected:?}"
            );
            // uv: de 0 a 1 con zoom ≥ ×1; con 1/K, `(x·K + 0,5 − K/2) / side` desde el borde.
            let uv = toolbox_uv(&vertices);
            let expected_2 = if f.1 == 1 {
                [0.0, 0.0, 1.0, 1.0]
            } else {
                let k = f.1 as f32;
                [
                    (0.5 - k / 2.0) / 64.0,
                    (0.5 - k / 2.0) / 48.0,
                    (ext_x * k + 0.5 - k / 2.0) / 64.0,
                    (ext_y * k + 0.5 - k / 2.0) / 48.0,
                ]
            };
            for (a, b) in uv.iter().zip(expected_2) {
                assert!(
                    (a - b).abs() <= 1e-4,
                    "{label_2}: uv {uv:?} y debía ser {expected_2:?}"
                );
            }
            checks_rect(&output_2.info, ppp, (64, 48), &label_2);
        }
    }
}

// ---------------------------------------------------------------------------------------------
// P1 · banco de egui
// ---------------------------------------------------------------------------------------------

proptest! {
    #![proptest_config(ProptestConfig::with_cases(24))]

    #[test]
    fn property_p1_hover_and_stroke_begin_of_any_pointer_are_the_oracle(
        ppp_i in 0usize..5,
        index_2 in 0usize..70,
        x_frac in 0.0f64..1.0,
        y_frac in 0.0f64..1.0,
        pan in (-0.4f32..0.4, -0.4f32..0.4),
    ) {
        let ppp = PPPS[ppp_i];
        let f = ladder()[index_2];
        let mut bench = Bench::with_image(ppp, TEST_SCREEN, &image_64x48());
        let info = bench.go_to_level(index_2);
        let g = bench.geom(&info);
        // Un píxel físico cualquiera del lienzo (no el del borde) más una fracción que no cambia
        // `round(v·ppp)`.
        let width_2 = i64::from(g.viewport.width);
        let height_2 = i64::from(g.viewport.height);
        let p = (
            1 + (x_frac * (width_2 - 3) as f64) as i64,
            1 + (y_frac * (height_2 - 3) as f64) as i64,
        );
        let pos = pos2((p.0 as f32 + pan.0) / ppp, (p.1 as f32 + pan.1) / ppp);
        prop_assert_eq!((pos.x * ppp).round() as i64, p.0);
        prop_assert_eq!((pos.y * ppp).round() as i64, p.1);
        let hover = bench.runs(vec![move_pointer(pos)], &[]).info.hover;
        prop_assert_eq!(hover, o_hover(f, p, g.source_2, (64, 48)), "{} ppp {} {:?}", label(f), ppp, p);
        let begin = bench.runs(vec![button_2(pos, true)], &[]).stroke_2();
        prop_assert_eq!(
            begin,
            vec![CanvasEvent::StrokeBegin(o_stroke(f, p, g.source_2))],
            "{} ppp {} {:?}", label(f), ppp, p
        );
    }
}

// ---------------------------------------------------------------------------------------------
// A7 · Trazo a 1/4
// ---------------------------------------------------------------------------------------------

fn red() -> Pixel {
    Pixel::Rgba(RED)
}

/// Quita los `StrokeMove` que repiten la posición anterior (soltar sin moverse repite la última).
fn without_moves_repeated(events_2: &[CanvasEvent]) -> Vec<CanvasEvent> {
    let mut output_2: Vec<CanvasEvent> = Vec::new();
    let mut last: Option<(i32, i32)> = None;
    for e in events_2 {
        match e {
            CanvasEvent::StrokeMove(p) if last == Some(*p) => continue,
            CanvasEvent::StrokeBegin(p) | CanvasEvent::StrokeMove(p) => last = Some(*p),
            CanvasEvent::StrokeEnd => last = None,
        }
        output_2.push(*e);
    }
    output_2
}

/// Un gesto: pulsa en `(ox + a.0, oy + a.1)`, mueve por cada punto de `moves` y suelta en el último.
fn gesture_2(
    bench: &mut Bench,
    source_2: (i64, i64),
    from_2: (i64, i64),
    moves: &[(i64, i64)],
) -> Vec<CanvasEvent> {
    let pos = |d: (i64, i64)| bench.pos(source_2.0 + d.0, source_2.1 + d.1);
    let (start_2, last_2, targets) = (
        pos(from_2),
        pos(*moves.last().expect("al menos un movimiento")),
        moves.iter().map(|d| pos(*d)).collect::<Vec<_>>(),
    );
    let mut events_2 = Vec::new();
    events_2.extend(bench.runs(vec![move_pointer(start_2)], &[]).stroke_2());
    events_2.extend(bench.runs(vec![button_2(start_2, true)], &[]).stroke_2());
    for target_2 in targets {
        events_2.extend(bench.runs(vec![move_pointer(target_2)], &[]).stroke_2());
    }
    events_2.extend(bench.runs(vec![button_2(last_2, false)], &[]).stroke_2());
    without_moves_repeated(&events_2)
}

/// Aplica los eventos a un `Stroke` de 64×48 y devuelve sus píxeles (sin aplicarlo).
fn pixels_of_the_stroke(events_2: &[CanvasEvent]) -> Vec<(u32, u32)> {
    let mut stroke = Stroke::new(64, 48);
    for e in events_2 {
        match e {
            CanvasEvent::StrokeBegin(p) | CanvasEvent::StrokeMove(p) => stroke.move_to(*p, red()),
            CanvasEvent::StrokeEnd => {}
        }
    }
    stroke.pixels().iter().map(|p| (p.x, p.y)).collect()
}

#[test]
fn row_a7_a_stroke_at_1_4_goes_along_the_grid_in_a_single_step() {
    for ppp in PPPS {
        let mut bench = Bench::with_image(ppp, TEST_SCREEN, &image_64x48());
        let info = bench.go_to_level(4);
        assert_eq!(info.zoom.to_string(), "×1/4");
        let source_2 = bench.geom(&info).source_2;
        let events_2 = gesture_2(&mut bench, source_2, (2, 1), &[(5, 1)]);
        assert_eq!(
            events_2,
            vec![
                CanvasEvent::StrokeBegin((8, 4)),
                CanvasEvent::StrokeMove((20, 4)),
                CanvasEvent::StrokeEnd
            ],
            "ppp {ppp}"
        );
        // Los 13 píxeles de la línea de (8, 4) a (20, 4), sin huecos y en orden.
        let expected_2_2: Vec<(u32, u32)> = (8..=20).map(|x| (x, 4)).collect();
        assert_eq!(pixels_of_the_stroke(&events_2), expected_2_2, "ppp {ppp}");

        // Un solo `SetPixels` (un solo paso): un `undo` restaura la imagen.
        let original = image_64x48();
        let mut doc = Document::new(original.clone());
        let mut stroke = Stroke::new(64, 48);
        for e in &events_2 {
            match e {
                CanvasEvent::StrokeBegin(p) | CanvasEvent::StrokeMove(p) => {
                    stroke.move_to(*p, red())
                }
                CanvasEvent::StrokeEnd => {}
            }
        }
        let id = ImageId::from_raw(0);
        assert_eq!(stroke.commit(&mut doc, id), Ok(true));
        let image_2 = doc.image(ImageId::from_raw(0)).expect("hay imagen");
        assert_eq!(image_2.get_pixel(8, 4), Ok(red()));
        assert_eq!(image_2.get_pixel(20, 4), Ok(red()));
        assert_eq!(
            image_2.get_pixel(14, 4),
            Ok(red()),
            "sin huecos en coordenadas de imagen"
        );
        assert_eq!(image_2.get_pixel(21, 4), Ok(Pixel::Rgba(TRANSPARENT)));
        assert!(doc.can_undo());
        assert_eq!(doc.undo(), Ok(()));
        assert!(!doc.can_undo(), "un solo paso");
        assert_eq!(
            doc.image(ImageId::from_raw(0)),
            Some(&original),
            "undo restaura"
        );
    }
}

#[test]
fn row_a7_a_stroke_that_leaves_on_the_left_and_comes_back_is_joined_with_line_and_clipped() {
    for ppp in PPPS {
        let mut bench = Bench::with_image(ppp, TEST_SCREEN, &image_64x48());
        let info = bench.go_to_level(4);
        let source_2 = bench.geom(&info).source_2;
        // (2, 1) → (-3, 1) → (6, 1): a 1/4, (8, 4), (-12, 4) y (24, 4).
        let events_2 = gesture_2(&mut bench, source_2, (2, 1), &[(-3, 1), (6, 1)]);
        assert_eq!(
            events_2,
            vec![
                CanvasEvent::StrokeBegin((8, 4)),
                CanvasEvent::StrokeMove((-12, 4)),
                CanvasEvent::StrokeMove((24, 4)),
                CanvasEvent::StrokeEnd
            ],
            "ppp {ppp}"
        );
        // Las dos líneas se recortan a la imagen: la unión es x de 0 a 24 en la fila 4.
        let pixels_2 = pixels_of_the_stroke(&events_2);
        let set: HashSet<(u32, u32)> = pixels_2.iter().copied().collect();
        let expected_2: HashSet<(u32, u32)> = (0..=24).map(|x| (x, 4)).collect();
        assert_eq!(pixels_2.len(), set.len(), "sin repetidos");
        assert_eq!(set, expected_2, "ppp {ppp}");
    }
}

#[test]
fn row_a7_a_stroke_that_leaves_on_the_right_and_comes_back_is_clipped_to_the_image() {
    for ppp in PPPS {
        let mut bench = Bench::with_image(ppp, TEST_SCREEN, &image_64x48());
        let info = bench.go_to_level(4);
        let source_2 = bench.geom(&info).source_2;
        // (14, 1) → (20, 1) → (13, 1): a 1/4, (56, 4), (80, 4) y (52, 4); la imagen acaba en 63.
        let events_2 = gesture_2(&mut bench, source_2, (14, 1), &[(20, 1), (13, 1)]);
        assert_eq!(
            events_2,
            vec![
                CanvasEvent::StrokeBegin((56, 4)),
                CanvasEvent::StrokeMove((80, 4)),
                CanvasEvent::StrokeMove((52, 4)),
                CanvasEvent::StrokeEnd
            ],
            "ppp {ppp}"
        );
        let set: HashSet<(u32, u32)> = pixels_of_the_stroke(&events_2).into_iter().collect();
        let expected_2: HashSet<(u32, u32)> = (52..=63).map(|x| (x, 4)).collect();
        assert_eq!(set, expected_2, "ppp {ppp}");
    }
}

// ---------------------------------------------------------------------------------------------
// A8 · Menú y columna
// ---------------------------------------------------------------------------------------------

fn state_2(zoom: Option<Zoom>) -> UiState {
    UiState {
        can_undo: true,
        can_redo: true,
        zoom,
        tool: Tool::Pencil,
        locks: Locks::default(),
    }
}

fn level(index_2: usize) -> Zoom {
    Zoom::levels().nth(index_2).expect("nivel de la escalera")
}

#[test]
fn row_a8_the_zoom_actions_along_the_ladder() {
    // Zoom::MIN, 1/2, ×1, ×5 y Zoom::MAX.
    let cases: [(&str, Zoom); 5] = [
        ("1/64", Zoom::MIN),
        ("1/2", level(5)),
        ("×1", Zoom::ONE),
        ("×5", Zoom::from_int(5).expect("×5")),
        ("×64", Zoom::MAX),
    ];
    for (name_2, zoom) in cases {
        let s = state_2(Some(zoom));
        assert_eq!(
            enabled(AppAction::ZoomOut, &s),
            name_2 != "1/64",
            "ZoomOut con {name_2}"
        );
        assert_eq!(
            enabled(AppAction::ZoomIn, &s),
            name_2 != "×64",
            "ZoomIn con {name_2}"
        );
        assert_eq!(
            enabled(AppAction::ZoomActualSize, &s),
            name_2 != "×1",
            "ZoomActual con {name_2}"
        );
        assert!(enabled(AppAction::ZoomFit, &s), "ZoomFit con {name_2}");
    }
    // Los niveles de la escalera, todos: Alejar solo se apaga en el mínimo y Acercar solo en el máximo.
    for (i, zoom) in Zoom::levels().enumerate() {
        let s = state_2(Some(zoom));
        assert_eq!(enabled(AppAction::ZoomOut, &s), i != 0, "{zoom}");
        assert_eq!(enabled(AppAction::ZoomIn, &s), i != 69, "{zoom}");
        assert_eq!(enabled(AppAction::ZoomActualSize, &s), i != 6, "{zoom}");
        assert!(enabled(AppAction::ZoomFit, &s), "{zoom}");
    }
    // Sin imagen, ninguna.
    let s = state_2(None);
    for action_2 in [
        AppAction::ZoomOut,
        AppAction::ZoomIn,
        AppAction::ZoomActualSize,
        AppAction::ZoomFit,
    ] {
        assert!(!enabled(action_2, &s), "{action_2:?} sin imagen");
    }
}

#[test]
fn row_a8_the_locks_of_the_17_do_not_change() {
    let zoom = Some(level(5));
    let with = |f: fn(&mut Locks)| {
        let mut locks = Locks::default();
        f(&mut locks);
        UiState {
            locks,
            ..state_2(zoom)
        }
    };
    // El zoom solo se bloquea con un campo de texto y desde el teclado; con un gesto, no.
    for action_2 in [
        AppAction::ZoomIn,
        AppAction::ZoomOut,
        AppAction::ZoomActualSize,
        AppAction::ZoomFit,
    ] {
        let text_2 = Locks {
            text_focus: true,
            ..Locks::default()
        };
        assert!(
            is_blocked(action_2, text_2, Origin::Key),
            "{action_2:?} con texto desde el teclado"
        );
        assert!(
            !is_blocked(action_2, text_2, Origin::Menu),
            "{action_2:?} con texto desde el menú"
        );
        let gesture_2 = Locks {
            gesture: true,
            ..Locks::default()
        };
        assert!(!is_blocked(action_2, gesture_2, Origin::Key));
        assert!(
            enabled(action_2, &with(|l| l.gesture = true)) == enabled(action_2, &state_2(zoom))
        );
        assert!(
            !enabled(action_2, &with(|l| l.dialog = true)),
            "{action_2:?} con un diálogo"
        );
        assert!(
            !enabled(action_2, &with(|l| l.modal = true)),
            "{action_2:?} con un modal"
        );
    }
}

#[test]
fn row_a8_the_level_label_is_zoom_and_the_display() {
    assert_eq!(zoom_label(Some(level(5))), "Zoom ×1/2");
    assert_eq!(zoom_label(Some(Zoom::MIN)), "Zoom ×1/64");
    assert_eq!(
        zoom_label(Some(Zoom::from_int(12).expect("×12"))),
        "Zoom ×12"
    );
    assert_eq!(zoom_label(Some(Zoom::ONE)), "Zoom ×1");
    assert_eq!(zoom_label(Some(Zoom::MAX)), "Zoom ×64");
    assert_eq!(zoom_label(None), "Zoom —");
    for zoom in Zoom::levels() {
        assert_eq!(zoom_label(Some(zoom)), format!("Zoom {zoom}"));
    }
    // 10 caracteres como mucho en la escalera entera.
    assert_eq!(zoom_label(Some(Zoom::MIN)).chars().count(), 10);
    assert!(Zoom::levels().all(|z| zoom_label(Some(z)).chars().count() <= 10));
}

/// Un fotograma de la columna sola con el rótulo dado; lo pintado en el segundo (el estable).
fn column_with(ctx: &Context, label_2: &str) -> common::Painted {
    let screen = vec2(400.0, 600.0);
    let s = state_2(Some(Zoom::ONE));
    let mut last_2 = None;
    for _ in 0..2 {
        let shapes_2 = frame(ctx, screen, vec![], |ui| {
            toolbox_2(ui, &s, label_2);
        });
        last_2 = Some(painted(&shapes_2));
    }
    last_2.expect("dos fotogramas")
}

#[test]
fn row_a8_zoom_1_2_zoom_1_64_and_zoom_x12_fit_in_the_column_at_the_five_scales() {
    let levels = [level(5), Zoom::MIN, Zoom::from_int(12).expect("×12")];
    for ppp in PPPS {
        let n = n_of(ppp);
        // La fuente real, con `MissingGlyphPolicy::Panic`: un glifo ausente (`×`) sería un pánico.
        let ctx = context(ppp);
        for zoom in levels {
            let label_2 = zoom_label(Some(zoom));
            let painted = column_with(&ctx, &label_2);
            let text_2 = painted.text_with(&label_2).unwrap_or_else(|| {
                panic!(
                    "ppp {ppp}: no se pinta `{label_2}`; hay {:?}",
                    painted.all_the_texts()
                )
            });
            assert_eq!(
                text_2.text_2, label_2,
                "ppp {ppp}: el texto se pinta entero, sin recorte"
            );
            // La columna va pegada al borde derecho de la pantalla de 400 × 600 pt.
            let toolbox_x = toolbox_single(
                ((400.0 * ppp).round() as i64, (600.0 * ppp).round() as i64),
                n as u32,
            )
            .x;
            let limit =
                (toolbox_x + (column_width_u(COLUMN_DEFAULT_COLS) - TOOLBAR_MARGIN_U) * n) as f32;
            assert!(
                text_2.rect().max.x * ppp <= limit + tolerance_of_egui(ppp),
                "ppp {ppp}: `{label_2}` termina en {} px y la rejilla de la caja acaba en {limit}",
                text_2.rect().max.x * ppp
            );
            assert!(
                text_2.rect().max.x <= text_2.clip.max.x + 1e-3,
                "ppp {ppp}: `{label_2}` queda fuera del recorte de la columna ({:?} frente a {:?})",
                text_2.rect(),
                text_2.clip
            );
            // 10 caracteres como mucho: 70 u de avance caben en los 225 u de la rejilla.
            assert!(label_2.chars().count() <= 10);
        }
    }
}

// ---------------------------------------------------------------------------------------------
// A9 · Superposición a zoom < 1
// ---------------------------------------------------------------------------------------------

/// Las celdas `K×K` de la rejilla (filas 1 y 2, hasta 12 columnas), rellenas **enteras**, con un
/// color por columna: rojo, transparente (el borrador) y rojo con alfa 128.
fn pending_for(k: i64) -> Vec<TestPending> {
    let colors_2 = [RED, TRANSPARENT, RED_MIDDLE];
    let columns_2 = ((256 + k - 1) / k).min(12);
    let mut v = Vec::new();
    for gx in 0..columns_2 {
        for gy in 1..=2i64 {
            for dy in 0..k {
                for dx in 0..k {
                    v.push((
                        (gx * k + dx) as u32,
                        (gy * k + dy) as u32,
                        colors_2[(gx % 3) as usize],
                    ));
                }
            }
        }
    }
    v
}

struct Failures {
    total: usize,
    first_2: Vec<String>,
}

impl Failures {
    fn notes(&mut self, message: impl FnOnce() -> String) {
        self.total += 1;
        if self.first_2.len() < 5 {
            self.first_2.push(message());
        }
    }
}

/// Todos los píxeles físicos del lienzo se ven como el oráculo: dentro de la imagen, el píxel `s·K`
/// de la imagen (o el pendiente, que lo sustituye, solo si cae en la rejilla) sobre el fondo
/// elegido; fuera, `BACKGROUND`.
fn compares_with_the_oracle(
    buffer: &Buffer,
    g: &Geom,
    k: i64,
    image_2: &TestImage,
    pending_2: &[TestPending],
    background_2: CanvasBackground,
    label_2: &str,
) -> Failures {
    let map: HashMap<(u32, u32), Rgba> = pending_2.iter().map(|&(x, y, c)| ((x, y), c)).collect();
    let ext = (
        (i64::from(image_2.width_2) + k - 1) / k,
        (i64::from(image_2.height_2) + k - 1) / k,
    );
    let mut failures = Failures {
        total: 0,
        first_2: Vec::new(),
    };
    for py in 0..buffer.height_2 as i64 {
        for px in 0..buffer.width_2 as i64 {
            let (sx, sy) = (px - g.source_2.0, py - g.source_2.1);
            let inside = (0..ext.0).contains(&sx) && (0..ext.1).contains(&sy);
            let expected_2 = if inside {
                let (ix, iy) = ((sx * k) as u32, (sy * k) as u32);
                let color = map
                    .get(&(ix, iy))
                    .copied()
                    .unwrap_or_else(|| image_2.pixel(ix, iy));
                composes(
                    background_in(background_2, px, py, g.source_2, g.cell),
                    color,
                )
            } else {
                Seen::of_color(BACKGROUND)
            };
            let seen = buffer.at(px, py);
            if !seen.near(expected_2, TOLERANCE) {
                failures.notes(|| {
                    format!(
                        "{label_2}: el píxel físico ({px}, {py}) se ve {seen:?} y debía verse {expected_2:?} (dentro de la imagen: {inside})"
                    )
                });
            }
        }
    }
    failures
}

/// Resumen de la estructura de un fotograma (cada forma: tipo, textura, caja y vértices, a una
/// milésima de punto). Dos fotogramas con la misma firma tienen las mismas formas: la
/// superposición, que es parte de la textura de la baldosa, **no añade ni cambia ninguna** (ni
/// rectángulos de color o de fondo por píxel pendiente, ni cuadriláteros del damero, ni mallas).
fn signature(layers: &[Layer]) -> Vec<String> {
    let thousandths = |r: Rect| {
        [r.min.x, r.min.y, r.max.x, r.max.y].map(|c| (f64::from(c) * 1000.0).round() as i64)
    };
    layers
        .iter()
        .map(|layer_2| match layer_2 {
            Layer::TestFill { rect } => format!("relleno {:?}", thousandths(*rect)),
            Layer::TestMesh {
                texture_2,
                vertices,
                ..
            } => format!(
                "malla {texture_2:?} {:?} {} vértices",
                thousandths(toolbox_pos(vertices)),
                vertices.len()
            ),
        })
        .collect()
}

/// Ninguna forma de área cero: ni rectángulos vacíos ni triángulos degenerados, antes y después de
/// teselar.
fn without_zero_area_shapes(
    shapes_2: &[ClippedShape],
    primitives: &[ClippedPrimitive],
    ppp: f32,
    label_2: &str,
) {
    for layer_2 in layers_of(shapes_2) {
        match layer_2 {
            Layer::TestFill { rect } => assert!(
                rect.width() > 0.0 && rect.height() > 0.0,
                "{label_2}: rectángulo de área cero {rect:?}"
            ),
            Layer::TestMesh {
                vertices, indices, ..
            } => {
                if let Some(area) = smaller_area_physical(&vertices, &indices, ppp) {
                    assert!(
                        area > 1e-6,
                        "{label_2}: triángulo degenerado (área {area} px²)"
                    );
                }
            }
        }
    }
    for primitive in primitives {
        if let Primitive::Mesh(mesh) = &primitive.primitive
            && let Some(area) = smaller_area_physical(&mesh.vertices, &mesh.indices, ppp)
        {
            assert!(
                area > 1e-6,
                "{label_2}: triángulo degenerado tras teselar (área {area} px²)"
            );
        }
    }
}

/// Un banco a `ppp`, con el degradado de 256×192 a 1/`k` y el fondo elegido.
fn bench_a_9(ppp: f32, k: i64, background_2: CanvasBackground, mesh: bool) -> Bench {
    let image_2 = TestImage::gradient(256, 192);
    let index_2 = 6 - (k as u32).trailing_zeros() as usize;
    let mut bench = Bench::new_2(ppp, SCREEN_A_9, image_2).with_background(background_2, mesh);
    bench.go_to_level(index_2);
    bench
}

fn checks_a_9(ppp: f32, k: i64, background_2: CanvasBackground, mesh: bool, with_overlay: bool) {
    let mut bench = bench_a_9(ppp, k, background_2, mesh);
    let pending_2 = if with_overlay {
        pending_for(k)
    } else {
        Vec::new()
    };
    // El fotograma de referencia (sin superposición) y el de la prueba, tras `set_overlay`.
    let without = bench.runs(vec![], &[]);
    let output_2 = bench.runs(vec![], &pending_2);
    let label_2 = format!(
        "ppp {ppp} ×1/{k} fondo {background_2:?}{}{}",
        if mesh { " (malla)" } else { "" },
        if with_overlay {
            ""
        } else {
            " sin superposición"
        }
    );
    assert_eq!(
        output_2.info.zoom.to_string(),
        format!("×1/{k}"),
        "{label_2}"
    );
    // La textura compuesta de la baldosa es la imagen con los píxeles pendientes escritos (el
    // texel nuevo es el color pendiente premultiplicado; el viejo no está).
    let image_2 = bench.image_2.as_ref().expect("imagen del banco");
    let mut texels = image_2.texels();
    for &(x, y, c) in &pending_2 {
        texels[(y * image_2.width_2 + x) as usize] =
            Color32::from_rgba_unmultiplied(c.r, c.g, c.b, c.a);
    }
    let composite = bench
        .composite_2
        .get(bench.canvas.texture_id().expect("hay textura"))
        .expect("la baldosa se subió");
    assert_eq!(
        composite.size_2,
        [image_2.width_2 as usize, image_2.height_2 as usize],
        "{label_2}"
    );
    assert!(
        composite.texels == texels,
        "{label_2}: la textura compuesta no es la imagen con los píxeles pendientes escritos"
    );
    // Ninguna forma de superposición: las formas son las del fotograma sin ella.
    assert_eq!(
        signature(&layers_of(&output_2.shapes_2)),
        signature(&layers_of(&without.shapes_2)),
        "{label_2}: la superposición no debe añadir ni cambiar ninguna forma"
    );
    let primitives = bench.ctx.tessellate(output_2.shapes_2.clone(), ppp);
    without_zero_area_shapes(&output_2.shapes_2, &primitives, ppp, &label_2);
    let viewport = bench.viewport();
    let buffer = rasterizes(
        &primitives,
        ppp,
        viewport.width as usize,
        viewport.height as usize,
        &bench.textures_2(),
    );
    let g = bench.geom(&output_2.info);
    let image_2 = bench.image_2.as_ref().expect("imagen del banco");
    let failures =
        compares_with_the_oracle(&buffer, &g, k, image_2, &pending_2, background_2, &label_2);
    assert_eq!(
        failures.total, 0,
        "{label_2}: {} píxeles físicos mal; los primeros: {:#?}",
        failures.total, failures.first_2
    );
}

#[test]
fn row_a9_control_without_overlay_each_screen_pixel_shows_the_pixel_s_by_k() {
    for ppp in PPPS {
        for k in [2, 4, 64] {
            checks_a_9(ppp, k, CanvasBackground::Checker, false, false);
        }
    }
}

#[test]
fn row_a9_only_the_pixels_of_the_grid_are_seen_each_one_1x1_with_the_four_backgrounds() {
    for ppp in PPPS {
        for k in [2, 4, 64] {
            for background_2 in BACKGROUNDS {
                checks_a_9(ppp, k, background_2, false, true);
            }
        }
    }
}

#[test]
fn row_a9_with_the_mesh_fallback_the_checker_looks_the_same() {
    // `PIXOKO_CHECKER_MESH=1` en la app es `Canvas::set_checker_mesh(true)`.
    for ppp in PPPS {
        for k in [2, 4, 64] {
            checks_a_9(ppp, k, CanvasBackground::Checker, true, true);
        }
    }
}

#[test]
fn row_a9_the_pixels_outside_the_grid_leave_no_visible_shape() {
    // Una superposición solo de píxeles fuera de la rejilla (ni `x` ni `y` múltiplos de K) no cambia
    // lo que se ve respecto a no tener superposición.
    for ppp in PPPS {
        for k in [2i64, 4, 64] {
            let outside: Vec<TestPending> = (0..12)
                .map(|i| ((i * k + 1) as u32, (k + 1) as u32, RED))
                .chain((0..12).map(|i| ((i * k) as u32, (k + 1) as u32, TRANSPARENT)))
                .chain((0..12).map(|i| ((i * k + 1) as u32, k as u32, RED_MIDDLE)))
                .filter(|&(x, y, _)| x < 256 && y < 192)
                .collect();
            let mut bench = bench_a_9(ppp, k, CanvasBackground::Checker, false);
            let viewport = bench.viewport();
            let render = |bench: &mut Bench, overlay: &[TestPending]| {
                let output_2 = bench.runs(vec![], overlay);
                let primitives = bench.ctx.tessellate(output_2.shapes_2.clone(), ppp);
                let buffer = rasterizes(
                    &primitives,
                    ppp,
                    viewport.width as usize,
                    viewport.height as usize,
                    &bench.textures_2(),
                );
                (buffer, signature(&layers_of(&output_2.shapes_2)))
            };
            let (without, shapes_without) = render(&mut bench, &[]);
            let (with, shapes_with) = render(&mut bench, &outside);
            assert_eq!(
                shapes_without, shapes_with,
                "ppp {ppp} ×1/{k}: la superposición no añade ninguna forma"
            );
            for (i, (a, b)) in without.pixels_2.iter().zip(&with.pixels_2).enumerate() {
                assert!(
                    a.near(*b, TOLERANCE),
                    "ppp {ppp} ×1/{k}: el píxel físico ({}, {}) se ve distinto con píxeles fuera de la rejilla ({a:?} frente a {b:?})",
                    i % without.width_2,
                    i / without.width_2
                );
            }
        }
    }
}
