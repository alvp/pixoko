//! La superposición del trazo con el píxel de imagen de menos de 2 píxeles físicos (fallo de la
//! prueba manual de la unidad 17: a zoom ×1 la vista previa del lápiz, la línea y el borrador no se
//! ve mientras se arrastra).
//!
//! El banco pinta un lienzo con una imagen azul opaca y una superposición (`Canvas::paint`), pasa
//! las formas del fotograma por el teselador de egui (`Context::tessellate`, con los mismos
//! ajustes que la app, suavizado de bordes incluido) y rasteriza **a mano** lo teselado en el centro
//! de cada píxel físico del bloque de cada píxel pendiente: el color que se vería. Se compara con el
//! color que debe verse (el de la superposición sobre la imagen, o sobre el fondo si su alfa es
//! menor que 255, que es lo que sustituye al píxel al soltar). Se hace a las cinco escalas y con
//! zoom ×1, ×2 y ×3. El zoom de la vista se mide en **píxeles físicos** (`View` trabaja en ellos),
//! así que a ×1 un píxel de imagen mide 1 píxel físico con cualquier `ppp` (1,0, 1,25, 1,5, 2,0 y
//! 3,0), y a ×2 y ×3, 2 y 3.
//!
//! Causa medida del fallo (`tessellate` sobre un bloque de 1×1 píxel físico): el suavizado de
//! bordes de egui estrecha medio píxel cada lado del relleno y ensancha otro medio la rampa, y con
//! un rectángulo de 1 px todos los vértices acaban en el mismo punto: los triángulos son
//! degenerados y no se pinta ningún píxel.
//!
//! Sin ventana: `Context::run_ui` con `RawInput` sintético, sin GPU ni capturas.

use egui::epaint::{ClippedPrimitive, Primitive, Vertex};
use egui::{Color32, Pos2, Rect, TextureId, pos2};
use px_core::{Image, Mode, Pixel, Rgba};
use px_ui::canvas::{BACKGROUND, Canvas, CanvasInfo};
use px_ui::view::ViewOp;

mod common;
use common::{PPPS, contexto, fotograma};

const PANTALLA: egui::Vec2 = egui::vec2(400.0, 300.0);
const AZUL: Color32 = Color32::from_rgb(0, 0, 255);

fn imagen_azul() -> Image {
    let mut image = Image::new(64, 48, Mode::Rgba).expect("imagen válida");
    for y in 0..48 {
        for x in 0..64 {
            image
                .set_pixel(x, y, Pixel::Rgba(Rgba::opaque(0, 0, 255)))
                .expect("dentro");
        }
    }
    image
}

/// Un píxel pendiente: posición y color (el que tendrá en la imagen).
type Pendiente = (u32, u32, Rgba);

/// Superposición de prueba: un bloque de 4×2 píxeles por cada caso.
fn superposicion() -> Vec<Pendiente> {
    let mut v = Vec::new();
    for (i, color) in [
        Rgba::opaque(255, 0, 0),
        // El borrador: alfa 0 sobre el fondo.
        Rgba::new(0, 0, 0, 0),
        // Alfa parcial: sobre el fondo, no sobre la imagen.
        Rgba::new(255, 0, 0, 128),
    ]
    .into_iter()
    .enumerate()
    {
        for y in 0..2 {
            for x in 0..4 {
                v.push((8 + 8 * i as u32 + x, 5 + y, color));
            }
        }
    }
    v
}

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

    fn de_color(c: Color32) -> Visto {
        Visto {
            r: f32::from(c.r()) / 255.0,
            g: f32::from(c.g()) / 255.0,
            b: f32::from(c.b()) / 255.0,
            a: f32::from(c.a()) / 255.0,
        }
    }

    fn cerca(&self, otro: Visto, tolerancia: f32) -> bool {
        (self.r - otro.r).abs() <= tolerancia
            && (self.g - otro.g).abs() <= tolerancia
            && (self.b - otro.b).abs() <= tolerancia
            && (self.a - otro.a).abs() <= tolerancia
    }
}

/// Coordenadas baricéntricas de `p` en el triángulo, si cae dentro (bordes incluidos).
fn baricentricas(p: Pos2, a: Pos2, b: Pos2, c: Pos2) -> Option<[f32; 3]> {
    let area = (b.x - a.x) * (c.y - a.y) - (c.x - a.x) * (b.y - a.y);
    if area.abs() < 1e-9 {
        return None;
    }
    let w0 = ((b.x - p.x) * (c.y - p.y) - (c.x - p.x) * (b.y - p.y)) / area;
    let w1 = ((c.x - p.x) * (a.y - p.y) - (a.x - p.x) * (c.y - p.y)) / area;
    let w2 = 1.0 - w0 - w1;
    // Sin tolerancia: un punto junto a un vértice compartido caería en dos triángulos y se
    // contaría dos veces.
    (w0 >= 0.0 && w1 >= 0.0 && w2 >= 0.0).then_some([w0, w1, w2])
}

/// Color que se ve en el punto `p` (puntos de egui) al pintar `primitivas` en orden. Las mallas
/// con la textura de la imagen valen azul opaco por el tinte de sus vértices; las demás (rellenos
/// y formas) usan el color de sus vértices tal cual (la textura es el píxel blanco de la fuente).
fn rasteriza(primitivas: &[ClippedPrimitive], imagen: TextureId, p: Pos2) -> Visto {
    let mut visto = Visto::VACIO;
    for primitiva in primitivas {
        if !primitiva.clip_rect.contains(p) {
            continue;
        }
        let Primitive::Mesh(malla) = &primitiva.primitive else {
            continue;
        };
        let color_de = |v: &Vertex| {
            let tinte = Visto::de_color(v.color);
            if malla.texture_id == imagen {
                let azul = Visto::de_color(AZUL);
                Visto {
                    r: azul.r * tinte.r,
                    g: azul.g * tinte.g,
                    b: azul.b * tinte.b,
                    a: tinte.a,
                }
            } else {
                tinte
            }
        };
        for triangulo in malla.indices.chunks_exact(3) {
            let v = [
                &malla.vertices[triangulo[0] as usize],
                &malla.vertices[triangulo[1] as usize],
                &malla.vertices[triangulo[2] as usize],
            ];
            if let Some(w) = baricentricas(p, v[0].pos, v[1].pos, v[2].pos) {
                let c = [color_de(v[0]), color_de(v[1]), color_de(v[2])];
                let mezcla = Visto {
                    r: w[0] * c[0].r + w[1] * c[1].r + w[2] * c[2].r,
                    g: w[0] * c[0].g + w[1] * c[1].g + w[2] * c[2].g,
                    b: w[0] * c[0].b + w[1] * c[1].b + w[2] * c[2].b,
                    a: w[0] * c[0].a + w[1] * c[1].a + w[2] * c[2].a,
                };
                visto = visto.debajo_de(mezcla);
            }
        }
    }
    visto
}

/// Color que debe verse: el del píxel pendiente sobre la imagen azul (alfa 255) o sobre el fondo
/// (alfa menor que 255: el píxel se sustituye).
fn esperado(color: Rgba) -> Visto {
    let base = if color.a == 255 {
        Visto::de_color(AZUL)
    } else {
        Visto::de_color(BACKGROUND)
    };
    let a = f32::from(color.a) / 255.0;
    let sobre = Visto {
        r: f32::from(color.r) / 255.0 * a,
        g: f32::from(color.g) / 255.0 * a,
        b: f32::from(color.b) / 255.0 * a,
        a,
    };
    base.debajo_de(sobre)
}

struct Banco {
    ctx: egui::Context,
    ppp: f32,
    canvas: Canvas,
}

impl Banco {
    fn nuevo(ppp: f32) -> Banco {
        let mut canvas = Canvas::new();
        canvas.set_image(&imagen_azul());
        let mut banco = Banco {
            ctx: contexto(ppp),
            ppp,
            canvas,
        };
        for _ in 0..3 {
            banco.pinta(&[]);
        }
        banco
    }

    /// Un fotograma con la superposición dada; devuelve lo pintado ya teselado y la información.
    fn pinta(&mut self, overlay: &[Pendiente]) -> (Vec<ClippedPrimitive>, CanvasInfo) {
        let mut info = None;
        let canvas = &mut self.canvas;
        let formas = fotograma(&self.ctx, PANTALLA, vec![], |ui| {
            canvas.interact_ordered(ui);
            info = Some(canvas.paint(ui, overlay.iter().copied()));
        });
        let primitivas = self.ctx.tessellate(formas, self.ppp);
        (primitivas, info.expect("el lienzo se pinta"))
    }

    /// Lleva el zoom a ×`zoom` (1, 2 o 3) con las operaciones de vista.
    fn con_zoom(&mut self, zoom: u32) {
        self.canvas.request_view(ViewOp::Actual);
        for _ in 1..zoom {
            self.canvas.request_view(ViewOp::ZoomIn);
        }
        self.pinta(&[]);
        assert_eq!(
            self.canvas.zoom().map(|z| z.to_string()),
            Some(format!("×{zoom}"))
        );
    }
}

/// Todos los píxeles físicos del bloque de cada píxel pendiente se ven del color esperado.
fn comprueba(ppp: f32, zoom: u32) -> Vec<String> {
    let mut banco = Banco::nuevo(ppp);
    banco.con_zoom(zoom);
    let overlay = superposicion();
    let (primitivas, info) = banco.pinta(&overlay);
    let imagen = banco.canvas.texture_id().expect("hay textura");
    let rect: Rect = info.image_rect.expect("hay imagen");
    let origen = (
        (rect.min.x * ppp).round() as i64,
        (rect.min.y * ppp).round() as i64,
    );
    let z = info.zoom.scale(1);
    assert!(z >= 1);
    assert_eq!(z, i64::from(zoom), "el zoom se mide en píxeles físicos");
    let mut fallos = Vec::new();
    for (x, y, color) in &overlay {
        let esperado = esperado(*color);
        for dy in 0..z {
            for dx in 0..z {
                let (px, py) = (
                    origen.0 + i64::from(*x) * z + dx,
                    origen.1 + i64::from(*y) * z + dy,
                );
                // Un poco desplazado del centro exacto: en un vértice o una arista compartidos la GPU
                // aplica una regla de desempate y cuenta el píxel una sola vez; aquí se evita el
                // empate para no contarlo dos veces.
                let centro = pos2(
                    (px as f32 + 0.5 + 0.0013) / ppp,
                    (py as f32 + 0.5 + 0.0007) / ppp,
                );
                let visto = rasteriza(&primitivas, imagen, centro);
                if !visto.cerca(esperado, 0.02) {
                    fallos.push(format!(
                        "ppp {ppp} ×{zoom} (píxel de {z} px físicos): el píxel de imagen ({x}, {y}) con {color:?} se ve {visto:?} en el píxel físico ({px}, {py}) y debía verse {esperado:?}"
                    ));
                }
            }
        }
    }
    fallos
}

#[test]
fn los_pixeles_no_pendientes_se_ven_azules_con_cualquier_escala_y_zoom() {
    // Control del rasterizador de la prueba: sin superposición, la imagen se ve azul opaca.
    for ppp in PPPS {
        for zoom in 1..=3 {
            let mut banco = Banco::nuevo(ppp);
            banco.con_zoom(zoom);
            let (primitivas, info) = banco.pinta(&[]);
            let imagen = banco.canvas.texture_id().expect("textura");
            let rect = info.image_rect.expect("imagen");
            // Fuera de la diagonal de la malla de la imagen, donde dos triángulos se tocan.
            let centro = pos2(
                (rect.min.x + rect.max.x) / 2.0 + 0.3,
                (rect.min.y + rect.max.y) / 2.0 + 0.1,
            );
            let visto = rasteriza(&primitivas, imagen, centro);
            assert!(
                visto.cerca(Visto::de_color(AZUL), 0.02),
                "ppp {ppp} ×{zoom}: {visto:?}"
            );
        }
    }
}

#[test]
fn con_zoom_x2_y_x3_a_todas_las_escalas_la_superposicion_se_ve_exacta() {
    // El caso que ya funciona: el píxel de imagen mide 2 y 3 píxeles físicos.
    let mut fallos = Vec::new();
    for ppp in PPPS {
        for zoom in [2, 3] {
            fallos.extend(comprueba(ppp, zoom));
        }
    }
    assert!(
        fallos.is_empty(),
        "{} fallos; el primero: {}",
        fallos.len(),
        fallos[0]
    );
}

#[test]
fn con_zoom_x1_a_todas_las_escalas_la_superposicion_se_ve_exacta() {
    // El fallo de la prueba manual: el píxel de imagen mide 1 píxel físico a ppp 1,0, 1,25, 1,5,
    // 2,0 y 3,0, y la superposición del lápiz, la línea y el borrador no se veía.
    let mut fallos = Vec::new();
    for ppp in PPPS {
        fallos.extend(comprueba(ppp, 1));
    }
    assert!(
        fallos.is_empty(),
        "{} fallos; el primero: {}",
        fallos.len(),
        fallos[0]
    );
}

#[test]
fn con_zoom_x1_ninguna_escala_pierde_pixeles_de_la_superposicion() {
    // Dice a qué escalas afecta el fallo: cuántos de los 24 píxeles pendientes (cada uno de 1 px
    // físico) se ven mal en cada una.
    let mal: Vec<(f32, usize)> = PPPS
        .iter()
        .map(|&ppp| (ppp, comprueba(ppp, 1).len()))
        .filter(|(_, n)| *n > 0)
        .collect();
    assert!(
        mal.is_empty(),
        "escalas con píxeles mal pintados a ×1: {mal:?}"
    );
}
