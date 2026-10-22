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

const SIDE: u32 = 8192;

#[derive(Debug)]
struct Measure {
    frame_worst: f64,
    stroke_total: f64,
    release_2: f64,
    undo: f64,
    redo: f64,
}

fn frame(ctx: &Context, canvas: &mut Canvas, f: impl FnOnce(&Context, &mut Canvas)) {
    let input_2 = RawInput {
        screen_rect: Some(Rect::from_min_size(Pos2::ZERO, vec2(1280.0, 720.0))),
        ..RawInput::default()
    };
    let mut f = Some(f);
    let mut output_2 = ctx.run_ui(input_2, |ui| {
        canvas.interact_roles(ui);
        if let Some(f) = f.take() {
            f(ui.ctx(), canvas);
        }
        canvas.paint(ui);
    });
    output_2.textures_delta.clear();
}

fn fills(doc: &mut Document, cel: ImageId, color: Rgba) {
    let spans = (0..SIDE)
        .map(|y| Span {
            y,
            x_start: 0,
            x_end: SIDE,
        })
        .collect();
    doc.apply(Command::FillSpans {
        image: cel,
        spans,
        pixel: Pixel::Rgba(color),
    })
    .expect("rellenar");
}

fn document(layers: usize, scenario: char) -> Document {
    let alpha_base = if scenario == 'c' { 128 } else { 255 };
    let mut doc = Document::new(Image::new(SIDE, SIDE, Mode::Rgba).expect("imagen"));
    doc.set_cells_limit(usize::MAX);
    let cel_0 = doc.layers()[0].cel();
    fills(&mut doc, cel_0, Rgba::new(40, 90, 160, alpha_base));
    for i in 1..layers {
        doc.apply(Command::AddLayer {
            name: format!("c{i}"),
            at: i,
        })
        .expect("capa");
        let (id, cel) = (doc.layers()[i].id(), doc.layers()[i].cel());
        let alpha = if scenario == 'c' { 128 } else { 255 };
        let color = Rgba::new(
            (i * 31 % 256) as u8,
            (i * 77 % 256) as u8,
            (i * 13 % 256) as u8,
            alpha,
        );
        fills(&mut doc, cel, color);
        if scenario != 'a' {
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

fn serpentine(n: usize) -> Vec<PixelEdit> {
    let pixel = Pixel::Rgba(Rgba::opaque(255, 0, 0));
    let mut outside = Vec::with_capacity(n);
    let (mut y, mut direction) = (0u32, 1i32);
    while outside.len() < n {
        let xs: Vec<u32> = if direction > 0 {
            (0..2048).collect()
        } else {
            (0..2048).rev().collect()
        };
        for x in xs {
            if outside.len() == n {
                break;
            }
            outside.push(PixelEdit { x, y, pixel });
        }
        y += 8;
        direction = -direction;
    }
    outside
}

fn worst_case() -> Vec<PixelEdit> {
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

fn ms(from_2: Instant) -> f64 {
    from_2.elapsed().as_secs_f64() * 1000.0
}

fn measures(layers: usize, scenario: char, stroke_2: &[PixelEdit]) -> Measure {
    let mut doc = document(layers, scenario);
    let ctx = Context::default();
    let mut canvas = Canvas::new();
    frame(&ctx, &mut canvas, |_, _| {});
    canvas.load(&ctx, &DocSource(&doc)).expect("load");
    for _ in 0..3 {
        frame(&ctx, &mut canvas, |ctx, c| {
            c.sync(ctx, &DocSource(&doc)).expect("sync");
        });
    }
    let active_2 = doc.layers()[layers - 1].cel();
    let (mut worst, mut total, mut k) = (0.0f64, 0.0f64, 0);
    let mut cache = OverlayCache::default();
    while k < stroke_2.len() {
        k = (k + 1000).min(stroke_2.len());
        let t = Instant::now();
        frame(&ctx, &mut canvas, |ctx, c| {
            let pending = cache.colors(&doc, active_2, stroke_2[..k].iter().copied());
            c.set_overlay(ctx, &DocSource(&doc), pending);
            c.sync(ctx, &DocSource(&doc)).expect("sync");
        });
        let d = ms(t);
        worst = worst.max(d);
        total += d;
    }
    let mut log = ChangeLog::new();
    let command = Command::SetPixels {
        image: active_2,
        pixels: stroke_2.to_vec(),
    };
    let region = dirty_of(&command, Size::new(SIDE, SIDE));
    doc.apply(command).expect("aplicar");
    log.record(doc.state_id(), region.clone());
    let t = Instant::now();
    frame(&ctx, &mut canvas, |ctx, c| {
        c.invalidate(&region);
        c.sync(ctx, &DocSource(&doc)).expect("sync");
    });
    let release_2 = ms(t);
    let t = Instant::now();
    let from_2 = doc.state_id();
    doc.undo().expect("deshacer");
    let blocks = log
        .get(from_2)
        .cloned()
        .unwrap_or_else(|| BlockSet::full(SIDE, SIDE));
    frame(&ctx, &mut canvas, |ctx, c| {
        c.invalidate(&blocks);
        c.sync(ctx, &DocSource(&doc)).expect("sync");
    });
    let undo = ms(t);
    let t = Instant::now();
    doc.redo().expect("rehacer");
    let blocks = log
        .get(doc.state_id())
        .cloned()
        .unwrap_or_else(|| BlockSet::full(SIDE, SIDE));
    frame(&ctx, &mut canvas, |ctx, c| {
        c.invalidate(&blocks);
        c.sync(ctx, &DocSource(&doc)).expect("sync");
    });
    let redo = ms(t);
    let measure = Measure {
        frame_worst: worst,
        stroke_total: total,
        release_2,
        undo,
        redo,
    };
    println!(
        "{layers} capas, escenario {scenario}, {} píxeles: {measure:?}",
        stroke_2.len()
    );
    measure
}

/// Con 8 capas: fotograma ≤ 16 ms y trazo acumulado ≤ 250 ms.
fn checks_8_layers(scenario: char) {
    let m = measures(8, scenario, &serpentine(100_000));
    assert!(m.frame_worst <= 16.0, "{scenario}: fotograma {m:?}");
    assert!(m.stroke_total <= 250.0, "{scenario}: acumulado {m:?}");
}

#[test]
#[ignore = "rendimiento: a mano, en release (specs/hito-3-capas-vista.md)"]
fn performance_1_layer() {
    let m = measures(1, 'a', &serpentine(100_000));
    assert!(m.frame_worst <= 3.0, "{m:?}");
}

#[test]
#[ignore = "rendimiento: a mano, en release (specs/hito-3-capas-vista.md)"]
fn performance_3_layers() {
    for scenario in ['a', 'b'] {
        let m = measures(3, scenario, &serpentine(100_000));
        assert!(m.frame_worst <= 3.0, "{scenario}: {m:?}");
    }
    // (c): 3,3 ms medidos; el umbral es 4 ms (el de 3 ms no cabe en el peor caso).
    let m = measures(3, 'c', &serpentine(100_000));
    assert!(m.frame_worst <= 4.0, "c: {m:?}");
}

#[test]
#[ignore = "rendimiento: a mano, en release, 2 GiB de celdas (specs/hito-3-capas-vista.md)"]
fn performance_8_layers_a() {
    checks_8_layers('a');
}

#[test]
#[ignore = "rendimiento: a mano, en release, 2 GiB de celdas (specs/hito-3-capas-vista.md)"]
fn performance_8_layers_b() {
    checks_8_layers('b');
}

#[test]
#[ignore = "rendimiento: a mano, en release, 2 GiB de celdas (specs/hito-3-capas-vista.md)"]
fn performance_8_layers_c() {
    checks_8_layers('c');
}

#[test]
#[ignore = "rendimiento: a mano, en release, 2 GiB de celdas (specs/hito-3-capas-vista.md)"]
fn performance_worst_case_of_a_tile_in_c() {
    let m = measures(8, 'c', &worst_case());
    for (that, value) in [
        ("soltar", m.release_2),
        ("deshacer", m.undo),
        ("rehacer", m.redo),
    ] {
        assert!(value <= 700.0, "{that}: {m:?}");
    }
}
