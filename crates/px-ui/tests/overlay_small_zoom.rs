//! La superposición del trazo con el píxel de imagen de menos de 2 píxeles físicos (fallo de la
//! prueba manual de la unidad 17: a zoom ×1 la vista previa del lápiz, la línea y el borrador no se
//! ve mientras se arrastra), reexpresada para la unidad 14a-2 (specs/hito-2-zoom-baldosas.md,
//! decisión 7 y fila B8): la superposición ya no son formas, es **parte de la textura** de la
//! baldosa.
//!
//! El banco pinta un lienzo con una imagen azul opaca y una superposición
//! (`Canvas::set_overlay`, cada fotograma tras `interact` y antes de `paint`), aplica los
//! `textures_delta` de **cada** fotograma (`common::textures::Textures`: completos y parciales),
//! pasa las formas por el teselador de egui (`Context::tessellate`, con los mismos ajustes que la
//! app) y rasteriza **a mano** lo teselado en el centro de cada píxel físico del bloque de cada
//! píxel pendiente: el color que se vería. La malla de la baldosa se rasteriza **muestreando su
//! textura** en el uv interpolado (`NEAREST`, sin envoltura: `Textures::samples`); el damero, con
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
//! primitivas del fotograma (`no_overlay_shape`).
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
use common::textures::Textures;
use common::{PPPS, context};

const TEST_SCREEN: egui::Vec2 = egui::vec2(400.0, 300.0);
const BLUE: Color32 = Color32::from_rgb(0, 0, 255);

fn image_blue() -> Image {
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
type TestPending = (u32, u32, Rgba);

/// Superposición de prueba: un bloque de 4×2 píxeles por cada caso.
fn overlay_2() -> Vec<TestPending> {
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

    fn of_color(c: Color32) -> Seen {
        Seen {
            r: f32::from(c.r()) / 255.0,
            g: f32::from(c.g()) / 255.0,
            b: f32::from(c.b()) / 255.0,
            a: f32::from(c.a()) / 255.0,
        }
    }

    fn near(&self, other_2: Seen, tolerance: f32) -> bool {
        (self.r - other_2.r).abs() <= tolerance
            && (self.g - other_2.g).abs() <= tolerance
            && (self.b - other_2.b).abs() <= tolerance
            && (self.a - other_2.a).abs() <= tolerance
    }
}

/// Coordenadas baricéntricas de `p` en el triángulo, si cae dentro (bordes incluidos).
fn barycentric(p: Pos2, a: Pos2, b: Pos2, c: Pos2) -> Option<[f32; 3]> {
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
fn texel_of_the_checker(u: f32, v: f32) -> Seen {
    let tx = (u * 2.0).floor().rem_euclid(2.0) as usize;
    let ty = (v * 2.0).floor().rem_euclid(2.0) as usize;
    let c = if (tx + ty).is_multiple_of(2) {
        CHECK_A
    } else {
        CHECK_B
    };
    Seen::of_color(Color32::from_rgb(c.r, c.g, c.b))
}

/// Color que se ve en el punto `p` (puntos de egui) al pintar `primitives` en orden. Las mallas
/// de las baldosas **muestrean su textura** (la compuesta por los `textures_delta`, con `NEAREST`
/// y sin envoltura) en el uv interpolado y la multiplican por el tinte; las del damero muestrean su
/// textura 2×2 con envoltura; las demás (rellenos y formas) usan el color de sus vértices tal cual
/// (la textura es el píxel blanco de la fuente).
fn rasterizes(
    primitives: &[ClippedPrimitive],
    textures_2: &Textures,
    tiles_2: &[TextureId],
    checker_2: TextureId,
    p: Pos2,
) -> Seen {
    let mut seen = Seen::EMPTY;
    for primitive in primitives {
        if !primitive.clip_rect.contains(p) {
            continue;
        }
        let Primitive::Mesh(mesh) = &primitive.primitive else {
            continue;
        };
        let is_tile = tiles_2.contains(&mesh.texture_id);
        for triangle in mesh.indices.as_chunks::<3>().0 {
            let v = [
                &mesh.vertices[triangle[0] as usize],
                &mesh.vertices[triangle[1] as usize],
                &mesh.vertices[triangle[2] as usize],
            ];
            if let Some(w) = barycentric(p, v[0].pos, v[1].pos, v[2].pos) {
                let c = v.map(|v| Seen::of_color(v.color));
                let mut blend_2 = Seen {
                    r: w[0] * c[0].r + w[1] * c[1].r + w[2] * c[2].r,
                    g: w[0] * c[0].g + w[1] * c[1].g + w[2] * c[2].g,
                    b: w[0] * c[0].b + w[1] * c[1].b + w[2] * c[2].b,
                    a: w[0] * c[0].a + w[1] * c[1].a + w[2] * c[2].a,
                };
                if is_tile || mesh.texture_id == checker_2 {
                    let u = w[0] * v[0].uv.x + w[1] * v[1].uv.x + w[2] * v[2].uv.x;
                    let t = w[0] * v[0].uv.y + w[1] * v[1].uv.y + w[2] * v[2].uv.y;
                    let texel = if is_tile {
                        let texel = textures_2
                            .samples(mesh.texture_id, f64::from(u), f64::from(t))
                            .expect("la baldosa tiene su textura aplicada");
                        Seen::of_color(texel)
                    } else {
                        texel_of_the_checker(u, t)
                    };
                    // El color de la malla es el tinte; el de la textura lo da el texel.
                    blend_2 = Seen {
                        r: texel.r * blend_2.r,
                        g: texel.g * blend_2.g,
                        b: texel.b * blend_2.b,
                        a: texel.a * blend_2.a,
                    };
                }
                seen = seen.below_of(blend_2);
            }
        }
    }
    seen
}

/// Tono del damero en el píxel físico `(px, py)` con la esquina de la imagen en `source_2` y la
/// casilla de `cell` píxeles físicos, calculado aparte de la implementación: la casilla `(0, 0)`
/// es A y alternan por paridad de `floor((px − ox)/cell) + floor((py − oy)/cell)`.
fn tone_of_the_checker(px: i64, py: i64, source_2: (i64, i64), cell: i64) -> Color32 {
    let par = ((px - source_2.0).div_euclid(cell) + (py - source_2.1).div_euclid(cell)) % 2 == 0;
    let c = if par { CHECK_A } else { CHECK_B };
    Color32::from_rgb(c.r, c.g, c.b)
}

/// Color que debe verse: el del píxel pendiente sobre la imagen azul (alfa 255) o sobre el damero
/// (alfa menor que 255: el píxel se sustituye), con `checker_2` el tono del damero en ese píxel físico.
fn expected_2(color: Rgba, checker_2: Color32) -> Seen {
    let base = if color.a == 255 {
        Seen::of_color(BLUE)
    } else {
        Seen::of_color(checker_2)
    };
    let a = f32::from(color.a) / 255.0;
    let over = Seen {
        r: f32::from(color.r) / 255.0 * a,
        g: f32::from(color.g) / 255.0 * a,
        b: f32::from(color.b) / 255.0 * a,
        a,
    };
    base.below_of(over)
}

/// Un fotograma: lo pintado (formas y ya teselado) y la información del lienzo.
struct TestFrame {
    shapes_2: Vec<ClippedShape>,
    primitives: Vec<ClippedPrimitive>,
    info: CanvasInfo,
}

/// Los cambios de textura sin los de la fuente de egui (`Managed(0)`): el banco no pinta texto, y
/// los de la fuente no interesan al oráculo de la imagen.
fn without_the_source(delta: &TexturesDelta) -> TexturesDelta {
    let mut delta = delta.clone();
    delta.set.remove(&TextureId::Managed(0));
    delta.free.remove(&TextureId::Managed(0));
    delta
}

struct Bench {
    ctx: egui::Context,
    ppp: f32,
    canvas: Canvas,
    /// El documento: la imagen azul, que no cambia (la superposición no la toca hasta soltar).
    image_2: Image,
    /// El contenido de las texturas tal como egui lo entrega a la GPU.
    textures_2: Textures,
}

impl Bench {
    fn new_2(ppp: f32) -> Bench {
        let image_2 = image_blue();
        let mut canvas = Canvas::new();
        canvas.set_image(&image_2);
        let mut bench = Bench {
            ctx: context(ppp),
            ppp,
            canvas,
            image_2,
            textures_2: Textures::new(),
        };
        for _ in 0..3 {
            bench.paints(&[]);
        }
        bench
    }

    /// Un fotograma con la superposición dada, en el orden de la app: `interact`, `set_overlay`,
    /// `sync` y `paint`. Aplica los cambios de textura de **cada** fotograma (la superposición
    /// sube bloques con deltas parciales) y devuelve lo pintado ya teselado y la información.
    fn paints(&mut self, overlay: &[TestPending]) -> TestFrame {
        let input_2 = RawInput {
            screen_rect: Some(Rect::from_min_size(Pos2::ZERO, TEST_SCREEN)),
            ..RawInput::default()
        };
        let mut info = None;
        let canvas = &mut self.canvas;
        let image_2 = &self.image_2;
        let mut output_2 = self.ctx.run_ui(input_2, |ui| {
            canvas.interact_roles(ui);
            canvas.set_overlay(ui.ctx(), image_2, overlay);
            canvas
                .sync(ui.ctx(), image_2)
                .expect("el documento se carga");
            info = Some(canvas.paint(ui));
        });
        let mut of_the_canvas = without_the_source(&output_2.textures_delta);
        self.textures_2.applies(&of_the_canvas);
        // La copia no se entrega a la GPU: egui exige vaciarla antes de soltarla.
        of_the_canvas.clear();
        output_2.textures_delta.clear();
        let shapes_2 = std::mem::take(&mut output_2.shapes);
        let primitives = self.ctx.tessellate(shapes_2.clone(), self.ppp);
        TestFrame {
            shapes_2,
            primitives,
            info: info.expect("el lienzo se pinta"),
        }
    }

    /// Las texturas de las baldosas del lienzo.
    fn tiles_2(&self) -> Vec<TextureId> {
        self.canvas.texture_ids()
    }

    /// Lleva el zoom a ×`zoom` (1, 2 o 3) con las operaciones de vista.
    fn with_zoom(&mut self, zoom: u32) {
        self.canvas.request_view(ViewOp::ActualSize);
        for _ in 1..zoom {
            self.canvas.request_view(ViewOp::ZoomIn);
        }
        self.paints(&[]);
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

fn toolbox_2(vertices: &[Vertex]) -> Rect {
    let mut r = Rect::NOTHING;
    for v in vertices {
        r.extend_with(v.pos);
    }
    r
}

/// Área del menor triángulo de una malla, en píxeles físicos cuadrados; `None` sin triángulos.
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

/// Resumen de la estructura de un fotograma (cada forma: tipo, textura, caja y vértices, a una
/// milésima de punto): sirve para decir que la superposición **no añade ni cambia** ninguna forma.
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
                thousandths(toolbox_2(vertices)),
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
fn no_overlay_shape(
    with: &TestFrame,
    without: &TestFrame,
    tiles_2: &[TextureId],
    checker_2: TextureId,
    ppp: f32,
    label_2: &str,
) {
    let image_2 = with.info.image_rect.expect("hay imagen");
    let slack = 1e-3;
    let layers = layers_of(&with.shapes_2);
    let (mut checkers, mut tiles) = (0, 0);
    for layer_2 in &layers {
        match layer_2 {
            Layer::TestFill { rect } => assert!(
                rect.expand(slack).contains_rect(image_2),
                "{label_2}: un rectángulo {rect:?} que no cubre la imagen {image_2:?} (¿de superposición?)"
            ),
            Layer::TestMesh {
                texture_2,
                vertices,
                indices,
            } => {
                let c = toolbox_2(vertices);
                if *texture_2 == checker_2 {
                    checkers += 1;
                    assert!(
                        vertices.len() == 4
                            && c.expand(slack).contains_rect(image_2)
                            && image_2.expand(slack).contains_rect(c),
                        "{label_2}: el damero no es un cuadrilátero de la imagen entera: {c:?}"
                    );
                } else if tiles_2.contains(texture_2) {
                    tiles += 1;
                    assert!(
                        vertices.len() == 4
                            && indices.len() == 6
                            && image_2.expand(slack).contains_rect(c),
                        "{label_2}: la baldosa no es un cuadrilátero dentro de la imagen: {c:?}"
                    );
                } else {
                    panic!(
                        "{label_2}: una malla de superposición ({texture_2:?}, {} vértices, {c:?})",
                        vertices.len()
                    );
                }
                if let Some(area) = smaller_area_physical(vertices, indices, ppp) {
                    assert!(
                        area > 1e-6,
                        "{label_2}: triángulo degenerado (área {area} px²)"
                    );
                }
            }
        }
    }
    assert_eq!(checkers, 1, "{label_2}: un solo cuadrilátero del damero");
    assert_eq!(tiles, tiles_2.len(), "{label_2}: una malla por baldosa");
    for primitive in &with.primitives {
        if let Primitive::Mesh(mesh) = &primitive.primitive
            && let Some(area) = smaller_area_physical(&mesh.vertices, &mesh.indices, ppp)
        {
            assert!(
                area > 1e-6,
                "{label_2}: triángulo degenerado tras teselar (área {area} px²)"
            );
        }
    }
    assert_eq!(
        signature(&layers),
        signature(&layers_of(&without.shapes_2)),
        "{label_2}: la superposición no debe añadir ni cambiar ninguna forma"
    );
}

/// Todos los píxeles físicos del bloque de cada píxel pendiente se ven del color esperado.
fn checks(ppp: f32, zoom: u32) -> Vec<String> {
    let mut bench = Bench::new_2(ppp);
    bench.with_zoom(zoom);
    let overlay = overlay_2();
    let TestFrame {
        primitives, info, ..
    } = bench.paints(&overlay);
    let tiles_2 = bench.tiles_2();
    let checker_2 = bench
        .canvas
        .checker_texture_id()
        .expect("el damero tiene su textura");
    let cell = 8 * i64::from(text_scale(ppp));
    let rect: Rect = info.image_rect.expect("hay imagen");
    let source_2 = (
        (rect.min.x * ppp).round() as i64,
        (rect.min.y * ppp).round() as i64,
    );
    let z = info.zoom.scale(1);
    assert!(z >= 1);
    assert_eq!(z, i64::from(zoom), "el zoom se mide en píxeles físicos");
    let mut failures = Vec::new();
    for (x, y, color) in &overlay {
        for dy in 0..z {
            for dx in 0..z {
                let (px, py) = (
                    source_2.0 + i64::from(*x) * z + dx,
                    source_2.1 + i64::from(*y) * z + dy,
                );
                let expected_2 = expected_2(*color, tone_of_the_checker(px, py, source_2, cell));
                // Un poco desplazado del centro exacto: en un vértice o una arista compartidos la GPU
                // aplica una regla de desempate y cuenta el píxel una sola vez; aquí se evita el
                // empate para no contarlo dos veces.
                let center = pos2(
                    (px as f32 + 0.5 + 0.0013) / ppp,
                    (py as f32 + 0.5 + 0.0007) / ppp,
                );
                let seen = rasterizes(&primitives, &bench.textures_2, &tiles_2, checker_2, center);
                if !seen.near(expected_2, 0.02) {
                    failures.push(format!(
                        "ppp {ppp} ×{zoom} (píxel de {z} px físicos): el píxel de imagen ({x}, {y}) con {color:?} se ve {seen:?} en el píxel físico ({px}, {py}) y debía verse {expected_2:?}"
                    ));
                }
            }
        }
    }
    failures
}

#[test]
fn the_non_pending_pixels_look_blue_at_any_scale_and_zoom() {
    // Control del rasterizador de la prueba: sin superposición, la imagen se ve azul opaca.
    for ppp in PPPS {
        for zoom in 1..=3 {
            let mut bench = Bench::new_2(ppp);
            bench.with_zoom(zoom);
            let TestFrame {
                primitives, info, ..
            } = bench.paints(&[]);
            let tiles_2 = bench.tiles_2();
            let checker_2 = bench
                .canvas
                .checker_texture_id()
                .expect("el damero tiene su textura");
            let rect = info.image_rect.expect("imagen");
            // Fuera de la diagonal de la malla de la imagen, donde dos triángulos se tocan.
            let center = pos2(
                (rect.min.x + rect.max.x) / 2.0 + 0.3,
                (rect.min.y + rect.max.y) / 2.0 + 0.1,
            );
            let seen = rasterizes(&primitives, &bench.textures_2, &tiles_2, checker_2, center);
            assert!(
                seen.near(Seen::of_color(BLUE), 0.02),
                "ppp {ppp} ×{zoom}: {seen:?}"
            );
        }
    }
}

#[test]
fn with_zoom_x2_and_x3_at_all_scales_the_overlay_looks_exact() {
    // El caso que ya funciona: el píxel de imagen mide 2 y 3 píxeles físicos.
    let mut failures = Vec::new();
    for ppp in PPPS {
        for zoom in [2, 3] {
            failures.extend(checks(ppp, zoom));
        }
    }
    assert!(
        failures.is_empty(),
        "{} fallos; el primero: {}",
        failures.len(),
        failures[0]
    );
}

#[test]
fn with_zoom_x1_at_all_scales_the_overlay_looks_exact() {
    // El fallo de la prueba manual: el píxel de imagen mide 1 píxel físico a ppp 1,0, 1,25, 1,5,
    // 2,0 y 3,0, y la superposición del lápiz, la línea y el borrador no se veía.
    let mut failures = Vec::new();
    for ppp in PPPS {
        failures.extend(checks(ppp, 1));
    }
    assert!(
        failures.is_empty(),
        "{} fallos; el primero: {}",
        failures.len(),
        failures[0]
    );
}

#[test]
fn with_zoom_x1_no_scale_loses_pixels_of_the_overlay() {
    // Dice a qué escalas afecta el fallo: cuántos de los 24 píxeles pendientes (cada uno de 1 px
    // físico) se ven mal en cada una.
    let badly: Vec<(f32, usize)> = PPPS
        .iter()
        .map(|&ppp| (ppp, checks(ppp, 1).len()))
        .filter(|(_, n)| *n > 0)
        .collect();
    assert!(
        badly.is_empty(),
        "escalas con píxeles mal pintados a ×1: {badly:?}"
    );
}

#[test]
fn at_any_scale_and_zoom_the_overlay_adds_no_degenerate_shape_or_mesh() {
    // La regresión `b702d8e` reexpresada: la superposición ya no son rectángulos de color ni mallas
    // del damero por píxel pendiente (con 1×1 píxel físico, el suavizado de egui los degeneraba);
    // entre las primitivas del fotograma solo están el fondo, el damero y la malla de la baldosa, y
    // ninguna malla tiene un triángulo de área cero.
    for ppp in PPPS {
        for zoom in 1..=3 {
            let mut bench = Bench::new_2(ppp);
            bench.with_zoom(zoom);
            let without = bench.paints(&[]);
            let with = bench.paints(&overlay_2());
            let checker_2 = bench
                .canvas
                .checker_texture_id()
                .expect("el damero tiene su textura");
            no_overlay_shape(
                &with,
                &without,
                &bench.tiles_2(),
                checker_2,
                ppp,
                &format!("ppp {ppp} ×{zoom}"),
            );
        }
    }
}
