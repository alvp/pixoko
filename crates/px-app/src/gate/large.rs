//! Fila 12 de la puerta del hito 2 (specs/hito-2-puerta.md): lienzos grandes, **`#[ignore]`**.
//!
//! RGBA de 8192² (el tope `MAX_OPEN_PIXELS`) e indexada de 6000², cada una en su test: generar y
//! guardar con `write_png`, `Session::load` + `install`, 3 gestos de lápiz de un píxel, una línea
//! horizontal de lado a lado, un relleno, `undo` ×3, `redo` ×3, `save` y relectura.
//!
//! Se lanzan a mano, una vez al cierre, en release y con un solo hilo:
//! `cargo test --release -p px-app -- --ignored --test-threads=1 large`. En depuración tardan
//! mucho y gastan ~1 GiB: no se ejecutan con `cargo test` a secas.
//!
//! Lo esperado sale de una **fórmula cerrada** sobre `Image::row`: el fondo, los tres píxeles del
//! lápiz, la línea y el relleno por capas, fila a fila; no hay un segundo relleno. El relleno parte
//! de la esquina superior izquierda, así que la línea de lado a lado lo deja en la mitad de arriba y
//! los píxeles de lápiz de esa mitad quedan como agujeros.
//!
//! Sin aserciones de tiempo (serían frágiles): los tiempos van por `eprintln!`. Todo se escribe en un
//! directorio de trabajo que se borra al terminar.

use px_core::ImageId;
use std::time::Instant;

use px_core::{Image, Palette, Pixel, Rgba};
use px_formats::{read_png, write_png};
use px_ui::canvas::{Canvas, CanvasEvent};
use px_ui::color::ColorChoice;
use px_ui::tool::{Tool, ToolAction, ToolState};

use super::common::*;
use crate::session::Session;

/// El lienzo como capas de gestos: la fila `y` tras `k` gestos se calcula sin tocar el documento.
struct Layers {
    width: usize,
    height: usize,
    /// Bytes por píxel: 4 (RGBA) o 1 (índice).
    bpp: usize,
    background_2: [u8; 4],
    /// Los tres píxeles de lápiz, con su posición y su valor.
    pencil: [((usize, usize), [u8; 4]); 3],
    y_line: usize,
    line: [u8; 4],
    fill: [u8; 4],
}

impl Layers {
    fn new(width: usize, height: usize, bpp: usize, values_2: [[u8; 4]; 6]) -> Layers {
        Layers {
            width,
            height,
            bpp,
            background_2: values_2[0],
            pencil: [
                ((100, 50), values_2[1]),
                ((width - 200, height / 2 - 100), values_2[2]),
                ((width - 1, height - 1), values_2[3]),
            ],
            y_line: height / 2,
            line: values_2[4],
            fill: values_2[5],
        }
    }

    /// La fila `y` en el formato de `Image::row`, tras `k` gestos (0 a 5: tres píxeles, la línea y
    /// el relleno), en `row`.
    fn row(&self, y: usize, k: usize, row: &mut Vec<u8>) {
        let n = self.bpp;
        row.clear();
        for _ in 0..self.width {
            row.extend_from_slice(&self.background_2[..n]);
        }
        for (i, ((x, py), valor)) in self.pencil.iter().enumerate() {
            if k > i && *py == y {
                row[x * n..(x + 1) * n].copy_from_slice(&valor[..n]);
            }
        }
        if k >= 4 && y == self.y_line {
            for pixel in row.chunks_exact_mut(n) {
                pixel.copy_from_slice(&self.line[..n]);
            }
        }
        // El relleno toma lo que sigue siendo fondo en la mitad de arriba.
        if k >= 5 && y < self.y_line {
            for pixel in row.chunks_exact_mut(n) {
                if pixel == &self.background_2[..n] {
                    pixel.copy_from_slice(&self.fill[..n]);
                }
            }
        }
    }

    /// Pánico, con la primera fila y columna que difieren, si la imagen no es el lienzo tras `k`
    /// gestos.
    fn verify(&self, image_2: &Image, k: usize, ctx: &str) {
        assert_eq!(
            (image_2.width() as usize, image_2.height() as usize),
            (self.width, self.height),
            "{ctx}: tamaño"
        );
        let n = self.bpp;
        let mut expected = Vec::new();
        for y in 0..self.height {
            self.row(y, k, &mut expected);
            let real = image_2.row(y as u32).expect("la fila existe");
            if real != &expected[..] {
                let x = real
                    .iter()
                    .zip(&expected)
                    .position(|(a, b)| a != b)
                    .unwrap_or(0)
                    / n;
                panic!(
                    "{ctx}: tras {k} gestos, el píxel ({x}, {y}) difiere: imagen {:?}, esperado {:?}",
                    &real[x * n..(x + 1) * n],
                    &expected[x * n..(x + 1) * n]
                );
            }
        }
    }
}

/// Un gesto con la herramienta, por `ToolState::canvas_event` sobre el documento de la sesión.
fn gesture(
    tools: &mut ToolState,
    s: &mut Session,
    tool_2: Tool,
    choice: ColorChoice,
    events: Vec<CanvasEvent>,
) -> bool {
    let id = ImageId::from_raw(0);
    tools.apply_action(ToolAction::Select(tool_2));
    let mut change = false;
    for event_2 in events {
        change |= tools
            .canvas_event(&mut s.document, id, choice, event_2)
            .unwrap_or_else(|e| panic!("el gesto dio error: {e}"));
    }
    let changes_2 = tools.take_changes();
    s.absorb(changes_2);
    change
}

fn take(ctx: &str, that: &str, from_2: Instant) {
    eprintln!("fila 12 · {ctx} · {that}: {:?}", from_2.elapsed());
}

/// El recorrido de la fila 12 sobre `image_2` (el fondo ya construido).
fn walk(ctx: &str, image_2: Image, layers_2: &Layers, colors: [ColorChoice; 5]) {
    let t = Work::new_2();

    // Generar y guardar con write_png.
    let start_2 = Instant::now();
    let bytes = write_png(&image_2).unwrap_or_else(|e| panic!("{ctx}: write_png: {e}"));
    drop(image_2);
    take(ctx, &format!("write_png ({} bytes)", bytes.len()), start_2);
    let path_2 = t.write("grande.png", &bytes);
    drop(bytes);

    // `Session::load` + `install`.
    let mut s = session_empty();
    let start_2 = Instant::now();
    let loaded = s
        .load(&path_2, Canvas::MAX_SIDE, false)
        .unwrap_or_else(|e| panic!("{ctx}: load: {e}"));
    take(ctx, "load", start_2);
    let start_2 = Instant::now();
    s.install(loaded);
    take(ctx, "install", start_2);
    {
        let image_2 = s.document.image(ImageId::from_raw(0)).expect("imagen");
        layers_2.verify(image_2, 0, &format!("{ctx}: tras abrir"));
    }

    // Tres píxeles de lápiz, una línea horizontal de lado a lado y un relleno.
    let mut tools = ToolState::new();
    let width = layers_2.width as i32;
    let height_line = layers_2.y_line as i32;
    let gestures: Vec<(&str, Tool, ColorChoice, Vec<CanvasEvent>)> = vec![
        (
            "lápiz 1",
            Tool::Pencil,
            colors[0],
            pixel_de(layers_2.pencil[0].0),
        ),
        (
            "lápiz 2",
            Tool::Pencil,
            colors[1],
            pixel_de(layers_2.pencil[1].0),
        ),
        (
            "lápiz 3",
            Tool::Pencil,
            colors[2],
            pixel_de(layers_2.pencil[2].0),
        ),
        (
            "línea",
            Tool::Line,
            colors[3],
            vec![
                CanvasEvent::StrokeBegin((0, height_line)),
                CanvasEvent::StrokeMove((width - 1, height_line)),
                CanvasEvent::StrokeEnd,
            ],
        ),
        (
            "relleno",
            Tool::Fill,
            colors[4],
            vec![CanvasEvent::StrokeBegin((5, 5)), CanvasEvent::StrokeEnd],
        ),
    ];
    for (name_2, tool_2, color, events) in gestures {
        let start_2 = Instant::now();
        assert!(
            gesture(&mut tools, &mut s, tool_2, color, events),
            "{ctx}: el gesto {name_2} no cambió nada"
        );
        take(ctx, name_2, start_2);
    }
    let check = |s: &Session, k: usize, that: &str| {
        let start_2 = Instant::now();
        let image_2 = s.document.image(ImageId::from_raw(0)).expect("imagen");
        layers_2.verify(image_2, k, &format!("{ctx}: {that}"));
        take(ctx, &format!("comprobar {that}"), start_2);
    };
    check(&s, 5, "tras los 5 gestos");

    // `undo` ×3 y `redo` ×3, con la imagen comprobada tras cada uno.
    for k in [4, 3, 2] {
        let start_2 = Instant::now();
        s.undo().unwrap_or_else(|e| panic!("{ctx}: undo: {e}"));
        take(ctx, "undo", start_2);
        check(&s, k, &format!("tras undo a {k} gestos"));
    }
    for k in [3, 4, 5] {
        let start_2 = Instant::now();
        s.redo().unwrap_or_else(|e| panic!("{ctx}: redo: {e}"));
        take(ctx, "redo", start_2);
        check(&s, k, &format!("tras redo a {k} gestos"));
    }

    // `save` y relectura.
    let start_2 = Instant::now();
    let saved = save(&mut s, t.path_2()).unwrap_or_else(|e| panic!("{ctx}: save: {e}"));
    take(ctx, "save", start_2);
    assert_eq!(
        saved.path, path_2,
        "{ctx}: el save no sobrescribió el fichero abierto"
    );
    drop(s);
    let start_2 = Instant::now();
    let read_2 = read_png(&std::fs::read(&path_2).expect("leer lo guardado"))
        .unwrap_or_else(|e| panic!("{ctx}: read_png: {e}"));
    take(ctx, "relectura", start_2);
    assert!(read_2.report.is_empty(), "{ctx}: la relectura se convirtió");
    layers_2.verify(&read_2.image, 5, &format!("{ctx}: relectura"));
    assert_eq!(
        t.names(),
        vec!["grande.png".to_string()],
        "{ctx}: ficheros del trabajo"
    );
    // El directorio de trabajo se borra al soltar `t`.
}

/// Un lápiz de un solo píxel: pulsar y soltar en esa posición.
fn pixel_de((x, y): (usize, usize)) -> Vec<CanvasEvent> {
    vec![
        CanvasEvent::StrokeBegin((x as i32, y as i32)),
        CanvasEvent::StrokeEnd,
    ]
}

/// El recorrido de la fila 12 con una RGBA cuadrada de `side_2` píxeles.
fn rgba(side_2: usize) {
    let label_2 = format!("RGBA {side_2}²");
    // Fondo (10, 20, 30) y los colores de las ranuras 3 a 7 de RGBA.
    let values_2 = [
        [10, 20, 30, 255],
        [255, 0, 0, 255],
        [0, 255, 0, 255],
        [0, 0, 255, 255],
        [255, 255, 0, 255],
        [255, 0, 255, 255],
    ];
    let layers_2 = Layers::new(side_2, side_2, 4, values_2);
    let [r, g, b, a] = values_2[0];
    let start_2 = Instant::now();
    let image_2 = Image::filled(
        side_2 as u32,
        side_2 as u32,
        Pixel::Rgba(Rgba::new(r, g, b, a)),
    )
    .expect("imagen RGBA");
    take(&label_2, "generar", start_2);
    let colors = [3, 4, 5, 6, 7].map(ColorChoice::Slot);
    walk(&label_2, image_2, &layers_2, colors);
}

/// El recorrido de la fila 12 con una indexada cuadrada de `side_2` píxeles.
fn indexed(side_2: usize) {
    let label_2 = format!("indexada {side_2}²");
    // Seis entradas: el fondo en el índice 0 y un índice por gesto.
    let values_2 = [
        [10, 20, 30, 255],
        [255, 0, 0, 255],
        [0, 255, 0, 255],
        [0, 0, 255, 255],
        [255, 255, 0, 255],
        [255, 0, 255, 255],
    ];
    let indices = [
        [0, 0, 0, 0],
        [1, 0, 0, 0],
        [2, 0, 0, 0],
        [3, 0, 0, 0],
        [4, 0, 0, 0],
        [5, 0, 0, 0],
    ];
    let layers_2 = Layers::new(side_2, side_2, 1, indices);
    let palette = Palette::from_colors(values_2.iter().map(|c| Rgba::new(c[0], c[1], c[2], c[3])))
        .expect("paleta");
    let start_2 = Instant::now();
    let image_2 =
        Image::filled_indexed(side_2 as u32, side_2 as u32, palette, 0).expect("imagen indexada");
    take(&label_2, "generar", start_2);
    let colors = [1, 2, 3, 4, 5].map(ColorChoice::Entry);
    walk(&label_2, image_2, &layers_2, colors);
}

#[test]
#[ignore = "lienzo grande: se lanza a mano, en release (cargo test --release -p px-app -- --ignored --test-threads=1 grande)"]
fn row_12_large_rgba_8192() {
    rgba(8192);
}

#[test]
#[ignore = "lienzo grande: se lanza a mano, en release (cargo test --release -p px-app -- --ignored --test-threads=1 grande)"]
fn row_12_large_indexed_6000() {
    indexed(6000);
}

/// Control de la fórmula cerrada y del recorrido, a escala pequeña y sin `#[ignore]`: lo mismo que
/// los dos grandes con 512² píxeles. Si la fórmula estuviera mal, los grandes fallarían por ella y no
/// por el código bajo prueba; así se ve aquí, en depuración y en el CI.
#[test]
fn row_12_the_same_walk_at_small_rgba_scale() {
    rgba(512);
}

#[test]
fn row_12_the_same_walk_at_small_indexed_scale() {
    indexed(512);
}
