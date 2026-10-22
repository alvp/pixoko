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
//!   `set_checker_mesh(true)` (lo que activa `PIXOKO_DAMERO_MALLA=1` en la app).
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
//!   del botón (`TOOLBAR_MARGIN_U + toolbar_button_width_u()` `u` desde el borde de la columna).
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
use px_ui::chrome::{TOOLBAR_MARGIN_U, toolbar_button_width_u};
use px_ui::palette_grid::{CHECK_A, CHECK_B};
use px_ui::stroke::Stroke;
use px_ui::text::text_scale;
use px_ui::tool::Tool;
use px_ui::toolbar::zoom_label;
use px_ui::view::{Size, ViewOp, Zoom};

mod common;
use common::toolbox::caja;
use common::zones::caja_sola;
use common::{
    PPPS, boton, contexto, en_puntos, fotograma, mover, n_de, pintado, rueda_linea, tecla,
    tolerancia_de_egui,
};

/// Pantalla de A6, A7 y P1: la imagen de 64×48 ocupa el centro.
const PANTALLA: Vec2 = vec2(800.0, 600.0);
/// Pantalla de A9: a 1/2 la imagen de 256×192 mide 128×96 píxeles físicos y cabe a `ppp` 1.
const PANTALLA_A9: Vec2 = vec2(160.0, 120.0);
const TOLERANCIA: f32 = 0.02;
const DESPLAZO_X: f64 = 0.0013;
const DESPLAZO_Y: f64 = 0.0007;

const ROJO: Rgba = Rgba::opaque(255, 0, 0);
const TRANSPARENTE: Rgba = Rgba::new(0, 0, 0, 0);
const ROJO_MEDIO: Rgba = Rgba::new(255, 0, 0, 128);

const FONDOS: [CanvasBackground; 4] = [
    CanvasBackground::Checker,
    CanvasBackground::Black,
    CanvasBackground::Gray,
    CanvasBackground::White,
];

/// Un píxel pendiente: posición y color (el que tendrá en la imagen).
type Pendiente = (u32, u32, Rgba);

// ---------------------------------------------------------------------------------------------
// Oráculo (fracciones e i128, sin llamar a las funciones bajo prueba)
// ---------------------------------------------------------------------------------------------

type Fraccion = (i128, i128);

/// Los 70 niveles de la decisión 2, de menor a mayor.
fn escalera() -> Vec<Fraccion> {
    let mut niveles: Vec<Fraccion> = [64, 32, 16, 8, 4, 2].iter().map(|&k| (1, k)).collect();
    niveles.extend((1..=64).map(|n| (n, 1)));
    niveles
}

fn rotulo((num, den): Fraccion) -> String {
    if den == 1 {
        format!("×{num}")
    } else {
        format!("×{num}/{den}")
    }
}

fn suelo(n: i128, d: i128) -> i128 {
    let q = n / d;
    if n % d != 0 && n < 0 { q - 1 } else { q }
}

fn techo(n: i128, d: i128) -> i128 {
    -suelo(-n, d)
}

fn o_extent(f: Fraccion, n: i64) -> i64 {
    techo(i128::from(n) * f.0, f.1) as i64
}

/// Píxel de imagen bajo el punto `p` de un eje, o `None`.
fn o_eje(f: Fraccion, p: i64, origen: i64, dim: u32) -> Option<u32> {
    let d = i128::from(p) - i128::from(origen);
    if d < 0 {
        return None;
    }
    let indice = suelo(d * f.1, f.0);
    (indice < i128::from(dim)).then_some(indice as u32)
}

/// Lo mismo, con signo y sin recortar a la imagen (lo que emite `StrokeBegin`).
fn o_eje_i32(f: Fraccion, p: i64, origen: i64) -> i32 {
    let d = i128::from(p) - i128::from(origen);
    suelo(d * f.1, f.0).clamp(i128::from(i32::MIN), i128::from(i32::MAX)) as i32
}

fn o_hover(f: Fraccion, p: (i64, i64), origen: (i64, i64), tam: (u32, u32)) -> Option<(u32, u32)> {
    Some((
        o_eje(f, p.0, origen.0, tam.0)?,
        o_eje(f, p.1, origen.1, tam.1)?,
    ))
}

fn o_stroke(f: Fraccion, p: (i64, i64), origen: (i64, i64)) -> (i32, i32) {
    (o_eje_i32(f, p.0, origen.0), o_eje_i32(f, p.1, origen.1))
}

/// Índice del mayor nivel con `extent ≤ lienzo` en los dos ejes (0 si ninguno cabe).
fn o_mejor_nivel(viewport: Size, image: (u32, u32)) -> usize {
    let niveles = escalera();
    (0..niveles.len())
        .rev()
        .find(|&i| {
            o_extent(niveles[i], i64::from(image.0)) <= i64::from(viewport.width)
                && o_extent(niveles[i], i64::from(image.1)) <= i64::from(viewport.height)
        })
        .unwrap_or(0)
}

/// Granularidad del nivel `indice`: su `K` si es fraccionario y 1 si no.
fn rejilla(indice: usize) -> i32 {
    if indice < 6 { 1 << (6 - indice) } else { 1 }
}

/// El píxel bajo el ancla tras pasar a un nivel con la granularidad `g` (decisión 4).
fn sube_a_la_rejilla(pixel: (u32, u32), g: i32) -> (u32, u32) {
    let g = g as u32;
    (pixel.0 / g * g, pixel.1 / g * g)
}

// ---------------------------------------------------------------------------------------------
// Imagen de la prueba
// ---------------------------------------------------------------------------------------------

#[derive(Clone)]
struct Imagen {
    ancho: u32,
    alto: u32,
    pixeles: Vec<Rgba>,
}

impl Imagen {
    /// Cada píxel tiene un color distinto según su posición (opaco).
    fn degradado(ancho: u32, alto: u32) -> Imagen {
        let mut pixeles = Vec::new();
        for y in 0..alto {
            for x in 0..ancho {
                pixeles.push(Rgba::opaque(
                    (x % 256) as u8,
                    (y % 256) as u8,
                    ((x * 5 + y * 3) % 251) as u8,
                ));
            }
        }
        Imagen {
            ancho,
            alto,
            pixeles,
        }
    }

    fn pixel(&self, x: u32, y: u32) -> Rgba {
        self.pixeles[(y * self.ancho + x) as usize]
    }

    fn image(&self) -> Image {
        let mut image = Image::new(self.ancho, self.alto, Mode::Rgba).expect("imagen válida");
        for y in 0..self.alto {
            for x in 0..self.ancho {
                image
                    .set_pixel(x, y, Pixel::Rgba(self.pixel(x, y)))
                    .expect("dentro");
            }
        }
        image
    }

    fn texels(&self) -> Vec<Color32> {
        self.pixeles
            .iter()
            .map(|c| Color32::from_rgba_unmultiplied(c.r, c.g, c.b, c.a))
            .collect()
    }
}

fn imagen_64x48() -> Image {
    Image::new(64, 48, Mode::Rgba).expect("imagen válida")
}

// ---------------------------------------------------------------------------------------------
// Color
// ---------------------------------------------------------------------------------------------

/// Lo que se ve: color premultiplicado, como egui.
#[derive(Clone, Copy, Debug)]
struct Visto {
    r: f32,
    g: f32,
    b: f32,
    a: f32,
}

impl Visto {
    const VACIO: Visto = Visto {
        r: 0.0,
        g: 0.0,
        b: 0.0,
        a: 0.0,
    };

    fn de_color(c: Color32) -> Visto {
        Visto {
            r: f32::from(c.r()) / 255.0,
            g: f32::from(c.g()) / 255.0,
            b: f32::from(c.b()) / 255.0,
            a: f32::from(c.a()) / 255.0,
        }
    }

    /// `src` por encima de `self` (alfa premultiplicado).
    fn debajo_de(self, src: Visto) -> Visto {
        let k = 1.0 - src.a;
        Visto {
            r: src.r + self.r * k,
            g: src.g + self.g * k,
            b: src.b + self.b * k,
            a: src.a + self.a * k,
        }
    }

    fn por(self, otro: Visto) -> Visto {
        Visto {
            r: self.r * otro.r,
            g: self.g * otro.g,
            b: self.b * otro.b,
            a: self.a * otro.a,
        }
    }

    fn cerca(&self, otro: Visto, tolerancia: f32) -> bool {
        (self.r - otro.r).abs() <= tolerancia
            && (self.g - otro.g).abs() <= tolerancia
            && (self.b - otro.b).abs() <= tolerancia
            && (self.a - otro.a).abs() <= tolerancia
    }
}

fn gris(c: Rgba) -> Color32 {
    Color32::from_rgb(c.r, c.g, c.b)
}

/// Color del fondo elegido bajo la transparencia en el píxel físico `(px, py)`: el oráculo de la
/// 17b (`background_fill` y `shade_at`).
fn fondo_en(fondo: CanvasBackground, px: i64, py: i64, origen: (i64, i64), cell: i64) -> Color32 {
    match background_fill(fondo) {
        Fill::Checker => gris(shade_at(px, py, origen, cell).color()),
        Fill::Solid(color) => gris(color),
    }
}

/// Un color con alfa directo por encima de un fondo opaco.
fn compone(fondo: Color32, color: Rgba) -> Visto {
    let encima = Color32::from_rgba_unmultiplied(color.r, color.g, color.b, color.a);
    Visto::de_color(fondo).debajo_de(Visto::de_color(encima))
}

// ---------------------------------------------------------------------------------------------
// Rasterizador a mano (el de `checker_canvas.rs`)
// ---------------------------------------------------------------------------------------------

fn baricentricas(p: (f64, f64), a: (f64, f64), b: (f64, f64), c: (f64, f64)) -> Option<[f64; 3]> {
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
struct Texturas {
    imagen: TextureId,
    ancho: usize,
    alto: usize,
    texels: Vec<Color32>,
    damero: Option<TextureId>,
}

/// Texel de la textura 2×2 del damero (por filas `A, B, B, A`) con `NEAREST` y envoltura `Repeat`.
fn texel_del_damero(u: f64, v: f64) -> Visto {
    let tx = ((u * 2.0).floor() as i64).rem_euclid(2);
    let ty = ((v * 2.0).floor() as i64).rem_euclid(2);
    let c = if (tx + ty) % 2 == 0 { CHECK_A } else { CHECK_B };
    Visto::de_color(gris(c))
}

struct Buffer {
    ancho: usize,
    alto: usize,
    pixeles: Vec<Visto>,
}

impl Buffer {
    fn en(&self, x: i64, y: i64) -> Visto {
        assert!(
            x >= 0 && y >= 0 && (x as usize) < self.ancho && (y as usize) < self.alto,
            "({x}, {y}) fuera del búfer de {}×{}",
            self.ancho,
            self.alto
        );
        self.pixeles[y as usize * self.ancho + x as usize]
    }
}

/// Pinta `primitivas`, en orden, en un búfer de `ancho × alto` píxeles físicos, muestreando cada
/// píxel en su centro (algo desplazado). La textura de la imagen se muestrea con `NEAREST` en su uv
/// interpolado, **sin** envoltura (se recorta al borde); la del damero, con envoltura; las mallas
/// de colores de vértice y los rellenos usan el color de sus vértices.
fn rasteriza(
    primitivas: &[ClippedPrimitive],
    ppp: f32,
    ancho: usize,
    alto: usize,
    tex: &Texturas,
) -> Buffer {
    let ppp = f64::from(ppp);
    let mut pixeles = vec![Visto::VACIO; ancho * alto];
    for primitiva in primitivas {
        let Primitive::Mesh(malla) = &primitiva.primitive else {
            continue;
        };
        let clip = primitiva.clip_rect;
        let (cx0, cy0) = (f64::from(clip.min.x) * ppp, f64::from(clip.min.y) * ppp);
        let (cx1, cy1) = (f64::from(clip.max.x) * ppp, f64::from(clip.max.y) * ppp);
        let es_imagen = malla.texture_id == tex.imagen;
        let es_damero = tex.damero == Some(malla.texture_id);
        for triangulo in malla.indices.as_chunks::<3>().0 {
            let v = [
                &malla.vertices[triangulo[0] as usize],
                &malla.vertices[triangulo[1] as usize],
                &malla.vertices[triangulo[2] as usize],
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
                .min(ancho as f64)
                .max(0.0) as usize;
            let y1 = (max_y.ceil() + 1.0)
                .min(cy1.ceil())
                .min(alto as f64)
                .max(0.0) as usize;
            let area =
                (p[1].0 - p[0].0) * (p[2].1 - p[0].1) - (p[2].0 - p[0].0) * (p[1].1 - p[0].1);
            if area.abs() < 1e-12 {
                continue;
            }
            let tintes = v.map(|v| Visto::de_color(v.color));
            let plano =
                !es_imagen && !es_damero && v[0].color == v[1].color && v[1].color == v[2].color;
            let aristas = [(p[0], p[1]), (p[1], p[2]), (p[2], p[0])];
            for py in y0..y1 {
                let sy = py as f64 + 0.5 + DESPLAZO_Y;
                if sy < cy0 || sy > cy1 {
                    continue;
                }
                let (mut izq, mut der) = (f64::INFINITY, f64::NEG_INFINITY);
                for (a, b) in aristas {
                    let (bajo, alto_) = if a.1 <= b.1 { (a, b) } else { (b, a) };
                    if sy < bajo.1 || sy > alto_.1 {
                        continue;
                    }
                    if alto_.1 == bajo.1 {
                        izq = izq.min(a.0.min(b.0));
                        der = der.max(a.0.max(b.0));
                    } else {
                        let t = (sy - bajo.1) / (alto_.1 - bajo.1);
                        let x = bajo.0 + t * (alto_.0 - bajo.0);
                        izq = izq.min(x);
                        der = der.max(x);
                    }
                }
                if izq > der {
                    continue;
                }
                let desde = ((izq.max(cx0) - 0.5 - DESPLAZO_X).ceil().max(x0 as f64)) as usize;
                let hasta = (der.min(cx1) - 0.5 - DESPLAZO_X)
                    .floor()
                    .min(x1 as f64 - 1.0);
                if hasta < 0.0 {
                    continue;
                }
                for px in desde..=(hasta as usize) {
                    let s = (px as f64 + 0.5 + DESPLAZO_X, sy);
                    let color = if plano {
                        tintes[0]
                    } else {
                        let Some(w) = baricentricas(s, p[0], p[1], p[2]) else {
                            continue;
                        };
                        let wf = w.map(|w| w as f32);
                        let tinte = Visto {
                            r: wf[0] * tintes[0].r + wf[1] * tintes[1].r + wf[2] * tintes[2].r,
                            g: wf[0] * tintes[0].g + wf[1] * tintes[1].g + wf[2] * tintes[2].g,
                            b: wf[0] * tintes[0].b + wf[1] * tintes[1].b + wf[2] * tintes[2].b,
                            a: wf[0] * tintes[0].a + wf[1] * tintes[1].a + wf[2] * tintes[2].a,
                        };
                        if es_imagen || es_damero {
                            let u = w[0] * f64::from(v[0].uv.x)
                                + w[1] * f64::from(v[1].uv.x)
                                + w[2] * f64::from(v[2].uv.x);
                            let t = w[0] * f64::from(v[0].uv.y)
                                + w[1] * f64::from(v[1].uv.y)
                                + w[2] * f64::from(v[2].uv.y);
                            let texel = if es_imagen {
                                let tx = ((u * tex.ancho as f64).floor().max(0.0) as usize)
                                    .min(tex.ancho - 1);
                                let ty = ((t * tex.alto as f64).floor().max(0.0) as usize)
                                    .min(tex.alto - 1);
                                Visto::de_color(tex.texels[ty * tex.ancho + tx])
                            } else {
                                texel_del_damero(u, t)
                            };
                            texel.por(tinte)
                        } else {
                            tinte
                        }
                    };
                    let destino = &mut pixeles[py * ancho + px];
                    *destino = destino.debajo_de(color);
                }
            }
        }
    }
    Buffer {
        ancho,
        alto,
        pixeles,
    }
}

// ---------------------------------------------------------------------------------------------
// Lo pintado, por capas (formas sin teselar)
// ---------------------------------------------------------------------------------------------

#[derive(Clone, Debug)]
enum Capa {
    Relleno {
        rect: Rect,
    },
    Malla {
        textura: TextureId,
        vertices: Vec<Vertex>,
        indices: Vec<u32>,
    },
}

fn recoge(shape: &Shape, salida: &mut Vec<Capa>) {
    match shape {
        Shape::Rect(r) => salida.push(Capa::Relleno { rect: r.rect }),
        Shape::Mesh(m) => salida.push(Capa::Malla {
            textura: m.texture_id,
            vertices: m.vertices.clone(),
            indices: m.indices.clone(),
        }),
        Shape::Vec(formas) => {
            for forma in formas {
                recoge(forma, salida);
            }
        }
        _ => {}
    }
}

fn capas_de(formas: &[ClippedShape]) -> Vec<Capa> {
    let mut v = Vec::new();
    for clipped in formas {
        recoge(&clipped.shape, &mut v);
    }
    v
}

fn caja_pos(vertices: &[Vertex]) -> Rect {
    let mut r = Rect::NOTHING;
    for v in vertices {
        r.extend_with(v.pos);
    }
    r
}

fn caja_uv(vertices: &[Vertex]) -> [f32; 4] {
    let mut r = Rect::NOTHING;
    for v in vertices {
        r.extend_with(v.uv);
    }
    [r.min.x, r.min.y, r.max.x, r.max.y]
}

/// Área de los triángulos de una malla, en píxeles físicos cuadrados (el menor de todos).
fn menor_area_fisica(vertices: &[Vertex], indices: &[u32], ppp: f32) -> Option<f64> {
    let k = f64::from(ppp);
    indices
        .as_chunks::<3>()
        .0
        .iter()
        .map(|t| {
            let [a, b, c] = t.map(|i| vertices[i as usize].pos);
            let cruz = f64::from(b.x - a.x) * f64::from(c.y - a.y)
                - f64::from(c.x - a.x) * f64::from(b.y - a.y);
            cruz.abs() / 2.0 * k * k
        })
        .reduce(f64::min)
}

// ---------------------------------------------------------------------------------------------
// El banco
// ---------------------------------------------------------------------------------------------

struct Salida {
    formas: Vec<ClippedShape>,
    info: CanvasInfo,
    entradas: Vec<CanvasInput>,
}

impl Salida {
    /// Solo los eventos de trazo, en orden.
    fn trazo(&self) -> Vec<CanvasEvent> {
        self.entradas
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
    origen: (i64, i64),
    viewport: Size,
    cell: i64,
}

struct Banco {
    ctx: Context,
    ppp: f32,
    pantalla: Vec2,
    canvas: Canvas,
    /// El documento (lo que `sync` y `set_overlay` reciben): no cambia en estas pruebas.
    documento: Image,
    /// Lo que la prueba sabe de la imagen (el oráculo), si la necesita.
    imagen: Option<Imagen>,
    /// El contenido de las texturas tal como egui lo entrega a la GPU: se aplican los
    /// `textures_delta` de **cada** fotograma, en orden.
    compuestas: common::textures::Texturas,
}

impl Banco {
    fn con_image(ppp: f32, pantalla: Vec2, image: &Image) -> Banco {
        let mut canvas = Canvas::new();
        canvas.set_image(image);
        let mut banco = Banco {
            ctx: contexto(ppp),
            ppp,
            pantalla,
            canvas,
            documento: image.clone(),
            imagen: None,
            compuestas: common::textures::Texturas::new(),
        };
        for _ in 0..3 {
            banco.reposo();
        }
        banco
    }

    fn nuevo(ppp: f32, pantalla: Vec2, imagen: Imagen) -> Banco {
        let mut banco = Banco::con_image(ppp, pantalla, &imagen.image());
        banco.imagen = Some(imagen);
        banco
    }

    fn con_fondo(mut self, fondo: CanvasBackground, malla: bool) -> Banco {
        self.canvas.set_background(fondo);
        self.canvas.set_checker_mesh(malla);
        for _ in 0..2 {
            self.reposo();
        }
        self
    }

    /// Un fotograma, en el orden de la app: `interact_ordered`, `set_overlay` con la superposición
    /// dada (el trazo pendiente entero; vacío la quita), `sync` y `paint`. Aplica los cambios de
    /// textura del fotograma (completos y parciales) antes de devolver las formas.
    fn corre(&mut self, events: Vec<Event>, overlay: &[Pendiente]) -> Salida {
        let entrada = RawInput {
            screen_rect: Some(Rect::from_min_size(Pos2::ZERO, self.pantalla)),
            events,
            ..RawInput::default()
        };
        let mut resultado = None;
        let canvas = &mut self.canvas;
        let documento = &self.documento;
        let mut salida = self.ctx.run_ui(entrada, |ui| {
            let entradas = canvas
                .interact_roles(ui)
                .into_iter()
                .map(|(input, _)| input)
                .collect::<Vec<_>>();
            canvas.set_overlay(ui.ctx(), documento, overlay);
            canvas
                .sync(ui.ctx(), documento)
                .expect("el documento se carga");
            let info = canvas.paint(ui);
            resultado = Some((info, entradas));
        });
        // Sin los cambios de la fuente de egui (`Managed(0)`): el banco no pinta texto.
        let mut delta: TexturesDelta = salida.textures_delta.clone();
        delta.set.remove(&TextureId::Managed(0));
        delta.free.remove(&TextureId::Managed(0));
        self.compuestas.aplica(&delta);
        // La copia no se entrega a la GPU: egui exige vaciarla antes de soltarla.
        delta.clear();
        salida.textures_delta.clear();
        let formas = std::mem::take(&mut salida.shapes);
        let (info, entradas) = resultado.expect("el lienzo se dibuja");
        Salida {
            formas,
            info,
            entradas,
        }
    }

    fn reposo(&mut self) -> CanvasInfo {
        self.corre(vec![], &[]).info
    }

    fn pide(&mut self, op: ViewOp) -> CanvasInfo {
        self.canvas.request_view(op);
        self.reposo()
    }

    /// Lleva el zoom al nivel `indice` de la escalera (0 = 1/64, 6 = ×1, 69 = ×64) con peticiones de
    /// vista, empezando por ×1 anclado en el centro.
    fn al_nivel(&mut self, indice: usize) -> CanvasInfo {
        let mut info = self.pide(ViewOp::Actual);
        for _ in 6..indice {
            info = self.pide(ViewOp::ZoomIn);
        }
        for _ in indice..6 {
            info = self.pide(ViewOp::ZoomOut);
        }
        assert_eq!(
            info.zoom.to_string(),
            rotulo(escalera()[indice]),
            "ppp {}: el banco no llegó al nivel {indice}",
            self.ppp
        );
        info
    }

    /// Tamaño del lienzo en píxeles físicos (todo el espacio de `ui`).
    fn viewport(&self) -> Size {
        Size::new(
            (self.pantalla.x * self.ppp).round() as u32,
            (self.pantalla.y * self.ppp).round() as u32,
        )
    }

    fn geom(&self, info: &CanvasInfo) -> Geom {
        let rect = info.image_rect.expect("hay imagen");
        let ppp = self.ppp;
        Geom {
            origen: (
                (rect.min.x * ppp).round() as i64,
                (rect.min.y * ppp).round() as i64,
            ),
            viewport: self.viewport(),
            cell: 8 * i64::from(text_scale(ppp)),
        }
    }

    /// Posición en puntos de un píxel físico (más una fracción que no cambia el redondeo).
    fn pos(&self, x: i64, y: i64) -> Pos2 {
        en_puntos(self.ppp, x, y)
    }

    /// Las texturas para rasterizar: la **compuesta** de la baldosa 0 (la imagen de A9 cabe en una
    /// sola baldosa, y con la superposición ya escrita en ella) y la del damero.
    fn texturas(&self) -> Texturas {
        let id = self.canvas.texture_id().expect("hay textura de la imagen");
        let compuesta = self
            .compuestas
            .get(id)
            .expect("la baldosa se subió y sus cambios se aplicaron");
        Texturas {
            imagen: id,
            ancho: compuesta.tamano[0],
            alto: compuesta.tamano[1],
            texels: compuesta.texels.clone(),
            damero: self.canvas.checker_texture_id(),
        }
    }
}

// ---------------------------------------------------------------------------------------------
// A6 · Lienzo a las cinco escalas
// ---------------------------------------------------------------------------------------------

/// El puntero queda en el centro de la pantalla, sobre la imagen de 64×48 centrada.
fn puntero_central(ppp: f32) -> (Pos2, (i64, i64)) {
    let pos = pos2(PANTALLA.x / 2.0, PANTALLA.y / 2.0);
    (
        pos,
        ((pos.x * ppp).round() as i64, (pos.y * ppp).round() as i64),
    )
}

/// `image_rect` mide `extent/ppp` en cada eje (`extent` por el oráculo).
fn comprueba_rect(info: &CanvasInfo, ppp: f32, tam: (u32, u32), etiqueta: &str) {
    let f = escalera()[escalera()
        .iter()
        .position(|n| rotulo(*n) == info.zoom.to_string())
        .expect("nivel de la escalera")];
    let rect = info.image_rect.expect("hay imagen");
    let (ancho, alto) = (
        o_extent(f, i64::from(tam.0)) as f32,
        o_extent(f, i64::from(tam.1)) as f32,
    );
    assert!(
        (rect.width() * ppp - ancho).abs() <= 0.02 && (rect.height() * ppp - alto).abs() <= 0.02,
        "{etiqueta}: el rectángulo mide {}×{} px físicos y debía medir {ancho}×{alto}",
        rect.width() * ppp,
        rect.height() * ppp
    );
}

#[test]
fn fila_a6_la_rueda_baja_hasta_1_64_y_sube_hasta_x64_con_el_pixel_bajo_el_puntero() {
    let niveles = escalera();
    for ppp in PPPS {
        let mut banco = Banco::con_image(ppp, PANTALLA, &imagen_64x48());
        let (pos, fisico) = puntero_central(ppp);
        let info = banco.al_nivel(6);
        assert_eq!(info.zoom, Zoom::ONE);
        banco.corre(vec![mover(pos)], &[]);
        let mut pixel = banco
            .reposo()
            .hover
            .expect("el centro está sobre la imagen");
        let mut indice = 6usize;
        // Ocho muescas hacia fuera: 1/2, 1/4, …, 1/64 y satura (dos muescas de más).
        let mut pasos: Vec<bool> = vec![false; 8];
        // Y 70 hacia dentro: ×2, …, ×64 y satura (la escalera tiene 69 pasos).
        pasos.extend(vec![true; 70]);
        for (n, acerca) in pasos.into_iter().enumerate() {
            let salida = banco.corre(vec![rueda_linea(acerca)], &[]);
            indice = if acerca {
                (indice + 1).min(69)
            } else {
                indice.saturating_sub(1)
            };
            let etiqueta = format!("ppp {ppp} muesca {n} → {}", rotulo(niveles[indice]));
            assert_eq!(
                salida.info.zoom.to_string(),
                rotulo(niveles[indice]),
                "{etiqueta}"
            );
            assert_eq!(banco.canvas.zoom(), Some(salida.info.zoom), "{etiqueta}");
            // El píxel bajo el puntero sube a la rejilla del nivel nuevo (exacto con zoom ≥ ×1).
            pixel = sube_a_la_rejilla(pixel, rejilla(indice));
            assert_eq!(
                salida.info.hover,
                Some(pixel),
                "{etiqueta}: píxel bajo el puntero"
            );
            // Y es el del oráculo con el `physical()` de egui.
            let g = banco.geom(&salida.info);
            assert_eq!(
                salida.info.hover,
                o_hover(niveles[indice], fisico, g.origen, (64, 48)),
                "{etiqueta}: oráculo"
            );
            comprueba_rect(&salida.info, ppp, (64, 48), &etiqueta);
        }
        assert_eq!(banco.canvas.zoom(), Some(Zoom::MAX), "ppp {ppp}");
    }
}

#[test]
fn fila_a6_la_rueda_hacia_fuera_satura_en_1_64_y_a_mitad_de_camino_baja_un_nivel_por_muesca() {
    for ppp in PPPS {
        let mut banco = Banco::con_image(ppp, PANTALLA, &imagen_64x48());
        let (pos, _) = puntero_central(ppp);
        banco.al_nivel(6);
        banco.corre(vec![mover(pos)], &[]);
        let esperados = [
            "×1/2", "×1/4", "×1/8", "×1/16", "×1/32", "×1/64", "×1/64", "×1/64",
        ];
        for (n, esperado) in esperados.iter().enumerate() {
            let info = banco.corre(vec![rueda_linea(false)], &[]).info;
            assert_eq!(info.zoom.to_string(), *esperado, "ppp {ppp} muesca {n}");
        }
        assert_eq!(banco.canvas.zoom(), Some(Zoom::MIN));
        // A 1/64 la imagen de 64×48 es un solo píxel físico y sigue bajo el puntero.
        let info = banco.reposo();
        assert_eq!(info.hover, Some((0, 0)), "ppp {ppp}");
        comprueba_rect(&info, ppp, (64, 48), "1/64");
    }
}

#[test]
fn fila_a6_las_teclas_mas_y_menos_en_los_extremos_no_cambian_el_nivel() {
    for ppp in PPPS {
        let mut banco = Banco::con_image(ppp, PANTALLA, &imagen_64x48());
        banco.al_nivel(6);
        let info = banco
            .corre(vec![tecla(Key::Minus, Modifiers::NONE)], &[])
            .info;
        assert_eq!(info.zoom.to_string(), "×1/2", "ppp {ppp}");
        let info = banco
            .corre(vec![tecla(Key::Plus, Modifiers::NONE)], &[])
            .info;
        assert_eq!(info.zoom, Zoom::ONE, "ppp {ppp}");
        for _ in 0..8 {
            banco.corre(vec![tecla(Key::Minus, Modifiers::NONE)], &[]);
        }
        assert_eq!(banco.canvas.zoom(), Some(Zoom::MIN), "ppp {ppp}");
        let antes = banco.reposo().image_rect;
        let info = banco
            .corre(vec![tecla(Key::Minus, Modifiers::NONE)], &[])
            .info;
        assert_eq!(info.zoom, Zoom::MIN, "ppp {ppp}: `-` satura");
        assert_eq!(info.image_rect, antes, "ppp {ppp}");
        for _ in 0..80 {
            banco.corre(vec![tecla(Key::Equals, Modifiers::NONE)], &[]);
        }
        assert_eq!(banco.canvas.zoom(), Some(Zoom::MAX), "ppp {ppp}");
        let antes = banco.reposo().image_rect;
        let info = banco
            .corre(vec![tecla(Key::Plus, Modifiers::NONE)], &[])
            .info;
        assert_eq!(info.zoom, Zoom::MAX, "ppp {ppp}: `+` satura");
        assert_eq!(info.image_rect, antes, "ppp {ppp}");
    }
}

/// Puntos de un eje que se prueban: el píxel físico anterior a la imagen, el siguiente, y, por cada
/// píxel de la rejilla, su primer y su último píxel físico (a 1/K son el mismo y se miran todos; con
/// zoom ≥ ×1 se miran los tres primeros y los tres últimos píxeles de la imagen).
fn puntos_del_eje(f: Fraccion, origen: i64, dim: u32) -> Vec<i64> {
    let extent = o_extent(f, i64::from(dim));
    let mut v = vec![origen - 1, origen + extent];
    if f.1 > 1 {
        v.extend(origen..origen + extent);
        return v;
    }
    let lado = f.0 as i64;
    let dim = i64::from(dim);
    for i in [0, 1, 2, dim - 3, dim - 2, dim - 1] {
        v.push(origen + i * lado);
        v.push(origen + (i + 1) * lado - 1);
    }
    v
}

#[test]
fn fila_a6_hover_y_stroke_begin_en_cada_pixel_de_la_rejilla_y_uno_fuera() {
    let niveles = escalera();
    // 1/64, 1/8, 1/4, 1/2, ×1 y ×3.
    for ppp in PPPS {
        for indice in [0usize, 3, 4, 5, 6, 8] {
            let f = niveles[indice];
            let mut banco = Banco::con_image(ppp, PANTALLA, &imagen_64x48());
            let info = banco.al_nivel(indice);
            let g = banco.geom(&info);
            let (ext_x, ext_y) = (o_extent(f, 64), o_extent(f, 48));
            let fila = g.origen.1 + ext_y / 2;
            let columna = g.origen.0 + ext_x / 2;
            let mut puntos: Vec<(i64, i64)> = puntos_del_eje(f, g.origen.0, 64)
                .into_iter()
                .map(|x| (x, fila))
                .collect();
            puntos.extend(
                puntos_del_eje(f, g.origen.1, 48)
                    .into_iter()
                    .map(|y| (columna, y)),
            );
            for p in puntos {
                let etiqueta = format!(
                    "ppp {ppp} {} punto físico {p:?} (origen {:?})",
                    rotulo(f),
                    g.origen
                );
                let pos = banco.pos(p.0, p.1);
                // hover
                let reposo = banco.corre(vec![mover(pos)], &[]);
                assert_eq!(
                    reposo.info.hover,
                    o_hover(f, p, g.origen, (64, 48)),
                    "{etiqueta}: hover"
                );
                // StrokeBegin (con signo, sin recortar a la imagen).
                let pulsa = banco.corre(vec![boton(pos, true)], &[]);
                assert_eq!(
                    pulsa.trazo(),
                    vec![CanvasEvent::StrokeBegin(o_stroke(f, p, g.origen))],
                    "{etiqueta}: StrokeBegin"
                );
                // Se suelta para el siguiente punto.
                let suelta = banco.corre(vec![boton(pos, false)], &[]);
                assert_eq!(
                    suelta.trazo().last(),
                    Some(&CanvasEvent::StrokeEnd),
                    "{etiqueta}"
                );
            }
        }
    }
}

#[test]
fn fila_a6_a_1_k_hover_y_stroke_begin_son_multiplos_de_k_y_fuera_hover_es_none() {
    for ppp in PPPS {
        for (indice, k) in [(5usize, 2i32), (4, 4), (0, 64)] {
            let f = escalera()[indice];
            let mut banco = Banco::con_image(ppp, PANTALLA, &imagen_64x48());
            let info = banco.al_nivel(indice);
            let g = banco.geom(&info);
            let (ext_x, ext_y) = (o_extent(f, 64), o_extent(f, 48));
            for (dx, dy) in [
                (0, 0),
                (ext_x - 1, ext_y - 1),
                (-1, 0),
                (0, -1),
                (ext_x, 0),
                (0, ext_y),
            ] {
                let p = (g.origen.0 + dx, g.origen.1 + dy);
                let pos = banco.pos(p.0, p.1);
                let dentro = (0..ext_x).contains(&dx) && (0..ext_y).contains(&dy);
                let hover = banco.corre(vec![mover(pos)], &[]).info.hover;
                let begin = banco.corre(vec![boton(pos, true)], &[]).trazo();
                banco.corre(vec![boton(pos, false)], &[]);
                let esperado = (dx as i32 * k, dy as i32 * k);
                assert_eq!(
                    begin,
                    vec![CanvasEvent::StrokeBegin(esperado)],
                    "ppp {ppp} 1/{k} ({dx}, {dy})"
                );
                if dentro {
                    assert_eq!(hover, Some((esperado.0 as u32, esperado.1 as u32)));
                    assert_eq!(esperado.0 % k, 0);
                    assert_eq!(esperado.1 % k, 0);
                } else {
                    assert_eq!(hover, None, "ppp {ppp} 1/{k} ({dx}, {dy}) está fuera");
                }
            }
        }
    }
}

/// Imagen de 2048×1536 (cabe en la textura de 2048 de egui) para que Ajustar caiga en fracciones.
fn imagen_grande() -> Image {
    Image::new(2048, 1536, Mode::Rgba).expect("imagen válida")
}

#[test]
fn fila_a6_home_fit_y_request_view_dan_lo_de_a4() {
    let niveles = escalera();
    for ppp in PPPS {
        let mut banco = Banco::con_image(ppp, PANTALLA, &imagen_grande());
        let viewport = banco.viewport();
        let mejor = o_mejor_nivel(viewport, (2048, 1536));
        let esperado = rotulo(niveles[mejor]);
        // El ajuste inicial.
        let info = banco.reposo();
        assert_eq!(info.zoom.to_string(), esperado, "ppp {ppp}: ajuste inicial");
        comprueba_rect(&info, ppp, (2048, 1536), "ajuste inicial");
        let g = banco.geom(&info);
        let f = niveles[mejor];
        for (eje, origen, vista, dim) in [
            ("x", g.origen.0, viewport.width, 2048),
            ("y", g.origen.1, viewport.height, 1536),
        ] {
            let extent = o_extent(f, dim);
            assert!(
                (2 * origen + extent - i64::from(vista)).abs() <= 1,
                "ppp {ppp}: centrada en {eje} (origen {origen}, extent {extent}, lienzo {vista})"
            );
        }
        // Home.
        banco.pide(ViewOp::Actual);
        assert_eq!(banco.canvas.zoom(), Some(Zoom::ONE));
        let info = banco
            .corre(vec![tecla(Key::Home, Modifiers::NONE)], &[])
            .info;
        assert_eq!(info.zoom.to_string(), esperado, "ppp {ppp}: Home");
        // request_view(Fit) desde otro nivel.
        banco.pide(ViewOp::Actual);
        let info = banco.pide(ViewOp::Fit);
        assert_eq!(info.zoom.to_string(), esperado, "ppp {ppp}: Fit");
        assert_eq!(
            banco.geom(&info).origen,
            g.origen,
            "ppp {ppp}: Fit centra igual"
        );
        // request_view(ZoomOut) y (ZoomIn) desde el ajuste: un nivel.
        let info = banco.pide(ViewOp::ZoomOut);
        assert_eq!(
            info.zoom.to_string(),
            rotulo(niveles[mejor.saturating_sub(1)]),
            "ppp {ppp}: ZoomOut"
        );
        banco.pide(ViewOp::Fit);
        let info = banco.pide(ViewOp::ZoomIn);
        assert_eq!(
            info.zoom.to_string(),
            rotulo(niveles[(mejor + 1).min(69)]),
            "ppp {ppp}: ZoomIn"
        );
        comprueba_rect(&info, ppp, (2048, 1536), "tras ZoomIn");
    }
}

/// La malla de la textura de la imagen (la de la única baldosa de una imagen pequeña).
fn malla_de_la_imagen(banco: &Banco, formas: &[ClippedShape]) -> (Vec<Vertex>, Vec<u32>) {
    let id = banco.canvas.texture_id().expect("hay textura de la imagen");
    let mut mallas: Vec<(Vec<Vertex>, Vec<u32>)> = capas_de(formas)
        .into_iter()
        .filter_map(|capa| match capa {
            Capa::Malla {
                textura,
                vertices,
                indices,
            } if textura == id => Some((vertices, indices)),
            _ => None,
        })
        .collect();
    assert_eq!(
        mallas.len(),
        1,
        "con una sola baldosa, una sola malla de la imagen"
    );
    mallas.remove(0)
}

#[test]
fn fila_a6_la_malla_de_la_imagen_es_un_cuadrilatero_de_extent_pixeles_con_el_uv_desplazado() {
    let niveles = escalera();
    // 1/64, 1/32, 1/8, 1/2, ×1, ×2 y ×6.
    for ppp in PPPS {
        for indice in [0usize, 1, 3, 5, 6, 7, 11] {
            let f = niveles[indice];
            let mut banco = Banco::con_image(ppp, PANTALLA, &imagen_64x48());
            banco.al_nivel(indice);
            let salida = banco.corre(vec![], &[]);
            let g = banco.geom(&salida.info);
            let (vertices, indices) = malla_de_la_imagen(&banco, &salida.formas);
            let etiqueta = format!("ppp {ppp} {}", rotulo(f));
            assert_eq!(vertices.len(), 4, "{etiqueta}: un cuadrilátero");
            assert_eq!(indices.len(), 6, "{etiqueta}");
            // Ocupa `extent` píxeles físicos desde el origen.
            let (ext_x, ext_y) = (o_extent(f, 64) as f32, o_extent(f, 48) as f32);
            let caja = caja_pos(&vertices);
            let esperada = Rect::from_min_max(
                pos2(g.origen.0 as f32 / ppp, g.origen.1 as f32 / ppp),
                pos2(
                    (g.origen.0 as f32 + ext_x) / ppp,
                    (g.origen.1 as f32 + ext_y) / ppp,
                ),
            );
            assert!(
                (caja.min.x - esperada.min.x).abs() <= 2e-3
                    && (caja.min.y - esperada.min.y).abs() <= 2e-3
                    && (caja.max.x - esperada.max.x).abs() <= 2e-3
                    && (caja.max.y - esperada.max.y).abs() <= 2e-3,
                "{etiqueta}: la malla ocupa {caja:?} y debía ocupar {esperada:?}"
            );
            // uv: de 0 a 1 con zoom ≥ ×1; con 1/K, `(x·K + 0,5 − K/2) / lado` desde el borde.
            let uv = caja_uv(&vertices);
            let esperado = if f.1 == 1 {
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
            for (a, b) in uv.iter().zip(esperado) {
                assert!(
                    (a - b).abs() <= 1e-4,
                    "{etiqueta}: uv {uv:?} y debía ser {esperado:?}"
                );
            }
            comprueba_rect(&salida.info, ppp, (64, 48), &etiqueta);
        }
    }
}

// ---------------------------------------------------------------------------------------------
// P1 · banco de egui
// ---------------------------------------------------------------------------------------------

proptest! {
    #![proptest_config(ProptestConfig::with_cases(24))]

    #[test]
    fn propiedad_p1_hover_y_stroke_begin_de_un_puntero_cualquiera_son_el_oraculo(
        ppp_i in 0usize..5,
        indice in 0usize..70,
        x_frac in 0.0f64..1.0,
        y_frac in 0.0f64..1.0,
        despl in (-0.4f32..0.4, -0.4f32..0.4),
    ) {
        let ppp = PPPS[ppp_i];
        let f = escalera()[indice];
        let mut banco = Banco::con_image(ppp, PANTALLA, &imagen_64x48());
        let info = banco.al_nivel(indice);
        let g = banco.geom(&info);
        // Un píxel físico cualquiera del lienzo (no el del borde) más una fracción que no cambia
        // `round(v·ppp)`.
        let ancho = i64::from(g.viewport.width);
        let alto = i64::from(g.viewport.height);
        let p = (
            1 + (x_frac * (ancho - 3) as f64) as i64,
            1 + (y_frac * (alto - 3) as f64) as i64,
        );
        let pos = pos2((p.0 as f32 + despl.0) / ppp, (p.1 as f32 + despl.1) / ppp);
        prop_assert_eq!((pos.x * ppp).round() as i64, p.0);
        prop_assert_eq!((pos.y * ppp).round() as i64, p.1);
        let hover = banco.corre(vec![mover(pos)], &[]).info.hover;
        prop_assert_eq!(hover, o_hover(f, p, g.origen, (64, 48)), "{} ppp {} {:?}", rotulo(f), ppp, p);
        let begin = banco.corre(vec![boton(pos, true)], &[]).trazo();
        prop_assert_eq!(
            begin,
            vec![CanvasEvent::StrokeBegin(o_stroke(f, p, g.origen))],
            "{} ppp {} {:?}", rotulo(f), ppp, p
        );
    }
}

// ---------------------------------------------------------------------------------------------
// A7 · Trazo a 1/4
// ---------------------------------------------------------------------------------------------

fn rojo() -> Pixel {
    Pixel::Rgba(ROJO)
}

/// Quita los `StrokeMove` que repiten la posición anterior (soltar sin moverse repite la última).
fn sin_moves_repetidos(eventos: &[CanvasEvent]) -> Vec<CanvasEvent> {
    let mut salida: Vec<CanvasEvent> = Vec::new();
    let mut ultima: Option<(i32, i32)> = None;
    for e in eventos {
        match e {
            CanvasEvent::StrokeMove(p) if ultima == Some(*p) => continue,
            CanvasEvent::StrokeBegin(p) | CanvasEvent::StrokeMove(p) => ultima = Some(*p),
            CanvasEvent::StrokeEnd => ultima = None,
        }
        salida.push(*e);
    }
    salida
}

/// Un gesto: pulsa en `(ox + a.0, oy + a.1)`, mueve por cada punto de `moves` y suelta en el último.
fn gesto(
    banco: &mut Banco,
    origen: (i64, i64),
    desde: (i64, i64),
    moves: &[(i64, i64)],
) -> Vec<CanvasEvent> {
    let pos = |d: (i64, i64)| banco.pos(origen.0 + d.0, origen.1 + d.1);
    let (inicio, ultimo, destinos) = (
        pos(desde),
        pos(*moves.last().expect("al menos un movimiento")),
        moves.iter().map(|d| pos(*d)).collect::<Vec<_>>(),
    );
    let mut eventos = Vec::new();
    eventos.extend(banco.corre(vec![mover(inicio)], &[]).trazo());
    eventos.extend(banco.corre(vec![boton(inicio, true)], &[]).trazo());
    for destino in destinos {
        eventos.extend(banco.corre(vec![mover(destino)], &[]).trazo());
    }
    eventos.extend(banco.corre(vec![boton(ultimo, false)], &[]).trazo());
    sin_moves_repetidos(&eventos)
}

/// Aplica los eventos a un `Stroke` de 64×48 y devuelve sus píxeles (sin aplicarlo).
fn pixeles_del_trazo(eventos: &[CanvasEvent]) -> Vec<(u32, u32)> {
    let mut stroke = Stroke::new(64, 48);
    for e in eventos {
        match e {
            CanvasEvent::StrokeBegin(p) | CanvasEvent::StrokeMove(p) => stroke.move_to(*p, rojo()),
            CanvasEvent::StrokeEnd => {}
        }
    }
    stroke.pixels().iter().map(|p| (p.x, p.y)).collect()
}

#[test]
fn fila_a7_un_trazo_a_1_4_va_por_la_rejilla_en_un_solo_paso() {
    for ppp in PPPS {
        let mut banco = Banco::con_image(ppp, PANTALLA, &imagen_64x48());
        let info = banco.al_nivel(4);
        assert_eq!(info.zoom.to_string(), "×1/4");
        let origen = banco.geom(&info).origen;
        let eventos = gesto(&mut banco, origen, (2, 1), &[(5, 1)]);
        assert_eq!(
            eventos,
            vec![
                CanvasEvent::StrokeBegin((8, 4)),
                CanvasEvent::StrokeMove((20, 4)),
                CanvasEvent::StrokeEnd
            ],
            "ppp {ppp}"
        );
        // Los 13 píxeles de la línea de (8, 4) a (20, 4), sin huecos y en orden.
        let esperados: Vec<(u32, u32)> = (8..=20).map(|x| (x, 4)).collect();
        assert_eq!(pixeles_del_trazo(&eventos), esperados, "ppp {ppp}");

        // Un solo `SetPixels` (un solo paso): un `undo` restaura la imagen.
        let original = imagen_64x48();
        let mut doc = Document::new(original.clone());
        let mut stroke = Stroke::new(64, 48);
        for e in &eventos {
            match e {
                CanvasEvent::StrokeBegin(p) | CanvasEvent::StrokeMove(p) => {
                    stroke.move_to(*p, rojo())
                }
                CanvasEvent::StrokeEnd => {}
            }
        }
        let id = ImageId::from_raw(0);
        assert_eq!(stroke.commit(&mut doc, id), Ok(true));
        let imagen = doc.image(ImageId::from_raw(0)).expect("hay imagen");
        assert_eq!(imagen.get_pixel(8, 4), Ok(rojo()));
        assert_eq!(imagen.get_pixel(20, 4), Ok(rojo()));
        assert_eq!(
            imagen.get_pixel(14, 4),
            Ok(rojo()),
            "sin huecos en coordenadas de imagen"
        );
        assert_eq!(imagen.get_pixel(21, 4), Ok(Pixel::Rgba(TRANSPARENTE)));
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
fn fila_a7_un_trazo_que_sale_por_la_izquierda_y_vuelve_se_une_con_line_y_recorta() {
    for ppp in PPPS {
        let mut banco = Banco::con_image(ppp, PANTALLA, &imagen_64x48());
        let info = banco.al_nivel(4);
        let origen = banco.geom(&info).origen;
        // (2, 1) → (-3, 1) → (6, 1): a 1/4, (8, 4), (-12, 4) y (24, 4).
        let eventos = gesto(&mut banco, origen, (2, 1), &[(-3, 1), (6, 1)]);
        assert_eq!(
            eventos,
            vec![
                CanvasEvent::StrokeBegin((8, 4)),
                CanvasEvent::StrokeMove((-12, 4)),
                CanvasEvent::StrokeMove((24, 4)),
                CanvasEvent::StrokeEnd
            ],
            "ppp {ppp}"
        );
        // Las dos líneas se recortan a la imagen: la unión es x de 0 a 24 en la fila 4.
        let pixeles = pixeles_del_trazo(&eventos);
        let conjunto: HashSet<(u32, u32)> = pixeles.iter().copied().collect();
        let esperado: HashSet<(u32, u32)> = (0..=24).map(|x| (x, 4)).collect();
        assert_eq!(pixeles.len(), conjunto.len(), "sin repetidos");
        assert_eq!(conjunto, esperado, "ppp {ppp}");
    }
}

#[test]
fn fila_a7_un_trazo_que_sale_por_la_derecha_y_vuelve_se_recorta_a_la_imagen() {
    for ppp in PPPS {
        let mut banco = Banco::con_image(ppp, PANTALLA, &imagen_64x48());
        let info = banco.al_nivel(4);
        let origen = banco.geom(&info).origen;
        // (14, 1) → (20, 1) → (13, 1): a 1/4, (56, 4), (80, 4) y (52, 4); la imagen acaba en 63.
        let eventos = gesto(&mut banco, origen, (14, 1), &[(20, 1), (13, 1)]);
        assert_eq!(
            eventos,
            vec![
                CanvasEvent::StrokeBegin((56, 4)),
                CanvasEvent::StrokeMove((80, 4)),
                CanvasEvent::StrokeMove((52, 4)),
                CanvasEvent::StrokeEnd
            ],
            "ppp {ppp}"
        );
        let conjunto: HashSet<(u32, u32)> = pixeles_del_trazo(&eventos).into_iter().collect();
        let esperado: HashSet<(u32, u32)> = (52..=63).map(|x| (x, 4)).collect();
        assert_eq!(conjunto, esperado, "ppp {ppp}");
    }
}

// ---------------------------------------------------------------------------------------------
// A8 · Menú y columna
// ---------------------------------------------------------------------------------------------

fn estado(zoom: Option<Zoom>) -> UiState {
    UiState {
        can_undo: true,
        can_redo: true,
        zoom,
        tool: Tool::Pencil,
        locks: Locks::default(),
    }
}

fn nivel(indice: usize) -> Zoom {
    Zoom::levels().nth(indice).expect("nivel de la escalera")
}

#[test]
fn fila_a8_las_acciones_de_zoom_a_lo_largo_de_la_escalera() {
    // Zoom::MIN, 1/2, ×1, ×5 y Zoom::MAX.
    let casos: [(&str, Zoom); 5] = [
        ("1/64", Zoom::MIN),
        ("1/2", nivel(5)),
        ("×1", Zoom::ONE),
        ("×5", Zoom::from_int(5).expect("×5")),
        ("×64", Zoom::MAX),
    ];
    for (nombre, zoom) in casos {
        let s = estado(Some(zoom));
        assert_eq!(
            enabled(AppAction::ZoomOut, &s),
            nombre != "1/64",
            "ZoomOut con {nombre}"
        );
        assert_eq!(
            enabled(AppAction::ZoomIn, &s),
            nombre != "×64",
            "ZoomIn con {nombre}"
        );
        assert_eq!(
            enabled(AppAction::ZoomActual, &s),
            nombre != "×1",
            "ZoomActual con {nombre}"
        );
        assert!(enabled(AppAction::ZoomFit, &s), "ZoomFit con {nombre}");
    }
    // Los niveles de la escalera, todos: Alejar solo se apaga en el mínimo y Acercar solo en el máximo.
    for (i, zoom) in Zoom::levels().enumerate() {
        let s = estado(Some(zoom));
        assert_eq!(enabled(AppAction::ZoomOut, &s), i != 0, "{zoom}");
        assert_eq!(enabled(AppAction::ZoomIn, &s), i != 69, "{zoom}");
        assert_eq!(enabled(AppAction::ZoomActual, &s), i != 6, "{zoom}");
        assert!(enabled(AppAction::ZoomFit, &s), "{zoom}");
    }
    // Sin imagen, ninguna.
    let s = estado(None);
    for accion in [
        AppAction::ZoomOut,
        AppAction::ZoomIn,
        AppAction::ZoomActual,
        AppAction::ZoomFit,
    ] {
        assert!(!enabled(accion, &s), "{accion:?} sin imagen");
    }
}

#[test]
fn fila_a8_los_bloqueos_de_la_17_no_cambian() {
    let zoom = Some(nivel(5));
    let con = |f: fn(&mut Locks)| {
        let mut locks = Locks::default();
        f(&mut locks);
        UiState {
            locks,
            ..estado(zoom)
        }
    };
    // El zoom solo se bloquea con un campo de texto y desde el teclado; con un gesto, no.
    for accion in [
        AppAction::ZoomIn,
        AppAction::ZoomOut,
        AppAction::ZoomActual,
        AppAction::ZoomFit,
    ] {
        let texto = Locks {
            text_focus: true,
            ..Locks::default()
        };
        assert!(
            is_blocked(accion, texto, Origin::Key),
            "{accion:?} con texto desde el teclado"
        );
        assert!(
            !is_blocked(accion, texto, Origin::Menu),
            "{accion:?} con texto desde el menú"
        );
        let gesto = Locks {
            gesture: true,
            ..Locks::default()
        };
        assert!(!is_blocked(accion, gesto, Origin::Key));
        assert!(enabled(accion, &con(|l| l.gesture = true)) == enabled(accion, &estado(zoom)));
        assert!(
            !enabled(accion, &con(|l| l.dialog = true)),
            "{accion:?} con un diálogo"
        );
        assert!(
            !enabled(accion, &con(|l| l.modal = true)),
            "{accion:?} con un modal"
        );
    }
}

#[test]
fn fila_a8_el_rotulo_del_nivel_es_zoom_y_el_display() {
    assert_eq!(zoom_label(Some(nivel(5))), "Zoom ×1/2");
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
fn columna_con(ctx: &Context, etiqueta: &str) -> common::Pintado {
    let pantalla = vec2(400.0, 600.0);
    let s = estado(Some(Zoom::ONE));
    let mut ultimo = None;
    for _ in 0..2 {
        let formas = fotograma(ctx, pantalla, vec![], |ui| {
            caja(ui, &s, etiqueta);
        });
        ultimo = Some(pintado(&formas));
    }
    ultimo.expect("dos fotogramas")
}

#[test]
fn fila_a8_zoom_1_2_zoom_1_64_y_zoom_x12_caben_en_la_columna_a_las_cinco_escalas() {
    let niveles = [nivel(5), Zoom::MIN, Zoom::from_int(12).expect("×12")];
    for ppp in PPPS {
        let n = n_de(ppp);
        // La fuente real, con `MissingGlyphPolicy::Panic`: un glifo ausente (`×`) sería un pánico.
        let ctx = contexto(ppp);
        for zoom in niveles {
            let etiqueta = zoom_label(Some(zoom));
            let pintado = columna_con(&ctx, &etiqueta);
            let texto = pintado.texto_con(&etiqueta).unwrap_or_else(|| {
                panic!(
                    "ppp {ppp}: no se pinta `{etiqueta}`; hay {:?}",
                    pintado.todos_los_textos()
                )
            });
            assert_eq!(
                texto.texto, etiqueta,
                "ppp {ppp}: el texto se pinta entero, sin recorte"
            );
            // La columna va pegada al borde derecho de la pantalla de 400 × 600 pt.
            let caja_x = caja_sola(
                ((400.0 * ppp).round() as i64, (600.0 * ppp).round() as i64),
                n as u32,
            )
            .x;
            let limite = (caja_x + (TOOLBAR_MARGIN_U + toolbar_button_width_u()) * n) as f32;
            assert!(
                texto.rect().max.x * ppp <= limite + tolerancia_de_egui(ppp),
                "ppp {ppp}: `{etiqueta}` termina en {} px y el botón acaba en {limite}",
                texto.rect().max.x * ppp
            );
            assert!(
                texto.rect().max.x <= texto.clip.max.x + 1e-3,
                "ppp {ppp}: `{etiqueta}` queda fuera del recorte de la columna ({:?} frente a {:?})",
                texto.rect(),
                texto.clip
            );
            // 10 caracteres como mucho: 70 u de avance caben en los 99 u del botón.
            assert!(etiqueta.chars().count() <= 10);
        }
    }
}

// ---------------------------------------------------------------------------------------------
// A9 · Superposición a zoom < 1
// ---------------------------------------------------------------------------------------------

/// Las celdas `K×K` de la rejilla (filas 1 y 2, hasta 12 columnas), rellenas **enteras**, con un
/// color por columna: rojo, transparente (el borrador) y rojo con alfa 128.
fn pendientes_para(k: i64) -> Vec<Pendiente> {
    let colores = [ROJO, TRANSPARENTE, ROJO_MEDIO];
    let columnas = ((256 + k - 1) / k).min(12);
    let mut v = Vec::new();
    for gx in 0..columnas {
        for gy in 1..=2i64 {
            for dy in 0..k {
                for dx in 0..k {
                    v.push((
                        (gx * k + dx) as u32,
                        (gy * k + dy) as u32,
                        colores[(gx % 3) as usize],
                    ));
                }
            }
        }
    }
    v
}

struct Fallos {
    total: usize,
    primeros: Vec<String>,
}

impl Fallos {
    fn anota(&mut self, mensaje: impl FnOnce() -> String) {
        self.total += 1;
        if self.primeros.len() < 5 {
            self.primeros.push(mensaje());
        }
    }
}

/// Todos los píxeles físicos del lienzo se ven como el oráculo: dentro de la imagen, el píxel `s·K`
/// de la imagen (o el pendiente, que lo sustituye, solo si cae en la rejilla) sobre el fondo
/// elegido; fuera, `BACKGROUND`.
fn compara_con_el_oraculo(
    buffer: &Buffer,
    g: &Geom,
    k: i64,
    imagen: &Imagen,
    pendientes: &[Pendiente],
    fondo: CanvasBackground,
    etiqueta: &str,
) -> Fallos {
    let mapa: HashMap<(u32, u32), Rgba> = pendientes.iter().map(|&(x, y, c)| ((x, y), c)).collect();
    let ext = (
        (i64::from(imagen.ancho) + k - 1) / k,
        (i64::from(imagen.alto) + k - 1) / k,
    );
    let mut fallos = Fallos {
        total: 0,
        primeros: Vec::new(),
    };
    for py in 0..buffer.alto as i64 {
        for px in 0..buffer.ancho as i64 {
            let (sx, sy) = (px - g.origen.0, py - g.origen.1);
            let dentro = (0..ext.0).contains(&sx) && (0..ext.1).contains(&sy);
            let esperado = if dentro {
                let (ix, iy) = ((sx * k) as u32, (sy * k) as u32);
                let color = mapa
                    .get(&(ix, iy))
                    .copied()
                    .unwrap_or_else(|| imagen.pixel(ix, iy));
                compone(fondo_en(fondo, px, py, g.origen, g.cell), color)
            } else {
                Visto::de_color(BACKGROUND)
            };
            let visto = buffer.en(px, py);
            if !visto.cerca(esperado, TOLERANCIA) {
                fallos.anota(|| {
                    format!(
                        "{etiqueta}: el píxel físico ({px}, {py}) se ve {visto:?} y debía verse {esperado:?} (dentro de la imagen: {dentro})"
                    )
                });
            }
        }
    }
    fallos
}

/// Resumen de la estructura de un fotograma (cada forma: tipo, textura, caja y vértices, a una
/// milésima de punto). Dos fotogramas con la misma firma tienen las mismas formas: la
/// superposición, que es parte de la textura de la baldosa, **no añade ni cambia ninguna** (ni
/// rectángulos de color o de fondo por píxel pendiente, ni cuadriláteros del damero, ni mallas).
fn firma(capas: &[Capa]) -> Vec<String> {
    let milesimas = |r: Rect| {
        [r.min.x, r.min.y, r.max.x, r.max.y].map(|c| (f64::from(c) * 1000.0).round() as i64)
    };
    capas
        .iter()
        .map(|capa| match capa {
            Capa::Relleno { rect } => format!("relleno {:?}", milesimas(*rect)),
            Capa::Malla {
                textura, vertices, ..
            } => format!(
                "malla {textura:?} {:?} {} vértices",
                milesimas(caja_pos(vertices)),
                vertices.len()
            ),
        })
        .collect()
}

/// Ninguna forma de área cero: ni rectángulos vacíos ni triángulos degenerados, antes y después de
/// teselar.
fn sin_formas_de_area_cero(
    formas: &[ClippedShape],
    primitivas: &[ClippedPrimitive],
    ppp: f32,
    etiqueta: &str,
) {
    for capa in capas_de(formas) {
        match capa {
            Capa::Relleno { rect } => assert!(
                rect.width() > 0.0 && rect.height() > 0.0,
                "{etiqueta}: rectángulo de área cero {rect:?}"
            ),
            Capa::Malla {
                vertices, indices, ..
            } => {
                if let Some(area) = menor_area_fisica(&vertices, &indices, ppp) {
                    assert!(
                        area > 1e-6,
                        "{etiqueta}: triángulo degenerado (área {area} px²)"
                    );
                }
            }
        }
    }
    for primitiva in primitivas {
        if let Primitive::Mesh(malla) = &primitiva.primitive
            && let Some(area) = menor_area_fisica(&malla.vertices, &malla.indices, ppp)
        {
            assert!(
                area > 1e-6,
                "{etiqueta}: triángulo degenerado tras teselar (área {area} px²)"
            );
        }
    }
}

/// Un banco a `ppp`, con el degradado de 256×192 a 1/`k` y el fondo elegido.
fn banco_a9(ppp: f32, k: i64, fondo: CanvasBackground, malla: bool) -> Banco {
    let imagen = Imagen::degradado(256, 192);
    let indice = 6 - (k as u32).trailing_zeros() as usize;
    let mut banco = Banco::nuevo(ppp, PANTALLA_A9, imagen).con_fondo(fondo, malla);
    banco.al_nivel(indice);
    banco
}

fn comprueba_a9(ppp: f32, k: i64, fondo: CanvasBackground, malla: bool, con_superposicion: bool) {
    let mut banco = banco_a9(ppp, k, fondo, malla);
    let pendientes = if con_superposicion {
        pendientes_para(k)
    } else {
        Vec::new()
    };
    // El fotograma de referencia (sin superposición) y el de la prueba, tras `set_overlay`.
    let sin = banco.corre(vec![], &[]);
    let salida = banco.corre(vec![], &pendientes);
    let etiqueta = format!(
        "ppp {ppp} ×1/{k} fondo {fondo:?}{}{}",
        if malla { " (malla)" } else { "" },
        if con_superposicion {
            ""
        } else {
            " sin superposición"
        }
    );
    assert_eq!(
        salida.info.zoom.to_string(),
        format!("×1/{k}"),
        "{etiqueta}"
    );
    // La textura compuesta de la baldosa es la imagen con los píxeles pendientes escritos (el
    // texel nuevo es el color pendiente premultiplicado; el viejo no está).
    let imagen = banco.imagen.as_ref().expect("imagen del banco");
    let mut texels = imagen.texels();
    for &(x, y, c) in &pendientes {
        texels[(y * imagen.ancho + x) as usize] =
            Color32::from_rgba_unmultiplied(c.r, c.g, c.b, c.a);
    }
    let compuesta = banco
        .compuestas
        .get(banco.canvas.texture_id().expect("hay textura"))
        .expect("la baldosa se subió");
    assert_eq!(
        compuesta.tamano,
        [imagen.ancho as usize, imagen.alto as usize],
        "{etiqueta}"
    );
    assert!(
        compuesta.texels == texels,
        "{etiqueta}: la textura compuesta no es la imagen con los píxeles pendientes escritos"
    );
    // Ninguna forma de superposición: las formas son las del fotograma sin ella.
    assert_eq!(
        firma(&capas_de(&salida.formas)),
        firma(&capas_de(&sin.formas)),
        "{etiqueta}: la superposición no debe añadir ni cambiar ninguna forma"
    );
    let primitivas = banco.ctx.tessellate(salida.formas.clone(), ppp);
    sin_formas_de_area_cero(&salida.formas, &primitivas, ppp, &etiqueta);
    let viewport = banco.viewport();
    let buffer = rasteriza(
        &primitivas,
        ppp,
        viewport.width as usize,
        viewport.height as usize,
        &banco.texturas(),
    );
    let g = banco.geom(&salida.info);
    let imagen = banco.imagen.as_ref().expect("imagen del banco");
    let fallos = compara_con_el_oraculo(&buffer, &g, k, imagen, &pendientes, fondo, &etiqueta);
    assert_eq!(
        fallos.total, 0,
        "{etiqueta}: {} píxeles físicos mal; los primeros: {:#?}",
        fallos.total, fallos.primeros
    );
}

#[test]
fn fila_a9_control_sin_superposicion_cada_pixel_de_pantalla_muestra_el_pixel_s_por_k() {
    for ppp in PPPS {
        for k in [2, 4, 64] {
            comprueba_a9(ppp, k, CanvasBackground::Checker, false, false);
        }
    }
}

#[test]
fn fila_a9_solo_se_ven_los_pixeles_de_la_rejilla_cada_uno_de_1x1_con_los_cuatro_fondos() {
    for ppp in PPPS {
        for k in [2, 4, 64] {
            for fondo in FONDOS {
                comprueba_a9(ppp, k, fondo, false, true);
            }
        }
    }
}

#[test]
fn fila_a9_con_el_respaldo_por_malla_del_damero_se_ve_igual() {
    // `PIXOKO_DAMERO_MALLA=1` en la app es `Canvas::set_checker_mesh(true)`.
    for ppp in PPPS {
        for k in [2, 4, 64] {
            comprueba_a9(ppp, k, CanvasBackground::Checker, true, true);
        }
    }
}

#[test]
fn fila_a9_los_pixeles_fuera_de_la_rejilla_no_dejan_ninguna_forma_visible() {
    // Una superposición solo de píxeles fuera de la rejilla (ni `x` ni `y` múltiplos de K) no cambia
    // lo que se ve respecto a no tener superposición.
    for ppp in PPPS {
        for k in [2i64, 4, 64] {
            let fuera: Vec<Pendiente> = (0..12)
                .map(|i| ((i * k + 1) as u32, (k + 1) as u32, ROJO))
                .chain((0..12).map(|i| ((i * k) as u32, (k + 1) as u32, TRANSPARENTE)))
                .chain((0..12).map(|i| ((i * k + 1) as u32, k as u32, ROJO_MEDIO)))
                .filter(|&(x, y, _)| x < 256 && y < 192)
                .collect();
            let mut banco = banco_a9(ppp, k, CanvasBackground::Checker, false);
            let viewport = banco.viewport();
            let render = |banco: &mut Banco, overlay: &[Pendiente]| {
                let salida = banco.corre(vec![], overlay);
                let primitivas = banco.ctx.tessellate(salida.formas.clone(), ppp);
                let buffer = rasteriza(
                    &primitivas,
                    ppp,
                    viewport.width as usize,
                    viewport.height as usize,
                    &banco.texturas(),
                );
                (buffer, firma(&capas_de(&salida.formas)))
            };
            let (sin, formas_sin) = render(&mut banco, &[]);
            let (con, formas_con) = render(&mut banco, &fuera);
            assert_eq!(
                formas_sin, formas_con,
                "ppp {ppp} ×1/{k}: la superposición no añade ninguna forma"
            );
            for (i, (a, b)) in sin.pixeles.iter().zip(&con.pixeles).enumerate() {
                assert!(
                    a.cerca(*b, TOLERANCIA),
                    "ppp {ppp} ×1/{k}: el píxel físico ({}, {}) se ve distinto con píxeles fuera de la rejilla ({a:?} frente a {b:?})",
                    i % sin.ancho,
                    i / sin.ancho
                );
            }
        }
    }
}
