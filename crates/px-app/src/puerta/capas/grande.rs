//! Fila 10 de la puerta del hito 3 (specs/hito-3-puerta.md): lienzos grandes con 4 capas,
//! **`#[ignore]`**: RGBA de 8192² e indexado de 6000².
//!
//! Se lanzan a mano, una vez al cierre, en release y con un solo hilo:
//! `cargo test --release -p px-app -- --ignored --test-threads=1 puerta::capas::grande`. En
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

use super::Trabajo3;
use super::fuera_de_golden_aseprite;
use super::guiones::{sesion_indexada_de, sesion_rgba_de};
use crate::merge_flow::MergeKind;
use crate::puerta::comun::{Lcg, sesion_vacia};
use crate::session::Session;

struct Escenario {
    lado: usize,
    indexado: bool,
    /// Valores de las capas: fondo, relleno de la 1, raya de la 1, lápiz, línea y relleno de la 3.
    v: [[u8; 4]; 6],
}

impl Escenario {
    fn nuevo(indexado: bool, lado: usize) -> Escenario {
        let v = if indexado {
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
        Escenario { lado, indexado, v }
    }

    fn bpp(&self) -> usize {
        if self.indexado { 1 } else { 4 }
    }

    fn transparente(&self) -> [u8; 4] {
        [0, 0, 0, 0]
    }

    /// El píxel `(x, y)` de la capa `capa` (`None`: celda vacía o transparente).
    fn valor(&self, capa: usize, x: usize, y: usize) -> [u8; 4] {
        match capa {
            0 => self.v[0],
            1 if y.is_multiple_of(64) => self.v[2],
            1 => self.v[1],
            2 if y == self.lado / 2 => self.v[4],
            2 if y == 50 && (100..=102).contains(&x) => self.v[3],
            2 => self.transparente(),
            _ => self.v[5],
        }
    }

    /// La fila `y` de la capa en bytes (con el transparente del modo: el índice 0 o `(0,0,0,0)`).
    fn fila(&self, capa: usize, y: usize, salida: &mut Vec<u8>) {
        salida.clear();
        for x in 0..self.lado {
            let c = self.valor(capa, x, y);
            salida.extend_from_slice(&c[..self.bpp()]);
        }
    }

    fn verificar(&self, doc: &Document, ctx: &str) {
        assert_eq!(doc.layers().len(), 4, "{ctx}: las 4 capas");
        let mut esperada = Vec::new();
        for (i, capa) in doc.layers().iter().enumerate() {
            let imagen = doc
                .image(capa.cel())
                .unwrap_or_else(|| panic!("{ctx}: la capa {i} tiene celda"));
            for y in 0..self.lado {
                self.fila(i, y, &mut esperada);
                let real = imagen.row(y as u32).expect("la fila existe");
                if real != &esperada[..] {
                    let n = self.bpp();
                    let x = real
                        .iter()
                        .zip(&esperada)
                        .position(|(a, b)| a != b)
                        .unwrap_or(0)
                        / n;
                    panic!(
                        "{ctx}: capa {i}, píxel ({x}, {y}): imagen {:?}, esperado {:?}",
                        &real[x * n..(x + 1) * n],
                        &esperada[x * n..(x + 1) * n]
                    );
                }
            }
        }
        if !self.indexado {
            let tres = &doc.layers()[3];
            assert_eq!(tres.blend(), BlendMode::Multiply, "{ctx}");
            assert_eq!(tres.opacity(), 128, "{ctx}");
        }
    }
}

/// Un gesto con la herramienta sobre la celda de la capa activa.
fn gesto(
    tools: &mut ToolState,
    s: &mut Session,
    herramienta: Tool,
    color: [u8; 4],
    indexado: bool,
    eventos: Vec<CanvasEvent>,
) {
    use px_core::{Pixel, Rgba};
    let pixel = if indexado {
        Pixel::Indexed(color[0])
    } else {
        Pixel::Rgba(Rgba::new(color[0], color[1], color[2], color[3]))
    };
    let id = s.active_cel();
    tools.apply_action(ToolAction::Select(herramienta));
    for evento in eventos {
        tools
            .canvas_event_with(&mut s.document, id, Some(pixel), evento)
            .unwrap_or_else(|e| panic!("el gesto dio error: {e}"));
    }
    let cambios = tools.take_changes();
    s.absorb(cambios);
}

fn trazo(a: (i32, i32), b: (i32, i32)) -> Vec<CanvasEvent> {
    vec![
        CanvasEvent::StrokeBegin(a),
        CanvasEvent::StrokeMove(b),
        CanvasEvent::StrokeEnd,
    ]
}

/// Un campo de `/proc/self/status` en bytes (solo Linux; `None` en otros sistemas).
fn campo(nombre: &str) -> Option<u64> {
    let status = fs::read_to_string("/proc/self/status").ok()?;
    let linea = status
        .lines()
        .find_map(|l| l.strip_prefix(nombre)?.strip_prefix(':'))?;
    let kb: u64 = linea.split_whitespace().next()?.parse().ok()?;
    Some(kb * 1024)
}

fn tomar(ctx: &str, que: &str, desde: Instant) {
    let pico = campo("VmHWM").map_or("?".to_string(), |b| format!("{} MiB", b >> 20));
    eprintln!(
        "fila 10 · {ctx} · {que}: {:?} (pico de RSS hasta aquí: {pico})",
        desde.elapsed()
    );
}

fn recorrido(ctx: &str, indexado: bool, lado: usize) {
    let esc = Escenario::nuevo(indexado, lado);
    let t = Trabajo3::nuevo();
    let l = lado as i32;
    let mut s = if indexado {
        sesion_indexada_de(lado as u32, lado as u32)
    } else {
        sesion_rgba_de(lado as u32, lado as u32)
    };
    let mut tools = ToolState::new();

    // Pintar las tres capas de encima del fondo.
    let inicio = Instant::now();
    assert!(s.new_layer().changed);
    gesto(
        &mut tools,
        &mut s,
        Tool::Fill,
        esc.v[1],
        indexado,
        vec![CanvasEvent::StrokeBegin((0, 0)), CanvasEvent::StrokeEnd],
    );
    for y in (0..l).step_by(64) {
        gesto(
            &mut tools,
            &mut s,
            Tool::Line,
            esc.v[2],
            indexado,
            trazo((0, y), (l - 1, y)),
        );
    }
    assert!(s.new_layer().changed);
    gesto(
        &mut tools,
        &mut s,
        Tool::Pencil,
        esc.v[3],
        indexado,
        trazo((100, 50), (102, 50)),
    );
    gesto(
        &mut tools,
        &mut s,
        Tool::Line,
        esc.v[4],
        indexado,
        trazo((0, l / 2), (l - 1, l / 2)),
    );
    assert!(s.new_layer().changed);
    gesto(
        &mut tools,
        &mut s,
        Tool::Fill,
        esc.v[5],
        indexado,
        vec![CanvasEvent::StrokeBegin((0, 0)), CanvasEvent::StrokeEnd],
    );
    if !indexado {
        let id = s.document.layers()[3].id();
        assert!(s.set_layer_blend(id, BlendMode::Multiply).changed);
        assert!(s.set_layer_opacity(id, 128).changed);
    }
    tomar(ctx, "pintar las 4 capas", inicio);
    let inicio = Instant::now();
    esc.verificar(&s.document, &format!("{ctx}: tras pintar"));
    tomar(ctx, "comprobar por fórmula", inicio);

    // Combinar visibles, deshacer, rehacer y deshacer otra vez.
    let inicio = Instant::now();
    assert!(s.merge(MergeKind::Visible).changed);
    tomar(ctx, "combinar visibles", inicio);
    assert_eq!(s.document.layers().len(), 1);
    let inicio = Instant::now();
    s.undo().expect("undo de combinar");
    tomar(ctx, "undo de combinar visibles", inicio);
    esc.verificar(&s.document, &format!("{ctx}: tras deshacer"));
    let inicio = Instant::now();
    s.redo().expect("redo de combinar");
    tomar(ctx, "redo de combinar visibles", inicio);
    assert_eq!(s.document.layers().len(), 1);
    s.undo().expect("undo de combinar");
    esc.verificar(&s.document, &format!("{ctx}: tras deshacer otra vez"));

    // Exportar el PNG aplanado.
    let plano = t.ruta().join("plano.png");
    fuera_de_golden_aseprite(&plano);
    let inicio = Instant::now();
    let exportado = s.export_flat(&plano).expect("exportar");
    tomar(ctx, "exportar el PNG aplanado", inicio);
    let png = read_png(&fs::read(&exportado.path).expect("leer"))
        .expect("PNG")
        .image;
    assert_eq!((png.width() as usize, png.height() as usize), (lado, lado));
    drop(png);
    fs::remove_file(&exportado.path).expect("borrar el PNG");

    // Guardar el `.aseprite` (el tiempo que importa para «Guardando…»).
    let destino = t.ruta().join("grande.aseprite");
    fuera_de_golden_aseprite(&destino);
    let inicio = Instant::now();
    s.save_to(&destino).expect("guardar el .aseprite");
    tomar(ctx, "GUARDAR el .aseprite", inicio);
    let bytes = fs::metadata(&destino).expect("metadatos").len();
    eprintln!("fila 10 · {ctx} · el .aseprite mide {bytes} bytes");

    // Soltar la sesión y reabrir: el pico no incluye dos documentos a la vez.
    drop(s);
    let mut otra = sesion_vacia();
    let inicio = Instant::now();
    otra.open(&destino, Canvas::MAX_SIDE, true)
        .expect("reabrir el .aseprite");
    tomar(ctx, "reabrir el .aseprite", inicio);
    let inicio = Instant::now();
    esc.verificar(&otra.document, &format!("{ctx}: tras reabrir"));
    tomar(ctx, "comprobar lo reabierto por fórmula", inicio);
    let nombres = t.nombres();
    assert_eq!(
        nombres,
        vec!["grande.aseprite".to_string()],
        "nada más en el trabajo"
    );
}

#[test]
#[ignore = "lienzo grande: se lanza a mano, en release (cargo test --release -p px-app -- --ignored --test-threads=1 puerta::capas::grande)"]
fn fila_10_rgba_8192_con_4_capas() {
    recorrido("RGBA 8192² · 4 capas", false, 8192);
}

#[test]
#[ignore = "lienzo grande: se lanza a mano, en release (cargo test --release -p px-app -- --ignored --test-threads=1 puerta::capas::grande)"]
fn fila_10_indexado_6000_con_4_capas() {
    recorrido("indexado 6000² · 4 capas", true, 6000);
}

/// Control del recorrido a escala pequeña y sin `#[ignore]`: el mismo guion y la misma fórmula
/// cerrada en 256² (RGBA) y 200² (indexado), para que el recorrido grande no se pudra sin que se
/// vea. Cubre las filas de las rayas (`y % 64`), la línea del medio y los píxeles de lápiz.
#[test]
fn fila_10_el_mismo_recorrido_a_escala_pequena() {
    recorrido("RGBA 256² · 4 capas", false, 256);
    recorrido("indexado 200² · 4 capas", true, 200);
}

/// El peor caso de guardar (medida de la condición 6, **solo cifras**): 4 capas de 8192² RGBA de
/// ruido, que no se comprimen. Sin `Session`: un documento de `from_layers`, `write_aseprite` y
/// `read_aseprite`. Los tiempos van por `eprintln!`; sin aserciones de tiempo.
#[test]
#[ignore = "lienzo grande: se lanza a mano, en release (cargo test --release -p px-app -- --ignored --test-threads=1 puerta::capas::grande)"]
fn fila_10_peor_caso_de_guardar_cuatro_capas_de_ruido_a_8192() {
    let lado = 8192usize;
    let mut azar = Lcg::new(0x5eed);
    let inicio = Instant::now();
    let capas: Vec<LayerSpec> = (0..4)
        .map(|i| {
            let mut bytes = vec![0u8; lado * lado * 4];
            for trozo in bytes.as_chunks_mut::<4>().0 {
                let n = azar.siguiente().to_le_bytes();
                *trozo = [n[0], n[1], n[2], 255];
            }
            LayerSpec {
                name: format!("ruido {i}"),
                visible: true,
                opacity: 255,
                blend: BlendMode::Normal,
                locked: false,
                image: Some(
                    Image::from_bytes(lado as u32, lado as u32, Mode::Rgba, bytes).expect("imagen"),
                ),
            }
        })
        .collect();
    let doc = Document::from_layers(lado as u32, lado as u32, Mode::Rgba, None, capas)
        .expect("documento de ruido");
    tomar("RGBA 8192² · 4 capas de ruido", "construir", inicio);
    let inicio = Instant::now();
    let bytes = write_aseprite(&doc).expect("write_aseprite");
    tomar(
        "RGBA 8192² · 4 capas de ruido",
        &format!("GUARDAR (escribir {} MiB)", bytes.len() >> 20),
        inicio,
    );
    drop(doc);
    let inicio = Instant::now();
    let leido = read_aseprite(&bytes, &AsepriteLimits::default()).expect("read_aseprite");
    tomar("RGBA 8192² · 4 capas de ruido", "abrir", inicio);
    assert_eq!(leido.document.layers().len(), 4);
}
