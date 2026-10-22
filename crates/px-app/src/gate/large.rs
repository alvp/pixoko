//! Fila 12 de la puerta del hito 2 (specs/hito-2-puerta.md): lienzos grandes, **`#[ignore]`**.
//!
//! RGBA de 8192² (el tope `MAX_OPEN_PIXELS`) e indexada de 6000², cada una en su test: generar y
//! guardar con `write_png`, `Session::load` + `install`, 3 gestos de lápiz de un píxel, una línea
//! horizontal de lado a lado, un relleno, `undo` ×3, `redo` ×3, `save` y relectura.
//!
//! Se lanzan a mano, una vez al cierre, en release y con un solo hilo:
//! `cargo test --release -p px-app -- --ignored --test-threads=1 grande`. En depuración tardan
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
struct Capas {
    ancho: usize,
    alto: usize,
    /// Bytes por píxel: 4 (RGBA) o 1 (índice).
    bpp: usize,
    fondo: [u8; 4],
    /// Los tres píxeles de lápiz, con su posición y su valor.
    lapiz: [((usize, usize), [u8; 4]); 3],
    y_linea: usize,
    linea: [u8; 4],
    relleno: [u8; 4],
}

impl Capas {
    fn nueva(ancho: usize, alto: usize, bpp: usize, valores: [[u8; 4]; 6]) -> Capas {
        Capas {
            ancho,
            alto,
            bpp,
            fondo: valores[0],
            lapiz: [
                ((100, 50), valores[1]),
                ((ancho - 200, alto / 2 - 100), valores[2]),
                ((ancho - 1, alto - 1), valores[3]),
            ],
            y_linea: alto / 2,
            linea: valores[4],
            relleno: valores[5],
        }
    }

    /// La fila `y` en el formato de `Image::row`, tras `k` gestos (0 a 5: tres píxeles, la línea y
    /// el relleno), en `fila`.
    fn fila(&self, y: usize, k: usize, fila: &mut Vec<u8>) {
        let n = self.bpp;
        fila.clear();
        for _ in 0..self.ancho {
            fila.extend_from_slice(&self.fondo[..n]);
        }
        for (i, ((x, py), valor)) in self.lapiz.iter().enumerate() {
            if k > i && *py == y {
                fila[x * n..(x + 1) * n].copy_from_slice(&valor[..n]);
            }
        }
        if k >= 4 && y == self.y_linea {
            for pixel in fila.chunks_exact_mut(n) {
                pixel.copy_from_slice(&self.linea[..n]);
            }
        }
        // El relleno toma lo que sigue siendo fondo en la mitad de arriba.
        if k >= 5 && y < self.y_linea {
            for pixel in fila.chunks_exact_mut(n) {
                if pixel == &self.fondo[..n] {
                    pixel.copy_from_slice(&self.relleno[..n]);
                }
            }
        }
    }

    /// Pánico, con la primera fila y columna que difieren, si la imagen no es el lienzo tras `k`
    /// gestos.
    fn verificar(&self, imagen: &Image, k: usize, ctx: &str) {
        assert_eq!(
            (imagen.width() as usize, imagen.height() as usize),
            (self.ancho, self.alto),
            "{ctx}: tamaño"
        );
        let n = self.bpp;
        let mut esperada = Vec::new();
        for y in 0..self.alto {
            self.fila(y, k, &mut esperada);
            let real = imagen.row(y as u32).expect("la fila existe");
            if real != &esperada[..] {
                let x = real
                    .iter()
                    .zip(&esperada)
                    .position(|(a, b)| a != b)
                    .unwrap_or(0)
                    / n;
                panic!(
                    "{ctx}: tras {k} gestos, el píxel ({x}, {y}) difiere: imagen {:?}, esperado {:?}",
                    &real[x * n..(x + 1) * n],
                    &esperada[x * n..(x + 1) * n]
                );
            }
        }
    }
}

/// Un gesto con la herramienta, por `ToolState::canvas_event` sobre el documento de la sesión.
fn gesto(
    tools: &mut ToolState,
    s: &mut Session,
    herramienta: Tool,
    eleccion: ColorChoice,
    eventos: Vec<CanvasEvent>,
) -> bool {
    let id = ImageId::from_raw(0);
    tools.apply_action(ToolAction::Select(herramienta));
    let mut cambio = false;
    for evento in eventos {
        cambio |= tools
            .canvas_event(&mut s.document, id, eleccion, evento)
            .unwrap_or_else(|e| panic!("el gesto dio error: {e}"));
    }
    let cambios = tools.take_changes();
    s.absorb(cambios);
    cambio
}

fn tomar(ctx: &str, que: &str, desde: Instant) {
    eprintln!("fila 12 · {ctx} · {que}: {:?}", desde.elapsed());
}

/// El recorrido de la fila 12 sobre `imagen` (el fondo ya construido).
fn recorrido(ctx: &str, imagen: Image, capas: &Capas, colores: [ColorChoice; 5]) {
    let t = Trabajo::nuevo();

    // Generar y guardar con write_png.
    let inicio = Instant::now();
    let bytes = write_png(&imagen).unwrap_or_else(|e| panic!("{ctx}: write_png: {e}"));
    drop(imagen);
    tomar(ctx, &format!("write_png ({} bytes)", bytes.len()), inicio);
    let ruta = t.escribir("grande.png", &bytes);
    drop(bytes);

    // `Session::load` + `install`.
    let mut s = sesion_vacia();
    let inicio = Instant::now();
    let cargado = s
        .load(&ruta, Canvas::MAX_SIDE, false)
        .unwrap_or_else(|e| panic!("{ctx}: load: {e}"));
    tomar(ctx, "load", inicio);
    let inicio = Instant::now();
    s.install(cargado);
    tomar(ctx, "install", inicio);
    {
        let imagen = s.document.image(ImageId::from_raw(0)).expect("imagen");
        capas.verificar(imagen, 0, &format!("{ctx}: tras abrir"));
    }

    // Tres píxeles de lápiz, una línea horizontal de lado a lado y un relleno.
    let mut tools = ToolState::new();
    let ancho = capas.ancho as i32;
    let alto_linea = capas.y_linea as i32;
    let gestos: Vec<(&str, Tool, ColorChoice, Vec<CanvasEvent>)> = vec![
        (
            "lápiz 1",
            Tool::Pencil,
            colores[0],
            pixel_de(capas.lapiz[0].0),
        ),
        (
            "lápiz 2",
            Tool::Pencil,
            colores[1],
            pixel_de(capas.lapiz[1].0),
        ),
        (
            "lápiz 3",
            Tool::Pencil,
            colores[2],
            pixel_de(capas.lapiz[2].0),
        ),
        (
            "línea",
            Tool::Line,
            colores[3],
            vec![
                CanvasEvent::StrokeBegin((0, alto_linea)),
                CanvasEvent::StrokeMove((ancho - 1, alto_linea)),
                CanvasEvent::StrokeEnd,
            ],
        ),
        (
            "relleno",
            Tool::Fill,
            colores[4],
            vec![CanvasEvent::StrokeBegin((5, 5)), CanvasEvent::StrokeEnd],
        ),
    ];
    for (nombre, herramienta, color, eventos) in gestos {
        let inicio = Instant::now();
        assert!(
            gesto(&mut tools, &mut s, herramienta, color, eventos),
            "{ctx}: el gesto {nombre} no cambió nada"
        );
        tomar(ctx, nombre, inicio);
    }
    let comprobar = |s: &Session, k: usize, que: &str| {
        let inicio = Instant::now();
        let imagen = s.document.image(ImageId::from_raw(0)).expect("imagen");
        capas.verificar(imagen, k, &format!("{ctx}: {que}"));
        tomar(ctx, &format!("comprobar {que}"), inicio);
    };
    comprobar(&s, 5, "tras los 5 gestos");

    // `undo` ×3 y `redo` ×3, con la imagen comprobada tras cada uno.
    for k in [4, 3, 2] {
        let inicio = Instant::now();
        s.undo().unwrap_or_else(|e| panic!("{ctx}: undo: {e}"));
        tomar(ctx, "undo", inicio);
        comprobar(&s, k, &format!("tras undo a {k} gestos"));
    }
    for k in [3, 4, 5] {
        let inicio = Instant::now();
        s.redo().unwrap_or_else(|e| panic!("{ctx}: redo: {e}"));
        tomar(ctx, "redo", inicio);
        comprobar(&s, k, &format!("tras redo a {k} gestos"));
    }

    // `save` y relectura.
    let inicio = Instant::now();
    let guardado = guardar(&mut s, t.ruta()).unwrap_or_else(|e| panic!("{ctx}: save: {e}"));
    tomar(ctx, "save", inicio);
    assert_eq!(
        guardado.path, ruta,
        "{ctx}: el save no sobrescribió el fichero abierto"
    );
    drop(s);
    let inicio = Instant::now();
    let leido = read_png(&std::fs::read(&ruta).expect("leer lo guardado"))
        .unwrap_or_else(|e| panic!("{ctx}: read_png: {e}"));
    tomar(ctx, "relectura", inicio);
    assert!(leido.report.is_empty(), "{ctx}: la relectura se convirtió");
    capas.verificar(&leido.image, 5, &format!("{ctx}: relectura"));
    assert_eq!(
        t.nombres(),
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

/// El recorrido de la fila 12 con una RGBA cuadrada de `lado` píxeles.
fn rgba(lado: usize) {
    let etiqueta = format!("RGBA {lado}²");
    // Fondo (10, 20, 30) y los colores de las ranuras 3 a 7 de RGBA.
    let valores = [
        [10, 20, 30, 255],
        [255, 0, 0, 255],
        [0, 255, 0, 255],
        [0, 0, 255, 255],
        [255, 255, 0, 255],
        [255, 0, 255, 255],
    ];
    let capas = Capas::nueva(lado, lado, 4, valores);
    let [r, g, b, a] = valores[0];
    let inicio = Instant::now();
    let imagen = Image::filled(lado as u32, lado as u32, Pixel::Rgba(Rgba::new(r, g, b, a)))
        .expect("imagen RGBA");
    tomar(&etiqueta, "generar", inicio);
    let colores = [3, 4, 5, 6, 7].map(ColorChoice::Slot);
    recorrido(&etiqueta, imagen, &capas, colores);
}

/// El recorrido de la fila 12 con una indexada cuadrada de `lado` píxeles.
fn indexada(lado: usize) {
    let etiqueta = format!("indexada {lado}²");
    // Seis entradas: el fondo en el índice 0 y un índice por gesto.
    let valores = [
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
    let capas = Capas::nueva(lado, lado, 1, indices);
    let paleta = Palette::from_colors(valores.iter().map(|c| Rgba::new(c[0], c[1], c[2], c[3])))
        .expect("paleta");
    let inicio = Instant::now();
    let imagen =
        Image::filled_indexed(lado as u32, lado as u32, paleta, 0).expect("imagen indexada");
    tomar(&etiqueta, "generar", inicio);
    let colores = [1, 2, 3, 4, 5].map(ColorChoice::Entry);
    recorrido(&etiqueta, imagen, &capas, colores);
}

#[test]
#[ignore = "lienzo grande: se lanza a mano, en release (cargo test --release -p px-app -- --ignored --test-threads=1 grande)"]
fn fila_12_grande_rgba_8192() {
    rgba(8192);
}

#[test]
#[ignore = "lienzo grande: se lanza a mano, en release (cargo test --release -p px-app -- --ignored --test-threads=1 grande)"]
fn fila_12_grande_indexada_6000() {
    indexada(6000);
}

/// Control de la fórmula cerrada y del recorrido, a escala pequeña y sin `#[ignore]`: lo mismo que
/// los dos grandes con 512² píxeles. Si la fórmula estuviera mal, los grandes fallarían por ella y no
/// por el código bajo prueba; así se ve aquí, en depuración y en el CI.
#[test]
fn fila_12_el_mismo_recorrido_a_escala_pequena_rgba() {
    rgba(512);
}

#[test]
fn fila_12_el_mismo_recorrido_a_escala_pequena_indexada() {
    indexada(512);
}
