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

fn documento(lado: u32, capas: usize, escenario: char) -> Document {
    let alfa_fondo = if escenario == 'c' { 180 } else { 255 };
    let base = Image::filled(lado, lado, Pixel::Rgba(Rgba::new(40, 90, 160, alfa_fondo)))
        .expect("imagen de la capa de abajo");
    let mut doc = Document::new(base);
    doc.set_cells_limit(usize::MAX);
    let modos = [
        BlendMode::Multiply,
        BlendMode::Screen,
        BlendMode::Overlay,
        BlendMode::Darken,
        BlendMode::Lighten,
        BlendMode::Add,
        BlendMode::Difference,
    ];
    for i in 1..capas {
        doc.apply(Command::AddLayer {
            name: format!("c{i}"),
            at: i,
        })
        .expect("añadir capa");
        let (id, cel) = (doc.layers()[i].id(), doc.layers()[i].cel());
        let alfa = if escenario == 'c' { 150 } else { 255 };
        let tramos: Vec<Span> = (0..lado)
            .map(|y| Span {
                y,
                x_start: 0,
                x_end: lado,
            })
            .collect();
        let color = Rgba::new((i * 30) as u8, (255 - i * 20) as u8, (i * 50) as u8, alfa);
        doc.apply(Command::FillSpans {
            image: cel,
            spans: tramos,
            pixel: Pixel::Rgba(color),
        })
        .expect("rellenar");
        if escenario != 'a' {
            doc.apply(Command::SetLayerOpacity {
                layer: id,
                opacity: 128,
            })
            .expect("opacidad");
            doc.apply(Command::SetLayerBlend {
                layer: id,
                blend: modos[(i - 1) % modos.len()],
            })
            .expect("mezcla");
        }
    }
    doc
}

fn mide<T>(etiqueta: &str, tope: Duration, f: impl FnOnce() -> T) -> T {
    let inicio = Instant::now();
    let valor = f();
    let tiempo = inicio.elapsed();
    println!(
        "{etiqueta}: {:.1} ms (tope {:.0} ms)",
        tiempo.as_secs_f64() * 1e3,
        tope.as_secs_f64() * 1e3
    );
    assert!(tiempo <= tope, "{etiqueta}: {tiempo:?} pasa de {tope:?}");
    valor
}

#[test]
#[ignore = "rendimiento: umbrales de presupuesto, no de CI"]
fn combinar_abajo_2_capas_de_2048_y_de_8192() {
    for (lado, topes) in [(2048u32, [150u64, 150, 150]), (8192, [2500, 2500, 2500])] {
        for (escenario, tope) in ['a', 'b', 'c'].into_iter().zip(topes) {
            let mut doc = documento(lado, 2, escenario);
            let arriba = doc.layers()[1].id();
            let orden = mide(
                &format!("combinar abajo {lado}² ({escenario}): construir"),
                Duration::from_millis(tope),
                || merge_down_command(&doc, arriba).expect("combinar abajo"),
            );
            mide(
                &format!("combinar abajo {lado}² ({escenario}): aplicar"),
                Duration::from_millis(10),
                || doc.apply(orden).expect("aplicar"),
            );
        }
    }
}

#[test]
#[ignore = "rendimiento: umbrales de presupuesto, no de CI"]
fn aplanar_y_combinar_visibles_8_capas_de_8192() {
    for (escenario, tope) in [('a', 500u64), ('b', 6000), ('c', 12000)] {
        let mut doc = documento(8192, 8, escenario);
        let orden = mide(
            &format!("combinar visibles 8×8192² ({escenario}): construir"),
            Duration::from_millis(tope),
            || merge_visible_command(&doc).expect("combinar visibles"),
        );
        mide(
            &format!("combinar visibles 8×8192² ({escenario}): aplicar"),
            Duration::from_millis(10),
            || doc.apply(orden).expect("aplicar"),
        );
        // Deshacer y rehacer solo mueven.
        mide("deshacer", Duration::from_millis(10), || {
            doc.undo().expect("deshacer")
        });
        mide("rehacer", Duration::from_millis(10), || {
            doc.redo().expect("rehacer")
        });
        drop(doc);
        let doc = documento(8192, 8, escenario);
        mide(
            &format!("aplanar 8×8192² ({escenario}): construir"),
            Duration::from_millis(tope),
            || flatten_command(&doc).expect("aplanar"),
        );
    }
}
