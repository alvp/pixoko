//! Rendimiento de `ConvertAll` (specs/hito-3-combinar.md, «Rendimiento»): **umbrales de
//! presupuesto, no de CI** (`#[ignore]`). Se ejecuta a mano, en release:
//!
//! `cargo test --release -p px-core --test convert_all_perf -- --ignored --nocapture --test-threads=1`
//!
//! 8 capas de 4096² RGBA: aplicar a Gris con alfa (con `luma`), informe (`analyze_cels`, solo la
//! pasada 1) exacto, con pérdida y a indexado.

use std::time::{Duration, Instant};

use px_core::{Allowed, Command, Document, Image, Mode, Pixel, Rgba, Span, analyze_cels};

const SIDE: u32 = 4096;

/// 8 capas de `SIDE`² rellenas por bandas de 64 filas con colores de `colors_2(layer, band)`.
fn document(colors_2: impl Fn(usize, u32) -> Rgba) -> Document {
    let base = Image::new(SIDE, SIDE, Mode::Rgba).expect("imagen");
    let mut doc = Document::new(base);
    doc.set_cells_limit(usize::MAX);
    for layer in 0..8usize {
        if layer > 0 {
            doc.apply(Command::AddLayer {
                name: format!("c{layer}"),
                at: layer,
            })
            .expect("añadir");
        }
        let cel = doc.layers()[layer].cel();
        for band in 0..SIDE / 64 {
            let spans: Vec<Span> = (band * 64..(band + 1) * 64)
                .map(|y| Span {
                    y,
                    x_start: 0,
                    x_end: SIDE,
                })
                .collect();
            doc.apply(Command::FillSpans {
                image: cel,
                spans,
                pixel: Pixel::Rgba(colors_2(layer, band)),
            })
            .expect("rellenar");
        }
    }
    doc
}

fn measures<T>(label: &str, cap: Duration, f: impl FnOnce() -> T) -> T {
    let start = Instant::now();
    let valor = f();
    let time = start.elapsed();
    println!(
        "{label}: {:.0} ms (tope {:.0} ms)",
        time.as_secs_f64() * 1e3,
        cap.as_secs_f64() * 1e3
    );
    assert!(time <= cap, "{label}: {time:?} pasa de {cap:?}");
    valor
}

fn report(doc: &Document, target: Mode, allowed: Allowed) -> bool {
    let cels: Vec<Option<&Image>> = doc.layers().iter().map(|l| doc.image(l.cel())).collect();
    analyze_cels(&cels, (SIDE, SIDE), Mode::Rgba, target, allowed).is_ok()
}

#[test]
#[ignore = "rendimiento: umbrales de presupuesto, no de CI"]
fn convert_all_8_layers_of_4096() {
    let gray = document(|c, b| {
        let v = (c as u32 * 31 + b * 3) as u8;
        Rgba::new(v, v, v, 255)
    });
    let color = document(|c, b| Rgba::new((c * 30) as u8, (b * 4) as u8, 90, 255));
    let palette_2 = document(|c, b| {
        let k = ((c as u32 * 5 + b) % 30) as u8;
        Rgba::new(k * 8, 255 - k * 8, k, 255)
    });
    let permission = Allowed {
        luma: true,
        drop_alpha: None,
    };

    assert!(measures(
        "informe exacto a Gris con alfa",
        Duration::from_millis(600),
        || { report(&gray, Mode::GrayAlpha, Allowed::NOTHING) }
    ));
    assert!(!measures(
        "informe con pérdida a Gris con alfa",
        Duration::from_millis(600),
        || { report(&color, Mode::GrayAlpha, Allowed::NOTHING) }
    ));
    assert!(measures(
        "informe exacto a Indexado",
        Duration::from_millis(3000),
        || { report(&palette_2, Mode::Indexed, Allowed::NOTHING) }
    ));
    let mut doc = color;
    measures(
        "aplicar a Gris con alfa (luma)",
        Duration::from_millis(1200),
        || {
            doc.apply(Command::ConvertAll {
                target: Mode::GrayAlpha,
                allowed: permission,
            })
            .expect("aplicar")
        },
    );
    measures("deshacer", Duration::from_millis(10), || {
        doc.undo().expect("deshacer")
    });
    measures("rehacer", Duration::from_millis(10), || {
        doc.redo().expect("rehacer")
    });
    let mut doc = palette_2;
    measures("aplicar a Indexado", Duration::from_millis(3000), || {
        doc.apply(Command::ConvertAll {
            target: Mode::Indexed,
            allowed: Allowed::NOTHING,
        })
        .expect("aplicar")
    });
}
