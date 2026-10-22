//! Rendimiento de combinar y aplanar (specs/hito-3-combinar.md, «Rendimiento»): **umbrales de
//! presupuesto, no de CI** (`#[ignore]`). Se ejecuta a mano, en release y con un solo hilo de
//! pruebas:
//!
//! `cargo test --release -p px-raster --test merge_perf -- --ignored --nocapture --test-threads=1`
//!
//! Escenarios de 3.2: (a) fondo opaco y capas Normal opacas; (b) fondo opaco y mezclas con
//! opacidad 128; (c) fondo y capas translúcidos con mezclas. Mide la construcción del comando (la
//! composición) y su `apply` (O(capas): mover, no copiar).

use std::time::{Duration, Instant};

use px_core::{BlendMode, Command, Document, Image, Pixel, Rgba, Span};
use px_raster::{flatten_command, merge_down_command, merge_visible_command};

fn document(side: u32, layers: usize, scenario: char) -> Document {
    let alpha_background = if scenario == 'c' { 180 } else { 255 };
    let base = Image::filled(
        side,
        side,
        Pixel::Rgba(Rgba::new(40, 90, 160, alpha_background)),
    )
    .expect("imagen de la capa de abajo");
    let mut doc = Document::new(base);
    doc.set_cells_limit(usize::MAX);
    let modes = [
        BlendMode::Multiply,
        BlendMode::Screen,
        BlendMode::Overlay,
        BlendMode::Darken,
        BlendMode::Lighten,
        BlendMode::Add,
        BlendMode::Difference,
    ];
    for i in 1..layers {
        doc.apply(Command::AddLayer {
            name: format!("c{i}"),
            at: i,
        })
        .expect("añadir capa");
        let (id, cel) = (doc.layers()[i].id(), doc.layers()[i].cel());
        let alpha = if scenario == 'c' { 150 } else { 255 };
        let spans_2: Vec<Span> = (0..side)
            .map(|y| Span {
                y,
                x_start: 0,
                x_end: side,
            })
            .collect();
        let color = Rgba::new((i * 30) as u8, (255 - i * 20) as u8, (i * 50) as u8, alpha);
        doc.apply(Command::FillSpans {
            image: cel,
            spans: spans_2,
            pixel: Pixel::Rgba(color),
        })
        .expect("rellenar");
        if scenario != 'a' {
            doc.apply(Command::SetLayerOpacity {
                layer: id,
                opacity: 128,
            })
            .expect("opacidad");
            doc.apply(Command::SetLayerBlend {
                layer: id,
                blend: modes[(i - 1) % modes.len()],
            })
            .expect("mezcla");
        }
    }
    doc
}

fn measures<T>(label: &str, cap: Duration, f: impl FnOnce() -> T) -> T {
    let start = Instant::now();
    let value = f();
    let time = start.elapsed();
    println!(
        "{label}: {:.1} ms (tope {:.0} ms)",
        time.as_secs_f64() * 1e3,
        cap.as_secs_f64() * 1e3
    );
    assert!(time <= cap, "{label}: {time:?} pasa de {cap:?}");
    value
}

#[test]
#[ignore = "rendimiento: umbrales de presupuesto, no de CI"]
fn merge_down_2_layers_of_2048_and_of_8192() {
    for (side, caps) in [(2048u32, [150u64, 150, 150]), (8192, [2500, 2500, 2500])] {
        for (scenario, cap) in ['a', 'b', 'c'].into_iter().zip(caps) {
            let mut doc = document(side, 2, scenario);
            let top = doc.layers()[1].id();
            let order = measures(
                &format!("combinar abajo {side}² ({scenario}): construir"),
                Duration::from_millis(cap),
                || merge_down_command(&doc, top).expect("combinar abajo"),
            );
            measures(
                &format!("combinar abajo {side}² ({scenario}): aplicar"),
                Duration::from_millis(10),
                || doc.apply(order).expect("aplicar"),
            );
        }
    }
}

#[test]
#[ignore = "rendimiento: umbrales de presupuesto, no de CI"]
fn flatten_and_merge_visible_8_layers_of_8192() {
    for (scenario, cap) in [('a', 500u64), ('b', 6000), ('c', 12000)] {
        let mut doc = document(8192, 8, scenario);
        let order = measures(
            &format!("combinar visibles 8×8192² ({scenario}): construir"),
            Duration::from_millis(cap),
            || merge_visible_command(&doc).expect("combinar visibles"),
        );
        measures(
            &format!("combinar visibles 8×8192² ({scenario}): aplicar"),
            Duration::from_millis(10),
            || doc.apply(order).expect("aplicar"),
        );
        // Deshacer y rehacer solo mueven.
        measures("deshacer", Duration::from_millis(10), || {
            doc.undo().expect("deshacer")
        });
        measures("rehacer", Duration::from_millis(10), || {
            doc.redo().expect("rehacer")
        });
        drop(doc);
        let doc = document(8192, 8, scenario);
        measures(
            &format!("aplanar 8×8192² ({scenario}): construir"),
            Duration::from_millis(cap),
            || flatten_command(&doc).expect("aplanar"),
        );
    }
}
