//! Rendimiento de `ConvertAll` (specs/hito-3-combinar.md, «Rendimiento»): **umbrales de
//! presupuesto, no de CI** (`#[ignore]`). Se ejecuta a mano, en release:
//!
//! `cargo test --release -p px-core --test convert_all_perf -- --ignored --nocapture --test-threads=1`
//!
//! 8 capas de 4096² RGBA: aplicar a Gris con alfa (con `luma`), informe (`analyze_cels`, solo la
//! pasada 1) exacto, con pérdida y a indexado.

use std::time::{Duration, Instant};

use px_core::{Allowed, Command, Document, Image, Mode, Pixel, Rgba, Span, analyze_cels};

const LADO: u32 = 4096;

/// 8 capas de `LADO`² rellenas por bandas de 64 filas con colores de `colores(capa, banda)`.
fn documento(colores: impl Fn(usize, u32) -> Rgba) -> Document {
    let base = Image::new(LADO, LADO, Mode::Rgba).expect("imagen");
    let mut doc = Document::new(base);
    doc.set_cells_limit(usize::MAX);
    for capa in 0..8usize {
        if capa > 0 {
            doc.apply(Command::AddLayer {
                name: format!("c{capa}"),
                at: capa,
            })
            .expect("añadir");
        }
        let cel = doc.layers()[capa].cel();
        for banda in 0..LADO / 64 {
            let spans: Vec<Span> = (banda * 64..(banda + 1) * 64)
                .map(|y| Span {
                    y,
                    x_start: 0,
                    x_end: LADO,
                })
                .collect();
            doc.apply(Command::FillSpans {
                image: cel,
                spans,
                pixel: Pixel::Rgba(colores(capa, banda)),
            })
            .expect("rellenar");
        }
    }
    doc
}

fn mide<T>(etiqueta: &str, tope: Duration, f: impl FnOnce() -> T) -> T {
    let inicio = Instant::now();
    let valor = f();
    let tiempo = inicio.elapsed();
    println!(
        "{etiqueta}: {:.0} ms (tope {:.0} ms)",
        tiempo.as_secs_f64() * 1e3,
        tope.as_secs_f64() * 1e3
    );
    assert!(tiempo <= tope, "{etiqueta}: {tiempo:?} pasa de {tope:?}");
    valor
}

fn informe(doc: &Document, destino: Mode, permitido: Allowed) -> bool {
    let celdas: Vec<Option<&Image>> = doc.layers().iter().map(|l| doc.image(l.cel())).collect();
    analyze_cels(&celdas, (LADO, LADO), Mode::Rgba, destino, permitido).is_ok()
}

#[test]
#[ignore = "rendimiento: umbrales de presupuesto, no de CI"]
fn convert_all_8_capas_de_4096() {
    let gris = documento(|c, b| {
        let v = (c as u32 * 31 + b * 3) as u8;
        Rgba::new(v, v, v, 255)
    });
    let color = documento(|c, b| Rgba::new((c * 30) as u8, (b * 4) as u8, 90, 255));
    let paleta = documento(|c, b| {
        let k = ((c as u32 * 5 + b) % 30) as u8;
        Rgba::new(k * 8, 255 - k * 8, k, 255)
    });
    let permiso = Allowed {
        luma: true,
        drop_alpha: None,
    };

    assert!(mide(
        "informe exacto a Gris con alfa",
        Duration::from_millis(600),
        || { informe(&gris, Mode::GrayAlpha, Allowed::NOTHING) }
    ));
    assert!(!mide(
        "informe con pérdida a Gris con alfa",
        Duration::from_millis(600),
        || { informe(&color, Mode::GrayAlpha, Allowed::NOTHING) }
    ));
    assert!(mide(
        "informe exacto a Indexado",
        Duration::from_millis(3000),
        || { informe(&paleta, Mode::Indexed, Allowed::NOTHING) }
    ));
    let mut doc = color;
    mide(
        "aplicar a Gris con alfa (luma)",
        Duration::from_millis(1200),
        || {
            doc.apply(Command::ConvertAll {
                target: Mode::GrayAlpha,
                allowed: permiso,
            })
            .expect("aplicar")
        },
    );
    mide("deshacer", Duration::from_millis(10), || {
        doc.undo().expect("deshacer")
    });
    mide("rehacer", Duration::from_millis(10), || {
        doc.redo().expect("rehacer")
    });
    let mut doc = paleta;
    mide("aplicar a Indexado", Duration::from_millis(3000), || {
        doc.apply(Command::ConvertAll {
            target: Mode::Indexed,
            allowed: Allowed::NOTHING,
        })
        .expect("aplicar")
    });
}
