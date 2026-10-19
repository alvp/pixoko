//! La superposición del trazo con el píxel de imagen de menos de 2 píxeles físicos (fallo de la
//! prueba manual de la unidad 17: a zoom ×1 la vista previa del lápiz, la línea y el borrador no se
//! ve mientras se arrastra), reexpresada para la unidad 14a-2 (specs/hito-2-zoom-baldosas.md,
//! decisión 7 y fila B8): la superposición ya no son formas, es **parte de la textura** de la
//! baldosa.
//!
//! El banco pinta un lienzo con una imagen azul opaca y una superposición
//! (`Canvas::set_overlay`, cada fotograma tras `interact` y antes de `paint`), aplica los
//! `textures_delta` de **cada** fotograma (`common::texturas::Texturas`: completos y parciales),
//! pasa las formas por el teselador de egui (`Context::tessellate`, con los mismos ajustes que la
//! app) y rasteriza **a mano** lo teselado en el centro de cada píxel físico del bloque de cada
//! píxel pendiente: el color que se vería. La malla de la baldosa se rasteriza **muestreando su
//! textura** en el uv interpolado (`NEAREST`, sin envoltura: `Texturas::muestrea`); el damero, con
//! su textura 2×2 con envoltura y sin filtro, como la GPU. Se compara con el color que debe verse
//! (el de la superposición sobre la imagen, o sobre el **damero de transparencia** si su alfa es
//! menor que 255, que es lo que sustituye al píxel al soltar: desde la unidad 17b el fondo bajo la
//! transparencia es el damero y no el gris liso). Se hace a las cinco escalas y con zoom ×1, ×2 y
//! ×3. El zoom de la vista se mide en **píxeles físicos** (`View` trabaja en ellos), así que a ×1
//! un píxel de imagen mide 1 píxel físico con cualquier `ppp` (1,0, 1,25, 1,5, 2,0 y 3,0), y a ×2
//! y ×3, 2 y 3.
//!
//! Regresión `b702d8e` (antes: el suavizado de bordes de egui estrechaba medio píxel cada lado de
//! un rectángulo de 1×1 píxel físico y todos sus vértices acababan en el mismo punto: triángulos
//! degenerados y ningún píxel pintado). Ahora no hay rectángulos de superposición, y la regresión
//! se afirma de dos modos: cada píxel físico del bloque se **ve** del color esperado (a ×1 el
//! bloque mide 1×1) y **no hay ninguna forma de superposición ni malla con área cero** entre las
//! primitivas del fotograma (`ninguna_forma_de_superposicion`).
//!
//! Sin ventana: `Context::run_ui` con `RawInput` sintético, sin GPU ni capturas.

use egui::epaint::{ClippedPrimitive, ClippedShape, Primitive, Shape, Vertex};
use egui::{Color32, Pos2, RawInput, Rect, TextureId, TexturesDelta, pos2};
use px_core::{Image, Mode, Pixel, Rgba};
use px_ui::canvas::{Canvas, CanvasInfo};
use px_ui::palette_grid::{CHECK_A, CHECK_B};
use px_ui::text::text_scale;
use px_ui::view::ViewOp;

mod common;
use common::texturas::Texturas;
use common::{PPPS, contexto};

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

/// Color de un texel de la textura 2×2 del damero (por filas: A, B, B, A), con `NEAREST` y
/// envoltura `Repeat`, en la coordenada de textura `(u, v)`.
fn texel_del_damero(u: f32, v: f32) -> Visto {
    let tx = (u * 2.0).floor().rem_euclid(2.0) as usize;
    let ty = (v * 2.0).floor().rem_euclid(2.0) as usize;
    let c = if (tx + ty).is_multiple_of(2) {
        CHECK_A
    } else {
        CHECK_B
    };
    Visto::de_color(Color32::from_rgb(c.r, c.g, c.b))
}

/// Color que se ve en el punto `p` (puntos de egui) al pintar `primitivas` en orden. Las mallas
/// de las baldosas **muestrean su textura** (la compuesta por los `textures_delta`, con `NEAREST`
/// y sin envoltura) en el uv interpolado y la multiplican por el tinte; las del damero muestrean su
/// textura 2×2 con envoltura; las demás (rellenos y formas) usan el color de sus vértices tal cual
/// (la textura es el píxel blanco de la fuente).
fn rasteriza(
    primitivas: &[ClippedPrimitive],
    texturas: &Texturas,
    baldosas: &[TextureId],
    damero: TextureId,
    p: Pos2,
) -> Visto {
    let mut visto = Visto::VACIO;
    for primitiva in primitivas {
        if !primitiva.clip_rect.contains(p) {
            continue;
        }
        let Primitive::Mesh(malla) = &primitiva.primitive else {
            continue;
        };
        let es_baldosa = baldosas.contains(&malla.texture_id);
        for triangulo in malla.indices.as_chunks::<3>().0 {
            let v = [
                &malla.vertices[triangulo[0] as usize],
                &malla.vertices[triangulo[1] as usize],
                &malla.vertices[triangulo[2] as usize],
            ];
            if let Some(w) = baricentricas(p, v[0].pos, v[1].pos, v[2].pos) {
                let c = v.map(|v| Visto::de_color(v.color));
                let mut mezcla = Visto {
                    r: w[0] * c[0].r + w[1] * c[1].r + w[2] * c[2].r,
                    g: w[0] * c[0].g + w[1] * c[1].g + w[2] * c[2].g,
                    b: w[0] * c[0].b + w[1] * c[1].b + w[2] * c[2].b,
                    a: w[0] * c[0].a + w[1] * c[1].a + w[2] * c[2].a,
                };
                if es_baldosa || malla.texture_id == damero {
                    let u = w[0] * v[0].uv.x + w[1] * v[1].uv.x + w[2] * v[2].uv.x;
                    let t = w[0] * v[0].uv.y + w[1] * v[1].uv.y + w[2] * v[2].uv.y;
                    let texel = if es_baldosa {
                        let texel = texturas
                            .muestrea(malla.texture_id, f64::from(u), f64::from(t))
                            .expect("la baldosa tiene su textura aplicada");
                        Visto::de_color(texel)
                    } else {
                        texel_del_damero(u, t)
                    };
                    // El color de la malla es el tinte; el de la textura lo da el texel.
                    mezcla = Visto {
                        r: texel.r * mezcla.r,
                        g: texel.g * mezcla.g,
                        b: texel.b * mezcla.b,
                        a: texel.a * mezcla.a,
                    };
                }
                visto = visto.debajo_de(mezcla);
            }
        }
    }
    visto
}

/// Tono del damero en el píxel físico `(px, py)` con la esquina de la imagen en `origen` y la
/// casilla de `cell` píxeles físicos, calculado aparte de la implementación: la casilla `(0, 0)`
/// es A y alternan por paridad de `floor((px − ox)/cell) + floor((py − oy)/cell)`.
fn tono_del_damero(px: i64, py: i64, origen: (i64, i64), cell: i64) -> Color32 {
    let par = ((px - origen.0).div_euclid(cell) + (py - origen.1).div_euclid(cell)) % 2 == 0;
    let c = if par { CHECK_A } else { CHECK_B };
    Color32::from_rgb(c.r, c.g, c.b)
}

/// Color que debe verse: el del píxel pendiente sobre la imagen azul (alfa 255) o sobre el damero
/// (alfa menor que 255: el píxel se sustituye), con `damero` el tono del damero en ese píxel físico.
fn esperado(color: Rgba, damero: Color32) -> Visto {
    let base = if color.a == 255 {
        Visto::de_color(AZUL)
    } else {
        Visto::de_color(damero)
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

/// Un fotograma: lo pintado (formas y ya teselado) y la información del lienzo.
struct Fotograma {
    formas: Vec<ClippedShape>,
    primitivas: Vec<ClippedPrimitive>,
    info: CanvasInfo,
}

/// Los cambios de textura sin los de la fuente de egui (`Managed(0)`): el banco no pinta texto, y
/// los de la fuente no interesan al oráculo de la imagen.
fn sin_la_fuente(delta: &TexturesDelta) -> TexturesDelta {
    let mut delta = delta.clone();
    delta.set.remove(&TextureId::Managed(0));
    delta.free.remove(&TextureId::Managed(0));
    delta
}

struct Banco {
    ctx: egui::Context,
    ppp: f32,
    canvas: Canvas,
    /// El documento: la imagen azul, que no cambia (la superposición no la toca hasta soltar).
    imagen: Image,
    /// El contenido de las texturas tal como egui lo entrega a la GPU.
    texturas: Texturas,
}

impl Banco {
    fn nuevo(ppp: f32) -> Banco {
        let imagen = imagen_azul();
        let mut canvas = Canvas::new();
        canvas.set_image(&imagen);
        let mut banco = Banco {
            ctx: contexto(ppp),
            ppp,
            canvas,
            imagen,
            texturas: Texturas::new(),
        };
        for _ in 0..3 {
            banco.pinta(&[]);
        }
        banco
    }

    /// Un fotograma con la superposición dada, en el orden de la app: `interact`, `set_overlay`,
    /// `sync` y `paint`. Aplica los cambios de textura de **cada** fotograma (la superposición
    /// sube bloques con deltas parciales) y devuelve lo pintado ya teselado y la información.
    fn pinta(&mut self, overlay: &[Pendiente]) -> Fotograma {
        let entrada = RawInput {
            screen_rect: Some(Rect::from_min_size(Pos2::ZERO, PANTALLA)),
            ..RawInput::default()
        };
        let mut info = None;
        let canvas = &mut self.canvas;
        let imagen = &self.imagen;
        let mut salida = self.ctx.run_ui(entrada, |ui| {
            canvas.interact_ordered(ui);
            canvas.set_overlay(ui.ctx(), imagen, overlay);
            canvas
                .sync(ui.ctx(), imagen)
                .expect("el documento se carga");
            info = Some(canvas.paint(ui));
        });
        let mut del_lienzo = sin_la_fuente(&salida.textures_delta);
        self.texturas.aplica(&del_lienzo);
        // La copia no se entrega a la GPU: egui exige vaciarla antes de soltarla.
        del_lienzo.clear();
        salida.textures_delta.clear();
        let formas = std::mem::take(&mut salida.shapes);
        let primitivas = self.ctx.tessellate(formas.clone(), self.ppp);
        Fotograma {
            formas,
            primitivas,
            info: info.expect("el lienzo se pinta"),
        }
    }

    /// Las texturas de las baldosas del lienzo.
    fn baldosas(&self) -> Vec<TextureId> {
        self.canvas.texture_ids()
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

// ---------------------------------------------------------------------------------------------
// «Ninguna forma de superposición» (la regresión `b702d8e`, reexpresada)
// ---------------------------------------------------------------------------------------------

/// Una forma del fotograma: un rectángulo relleno o una malla.
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

fn caja(vertices: &[Vertex]) -> Rect {
    let mut r = Rect::NOTHING;
    for v in vertices {
        r.extend_with(v.pos);
    }
    r
}

/// Área del menor triángulo de una malla, en píxeles físicos cuadrados; `None` sin triángulos.
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

/// Resumen de la estructura de un fotograma (cada forma: tipo, textura, caja y vértices, a una
/// milésima de punto): sirve para decir que la superposición **no añade ni cambia** ninguna forma.
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
                milesimas(caja(vertices)),
                vertices.len()
            ),
        })
        .collect()
}

/// Entre las formas del fotograma no hay **ninguna** de superposición: solo el fondo del lienzo y
/// el relleno del damero (rectángulos que cubren la imagen entera), el cuadrilátero del damero (el
/// de la imagen entera) y los cuadriláteros de las baldosas (dentro de la imagen); y **ninguna
/// malla tiene área cero ni triángulos degenerados** (vértices colineales o coincidentes), antes y
/// después de teselar. Además la estructura es **idéntica** a la del fotograma sin superposición.
fn ninguna_forma_de_superposicion(
    con: &Fotograma,
    sin: &Fotograma,
    baldosas: &[TextureId],
    damero: TextureId,
    ppp: f32,
    etiqueta: &str,
) {
    let imagen = con.info.image_rect.expect("hay imagen");
    let holgura = 1e-3;
    let capas = capas_de(&con.formas);
    let (mut dameros, mut tiles) = (0, 0);
    for capa in &capas {
        match capa {
            Capa::Relleno { rect } => assert!(
                rect.expand(holgura).contains_rect(imagen),
                "{etiqueta}: un rectángulo {rect:?} que no cubre la imagen {imagen:?} (¿de superposición?)"
            ),
            Capa::Malla {
                textura,
                vertices,
                indices,
            } => {
                let c = caja(vertices);
                if *textura == damero {
                    dameros += 1;
                    assert!(
                        vertices.len() == 4
                            && c.expand(holgura).contains_rect(imagen)
                            && imagen.expand(holgura).contains_rect(c),
                        "{etiqueta}: el damero no es un cuadrilátero de la imagen entera: {c:?}"
                    );
                } else if baldosas.contains(textura) {
                    tiles += 1;
                    assert!(
                        vertices.len() == 4
                            && indices.len() == 6
                            && imagen.expand(holgura).contains_rect(c),
                        "{etiqueta}: la baldosa no es un cuadrilátero dentro de la imagen: {c:?}"
                    );
                } else {
                    panic!(
                        "{etiqueta}: una malla de superposición ({textura:?}, {} vértices, {c:?})",
                        vertices.len()
                    );
                }
                if let Some(area) = menor_area_fisica(vertices, indices, ppp) {
                    assert!(
                        area > 1e-6,
                        "{etiqueta}: triángulo degenerado (área {area} px²)"
                    );
                }
            }
        }
    }
    assert_eq!(dameros, 1, "{etiqueta}: un solo cuadrilátero del damero");
    assert_eq!(tiles, baldosas.len(), "{etiqueta}: una malla por baldosa");
    for primitiva in &con.primitivas {
        if let Primitive::Mesh(malla) = &primitiva.primitive
            && let Some(area) = menor_area_fisica(&malla.vertices, &malla.indices, ppp)
        {
            assert!(
                area > 1e-6,
                "{etiqueta}: triángulo degenerado tras teselar (área {area} px²)"
            );
        }
    }
    assert_eq!(
        firma(&capas),
        firma(&capas_de(&sin.formas)),
        "{etiqueta}: la superposición no debe añadir ni cambiar ninguna forma"
    );
}

/// Todos los píxeles físicos del bloque de cada píxel pendiente se ven del color esperado.
fn comprueba(ppp: f32, zoom: u32) -> Vec<String> {
    let mut banco = Banco::nuevo(ppp);
    banco.con_zoom(zoom);
    let overlay = superposicion();
    let Fotograma {
        primitivas, info, ..
    } = banco.pinta(&overlay);
    let baldosas = banco.baldosas();
    let damero = banco
        .canvas
        .checker_texture_id()
        .expect("el damero tiene su textura");
    let cell = 8 * i64::from(text_scale(ppp));
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
        for dy in 0..z {
            for dx in 0..z {
                let (px, py) = (
                    origen.0 + i64::from(*x) * z + dx,
                    origen.1 + i64::from(*y) * z + dy,
                );
                let esperado = esperado(*color, tono_del_damero(px, py, origen, cell));
                // Un poco desplazado del centro exacto: en un vértice o una arista compartidos la GPU
                // aplica una regla de desempate y cuenta el píxel una sola vez; aquí se evita el
                // empate para no contarlo dos veces.
                let centro = pos2(
                    (px as f32 + 0.5 + 0.0013) / ppp,
                    (py as f32 + 0.5 + 0.0007) / ppp,
                );
                let visto = rasteriza(&primitivas, &banco.texturas, &baldosas, damero, centro);
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
            let Fotograma {
                primitivas, info, ..
            } = banco.pinta(&[]);
            let baldosas = banco.baldosas();
            let damero = banco
                .canvas
                .checker_texture_id()
                .expect("el damero tiene su textura");
            let rect = info.image_rect.expect("imagen");
            // Fuera de la diagonal de la malla de la imagen, donde dos triángulos se tocan.
            let centro = pos2(
                (rect.min.x + rect.max.x) / 2.0 + 0.3,
                (rect.min.y + rect.max.y) / 2.0 + 0.1,
            );
            let visto = rasteriza(&primitivas, &banco.texturas, &baldosas, damero, centro);
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

#[test]
fn con_cualquier_escala_y_zoom_la_superposicion_no_añade_ninguna_forma_ni_malla_degenerada() {
    // La regresión `b702d8e` reexpresada: la superposición ya no son rectángulos de color ni mallas
    // del damero por píxel pendiente (con 1×1 píxel físico, el suavizado de egui los degeneraba);
    // entre las primitivas del fotograma solo están el fondo, el damero y la malla de la baldosa, y
    // ninguna malla tiene un triángulo de área cero.
    for ppp in PPPS {
        for zoom in 1..=3 {
            let mut banco = Banco::nuevo(ppp);
            banco.con_zoom(zoom);
            let sin = banco.pinta(&[]);
            let con = banco.pinta(&superposicion());
            let damero = banco
                .canvas
                .checker_texture_id()
                .expect("el damero tiene su textura");
            ninguna_forma_de_superposicion(
                &con,
                &sin,
                &banco.baldosas(),
                damero,
                ppp,
                &format!("ppp {ppp} ×{zoom}"),
            );
        }
    }
}
