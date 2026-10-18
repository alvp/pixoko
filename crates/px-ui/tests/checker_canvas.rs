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
//! - Fila 7: superposición con damero, en dos pasadas, antes y después de soltar.
//! - Fila 8: límites (1×1 a ×64, vista en el extremo, lienzo de 1×1 píxel físico, sin imagen y
//!   texturas máximas).
//! - Fila 14: superposición con cada fondo sólido (una sola pasada) y cambio de fondo a mitad de
//!   trazo.
//! - Fila 15 (lienzo): respaldo por malla en los bancos de las filas 5 y 7.
//! - Propiedad 2: pintar no cambia la conversión pantalla-imagen (24 casos).
//! - Propiedad 3: lo que se ve es el oráculo puro, con los cuatro fondos y los dos caminos
//!   (24 casos).
//!
//! El banco es el de `overlay_small_zoom.rs`: `Canvas::interact` y `Canvas::paint` dentro de
//! `Context::run_ui` con `RawInput` sintético, lo pintado pasa por el teselador de egui
//! (`Context::tessellate`, con los ajustes de la app) y se rasteriza **a mano** en el centro de cada
//! píxel físico (tolerancia 0,02 por canal y centro desplazado `+0,0013` y `+0,0007`, como allí; la
//! GPU aplica una regla de desempate en las aristas compartidas y aquí se evita el empate). El
//! rasterizador de esta versión pinta **todo el lienzo** en un búfer y muestrea: la textura de la
//! imagen (con su contenido conocido: el oráculo de la prueba), la textura 2×2 del damero (`A, B,
//! B, A` con envoltura y sin filtro, como la GPU) y, para el modo malla, los vértices coloreados
//! sin textura.
//!
//! Suposiciones (las filas no las fijan; la spec las deja a «Piezas»):
//! - El cuadrilátero del damero va sin tinte (vértices blancos), como ya exige el
//!   `eraser_canvas.rs` adaptado.
//! - Con un fondo sólido, la superposición va **por píxel**: el rectángulo de `c` y justo detrás
//!   el de color (una sola pasada, fila 14). Con el damero (textura), los cuadriláteros van todos
//!   antes que los rectángulos de color (dos pasadas); entre píxeles el orden no se exige.
//! - En modo malla solo se comprueba la estructura del fondo de la imagen (rectángulo `CHECK_A` y
//!   una malla de casillas `CHECK_B`); la superposición se comprueba por lo que se ve.
//! - El origen del damero es la esquina del rectángulo de la imagen en píxeles físicos, redondeada
//!   desde `CanvasInfo::image_rect`.

use std::collections::HashMap;

use egui::epaint::{ClippedPrimitive, ClippedShape, ImageDelta, Primitive, Shape, Vertex};
use egui::{
    Color32, Context, Event, ImageData, Key, Modifiers, PointerButton, Pos2, RawInput, Rect,
    TextureId, TextureOptions, Ui, Vec2, pos2, vec2,
};
use proptest::prelude::*;
use px_core::{Document, Image, Mode, Palette, Pixel, Rgba};
use px_ui::background::CanvasBackground;
use px_ui::canvas::{BACKGROUND, Canvas, CanvasError, CanvasEvent, CanvasInfo};
use px_ui::checker::{CheckerQuad, Shade, cells, clip, quad, shade_at, texels};
use px_ui::palette_grid::{CHECK_A, CHECK_B};
use px_ui::text::text_scale;
use px_ui::texture::{pixel_rgba, to_rgba8};
use px_ui::tool::{Tool, ToolAction, ToolState};
use px_ui::view::{MIN_VISIBLE, ScreenRect, Size, ViewOp};

mod common;
use common::{PPPS, boton_de, contexto, en_puntos, mover, rueda_linea, tecla};

const PANTALLA: Vec2 = vec2(400.0, 300.0);
const TOLERANCIA: f32 = 0.02;
const DESPLAZO_X: f64 = 0.0013;
const DESPLAZO_Y: f64 = 0.0007;

const TRANSPARENTE: Rgba = Rgba::new(0, 0, 0, 0);
const ROJO: Rgba = Rgba::opaque(255, 0, 0);
const ROJO_MEDIO: Rgba = Rgba::new(255, 0, 0, 128);
const AZUL: Rgba = Rgba::opaque(0, 0, 255);

const FONDOS: [CanvasBackground; 4] = [
    CanvasBackground::Checker,
    CanvasBackground::Black,
    CanvasBackground::Gray,
    CanvasBackground::White,
];
const SOLIDOS: [CanvasBackground; 3] = [
    CanvasBackground::Black,
    CanvasBackground::Gray,
    CanvasBackground::White,
];

/// Un píxel pendiente: posición y color (el que tendrá en la imagen).
type Pendiente = (u32, u32, Rgba);

// ---------------------------------------------------------------------------------------------
// Oráculos de color
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

/// Casilla del damero en el píxel físico `(px, py)` con la esquina de la imagen en `origen` y la
/// casilla de `cell` píxeles físicos, calculada aparte de la implementación: la casilla `(0, 0)` es
/// `A` y alternan por la paridad de `floor((px − ox)/cell) + floor((py − oy)/cell)`.
fn tono_del_damero(px: i64, py: i64, origen: (i64, i64), cell: i64) -> Color32 {
    let suma = (px - origen.0).div_euclid(cell) + (py - origen.1).div_euclid(cell);
    gris(if suma.rem_euclid(2) == 0 {
        CHECK_A
    } else {
        CHECK_B
    })
}

/// Color del fondo elegido bajo la transparencia en el píxel físico `(px, py)`.
fn fondo_en(fondo: CanvasBackground, px: i64, py: i64, origen: (i64, i64), cell: i64) -> Color32 {
    match fondo {
        CanvasBackground::Checker => tono_del_damero(px, py, origen, cell),
        CanvasBackground::Black => Color32::from_rgb(0, 0, 0),
        CanvasBackground::Gray => BACKGROUND,
        CanvasBackground::White => Color32::from_rgb(255, 255, 255),
    }
}

/// Un color con alfa directo por encima de un fondo opaco: lo que se ve.
fn compone(fondo: Color32, color: Rgba) -> Visto {
    let encima = Color32::from_rgba_unmultiplied(color.r, color.g, color.b, color.a);
    Visto::de_color(fondo).debajo_de(Visto::de_color(encima))
}

// ---------------------------------------------------------------------------------------------
// Imagen de la prueba (el oráculo del contenido de la textura de la imagen)
// ---------------------------------------------------------------------------------------------

#[derive(Clone)]
struct Imagen {
    ancho: u32,
    alto: u32,
    pixeles: Vec<Rgba>,
}

impl Imagen {
    fn uniforme(ancho: u32, alto: u32, color: Rgba) -> Imagen {
        Imagen {
            ancho,
            alto,
            pixeles: vec![color; (ancho * alto) as usize],
        }
    }

    fn pixel(&self, x: u32, y: u32) -> Rgba {
        self.pixeles[(y * self.ancho + x) as usize]
    }

    fn con(mut self, x: u32, y: u32, color: Rgba) -> Imagen {
        self.pixeles[(y * self.ancho + x) as usize] = color;
        self
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

    /// Los texels de la textura que sube el lienzo (premultiplicados, como los pasa egui).
    fn texels(&self) -> Vec<Color32> {
        self.pixeles
            .iter()
            .map(|c| Color32::from_rgba_unmultiplied(c.r, c.g, c.b, c.a))
            .collect()
    }
}

fn imagen_transparente() -> Imagen {
    Imagen::uniforme(64, 48, TRANSPARENTE)
}

fn imagen_azul() -> Imagen {
    Imagen::uniforme(64, 48, AZUL)
}

/// Superposición de prueba: un bloque de 4×2 píxeles por cada caso (como `overlay_small_zoom.rs`).
fn bloques() -> Vec<Pendiente> {
    let mut v = Vec::new();
    for (i, color) in [ROJO, TRANSPARENTE, ROJO_MEDIO].into_iter().enumerate() {
        for y in 0..2 {
            for x in 0..4 {
                v.push((8 + 8 * i as u32 + x, 5 + y, color));
            }
        }
    }
    v
}

/// Los veinte píxeles del trazo del borrador de la fila 10 de la 17a: la fila 5, de `x = 10` a 29.
fn los_veinte() -> Vec<(u32, u32)> {
    (10..=29).map(|x| (x, 5)).collect()
}

// ---------------------------------------------------------------------------------------------
// Rasterizador a mano
// ---------------------------------------------------------------------------------------------

/// Coordenadas baricéntricas de `p` en el triángulo, si cae dentro (bordes incluidos), en `f64`.
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
    /// Textura 2×2 del damero, si el lienzo la ha creado.
    damero: Option<TextureId>,
}

/// Texel de la textura 2×2 del damero (por filas `A, B, B, A`) con `NEAREST` y envoltura `Repeat`.
fn texel_del_damero(u: f64, v: f64) -> Visto {
    let tx = ((u * 2.0).floor() as i64).rem_euclid(2);
    let ty = ((v * 2.0).floor() as i64).rem_euclid(2);
    let c = if (tx + ty) % 2 == 0 { CHECK_A } else { CHECK_B };
    Visto::de_color(gris(c))
}

/// Búfer de lo que se ve, de `ancho × alto` píxeles físicos.
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

/// Pinta `primitivas`, en orden, en un búfer de `ancho × alto` píxeles físicos. Cada píxel se
/// muestrea en su centro (algo desplazado). Las mallas con la textura de la imagen muestrean su
/// contenido conocido; las del damero, la textura 2×2; las demás (rellenos, formas y mallas de
/// colores de vértice) usan el color de sus vértices (la textura es el píxel blanco de la fuente).
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
                // Tramo de la fila dentro del triángulo (bordes incluidos).
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
        color: Color32,
    },
    Malla {
        textura: TextureId,
        vertices: Vec<Vertex>,
        indices: Vec<u32>,
    },
}

fn recoge(shape: &Shape, salida: &mut Vec<Capa>) {
    match shape {
        Shape::Rect(r) => salida.push(Capa::Relleno {
            rect: r.rect,
            color: r.fill,
        }),
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

fn caja(vertices: &[Vertex]) -> Rect {
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

fn casi(a: Rect, b: Rect) -> bool {
    casi_con(a, b, 1e-3)
}

fn casi_con(a: Rect, b: Rect, t: f32) -> bool {
    (a.min.x - b.min.x).abs() <= t
        && (a.min.y - b.min.y).abs() <= t
        && (a.max.x - b.max.x).abs() <= t
        && (a.max.y - b.max.y).abs() <= t
}

fn rect_en_puntos(a: ScreenRect, ppp: f32) -> Rect {
    Rect::from_min_max(
        pos2(a.x as f32 / ppp, a.y as f32 / ppp),
        pos2((a.x + a.width) as f32 / ppp, (a.y + a.height) as f32 / ppp),
    )
}

// ---------------------------------------------------------------------------------------------
// El banco
// ---------------------------------------------------------------------------------------------

/// Lo que dejó un fotograma. Los cambios de textura ya están limpiados (egui entra en pánico si
/// se sueltan sin aplicar) y copiados en `subidas` y `liberadas`.
struct Salida {
    formas: Vec<ClippedShape>,
    subidas: Vec<(TextureId, ImageDelta)>,
    liberadas: Vec<TextureId>,
    info: Option<CanvasInfo>,
    trazo: Vec<CanvasEvent>,
}

impl Salida {
    fn capas(&self) -> Vec<Capa> {
        capas_de(&self.formas)
    }

    fn info(&self) -> &CanvasInfo {
        self.info.as_ref().expect("el lienzo se pinta")
    }

    fn subidas_de(&self, id: TextureId) -> Vec<&ImageDelta> {
        self.subidas
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
    origen: (i64, i64),
    ancho: i64,
    alto: i64,
    /// Píxeles físicos por píxel de imagen.
    z: i64,
    viewport: Size,
    /// Casilla del damero.
    cell: i64,
}

impl Geom {
    fn rect(&self) -> ScreenRect {
        ScreenRect {
            x: self.origen.0,
            y: self.origen.1,
            width: self.ancho,
            height: self.alto,
        }
    }

    fn recorte(&self) -> Option<ScreenRect> {
        clip(self.rect(), self.viewport)
    }

    fn bloque(&self, x: u32, y: u32) -> ScreenRect {
        ScreenRect {
            x: self.origen.0 + i64::from(x) * self.z,
            y: self.origen.1 + i64::from(y) * self.z,
            width: self.z,
            height: self.z,
        }
    }

    fn cuadrilatero(&self, area: ScreenRect) -> Option<CheckerQuad> {
        quad(area, self.origen, self.cell)
    }
}

struct Banco {
    ctx: Context,
    ppp: f32,
    pantalla: Vec2,
    canvas: Canvas,
    /// El contenido que debe tener la textura de la imagen.
    imagen: Imagen,
}

impl Banco {
    /// Sin ningún fotograma ejecutado: lo que sube el primero se puede mirar.
    fn sin_calentar(
        pantalla: Vec2,
        ppp: f32,
        imagen: Imagen,
        fondo: CanvasBackground,
        malla: bool,
    ) -> Banco {
        let mut canvas = Canvas::new();
        canvas.set_background(fondo);
        canvas.set_checker_mesh(malla);
        canvas.set_image(&imagen.image());
        Banco {
            ctx: contexto(ppp),
            ppp,
            pantalla,
            canvas,
            imagen,
        }
    }

    fn nuevo_con(ppp: f32, imagen: Imagen, fondo: CanvasBackground, malla: bool) -> Banco {
        let mut banco = Banco::sin_calentar(PANTALLA, ppp, imagen, fondo, malla);
        for _ in 0..3 {
            banco.corre(vec![], &[]);
        }
        banco
    }

    fn nuevo(ppp: f32, imagen: Imagen) -> Banco {
        Banco::nuevo_con(ppp, imagen, CanvasBackground::Checker, false)
    }

    /// Un fotograma: `dibuja` recibe el lienzo y la interfaz y devuelve los eventos de trazo y la
    /// información de `paint`, si lo llamó.
    fn corre_con(
        &mut self,
        events: Vec<Event>,
        max_texture_side: Option<usize>,
        dibuja: impl FnOnce(&mut Canvas, &mut Ui) -> (Vec<CanvasEvent>, Option<CanvasInfo>),
    ) -> Salida {
        let input = RawInput {
            screen_rect: Some(Rect::from_min_size(Pos2::ZERO, self.pantalla)),
            events,
            max_texture_side,
            ..RawInput::default()
        };
        let canvas = &mut self.canvas;
        let mut dibuja = Some(dibuja);
        let mut resultado = None;
        let mut output = self.ctx.run_ui(input, |ui| {
            let dibuja = dibuja.take().expect("un solo dibujo por fotograma");
            resultado = Some(dibuja(canvas, ui));
        });
        let (trazo, info) = resultado.expect("el lienzo se dibuja");
        let mut subidas = Vec::new();
        for (id, deltas) in &output.textures_delta.set {
            for delta in deltas {
                subidas.push((*id, delta.clone()));
            }
        }
        let liberadas = output.textures_delta.free.iter().copied().collect();
        output.textures_delta.clear();
        Salida {
            formas: std::mem::take(&mut output.shapes),
            subidas,
            liberadas,
            info,
            trazo,
        }
    }

    /// `interact` y `paint` con la superposición dada (lo que hace la app, sin herramienta).
    fn corre(&mut self, events: Vec<Event>, overlay: &[Pendiente]) -> Salida {
        self.corre_max(events, overlay, None)
    }

    fn corre_max(
        &mut self,
        events: Vec<Event>,
        overlay: &[Pendiente],
        max: Option<usize>,
    ) -> Salida {
        self.corre_con(events, max, |canvas, ui| {
            let trazo = canvas.interact(ui);
            let info = canvas.paint(ui, overlay.iter().copied());
            (trazo, Some(info))
        })
    }

    /// Solo `interact`: sin pintar.
    fn solo_interact(&mut self, events: Vec<Event>) -> Salida {
        self.corre_con(events, None, |canvas, ui| (canvas.interact(ui), None))
    }

    /// Tamaño del lienzo en píxeles físicos.
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
            ppp,
            origen: (
                (rect.min.x * ppp).round() as i64,
                (rect.min.y * ppp).round() as i64,
            ),
            ancho: (rect.width() * ppp).round() as i64,
            alto: (rect.height() * ppp).round() as i64,
            z: info.zoom.scale(1),
            viewport: self.viewport(),
            cell: 8 * i64::from(text_scale(ppp)),
        }
    }

    fn texturas(&self) -> Texturas {
        Texturas {
            imagen: self.canvas.texture_id().expect("hay textura de la imagen"),
            ancho: self.imagen.ancho as usize,
            alto: self.imagen.alto as usize,
            texels: self.imagen.texels(),
            damero: self.canvas.checker_texture_id(),
        }
    }

    /// Un fotograma con la superposición dada, teselado y rasterizado entero.
    fn escena(&mut self, overlay: &[Pendiente]) -> Escena {
        let salida = self.corre(vec![], overlay);
        self.rasteriza_salida(salida)
    }

    fn rasteriza_salida(&self, salida: Salida) -> Escena {
        let primitivas = self.ctx.tessellate(salida.formas.clone(), self.ppp);
        let viewport = self.viewport();
        let buffer = rasteriza(
            &primitivas,
            self.ppp,
            viewport.width as usize,
            viewport.height as usize,
            &self.texturas(),
        );
        let geom = self.geom(salida.info());
        Escena {
            salida,
            primitivas,
            buffer,
            geom,
        }
    }

    /// Lleva el zoom a ×`zoom` con las operaciones de vista.
    fn con_zoom(&mut self, zoom: u32) {
        self.canvas.request_view(ViewOp::Actual);
        for _ in 1..zoom {
            self.canvas.request_view(ViewOp::ZoomIn);
        }
        self.corre(vec![], &[]);
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
    fn arrastra(&mut self, dx: i64, dy: i64) -> Vec<Salida> {
        let inicio = pos2(self.pantalla.x / 2.0, self.pantalla.y / 2.0);
        let lejos = inicio + vec2(40.0, 40.0);
        let destino = inicio + vec2(dx as f32 / self.ppp, dy as f32 / self.ppp);
        let pasos = [
            vec![mover(inicio)],
            vec![boton_de(inicio, PointerButton::Middle, true)],
            vec![mover(lejos)],
            vec![mover(destino)],
            vec![boton_de(destino, PointerButton::Middle, false)],
            vec![],
        ];
        pasos
            .into_iter()
            .map(|events| self.corre(events, &[]))
            .collect()
    }

    /// Arrastra la vista varias veces hacia un lado hasta que el límite de la vista la frena.
    fn arrastra_al_extremo(&mut self, sentido_x: i64, sentido_y: i64) {
        let paso = (
            (150.0 * self.ppp).round() as i64,
            (100.0 * self.ppp).round() as i64,
        );
        for _ in 0..6 {
            self.arrastra(sentido_x * paso.0, sentido_y * paso.1);
        }
    }

    /// Como `Canvas::refresh` y deja anotado el contenido que debe tener la textura.
    fn refresca(&mut self, imagen: Imagen) {
        self.canvas.refresh(&self.ctx, &imagen.image());
        self.imagen = imagen;
    }

    fn pone_imagen(&mut self, imagen: Imagen) {
        self.canvas.set_image(&imagen.image());
        self.imagen = imagen;
    }
}

/// Un fotograma teselado y rasterizado.
struct Escena {
    salida: Salida,
    primitivas: Vec<ClippedPrimitive>,
    buffer: Buffer,
    geom: Geom,
}

// ---------------------------------------------------------------------------------------------
// Comprobaciones
// ---------------------------------------------------------------------------------------------

struct Fallos {
    total: usize,
    primeros: Vec<String>,
}

impl Fallos {
    fn nuevo() -> Fallos {
        Fallos {
            total: 0,
            primeros: Vec::new(),
        }
    }

    fn anota(&mut self, mensaje: impl FnOnce() -> String) {
        self.total += 1;
        if self.primeros.len() < 5 {
            self.primeros.push(mensaje());
        }
    }

    fn vacio(&self) -> bool {
        self.total == 0
    }

    fn resumen(&self) -> String {
        format!("{} fallos; los primeros: {:#?}", self.total, self.primeros)
    }
}

/// Todos los píxeles físicos del lienzo se ven como el oráculo puro: dentro de la imagen, el píxel
/// de la imagen (o el de la superposición, que lo sustituye) sobre el fondo elegido; fuera,
/// `BACKGROUND`.
fn fallos(
    escena: &Escena,
    imagen: &Imagen,
    overlay: &[Pendiente],
    fondo: CanvasBackground,
) -> Fallos {
    let g = escena.geom;
    let mapa: HashMap<(u32, u32), Rgba> = overlay.iter().map(|&(x, y, c)| ((x, y), c)).collect();
    let mut fallos = Fallos::nuevo();
    for py in 0..escena.buffer.alto as i64 {
        for px in 0..escena.buffer.ancho as i64 {
            let dentro = px >= g.origen.0
                && px < g.origen.0 + g.ancho
                && py >= g.origen.1
                && py < g.origen.1 + g.alto;
            let esperado = if dentro {
                let ix = ((px - g.origen.0) / g.z) as u32;
                let iy = ((py - g.origen.1) / g.z) as u32;
                let color = mapa
                    .get(&(ix, iy))
                    .copied()
                    .unwrap_or_else(|| imagen.pixel(ix, iy));
                compone(fondo_en(fondo, px, py, g.origen, g.cell), color)
            } else {
                Visto::de_color(BACKGROUND)
            };
            let visto = escena.buffer.en(px, py);
            if !visto.cerca(esperado, TOLERANCIA) {
                fallos.anota(|| {
                    format!(
                        "ppp {} zoom ×{}: el píxel físico ({px}, {py}) se ve {visto:?} y debía verse {esperado:?} (dentro de la imagen: {dentro})",
                        g.ppp, g.z
                    )
                });
            }
        }
    }
    fallos
}

fn sin_fallos(fallos: &Fallos, etiqueta: &str) {
    assert!(fallos.vacio(), "{etiqueta}: {}", fallos.resumen());
}

/// Un cuadrilátero del damero: 4 vértices, 6 índices, sin tinte, en el sitio y con los uv de
/// `quad`.
fn comprueba_cuadrilatero(capa: &Capa, esperado: &CheckerQuad, ppp: f32, etiqueta: &str) {
    let Capa::Malla {
        vertices, indices, ..
    } = capa
    else {
        panic!("{etiqueta}: se esperaba una malla y hay {capa:?}");
    };
    assert_eq!(vertices.len(), 4, "{etiqueta}: 4 vértices");
    assert_eq!(indices.len(), 6, "{etiqueta}: 6 índices");
    let en_puntos = rect_en_puntos(esperado.rect, ppp);
    assert!(
        casi(caja(vertices), en_puntos),
        "{etiqueta}: la malla ocupa {:?} y debía ocupar {en_puntos:?}",
        caja(vertices)
    );
    assert_eq!(
        caja_uv(vertices),
        esperado.uv,
        "{etiqueta}: uv de `quad` ({:?})",
        esperado.rect
    );
    assert!(
        vertices.iter().all(|v| v.color == Color32::WHITE),
        "{etiqueta}: sin tinte"
    );
}

/// Área de un triángulo de la malla, en píxeles físicos cuadrados.
fn area_de_malla(vertices: &[Vertex], indices: &[u32], ppp: f32) -> f64 {
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
        .sum()
}

/// Comprueba las capas anteriores a la malla de la imagen según el fondo y el camino, y devuelve
/// el índice de la malla de la imagen:
/// - `BACKGROUND` en todo el lienzo;
/// - `Checker` con textura: el cuadrilátero del damero (`quad(clip(imagen))`);
/// - `Checker` con malla: rectángulo `CHECK_A` y **una** malla de colores de vértice con las casillas
///   `CHECK_B`, sin textura del damero;
/// - un sólido: un rectángulo de ese color sobre `clip(imagen)`;
///
/// y, después, la malla de la imagen (una sola, con la textura de la imagen).
fn comprueba_base(
    banco: &Banco,
    salida: &Salida,
    fondo: CanvasBackground,
    malla: bool,
    etiqueta: &str,
) -> usize {
    let capas = salida.capas();
    let g = banco.geom(salida.info());
    let imagen = banco.canvas.texture_id().expect("textura de la imagen");
    match &capas[0] {
        Capa::Relleno { rect, color } => {
            assert_eq!(*color, BACKGROUND, "{etiqueta}: el fondo liso");
            // El rectángulo del lienzo lo coloca egui, que redondea el diseño a 1/16 de punto.
            let lienzo = Rect::from_min_size(Pos2::ZERO, banco.pantalla);
            assert!(
                casi_con(*rect, lienzo, 0.04),
                "{etiqueta}: el fondo cubre el lienzo: {rect:?} y {lienzo:?}"
            );
        }
        otra => panic!("{etiqueta}: la primera capa es el fondo liso y hay {otra:?}"),
    }
    let mut i = 1;
    if let Some(area) = g.recorte() {
        let en_puntos = rect_en_puntos(area, g.ppp);
        match (fondo, malla) {
            (CanvasBackground::Checker, false) => {
                let damero = banco
                    .canvas
                    .checker_texture_id()
                    .unwrap_or_else(|| panic!("{etiqueta}: hay textura del damero"));
                assert_ne!(damero, imagen, "{etiqueta}: texturas distintas");
                let Capa::Malla { textura, .. } = &capas[i] else {
                    panic!(
                        "{etiqueta}: la segunda capa es el damero y hay {:?}",
                        capas[i]
                    );
                };
                assert_eq!(*textura, damero, "{etiqueta}: textura del damero");
                let esperado = g.cuadrilatero(area).expect("recorte no vacío");
                assert_eq!(esperado.rect, area);
                comprueba_cuadrilatero(&capas[i], &esperado, g.ppp, etiqueta);
                i += 1;
            }
            (CanvasBackground::Checker, true) => {
                if let Some(damero) = banco.canvas.checker_texture_id() {
                    assert!(
                        !capas.iter().any(
                            |c| matches!(c, Capa::Malla { textura, .. } if *textura == damero)
                        ),
                        "{etiqueta}: en modo malla nadie usa la textura del damero"
                    );
                }
                let Capa::Relleno { rect, color } = &capas[i] else {
                    panic!("{etiqueta}: rectángulo CHECK_A y hay {:?}", capas[i]);
                };
                assert_eq!(*color, gris(CHECK_A), "{etiqueta}: rectángulo de CHECK_A");
                assert!(casi(*rect, en_puntos), "{etiqueta}: {rect:?}");
                i += 1;
                let casillas_b: Vec<ScreenRect> = cells(area, g.origen, g.cell)
                    .filter(|c| c.shade == Shade::B)
                    .map(|c| c.rect)
                    .collect();
                if !casillas_b.is_empty() {
                    let Capa::Malla {
                        textura,
                        vertices,
                        indices,
                    } = &capas[i]
                    else {
                        panic!("{etiqueta}: una malla de casillas B y hay {:?}", capas[i]);
                    };
                    assert_ne!(*textura, imagen, "{etiqueta}");
                    assert!(
                        vertices.iter().all(|v| v.color == gris(CHECK_B)),
                        "{etiqueta}: colores de vértice CHECK_B"
                    );
                    let esperado: i64 = casillas_b.iter().map(|c| c.width * c.height).sum();
                    let real = area_de_malla(vertices, indices, g.ppp);
                    assert!(
                        (real - esperado as f64).abs() < 0.5,
                        "{etiqueta}: las casillas B suman {esperado} px² y la malla {real}"
                    );
                    i += 1;
                }
            }
            (solido, _) => {
                let color = fondo_en(solido, 0, 0, (0, 0), 8);
                let Capa::Relleno { rect, color: real } = &capas[i] else {
                    panic!(
                        "{etiqueta}: rectángulo de fondo sólido y hay {:?}",
                        capas[i]
                    );
                };
                assert_eq!(*real, color, "{etiqueta}: color del fondo sólido");
                assert!(
                    casi(*rect, en_puntos),
                    "{etiqueta}: {rect:?} y {en_puntos:?}"
                );
                assert_eq!(
                    banco.canvas.checker_texture_id(),
                    None,
                    "{etiqueta}: con un sólido no se crea la textura del damero"
                );
                i += 1;
            }
        }
    }
    match &capas[i] {
        Capa::Malla { textura, .. } => {
            assert_eq!(*textura, imagen, "{etiqueta}: la malla de la imagen");
        }
        otra => panic!("{etiqueta}: la malla de la imagen y hay {otra:?}"),
    }
    let con_la_imagen = capas
        .iter()
        .filter(|c| matches!(c, Capa::Malla { textura, .. } if *textura == imagen))
        .count();
    assert_eq!(con_la_imagen, 1, "{etiqueta}: una sola malla con la imagen");
    i
}

/// Esquina, tamaño y valor de una pieza de la superposición, en píxeles físicos, para comparar sin
/// importar el orden.
type Pieza = (i64, i64, i64, i64, [u32; 4]);

fn pieza_de_caja(r: Rect, ppp: f32, extra: [u32; 4]) -> Pieza {
    (
        (r.min.x * ppp).round() as i64,
        (r.min.y * ppp).round() as i64,
        (r.width() * ppp).round() as i64,
        (r.height() * ppp).round() as i64,
        extra,
    )
}

fn color_bits(c: Color32) -> [u32; 4] {
    [
        u32::from(c.r()),
        u32::from(c.g()),
        u32::from(c.b()),
        u32::from(c.a()),
    ]
}

fn color_del_pixel(color: Rgba) -> Color32 {
    Color32::from_rgba_unmultiplied(color.r, color.g, color.b, color.a)
}

/// La superposición (capas posteriores a la malla de la imagen) según el fondo.
///
/// Con `Checker` y textura: primera pasada, un cuadrilátero `quad(bloque)` por píxel con alfa menor
/// que 255 y ninguno con alfa 255; segunda pasada, un rectángulo de color por píxel con alfa mayor
/// que 0; los cuadriláteros van **todos antes** que los rectángulos de color y no hay ningún
/// rectángulo de `BACKGROUND`. Con un sólido: una sola pasada, por píxel el rectángulo del fondo
/// (si alfa < 255) y justo detrás el de color (si alfa > 0), sin ningún cuadrilátero del damero.
fn comprueba_superposicion(
    banco: &Banco,
    g: &Geom,
    despues_de_la_imagen: &[Capa],
    overlay: &[Pendiente],
    fondo: CanvasBackground,
    malla: bool,
    etiqueta: &str,
) {
    let ppp = g.ppp;
    for capa in despues_de_la_imagen {
        // Con el sólido `Gray` el rectángulo de fondo vale lo mismo que `BACKGROUND`: legítimo.
        if fondo == CanvasBackground::Checker
            && let Capa::Relleno { color, .. } = capa
        {
            assert_ne!(
                *color, BACKGROUND,
                "{etiqueta}: ningún rectángulo de BACKGROUND en la superposición"
            );
        }
    }
    let mut colores: Vec<Pieza> = overlay
        .iter()
        .filter(|(_, _, c)| c.a > 0)
        .map(|&(x, y, c)| {
            let b = g.bloque(x, y);
            (b.x, b.y, b.width, b.height, color_bits(color_del_pixel(c)))
        })
        .collect();
    colores.sort_unstable();
    match (fondo, malla) {
        (CanvasBackground::Checker, false) => {
            let damero = banco
                .canvas
                .checker_texture_id()
                .expect("textura del damero");
            let n_cuadrilateros = despues_de_la_imagen
                .iter()
                .take_while(|c| matches!(c, Capa::Malla { textura, .. } if *textura == damero))
                .count();
            // Los rectángulos de color vienen después de todos los cuadriláteros y no hay más
            // mallas del damero entre ellos.
            let resto = &despues_de_la_imagen[n_cuadrilateros..];
            assert!(
                resto.iter().all(|c| matches!(c, Capa::Relleno { .. })),
                "{etiqueta}: tras los cuadriláteros solo hay rectángulos de color: {resto:?}"
            );
            let mut esperados: Vec<Pieza> = Vec::new();
            let mut reales: Vec<Pieza> = Vec::new();
            for &(x, y, c) in overlay {
                if c.a < 255 {
                    let b = g.bloque(x, y);
                    let q = g.cuadrilatero(b).expect("bloque no vacío");
                    esperados.push((b.x, b.y, b.width, b.height, q.uv.map(f32::to_bits)));
                }
            }
            for capa in &despues_de_la_imagen[..n_cuadrilateros] {
                let Capa::Malla {
                    vertices, indices, ..
                } = capa
                else {
                    unreachable!()
                };
                assert_eq!(vertices.len(), 4, "{etiqueta}: 4 vértices");
                assert_eq!(indices.len(), 6, "{etiqueta}: 6 índices");
                assert!(
                    vertices.iter().all(|v| v.color == Color32::WHITE),
                    "{etiqueta}: sin tinte"
                );
                reales.push(pieza_de_caja(
                    caja(vertices),
                    ppp,
                    caja_uv(vertices).map(f32::to_bits),
                ));
            }
            esperados.sort_unstable();
            reales.sort_unstable();
            assert_eq!(
                reales, esperados,
                "{etiqueta}: un cuadrilátero del damero por píxel con alfa < 255, con los uv de `quad`"
            );
            let mut rects: Vec<Pieza> = resto
                .iter()
                .map(|c| {
                    let Capa::Relleno { rect, color } = c else {
                        unreachable!()
                    };
                    pieza_de_caja(*rect, ppp, color_bits(*color))
                })
                .collect();
            rects.sort_unstable();
            assert_eq!(
                rects, colores,
                "{etiqueta}: un rectángulo de color por píxel con alfa > 0"
            );
        }
        (CanvasBackground::Checker, true) => {
            // Modo malla: la superposición se comprueba por lo que se ve. Sin textura del damero.
            assert!(
                !despues_de_la_imagen.iter().any(|c| matches!(
                    c,
                    Capa::Malla { textura, .. } if Some(*textura) == banco.canvas.checker_texture_id()
                )),
                "{etiqueta}: sin cuadriláteros de textura en modo malla"
            );
        }
        (solido, _) => {
            // Una sola pasada, por píxel y en orden.
            let c = fondo_en(solido, 0, 0, (0, 0), 8);
            let mut esperado: Vec<Pieza> = Vec::new();
            for &(x, y, color) in overlay {
                let b = g.bloque(x, y);
                if color.a < 255 {
                    esperado.push((b.x, b.y, b.width, b.height, color_bits(c)));
                }
                if color.a > 0 {
                    esperado.push((
                        b.x,
                        b.y,
                        b.width,
                        b.height,
                        color_bits(color_del_pixel(color)),
                    ));
                }
            }
            let reales: Vec<Pieza> = despues_de_la_imagen
                .iter()
                .map(|capa| match capa {
                    Capa::Relleno { rect, color } => pieza_de_caja(*rect, ppp, color_bits(*color)),
                    otra => panic!(
                        "{etiqueta}: con un fondo sólido la superposición son rectángulos y hay {otra:?}"
                    ),
                })
                .collect();
            assert_eq!(
                reales, esperado,
                "{etiqueta}: una sola pasada: por píxel el rectángulo del fondo y encima el de color"
            );
        }
    }
}

// ---------------------------------------------------------------------------------------------
// Fila 5: damero sin bordes grises
// ---------------------------------------------------------------------------------------------

/// Fila 5 para una escala, un zoom y un camino: cada píxel físico del rectángulo de la imagen se
/// ve exactamente `shade_at`, fuera `BACKGROUND`.
fn fila_5_escena(ppp: f32, zoom: u32, malla: bool) -> (Banco, Escena) {
    let etiqueta = format!("ppp {ppp} ×{zoom} malla={malla}");
    let mut banco = Banco::nuevo_con(ppp, imagen_transparente(), CanvasBackground::Checker, malla);
    banco.con_zoom(zoom);
    let escena = banco.escena(&[]);
    let g = escena.geom;
    assert_eq!(
        g.z,
        i64::from(zoom),
        "{etiqueta}: el zoom se mide en píxeles físicos"
    );
    assert_eq!(g.ancho, 64 * g.z);
    assert_eq!(g.alto, 48 * g.z);
    let f = fallos(&escena, &banco.imagen, &[], CanvasBackground::Checker);
    sin_fallos(&f, &etiqueta);
    // Y con la función de la spec, `shade_at`, en cada píxel de la imagen (también la primera y la
    // última fila y columna).
    for py in g.origen.1..g.origen.1 + g.alto {
        for px in g.origen.0..g.origen.0 + g.ancho {
            let esperado = shade_at(px, py, g.origen, g.cell).color();
            let visto = escena.buffer.en(px, py);
            assert!(
                visto.cerca(Visto::de_color(gris(esperado)), TOLERANCIA),
                "{etiqueta}: ({px}, {py}) se ve {visto:?} y `shade_at` da {esperado:?}"
            );
        }
    }
    (banco, escena)
}

#[test]
fn fila_5_cada_pixel_de_la_imagen_se_ve_exactamente_como_shade_at_y_fuera_el_fondo() {
    for ppp in PPPS {
        for zoom in 1..=3 {
            fila_5_escena(ppp, zoom, false);
        }
    }
}

/// Fila 5, tras arrastrar `(dx, dy)`: el color de cada píxel físico de la imagen relativo a su
/// esquina es el mismo que antes (el damero va con la imagen).
fn fila_5_tras_arrastrar(ppp: f32, zoom: u32, desplazos: &[(i64, i64)], malla: bool) {
    let (mut banco, antes) = fila_5_escena(ppp, zoom, malla);
    let g0 = antes.geom;
    let (mut x, mut y) = g0.origen;
    for &(dx, dy) in desplazos {
        let etiqueta = format!("ppp {ppp} ×{zoom} arrastre ({dx}, {dy}) malla={malla}");
        banco.arrastra(dx, dy);
        let despues = banco.escena(&[]);
        let g1 = despues.geom;
        assert_eq!(
            g1.origen,
            (x + dx, y + dy),
            "{etiqueta}: la imagen se desplazó exactamente lo arrastrado"
        );
        (x, y) = g1.origen;
        let f = fallos(&despues, &banco.imagen, &[], CanvasBackground::Checker);
        sin_fallos(&f, &etiqueta);
        for ry in 0..g0.alto {
            for rx in 0..g0.ancho {
                let antes_ = antes.buffer.en(g0.origen.0 + rx, g0.origen.1 + ry);
                let despues_ = despues.buffer.en(g1.origen.0 + rx, g1.origen.1 + ry);
                assert!(
                    antes_.cerca(despues_, TOLERANCIA),
                    "{etiqueta}: el píxel ({rx}, {ry}) relativo a la esquina era {antes_:?} y ahora es {despues_:?}"
                );
            }
        }
    }
}

#[test]
fn fila_5_tras_arrastrar_la_vista_7_y_13_pixeles_el_damero_va_con_la_imagen() {
    for ppp in PPPS {
        for zoom in 1..=3 {
            // Un arrastre de 7 píxeles en x y 13 en y, y luego otro de 13 en x y 7 en y.
            fila_5_tras_arrastrar(ppp, zoom, &[(7, 13), (13, 7)], false);
        }
    }
}

// ---------------------------------------------------------------------------------------------
// Fila 6: capas, textura, coste y documento
// ---------------------------------------------------------------------------------------------

/// Una imagen indexada de 64×48 con cuatro entradas y la entrada 0 transparente.
fn indexada() -> Image {
    let mut paleta = Palette::from_colors([
        Rgba::new(0, 0, 0, 0),
        Rgba::opaque(255, 0, 0),
        Rgba::opaque(0, 255, 0),
        Rgba::opaque(0, 0, 255),
    ])
    .expect("paleta");
    paleta
        .set_transparent_index(Some(0))
        .expect("índice transparente");
    let mut image = Image::new_indexed(64, 48, paleta).expect("imagen indexada");
    for x in 0..32 {
        image.set_pixel(x, 3, Pixel::Indexed(1)).expect("dentro");
    }
    image
}

/// Las comprobaciones de un fotograma en reposo con el damero por textura: capas en orden, un solo
/// cuadrilátero con la textura del damero y nada más (sin superposición).
fn comprueba_reposo(banco: &Banco, salida: &Salida, etiqueta: &str) {
    let i = comprueba_base(banco, salida, CanvasBackground::Checker, false, etiqueta);
    assert_eq!(
        salida.capas().len(),
        i + 1,
        "{etiqueta}: fondo, damero e imagen y nada más"
    );
}

#[test]
fn fila_6_el_primer_fotograma_pinta_fondo_damero_e_imagen_en_ese_orden() {
    for ppp in PPPS {
        let mut banco = Banco::sin_calentar(
            PANTALLA,
            ppp,
            imagen_transparente(),
            CanvasBackground::Checker,
            false,
        );
        assert_eq!(banco.canvas.checker_texture_id(), None, "antes de pintar");
        let salida = banco.corre(vec![], &[]);
        assert!(banco.canvas.checker_texture_id().is_some());
        comprueba_reposo(&banco, &salida, &format!("ppp {ppp} primer fotograma"));
        // Mientras las cuenta: tres capas, una malla de la textura del damero, otra de la imagen.
        let capas = salida.capas();
        assert_eq!(capas.len(), 3);
        assert!(matches!(capas[0], Capa::Relleno { .. }));
        assert!(matches!(capas[1], Capa::Malla { .. }));
        assert!(matches!(capas[2], Capa::Malla { .. }));
    }
}

#[test]
fn fila_6_la_textura_del_damero_es_2x2_con_envoltura_con_los_texels_y_se_sube_una_vez() {
    for ppp in PPPS {
        let mut banco = Banco::sin_calentar(
            PANTALLA,
            ppp,
            imagen_transparente(),
            CanvasBackground::Checker,
            false,
        );
        let primero = banco.corre(vec![], &[]);
        let damero = banco
            .canvas
            .checker_texture_id()
            .expect("textura del damero");
        let imagen = banco.canvas.texture_id().expect("textura de la imagen");
        assert_ne!(damero, imagen);
        let subidas = primero.subidas_de(damero);
        assert_eq!(subidas.len(), 1, "una sola subida de la textura del damero");
        let delta = subidas[0];
        assert!(delta.is_whole(), "se sube entera");
        assert_eq!(delta.options, TextureOptions::NEAREST_REPEAT);
        let ImageData::Color(color) = &delta.image;
        assert_eq!(color.size, [2, 2], "ppp {ppp}");
        let esperado: Vec<Color32> = texels().iter().map(|c| gris(*c)).collect();
        assert_eq!(color.pixels, esperado, "píxeles = texels() = [A, B, B, A]");
        assert_eq!(esperado[0], gris(CHECK_A));
        assert_eq!(esperado[1], gris(CHECK_B));
        // La imagen se sube una vez, con su contenido.
        let de_la_imagen = primero.subidas_de(imagen);
        assert_eq!(de_la_imagen.len(), 1);
        let ImageData::Color(color) = &de_la_imagen[0].image;
        assert_eq!(color.size, [64, 48]);
        assert_eq!(color.pixels, banco.imagen.texels());
        assert_eq!(primero.subidas.len(), 2, "solo esas dos subidas");

        // Ningún fotograma posterior sube la textura del damero.
        for k in 0..4 {
            let f = banco.corre(vec![], &[]);
            assert!(f.subidas.is_empty(), "ppp {ppp} fotograma {k}: sin subidas");
            assert!(f.liberadas.is_empty());
            assert_eq!(banco.canvas.checker_texture_id(), Some(damero));
        }
    }
}

#[test]
fn fila_6_el_damero_conserva_su_textura_con_pan_zoom_refresh_set_image_y_preview_entry() {
    let ppp = 1.5;
    let mut banco = Banco::sin_calentar(
        PANTALLA,
        ppp,
        imagen_transparente(),
        CanvasBackground::Checker,
        false,
    );
    let primero = banco.corre(vec![], &[]);
    comprueba_reposo(&banco, &primero, "primer fotograma");
    let damero = banco
        .canvas
        .checker_texture_id()
        .expect("textura del damero");
    let revisa = |banco: &Banco, salida: &Salida, etiqueta: &str| {
        assert_eq!(
            banco.canvas.checker_texture_id(),
            Some(damero),
            "{etiqueta}: el id no cambia"
        );
        assert!(
            salida.subidas_de(damero).is_empty(),
            "{etiqueta}: ninguna subida del damero"
        );
        assert!(
            !salida.liberadas.contains(&damero),
            "{etiqueta}: no se libera"
        );
        comprueba_reposo(banco, salida, etiqueta);
    };

    // Un `pan`.
    for (k, salida) in banco.arrastra(7, 13).iter().enumerate() {
        revisa(&banco, salida, &format!("pan, fotograma {k}"));
    }
    // Un `zoom`.
    banco.canvas.request_view(ViewOp::ZoomIn);
    let salida = banco.corre(vec![], &[]);
    revisa(&banco, &salida, "zoom");
    // `refresh`: la imagen se sube de nuevo; el damero, no.
    let imagen = banco.canvas.texture_id().expect("textura de la imagen");
    banco.refresca(imagen_transparente().con(3, 3, ROJO));
    let salida = banco.corre(vec![], &[]);
    revisa(&banco, &salida, "refresh");
    assert_eq!(salida.subidas_de(imagen).len(), 1, "refresh sube la imagen");
    // `set_image` con otra imagen (otro tamaño): otra textura de la imagen, el mismo damero.
    banco.pone_imagen(Imagen::uniforme(40, 30, TRANSPARENTE));
    let salida = banco.corre(vec![], &[]);
    revisa(&banco, &salida, "set_image");
    assert_ne!(banco.canvas.texture_id(), Some(imagen), "otra imagen");
    // `preview_entry` sobre una indexada.
    let indexada = indexada();
    banco.canvas.set_image(&indexada);
    banco.imagen = Imagen {
        ancho: 64,
        alto: 48,
        pixeles: vec![TRANSPARENTE; 64 * 48],
    };
    let salida = banco.corre(vec![], &[]);
    revisa(&banco, &salida, "set_image indexada");
    let textura_indexada = banco.canvas.texture_id().expect("textura de la imagen");
    banco
        .canvas
        .preview_entry(&banco.ctx, &indexada, 2, Rgba::opaque(9, 9, 9));
    let salida = banco.corre(vec![], &[]);
    revisa(&banco, &salida, "preview_entry");
    assert_eq!(
        salida.subidas_de(textura_indexada).len(),
        1,
        "la vista previa sube la imagen"
    );
    banco.canvas.end_preview(&banco.ctx, &indexada);
    let salida = banco.corre(vec![], &[]);
    revisa(&banco, &salida, "end_preview");
}

#[test]
fn fila_6_con_una_imagen_de_2048x64_a_x64_el_damero_no_mide_mas_que_el_lienzo() {
    for ppp in PPPS {
        let mut banco = Banco::nuevo(ppp, Imagen::uniforme(2048, 64, TRANSPARENTE));
        banco.con_zoom(64);
        let salida = banco.corre(vec![], &[]);
        let etiqueta = format!("ppp {ppp} 2048×64 a ×64");
        let g = banco.geom(salida.info());
        assert_eq!(g.ancho, 131_072, "{etiqueta}: el rectángulo de la imagen");
        assert_eq!(g.alto, 4096, "{etiqueta}");
        let i = comprueba_base(&banco, &salida, CanvasBackground::Checker, false, &etiqueta);
        assert_eq!(i, 2);
        let capas = salida.capas();
        let Capa::Malla { vertices, .. } = &capas[1] else {
            panic!("{etiqueta}: el damero es una malla");
        };
        let caja = caja(vertices);
        let lienzo = Rect::from_min_size(Pos2::ZERO, banco.pantalla);
        assert!(
            caja.min.x >= lienzo.min.x - 1e-3
                && caja.min.y >= lienzo.min.y - 1e-3
                && caja.max.x <= lienzo.max.x + 1e-3
                && caja.max.y <= lienzo.max.y + 1e-3,
            "{etiqueta}: el damero cabe en el lienzo: {caja:?}"
        );
        // Los uv no se disparan: a lo sumo unos cientos de períodos.
        let uv = caja_uv(vertices);
        assert!(
            (0.0..1.0).contains(&uv[0]) && (0.0..1.0).contains(&uv[1]),
            "{etiqueta}: {uv:?}"
        );
        assert!(
            uv[2] - uv[0] < 400.0 && uv[3] - uv[1] < 400.0,
            "{etiqueta}: {uv:?}"
        );
    }
}

#[test]
fn fila_6_con_too_large_no_hay_damero_ni_textura_del_damero_solo_el_fondo() {
    for ppp in PPPS {
        let mut banco = Banco::sin_calentar(
            PANTALLA,
            ppp,
            Imagen::uniforme(1025, 2, TRANSPARENTE),
            CanvasBackground::Checker,
            false,
        );
        for k in 0..3 {
            let salida = banco.corre_max(vec![], &[], Some(1024));
            assert_eq!(
                salida.info().error,
                Some(CanvasError::TooLarge {
                    width: 1025,
                    height: 2,
                    max: 1024
                }),
                "ppp {ppp} fotograma {k}"
            );
            assert!(salida.info().image_rect.is_none());
            let capas = salida.capas();
            assert_eq!(capas.len(), 1, "ppp {ppp}: solo BACKGROUND");
            let Capa::Relleno { color, .. } = &capas[0] else {
                panic!("el fondo es un rectángulo");
            };
            assert_eq!(*color, BACKGROUND);
            assert_eq!(
                banco.canvas.checker_texture_id(),
                None,
                "sin textura del damero"
            );
            assert_eq!(banco.canvas.texture_id(), None);
            // Con otro lado máximo de textura egui rehace el atlas de la fuente (`Managed(0)`),
            // pero no se sube ninguna textura del lienzo.
            assert!(
                salida
                    .subidas
                    .iter()
                    .all(|(id, _)| *id == TextureId::default()),
                "sin subidas del lienzo: {:?}",
                salida.subidas.iter().map(|(id, _)| id).collect::<Vec<_>>()
            );
        }
    }
}

#[test]
fn fila_6_el_documento_la_textura_de_la_imagen_y_el_state_id_quedan_intactos() {
    let ppp = 2.0;
    let imagen = imagen_transparente().con(5, 5, ROJO).con(6, 5, ROJO_MEDIO);
    let doc = Document::new(imagen.image());
    let id = doc.main_image();
    let state_id = doc.state_id();
    let bytes = to_rgba8(doc.image(id).expect("imagen"));
    let mut banco = Banco::sin_calentar(
        PANTALLA,
        ppp,
        imagen.clone(),
        CanvasBackground::Checker,
        false,
    );
    let primero = banco.corre(vec![], &[]);
    let textura = banco.canvas.texture_id().expect("textura de la imagen");
    let subida = primero.subidas_de(textura);
    assert_eq!(subida.len(), 1);
    let ImageData::Color(color) = &subida[0].image;
    // Lo que sube el lienzo es la imagen, intacta: los mismos bytes que `to_rgba8`.
    let esperado = egui::ColorImage::from_rgba_unmultiplied([64, 48], &bytes);
    assert_eq!(color.pixels, esperado.pixels);
    // Varios fotogramas con superposición y con cada fondo no tocan nada.
    for fondo in FONDOS {
        banco.canvas.set_background(fondo);
        for _ in 0..2 {
            let salida = banco.corre(vec![], &bloques());
            assert!(salida.subidas_de(textura).is_empty(), "{fondo:?}");
        }
    }
    assert_eq!(to_rgba8(doc.image(id).expect("imagen")), bytes);
    assert_eq!(doc.state_id(), state_id);
    assert!(!doc.can_undo());
}

// ---------------------------------------------------------------------------------------------
// Fila 7: la superposición con el damero
// ---------------------------------------------------------------------------------------------

/// Fila 7 (y 14 y 15 con otro fondo o camino): el banco de bloques de `overlay_small_zoom.rs`.
fn bloques_con(ppp: f32, zoom: u32, fondo: CanvasBackground, malla: bool) {
    let etiqueta = format!("ppp {ppp} ×{zoom} {fondo:?} malla={malla}");
    let mut banco = Banco::nuevo_con(ppp, imagen_azul(), fondo, malla);
    banco.con_zoom(zoom);
    let overlay = bloques();
    let escena = banco.escena(&overlay);
    let g = escena.geom;
    assert_eq!(
        g.z,
        i64::from(zoom),
        "{etiqueta}: el zoom se mide en píxeles físicos"
    );
    let i = comprueba_base(&banco, &escena.salida, fondo, malla, &etiqueta);
    let capas = escena.salida.capas();
    comprueba_superposicion(
        &banco,
        &g,
        &capas[i + 1..],
        &overlay,
        fondo,
        malla,
        &etiqueta,
    );
    // Teselado: con el damero por textura, como mucho dos mallas de superposición (todos los
    // cuadriláteros y todos los rectángulos de color).
    if fondo == CanvasBackground::Checker && !malla {
        let imagen = banco.canvas.texture_id().expect("textura");
        let en_la_imagen = escena
            .primitivas
            .iter()
            .position(|p| matches!(&p.primitive, Primitive::Mesh(m) if m.texture_id == imagen))
            .expect("malla de la imagen teselada");
        assert!(
            escena.primitivas.len() - en_la_imagen - 1 <= 2,
            "{etiqueta}: como mucho 2 mallas de superposición, hay {}",
            escena.primitivas.len() - en_la_imagen - 1
        );
    }
    // Lo que se ve en cada píxel físico (a ×1, también los de 1×1 píxel físico).
    let f = fallos(&escena, &banco.imagen, &overlay, fondo);
    sin_fallos(&f, &etiqueta);
}

#[test]
fn fila_7_la_superposicion_es_un_cuadrilatero_por_pixel_con_alfa_menor_que_255_y_se_ve_exacta() {
    for ppp in PPPS {
        for zoom in 1..=3 {
            bloques_con(ppp, zoom, CanvasBackground::Checker, false);
        }
    }
}

#[test]
fn fila_7_a_x1_se_ven_los_pixeles_de_1x1_pixel_fisico_con_cualquier_escala() {
    // El fallo de `b702d8e`: a ×1 la superposición de 1×1 píxel físico no se veía.
    for ppp in PPPS {
        let mut banco = Banco::nuevo(ppp, imagen_azul());
        banco.con_zoom(1);
        let overlay = bloques();
        let escena = banco.escena(&overlay);
        assert_eq!(escena.geom.z, 1);
        let f = fallos(&escena, &banco.imagen, &overlay, CanvasBackground::Checker);
        sin_fallos(&f, &format!("ppp {ppp} ×1"));
        // Y los 24 píxeles pendientes no se ven como la imagen azul que hay debajo.
        for &(x, y, color) in &overlay {
            let b = escena.geom.bloque(x, y);
            let visto = escena.buffer.en(b.x, b.y);
            let azul = Visto::de_color(Color32::from_rgb(0, 0, 255));
            if color.a < 255 {
                assert!(!visto.cerca(azul, TOLERANCIA), "({x}, {y}) con {color:?}");
            }
        }
    }
}

#[test]
fn fila_7_con_solo_pixeles_opacos_no_hay_cuadrilateros_en_la_superposicion() {
    for ppp in PPPS {
        let mut banco = Banco::nuevo(ppp, imagen_azul());
        let overlay: Vec<Pendiente> = (0..6).map(|x| (10 + x, 4, ROJO)).collect();
        let salida = banco.corre(vec![], &overlay);
        let capas = salida.capas();
        let damero = banco
            .canvas
            .checker_texture_id()
            .expect("textura del damero");
        let cuadrilateros = capas
            .iter()
            .filter(|c| matches!(c, Capa::Malla { textura, .. } if *textura == damero))
            .count();
        assert_eq!(
            cuadrilateros, 1,
            "ppp {ppp}: solo el damero de fondo de la imagen"
        );
        let i = comprueba_base(&banco, &salida, CanvasBackground::Checker, false, "opacos");
        let g = banco.geom(salida.info());
        comprueba_superposicion(
            &banco,
            &g,
            &capas[i + 1..],
            &overlay,
            CanvasBackground::Checker,
            false,
            "opacos",
        );
        assert_eq!(capas.len() - i - 1, 6, "un rectángulo de color por píxel");
    }
}

/// El trazo del borrador de la fila 10 de la 17a: veinte píxeles de alfa 0 sobre una imagen azul,
/// varios fotogramas, `End` (la imagen cambia) y `refresh`.
fn trazo_del_borrador(ppp: f32, fondo: CanvasBackground, malla: bool) {
    let etiqueta = format!("ppp {ppp} {fondo:?} malla={malla}");
    let mut banco = Banco::nuevo_con(ppp, imagen_azul(), fondo, malla);
    let texto_imagen = banco.canvas.texture_id().expect("textura de la imagen");
    let damero_antes = banco.canvas.checker_texture_id();
    let pixeles = los_veinte();
    let overlay_de = |n: usize| -> Vec<Pendiente> {
        pixeles[..n]
            .iter()
            .map(|&(x, y)| (x, y, TRANSPARENTE))
            .collect()
    };
    // Cada fotograma añade un píxel; la superposición no sube nada.
    for n in 1..=20 {
        let overlay = overlay_de(n);
        let salida = banco.corre(vec![], &overlay);
        assert!(
            salida.subidas.is_empty(),
            "{etiqueta}: {n} píxeles, sin subidas"
        );
        let i = comprueba_base(&banco, &salida, fondo, malla, &etiqueta);
        let g = banco.geom(salida.info());
        comprueba_superposicion(
            &banco,
            &g,
            &salida.capas()[i + 1..],
            &overlay,
            fondo,
            malla,
            &etiqueta,
        );
    }
    // Varios fotogramas más con los veinte, sin cambios.
    let overlay = overlay_de(20);
    for _ in 0..3 {
        let salida = banco.corre(vec![], &overlay);
        assert!(salida.subidas.is_empty());
    }
    let con_superposicion = banco.escena(&overlay);
    let f = fallos(&con_superposicion, &banco.imagen, &overlay, fondo);
    sin_fallos(&f, &format!("{etiqueta}: con la superposición"));

    // `End` y `refresh`: la imagen con esos veinte píxeles de alfa 0 y sin superposición.
    let mut borrada = imagen_azul();
    for &(x, y) in &pixeles {
        borrada = borrada.con(x, y, TRANSPARENTE);
    }
    banco.refresca(borrada);
    let tras_soltar = banco.escena(&[]);
    assert_eq!(
        tras_soltar.salida.subidas.len(),
        1,
        "{etiqueta}: una sola subida tras el refresh"
    );
    assert_eq!(tras_soltar.salida.subidas_de(texto_imagen).len(), 1);
    assert_eq!(banco.canvas.checker_texture_id(), damero_antes);
    let f = fallos(&tras_soltar, &banco.imagen, &[], fondo);
    sin_fallos(&f, &format!("{etiqueta}: tras soltar"));

    // Antes y después de soltar, igual: cada píxel físico de los veinte bloques.
    let (a, d) = (con_superposicion.geom, tras_soltar.geom);
    assert_eq!(a.origen, d.origen);
    for &(x, y) in &pixeles {
        let b = a.bloque(x, y);
        for py in b.y..b.y + b.height {
            for px in b.x..b.x + b.width {
                let antes = con_superposicion.buffer.en(px, py);
                let despues = tras_soltar.buffer.en(px, py);
                assert!(
                    antes.cerca(despues, TOLERANCIA),
                    "{etiqueta}: el píxel físico ({px}, {py}) de ({x}, {y}) se veía {antes:?} con la superposición y {despues:?} tras soltar"
                );
            }
        }
    }
}

#[test]
fn fila_7_el_trazo_del_borrador_se_ve_igual_antes_y_despues_de_soltar() {
    for ppp in PPPS {
        trazo_del_borrador(ppp, CanvasBackground::Checker, false);
    }
}

// ---------------------------------------------------------------------------------------------
// Fila 8: límites
// ---------------------------------------------------------------------------------------------

#[test]
fn fila_8_una_imagen_de_1x1_a_x64_es_un_cuadrilatero_de_64x64_con_uv_0_0_4_4_a_cell_8() {
    for ppp in PPPS {
        let mut banco = Banco::nuevo(ppp, Imagen::uniforme(1, 1, TRANSPARENTE));
        let escena = banco.escena(&[]);
        let etiqueta = format!("ppp {ppp} 1×1");
        let g = escena.geom;
        assert_eq!(g.z, 64, "{etiqueta}: el ajuste da ×64");
        assert_eq!((g.ancho, g.alto), (64, 64));
        let i = comprueba_base(
            &banco,
            &escena.salida,
            CanvasBackground::Checker,
            false,
            &etiqueta,
        );
        let Capa::Malla { vertices, .. } = &escena.salida.capas()[i - 1] else {
            panic!("{etiqueta}: el damero es una malla");
        };
        let caja_pos = caja(vertices);
        assert!(
            casi(caja_pos, rect_en_puntos(g.rect(), ppp)),
            "{etiqueta}: 64×64 píxeles físicos: {caja_pos:?}"
        );
        let uv = caja_uv(vertices);
        if g.cell == 8 {
            assert_eq!(uv, [0.0, 0.0, 4.0, 4.0], "{etiqueta}: a cell 8");
        }
        let periodos = 64.0 / (2.0 * g.cell as f32);
        assert!(
            (uv[2] - periodos).abs() < 1e-5 && (uv[3] - periodos).abs() < 1e-5,
            "{etiqueta}: {uv:?}"
        );
        assert_eq!(
            (uv[0], uv[1]),
            (0.0, 0.0),
            "{etiqueta}: con el origen en la esquina"
        );
        let f = fallos(&escena, &banco.imagen, &[], CanvasBackground::Checker);
        sin_fallos(&f, &etiqueta);
    }
}

#[test]
fn fila_8_con_la_vista_en_el_extremo_el_cuadrilatero_es_solo_la_franja_visible_y_se_ve_bien() {
    for ppp in PPPS {
        for zoom in 1..=3 {
            for (sx, sy) in [(-1, -1), (1, 1), (-1, 1), (1, -1)] {
                let etiqueta = format!("ppp {ppp} ×{zoom} extremo ({sx}, {sy})");
                let mut banco = Banco::nuevo(ppp, imagen_transparente());
                banco.con_zoom(zoom);
                banco.arrastra_al_extremo(sx, sy);
                let escena = banco.escena(&[]);
                let g = escena.geom;
                let vp = banco.viewport();
                // Quedan visibles `MIN_VISIBLE` píxeles por eje (View::clamp), como mucho.
                let visible = |origen: i64, largo: i64, lienzo: u32| {
                    let desde = origen.max(0);
                    let hasta = (origen + largo).min(i64::from(lienzo));
                    hasta - desde
                };
                assert_eq!(
                    visible(g.origen.0, g.ancho, vp.width),
                    i64::from(MIN_VISIBLE),
                    "{etiqueta}: visibles en x"
                );
                assert_eq!(
                    visible(g.origen.1, g.alto, vp.height),
                    i64::from(MIN_VISIBLE),
                    "{etiqueta}: visibles en y"
                );
                let i = comprueba_base(
                    &banco,
                    &escena.salida,
                    CanvasBackground::Checker,
                    false,
                    &etiqueta,
                );
                let Capa::Malla { vertices, .. } = &escena.salida.capas()[i - 1] else {
                    panic!("{etiqueta}: el damero es una malla");
                };
                let franja = g.recorte().expect("hay franja visible");
                assert_eq!(
                    (franja.width, franja.height),
                    (i64::from(MIN_VISIBLE), i64::from(MIN_VISIBLE)),
                    "{etiqueta}: la franja visible"
                );
                // Dentro del lienzo, y solo la franja.
                let lienzo = Rect::from_min_size(Pos2::ZERO, banco.pantalla);
                let caja_pos = caja(vertices);
                assert!(
                    caja_pos.min.x >= lienzo.min.x - 1e-3
                        && caja_pos.min.y >= lienzo.min.y - 1e-3
                        && caja_pos.max.x <= lienzo.max.x + 1e-3
                        && caja_pos.max.y <= lienzo.max.y + 1e-3,
                    "{etiqueta}: {caja_pos:?}"
                );
                assert!(
                    casi(caja_pos, rect_en_puntos(franja, ppp)),
                    "{etiqueta}: {caja_pos:?}"
                );
                // El uv parte de la fase correcta y cada píxel se ve como `shade_at`.
                let f = fallos(&escena, &banco.imagen, &[], CanvasBackground::Checker);
                sin_fallos(&f, &etiqueta);
                for py in franja.y..franja.y + franja.height {
                    for px in franja.x..franja.x + franja.width {
                        let esperado = gris(shade_at(px, py, g.origen, g.cell).color());
                        assert!(
                            escena
                                .buffer
                                .en(px, py)
                                .cerca(Visto::de_color(esperado), TOLERANCIA),
                            "{etiqueta}: ({px}, {py})"
                        );
                    }
                }
            }
        }
    }
}

#[test]
fn fila_8_con_un_lienzo_de_1x1_pixel_fisico_no_hay_panico_y_el_recorte_es_none_o_1x1() {
    for ppp in PPPS {
        let pantalla = vec2(1.0 / ppp, 1.0 / ppp);
        let mut banco = Banco::sin_calentar(
            pantalla,
            ppp,
            imagen_transparente(),
            CanvasBackground::Checker,
            false,
        );
        assert_eq!(banco.viewport(), Size::new(1, 1), "ppp {ppp}");
        for _ in 0..3 {
            banco.corre(vec![], &[]);
        }
        let escena = banco.escena(&[]);
        let etiqueta = format!("ppp {ppp} lienzo de 1×1");
        let recorte = escena.geom.recorte();
        assert!(
            matches!(
                recorte,
                None | Some(ScreenRect {
                    width: 1,
                    height: 1,
                    ..
                })
            ),
            "{etiqueta}: {recorte:?}"
        );
        comprueba_base(
            &banco,
            &escena.salida,
            CanvasBackground::Checker,
            false,
            &etiqueta,
        );
        let f = fallos(&escena, &banco.imagen, &[], CanvasBackground::Checker);
        sin_fallos(&f, &etiqueta);
        // Y se puede arrastrar y hacer zoom sin pánico.
        banco.canvas.request_view(ViewOp::ZoomIn);
        banco.corre(vec![mover(pos2(0.2 / ppp, 0.2 / ppp))], &[]);
        banco.corre(
            vec![rueda_linea(true), tecla(Key::Home, Modifiers::NONE)],
            &[],
        );
    }
}

#[test]
fn fila_8_sin_imagen_solo_se_pinta_el_fondo_sin_fallo_ni_textura_del_damero() {
    for ppp in PPPS {
        let mut banco = Banco {
            ctx: contexto(ppp),
            ppp,
            pantalla: PANTALLA,
            canvas: Canvas::new(),
            imagen: Imagen::uniforme(1, 1, TRANSPARENTE),
        };
        for k in 0..3 {
            let salida = banco.corre(
                vec![rueda_linea(true), tecla(Key::Home, Modifiers::NONE)],
                &bloques(),
            );
            let capas = salida.capas();
            assert_eq!(capas.len(), 1, "ppp {ppp} fotograma {k}: solo el fondo");
            assert!(matches!(&capas[0], Capa::Relleno { color, .. } if *color == BACKGROUND));
            assert_eq!(banco.canvas.checker_texture_id(), None);
            assert_eq!(banco.canvas.texture_id(), None);
            assert!(salida.subidas.is_empty(), "sin subidas");
            assert!(salida.info().image_rect.is_none());
            assert!(salida.info().error.is_none());
        }
    }
}

#[test]
fn fila_8_con_la_textura_maxima_de_1024_y_una_imagen_de_1025_de_ancho_solo_se_pinta_el_fondo() {
    let mut banco = Banco::sin_calentar(
        PANTALLA,
        1.0,
        Imagen::uniforme(1025, 2, TRANSPARENTE),
        CanvasBackground::Checker,
        false,
    );
    for _ in 0..2 {
        let salida = banco.corre_max(vec![], &bloques(), Some(1024));
        assert!(matches!(
            salida.info().error,
            Some(CanvasError::TooLarge { max: 1024, .. })
        ));
        assert_eq!(salida.capas().len(), 1);
        assert_eq!(banco.canvas.checker_texture_id(), None);
        assert!(
            salida
                .subidas
                .iter()
                .all(|(id, _)| *id == TextureId::default())
        );
    }
}

// ---------------------------------------------------------------------------------------------
// Fila 14: la superposición con cada fondo
// ---------------------------------------------------------------------------------------------

#[test]
fn fila_14_con_un_fondo_solido_la_superposicion_es_una_sola_pasada_y_se_ve_exacta() {
    for fondo in SOLIDOS {
        for ppp in PPPS {
            for zoom in 1..=3 {
                bloques_con(ppp, zoom, fondo, false);
            }
        }
    }
}

#[test]
fn fila_14_con_un_fondo_solido_no_se_crea_la_textura_del_damero() {
    for fondo in SOLIDOS {
        let mut banco = Banco::sin_calentar(PANTALLA, 1.0, imagen_azul(), fondo, false);
        for _ in 0..3 {
            let salida = banco.corre(vec![], &bloques());
            assert_eq!(banco.canvas.checker_texture_id(), None, "{fondo:?}");
            let imagen = banco.canvas.texture_id().expect("textura de la imagen");
            assert_eq!(
                salida.subidas.len(),
                salida.subidas_de(imagen).len(),
                "{fondo:?}: solo se sube la imagen"
            );
        }
        assert_eq!(banco.canvas.background(), fondo);
    }
}

#[test]
fn fila_14_el_trazo_del_borrador_se_ve_igual_antes_y_despues_de_soltar_con_cada_solido() {
    for fondo in SOLIDOS {
        for ppp in PPPS {
            trazo_del_borrador(ppp, fondo, false);
        }
    }
}

/// Un banco con documento y herramienta, como la app: el trazo del borrador sobre una imagen azul.
struct Trazo {
    banco: Banco,
    doc: Document,
    tool: ToolState,
    /// Rectángulo de la imagen y zoom del primer fotograma.
    rect: Rect,
    zoom: i64,
}

impl Trazo {
    fn nuevo(ppp: f32) -> Trazo {
        let banco = Banco::sin_calentar(
            vec2(800.0, 600.0),
            ppp,
            imagen_azul(),
            CanvasBackground::Checker,
            false,
        );
        let doc = Document::new(banco.imagen.image());
        let mut tool = ToolState::new();
        tool.apply_action(ToolAction::Select(Tool::Eraser));
        assert_eq!(tool.tool(), Tool::Eraser);
        let mut trazo = Trazo {
            banco,
            doc,
            tool,
            rect: Rect::NOTHING,
            zoom: 0,
        };
        trazo.fotograma(vec![]);
        let salida = trazo.fotograma(vec![]);
        trazo.rect = salida.info().image_rect.expect("hay imagen");
        trazo.zoom = salida.info().zoom.scale(1);
        trazo
    }

    /// Un fotograma como el de la app: eventos del lienzo a la herramienta, `refresh` si el
    /// documento cambió y `paint` con la superposición de `ToolState::preview`.
    fn fotograma(&mut self, events: Vec<Event>) -> Salida {
        let doc = &mut self.doc;
        let tool = &mut self.tool;
        let id = doc.main_image();
        self.banco.corre_con(events, None, |canvas, ui| {
            let mut cambio = false;
            for entrada in canvas.interact_ordered(ui) {
                cambio |= tool
                    .canvas_input_with(doc, id, None, entrada)
                    .expect("el borrador no falla");
            }
            let image = doc.image(id).expect("hay imagen");
            if cambio {
                canvas.refresh(ui.ctx(), image);
            }
            let overlay: Vec<Pendiente> = tool
                .preview()
                .filter_map(|e| Some((e.x, e.y, pixel_rgba(image, e.pixel)?)))
                .collect();
            let info = canvas.paint(ui, overlay);
            (vec![], Some(info))
        })
    }

    fn centro(&self, x: i32, y: i32) -> Pos2 {
        let z = self.zoom as f32;
        pos2(
            self.rect.min.x + (x as f32 + 0.5) * z,
            self.rect.min.y + (y as f32 + 0.5) * z,
        )
    }

    fn pendientes(&self) -> Vec<Pendiente> {
        let image = self.doc.image(self.doc.main_image()).expect("imagen");
        self.tool
            .preview()
            .filter_map(|e| Some((e.x, e.y, pixel_rgba(image, e.pixel)?)))
            .collect()
    }
}

#[test]
fn fila_14_cambiar_de_fondo_a_mitad_de_trazo_cambia_el_siguiente_fotograma_sin_tocar_nada() {
    let mut t = Trazo::nuevo(1.0);
    let state_id = t.doc.state_id();
    let bytes = to_rgba8(t.doc.image(t.doc.main_image()).expect("imagen"));
    let mover_a = |x| mover(x);
    let inicio = t.centro(10, 5);
    t.fotograma(vec![mover_a(inicio)]);
    t.fotograma(vec![boton_de(inicio, PointerButton::Primary, true)]);
    for x in 11..=14 {
        t.fotograma(vec![mover_a(t.centro(x, 5))]);
    }
    assert!(t.tool.busy(), "el trazo está en curso");
    let n_pendientes = t.pendientes().len();
    assert_eq!(n_pendientes, 5);
    let damero = t
        .banco
        .canvas
        .checker_texture_id()
        .expect("textura del damero");

    // Con el damero: un cuadrilátero por píxel pendiente.
    let salida = t.fotograma(vec![]);
    let i = comprueba_base(
        &t.banco,
        &salida,
        CanvasBackground::Checker,
        false,
        "damero",
    );
    let g = t.banco.geom(salida.info());
    comprueba_superposicion(
        &t.banco,
        &g,
        &salida.capas()[i + 1..],
        &t.pendientes(),
        CanvasBackground::Checker,
        false,
        "damero",
    );

    // Cambia el fondo a mitad de trazo: el siguiente fotograma ya es el sólido.
    for fondo in [
        CanvasBackground::Black,
        CanvasBackground::White,
        CanvasBackground::Gray,
    ] {
        t.banco.canvas.set_background(fondo);
        assert_eq!(t.banco.canvas.background(), fondo);
        let salida = t.fotograma(vec![]);
        let i = comprueba_base_con_damero_existente(&t.banco, &salida, fondo);
        let g = t.banco.geom(salida.info());
        comprueba_superposicion(
            &t.banco,
            &g,
            &salida.capas()[i + 1..],
            &t.pendientes(),
            fondo,
            false,
            &format!("{fondo:?} a mitad de trazo"),
        );
        // El trazo sigue: el documento, la herramienta y la superposición, como estaban.
        assert!(t.tool.busy());
        assert_eq!(t.tool.tool(), Tool::Eraser);
        assert_eq!(t.doc.state_id(), state_id);
        assert!(!t.doc.can_undo());
        assert_eq!(t.pendientes().len(), n_pendientes);
        assert_eq!(
            to_rgba8(t.doc.image(t.doc.main_image()).expect("imagen")),
            bytes
        );
    }

    // El trazo sigue con el fondo nuevo y, de vuelta a `Checker`, la textura es la misma.
    t.fotograma(vec![mover_a(t.centro(15, 5))]);
    t.banco.canvas.set_background(CanvasBackground::Checker);
    let salida = t.fotograma(vec![mover_a(t.centro(16, 5))]);
    assert_eq!(t.banco.canvas.checker_texture_id(), Some(damero));
    assert!(salida.subidas_de(damero).is_empty(), "no se vuelve a subir");
    let i = comprueba_base(
        &t.banco,
        &salida,
        CanvasBackground::Checker,
        false,
        "de vuelta",
    );
    let g = t.banco.geom(salida.info());
    comprueba_superposicion(
        &t.banco,
        &g,
        &salida.capas()[i + 1..],
        &t.pendientes(),
        CanvasBackground::Checker,
        false,
        "de vuelta",
    );
    assert_eq!(t.pendientes().len(), 7);
    assert_eq!(t.doc.state_id(), state_id);
}

/// Como `comprueba_base` para un sólido cuando la textura del damero ya existe de antes (no se
/// vuelve a crear, pero tampoco se usa): sin la comprobación de que no existe.
fn comprueba_base_con_damero_existente(
    banco: &Banco,
    salida: &Salida,
    fondo: CanvasBackground,
) -> usize {
    let capas = salida.capas();
    let g = banco.geom(salida.info());
    let imagen = banco.canvas.texture_id().expect("textura de la imagen");
    let damero = banco
        .canvas
        .checker_texture_id()
        .expect("la textura sigue creada");
    assert!(
        !capas
            .iter()
            .any(|c| matches!(c, Capa::Malla { textura, .. } if *textura == damero)),
        "{fondo:?}: ningún cuadrilátero de la textura del damero con un fondo sólido"
    );
    let area = g.recorte().expect("recorte");
    let Capa::Relleno { rect, color } = &capas[1] else {
        panic!(
            "{fondo:?}: rectángulo del fondo sólido y hay {:?}",
            capas[1]
        );
    };
    assert_eq!(*color, fondo_en(fondo, 0, 0, (0, 0), 8));
    assert!(casi(*rect, rect_en_puntos(area, g.ppp)));
    assert!(matches!(&capas[2], Capa::Malla { textura, .. } if *textura == imagen));
    2
}

#[test]
fn fila_14_volver_a_checker_crea_la_textura_del_damero_una_sola_vez() {
    let mut banco =
        Banco::sin_calentar(PANTALLA, 1.0, imagen_azul(), CanvasBackground::Black, false);
    for k in 0..3 {
        let salida = banco.corre(vec![], &bloques());
        assert_eq!(banco.canvas.checker_texture_id(), None);
        assert_eq!(
            salida.subidas.len(),
            usize::from(k == 0),
            "solo la imagen, una vez"
        );
    }
    banco.canvas.set_background(CanvasBackground::Checker);
    let salida = banco.corre(vec![], &bloques());
    let damero = banco
        .canvas
        .checker_texture_id()
        .expect("se crea al volver");
    assert_eq!(salida.subidas_de(damero).len(), 1, "una sola subida");
    // Otros cambios de fondo y vuelta: ni nueva subida ni otro id.
    for fondo in [
        CanvasBackground::White,
        CanvasBackground::Checker,
        CanvasBackground::Gray,
        CanvasBackground::Checker,
        CanvasBackground::Black,
        CanvasBackground::Checker,
    ] {
        banco.canvas.set_background(fondo);
        let salida = banco.corre(vec![], &bloques());
        assert!(salida.subidas.is_empty(), "{fondo:?}: sin subidas");
        assert_eq!(banco.canvas.checker_texture_id(), Some(damero), "{fondo:?}");
    }
}

#[test]
fn fila_14_el_fondo_por_defecto_es_el_damero_y_el_camino_por_textura() {
    let canvas = Canvas::new();
    assert_eq!(canvas.background(), CanvasBackground::Checker);
    assert!(!canvas.checker_mesh());
    assert_eq!(canvas.checker_texture_id(), None);
}

// ---------------------------------------------------------------------------------------------
// Fila 15: respaldo por malla
// ---------------------------------------------------------------------------------------------

#[test]
fn fila_15_en_modo_malla_la_imagen_transparente_se_ve_igual_que_con_la_textura() {
    for ppp in PPPS {
        for zoom in 1..=3 {
            let (banco, escena) = fila_5_escena(ppp, zoom, true);
            assert!(banco.canvas.checker_mesh());
            assert_eq!(
                banco.canvas.checker_texture_id(),
                None,
                "sin textura del damero"
            );
            let imagen = banco.canvas.texture_id().expect("textura de la imagen");
            assert_eq!(
                escena.salida.subidas.len(),
                0,
                "ya se subió en el calentamiento"
            );
            let capas = escena.salida.capas();
            let i = comprueba_base(
                &banco,
                &escena.salida,
                CanvasBackground::Checker,
                true,
                &format!("ppp {ppp} ×{zoom}"),
            );
            // El fondo y las casillas oscuras: un rectángulo CHECK_A y UNA sola malla de colores de
            // vértice (sin la textura de la imagen).
            assert_eq!(i, 3, "BACKGROUND, CHECK_A, una malla de casillas B, imagen");
            assert!(matches!(&capas[1], Capa::Relleno { .. }));
            assert!(matches!(&capas[2], Capa::Malla { textura, .. } if *textura != imagen));
        }
    }
}

#[test]
fn fila_15_en_modo_malla_tras_arrastrar_el_damero_va_con_la_imagen() {
    for ppp in PPPS {
        for zoom in 1..=3 {
            fila_5_tras_arrastrar(ppp, zoom, &[(7, 13)], true);
        }
    }
}

#[test]
fn fila_15_en_modo_malla_la_superposicion_se_ve_exacta_y_no_se_sube_ningun_damero() {
    for ppp in PPPS {
        for zoom in 1..=3 {
            bloques_con(ppp, zoom, CanvasBackground::Checker, true);
        }
    }
}

#[test]
fn fila_15_en_modo_malla_el_trazo_del_borrador_se_ve_igual_antes_y_despues_de_soltar() {
    for ppp in PPPS {
        trazo_del_borrador(ppp, CanvasBackground::Checker, true);
    }
}

#[test]
fn fila_15_en_modo_malla_no_hay_textura_del_damero_ni_subida_en_ningun_fotograma() {
    let mut banco = Banco::sin_calentar(
        PANTALLA,
        1.0,
        imagen_transparente(),
        CanvasBackground::Checker,
        true,
    );
    let primero = banco.corre(vec![], &bloques());
    assert_eq!(banco.canvas.checker_texture_id(), None);
    // Solo la imagen se sube.
    let textura = banco.canvas.texture_id().expect("textura de la imagen");
    assert_eq!(primero.subidas.len(), 1);
    assert_eq!(primero.subidas_de(textura).len(), 1);
    for _ in 0..3 {
        let f = banco.corre(vec![], &bloques());
        assert!(f.subidas.is_empty());
        assert_eq!(banco.canvas.checker_texture_id(), None);
    }
}

#[test]
fn fila_15_al_desactivar_el_modo_malla_vuelve_el_camino_de_la_textura() {
    let mut banco = Banco::nuevo_con(1.0, imagen_transparente(), CanvasBackground::Checker, true);
    assert_eq!(banco.canvas.checker_texture_id(), None);
    // Desactivar: la textura se crea (una subida) y el fondo vuelve a ser el cuadrilátero.
    banco.canvas.set_checker_mesh(false);
    assert!(!banco.canvas.checker_mesh());
    let salida = banco.corre(vec![], &[]);
    let damero = banco
        .canvas
        .checker_texture_id()
        .expect("vuelve la textura");
    assert_eq!(salida.subidas_de(damero).len(), 1, "una subida");
    comprueba_reposo(&banco, &salida, "tras desactivar");
    // Activarlo de nuevo no crea otra textura ni sube nada; el camino de la malla vuelve.
    banco.canvas.set_checker_mesh(true);
    let salida = banco.corre(vec![], &[]);
    assert!(salida.subidas.is_empty());
    let i = comprueba_base(
        &banco,
        &salida,
        CanvasBackground::Checker,
        true,
        "reactivado",
    );
    assert_eq!(i, 3);
    // Y a la textura otra vez, con el mismo id y sin subida.
    banco.canvas.set_checker_mesh(false);
    let salida = banco.corre(vec![], &[]);
    assert!(salida.subidas.is_empty(), "no se vuelve a subir");
    assert_eq!(banco.canvas.checker_texture_id(), Some(damero));
    comprueba_reposo(&banco, &salida, "desactivado otra vez");
}

// ---------------------------------------------------------------------------------------------
// Propiedad 2: pintar el damero no cambia la conversión pantalla-imagen
// ---------------------------------------------------------------------------------------------

#[derive(Clone, Copy, Debug)]
enum Op {
    /// Mueve el puntero a una posición del lienzo, en milésimas de su ancho y alto.
    Mover(u32, u32),
    Rueda(bool),
    Mas,
    Menos,
    Inicio,
    /// Pulsa un botón (el central desplaza, el izquierdo dibuja) si no hay ninguno abajo.
    Pulsar(bool),
    Soltar,
}

fn una_op() -> impl Strategy<Value = Op> {
    prop_oneof![
        3 => (0u32..1000, 0u32..1000).prop_map(|(x, y)| Op::Mover(x, y)),
        2 => any::<bool>().prop_map(Op::Rueda),
        1 => Just(Op::Mas),
        1 => Just(Op::Menos),
        1 => Just(Op::Inicio),
        2 => any::<bool>().prop_map(Op::Pulsar),
        2 => Just(Op::Soltar),
    ]
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(24))]

    /// Secuencias aleatorias de rueda, arrastre, `+`, `-`, `Home` y movimientos del puntero: con
    /// `interact` y `paint` frente a solo `interact`, el zoom y los eventos de trazo son idénticos;
    /// `hover`, `zoom` e `image_rect` de `CanvasInfo` no dependen del fondo ni del camino de pintado,
    /// y `hover` coincide con `floor((puntero − image_rect.min)·ppp / zoom)`.
    #[test]
    fn propiedad_2_pintar_no_cambia_la_conversion_pantalla_imagen(
        ppp_i in 0usize..5,
        ops in prop::collection::vec(una_op(), 1..30),
        fondo_i in 0usize..4,
        malla in any::<bool>(),
    ) {
        let ppp = PPPS[ppp_i];
        let imagen = imagen_azul();
        let mut pintando = Banco::sin_calentar(PANTALLA, ppp, imagen.clone(), CanvasBackground::Checker, false);
        let mut solo = Banco::sin_calentar(PANTALLA, ppp, imagen.clone(), CanvasBackground::Checker, false);
        let mut otro = Banco::sin_calentar(PANTALLA, ppp, imagen, FONDOS[fondo_i], malla);
        let vp = pintando.viewport();
        let mut pos = en_puntos(ppp, i64::from(vp.width) / 2, i64::from(vp.height) / 2);
        let mut boton: Option<PointerButton> = None;
        let mut secuencia = vec![vec![mover(pos)]];
        for op in ops {
            secuencia.push(match op {
                Op::Mover(fx, fy) => {
                    let x = i64::from(fx) * i64::from(vp.width) / 1000;
                    let y = i64::from(fy) * i64::from(vp.height) / 1000;
                    pos = en_puntos(ppp, x, y);
                    vec![mover(pos)]
                }
                Op::Rueda(arriba) => vec![rueda_linea(arriba)],
                Op::Mas => vec![tecla(Key::Plus, Modifiers::NONE)],
                Op::Menos => vec![tecla(Key::Minus, Modifiers::NONE)],
                Op::Inicio => vec![tecla(Key::Home, Modifiers::NONE)],
                Op::Pulsar(central) if boton.is_none() => {
                    let b = if central { PointerButton::Middle } else { PointerButton::Primary };
                    boton = Some(b);
                    vec![boton_de(pos, b, true)]
                }
                Op::Pulsar(_) => vec![],
                Op::Soltar => match boton.take() {
                    Some(b) => vec![boton_de(pos, b, false)],
                    None => vec![],
                },
            });
        }
        let mut pos_actual: Option<Pos2> = None;
        for (k, events) in secuencia.into_iter().enumerate() {
            for e in &events {
                if let Event::PointerMoved(p) = e {
                    pos_actual = Some(*p);
                }
            }
            let con = pintando.corre(events.clone(), &[]);
            let sin = solo.solo_interact(events.clone());
            let otra = otro.corre(events, &[]);
            prop_assert_eq!(&con.trazo, &sin.trazo, "fotograma {}: eventos de trazo", k);
            prop_assert_eq!(&con.trazo, &otra.trazo, "fotograma {}: eventos de trazo (otro fondo)", k);
            prop_assert_eq!(pintando.canvas.zoom(), solo.canvas.zoom(), "fotograma {}: zoom", k);
            prop_assert_eq!(
                con.info.as_ref(),
                otra.info.as_ref(),
                "fotograma {}: hover, zoom e image_rect no dependen del fondo", k
            );
            // `hover` es `floor((puntero − image_rect.min)·ppp / zoom)`.
            let info = con.info();
            let rect = info.image_rect.expect("hay imagen");
            let z = info.zoom.scale(1);
            let esperado = pos_actual.and_then(|p| {
                let fx = ((p.x - rect.min.x) * ppp).round() as i64;
                let fy = ((p.y - rect.min.y) * ppp).round() as i64;
                let (ix, iy) = (fx.div_euclid(z), fy.div_euclid(z));
                (fx >= 0 && fy >= 0 && ix < 64 && iy < 48).then_some((ix as u32, iy as u32))
            });
            prop_assert_eq!(info.hover, esperado, "fotograma {}: hover", k);
            prop_assert_eq!(Some(info.zoom), pintando.canvas.zoom());
        }
    }
}

// ---------------------------------------------------------------------------------------------
// Propiedad 3: lo que se ve es el oráculo puro
// ---------------------------------------------------------------------------------------------

#[derive(Clone, Debug)]
struct CasoVisto {
    imagen: Imagen,
    overlay: Vec<Pendiente>,
    fondo: CanvasBackground,
    malla: bool,
    ppp: f32,
    zoom: u32,
    desplazo: (i64, i64),
}

impl std::fmt::Debug for Imagen {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Imagen {}×{} {:?}", self.ancho, self.alto, self.pixeles)
    }
}

/// Un color con alfa 0, de 1 a 254 o 255 al azar.
fn color() -> impl Strategy<Value = Rgba> {
    (0u8..3, any::<u8>(), any::<u8>(), any::<u8>(), 1u8..=254).prop_map(|(clase, r, g, b, a)| {
        let alfa = match clase {
            0 => 0,
            1 => a,
            _ => 255,
        };
        Rgba::new(r, g, b, alfa)
    })
}

fn caso_visto() -> impl Strategy<Value = CasoVisto> {
    (1u32..=24, 1u32..=24)
        .prop_flat_map(|(ancho, alto)| {
            (
                Just((ancho, alto)),
                prop::collection::vec(color(), (ancho * alto) as usize),
                prop::collection::vec((any::<u32>(), color()), 0..=12),
                0usize..4,
                any::<bool>(),
                0usize..5,
                1u32..=3,
                (-80i64..=80, -80i64..=80),
            )
        })
        .prop_map(
            |((ancho, alto), pixeles, pendientes, fondo, malla, ppp, zoom, desplazo)| {
                let mut vistos = std::collections::HashSet::new();
                let overlay = pendientes
                    .into_iter()
                    .filter_map(|(semilla, color)| {
                        let i = semilla % (ancho * alto);
                        vistos.insert(i).then_some((i % ancho, i / ancho, color))
                    })
                    .collect();
                CasoVisto {
                    imagen: Imagen {
                        ancho,
                        alto,
                        pixeles,
                    },
                    overlay,
                    fondo: FONDOS[fondo],
                    malla,
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
    /// `composición(fondo en ese píxel, píxel de la imagen o de la superposición)` y el resto del
    /// lienzo, `BACKGROUND`: con uno de los cuatro fondos, con la textura o con el respaldo por
    /// malla, a cualquiera de las cinco escalas, a ×1, ×2 o ×3 y con la vista desplazada.
    #[test]
    fn propiedad_3_lo_que_se_ve_es_el_oraculo_puro(c in caso_visto()) {
        let mut banco = Banco::nuevo_con(c.ppp, c.imagen.clone(), c.fondo, c.malla);
        banco.con_zoom(c.zoom);
        banco.arrastra(c.desplazo.0, c.desplazo.1);
        let escena = banco.escena(&c.overlay);
        let f = fallos(&escena, &c.imagen, &c.overlay, c.fondo);
        prop_assert!(
            f.vacio(),
            "ppp {} ×{} {:?} malla={} desplazo {:?}: {}",
            c.ppp, c.zoom, c.fondo, c.malla, c.desplazo, f.resumen()
        );
        // Y las capas del fondo de la imagen según el fondo y el camino.
        let i = comprueba_base(&banco, &escena.salida, c.fondo, c.malla, "propiedad 3");
        let capas = escena.salida.capas();
        comprueba_superposicion(
            &banco,
            &escena.geom,
            &capas[i + 1..],
            &c.overlay,
            c.fondo,
            c.malla,
            "propiedad 3",
        );
    }
}
