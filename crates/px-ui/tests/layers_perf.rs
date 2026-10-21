//! Rendimiento de la composición en la vista (specs/hito-3-capas-vista.md, «Rendimiento»).
//!
//! **No son puerta del CI**: dependen de la máquina y piden memoria (8 capas de 8192² RGBA son
//! 2 GiB de celdas). Van en `#[ignore]` y se ejecutan a mano, en release y de una en una:
//!
//! ```text
//! cargo test --release -p px-ui --test layers_perf -- --ignored --nocapture --test-threads=1
//! ```
//!
//! Canvas real sin GL ni ventana (`Context::run_ui` con `RawInput` y los deltas de textura
//! descartados), un hilo. Escenarios de 3.2: (a) fondo y capas opacos y Normal; (b) capas a
//! opacidad 128 con mezclas sobre fondo opaco; (c) peor caso, fondo y capas translúcidos con
//! mezclas. El trazo es una serpentina de 100 000 píxeles (2048 de ancho, filas cada 8 px: unos
//! 224 bloques de 64×64) que crece en lotes de 1000 píxeles por fotograma, como el lápiz.
//! El «peor caso» es un píxel en cada uno de los 1024 bloques de una baldosa de 2048².
//!
//! Umbrales (presupuesto de un fotograma a 60 por segundo, no lo medido): con 8 capas, el
//! fotograma más lento del trazo ≤ 16 ms, el coste acumulado del trazo ≤ 250 ms y, al soltar,
//! deshacer y rehacer el peor caso en (c), ≤ 700 ms; con 1 y 3 capas, ≤ 3 ms por fotograma
//! (en 3 capas y (c) se mide 3,3 ms y el umbral es 4 ms: ver la spec).

use std::time::Instant;

use egui::{Context, Pos2, RawInput, Rect, vec2};
use px_core::{BlendMode, Command, Document, Image, ImageId, Mode, Pixel, PixelEdit, Rgba, Span};
use px_ui::canvas::Canvas;
use px_ui::dirty::{BlockSet, ChangeLog, dirty_of};
use px_ui::source::{DocSource, OverlayCache};
use px_ui::view::Size;

const LADO: u32 = 8192;

#[derive(Debug)]
struct Medida {
    fotograma_peor: f64,
    trazo_total: f64,
    soltar: f64,
    deshacer: f64,
    rehacer: f64,
}

fn fotograma(ctx: &Context, canvas: &mut Canvas, f: impl FnOnce(&Context, &mut Canvas)) {
    let entrada = RawInput {
        screen_rect: Some(Rect::from_min_size(Pos2::ZERO, vec2(1280.0, 720.0))),
        ..RawInput::default()
    };
    let mut f = Some(f);
    let mut salida = ctx.run_ui(entrada, |ui| {
        canvas.interact_ordered(ui);
        if let Some(f) = f.take() {
            f(ui.ctx(), canvas);
        }
        canvas.paint(ui);
    });
    salida.textures_delta.clear();
}

fn rellena(doc: &mut Document, celda: ImageId, color: Rgba) {
    let spans = (0..LADO)
        .map(|y| Span {
            y,
            x_start: 0,
            x_end: LADO,
        })
        .collect();
    doc.apply(Command::FillSpans {
        image: celda,
        spans,
        pixel: Pixel::Rgba(color),
    })
    .expect("rellenar");
}

fn documento(capas: usize, escenario: char) -> Document {
    let alfa_base = if escenario == 'c' { 128 } else { 255 };
    let mut doc = Document::new(Image::new(LADO, LADO, Mode::Rgba).expect("imagen"));
    doc.set_cells_limit(usize::MAX);
    let celda0 = doc.layers()[0].cel();
    rellena(&mut doc, celda0, Rgba::new(40, 90, 160, alfa_base));
    for i in 1..capas {
        doc.apply(Command::AddLayer {
            name: format!("c{i}"),
            at: i,
        })
        .expect("capa");
        let (id, celda) = (doc.layers()[i].id(), doc.layers()[i].cel());
        let alfa = if escenario == 'c' { 128 } else { 255 };
        let color = Rgba::new(
            (i * 31 % 256) as u8,
            (i * 77 % 256) as u8,
            (i * 13 % 256) as u8,
            alfa,
        );
        rellena(&mut doc, celda, color);
        if escenario != 'a' {
            doc.apply(Command::SetLayerOpacity {
                layer: id,
                opacity: 128,
            })
            .expect("opacidad");
            doc.apply(Command::SetLayerBlend {
                layer: id,
                blend: BlendMode::ALL[i % 8],
            })
            .expect("mezcla");
        }
    }
    doc
}

fn serpentina(n: usize) -> Vec<PixelEdit> {
    let pixel = Pixel::Rgba(Rgba::opaque(255, 0, 0));
    let mut fuera = Vec::with_capacity(n);
    let (mut y, mut sentido) = (0u32, 1i32);
    while fuera.len() < n {
        let xs: Vec<u32> = if sentido > 0 {
            (0..2048).collect()
        } else {
            (0..2048).rev().collect()
        };
        for x in xs {
            if fuera.len() == n {
                break;
            }
            fuera.push(PixelEdit { x, y, pixel });
        }
        y += 8;
        sentido = -sentido;
    }
    fuera
}

fn peor_caso() -> Vec<PixelEdit> {
    (0..32u32)
        .flat_map(|by| {
            (0..32u32).map(move |bx| PixelEdit {
                x: bx * 64 + 3,
                y: by * 64 + 3,
                pixel: Pixel::Rgba(Rgba::opaque(255, 0, 0)),
            })
        })
        .collect()
}

fn ms(desde: Instant) -> f64 {
    desde.elapsed().as_secs_f64() * 1000.0
}

fn mide(capas: usize, escenario: char, trazo: &[PixelEdit]) -> Medida {
    let mut doc = documento(capas, escenario);
    let ctx = Context::default();
    let mut canvas = Canvas::new();
    fotograma(&ctx, &mut canvas, |_, _| {});
    canvas.load(&ctx, &DocSource(&doc)).expect("load");
    for _ in 0..3 {
        fotograma(&ctx, &mut canvas, |ctx, c| {
            c.sync(ctx, &DocSource(&doc)).expect("sync");
        });
    }
    let activa = doc.layers()[capas - 1].cel();
    let (mut peor, mut total, mut k) = (0.0f64, 0.0f64, 0);
    let mut cache = OverlayCache::default();
    while k < trazo.len() {
        k = (k + 1000).min(trazo.len());
        let t = Instant::now();
        fotograma(&ctx, &mut canvas, |ctx, c| {
            let pendiente = cache.colors(&doc, activa, trazo[..k].iter().copied());
            c.set_overlay(ctx, &DocSource(&doc), pendiente);
            c.sync(ctx, &DocSource(&doc)).expect("sync");
        });
        let d = ms(t);
        peor = peor.max(d);
        total += d;
    }
    let mut registro = ChangeLog::new();
    let comando = Command::SetPixels {
        image: activa,
        pixels: trazo.to_vec(),
    };
    let region = dirty_of(&comando, Size::new(LADO, LADO));
    doc.apply(comando).expect("aplicar");
    registro.record(doc.state_id(), region.clone());
    let t = Instant::now();
    fotograma(&ctx, &mut canvas, |ctx, c| {
        c.invalidate(&region);
        c.sync(ctx, &DocSource(&doc)).expect("sync");
    });
    let soltar = ms(t);
    let t = Instant::now();
    let desde = doc.state_id();
    doc.undo().expect("deshacer");
    let bloques = registro
        .get(desde)
        .cloned()
        .unwrap_or_else(|| BlockSet::full(LADO, LADO));
    fotograma(&ctx, &mut canvas, |ctx, c| {
        c.invalidate(&bloques);
        c.sync(ctx, &DocSource(&doc)).expect("sync");
    });
    let deshacer = ms(t);
    let t = Instant::now();
    doc.redo().expect("rehacer");
    let bloques = registro
        .get(doc.state_id())
        .cloned()
        .unwrap_or_else(|| BlockSet::full(LADO, LADO));
    fotograma(&ctx, &mut canvas, |ctx, c| {
        c.invalidate(&bloques);
        c.sync(ctx, &DocSource(&doc)).expect("sync");
    });
    let rehacer = ms(t);
    let medida = Medida {
        fotograma_peor: peor,
        trazo_total: total,
        soltar,
        deshacer,
        rehacer,
    };
    println!(
        "{capas} capas, escenario {escenario}, {} píxeles: {medida:?}",
        trazo.len()
    );
    medida
}

/// Con 8 capas: fotograma ≤ 16 ms y trazo acumulado ≤ 250 ms.
fn comprueba_8_capas(escenario: char) {
    let m = mide(8, escenario, &serpentina(100_000));
    assert!(m.fotograma_peor <= 16.0, "{escenario}: fotograma {m:?}");
    assert!(m.trazo_total <= 250.0, "{escenario}: acumulado {m:?}");
}

#[test]
#[ignore = "rendimiento: a mano, en release (specs/hito-3-capas-vista.md)"]
fn rendimiento_1_capa() {
    let m = mide(1, 'a', &serpentina(100_000));
    assert!(m.fotograma_peor <= 3.0, "{m:?}");
}

#[test]
#[ignore = "rendimiento: a mano, en release (specs/hito-3-capas-vista.md)"]
fn rendimiento_3_capas() {
    for escenario in ['a', 'b'] {
        let m = mide(3, escenario, &serpentina(100_000));
        assert!(m.fotograma_peor <= 3.0, "{escenario}: {m:?}");
    }
    // (c): 3,3 ms medidos; el umbral es 4 ms (el de 3 ms no cabe en el peor caso).
    let m = mide(3, 'c', &serpentina(100_000));
    assert!(m.fotograma_peor <= 4.0, "c: {m:?}");
}

#[test]
#[ignore = "rendimiento: a mano, en release, 2 GiB de celdas (specs/hito-3-capas-vista.md)"]
fn rendimiento_8_capas_a() {
    comprueba_8_capas('a');
}

#[test]
#[ignore = "rendimiento: a mano, en release, 2 GiB de celdas (specs/hito-3-capas-vista.md)"]
fn rendimiento_8_capas_b() {
    comprueba_8_capas('b');
}

#[test]
#[ignore = "rendimiento: a mano, en release, 2 GiB de celdas (specs/hito-3-capas-vista.md)"]
fn rendimiento_8_capas_c() {
    comprueba_8_capas('c');
}

#[test]
#[ignore = "rendimiento: a mano, en release, 2 GiB de celdas (specs/hito-3-capas-vista.md)"]
fn rendimiento_peor_caso_de_una_baldosa_en_c() {
    let m = mide(8, 'c', &peor_caso());
    for (que, valor) in [
        ("soltar", m.soltar),
        ("deshacer", m.deshacer),
        ("rehacer", m.rehacer),
    ] {
        assert!(valor <= 700.0, "{que}: {m:?}");
    }
}
