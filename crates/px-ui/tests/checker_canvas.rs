//! Tests del damero de transparencia en el lienzo de egui, sin ventana (specs/hito-2-damero-
//! lienzo.md, «Casos» filas 5 a 8, 14 y la parte de lienzo de la 15, y propiedades 2 y 3). Las
//! piezas puras (`checker`, `background`) están en `checker.rs`; el menú y los bloqueos, en
//! `background_menu.rs`; el damero que no se guarda en el PNG (fila 11), en `px-app` (necesita
//! `px-formats`); el despacho (fila 13), en `px-app/src/dispatch/tests.rs`.
//!
//! - Fila 5: damero sin bordes grises (64×48 con alfa 0, cinco escalas × ×1, ×2 y ×3, y tras
//!   arrastrar la vista 7 y 13 píxeles físicos).
//! - Fila 6: capas, textura 2×2, una sola subida, imagen de 2048×64 a ×64, `TooLarge` y documento
//!   intacto.
//! - Fila 7: superposición con damero (sin formas propias), antes y después de soltar.
//! - Fila 8: límites (1×1 a ×64, vista en el extremo, lienzo de 1×1 píxel físico, sin imagen y
//!   una imagen de 1025 de ancho con textura máxima de 1024, que se pinta en dos baldosas).
//! - Fila 14: superposición con cada fondo sólido y cambio de fondo a mitad de trazo.
//! - Fila 15 (lienzo): respaldo por malla en los bancos de las filas 5 y 7.
//! - Propiedad 2: pintar no cambia la conversión pantalla-imagen (24 casos).
//! - Propiedad 3: lo que se ve es el oráculo puro, con los cuatro fondos y los dos caminos
//!   (24 casos).
//!
//! Unidad 19b (specs/hito-2-fondo-color.md, al final del fichero): las filas 6, 7 y 8 y la propiedad
//! 3 con un color propio (`CanvasBackground::Custom`), con el mismo banco y el mismo rasterizador.
//! Fila 6: oráculo píxel a píxel a las cinco escalas, ×1, ×2 y ×3, tras arrastrar la vista, con el
//! respaldo por malla y con los zooms de alejamiento 1/2, 1/4 y 1/8; sin textura del damero. Fila 7:
//! la superposición del trazo, el borrador y `preview_entry` sobre el color. Fila 8: el cambio de
//! fondo en caliente con un trazo a medias y sin regenerar baldosas. Propiedad 3: el oráculo con un
//! color propio al azar. (La parte de `write_png` de la fila 8 necesita `px-formats` y está en
//! `px-app/src/background_color/tests.rs`.)
//!
//! El banco es el de `overlay_small_zoom.rs`: `Canvas::interact`, `Canvas::set_overlay`,
//! `Canvas::sync` y `Canvas::paint` (el orden de un fotograma de la app) dentro de
//! `Context::run_ui` con `RawInput` sintético, lo pintado pasa por el teselador de egui
//! (`Context::tessellate`, con los ajustes de la app) y se rasteriza **a mano** en el centro de cada
//! píxel físico (tolerancia 0,02 por canal y centro desplazado `+0,0013` y `+0,0007`, como allí; la
//! GPU aplica una regla de desempate en las aristas compartidas y aquí se evita el empate). El
//! rasterizador de esta versión pinta **todo el lienzo** en un búfer y muestrea: la textura de cada
//! baldosa de la imagen **tal como la deja egui** (el banco aplica en orden los `textures_delta`
//! de cada fotograma con `common::textures::Textures`, y la superposición del trazo va dentro de
//! esa textura), la textura 2×2 del damero (`A, B, B, A` con envoltura y sin filtro, como la GPU)
//! y, para el modo malla, los vértices coloreados sin textura.
//!
//! Suposiciones (las filas no las fijan; la spec las deja a «Piezas»):
//! - El cuadrilátero del damero va sin tinte (vértices blancos), como ya exige el
//!   `eraser_canvas.rs` adaptado, y va **debajo** de las mallas de las baldosas.
//! - No hay ninguna forma de superposición: tras el fondo y el damero de la imagen solo hay una
//!   malla (un cuadrilátero blanco) por baldosa visible, y ninguna forma tiene área cero.
//! - En modo malla solo se comprueba la estructura del fondo de la imagen (rectángulo `CHECK_A` y
//!   una malla de casillas `CHECK_B`); la superposición se comprueba por lo que se ve.
//! - El origen del damero es la esquina del rectángulo de la imagen en píxeles físicos, redondeada
//!   desde `CanvasInfo::image_rect`.

use px_core::ImageId;
use std::collections::{HashMap, HashSet};

use egui::epaint::{ClippedPrimitive, ClippedShape, ImageDelta, Primitive, Shape, Vertex};
use egui::{
    Color32, Context, Event, ImageData, Key, Modifiers, PointerButton, Pos2, RawInput, Rect,
    TextureId, TextureOptions, Ui, Vec2, pos2, vec2,
};
use proptest::prelude::*;
use px_core::{Document, Image, Mode, Palette, Pixel, Rgba};
use px_ui::background::CanvasBackground;
use px_ui::canvas::{BACKGROUND, Canvas, CanvasError, CanvasEvent, CanvasInfo, CanvasInput};
use px_ui::checker::{CheckerQuad, Shade, cells, clip, quad, shade_at, texels};
use px_ui::palette_grid::{CHECK_A, CHECK_B};
use px_ui::text::text_scale;
use px_ui::texture::{pixel_rgba, to_rgba8};
use px_ui::tool::{Tool, ToolAction, ToolState};
use px_ui::view::{MIN_VISIBLE, ScreenRect, Size, ViewOp, Zoom};

mod common;
use common::textures::Textures;
use common::{PPPS, button_of_2, context, in_points, key_2, mover, wheel_line};

const TEST_SCREEN: Vec2 = vec2(400.0, 300.0);
const TOLERANCE: f32 = 0.02;
const DESPLAZO_X: f64 = 0.0013;
const DESPLAZO_Y: f64 = 0.0007;

const TRANSPARENT: Rgba = Rgba::new(0, 0, 0, 0);
const RED: Rgba = Rgba::opaque(255, 0, 0);
const RED_MIDDLE: Rgba = Rgba::new(255, 0, 0, 128);
const BLUE: Rgba = Rgba::opaque(0, 0, 255);

const BACKGROUNDS: [CanvasBackground; 4] = [
    CanvasBackground::Checker,
    CanvasBackground::Black,
    CanvasBackground::Gray,
    CanvasBackground::White,
];
const SOLID: [CanvasBackground; 3] = [
    CanvasBackground::Black,
    CanvasBackground::Gray,
    CanvasBackground::White,
];

/// Un píxel pendiente: posición y color (el que tendrá en la imagen).
type TestPending = (u32, u32, Rgba);

// ---------------------------------------------------------------------------------------------
// Oráculos de color
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

    fn de_color(c: Color32) -> Seen {
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

/// Casilla del damero en el píxel físico `(px, py)` con la esquina de la imagen en `source_2` y la
/// casilla de `cell` píxeles físicos, calculada aparte de la implementación: la casilla `(0, 0)` es
/// `A` y alternan por la paridad de `floor((px − ox)/cell) + floor((py − oy)/cell)`.
fn tone_of_the_checker(px: i64, py: i64, source_2: (i64, i64), cell: i64) -> Color32 {
    let sum = (px - source_2.0).div_euclid(cell) + (py - source_2.1).div_euclid(cell);
    gray_2(if sum.rem_euclid(2) == 0 {
        CHECK_A
    } else {
        CHECK_B
    })
}

/// Color del fondo elegido bajo la transparencia en el píxel físico `(px, py)`.
fn background_in(
    background_2: CanvasBackground,
    px: i64,
    py: i64,
    source_2: (i64, i64),
    cell: i64,
) -> Color32 {
    match background_2 {
        CanvasBackground::Checker => tone_of_the_checker(px, py, source_2, cell),
        CanvasBackground::Black => Color32::from_rgb(0, 0, 0),
        CanvasBackground::Gray => BACKGROUND,
        CanvasBackground::White => Color32::from_rgb(255, 255, 255),
        CanvasBackground::Custom(c) => Color32::from_rgb(c.r, c.g, c.b),
    }
}

/// Un color con alfa directo por encima de un fondo opaco: lo que se ve.
fn composes(background_2: Color32, color: Rgba) -> Seen {
    let above = Color32::from_rgba_unmultiplied(color.r, color.g, color.b, color.a);
    Seen::de_color(background_2).below_of(Seen::de_color(above))
}

// ---------------------------------------------------------------------------------------------
// Imagen de la prueba (el oráculo del contenido de la textura de la imagen)
// ---------------------------------------------------------------------------------------------

#[derive(Clone)]
struct TestImage {
    width_2: u32,
    height_2: u32,
    pixels_2: Vec<Rgba>,
}

impl TestImage {
    fn uniform(width_2: u32, height_2: u32, color: Rgba) -> TestImage {
        TestImage {
            width_2,
            height_2,
            pixels_2: vec![color; (width_2 * height_2) as usize],
        }
    }

    fn pixel(&self, x: u32, y: u32) -> Rgba {
        self.pixels_2[(y * self.width_2 + x) as usize]
    }

    fn with(mut self, x: u32, y: u32, color: Rgba) -> TestImage {
        self.pixels_2[(y * self.width_2 + x) as usize] = color;
        self
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

    /// Los texels de la textura que sube el lienzo (premultiplicados, como los pasa egui).
    fn texels(&self) -> Vec<Color32> {
        self.pixels_2
            .iter()
            .map(|c| Color32::from_rgba_unmultiplied(c.r, c.g, c.b, c.a))
            .collect()
    }

    /// Los texels que debe tener la textura mientras dura la superposición: la imagen con los
    /// píxeles pendientes escritos encima (el color pendiente sustituye al de la imagen; el alfa 0
    /// del borrador es el texel transparente).
    fn texels_with(&self, overlay: &[TestPending]) -> Vec<Color32> {
        let mut texels = self.texels();
        for &(x, y, c) in overlay {
            texels[(y * self.width_2 + x) as usize] =
                Color32::from_rgba_unmultiplied(c.r, c.g, c.b, c.a);
        }
        texels
    }
}

fn image_transparent() -> TestImage {
    TestImage::uniform(64, 48, TRANSPARENT)
}

fn image_blue() -> TestImage {
    TestImage::uniform(64, 48, BLUE)
}

/// Superposición de prueba: un bloque de 4×2 píxeles por cada caso (como `overlay_small_zoom.rs`).
fn blocks() -> Vec<TestPending> {
    let mut v = Vec::new();
    for (i, color) in [RED, TRANSPARENT, RED_MIDDLE].into_iter().enumerate() {
        for y in 0..2 {
            for x in 0..4 {
                v.push((8 + 8 * i as u32 + x, 5 + y, color));
            }
        }
    }
    v
}

/// Los veinte píxeles del trazo del borrador de la fila 10 de la 17a: la fila 5, de `x = 10` a 29.
fn the_twenty() -> Vec<(u32, u32)> {
    (10..=29).map(|x| (x, 5)).collect()
}

// ---------------------------------------------------------------------------------------------
// Rasterizador a mano
// ---------------------------------------------------------------------------------------------

/// Coordenadas baricéntricas de `p` en el triángulo, si cae dentro (bordes incluidos), en `f64`.
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

/// Las texturas que el rasterizador sabe muestrear.
struct Sources<'a> {
    /// Las texturas de las baldosas de la imagen (en el orden de `Canvas::texture_ids`).
    tiles_2: Vec<TextureId>,
    /// Su contenido compuesto: lo que egui entregó a la GPU, subidas parciales incluidas.
    textures_2: &'a Textures,
    /// Textura 2×2 del damero, si el lienzo la ha creado.
    checker_2: Option<TextureId>,
}

/// Texel de la textura 2×2 del damero (por filas `A, B, B, A`) con `NEAREST` y envoltura `Repeat`.
fn texel_of_the_checker(u: f64, v: f64) -> Seen {
    let tx = ((u * 2.0).floor() as i64).rem_euclid(2);
    let ty = ((v * 2.0).floor() as i64).rem_euclid(2);
    let c = if (tx + ty) % 2 == 0 { CHECK_A } else { CHECK_B };
    Seen::de_color(gray_2(c))
}

/// Búfer de lo que se ve, de `width_2 × height_2` píxeles físicos.
struct Buffer {
    width_2: usize,
    height_2: usize,
    pixels_2: Vec<Seen>,
}

impl Buffer {
    fn en(&self, x: i64, y: i64) -> Seen {
        assert!(
            x >= 0 && y >= 0 && (x as usize) < self.width_2 && (y as usize) < self.height_2,
            "({x}, {y}) fuera del búfer de {}×{}",
            self.width_2,
            self.height_2
        );
        self.pixels_2[y as usize * self.width_2 + x as usize]
    }
}

/// Pinta `primitives`, en orden, en un búfer de `width_2 × height_2` píxeles físicos. Cada píxel se
/// muestrea en su centro (algo desplazado). Las mallas de una baldosa muestrean, por su uv
/// interpolado, el texel de su textura compuesta (`Textures::samples`: `NEAREST`, sin
/// envoltura); las del damero, la textura 2×2; las demás (rellenos, formas y mallas de colores de
/// vértice) usan el color de sus vértices (la textura es el píxel blanco de la fuente).
fn rasterizes(
    primitives: &[ClippedPrimitive],
    ppp: f32,
    width_2: usize,
    height_2: usize,
    tex: &Sources,
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
        let is_image = tex.tiles_2.contains(&mesh.texture_id);
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
            let tints = v.map(|v| Seen::de_color(v.color));
            let plane =
                !is_image && !is_checker && v[0].color == v[1].color && v[1].color == v[2].color;
            let edges = [(p[0], p[1]), (p[1], p[2]), (p[2], p[0])];
            for py in y0..y1 {
                let sy = py as f64 + 0.5 + DESPLAZO_Y;
                if sy < cy0 || sy > cy1 {
                    continue;
                }
                // Tramo de la fila dentro del triángulo (bordes incluidos).
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
                                let texel = tex
                                    .textures_2
                                    .samples(mesh.texture_id, u, t)
                                    .expect("la baldosa tiene contenido");
                                Seen::de_color(texel)
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
        color: Color32,
    },
    TestMesh {
        texture_2: TextureId,
        vertices: Vec<Vertex>,
        indices: Vec<u32>,
    },
}

fn collects(shape: &Shape, output_2: &mut Vec<Layer>) {
    match shape {
        Shape::Rect(r) => output_2.push(Layer::TestFill {
            rect: r.rect,
            color: r.fill,
        }),
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

fn toolbox_2(vertices: &[Vertex]) -> Rect {
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

fn almost(a: Rect, b: Rect) -> bool {
    almost_with(a, b, 1e-3)
}

fn almost_with(a: Rect, b: Rect, t: f32) -> bool {
    (a.min.x - b.min.x).abs() <= t
        && (a.min.y - b.min.y).abs() <= t
        && (a.max.x - b.max.x).abs() <= t
        && (a.max.y - b.max.y).abs() <= t
}

fn rect_in_points(a: ScreenRect, ppp: f32) -> Rect {
    Rect::from_min_max(
        pos2(a.x as f32 / ppp, a.y as f32 / ppp),
        pos2((a.x + a.width) as f32 / ppp, (a.y + a.height) as f32 / ppp),
    )
}

// ---------------------------------------------------------------------------------------------
// El banco
// ---------------------------------------------------------------------------------------------

/// Lo que dejó un fotograma. Los cambios de textura ya están aplicados a `Bench::textures` y
/// limpiados (egui entra en pánico si se sueltan sin aplicar) y copiados en `uploads` y
/// `released`.
struct TestOutput {
    shapes_2: Vec<ClippedShape>,
    uploads: Vec<(TextureId, ImageDelta)>,
    released: Vec<TextureId>,
    info: Option<CanvasInfo>,
    stroke_2: Vec<CanvasEvent>,
}

impl TestOutput {
    fn layers(&self) -> Vec<Layer> {
        layers_of(&self.shapes_2)
    }

    fn info(&self) -> &CanvasInfo {
        self.info.as_ref().expect("el lienzo se pinta")
    }

    fn uploads_of(&self, id: TextureId) -> Vec<&ImageDelta> {
        self.uploads
            .iter()
            .filter(|(t, _)| *t == id)
            .map(|(_, d)| d)
            .collect()
    }
}

/// Geometría de lo pintado, en píxeles físicos.
#[derive(Clone, Copy, Debug)]
struct Geom {
    ppp: f32,
    /// Esquina de la imagen.
    source_2: (i64, i64),
    width_2: i64,
    height_2: i64,
    /// Píxeles físicos por píxel de imagen.
    z: i64,
    viewport: Size,
    /// Casilla del damero.
    cell: i64,
}

impl Geom {
    fn rect(&self) -> ScreenRect {
        ScreenRect {
            x: self.source_2.0,
            y: self.source_2.1,
            width: self.width_2,
            height: self.height_2,
        }
    }

    fn clip_2(&self) -> Option<ScreenRect> {
        clip(self.rect(), self.viewport)
    }

    fn block(&self, x: u32, y: u32) -> ScreenRect {
        ScreenRect {
            x: self.source_2.0 + i64::from(x) * self.z,
            y: self.source_2.1 + i64::from(y) * self.z,
            width: self.z,
            height: self.z,
        }
    }

    fn quad_2(&self, area: ScreenRect) -> Option<CheckerQuad> {
        quad(area, self.source_2, self.cell)
    }
}

struct Bench {
    ctx: Context,
    ppp: f32,
    screen: Vec2,
    canvas: Canvas,
    /// El contenido que debe tener la imagen del documento (el oráculo de la prueba).
    image_2: TestImage,
    /// Esa misma imagen como `px_core::Image`: el documento con el que se llama a `set_overlay` y
    /// `sync`.
    document: Image,
    /// El contenido de las texturas tal como egui lo entrega a la GPU, con los cambios de **cada**
    /// fotograma aplicados en orden.
    textures_2: Textures,
}

impl Bench {
    /// Sin ningún fotograma ejecutado: lo que sube el primero se puede mirar.
    fn without_warm_up(
        screen: Vec2,
        ppp: f32,
        image_2: TestImage,
        background_2: CanvasBackground,
        mesh: bool,
    ) -> Bench {
        let mut canvas = Canvas::new();
        canvas.set_background(background_2);
        canvas.set_checker_mesh(mesh);
        let document = image_2.image();
        canvas.set_image(&document);
        Bench {
            ctx: context(ppp),
            ppp,
            screen,
            canvas,
            image_2,
            document,
            textures_2: Textures::new(),
        }
    }

    fn new_with(ppp: f32, image_2: TestImage, background_2: CanvasBackground, mesh: bool) -> Bench {
        let mut bench = Bench::without_warm_up(TEST_SCREEN, ppp, image_2, background_2, mesh);
        for _ in 0..3 {
            bench.runs(vec![], &[]);
        }
        bench
    }

    fn new_2(ppp: f32, image_2: TestImage) -> Bench {
        Bench::new_with(ppp, image_2, CanvasBackground::Checker, false)
    }

    /// Un fotograma: `draws` recibe el lienzo y la interfaz y devuelve los eventos de trazo y la
    /// información de `paint`, si lo llamó.
    fn runs_with(
        &mut self,
        events: Vec<Event>,
        max_texture_side: Option<usize>,
        draws: impl FnOnce(&mut Canvas, &mut Ui) -> (Vec<CanvasEvent>, Option<CanvasInfo>),
    ) -> TestOutput {
        let input = RawInput {
            screen_rect: Some(Rect::from_min_size(Pos2::ZERO, self.screen)),
            events,
            max_texture_side,
            ..RawInput::default()
        };
        let canvas = &mut self.canvas;
        let mut draws = Some(draws);
        let mut result = None;
        let mut output = self.ctx.run_ui(input, |ui| {
            let draws = draws.take().expect("un solo dibujo por fotograma");
            result = Some(draws(canvas, ui));
        });
        let (stroke_2, info) = result.expect("el lienzo se dibuja");
        let mut uploads = Vec::new();
        for (id, deltas) in &output.textures_delta.set {
            for delta in deltas {
                uploads.push((*id, delta.clone()));
            }
        }
        let released = output.textures_delta.free.iter().copied().collect();
        // El contenido compuesto de las texturas del lienzo; la fuente de egui (`Managed(0)`) no
        // interesa (el banco no pinta texto) y sus parches llegarían a una textura que el banco no
        // vio crear.
        let mut of_the_canvas = output.textures_delta.clone();
        of_the_canvas.set.remove(&TextureId::default());
        of_the_canvas.free.retain(|id| *id != TextureId::default());
        self.textures_2.applies(&of_the_canvas);
        // La copia no se entrega a la GPU: egui exige vaciarla antes de soltarla.
        of_the_canvas.clear();
        output.textures_delta.clear();
        TestOutput {
            shapes_2: std::mem::take(&mut output.shapes),
            uploads,
            released,
            info,
            stroke_2,
        }
    }

    /// Un fotograma de la app, sin herramienta: `interact`, `set_overlay` con la superposición dada
    /// (el trazo pendiente entero; vacía, la quita), `sync` con el documento y `paint`.
    fn runs(&mut self, events: Vec<Event>, overlay: &[TestPending]) -> TestOutput {
        self.runs_max(events, overlay, None)
    }

    fn runs_max(
        &mut self,
        events: Vec<Event>,
        overlay: &[TestPending],
        max: Option<usize>,
    ) -> TestOutput {
        let document = self.document.clone();
        self.runs_with(events, max, |canvas, ui| {
            let stroke_2: Vec<CanvasEvent> = canvas
                .interact_roles(ui)
                .into_iter()
                .filter_map(|(input, _)| match input {
                    CanvasInput::Stroke(event) => Some(event),
                    CanvasInput::Escape => None,
                })
                .collect();
            canvas.set_overlay(ui.ctx(), &document, overlay);
            // Sin imagen cargada (o con error) no hay nada que sincronizar.
            if canvas.texture_id().is_some() {
                canvas
                    .sync(ui.ctx(), &document)
                    .expect("el documento se carga");
            }
            let info = canvas.paint(ui);
            (stroke_2, Some(info))
        })
    }

    /// Solo `interact`: sin pintar.
    fn only_interact(&mut self, events: Vec<Event>) -> TestOutput {
        self.runs_with(events, None, |canvas, ui| {
            let stroke_2: Vec<CanvasEvent> = canvas
                .interact_roles(ui)
                .into_iter()
                .filter_map(|(input, _)| match input {
                    CanvasInput::Stroke(event) => Some(event),
                    CanvasInput::Escape => None,
                })
                .collect();
            (stroke_2, None)
        })
    }

    /// Tamaño del lienzo en píxeles físicos.
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
            ppp,
            source_2: (
                (rect.min.x * ppp).round() as i64,
                (rect.min.y * ppp).round() as i64,
            ),
            width_2: (rect.width() * ppp).round() as i64,
            height_2: (rect.height() * ppp).round() as i64,
            z: info.zoom.scale(1),
            viewport: self.viewport(),
            cell: 8 * i64::from(text_scale(ppp)),
        }
    }

    /// Las texturas que el rasterizador muestrea: las baldosas (con su contenido compuesto) y el
    /// damero.
    fn sources(&self) -> Sources<'_> {
        Sources {
            tiles_2: self.canvas.texture_ids(),
            textures_2: &self.textures_2,
            checker_2: self.canvas.checker_texture_id(),
        }
    }

    /// Un fotograma con la superposición dada, teselado y rasterizado entero.
    fn scene(&mut self, overlay: &[TestPending]) -> Scene {
        let output_2 = self.runs(vec![], overlay);
        self.rasterizes_output(output_2)
    }

    fn rasterizes_output(&self, output_2: TestOutput) -> Scene {
        let primitives = self.ctx.tessellate(output_2.shapes_2.clone(), self.ppp);
        let viewport = self.viewport();
        let buffer = rasterizes(
            &primitives,
            self.ppp,
            viewport.width as usize,
            viewport.height as usize,
            &self.sources(),
        );
        let geom = self.geom(output_2.info());
        Scene {
            output_2,
            buffer,
            geom,
        }
    }

    /// Lleva el zoom a ×`zoom` con las operaciones de vista.
    fn with_zoom(&mut self, zoom: u32) {
        self.canvas.request_view(ViewOp::Actual);
        for _ in 1..zoom {
            self.canvas.request_view(ViewOp::ZoomIn);
        }
        self.runs(vec![], &[]);
        assert_eq!(
            self.canvas.zoom().map(|z| z.to_string()),
            Some(format!("×{zoom}"))
        );
    }

    /// Arrastra la vista con el botón central `dx` y `dy` píxeles físicos (si el límite de la vista
    /// no lo impide). egui no decide que es un arrastre hasta que el puntero se aleja más de
    /// `max_click_dist` de donde se pulsó: el primer movimiento va lejos (40 puntos, que ya es
    /// arrastre y se cuenta entero) y el segundo vuelve al destino, así el desplazamiento neto es
    /// exactamente `(dx, dy)`.
    fn drags(&mut self, dx: i64, dy: i64) -> Vec<TestOutput> {
        let start_2 = pos2(self.screen.x / 2.0, self.screen.y / 2.0);
        let far = start_2 + vec2(40.0, 40.0);
        let target_2 = start_2 + vec2(dx as f32 / self.ppp, dy as f32 / self.ppp);
        let steps_2 = [
            vec![mover(start_2)],
            vec![button_of_2(start_2, PointerButton::Middle, true)],
            vec![mover(far)],
            vec![mover(target_2)],
            vec![button_of_2(target_2, PointerButton::Middle, false)],
            vec![],
        ];
        steps_2
            .into_iter()
            .map(|events| self.runs(events, &[]))
            .collect()
    }

    /// Arrastra la vista varias veces hacia un lado hasta que el límite de la vista la frena.
    fn drags_to_the_end(&mut self, direction_x: i64, direction_y: i64) {
        let step = (
            (150.0 * self.ppp).round() as i64,
            (100.0 * self.ppp).round() as i64,
        );
        for _ in 0..6 {
            self.drags(direction_x * step.0, direction_y * step.1);
        }
    }

    /// Suelta la superposición (si la hay) y llama a `Canvas::refresh` con la imagen nueva, como
    /// la app al aplicar un trazo; deja anotado el contenido que debe tener el documento.
    fn refreshes(&mut self, image_2: TestImage) {
        let document = image_2.image();
        self.canvas.set_overlay(&self.ctx, &document, &[]);
        self.canvas.refresh(&self.ctx, &document);
        self.image_2 = image_2;
        self.document = document;
    }

    fn sets_image(&mut self, image_2: TestImage) {
        let document = image_2.image();
        self.canvas.set_image(&document);
        self.image_2 = image_2;
        self.document = document;
    }
}

/// Un fotograma teselado y rasterizado.
struct Scene {
    output_2: TestOutput,
    buffer: Buffer,
    geom: Geom,
}

// ---------------------------------------------------------------------------------------------
// Comprobaciones
// ---------------------------------------------------------------------------------------------

struct Failures {
    total: usize,
    first_2: Vec<String>,
}

impl Failures {
    fn new_2() -> Failures {
        Failures {
            total: 0,
            first_2: Vec::new(),
        }
    }

    fn notes(&mut self, message: impl FnOnce() -> String) {
        self.total += 1;
        if self.first_2.len() < 5 {
            self.first_2.push(message());
        }
    }

    fn empty(&self) -> bool {
        self.total == 0
    }

    fn summary_2(&self) -> String {
        format!("{} fallos; los primeros: {:#?}", self.total, self.first_2)
    }
}

/// Todos los píxeles físicos del lienzo se ven como el oráculo puro: dentro de la imagen, el píxel
/// de la imagen (o el de la superposición, que lo sustituye) sobre el fondo elegido; fuera,
/// `BACKGROUND`.
fn failures(
    scene: &Scene,
    image_2: &TestImage,
    overlay: &[TestPending],
    background_2: CanvasBackground,
) -> Failures {
    let g = scene.geom;
    let map: HashMap<(u32, u32), Rgba> = overlay.iter().map(|&(x, y, c)| ((x, y), c)).collect();
    let mut failures = Failures::new_2();
    for py in 0..scene.buffer.height_2 as i64 {
        for px in 0..scene.buffer.width_2 as i64 {
            let inside = px >= g.source_2.0
                && px < g.source_2.0 + g.width_2
                && py >= g.source_2.1
                && py < g.source_2.1 + g.height_2;
            let expected_2 = if inside {
                let ix = ((px - g.source_2.0) / g.z) as u32;
                let iy = ((py - g.source_2.1) / g.z) as u32;
                let color = map
                    .get(&(ix, iy))
                    .copied()
                    .unwrap_or_else(|| image_2.pixel(ix, iy));
                composes(
                    background_in(background_2, px, py, g.source_2, g.cell),
                    color,
                )
            } else {
                Seen::de_color(BACKGROUND)
            };
            let seen = scene.buffer.en(px, py);
            if !seen.near(expected_2, TOLERANCE) {
                failures.notes(|| {
                    format!(
                        "ppp {} zoom ×{}: el píxel físico ({px}, {py}) se ve {seen:?} y debía verse {expected_2:?} (dentro de la imagen: {inside})",
                        g.ppp, g.z
                    )
                });
            }
        }
    }
    failures
}

fn without_failures(failures: &Failures, label_2: &str) {
    assert!(failures.empty(), "{label_2}: {}", failures.summary_2());
}

/// Un cuadrilátero del damero: 4 vértices, 6 índices, sin tinte, en el sitio y con los uv de
/// `quad`.
fn checks_quad(layer_2: &Layer, expected_2: &CheckerQuad, ppp: f32, label_2: &str) {
    let Layer::TestMesh {
        vertices, indices, ..
    } = layer_2
    else {
        panic!("{label_2}: se esperaba una malla y hay {layer_2:?}");
    };
    assert_eq!(vertices.len(), 4, "{label_2}: 4 vértices");
    assert_eq!(indices.len(), 6, "{label_2}: 6 índices");
    let in_points = rect_in_points(expected_2.rect, ppp);
    assert!(
        almost(toolbox_2(vertices), in_points),
        "{label_2}: la malla ocupa {:?} y debía ocupar {in_points:?}",
        toolbox_2(vertices)
    );
    assert_eq!(
        toolbox_uv(vertices),
        expected_2.uv,
        "{label_2}: uv de `quad` ({:?})",
        expected_2.rect
    );
    assert!(
        vertices.iter().all(|v| v.color == Color32::WHITE),
        "{label_2}: sin tinte"
    );
}

/// Área de un triángulo de la malla, en píxeles físicos cuadrados.
fn area_of_mesh(vertices: &[Vertex], indices: &[u32], ppp: f32) -> f64 {
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
        .sum()
}

/// Comprueba todo lo pintado según el fondo y el camino, y devuelve el índice de la primera malla
/// de baldosa:
/// - `BACKGROUND` en todo el lienzo;
/// - `Checker` con textura: el cuadrilátero del damero (`quad(clip(image_2))`);
/// - `Checker` con malla: rectángulo `CHECK_A` y **una** malla de colores de vértice con las casillas
///   `CHECK_B`, sin textura del damero;
/// - un sólido: un rectángulo de ese color sobre `clip(image_2)`;
///
/// y, después, las mallas de las baldosas visibles y **nada más** (`checks_tiles`): el
/// fotograma, con o sin superposición, no tiene ninguna forma de superposición.
fn checks_base(
    bench: &Bench,
    output_2: &TestOutput,
    background_2: CanvasBackground,
    mesh: bool,
    label_2: &str,
) -> usize {
    let layers = output_2.layers();
    let g = bench.geom(output_2.info());
    let tiles_2 = bench.canvas.texture_ids();
    match &layers[0] {
        Layer::TestFill { rect, color } => {
            assert_eq!(*color, BACKGROUND, "{label_2}: el fondo liso");
            // El rectángulo del lienzo lo coloca egui, que redondea el diseño a 1/16 de punto.
            let canvas_2 = Rect::from_min_size(Pos2::ZERO, bench.screen);
            assert!(
                almost_with(*rect, canvas_2, 0.04),
                "{label_2}: el fondo cubre el lienzo: {rect:?} y {canvas_2:?}"
            );
        }
        other => panic!("{label_2}: la primera capa es el fondo liso y hay {other:?}"),
    }
    let mut i = 1;
    if let Some(area) = g.clip_2() {
        let in_points = rect_in_points(area, g.ppp);
        match (background_2, mesh) {
            (CanvasBackground::Checker, false) => {
                let checker_2 = bench
                    .canvas
                    .checker_texture_id()
                    .unwrap_or_else(|| panic!("{label_2}: hay textura del damero"));
                assert!(
                    !tiles_2.contains(&checker_2),
                    "{label_2}: el damero no es una baldosa"
                );
                let Layer::TestMesh { texture_2, .. } = &layers[i] else {
                    panic!(
                        "{label_2}: la segunda capa es el damero y hay {:?}",
                        layers[i]
                    );
                };
                assert_eq!(*texture_2, checker_2, "{label_2}: textura del damero");
                let expected_2 = g.quad_2(area).expect("recorte no vacío");
                assert_eq!(expected_2.rect, area);
                checks_quad(&layers[i], &expected_2, g.ppp, label_2);
                i += 1;
            }
            (CanvasBackground::Checker, true) => {
                if let Some(checker_2) = bench.canvas.checker_texture_id() {
                    assert!(
                        !layers.iter().any(
                            |c| matches!(c, Layer::TestMesh { texture_2, .. } if *texture_2 == checker_2)
                        ),
                        "{label_2}: en modo malla nadie usa la textura del damero"
                    );
                }
                let Layer::TestFill { rect, color } = &layers[i] else {
                    panic!("{label_2}: rectángulo CHECK_A y hay {:?}", layers[i]);
                };
                assert_eq!(*color, gray_2(CHECK_A), "{label_2}: rectángulo de CHECK_A");
                assert!(almost(*rect, in_points), "{label_2}: {rect:?}");
                i += 1;
                let boxes_b: Vec<ScreenRect> = cells(area, g.source_2, g.cell)
                    .filter(|c| c.shade == Shade::B)
                    .map(|c| c.rect)
                    .collect();
                if !boxes_b.is_empty() {
                    let Layer::TestMesh {
                        texture_2,
                        vertices,
                        indices,
                    } = &layers[i]
                    else {
                        panic!("{label_2}: una malla de casillas B y hay {:?}", layers[i]);
                    };
                    assert!(
                        !tiles_2.contains(texture_2),
                        "{label_2}: las casillas no usan una baldosa"
                    );
                    assert!(
                        vertices.iter().all(|v| v.color == gray_2(CHECK_B)),
                        "{label_2}: colores de vértice CHECK_B"
                    );
                    let expected_2: i64 = boxes_b.iter().map(|c| c.width * c.height).sum();
                    let real = area_of_mesh(vertices, indices, g.ppp);
                    assert!(
                        (real - expected_2 as f64).abs() < 0.5,
                        "{label_2}: las casillas B suman {expected_2} px² y la malla {real}"
                    );
                    i += 1;
                }
            }
            (solid, _) => {
                let color = background_in(solid, 0, 0, (0, 0), 8);
                let Layer::TestFill { rect, color: real } = &layers[i] else {
                    panic!(
                        "{label_2}: rectángulo de fondo sólido y hay {:?}",
                        layers[i]
                    );
                };
                assert_eq!(*real, color, "{label_2}: color del fondo sólido");
                assert!(
                    almost(*rect, in_points),
                    "{label_2}: {rect:?} y {in_points:?}"
                );
                assert_eq!(
                    bench.canvas.checker_texture_id(),
                    None,
                    "{label_2}: con un sólido no se crea la textura del damero"
                );
                i += 1;
            }
        }
    }
    checks_tiles(bench, &g, &layers, i, label_2);
    i
}

fn is_of_tile(layer_2: &Layer, tiles_2: &[TextureId]) -> bool {
    matches!(layer_2, Layer::TestMesh { texture_2, .. } if tiles_2.contains(texture_2))
}

/// Lo que hay desde `from_2`: solo las mallas de las baldosas visibles (un cuadrilátero blanco por
/// baldosa, cada textura una sola vez; al menos una si algo de la imagen cae en el lienzo) y
/// **ninguna otra forma**: ni rectángulos de color o de fondo por píxel pendiente, ni cuadriláteros
/// del damero por píxel, ni mallas de superposición. Ninguna forma de todo el fotograma tiene
/// área cero (la regresión `b702d8e`: a ×1 la superposición de 1×1 píxel físico no se veía; ahora
/// no hay formas que se degeneren y el píxel va en la textura).
fn checks_tiles(bench: &Bench, g: &Geom, layers: &[Layer], from_2: usize, label_2: &str) {
    let tiles_2 = bench.canvas.texture_ids();
    let own = &layers[from_2..];
    let n = own.iter().take_while(|c| is_of_tile(c, &tiles_2)).count();
    assert_eq!(
        n,
        own.len(),
        "{label_2}: tras el fondo solo hay mallas de baldosas (ninguna forma de superposición) y sobra {:?}",
        &own[n..]
    );
    if g.clip_2().is_some() {
        assert!(
            n >= 1,
            "{label_2}: la imagen visible se pinta con sus baldosas"
        );
    }
    let mut different = HashSet::new();
    for layer_2 in own {
        let Layer::TestMesh {
            texture_2,
            vertices,
            indices,
        } = layer_2
        else {
            unreachable!("ya comprobado")
        };
        assert!(
            different.insert(*texture_2),
            "{label_2}: una malla por baldosa visible"
        );
        assert_eq!(vertices.len(), 4, "{label_2}: un cuadrilátero por baldosa");
        assert_eq!(indices.len(), 6, "{label_2}: 6 índices");
        assert!(
            vertices.iter().all(|v| v.color == Color32::WHITE),
            "{label_2}: la baldosa va sin tinte"
        );
    }
    without_zero_area_shapes(layers, label_2);
}

/// Ningún rectángulo ni triángulo de las capas tiene área cero.
fn without_zero_area_shapes(layers: &[Layer], label_2: &str) {
    for layer_2 in layers {
        match layer_2 {
            Layer::TestFill { rect, .. } => assert!(
                rect.width() > 0.0 && rect.height() > 0.0,
                "{label_2}: rectángulo de área cero {rect:?}"
            ),
            Layer::TestMesh {
                vertices, indices, ..
            } => {
                for t in indices.as_chunks::<3>().0 {
                    let [a, b, c] = t.map(|i| vertices[i as usize].pos);
                    let cross = (b.x - a.x) * (c.y - a.y) - (c.x - a.x) * (b.y - a.y);
                    assert!(
                        cross != 0.0,
                        "{label_2}: triángulo degenerado {a:?} {b:?} {c:?}"
                    );
                }
            }
        }
    }
}

/// El contenido compuesto de la única baldosa de la imagen es el de la imagen del documento con
/// los píxeles pendientes escritos encima (sin ellos, el documento): lo que la GPU muestrea.
fn checks_texture(bench: &Bench, overlay: &[TestPending], label_2: &str) {
    let ids = bench.canvas.texture_ids();
    assert_eq!(ids.len(), 1, "{label_2}: una sola baldosa");
    let texture_2 = bench
        .textures_2
        .get(ids[0])
        .expect("la baldosa tiene contenido");
    assert_eq!(
        texture_2.size_2,
        [
            bench.image_2.width_2 as usize,
            bench.image_2.height_2 as usize
        ],
        "{label_2}: tamaño de la textura"
    );
    assert!(
        texture_2.texels == bench.image_2.texels_with(overlay),
        "{label_2}: la textura compuesta no es el documento con la superposición escrita"
    );
}

// ---------------------------------------------------------------------------------------------
// Fila 5: damero sin bordes grises
// ---------------------------------------------------------------------------------------------

/// Fila 5 para una escala, un zoom y un camino: cada píxel físico del rectángulo de la imagen se
/// ve exactamente `shade_at`, fuera `BACKGROUND`.
fn row_5_scene(ppp: f32, zoom: u32, mesh: bool) -> (Bench, Scene) {
    let label_2 = format!("ppp {ppp} ×{zoom} malla={mesh}");
    let mut bench = Bench::new_with(ppp, image_transparent(), CanvasBackground::Checker, mesh);
    bench.with_zoom(zoom);
    let scene = bench.scene(&[]);
    let g = scene.geom;
    assert_eq!(
        g.z,
        i64::from(zoom),
        "{label_2}: el zoom se mide en píxeles físicos"
    );
    assert_eq!(g.width_2, 64 * g.z);
    assert_eq!(g.height_2, 48 * g.z);
    let f = failures(&scene, &bench.image_2, &[], CanvasBackground::Checker);
    without_failures(&f, &label_2);
    // Y con la función de la spec, `shade_at`, en cada píxel de la imagen (también la primera y la
    // última fila y columna).
    for py in g.source_2.1..g.source_2.1 + g.height_2 {
        for px in g.source_2.0..g.source_2.0 + g.width_2 {
            let expected_2 = shade_at(px, py, g.source_2, g.cell).color();
            let seen = scene.buffer.en(px, py);
            assert!(
                seen.near(Seen::de_color(gray_2(expected_2)), TOLERANCE),
                "{label_2}: ({px}, {py}) se ve {seen:?} y `shade_at` da {expected_2:?}"
            );
        }
    }
    (bench, scene)
}

#[test]
fn row_5_each_pixel_of_the_image_is_seen_exactly_like_shade_at_and_outside_the_background() {
    for ppp in PPPS {
        for zoom in 1..=3 {
            row_5_scene(ppp, zoom, false);
        }
    }
}

/// Fila 5, tras arrastrar `(dx, dy)`: el color de cada píxel físico de la imagen relativo a su
/// esquina es el mismo que antes (el damero va con la imagen).
fn row_5_after_dragging(ppp: f32, zoom: u32, offsets: &[(i64, i64)], mesh: bool) {
    let (mut bench, antes) = row_5_scene(ppp, zoom, mesh);
    let g0 = antes.geom;
    let (mut x, mut y) = g0.source_2;
    for &(dx, dy) in offsets {
        let label_2 = format!("ppp {ppp} ×{zoom} arrastre ({dx}, {dy}) malla={mesh}");
        bench.drags(dx, dy);
        let after_2 = bench.scene(&[]);
        let g1 = after_2.geom;
        assert_eq!(
            g1.source_2,
            (x + dx, y + dy),
            "{label_2}: la imagen se desplazó exactamente lo arrastrado"
        );
        (x, y) = g1.source_2;
        let f = failures(&after_2, &bench.image_2, &[], CanvasBackground::Checker);
        without_failures(&f, &label_2);
        for ry in 0..g0.height_2 {
            for rx in 0..g0.width_2 {
                let antes_ = antes.buffer.en(g0.source_2.0 + rx, g0.source_2.1 + ry);
                let after_ = after_2.buffer.en(g1.source_2.0 + rx, g1.source_2.1 + ry);
                assert!(
                    antes_.near(after_, TOLERANCE),
                    "{label_2}: el píxel ({rx}, {ry}) relativo a la esquina era {antes_:?} y ahora es {after_:?}"
                );
            }
        }
    }
}

#[test]
fn row_5_after_dragging_the_view_7_and_13_pixels_the_checker_goes_with_the_image() {
    for ppp in PPPS {
        for zoom in 1..=3 {
            // Un arrastre de 7 píxeles en x y 13 en y, y luego otro de 13 en x y 7 en y.
            row_5_after_dragging(ppp, zoom, &[(7, 13), (13, 7)], false);
        }
    }
}

// ---------------------------------------------------------------------------------------------
// Fila 6: capas, textura, coste y documento
// ---------------------------------------------------------------------------------------------

/// Una imagen indexada de 64×48 con cuatro entradas y la entrada 0 transparente.
fn indexed() -> Image {
    let mut palette = Palette::from_colors([
        Rgba::new(0, 0, 0, 0),
        Rgba::opaque(255, 0, 0),
        Rgba::opaque(0, 255, 0),
        Rgba::opaque(0, 0, 255),
    ])
    .expect("paleta");
    palette
        .set_transparent_index(Some(0))
        .expect("índice transparente");
    let mut image = Image::new_indexed(64, 48, palette).expect("imagen indexada");
    for x in 0..32 {
        image.set_pixel(x, 3, Pixel::Indexed(1)).expect("dentro");
    }
    image
}

#[test]
fn row_6_the_first_frame_paints_background_checker_and_image_in_that_order() {
    for ppp in PPPS {
        let mut bench = Bench::without_warm_up(
            TEST_SCREEN,
            ppp,
            image_transparent(),
            CanvasBackground::Checker,
            false,
        );
        assert_eq!(bench.canvas.checker_texture_id(), None, "antes de pintar");
        let output_2 = bench.runs(vec![], &[]);
        assert!(bench.canvas.checker_texture_id().is_some());
        let label_2 = format!("ppp {ppp} primer fotograma");
        checks_base(
            &bench,
            &output_2,
            CanvasBackground::Checker,
            false,
            &label_2,
        );
        // Mientras las cuenta: tres capas, una malla de la textura del damero, otra de la imagen.
        let layers = output_2.layers();
        assert_eq!(layers.len(), 3);
        assert!(matches!(layers[0], Layer::TestFill { .. }));
        assert!(matches!(layers[1], Layer::TestMesh { .. }));
        assert!(matches!(layers[2], Layer::TestMesh { .. }));
    }
}

#[test]
fn row_6_the_checker_texture_is_2x2_with_wrapping_with_the_texels_and_is_uploaded_once() {
    for ppp in PPPS {
        let mut bench = Bench::without_warm_up(
            TEST_SCREEN,
            ppp,
            image_transparent(),
            CanvasBackground::Checker,
            false,
        );
        let first = bench.runs(vec![], &[]);
        let checker_2 = bench
            .canvas
            .checker_texture_id()
            .expect("textura del damero");
        let image_2 = bench.canvas.texture_id().expect("textura de la imagen");
        assert_ne!(checker_2, image_2);
        let uploads = first.uploads_of(checker_2);
        assert_eq!(uploads.len(), 1, "una sola subida de la textura del damero");
        let delta = uploads[0];
        assert!(delta.is_whole(), "se sube entera");
        assert_eq!(delta.options, TextureOptions::NEAREST_REPEAT);
        let ImageData::Color(color) = &delta.image;
        assert_eq!(color.size, [2, 2], "ppp {ppp}");
        let expected_2: Vec<Color32> = texels().iter().map(|c| gray_2(*c)).collect();
        assert_eq!(
            color.pixels, expected_2,
            "píxeles = texels() = [A, B, B, A]"
        );
        assert_eq!(expected_2[0], gray_2(CHECK_A));
        assert_eq!(expected_2[1], gray_2(CHECK_B));
        // La imagen se sube una vez, con su contenido.
        let of_the_image = first.uploads_of(image_2);
        assert_eq!(of_the_image.len(), 1);
        let ImageData::Color(color) = &of_the_image[0].image;
        assert_eq!(color.size, [64, 48]);
        assert_eq!(color.pixels, bench.image_2.texels());
        assert_eq!(first.uploads.len(), 2, "solo esas dos subidas");

        // Ningún fotograma posterior sube la textura del damero.
        for k in 0..4 {
            let f = bench.runs(vec![], &[]);
            assert!(f.uploads.is_empty(), "ppp {ppp} fotograma {k}: sin subidas");
            assert!(f.released.is_empty());
            assert_eq!(bench.canvas.checker_texture_id(), Some(checker_2));
        }
    }
}

#[test]
fn row_6_the_checker_keeps_its_texture_with_pan_zoom_refresh_set_image_and_preview_entry() {
    let ppp = 1.5;
    let mut bench = Bench::without_warm_up(
        TEST_SCREEN,
        ppp,
        image_transparent(),
        CanvasBackground::Checker,
        false,
    );
    let first = bench.runs(vec![], &[]);
    checks_base(
        &bench,
        &first,
        CanvasBackground::Checker,
        false,
        "primer fotograma",
    );
    let checker_2 = bench
        .canvas
        .checker_texture_id()
        .expect("textura del damero");
    let reviews = |bench: &Bench, output_2: &TestOutput, label_2: &str| {
        assert_eq!(
            bench.canvas.checker_texture_id(),
            Some(checker_2),
            "{label_2}: el id no cambia"
        );
        assert!(
            output_2.uploads_of(checker_2).is_empty(),
            "{label_2}: ninguna subida del damero"
        );
        assert!(
            !output_2.released.contains(&checker_2),
            "{label_2}: no se libera"
        );
        checks_base(bench, output_2, CanvasBackground::Checker, false, label_2);
    };

    // Un `pan`.
    for (k, output_2) in bench.drags(7, 13).iter().enumerate() {
        reviews(&bench, output_2, &format!("pan, fotograma {k}"));
    }
    // Un `zoom`.
    bench.canvas.request_view(ViewOp::ZoomIn);
    let output_2 = bench.runs(vec![], &[]);
    reviews(&bench, &output_2, "zoom");
    // `refresh`: la imagen se sube de nuevo; el damero, no.
    let image_2 = bench.canvas.texture_id().expect("textura de la imagen");
    bench.refreshes(image_transparent().with(3, 3, RED));
    let output_2 = bench.runs(vec![], &[]);
    reviews(&bench, &output_2, "refresh");
    assert_eq!(
        output_2.uploads_of(image_2).len(),
        1,
        "refresh sube la imagen"
    );
    checks_texture(&bench, &[], "refresh");
    // `set_image` con otra imagen (otro tamaño): otra textura de la imagen, el mismo damero.
    bench.sets_image(TestImage::uniform(40, 30, TRANSPARENT));
    let output_2 = bench.runs(vec![], &[]);
    reviews(&bench, &output_2, "set_image");
    assert_ne!(bench.canvas.texture_id(), Some(image_2), "otra imagen");
    // `preview_entry` sobre una indexada.
    let indexed = indexed();
    bench.canvas.set_image(&indexed);
    bench.document = indexed.clone();
    bench.image_2 = TestImage {
        width_2: 64,
        height_2: 48,
        pixels_2: vec![TRANSPARENT; 64 * 48],
    };
    let output_2 = bench.runs(vec![], &[]);
    reviews(&bench, &output_2, "set_image indexada");
    let texture_indexed = bench.canvas.texture_id().expect("textura de la imagen");
    bench
        .canvas
        .preview_entry(&bench.ctx, &indexed, 2, Rgba::opaque(9, 9, 9));
    let output_2 = bench.runs(vec![], &[]);
    reviews(&bench, &output_2, "preview_entry");
    assert_eq!(
        output_2.uploads_of(texture_indexed).len(),
        1,
        "la vista previa sube la imagen"
    );
    bench.canvas.end_preview(&bench.ctx, &indexed);
    let output_2 = bench.runs(vec![], &[]);
    reviews(&bench, &output_2, "end_preview");
}

#[test]
fn row_6_with_a_2048x64_image_at_x64_the_checker_does_not_measure_more_than_the_canvas() {
    for ppp in PPPS {
        let mut bench = Bench::new_2(ppp, TestImage::uniform(2048, 64, TRANSPARENT));
        bench.with_zoom(64);
        let output_2 = bench.runs(vec![], &[]);
        let label_2 = format!("ppp {ppp} 2048×64 a ×64");
        let g = bench.geom(output_2.info());
        assert_eq!(g.width_2, 131_072, "{label_2}: el rectángulo de la imagen");
        assert_eq!(g.height_2, 4096, "{label_2}");
        let i = checks_base(
            &bench,
            &output_2,
            CanvasBackground::Checker,
            false,
            &label_2,
        );
        assert_eq!(i, 2);
        let layers = output_2.layers();
        let Layer::TestMesh { vertices, .. } = &layers[1] else {
            panic!("{label_2}: el damero es una malla");
        };
        let toolbox_2 = toolbox_2(vertices);
        let canvas_2 = Rect::from_min_size(Pos2::ZERO, bench.screen);
        assert!(
            toolbox_2.min.x >= canvas_2.min.x - 1e-3
                && toolbox_2.min.y >= canvas_2.min.y - 1e-3
                && toolbox_2.max.x <= canvas_2.max.x + 1e-3
                && toolbox_2.max.y <= canvas_2.max.y + 1e-3,
            "{label_2}: el damero cabe en el lienzo: {toolbox_2:?}"
        );
        // Los uv no se disparan: a lo sumo unos cientos de períodos.
        let uv = toolbox_uv(vertices);
        assert!(
            (0.0..1.0).contains(&uv[0]) && (0.0..1.0).contains(&uv[1]),
            "{label_2}: {uv:?}"
        );
        assert!(
            uv[2] - uv[0] < 400.0 && uv[3] - uv[1] < 400.0,
            "{label_2}: {uv:?}"
        );
    }
}

#[test]
fn row_6_with_too_large_there_is_no_checker_or_checker_texture_only_the_background() {
    // `TooLarge` ya no depende de la textura máxima de la GPU: solo de un lado mayor que
    // `Canvas::MAX_SIDE` (16384).
    for ppp in PPPS {
        let mut bench = Bench::without_warm_up(
            TEST_SCREEN,
            ppp,
            TestImage::uniform(16385, 1, TRANSPARENT),
            CanvasBackground::Checker,
            false,
        );
        for k in 0..3 {
            let output_2 = bench.runs(vec![], &[]);
            assert_eq!(
                output_2.info().error,
                Some(CanvasError::TooLarge {
                    width: 16385,
                    height: 1,
                    max: 16384
                }),
                "ppp {ppp} fotograma {k}"
            );
            assert!(output_2.info().image_rect.is_none());
            let layers = output_2.layers();
            assert_eq!(layers.len(), 1, "ppp {ppp}: solo BACKGROUND");
            let Layer::TestFill { color, .. } = &layers[0] else {
                panic!("el fondo es un rectángulo");
            };
            assert_eq!(*color, BACKGROUND);
            assert_eq!(
                bench.canvas.checker_texture_id(),
                None,
                "sin textura del damero"
            );
            assert_eq!(bench.canvas.texture_id(), None);
            // El primer fotograma de egui sube el atlas de la fuente (`Managed(0)`), pero no se
            // sube ninguna textura del lienzo.
            assert!(
                output_2
                    .uploads
                    .iter()
                    .all(|(id, _)| *id == TextureId::default()),
                "sin subidas del lienzo: {:?}",
                output_2
                    .uploads
                    .iter()
                    .map(|(id, _)| id)
                    .collect::<Vec<_>>()
            );
        }
    }
}

#[test]
fn row_6_the_document_the_image_texture_and_the_state_id_stay_intact() {
    let ppp = 2.0;
    let image_2 = image_transparent().with(5, 5, RED).with(6, 5, RED_MIDDLE);
    let doc = Document::new(image_2.image());
    let id = ImageId::from_raw(0);
    let state_id = doc.state_id();
    let bytes = to_rgba8(doc.image(id).expect("imagen"));
    let mut bench = Bench::without_warm_up(
        TEST_SCREEN,
        ppp,
        image_2.clone(),
        CanvasBackground::Checker,
        false,
    );
    let first = bench.runs(vec![], &[]);
    let texture_2 = bench.canvas.texture_id().expect("textura de la imagen");
    let upload = first.uploads_of(texture_2);
    assert_eq!(upload.len(), 1);
    let ImageData::Color(color) = &upload[0].image;
    // Lo que sube el lienzo es la imagen, intacta: los mismos bytes que `to_rgba8`.
    let expected_2 = egui::ColorImage::from_rgba_unmultiplied([64, 48], &bytes);
    assert_eq!(color.pixels, expected_2.pixels);
    // Con la superposición la textura es el documento con esos píxeles escritos, y varios
    // fotogramas con cada fondo no la tocan (la superposición ya está en la textura).
    let overlay = blocks();
    bench.runs(vec![], &overlay);
    checks_texture(&bench, &overlay, "con la superposición");
    for background_2 in BACKGROUNDS {
        bench.canvas.set_background(background_2);
        for _ in 0..2 {
            let output_2 = bench.runs(vec![], &overlay);
            assert!(
                output_2.uploads_of(texture_2).is_empty(),
                "{background_2:?}"
            );
        }
        checks_texture(&bench, &overlay, &format!("{background_2:?}"));
    }
    // Al quitarla (y `sync`), la textura vuelve a ser el documento, byte a byte.
    bench.runs(vec![], &[]);
    checks_texture(&bench, &[], "tras quitar la superposición");
    assert!(bench.textures_2.get(texture_2).expect("textura").texels == expected_2.pixels);
    assert_eq!(to_rgba8(doc.image(id).expect("imagen")), bytes);
    assert_eq!(doc.state_id(), state_id);
    assert!(!doc.can_undo());
}

// ---------------------------------------------------------------------------------------------
// Fila 7: la superposición con el damero
// ---------------------------------------------------------------------------------------------

/// Fila 7 (y 14 y 15 con otro fondo o camino): el banco de bloques de `overlay_small_zoom.rs`. La
/// superposición va en la textura de la baldosa: no hay ninguna forma de superposición
/// (`checks_base`) y cada píxel físico se ve como el oráculo (`failures`), a ×1 también los de
/// 1×1 píxel físico.
fn blocks_with(ppp: f32, zoom: u32, background_2: CanvasBackground, mesh: bool) {
    let label_2 = format!("ppp {ppp} ×{zoom} {background_2:?} malla={mesh}");
    let mut bench = Bench::new_with(ppp, image_blue(), background_2, mesh);
    bench.with_zoom(zoom);
    let overlay = blocks();
    let scene = bench.scene(&overlay);
    let g = scene.geom;
    assert_eq!(
        g.z,
        i64::from(zoom),
        "{label_2}: el zoom se mide en píxeles físicos"
    );
    checks_base(&bench, &scene.output_2, background_2, mesh, &label_2);
    checks_texture(&bench, &overlay, &label_2);
    let f = failures(&scene, &bench.image_2, &overlay, background_2);
    without_failures(&f, &label_2);
}

#[test]
fn row_7_the_overlay_has_no_shapes_of_its_own_and_looks_exact() {
    for ppp in PPPS {
        for zoom in 1..=3 {
            blocks_with(ppp, zoom, CanvasBackground::Checker, false);
        }
    }
}

#[test]
fn row_7_at_x1_the_pixels_are_seen_as_1x1_physical_pixel_at_any_scale() {
    // El fallo de `b702d8e`: a ×1 la superposición de 1×1 píxel físico no se veía. Con la
    // superposición en la textura se reexpresa así: no hay formas que se degeneren y cada píxel
    // pendiente se ve.
    for ppp in PPPS {
        let mut bench = Bench::new_2(ppp, image_blue());
        bench.with_zoom(1);
        let overlay = blocks();
        let scene = bench.scene(&overlay);
        assert_eq!(scene.geom.z, 1);
        checks_base(
            &bench,
            &scene.output_2,
            CanvasBackground::Checker,
            false,
            &format!("ppp {ppp} ×1"),
        );
        let f = failures(&scene, &bench.image_2, &overlay, CanvasBackground::Checker);
        without_failures(&f, &format!("ppp {ppp} ×1"));
        // Y los 24 píxeles pendientes no se ven como la imagen azul que hay debajo.
        for &(x, y, color) in &overlay {
            let b = scene.geom.block(x, y);
            let seen = scene.buffer.en(b.x, b.y);
            let blue = Seen::de_color(Color32::from_rgb(0, 0, 255));
            if color.a < 255 {
                assert!(!seen.near(blue, TOLERANCE), "({x}, {y}) con {color:?}");
            }
        }
    }
}

#[test]
fn row_7_with_only_opaque_pixels_there_are_no_shapes_in_the_overlay_and_they_look_opaque() {
    for ppp in PPPS {
        let mut bench = Bench::new_2(ppp, image_blue());
        let overlay: Vec<TestPending> = (0..6).map(|x| (10 + x, 4, RED)).collect();
        let scene = bench.scene(&overlay);
        let layers = scene.output_2.layers();
        let checker_2 = bench
            .canvas
            .checker_texture_id()
            .expect("textura del damero");
        let quads = layers
            .iter()
            .filter(|c| matches!(c, Layer::TestMesh { texture_2, .. } if *texture_2 == checker_2))
            .count();
        assert_eq!(quads, 1, "ppp {ppp}: solo el damero de fondo de la imagen");
        // Fondo, damero y la baldosa: ningún rectángulo de color por píxel.
        let i = checks_base(
            &bench,
            &scene.output_2,
            CanvasBackground::Checker,
            false,
            "opacos",
        );
        assert_eq!(layers.len(), i + 1, "ppp {ppp}: la baldosa y nada más");
        let f = failures(&scene, &bench.image_2, &overlay, CanvasBackground::Checker);
        without_failures(&f, &format!("ppp {ppp} opacos"));
        // Cada píxel pendiente se ve rojo opaco, sin el damero.
        let red = Seen::de_color(gray_2(RED));
        for &(x, y, _) in &overlay {
            let b = scene.geom.block(x, y);
            for py in b.y..b.y + b.height {
                for px in b.x..b.x + b.width {
                    let seen = scene.buffer.en(px, py);
                    assert!(seen.near(red, TOLERANCE), "({px}, {py}) se ve {seen:?}");
                }
            }
        }
    }
}

/// El trazo del borrador de la fila 10 de la 17a: veinte píxeles de alfa 0 sobre una imagen azul,
/// varios fotogramas, `End` (la imagen cambia) y `refresh`. El borrador deja ver el fondo
/// elegido: se mira muestreando la textura (cada píxel pendiente es transparente) y, tras
/// rasterizar, el color de cada píxel borrado es el mismo antes y después de soltar.
fn stroke_of_the_eraser(ppp: f32, background_2: CanvasBackground, mesh: bool) {
    let label_2 = format!("ppp {ppp} {background_2:?} malla={mesh}");
    let mut bench = Bench::new_with(ppp, image_blue(), background_2, mesh);
    let text_image = bench.canvas.texture_id().expect("textura de la imagen");
    let checker_before = bench.canvas.checker_texture_id();
    let pixels_2 = the_twenty();
    let overlay_de = |n: usize| -> Vec<TestPending> {
        pixels_2[..n]
            .iter()
            .map(|&(x, y)| (x, y, TRANSPARENT))
            .collect()
    };
    // Cada fotograma añade un píxel: la textura lo lleva transparente y no hay formas nuevas.
    for n in 1..=20 {
        let overlay = overlay_de(n);
        let output_2 = bench.runs(vec![], &overlay);
        checks_base(&bench, &output_2, background_2, mesh, &label_2);
        checks_texture(&bench, &overlay, &format!("{label_2}: {n} píxeles"));
    }
    // Varios fotogramas más con los veinte, sin cambios: no hay nada nuevo que subir.
    let overlay = overlay_de(20);
    for _ in 0..3 {
        let output_2 = bench.runs(vec![], &overlay);
        assert!(output_2.uploads.is_empty());
    }
    let with_overlay = bench.scene(&overlay);
    let f = failures(&with_overlay, &bench.image_2, &overlay, background_2);
    without_failures(&f, &format!("{label_2}: con la superposición"));

    // `End` y `refresh`: la imagen con esos veinte píxeles de alfa 0 y sin superposición.
    let mut deleted = image_blue();
    for &(x, y) in &pixels_2 {
        deleted = deleted.with(x, y, TRANSPARENT);
    }
    bench.refreshes(deleted);
    let after_release = bench.scene(&[]);
    assert_eq!(
        after_release.output_2.uploads.len(),
        1,
        "{label_2}: una sola subida tras el refresh"
    );
    assert_eq!(after_release.output_2.uploads_of(text_image).len(), 1);
    assert_eq!(bench.canvas.checker_texture_id(), checker_before);
    checks_base(
        &bench,
        &after_release.output_2,
        background_2,
        mesh,
        &label_2,
    );
    checks_texture(&bench, &[], &format!("{label_2}: tras soltar"));
    let f = failures(&after_release, &bench.image_2, &[], background_2);
    without_failures(&f, &format!("{label_2}: tras soltar"));

    // Antes y después de soltar, igual: cada píxel físico de los veinte bloques.
    let (a, d) = (with_overlay.geom, after_release.geom);
    assert_eq!(a.source_2, d.source_2);
    for &(x, y) in &pixels_2 {
        let b = a.block(x, y);
        for py in b.y..b.y + b.height {
            for px in b.x..b.x + b.width {
                let antes = with_overlay.buffer.en(px, py);
                let after_2 = after_release.buffer.en(px, py);
                assert!(
                    antes.near(after_2, TOLERANCE),
                    "{label_2}: el píxel físico ({px}, {py}) de ({x}, {y}) se veía {antes:?} con la superposición y {after_2:?} tras soltar"
                );
            }
        }
    }
}

#[test]
fn row_7_the_eraser_stroke_looks_the_same_before_and_after_releasing() {
    for ppp in PPPS {
        stroke_of_the_eraser(ppp, CanvasBackground::Checker, false);
    }
}

// ---------------------------------------------------------------------------------------------
// Fila 8: límites
// ---------------------------------------------------------------------------------------------

#[test]
fn row_8_a_1x1_image_at_x64_is_a_64x64_quad_with_uv_0_0_4_4_at_cell_8() {
    for ppp in PPPS {
        let mut bench = Bench::new_2(ppp, TestImage::uniform(1, 1, TRANSPARENT));
        let scene = bench.scene(&[]);
        let label_2 = format!("ppp {ppp} 1×1");
        let g = scene.geom;
        assert_eq!(g.z, 64, "{label_2}: el ajuste da ×64");
        assert_eq!((g.width_2, g.height_2), (64, 64));
        let i = checks_base(
            &bench,
            &scene.output_2,
            CanvasBackground::Checker,
            false,
            &label_2,
        );
        let Layer::TestMesh { vertices, .. } = &scene.output_2.layers()[i - 1] else {
            panic!("{label_2}: el damero es una malla");
        };
        let toolbox_pos = toolbox_2(vertices);
        assert!(
            almost(toolbox_pos, rect_in_points(g.rect(), ppp)),
            "{label_2}: 64×64 píxeles físicos: {toolbox_pos:?}"
        );
        let uv = toolbox_uv(vertices);
        if g.cell == 8 {
            assert_eq!(uv, [0.0, 0.0, 4.0, 4.0], "{label_2}: a cell 8");
        }
        let periods = 64.0 / (2.0 * g.cell as f32);
        assert!(
            (uv[2] - periods).abs() < 1e-5 && (uv[3] - periods).abs() < 1e-5,
            "{label_2}: {uv:?}"
        );
        assert_eq!(
            (uv[0], uv[1]),
            (0.0, 0.0),
            "{label_2}: con el origen en la esquina"
        );
        let f = failures(&scene, &bench.image_2, &[], CanvasBackground::Checker);
        without_failures(&f, &label_2);
    }
}

#[test]
fn row_8_with_the_view_at_the_extreme_the_quad_is_only_the_visible_strip_and_looks_right() {
    for ppp in PPPS {
        for zoom in 1..=3 {
            for (sx, sy) in [(-1, -1), (1, 1), (-1, 1), (1, -1)] {
                let label_2 = format!("ppp {ppp} ×{zoom} extremo ({sx}, {sy})");
                let mut bench = Bench::new_2(ppp, image_transparent());
                bench.with_zoom(zoom);
                bench.drags_to_the_end(sx, sy);
                let scene = bench.scene(&[]);
                let g = scene.geom;
                let vp = bench.viewport();
                // Quedan visibles `MIN_VISIBLE` píxeles por eje (View::clamp), como mucho.
                let visible = |source_2: i64, largo: i64, canvas_2: u32| {
                    let from_2 = source_2.max(0);
                    let until = (source_2 + largo).min(i64::from(canvas_2));
                    until - from_2
                };
                assert_eq!(
                    visible(g.source_2.0, g.width_2, vp.width),
                    i64::from(MIN_VISIBLE),
                    "{label_2}: visibles en x"
                );
                assert_eq!(
                    visible(g.source_2.1, g.height_2, vp.height),
                    i64::from(MIN_VISIBLE),
                    "{label_2}: visibles en y"
                );
                let i = checks_base(
                    &bench,
                    &scene.output_2,
                    CanvasBackground::Checker,
                    false,
                    &label_2,
                );
                let Layer::TestMesh { vertices, .. } = &scene.output_2.layers()[i - 1] else {
                    panic!("{label_2}: el damero es una malla");
                };
                let band = g.clip_2().expect("hay franja visible");
                assert_eq!(
                    (band.width, band.height),
                    (i64::from(MIN_VISIBLE), i64::from(MIN_VISIBLE)),
                    "{label_2}: la franja visible"
                );
                // Dentro del lienzo, y solo la franja.
                let canvas_2 = Rect::from_min_size(Pos2::ZERO, bench.screen);
                let toolbox_pos = toolbox_2(vertices);
                assert!(
                    toolbox_pos.min.x >= canvas_2.min.x - 1e-3
                        && toolbox_pos.min.y >= canvas_2.min.y - 1e-3
                        && toolbox_pos.max.x <= canvas_2.max.x + 1e-3
                        && toolbox_pos.max.y <= canvas_2.max.y + 1e-3,
                    "{label_2}: {toolbox_pos:?}"
                );
                assert!(
                    almost(toolbox_pos, rect_in_points(band, ppp)),
                    "{label_2}: {toolbox_pos:?}"
                );
                // El uv parte de la fase correcta y cada píxel se ve como `shade_at`.
                let f = failures(&scene, &bench.image_2, &[], CanvasBackground::Checker);
                without_failures(&f, &label_2);
                for py in band.y..band.y + band.height {
                    for px in band.x..band.x + band.width {
                        let expected_2 = gray_2(shade_at(px, py, g.source_2, g.cell).color());
                        assert!(
                            scene
                                .buffer
                                .en(px, py)
                                .near(Seen::de_color(expected_2), TOLERANCE),
                            "{label_2}: ({px}, {py})"
                        );
                    }
                }
            }
        }
    }
}

#[test]
fn row_8_with_a_1x1_physical_pixel_canvas_there_is_no_panic_and_the_clip_is_none_or_1x1() {
    for ppp in PPPS {
        let screen = vec2(1.0 / ppp, 1.0 / ppp);
        let mut bench = Bench::without_warm_up(
            screen,
            ppp,
            image_transparent(),
            CanvasBackground::Checker,
            false,
        );
        assert_eq!(bench.viewport(), Size::new(1, 1), "ppp {ppp}");
        // Ajustar da ahora 1/K en un lienzo de 1×1 (el rasterizador del banco es el de los zooms
        // enteros): se pide ×1, el zoom en el que esta prueba se escribió.
        bench.canvas.request_view(ViewOp::Actual);
        for _ in 0..3 {
            bench.runs(vec![], &[]);
        }
        let scene = bench.scene(&[]);
        let label_2 = format!("ppp {ppp} lienzo de 1×1");
        let clip_2 = scene.geom.clip_2();
        assert!(
            matches!(
                clip_2,
                None | Some(ScreenRect {
                    width: 1,
                    height: 1,
                    ..
                })
            ),
            "{label_2}: {clip_2:?}"
        );
        checks_base(
            &bench,
            &scene.output_2,
            CanvasBackground::Checker,
            false,
            &label_2,
        );
        let f = failures(&scene, &bench.image_2, &[], CanvasBackground::Checker);
        without_failures(&f, &label_2);
        // Y se puede arrastrar y hacer zoom sin pánico.
        bench.canvas.request_view(ViewOp::ZoomIn);
        bench.runs(vec![mover(pos2(0.2 / ppp, 0.2 / ppp))], &[]);
        bench.runs(
            vec![wheel_line(true), key_2(Key::Home, Modifiers::NONE)],
            &[],
        );
    }
}

#[test]
fn row_8_without_an_image_only_the_background_is_painted_without_failure_or_checker_texture() {
    for ppp in PPPS {
        let mut bench = Bench {
            ctx: context(ppp),
            ppp,
            screen: TEST_SCREEN,
            canvas: Canvas::new(),
            image_2: TestImage::uniform(1, 1, TRANSPARENT),
            document: TestImage::uniform(1, 1, TRANSPARENT).image(),
            textures_2: Textures::new(),
        };
        for k in 0..3 {
            let output_2 = bench.runs(
                vec![wheel_line(true), key_2(Key::Home, Modifiers::NONE)],
                &blocks(),
            );
            let layers = output_2.layers();
            assert_eq!(layers.len(), 1, "ppp {ppp} fotograma {k}: solo el fondo");
            assert!(matches!(&layers[0], Layer::TestFill { color, .. } if *color == BACKGROUND));
            assert_eq!(bench.canvas.checker_texture_id(), None);
            assert_eq!(bench.canvas.texture_id(), None);
            assert!(output_2.uploads.is_empty(), "sin subidas");
            assert!(output_2.info().image_rect.is_none());
            assert!(output_2.info().error.is_none());
        }
    }
}

#[test]
fn row_8_with_the_maximum_texture_of_1024_and_an_image_1025_wide_it_is_painted_in_two_tiles() {
    // Antes `TooLarge` y solo el fondo; ahora la imagen se abre y se pinta en dos baldosas (de 1024×2
    // y de 1×2) con el damero debajo. El lienzo es lo bastante ancho para que Ajustar dé ×1 y se
    // vea entera, y la superposición cruza la unión de las dos baldosas.
    let image_2 = TestImage::uniform(1025, 2, RED)
        .with(1023, 0, RED_MIDDLE)
        .with(1023, 1, TRANSPARENT)
        .with(1024, 0, BLUE)
        .with(1024, 1, RED_MIDDLE);
    let mut bench = Bench::without_warm_up(
        vec2(1100.0, 300.0),
        1.0,
        image_2,
        CanvasBackground::Checker,
        false,
    );
    let mut overlay = blocks();
    overlay.extend([
        (1022, 0, BLUE),
        (1023, 0, TRANSPARENT),
        (1024, 0, RED_MIDDLE),
        (1024, 1, TRANSPARENT),
    ]);
    let mut last = None;
    for _ in 0..2 {
        let output_2 = bench.runs_max(vec![], &overlay, Some(1024));
        assert_eq!(output_2.info().error, None, "se abre sin error");
        last = Some(output_2);
    }
    let output_2 = last.expect("dos fotogramas");
    let ids = bench.canvas.texture_ids();
    assert_eq!(ids.len(), 2, "dos baldosas de 1024 de lado como mucho");
    let sizes: Vec<[usize; 2]> = ids
        .iter()
        .map(|id| bench.textures_2.get(*id).expect("contenido").size_2)
        .collect();
    assert_eq!(sizes, [[1024, 2], [1, 2]], "tamaños recortados");
    let i = checks_base(
        &bench,
        &output_2,
        CanvasBackground::Checker,
        false,
        "1025×2",
    );
    assert_eq!(output_2.layers().len(), i + 2, "una malla por baldosa");
    let scene = bench.rasterizes_output(output_2);
    assert_eq!(scene.geom.z, 1, "Ajustar da ×1");
    assert_eq!(scene.geom.width_2, 1025);
    // Cada píxel físico es el oráculo: incluida la unión de las baldosas, sin huecos ni solapes
    // (un hueco dejaría ver el damero bajo el rojo; un solape compondría dos veces el alfa).
    let f = failures(&scene, &bench.image_2, &overlay, CanvasBackground::Checker);
    without_failures(&f, "1025×2 con textura máxima de 1024");
}

// ---------------------------------------------------------------------------------------------
// Fila 14: la superposición con cada fondo
// ---------------------------------------------------------------------------------------------

#[test]
fn row_14_with_a_solid_background_the_overlay_has_no_shapes_of_its_own_and_looks_exact() {
    for background_2 in SOLID {
        for ppp in PPPS {
            for zoom in 1..=3 {
                blocks_with(ppp, zoom, background_2, false);
            }
        }
    }
}

#[test]
fn row_14_with_a_solid_background_the_checker_texture_is_not_created() {
    for background_2 in SOLID {
        let mut bench = Bench::without_warm_up(TEST_SCREEN, 1.0, image_blue(), background_2, false);
        for _ in 0..3 {
            let output_2 = bench.runs(vec![], &blocks());
            assert_eq!(bench.canvas.checker_texture_id(), None, "{background_2:?}");
            let image_2 = bench.canvas.texture_id().expect("textura de la imagen");
            assert_eq!(
                output_2.uploads.len(),
                output_2.uploads_of(image_2).len(),
                "{background_2:?}: solo se sube la baldosa de la imagen"
            );
        }
        assert_eq!(bench.canvas.background(), background_2);
    }
}

#[test]
fn row_14_the_eraser_stroke_looks_the_same_before_and_after_releasing_with_each_solid() {
    for background_2 in SOLID {
        for ppp in PPPS {
            stroke_of_the_eraser(ppp, background_2, false);
        }
    }
}

/// Un banco con documento y herramienta, como la app: el trazo del borrador sobre una imagen azul.
struct TestStroke {
    bench: Bench,
    doc: Document,
    tool: ToolState,
    /// Rectángulo de la imagen y zoom del primer fotograma.
    rect: Rect,
    zoom: i64,
}

impl TestStroke {
    fn new_2(ppp: f32) -> TestStroke {
        let bench = Bench::without_warm_up(
            vec2(800.0, 600.0),
            ppp,
            image_blue(),
            CanvasBackground::Checker,
            false,
        );
        let doc = Document::new(bench.image_2.image());
        let mut tool = ToolState::new();
        tool.apply_action(ToolAction::Select(Tool::Eraser));
        assert_eq!(tool.tool(), Tool::Eraser);
        let mut stroke_2 = TestStroke {
            bench,
            doc,
            tool,
            rect: Rect::NOTHING,
            zoom: 0,
        };
        stroke_2.frame(vec![]);
        let output_2 = stroke_2.frame(vec![]);
        stroke_2.rect = output_2.info().image_rect.expect("hay imagen");
        stroke_2.zoom = output_2.info().zoom.scale(1);
        stroke_2
    }

    /// Un fotograma como el de la app: eventos del lienzo a la herramienta, `set_overlay` con la
    /// superposición de `ToolState::preview`, `refresh` si el documento cambió (si no, `sync`) y
    /// `paint`.
    fn frame(&mut self, events: Vec<Event>) -> TestOutput {
        let doc = &mut self.doc;
        let tool = &mut self.tool;
        let id = ImageId::from_raw(0);
        self.bench.runs_with(events, None, |canvas, ui| {
            let mut change = false;
            for input_2 in canvas
                .interact_roles(ui)
                .into_iter()
                .map(|(input, _)| input)
                .collect::<Vec<_>>()
            {
                change |= tool
                    .canvas_input_with(doc, id, None, input_2)
                    .expect("el borrador no falla");
            }
            let image = doc.image(id).expect("hay imagen");
            let overlay: Vec<TestPending> = tool
                .preview()
                .filter_map(|e| Some((e.x, e.y, pixel_rgba(image, e.pixel)?)))
                .collect();
            canvas.set_overlay(ui.ctx(), image, &overlay);
            if change {
                canvas.refresh(ui.ctx(), image);
            } else {
                canvas.sync(ui.ctx(), image).expect("el documento se carga");
            }
            let info = canvas.paint(ui);
            (vec![], Some(info))
        })
    }

    fn center(&self, x: i32, y: i32) -> Pos2 {
        let z = self.zoom as f32;
        pos2(
            self.rect.min.x + (x as f32 + 0.5) * z,
            self.rect.min.y + (y as f32 + 0.5) * z,
        )
    }

    fn pending_2(&self) -> Vec<TestPending> {
        let image = self.doc.image(ImageId::from_raw(0)).expect("imagen");
        self.tool
            .preview()
            .filter_map(|e| Some((e.x, e.y, pixel_rgba(image, e.pixel)?)))
            .collect()
    }

    /// Rasteriza el fotograma y comprueba que cada píxel físico se ve como el oráculo: la imagen
    /// azul del documento (que no cambia hasta soltar) con los píxeles pendientes borrados sobre
    /// el fondo.
    fn it_looks_like_the_oracle(
        &self,
        output_2: TestOutput,
        background_2: CanvasBackground,
        label_2: &str,
    ) {
        let scene = self.bench.rasterizes_output(output_2);
        let f = failures(&scene, &self.bench.image_2, &self.pending_2(), background_2);
        without_failures(&f, label_2);
    }
}

#[test]
fn row_14_changing_the_background_in_the_middle_of_a_stroke_changes_the_next_frame_without_touching_anything()
 {
    let mut t = TestStroke::new_2(1.0);
    let state_id = t.doc.state_id();
    let bytes = to_rgba8(t.doc.image(ImageId::from_raw(0)).expect("imagen"));
    let mover_a = |x| mover(x);
    let start_2 = t.center(10, 5);
    t.frame(vec![mover_a(start_2)]);
    t.frame(vec![button_of_2(start_2, PointerButton::Primary, true)]);
    for x in 11..=14 {
        t.frame(vec![mover_a(t.center(x, 5))]);
    }
    assert!(t.tool.busy(), "el trazo está en curso");
    let n_pending = t.pending_2().len();
    assert_eq!(n_pending, 5);
    let checker_2 = t
        .bench
        .canvas
        .checker_texture_id()
        .expect("textura del damero");

    // Con el damero: sin formas de superposición y los cinco píxeles borrados dejan ver el damero.
    let output_2 = t.frame(vec![]);
    checks_base(
        &t.bench,
        &output_2,
        CanvasBackground::Checker,
        false,
        "damero",
    );
    t.it_looks_like_the_oracle(output_2, CanvasBackground::Checker, "damero");

    // Cambia el fondo a mitad de trazo: el siguiente fotograma ya es el sólido.
    for background_2 in [
        CanvasBackground::Black,
        CanvasBackground::White,
        CanvasBackground::Gray,
    ] {
        t.bench.canvas.set_background(background_2);
        assert_eq!(t.bench.canvas.background(), background_2);
        let output_2 = t.frame(vec![]);
        checks_base_with_existing_checker(&t.bench, &output_2, background_2);
        t.it_looks_like_the_oracle(
            output_2,
            background_2,
            &format!("{background_2:?} a mitad de trazo"),
        );
        // El trazo sigue: el documento, la herramienta y la superposición, como estaban.
        assert!(t.tool.busy());
        assert_eq!(t.tool.tool(), Tool::Eraser);
        assert_eq!(t.doc.state_id(), state_id);
        assert!(!t.doc.can_undo());
        assert_eq!(t.pending_2().len(), n_pending);
        assert_eq!(
            to_rgba8(t.doc.image(ImageId::from_raw(0)).expect("imagen")),
            bytes
        );
    }

    // El trazo sigue con el fondo nuevo y, de vuelta a `Checker`, la textura es la misma.
    t.frame(vec![mover_a(t.center(15, 5))]);
    t.bench.canvas.set_background(CanvasBackground::Checker);
    let output_2 = t.frame(vec![mover_a(t.center(16, 5))]);
    assert_eq!(t.bench.canvas.checker_texture_id(), Some(checker_2));
    assert!(
        output_2.uploads_of(checker_2).is_empty(),
        "no se vuelve a subir"
    );
    checks_base(
        &t.bench,
        &output_2,
        CanvasBackground::Checker,
        false,
        "de vuelta",
    );
    t.it_looks_like_the_oracle(output_2, CanvasBackground::Checker, "de vuelta");
    assert_eq!(t.pending_2().len(), 7);
    assert_eq!(t.doc.state_id(), state_id);
}

/// Como `checks_base` para un sólido cuando la textura del damero ya existe de antes (no se
/// vuelve a crear, pero tampoco se usa): sin la comprobación de que no existe.
fn checks_base_with_existing_checker(
    bench: &Bench,
    output_2: &TestOutput,
    background_2: CanvasBackground,
) {
    let layers = output_2.layers();
    let g = bench.geom(output_2.info());
    let checker_2 = bench
        .canvas
        .checker_texture_id()
        .expect("la textura sigue creada");
    assert!(
        !layers
            .iter()
            .any(|c| matches!(c, Layer::TestMesh { texture_2, .. } if *texture_2 == checker_2)),
        "{background_2:?}: ningún cuadrilátero de la textura del damero con un fondo sólido"
    );
    let area = g.clip_2().expect("recorte");
    let Layer::TestFill { rect, color } = &layers[1] else {
        panic!(
            "{background_2:?}: rectángulo del fondo sólido y hay {:?}",
            layers[1]
        );
    };
    assert_eq!(*color, background_in(background_2, 0, 0, (0, 0), 8));
    assert!(almost(*rect, rect_in_points(area, g.ppp)));
    checks_tiles(bench, &g, &layers, 2, &format!("{background_2:?}"));
}

#[test]
fn row_14_going_back_to_checker_creates_the_checker_texture_only_once() {
    let mut bench = Bench::without_warm_up(
        TEST_SCREEN,
        1.0,
        image_blue(),
        CanvasBackground::Black,
        false,
    );
    for k in 0..3 {
        let output_2 = bench.runs(vec![], &blocks());
        assert_eq!(bench.canvas.checker_texture_id(), None);
        if k == 0 {
            let image_2 = bench.canvas.texture_id().expect("textura de la imagen");
            // La carga de la baldosa y los bloques de la superposición: solo la imagen.
            assert!(!output_2.uploads.is_empty());
            assert_eq!(
                output_2.uploads.len(),
                output_2.uploads_of(image_2).len(),
                "solo la baldosa de la imagen"
            );
        } else {
            assert!(output_2.uploads.is_empty(), "nada nuevo que subir");
        }
    }
    bench.canvas.set_background(CanvasBackground::Checker);
    let output_2 = bench.runs(vec![], &blocks());
    let checker_2 = bench
        .canvas
        .checker_texture_id()
        .expect("se crea al volver");
    assert_eq!(output_2.uploads_of(checker_2).len(), 1, "una sola subida");
    // Otros cambios de fondo y vuelta: ni nueva subida ni otro id.
    for background_2 in [
        CanvasBackground::White,
        CanvasBackground::Checker,
        CanvasBackground::Gray,
        CanvasBackground::Checker,
        CanvasBackground::Black,
        CanvasBackground::Checker,
    ] {
        bench.canvas.set_background(background_2);
        let output_2 = bench.runs(vec![], &blocks());
        assert!(output_2.uploads.is_empty(), "{background_2:?}: sin subidas");
        assert_eq!(
            bench.canvas.checker_texture_id(),
            Some(checker_2),
            "{background_2:?}"
        );
    }
}

#[test]
fn row_14_the_default_background_is_the_checker_and_the_path_by_texture() {
    let canvas = Canvas::new();
    assert_eq!(canvas.background(), CanvasBackground::Checker);
    assert!(!canvas.checker_mesh());
    assert_eq!(canvas.checker_texture_id(), None);
}

// ---------------------------------------------------------------------------------------------
// Fila 15: respaldo por malla
// ---------------------------------------------------------------------------------------------

#[test]
fn row_15_in_mesh_mode_the_transparent_image_looks_the_same_as_with_the_texture() {
    for ppp in PPPS {
        for zoom in 1..=3 {
            let (bench, scene) = row_5_scene(ppp, zoom, true);
            assert!(bench.canvas.checker_mesh());
            assert_eq!(
                bench.canvas.checker_texture_id(),
                None,
                "sin textura del damero"
            );
            let image_2 = bench.canvas.texture_id().expect("textura de la imagen");
            assert_eq!(
                scene.output_2.uploads.len(),
                0,
                "ya se subió en el calentamiento"
            );
            let layers = scene.output_2.layers();
            let i = checks_base(
                &bench,
                &scene.output_2,
                CanvasBackground::Checker,
                true,
                &format!("ppp {ppp} ×{zoom}"),
            );
            // El fondo y las casillas oscuras: un rectángulo CHECK_A y UNA sola malla de colores de
            // vértice (sin la textura de la imagen).
            assert_eq!(i, 3, "BACKGROUND, CHECK_A, una malla de casillas B, imagen");
            assert!(matches!(&layers[1], Layer::TestFill { .. }));
            assert!(
                matches!(&layers[2], Layer::TestMesh { texture_2, .. } if *texture_2 != image_2)
            );
        }
    }
}

#[test]
fn row_15_in_mesh_mode_after_dragging_the_checker_goes_with_the_image() {
    for ppp in PPPS {
        for zoom in 1..=3 {
            row_5_after_dragging(ppp, zoom, &[(7, 13)], true);
        }
    }
}

#[test]
fn row_15_in_mesh_mode_the_overlay_looks_exact_and_no_checker_is_uploaded() {
    for ppp in PPPS {
        for zoom in 1..=3 {
            blocks_with(ppp, zoom, CanvasBackground::Checker, true);
        }
    }
}

#[test]
fn row_15_in_mesh_mode_the_eraser_stroke_looks_the_same_before_and_after_releasing() {
    for ppp in PPPS {
        stroke_of_the_eraser(ppp, CanvasBackground::Checker, true);
    }
}

#[test]
fn row_15_in_mesh_mode_there_is_no_checker_texture_or_upload_in_any_frame() {
    let mut bench = Bench::without_warm_up(
        TEST_SCREEN,
        1.0,
        image_transparent(),
        CanvasBackground::Checker,
        true,
    );
    let first = bench.runs(vec![], &blocks());
    assert_eq!(bench.canvas.checker_texture_id(), None);
    // Solo se sube la baldosa de la imagen (la carga y los bloques de la superposición).
    let texture_2 = bench.canvas.texture_id().expect("textura de la imagen");
    assert!(!first.uploads.is_empty());
    assert_eq!(first.uploads.len(), first.uploads_of(texture_2).len());
    for _ in 0..3 {
        let f = bench.runs(vec![], &blocks());
        assert!(f.uploads.is_empty());
        assert_eq!(bench.canvas.checker_texture_id(), None);
    }
}

#[test]
fn row_15_when_turning_off_the_mesh_mode_the_texture_path_returns() {
    let mut bench = Bench::new_with(1.0, image_transparent(), CanvasBackground::Checker, true);
    assert_eq!(bench.canvas.checker_texture_id(), None);
    // Desactivar: la textura se crea (una subida) y el fondo vuelve a ser el cuadrilátero.
    bench.canvas.set_checker_mesh(false);
    assert!(!bench.canvas.checker_mesh());
    let output_2 = bench.runs(vec![], &[]);
    let checker_2 = bench
        .canvas
        .checker_texture_id()
        .expect("vuelve la textura");
    assert_eq!(output_2.uploads_of(checker_2).len(), 1, "una subida");
    checks_base(
        &bench,
        &output_2,
        CanvasBackground::Checker,
        false,
        "tras desactivar",
    );
    // Activarlo de nuevo no crea otra textura ni sube nada; el camino de la malla vuelve.
    bench.canvas.set_checker_mesh(true);
    let output_2 = bench.runs(vec![], &[]);
    assert!(output_2.uploads.is_empty());
    let i = checks_base(
        &bench,
        &output_2,
        CanvasBackground::Checker,
        true,
        "reactivado",
    );
    assert_eq!(i, 3);
    // Y a la textura otra vez, con el mismo id y sin subida.
    bench.canvas.set_checker_mesh(false);
    let output_2 = bench.runs(vec![], &[]);
    assert!(output_2.uploads.is_empty(), "no se vuelve a subir");
    assert_eq!(bench.canvas.checker_texture_id(), Some(checker_2));
    checks_base(
        &bench,
        &output_2,
        CanvasBackground::Checker,
        false,
        "desactivado otra vez",
    );
}

// ---------------------------------------------------------------------------------------------
// Propiedad 2: pintar el damero no cambia la conversión pantalla-imagen
// ---------------------------------------------------------------------------------------------

#[derive(Clone, Copy, Debug)]
enum Op {
    /// Mueve el puntero a una posición del lienzo, en milésimas de su ancho y alto.
    Mover(u32, u32),
    Wheel(bool),
    More,
    Less,
    Start,
    /// Pulsa un botón (el central desplaza, el izquierdo dibuja) si no hay ninguno abajo.
    Pulsar(bool),
    Release,
}

fn a_op() -> impl Strategy<Value = Op> {
    prop_oneof![
        3 => (0u32..1000, 0u32..1000).prop_map(|(x, y)| Op::Mover(x, y)),
        2 => any::<bool>().prop_map(Op::Wheel),
        1 => Just(Op::More),
        1 => Just(Op::Less),
        1 => Just(Op::Start),
        2 => any::<bool>().prop_map(Op::Pulsar),
        2 => Just(Op::Release),
    ]
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(24))]

    /// Secuencias aleatorias de rueda, arrastre, `+`, `-`, `Home` y movimientos del puntero: con
    /// `interact` y `paint` frente a solo `interact`, el zoom y los eventos de trazo son idénticos;
    /// `hover`, `zoom` e `image_rect` de `CanvasInfo` no dependen del fondo ni del camino de pintado,
    /// y `hover` coincide con `floor((pointer − image_rect.min)·ppp / zoom)`.
    #[test]
    fn property_2_painting_does_not_change_the_screen_image_conversion(
        ppp_i in 0usize..5,
        ops in prop::collection::vec(a_op(), 1..30),
        background_i in 0usize..4,
        mesh in any::<bool>(),
    ) {
        let ppp = PPPS[ppp_i];
        let image_2 = image_blue();
        let mut painting = Bench::without_warm_up(TEST_SCREEN, ppp, image_2.clone(), CanvasBackground::Checker, false);
        let mut only = Bench::without_warm_up(TEST_SCREEN, ppp, image_2.clone(), CanvasBackground::Checker, false);
        let mut other_2 = Bench::without_warm_up(TEST_SCREEN, ppp, image_2, BACKGROUNDS[background_i], mesh);
        let vp = painting.viewport();
        let mut pos = in_points(ppp, i64::from(vp.width) / 2, i64::from(vp.height) / 2);
        let mut button_2: Option<PointerButton> = None;
        let mut sequence = vec![vec![mover(pos)]];
        for op in ops {
            sequence.push(match op {
                Op::Mover(fx, fy) => {
                    let x = i64::from(fx) * i64::from(vp.width) / 1000;
                    let y = i64::from(fy) * i64::from(vp.height) / 1000;
                    pos = in_points(ppp, x, y);
                    vec![mover(pos)]
                }
                Op::Wheel(top) => vec![wheel_line(top)],
                Op::More => vec![key_2(Key::Plus, Modifiers::NONE)],
                Op::Less => vec![key_2(Key::Minus, Modifiers::NONE)],
                Op::Start => vec![key_2(Key::Home, Modifiers::NONE)],
                Op::Pulsar(central) if button_2.is_none() => {
                    let b = if central { PointerButton::Middle } else { PointerButton::Primary };
                    button_2 = Some(b);
                    vec![button_of_2(pos, b, true)]
                }
                Op::Pulsar(_) => vec![],
                Op::Release => match button_2.take() {
                    Some(b) => vec![button_of_2(pos, b, false)],
                    None => vec![],
                },
            });
        }
        let mut pos_actual: Option<Pos2> = None;
        for (k, events) in sequence.into_iter().enumerate() {
            for e in &events {
                if let Event::PointerMoved(p) = e {
                    pos_actual = Some(*p);
                }
            }
            let with = painting.runs(events.clone(), &[]);
            let without = only.only_interact(events.clone());
            let other = other_2.runs(events, &[]);
            prop_assert_eq!(&with.stroke_2, &without.stroke_2, "fotograma {}: eventos de trazo", k);
            prop_assert_eq!(&with.stroke_2, &other.stroke_2, "fotograma {}: eventos de trazo (otro fondo)", k);
            prop_assert_eq!(painting.canvas.zoom(), only.canvas.zoom(), "fotograma {}: zoom", k);
            prop_assert_eq!(
                with.info.as_ref(),
                other.info.as_ref(),
                "fotograma {}: hover, zoom e image_rect no dependen del fondo", k
            );
            // `hover` es `floor((pointer − image_rect.min)·ppp / zoom)`.
            let info = with.info();
            let rect = info.image_rect.expect("hay imagen");
            // Con zoom ≥ ×1 un píxel de imagen ocupa `z` píxeles físicos (`scale(1)`); con 1/K,
            // `scale(1)` vale 0 y es el píxel físico `s` el que muestra el de imagen `s·K`.
            let z = info.zoom.scale(1);
            let zoomed_out = info.zoom.shrink_factor();
            let expected_2 = pos_actual.and_then(|p| {
                let fx = ((p.x - rect.min.x) * ppp).round() as i64;
                let fy = ((p.y - rect.min.y) * ppp).round() as i64;
                let (ix, iy) = match zoomed_out {
                    Some(k) => (fx * i64::from(k), fy * i64::from(k)),
                    None => (fx.div_euclid(z), fy.div_euclid(z)),
                };
                (fx >= 0 && fy >= 0 && ix < 64 && iy < 48).then_some((ix as u32, iy as u32))
            });
            prop_assert_eq!(info.hover, expected_2, "fotograma {}: hover", k);
            prop_assert_eq!(Some(info.zoom), painting.canvas.zoom());
        }
    }
}

// ---------------------------------------------------------------------------------------------
// Propiedad 3: lo que se ve es el oráculo puro
// ---------------------------------------------------------------------------------------------

#[derive(Clone, Debug)]
struct CaseSeen {
    image_2: TestImage,
    overlay: Vec<TestPending>,
    background_2: CanvasBackground,
    mesh: bool,
    ppp: f32,
    zoom: u32,
    desplazo: (i64, i64),
}

impl std::fmt::Debug for TestImage {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "Imagen {}×{} {:?}",
            self.width_2, self.height_2, self.pixels_2
        )
    }
}

/// Un color con alfa 0, de 1 a 254 o 255 al azar.
fn color() -> impl Strategy<Value = Rgba> {
    (0u8..3, any::<u8>(), any::<u8>(), any::<u8>(), 1u8..=254).prop_map(|(class, r, g, b, a)| {
        let alpha = match class {
            0 => 0,
            1 => a,
            _ => 255,
        };
        Rgba::new(r, g, b, alpha)
    })
}

fn case_seen() -> impl Strategy<Value = CaseSeen> {
    (1u32..=24, 1u32..=24)
        .prop_flat_map(|(width_2, height_2)| {
            (
                Just((width_2, height_2)),
                prop::collection::vec(color(), (width_2 * height_2) as usize),
                prop::collection::vec((any::<u32>(), color()), 0..=12),
                0usize..4,
                any::<bool>(),
                0usize..5,
                1u32..=3,
                (-80i64..=80, -80i64..=80),
            )
        })
        .prop_map(
            |(
                (width_2, height_2),
                pixels_2,
                pending_2,
                background_2,
                mesh,
                ppp,
                zoom,
                desplazo,
            )| {
                let mut seen_2 = std::collections::HashSet::new();
                let overlay = pending_2
                    .into_iter()
                    .filter_map(|(seed, color)| {
                        let i = seed % (width_2 * height_2);
                        seen_2
                            .insert(i)
                            .then_some((i % width_2, i / width_2, color))
                    })
                    .collect();
                CaseSeen {
                    image_2: TestImage {
                        width_2,
                        height_2,
                        pixels_2,
                    },
                    overlay,
                    background_2: BACKGROUNDS[background_2],
                    mesh,
                    ppp: PPPS[ppp],
                    zoom,
                    desplazo,
                }
            },
        )
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(24))]

    /// Cada píxel físico del rectángulo de la imagen se ve igual que
    /// `composición(background_2 en ese píxel, píxel de la image_2 o de la superposición)` y el resto del
    /// lienzo, `BACKGROUND`: con uno de los cuatro fondos, con la textura o con el respaldo por
    /// malla, a cualquiera de las cinco escalas, a ×1, ×2 o ×3 y con la vista desplazada.
    #[test]
    fn property_3_what_is_seen_is_the_pure_oracle(c in case_seen()) {
        let mut bench = Bench::new_with(c.ppp, c.image_2.clone(), c.background_2, c.mesh);
        bench.with_zoom(c.zoom);
        bench.drags(c.desplazo.0, c.desplazo.1);
        let scene = bench.scene(&c.overlay);
        let f = failures(&scene, &c.image_2, &c.overlay, c.background_2);
        prop_assert!(
            f.empty(),
            "ppp {} ×{} {:?} malla={} desplazo {:?}: {}",
            c.ppp, c.zoom, c.background_2, c.mesh, c.desplazo, f.summary_2()
        );
        // Y las capas según el fondo y el camino: sin formas de superposición.
        checks_base(&bench, &scene.output_2, c.background_2, c.mesh, "propiedad 3");
        checks_texture(&bench, &c.overlay, "propiedad 3");
    }
}

// =============================================================================================
// Unidad 19b: un color propio como fondo (specs/hito-2-fondo-color.md, filas 6 a 8 y propiedad 3)
// =============================================================================================
//
// `background_in` ya tiene el brazo `Custom`; `failures`, `checks_base`, `checks_texture`,
// `blocks_with` y `stroke_of_the_eraser` valen con cualquier fondo. Lo que se añade es el recorrido
// con colores de canales distintos (para que un canal cambiado o un redondeo mal sentado no pasen)
// y el oráculo de los zooms de alejamiento, que el banco de arriba no tenía (`Geom::z` vale 0 a 1/K).
//
// Suposiciones: a 1/K el píxel físico `(i, j)` del rectángulo de la imagen muestra el píxel de imagen
// `(i·K, j·K)` (`Zoom::shrink_factor` y `View::pixel_block`, como en `zoom_canvas.rs`); `Zoom::shrink(k)`
// y `ViewOp::ZoomOut` desde `Actual` dan 1/2, 1/4 y 1/8 con una, dos y tres operaciones.

const BLUE_STEEL: Rgba = Rgba::opaque(0x33, 0x66, 0x99);
const ORANGE: Rgba = Rgba::opaque(0xff, 0x80, 0x00);
const DARK: Rgba = Rgba::opaque(0x01, 0x02, 0x03);
/// El color de `BACKGROUND` como fondo propio: se ve igual que el gris de fuera del lienzo.
const AS_THE_BACKGROUND: Rgba = Rgba::opaque(40, 40, 40);

const OWN: [CanvasBackground; 4] = [
    CanvasBackground::Custom(BLUE_STEEL),
    CanvasBackground::Custom(ORANGE),
    CanvasBackground::Custom(DARK),
    CanvasBackground::Custom(AS_THE_BACKGROUND),
];

#[test]
fn row_6_the_custom_color_of_the_background_is_that_of_background_and_looks_like_color32() {
    assert_eq!(
        Color32::from_rgb(
            AS_THE_BACKGROUND.r,
            AS_THE_BACKGROUND.g,
            AS_THE_BACKGROUND.b
        ),
        BACKGROUND
    );
    for background_2 in OWN {
        let CanvasBackground::Custom(c) = background_2 else {
            unreachable!()
        };
        assert_eq!(background_in(background_2, 5, 7, (0, 0), 8), gray_2(c));
    }
}

/// Imagen RGBA de 64×48 en bloques de 8×8 con alfa 0, 128 y 255 (según `(bx + by) % 3`) y un color
/// distinto en cada bloque: todos los canales difieren, también en los de alfa 0.
fn image_of_alphas() -> TestImage {
    let mut pixels_2 = Vec::with_capacity(64 * 48);
    for y in 0..48u32 {
        for x in 0..64u32 {
            let (bx, by) = (x / 8, y / 8);
            let alpha = [0u8, 128, 255][((bx + by) % 3) as usize];
            pixels_2.push(Rgba::new(
                (40 + 24 * bx) as u8,
                (30 + 30 * by) as u8,
                (250 - 10 * (bx + by)) as u8,
                alpha,
            ));
        }
    }
    TestImage {
        width_2: 64,
        height_2: 48,
        pixels_2,
    }
}

/// Con un color propio ni el damero, ni el gris de fuera, ni un borde asoman bajo la imagen: el
/// píxel de la imagen con alfa 0 se ve exactamente el color, el de alfa 128 mezclado y el opaco, el
/// suyo.
fn checks_alpha_swatches(
    scene: &Scene,
    image_2: &TestImage,
    background_2: CanvasBackground,
    k: i64,
) {
    let g = scene.geom;
    for (ix, iy, alpha) in [(0u32, 0u32, 0u8), (8, 0, 128), (16, 0, 255)] {
        let color = image_2.pixel(ix, iy);
        assert_eq!(color.a, alpha);
        // Con zoom ≥ ×1 el bloque de `z × z` píxeles físicos; a 1/K, el píxel `(ix / K, iy / K)`.
        let (px, py) = if k == 1 {
            let b = g.block(ix, iy);
            (b.x, b.y)
        } else {
            (
                g.source_2.0 + i64::from(ix) / k,
                g.source_2.1 + i64::from(iy) / k,
            )
        };
        let expected_2 = composes(
            background_in(background_2, px, py, g.source_2, g.cell),
            color,
        );
        let seen = scene.buffer.en(px, py);
        assert!(
            seen.near(expected_2, TOLERANCE),
            "alfa {alpha}: el píxel ({px}, {py}) se ve {seen:?} y debía verse {expected_2:?}"
        );
    }
}

#[test]
fn row_6_custom_color_each_pixel_shows_the_composition_at_the_five_scales_and_three_zooms() {
    for background_2 in OWN {
        for ppp in PPPS {
            for zoom in 1..=3 {
                let label_2 = format!("ppp {ppp} ×{zoom} {background_2:?}");
                let mut bench = Bench::new_with(ppp, image_of_alphas(), background_2, false);
                bench.with_zoom(zoom);
                let scene = bench.scene(&[]);
                assert_eq!(scene.geom.z, i64::from(zoom), "{label_2}");
                assert_eq!(scene.geom.width_2, 64 * scene.geom.z, "{label_2}");
                assert_eq!(scene.geom.height_2, 48 * scene.geom.z, "{label_2}");
                // Un solo rectángulo de fondo bajo las baldosas y ninguna textura del damero.
                checks_base(&bench, &scene.output_2, background_2, false, &label_2);
                assert_eq!(bench.canvas.checker_texture_id(), None, "{label_2}");
                without_failures(
                    &failures(&scene, &bench.image_2, &[], background_2),
                    &label_2,
                );
                checks_alpha_swatches(&scene, &bench.image_2, background_2, 1);
            }
        }
    }
}

#[test]
fn row_6_custom_color_with_the_mesh_fallback_looks_the_same_as_with_the_texture() {
    for background_2 in OWN {
        for ppp in PPPS {
            for zoom in 1..=3 {
                let label_2 = format!("ppp {ppp} ×{zoom} {background_2:?}");
                let mut with_texture = Bench::new_with(ppp, image_of_alphas(), background_2, false);
                with_texture.with_zoom(zoom);
                let a = with_texture.scene(&[]);
                let mut with_mesh = Bench::new_with(ppp, image_of_alphas(), background_2, true);
                with_mesh.with_zoom(zoom);
                assert!(with_mesh.canvas.checker_mesh());
                let b = with_mesh.scene(&[]);
                checks_base(&with_mesh, &b.output_2, background_2, true, &label_2);
                assert_eq!(with_mesh.canvas.checker_texture_id(), None, "{label_2}");
                without_failures(
                    &failures(&b, &with_mesh.image_2, &[], background_2),
                    &label_2,
                );
                // Igual, píxel a píxel y en todo el lienzo.
                assert_eq!(a.geom.source_2, b.geom.source_2, "{label_2}");
                assert_eq!(
                    (a.buffer.width_2, a.buffer.height_2),
                    (b.buffer.width_2, b.buffer.height_2)
                );
                for py in 0..a.buffer.height_2 as i64 {
                    for px in 0..a.buffer.width_2 as i64 {
                        assert!(
                            a.buffer.en(px, py).near(b.buffer.en(px, py), TOLERANCE),
                            "{label_2}: el píxel ({px}, {py}) cambia con el respaldo por malla"
                        );
                    }
                }
            }
        }
    }
}

/// Fila 6 tras arrastrar `(dx, dy)`: cada píxel físico sigue siendo el oráculo y, relativo a la
/// esquina de la imagen, es el mismo que antes (el fondo liso va con la imagen).
fn row_6_after_dragging(
    background_2: CanvasBackground,
    ppp: f32,
    zoom: u32,
    offsets: &[(i64, i64)],
) {
    let mut bench = Bench::new_with(ppp, image_of_alphas(), background_2, false);
    bench.with_zoom(zoom);
    let antes = bench.scene(&[]);
    let g0 = antes.geom;
    let (mut x, mut y) = g0.source_2;
    for &(dx, dy) in offsets {
        let label_2 = format!("ppp {ppp} ×{zoom} {background_2:?} arrastre ({dx}, {dy})");
        bench.drags(dx, dy);
        let after_2 = bench.scene(&[]);
        let g1 = after_2.geom;
        assert_eq!(
            g1.source_2,
            (x + dx, y + dy),
            "{label_2}: la imagen se desplazó exactamente lo arrastrado"
        );
        (x, y) = g1.source_2;
        checks_base(&bench, &after_2.output_2, background_2, false, &label_2);
        without_failures(
            &failures(&after_2, &bench.image_2, &[], background_2),
            &label_2,
        );
        for ry in 0..g0.height_2 {
            for rx in 0..g0.width_2 {
                let antes_ = antes.buffer.en(g0.source_2.0 + rx, g0.source_2.1 + ry);
                let after_ = after_2.buffer.en(g1.source_2.0 + rx, g1.source_2.1 + ry);
                assert!(
                    antes_.near(after_, TOLERANCE),
                    "{label_2}: el píxel ({rx}, {ry}) relativo a la esquina era {antes_:?} y ahora es {after_:?}"
                );
            }
        }
    }
}

#[test]
fn row_6_custom_color_after_dragging_the_view_7_and_13_pixels_the_background_goes_with_the_image() {
    for background_2 in OWN {
        for ppp in PPPS {
            for zoom in 1..=3 {
                // Un arrastre de 7 píxeles en x y 13 en y, y luego otro de 13 en x y 7 en y.
                row_6_after_dragging(background_2, ppp, zoom, &[(7, 13), (13, 7)]);
            }
        }
    }
}

/// Lleva el zoom a 1/`k` (de ×1 por `k` = 2, 4 u 8 con una, dos o tres operaciones de alejar).
fn with_zoom_out(bench: &mut Bench, k: u32) {
    bench.canvas.request_view(ViewOp::Actual);
    for _ in 0..k.trailing_zeros() {
        bench.canvas.request_view(ViewOp::ZoomOut);
    }
    bench.runs(vec![], &[]);
    assert_eq!(bench.canvas.zoom(), Zoom::shrink(k), "1/{k}");
}

/// El oráculo a 1/`k`: el píxel físico `(i, j)` del rectángulo de la imagen muestra el píxel
/// `(i·k, j·k)` (el de la superposición, si lo hay) sobre el fondo elegido; fuera, `BACKGROUND`.
fn failures_zoomed_out(
    scene: &Scene,
    image_2: &TestImage,
    overlay: &[TestPending],
    background_2: CanvasBackground,
    k: i64,
) -> Failures {
    let g = scene.geom;
    let map: HashMap<(u32, u32), Rgba> = overlay.iter().map(|&(x, y, c)| ((x, y), c)).collect();
    let mut failures = Failures::new_2();
    for py in 0..scene.buffer.height_2 as i64 {
        for px in 0..scene.buffer.width_2 as i64 {
            let inside = px >= g.source_2.0
                && px < g.source_2.0 + g.width_2
                && py >= g.source_2.1
                && py < g.source_2.1 + g.height_2;
            let expected_2 = if inside {
                let ix = ((px - g.source_2.0) * k) as u32;
                let iy = ((py - g.source_2.1) * k) as u32;
                let color = map
                    .get(&(ix, iy))
                    .copied()
                    .unwrap_or_else(|| image_2.pixel(ix, iy));
                composes(
                    background_in(background_2, px, py, g.source_2, g.cell),
                    color,
                )
            } else {
                Seen::de_color(BACKGROUND)
            };
            let seen = scene.buffer.en(px, py);
            if !seen.near(expected_2, TOLERANCE) {
                failures.notes(|| {
                    format!(
                        "ppp {} 1/{k}: el píxel físico ({px}, {py}) se ve {seen:?} y debía verse {expected_2:?} (dentro de la imagen: {inside})",
                        g.ppp
                    )
                });
            }
        }
    }
    failures
}

#[test]
fn row_6_custom_color_at_1_2_1_4_and_1_8_each_pixel_shows_the_composition() {
    for background_2 in OWN {
        for ppp in PPPS {
            for k in [2u32, 4, 8] {
                let label_2 = format!("ppp {ppp} 1/{k} {background_2:?}");
                let mut bench = Bench::new_with(ppp, image_of_alphas(), background_2, false);
                with_zoom_out(&mut bench, k);
                let scene = bench.scene(&[]);
                let g = scene.geom;
                assert_eq!(g.width_2, 64 / i64::from(k), "{label_2}");
                assert_eq!(g.height_2, 48 / i64::from(k), "{label_2}");
                checks_base(&bench, &scene.output_2, background_2, false, &label_2);
                assert_eq!(bench.canvas.checker_texture_id(), None, "{label_2}");
                let f =
                    failures_zoomed_out(&scene, &bench.image_2, &[], background_2, i64::from(k));
                without_failures(&f, &label_2);
                checks_alpha_swatches(&scene, &bench.image_2, background_2, i64::from(k));
            }
        }
    }
}

// ---------------------------------------------------------------------------------------------
// Fila 7: la superposición y la vista previa sobre el color
// ---------------------------------------------------------------------------------------------

#[test]
fn row_7_custom_color_the_pending_blocks_are_seen_over_the_color_at_five_scales_and_three_zooms() {
    for background_2 in OWN {
        for ppp in PPPS {
            for zoom in 1..=3 {
                // Sin formas de superposición, la textura es el documento con los bloques escritos
                // y cada píxel físico es el oráculo; a ×1 los de 1×1 píxel físico también.
                blocks_with(ppp, zoom, background_2, false);
            }
        }
    }
}

#[test]
fn row_7_custom_color_with_the_mesh_fallback_the_blocks_look_the_same() {
    for background_2 in [OWN[0], OWN[2]] {
        for ppp in PPPS {
            for zoom in 1..=3 {
                blocks_with(ppp, zoom, background_2, true);
            }
        }
    }
}

#[test]
fn row_7_custom_color_at_x1_the_translucent_pixels_are_seen_mixed_with_the_color_and_not_with_the_image()
 {
    // El fallo de `b702d8e`: a ×1 la superposición de 1×1 píxel físico no se veía. Con un color propio
    // el píxel `(255, 0, 0, 128)` se ve mezclado con **el color**, no con el azul de la imagen ni con
    // el damero, y el borrador (`alpha 0`) deja ver el color exacto.
    for background_2 in OWN {
        let CanvasBackground::Custom(c) = background_2 else {
            unreachable!()
        };
        for ppp in PPPS {
            let mut bench = Bench::new_with(ppp, image_blue(), background_2, false);
            bench.with_zoom(1);
            let overlay = blocks();
            let scene = bench.scene(&overlay);
            assert_eq!(scene.geom.z, 1);
            let f = failures(&scene, &bench.image_2, &overlay, background_2);
            without_failures(&f, &format!("ppp {ppp} ×1 {background_2:?}"));
            for &(x, y, color) in &overlay {
                let b = scene.geom.block(x, y);
                let seen = scene.buffer.en(b.x, b.y);
                let expected_2 = match color.a {
                    0 => Seen::de_color(gray_2(c)),
                    _ => composes(gray_2(c), color),
                };
                assert!(
                    seen.near(expected_2, TOLERANCE),
                    "ppp {ppp} {background_2:?}: ({x}, {y}) con {color:?} se ve {seen:?} y debía verse {expected_2:?}"
                );
            }
        }
    }
}

#[test]
fn row_7_custom_color_the_eraser_stroke_looks_the_same_before_and_after_releasing() {
    for background_2 in OWN {
        for ppp in PPPS {
            stroke_of_the_eraser(ppp, background_2, false);
        }
    }
}

#[test]
fn row_7_custom_color_with_the_mesh_fallback_the_eraser_stroke_looks_the_same() {
    for ppp in PPPS {
        stroke_of_the_eraser(ppp, OWN[0], true);
    }
}

/// Imagen indexada de 64×48 con la entrada 0 transparente (`alpha 0`) y las 1 a 3 opacas; las filas 3
/// (`x < 32`, entrada 1) y 10 a 12 (`x` de 8 a 39, entrada 2) tienen píxeles, el resto es la entrada
/// transparente. La que se edita es la 1 y la 2.
fn indexed_for_the_preview() -> Image {
    let mut image = indexed();
    for y in 10..13 {
        for x in 8..40 {
            image.set_pixel(x, y, Pixel::Indexed(2)).expect("dentro");
        }
    }
    image
}

/// Lo que se debe ver de `image` con la entrada `index` valiendo `color` (la `0`, transparente, no
/// cambia): la imagen RGBA equivalente.
fn image_with_the_entry(image: &Image, index: u8, color: Rgba) -> TestImage {
    let palette = image.palette().expect("indexada");
    let mut pixels_2 = Vec::with_capacity(64 * 48);
    for y in 0..48 {
        for x in 0..64 {
            let Pixel::Indexed(i) = image.get_pixel(x, y).expect("dentro") else {
                panic!("es indexada");
            };
            pixels_2.push(if i == index {
                color
            } else {
                palette.get(i).expect("entrada")
            });
        }
    }
    TestImage {
        width_2: 64,
        height_2: 48,
        pixels_2,
    }
}

#[test]
fn row_7_custom_color_preview_entry_is_seen_over_the_color_the_same_as_after_end_preview() {
    // Una entrada de alfa 0, de 128 y opaca.
    let colors_2 = [
        Rgba::new(255, 255, 0, 0),
        Rgba::new(255, 255, 0, 128),
        Rgba::opaque(255, 255, 0),
    ];
    for background_2 in OWN {
        for ppp in PPPS {
            for zoom in 1..=3 {
                let label_2 = format!("ppp {ppp} ×{zoom} {background_2:?}");
                let indexed = indexed_for_the_preview();
                let original = image_with_the_entry(&indexed, 1, Rgba::opaque(255, 0, 0));
                let mut bench = Bench::new_with(ppp, image_transparent(), background_2, false);
                bench.canvas.set_image(&indexed);
                bench.document = indexed.clone();
                bench.image_2 = original.clone();
                bench.with_zoom(zoom);
                let without_view_preview = bench.scene(&[]);
                without_failures(
                    &failures(&without_view_preview, &original, &[], background_2),
                    &format!("{label_2}: antes de la vista previa"),
                );
                let texture_2 = bench.canvas.texture_id().expect("textura de la imagen");
                let uploads_before = bench.canvas.preview_uploads();
                let mut expected_2_2 = Vec::new();
                for (n, color) in colors_2.into_iter().enumerate() {
                    bench.canvas.preview_entry(&bench.ctx, &indexed, 1, color);
                    let vista = image_with_the_entry(&indexed, 1, color);
                    let scene = bench.scene(&[]);
                    let label_2 = format!("{label_2}, vista previa con {color:?}");
                    // Sin superposición propia ni damero, y cada píxel físico es el oráculo.
                    checks_base(&bench, &scene.output_2, background_2, false, &label_2);
                    assert_eq!(bench.canvas.checker_texture_id(), None, "{label_2}");
                    without_failures(&failures(&scene, &vista, &[], background_2), &label_2);
                    // Una sola subida (la baldosa de la imagen) y ninguna del damero.
                    assert_eq!(
                        scene.output_2.uploads.len(),
                        scene.output_2.uploads_of(texture_2).len(),
                        "{label_2}: solo se sube la baldosa de la imagen"
                    );
                    assert_eq!(scene.output_2.uploads_of(texture_2).len(), 1, "{label_2}");
                    assert_eq!(
                        bench.canvas.preview_uploads(),
                        uploads_before + n as u32 + 1,
                        "{label_2}: una subida de vista previa por valor nuevo"
                    );
                    // Otro fotograma con el mismo valor: nada que subir.
                    let other = bench.runs(vec![], &[]);
                    assert!(
                        other.uploads.is_empty(),
                        "{label_2}: sin cambios no se sube"
                    );
                    expected_2_2.push((color, scene));
                }
                // Al soltar (`end_preview`), vuelve la imagen del documento y se ve sobre el color.
                bench.canvas.end_preview(&bench.ctx, &indexed);
                let output_2 = bench.runs(vec![], &[]);
                let after = bench.rasterizes_output(output_2);
                checks_base(&bench, &after.output_2, background_2, false, &label_2);
                without_failures(
                    &failures(&after, &original, &[], background_2),
                    &format!("{label_2}: tras end_preview"),
                );
                // Fuera de los píxeles de la entrada editada, la vista previa se ve igual que antes
                // y que después: solo cambian los de la entrada 1.
                let g = after.geom;
                for (color, vista) in &expected_2_2 {
                    for py in g.source_2.1..g.source_2.1 + g.height_2 {
                        for px in g.source_2.0..g.source_2.0 + g.width_2 {
                            let (ix, iy) = (
                                ((px - g.source_2.0) / g.z) as u32,
                                ((py - g.source_2.1) / g.z) as u32,
                            );
                            if matches!(indexed.get_pixel(ix, iy), Ok(Pixel::Indexed(1))) {
                                continue;
                            }
                            assert!(
                                vista
                                    .buffer
                                    .en(px, py)
                                    .near(after.buffer.en(px, py), TOLERANCE),
                                "{label_2}: el píxel ({px}, {py}) de otra entrada cambió con la vista previa de {color:?}"
                            );
                        }
                    }
                }
            }
        }
    }
}

#[test]
fn row_7_custom_color_at_1_2_1_4_and_1_8_the_overlay_is_seen_over_the_color() {
    // Solo se ven los píxeles de imagen con `x` e `y` múltiplos de `K`: los pendientes se ponen en
    // múltiplos de 8, visibles a los tres niveles.
    let mut overlay: Vec<TestPending> = Vec::new();
    for (i, color) in [RED, TRANSPARENT, RED_MIDDLE].into_iter().enumerate() {
        for row_2 in [8u32, 16] {
            overlay.push((8 * (2 * i as u32 + 1), row_2, color));
        }
    }
    for background_2 in OWN {
        for ppp in PPPS {
            for k in [2u32, 4, 8] {
                let label_2 = format!("ppp {ppp} 1/{k} {background_2:?}");
                let mut bench = Bench::new_with(ppp, image_blue(), background_2, false);
                with_zoom_out(&mut bench, k);
                let scene = bench.scene(&overlay);
                checks_base(&bench, &scene.output_2, background_2, false, &label_2);
                checks_texture(&bench, &overlay, &label_2);
                let f = failures_zoomed_out(
                    &scene,
                    &bench.image_2,
                    &overlay,
                    background_2,
                    i64::from(k),
                );
                without_failures(&f, &label_2);
            }
        }
    }
}

// ---------------------------------------------------------------------------------------------
// Fila 8: cambio de fondo en caliente, recursos
// ---------------------------------------------------------------------------------------------

impl TestStroke {
    /// Como `TestStroke::new_2`, con el fondo de partida que se pida.
    fn with_background(ppp: f32, background_2: CanvasBackground) -> TestStroke {
        let bench =
            Bench::without_warm_up(vec2(800.0, 600.0), ppp, image_blue(), background_2, false);
        let doc = Document::new(bench.image_2.image());
        let mut tool = ToolState::new();
        tool.apply_action(ToolAction::Select(Tool::Eraser));
        let mut stroke_2 = TestStroke {
            bench,
            doc,
            tool,
            rect: Rect::NOTHING,
            zoom: 0,
        };
        stroke_2.frame(vec![]);
        let output_2 = stroke_2.frame(vec![]);
        stroke_2.rect = output_2.info().image_rect.expect("hay imagen");
        stroke_2.zoom = output_2.info().zoom.scale(1);
        stroke_2
    }
}

#[test]
fn row_8_custom_color_changing_the_background_in_the_middle_of_a_stroke_is_seen_in_the_next_frame()
{
    let a = CanvasBackground::Custom(BLUE_STEEL);
    let b = CanvasBackground::Custom(ORANGE);
    let mut t = TestStroke::with_background(1.0, a);
    let state_id = t.doc.state_id();
    let bytes = to_rgba8(t.doc.image(ImageId::from_raw(0)).expect("imagen"));
    let start_2 = t.center(10, 5);
    t.frame(vec![mover(start_2)]);
    t.frame(vec![button_of_2(start_2, PointerButton::Primary, true)]);
    for x in 11..=14 {
        t.frame(vec![mover(t.center(x, 5))]);
    }
    assert!(t.tool.busy(), "el trazo está en curso");
    let n_pending = t.pending_2().len();
    assert_eq!(n_pending, 5);
    // Con un color propio nunca se creó la textura del damero.
    assert_eq!(t.bench.canvas.checker_texture_id(), None);
    let output_2 = t.frame(vec![]);
    checks_base(&t.bench, &output_2, a, false, "color propio inicial");
    t.it_looks_like_the_oracle(output_2, a, "color propio inicial");

    // La cadena de la spec: Custom(a) → Custom(b) → Damero → Custom(a) → Negro → Damero.
    let string = [
        b,
        CanvasBackground::Checker,
        a,
        CanvasBackground::Black,
        CanvasBackground::Checker,
    ];
    let mut checker_2: Option<TextureId> = None;
    for (step, background_2) in string.into_iter().enumerate() {
        let label_2 = format!("paso {step}: {background_2:?}");
        t.bench.canvas.set_background(background_2);
        assert_eq!(t.bench.canvas.background(), background_2);
        let output_2 = t.frame(vec![]);
        // `textures_delta` vacío en el fotograma del cambio, salvo la textura del damero la primera
        // vez que se vuelve a él (se crea una sola vez: mismo id después, sin otra subida).
        let new_checker = background_2 == CanvasBackground::Checker && checker_2.is_none();
        if new_checker {
            let id = t
                .bench
                .canvas
                .checker_texture_id()
                .unwrap_or_else(|| panic!("{label_2}: se crea la textura del damero"));
            assert_eq!(
                output_2.uploads.len(),
                1,
                "{label_2}: solo la textura del damero"
            );
            assert_eq!(output_2.uploads_of(id).len(), 1, "{label_2}");
            checker_2 = Some(id);
        } else {
            assert!(
                output_2.uploads.is_empty(),
                "{label_2}: el cambio de fondo no sube ninguna textura ({} subidas)",
                output_2.uploads.len()
            );
            assert_eq!(t.bench.canvas.checker_texture_id(), checker_2, "{label_2}");
        }
        assert!(output_2.released.is_empty(), "{label_2}: no se libera nada");
        // Lo que se ve es el fondo nuevo, con el trazo a medias encima.
        if background_2 == CanvasBackground::Checker {
            checks_base(&t.bench, &output_2, background_2, false, &label_2);
        } else if checker_2.is_some() {
            checks_base_with_existing_checker(&t.bench, &output_2, background_2);
        } else {
            checks_base(&t.bench, &output_2, background_2, false, &label_2);
        }
        t.it_looks_like_the_oracle(output_2, background_2, &label_2);
        // El trazo sigue, el documento y la herramienta, como estaban.
        assert!(t.tool.busy(), "{label_2}");
        assert_eq!(t.tool.tool(), Tool::Eraser, "{label_2}");
        assert_eq!(t.doc.state_id(), state_id, "{label_2}");
        assert!(!t.doc.can_undo(), "{label_2}");
        assert_eq!(t.pending_2().len(), n_pending, "{label_2}");
        assert_eq!(
            to_rgba8(t.doc.image(ImageId::from_raw(0)).expect("imagen")),
            bytes,
            "{label_2}"
        );
    }

    // El trazo no se cortó: sigue con otro fondo propio y termina en un solo paso de historia, con
    // los píxeles borrados sobre el fondo de ese momento.
    t.frame(vec![mover(t.center(15, 5))]);
    t.bench.canvas.set_background(b);
    let output_2 = t.frame(vec![mover(t.center(16, 5))]);
    assert_eq!(t.pending_2().len(), 7);
    assert_eq!(
        output_2.uploads.len(),
        output_2
            .uploads_of(t.bench.canvas.texture_id().expect("textura"))
            .len(),
        "con el trazo, solo se sube la baldosa de la imagen (nunca el damero)"
    );
    checks_base_with_existing_checker(&t.bench, &output_2, b);
    t.it_looks_like_the_oracle(output_2, b, "el trazo sigue");
    let fin = t.center(16, 5);
    let output_2 = t.frame(vec![button_of_2(fin, PointerButton::Primary, false)]);
    assert!(!t.tool.busy(), "el trazo terminó");
    assert!(t.doc.can_undo(), "el trazo es un paso de historia");
    assert_ne!(t.doc.state_id(), state_id);
    // Soltar sube lo borrado y se ve sobre el color elegido, como se veía con la superposición.
    let scene = t.bench.rasterizes_output(output_2);
    let mut image_final = TestImage::uniform(64, 48, BLUE);
    for x in 10..=16 {
        image_final = image_final.with(x, 5, TRANSPARENT);
    }
    let f = failures(&scene, &image_final, &[], b);
    without_failures(&f, "tras soltar");
}

#[test]
fn row_8_custom_color_changing_the_background_does_not_regenerate_tiles_and_the_checker_is_created_only_once()
 {
    for ppp in PPPS {
        let a = CanvasBackground::Custom(BLUE_STEEL);
        let mut bench = Bench::without_warm_up(TEST_SCREEN, ppp, image_of_alphas(), a, false);
        for _ in 0..3 {
            bench.runs(vec![], &[]);
        }
        let image_2 = bench.canvas.texture_id().expect("textura de la imagen");
        assert_eq!(bench.canvas.checker_texture_id(), None);
        let mut checker_2: Option<TextureId> = None;
        for (step, background_2) in [
            CanvasBackground::Custom(ORANGE),
            CanvasBackground::Custom(DARK),
            CanvasBackground::Custom(ORANGE),
            CanvasBackground::Checker,
            a,
            CanvasBackground::Checker,
            CanvasBackground::Black,
            CanvasBackground::Custom(AS_THE_BACKGROUND),
            CanvasBackground::Checker,
        ]
        .into_iter()
        .enumerate()
        {
            bench.canvas.set_background(background_2);
            let output_2 = bench.runs(vec![], &[]);
            let label_2 = format!("ppp {ppp} paso {step}: {background_2:?}");
            if background_2 == CanvasBackground::Checker && checker_2.is_none() {
                let id = bench
                    .canvas
                    .checker_texture_id()
                    .unwrap_or_else(|| panic!("{label_2}: se crea al volver al damero"));
                assert_eq!(output_2.uploads.len(), 1, "{label_2}");
                assert_eq!(output_2.uploads_of(id).len(), 1, "{label_2}");
                checker_2 = Some(id);
            } else {
                assert!(output_2.uploads.is_empty(), "{label_2}: sin subidas");
                assert_eq!(bench.canvas.checker_texture_id(), checker_2, "{label_2}");
            }
            assert!(output_2.released.is_empty(), "{label_2}");
            // La imagen no se regenera: es la misma baldosa, con el mismo contenido.
            assert_eq!(bench.canvas.texture_id(), Some(image_2), "{label_2}");
            checks_texture(&bench, &[], &label_2);
            assert_eq!(bench.canvas.background(), background_2);
        }
    }
}

#[test]
fn row_8_custom_color_the_document_to_rgba8_the_state_id_and_the_image_texture_stay_intact() {
    let ppp = 2.0;
    let image_2 = image_of_alphas();
    let doc = Document::new(image_2.image());
    let id = ImageId::from_raw(0);
    let state_id = doc.state_id();
    let bytes = to_rgba8(doc.image(id).expect("imagen"));
    let mut bench = Bench::without_warm_up(
        TEST_SCREEN,
        ppp,
        image_2.clone(),
        CanvasBackground::Custom(BLUE_STEEL),
        false,
    );
    let first = bench.runs(vec![], &[]);
    let texture_2 = bench.canvas.texture_id().expect("textura de la imagen");
    // La imagen se sube una sola vez, tal cual: los mismos bytes que `to_rgba8`, sin el color.
    let upload = first.uploads_of(texture_2);
    assert_eq!(upload.len(), 1);
    let ImageData::Color(color) = &upload[0].image;
    let expected_2 = egui::ColorImage::from_rgba_unmultiplied([64, 48], &bytes);
    assert_eq!(color.pixels, expected_2.pixels);
    assert_eq!(bench.canvas.checker_texture_id(), None);
    // Con la superposición y con cada fondo propio (y los de siempre), la textura sigue siendo el
    // documento con los píxeles pendientes y no se vuelve a subir nada.
    let overlay = blocks();
    bench.runs(vec![], &overlay);
    checks_texture(&bench, &overlay, "con la superposición");
    for background_2 in OWN
        .into_iter()
        .chain(BACKGROUNDS)
        .chain([CanvasBackground::Custom(BLUE_STEEL)])
    {
        bench.canvas.set_background(background_2);
        for _ in 0..2 {
            let output_2 = bench.runs(vec![], &overlay);
            if background_2 == CanvasBackground::Checker {
                // La textura del damero se crea la primera vez que se vuelve a él; la de la imagen,
                // nunca se toca.
                assert!(
                    output_2.uploads_of(texture_2).is_empty(),
                    "{background_2:?}"
                );
            } else {
                assert!(output_2.uploads.is_empty(), "{background_2:?}");
            }
        }
        checks_texture(&bench, &overlay, &format!("{background_2:?}"));
    }
    // Al quitar la superposición, la textura vuelve a ser el documento, byte a byte.
    bench.runs(vec![], &[]);
    checks_texture(&bench, &[], "tras quitar la superposición");
    assert!(bench.textures_2.get(texture_2).expect("textura").texels == expected_2.pixels);
    assert_eq!(to_rgba8(doc.image(id).expect("imagen")), bytes);
    assert_eq!(doc.state_id(), state_id);
    assert!(!doc.can_undo());
}

// ---------------------------------------------------------------------------------------------
// Propiedad 3: lo que se ve es el oráculo puro, con un color propio
// ---------------------------------------------------------------------------------------------

proptest! {
    #![proptest_config(ProptestConfig::with_cases(24))]

    /// Como la propiedad 3 de la 17b, con el fondo `Custom(c)` de un `c` opaco al azar: imagen de 1
    /// a 24 de lado con píxeles de alfa 0, 1 a 254 y 255, superposición al azar, una de las cinco
    /// escalas, ×1, ×2 o ×3, vista desplazada al azar, con la textura o con el respaldo por malla:
    /// cada píxel físico del rectángulo de la imagen vale `composición(c, píxel)` y el resto del
    /// lienzo, `BACKGROUND`.
    #[test]
    fn property_3_custom_color_what_is_seen_is_the_pure_oracle(
        c in case_seen(),
        rgb in any::<[u8; 3]>(),
    ) {
        let background_2 = CanvasBackground::Custom(Rgba::opaque(rgb[0], rgb[1], rgb[2]));
        let mut bench = Bench::new_with(c.ppp, c.image_2.clone(), background_2, c.mesh);
        bench.with_zoom(c.zoom);
        bench.drags(c.desplazo.0, c.desplazo.1);
        let scene = bench.scene(&c.overlay);
        let f = failures(&scene, &c.image_2, &c.overlay, background_2);
        prop_assert!(
            f.empty(),
            "ppp {} ×{} {:?} malla={} desplazo {:?}: {}",
            c.ppp, c.zoom, background_2, c.mesh, c.desplazo, f.summary_2()
        );
        // Las capas: un solo rectángulo de fondo, sin textura del damero ni formas de superposición.
        checks_base(&bench, &scene.output_2, background_2, c.mesh, "propiedad 3 (color propio)");
        prop_assert_eq!(bench.canvas.checker_texture_id(), None);
        checks_texture(&bench, &c.overlay, "propiedad 3 (color propio)");
    }
}
