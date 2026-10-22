//! Fila 10 de la puerta del hito 3 (specs/hito-3-puerta.md): lienzos grandes con 4 capas,
//! **`#[ignore]`**: RGBA de 8192² e indexado de 6000².
//!
//! Se lanzan a mano, una vez al cierre, en release y con un solo hilo:
//! `cargo test --release -p px-app -- --ignored --test-threads=1 gate::layers::large`. En
//! depuración tardan mucho y gastan GiB: no se ejecutan con `cargo test` a secas.
//!
//! Las 4 capas (el fondo es la 0; en RGBA la 3 es Multiplicar con opacidad 128):
//! - 0: el fondo, plano, de todo el lienzo;
//! - 1: relleno plano de todo el lienzo y una raya horizontal cada 64 filas (cubre todo);
//! - 2: tres píxeles de lápiz y una línea de lado a lado en la fila del medio (casi vacía);
//! - 3: relleno plano de todo el lienzo.
//!
//! Recorrido: pintar, combinar visibles, `undo`, `redo`, `undo`, `export_flat`, **guardar el
//! `.aseprite`** (el tiempo que pide la condición 6 de la spec), soltar la sesión y reabrirlo. Lo
//! esperado sale de una **fórmula cerrada** sobre `Image::row`, capa a capa y fila a fila (no hay un
//! segundo documento: el pico no se duplica), y se comprueba tras pintar, tras cada `undo` y `redo`
//! y tras reabrir. Sin aserciones de tiempo: los tiempos y el pico de RSS van por `eprintln!`.

use std::fs;
use std::time::Instant;

use px_core::{BlendMode, Document, Image, LayerSpec, Mode};
use px_formats::read_png;
use px_formats::{AsepriteLimits, read_aseprite, write_aseprite};
use px_ui::canvas::{Canvas, CanvasEvent};
use px_ui::tool::{Tool, ToolAction, ToolState};

use super::Work3;
use super::outside_golden_aseprite;
use super::scripts::{session_indexed_of, session_rgba_of};
use crate::gate::common::{Lcg, session_empty};
use crate::merge_flow::MergeKind;
use crate::session::Session;

struct Scenario {
    side_2: usize,
    indexed_2: bool,
    /// Valores de las capas: fondo, relleno de la 1, raya de la 1, lápiz, línea y relleno de la 3.
    v: [[u8; 4]; 6],
}

impl Scenario {
    fn new_2(indexed_2: bool, side_2: usize) -> Scenario {
        let v = if indexed_2 {
            [5, 3, 7, 9, 12, 14].map(|i| [i, 0, 0, 0])
        } else {
            [
                [196, 46, 52, 255],
                [30, 60, 90, 255],
                [250, 250, 0, 255],
                [255, 0, 255, 255],
                [0, 255, 255, 255],
                [120, 200, 40, 255],
            ]
        };
        Scenario {
            side_2,
            indexed_2,
            v,
        }
    }

    fn bpp(&self) -> usize {
        if self.indexed_2 { 1 } else { 4 }
    }

    fn transparent(&self) -> [u8; 4] {
        [0, 0, 0, 0]
    }

    /// El píxel `(x, y)` de la capa `layer_2` (`None`: celda vacía o transparente).
    fn valor(&self, layer_2: usize, x: usize, y: usize) -> [u8; 4] {
        match layer_2 {
            0 => self.v[0],
            1 if y.is_multiple_of(64) => self.v[2],
            1 => self.v[1],
            2 if y == self.side_2 / 2 => self.v[4],
            2 if y == 50 && (100..=102).contains(&x) => self.v[3],
            2 => self.transparent(),
            _ => self.v[5],
        }
    }

    /// La fila `y` de la capa en bytes (con el transparente del modo: el índice 0 o `(0,0,0,0)`).
    fn row(&self, layer_2: usize, y: usize, output: &mut Vec<u8>) {
        output.clear();
        for x in 0..self.side_2 {
            let c = self.valor(layer_2, x, y);
            output.extend_from_slice(&c[..self.bpp()]);
        }
    }

    fn verify(&self, doc: &Document, ctx: &str) {
        assert_eq!(doc.layers().len(), 4, "{ctx}: las 4 capas");
        let mut expected = Vec::new();
        for (i, layer_2) in doc.layers().iter().enumerate() {
            let image_2 = doc
                .image(layer_2.cel())
                .unwrap_or_else(|| panic!("{ctx}: la capa {i} tiene celda"));
            for y in 0..self.side_2 {
                self.row(i, y, &mut expected);
                let real = image_2.row(y as u32).expect("la fila existe");
                if real != &expected[..] {
                    let n = self.bpp();
                    let x = real
                        .iter()
                        .zip(&expected)
                        .position(|(a, b)| a != b)
                        .unwrap_or(0)
                        / n;
                    panic!(
                        "{ctx}: capa {i}, píxel ({x}, {y}): imagen {:?}, esperado {:?}",
                        &real[x * n..(x + 1) * n],
                        &expected[x * n..(x + 1) * n]
                    );
                }
            }
        }
        if !self.indexed_2 {
            let three = &doc.layers()[3];
            assert_eq!(three.blend(), BlendMode::Multiply, "{ctx}");
            assert_eq!(three.opacity(), 128, "{ctx}");
        }
    }
}

/// Un gesto con la herramienta sobre la celda de la capa activa.
fn gesture(
    tools: &mut ToolState,
    s: &mut Session,
    tool_2: Tool,
    color: [u8; 4],
    indexed_2: bool,
    events: Vec<CanvasEvent>,
) {
    use px_core::{Pixel, Rgba};
    let pixel = if indexed_2 {
        Pixel::Indexed(color[0])
    } else {
        Pixel::Rgba(Rgba::new(color[0], color[1], color[2], color[3]))
    };
    let id = s.active_cel();
    tools.apply_action(ToolAction::Select(tool_2));
    for event_2 in events {
        tools
            .canvas_event_with(&mut s.document, id, Some(pixel), event_2)
            .unwrap_or_else(|e| panic!("el gesto dio error: {e}"));
    }
    let changes_2 = tools.take_changes();
    s.absorb(changes_2);
}

fn stroke(a: (i32, i32), b: (i32, i32)) -> Vec<CanvasEvent> {
    vec![
        CanvasEvent::StrokeBegin(a),
        CanvasEvent::StrokeMove(b),
        CanvasEvent::StrokeEnd,
    ]
}

/// Un campo de `/proc/self/status` en bytes (solo Linux; `None` en otros sistemas).
fn field_2(name_2: &str) -> Option<u64> {
    let status = fs::read_to_string("/proc/self/status").ok()?;
    let line = status
        .lines()
        .find_map(|l| l.strip_prefix(name_2)?.strip_prefix(':'))?;
    let kb: u64 = line.split_whitespace().next()?.parse().ok()?;
    Some(kb * 1024)
}

fn take(ctx: &str, that: &str, from_2: Instant) {
    let peak = field_2("VmHWM").map_or("?".to_string(), |b| format!("{} MiB", b >> 20));
    eprintln!(
        "fila 10 · {ctx} · {that}: {:?} (pico de RSS hasta aquí: {peak})",
        from_2.elapsed()
    );
}

fn walk(ctx: &str, indexed_2: bool, side_2: usize) {
    let esc = Scenario::new_2(indexed_2, side_2);
    let t = Work3::new_2();
    let l = side_2 as i32;
    let mut s = if indexed_2 {
        session_indexed_of(side_2 as u32, side_2 as u32)
    } else {
        session_rgba_of(side_2 as u32, side_2 as u32)
    };
    let mut tools = ToolState::new();

    // Pintar las tres capas de encima del fondo.
    let start_2 = Instant::now();
    assert!(s.new_layer().changed);
    gesture(
        &mut tools,
        &mut s,
        Tool::Fill,
        esc.v[1],
        indexed_2,
        vec![CanvasEvent::StrokeBegin((0, 0)), CanvasEvent::StrokeEnd],
    );
    for y in (0..l).step_by(64) {
        gesture(
            &mut tools,
            &mut s,
            Tool::Line,
            esc.v[2],
            indexed_2,
            stroke((0, y), (l - 1, y)),
        );
    }
    assert!(s.new_layer().changed);
    gesture(
        &mut tools,
        &mut s,
        Tool::Pencil,
        esc.v[3],
        indexed_2,
        stroke((100, 50), (102, 50)),
    );
    gesture(
        &mut tools,
        &mut s,
        Tool::Line,
        esc.v[4],
        indexed_2,
        stroke((0, l / 2), (l - 1, l / 2)),
    );
    assert!(s.new_layer().changed);
    gesture(
        &mut tools,
        &mut s,
        Tool::Fill,
        esc.v[5],
        indexed_2,
        vec![CanvasEvent::StrokeBegin((0, 0)), CanvasEvent::StrokeEnd],
    );
    if !indexed_2 {
        let id = s.document.layers()[3].id();
        assert!(s.set_layer_blend(id, BlendMode::Multiply).changed);
        assert!(s.set_layer_opacity(id, 128).changed);
    }
    take(ctx, "pintar las 4 capas", start_2);
    let start_2 = Instant::now();
    esc.verify(&s.document, &format!("{ctx}: tras pintar"));
    take(ctx, "comprobar por fórmula", start_2);

    // Combinar visibles, deshacer, rehacer y deshacer otra vez.
    let start_2 = Instant::now();
    assert!(s.merge(MergeKind::Visible).changed);
    take(ctx, "combinar visibles", start_2);
    assert_eq!(s.document.layers().len(), 1);
    let start_2 = Instant::now();
    s.undo().expect("undo de combinar");
    take(ctx, "undo de combinar visibles", start_2);
    esc.verify(&s.document, &format!("{ctx}: tras deshacer"));
    let start_2 = Instant::now();
    s.redo().expect("redo de combinar");
    take(ctx, "redo de combinar visibles", start_2);
    assert_eq!(s.document.layers().len(), 1);
    s.undo().expect("undo de combinar");
    esc.verify(&s.document, &format!("{ctx}: tras deshacer otra vez"));

    // Exportar el PNG aplanado.
    let plane = t.path_2().join("plano.png");
    outside_golden_aseprite(&plane);
    let start_2 = Instant::now();
    let exported = s.export_flat(&plane).expect("exportar");
    take(ctx, "exportar el PNG aplanado", start_2);
    let png = read_png(&fs::read(&exported.path).expect("leer"))
        .expect("PNG")
        .image;
    assert_eq!(
        (png.width() as usize, png.height() as usize),
        (side_2, side_2)
    );
    drop(png);
    fs::remove_file(&exported.path).expect("borrar el PNG");

    // Guardar el `.aseprite` (el tiempo que importa para «Guardando…»).
    let target_2 = t.path_2().join("grande.aseprite");
    outside_golden_aseprite(&target_2);
    let start_2 = Instant::now();
    s.save_to(&target_2).expect("guardar el .aseprite");
    take(ctx, "GUARDAR el .aseprite", start_2);
    let bytes = fs::metadata(&target_2).expect("metadatos").len();
    eprintln!("fila 10 · {ctx} · el .aseprite mide {bytes} bytes");

    // Soltar la sesión y reabrir: el pico no incluye dos documentos a la vez.
    drop(s);
    let mut other = session_empty();
    let start_2 = Instant::now();
    other
        .open(&target_2, Canvas::MAX_SIDE, true)
        .expect("reabrir el .aseprite");
    take(ctx, "reabrir el .aseprite", start_2);
    let start_2 = Instant::now();
    esc.verify(&other.document, &format!("{ctx}: tras reabrir"));
    take(ctx, "comprobar lo reabierto por fórmula", start_2);
    let names = t.names();
    assert_eq!(
        names,
        vec!["grande.aseprite".to_string()],
        "nada más en el trabajo"
    );
}

#[test]
#[ignore = "lienzo grande: se lanza a mano, en release (cargo test --release -p px-app -- --ignored --test-threads=1 puerta::capas::grande)"]
fn row_10_rgba_8192_with_4_layers() {
    walk("RGBA 8192² · 4 capas", false, 8192);
}

#[test]
#[ignore = "lienzo grande: se lanza a mano, en release (cargo test --release -p px-app -- --ignored --test-threads=1 puerta::capas::grande)"]
fn row_10_indexed_6000_with_4_layers() {
    walk("indexado 6000² · 4 capas", true, 6000);
}

/// Control del recorrido a escala pequeña y sin `#[ignore]`: el mismo guion y la misma fórmula
/// cerrada en 256² (RGBA) y 200² (indexado), para que el recorrido grande no se pudra sin que se
/// vea. Cubre las filas de las rayas (`y % 64`), la línea del medio y los píxeles de lápiz.
#[test]
fn row_10_the_same_walk_at_small_scale() {
    walk("RGBA 256² · 4 capas", false, 256);
    walk("indexado 200² · 4 capas", true, 200);
}

/// El peor caso de guardar (medida de la condición 6, **solo cifras**): 4 capas de 8192² RGBA de
/// ruido, que no se comprimen. Sin `Session`: un documento de `from_layers`, `write_aseprite` y
/// `read_aseprite`. Los tiempos van por `eprintln!`; sin aserciones de tiempo.
#[test]
#[ignore = "lienzo grande: se lanza a mano, en release (cargo test --release -p px-app -- --ignored --test-threads=1 puerta::capas::grande)"]
fn row_10_worst_case_of_saving_four_noise_layers_at_8192() {
    let side_2 = 8192usize;
    let mut chance = Lcg::new(0x5eed);
    let start_2 = Instant::now();
    let layers_2: Vec<LayerSpec> = (0..4)
        .map(|i| {
            let mut bytes = vec![0u8; side_2 * side_2 * 4];
            for piece in bytes.as_chunks_mut::<4>().0 {
                let n = chance.next().to_le_bytes();
                *piece = [n[0], n[1], n[2], 255];
            }
            LayerSpec {
                name: format!("ruido {i}"),
                visible: true,
                opacity: 255,
                blend: BlendMode::Normal,
                locked: false,
                image: Some(
                    Image::from_bytes(side_2 as u32, side_2 as u32, Mode::Rgba, bytes)
                        .expect("imagen"),
                ),
            }
        })
        .collect();
    let doc = Document::from_layers(side_2 as u32, side_2 as u32, Mode::Rgba, None, layers_2)
        .expect("documento de ruido");
    take("RGBA 8192² · 4 capas de ruido", "construir", start_2);
    let start_2 = Instant::now();
    let bytes = write_aseprite(&doc).expect("write_aseprite");
    take(
        "RGBA 8192² · 4 capas de ruido",
        &format!("GUARDAR (escribir {} MiB)", bytes.len() >> 20),
        start_2,
    );
    drop(doc);
    let start_2 = Instant::now();
    let read_2 = read_aseprite(&bytes, &AsepriteLimits::default()).expect("read_aseprite");
    take("RGBA 8192² · 4 capas de ruido", "abrir", start_2);
    assert_eq!(read_2.document.layers().len(), 4);
}
