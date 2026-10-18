//! Fila 11 de la unidad 17b (specs/hito-2-damero-lienzo.md): **el damero no está en el fichero
//! guardado**. Es solo visualización: pintar el lienzo con cada fondo y con la superposición no
//! cambia el documento, ni lo que sube a la textura de la imagen, ni los bytes de `write_png`.
//!
//! Va en `px-app` y no en `px-ui/tests/` porque necesita `px-formats` (`write_png` y `read_png`),
//! que `px-ui` no tiene como dependencia. Sin ventana: `egui::Context::run_ui` con `RawInput`
//! sintético, sin GPU ni capturas.
//!
//! Para una imagen RGBA con píxeles de alfa 0 y de alfa 128 y para una indexada con índice
//! transparente: se guarda el PNG, se pintan varios fotogramas con cada fondo (`Checker`, `Black`,
//! `Gray`, `White` y el respaldo por malla), con la superposición del borrador (alfa 0) y del lápiz
//! con alfa parcial y sin ella, y se vuelve a guardar. Debe dar:
//!
//! - `write_png` **los mismos bytes** antes y después;
//! - `read_png` de esos bytes devuelve la imagen original (la RGBA, idéntica; la indexada, con los
//!   mismos píxeles RGBA: el formato guarda el índice transparente como alfa 0 de la paleta), con
//!   alfa 0 donde era 0 y sin ningún gris `CHECK_A` ni `CHECK_B`;
//! - `to_rgba8` de la imagen y la textura que sube el lienzo, intactas (la imagen se sube una sola
//!   vez, con sus bytes), y el `state_id` del documento, igual.
//!
//! Suposición sobre «Piezas»: el fondo y el camino se eligen con `Canvas::set_background` y
//! `Canvas::set_checker_mesh`.

use eframe::egui::{ColorImage, Context, ImageData, Pos2, RawInput, Rect, TextureId, vec2};
use px_core::{Document, Image, Mode, Palette, Pixel, Rgba};
use px_formats::{read_png, write_png};
use px_ui::background::CanvasBackground;
use px_ui::canvas::Canvas;
use px_ui::palette_grid::{CHECK_A, CHECK_B};
use px_ui::texture::to_rgba8;

const ANCHO: u32 = 16;
const ALTO: u32 = 12;

/// Los cinco modos de pintar el fondo: los cuatro fondos con la textura y el respaldo por malla.
const MODOS: [(CanvasBackground, bool); 5] = [
    (CanvasBackground::Checker, false),
    (CanvasBackground::Black, false),
    (CanvasBackground::Gray, false),
    (CanvasBackground::White, false),
    (CanvasBackground::Checker, true),
];

/// RGBA de 16×12: las cuatro primeras columnas transparentes (alfa 0), las cuatro siguientes con
/// alfa 128 y el resto opacas; ningún píxel opaco tiene un gris del damero.
fn rgba_con_transparencia() -> Image {
    let mut image = Image::new(ANCHO, ALTO, Mode::Rgba).expect("imagen RGBA");
    for y in 0..ALTO {
        for x in 0..ANCHO {
            let color = match x {
                0..=3 => Rgba::new(0, 0, 0, 0),
                4..=7 => Rgba::new(200, 10, 10, 128),
                _ => Rgba::opaque(10, 200 - y as u8, 30 + x as u8),
            };
            image.set_pixel(x, y, Pixel::Rgba(color)).expect("dentro");
        }
    }
    image
}

/// Indexada de 16×12 con cuatro entradas y la 0 como índice transparente: las cuatro primeras
/// columnas valen el índice transparente.
fn indexada_con_transparente() -> Image {
    let mut paleta = Palette::from_colors([
        Rgba::opaque(10, 20, 30),
        Rgba::opaque(255, 0, 0),
        Rgba::opaque(0, 255, 0),
        Rgba::opaque(0, 0, 255),
    ])
    .expect("paleta");
    paleta
        .set_transparent_index(Some(0))
        .expect("índice transparente");
    let mut image = Image::new_indexed(ANCHO, ALTO, paleta).expect("imagen indexada");
    for y in 0..ALTO {
        for x in 0..ANCHO {
            let indice = if x < 4 { 0 } else { 1 + ((x + y) % 3) as u8 };
            image
                .set_pixel(x, y, Pixel::Indexed(indice))
                .expect("dentro");
        }
    }
    image
}

/// Superposición del borrador (alfa 0) y del lápiz con alfa parcial sobre píxeles de la imagen.
fn superposicion() -> Vec<(u32, u32, Rgba)> {
    let mut v = Vec::new();
    for x in 2..9 {
        v.push((x, 3, Rgba::new(0, 0, 0, 0)));
    }
    for x in 6..12 {
        v.push((x, 6, Rgba::new(255, 0, 0, 128)));
    }
    v.push((14, 9, Rgba::opaque(255, 0, 0)));
    v
}

/// Un fotograma sin ventana: `interact` y `paint`. Devuelve lo subido a texturas.
fn fotograma(
    ctx: &Context,
    canvas: &mut Canvas,
    overlay: &[(u32, u32, Rgba)],
) -> Vec<(TextureId, ImageData)> {
    let input = RawInput {
        screen_rect: Some(Rect::from_min_size(Pos2::ZERO, vec2(200.0, 150.0))),
        ..RawInput::default()
    };
    let mut output = ctx.run_ui(input, |ui| {
        canvas.interact(ui);
        canvas.paint(ui, overlay.iter().copied());
    });
    let mut subidas = Vec::new();
    for (id, deltas) in &output.textures_delta.set {
        for delta in deltas {
            subidas.push((*id, delta.image.clone()));
        }
    }
    output.textures_delta.clear();
    subidas
}

fn es_gris_del_damero(c: &[u8; 4]) -> bool {
    let (r, g, b, a) = (c[0], c[1], c[2], c[3]);
    a == 255
        && [CHECK_A, CHECK_B]
            .iter()
            .any(|gris| (gris.r, gris.g, gris.b) == (r, g, b))
}

fn comprueba(nombre: &str, image: &Image) {
    let original = image.clone();
    let doc = Document::new(image.clone());
    let state_id = doc.state_id();
    let bytes_de_la_imagen = to_rgba8(image);
    let png_antes = write_png(image).expect("PNG antes");

    let ctx = Context::default();
    let mut canvas = Canvas::new();
    canvas.set_image(image);
    let esperado =
        ColorImage::from_rgba_unmultiplied([ANCHO as usize, ALTO as usize], &bytes_de_la_imagen);
    let mut subidas_de_la_imagen = 0;
    let overlay = superposicion();
    for (fondo, malla) in MODOS {
        canvas.set_background(fondo);
        canvas.set_checker_mesh(malla);
        for con_superposicion in [false, true] {
            for _ in 0..2 {
                let vacia: &[(u32, u32, Rgba)] = &[];
                let subidas = fotograma(
                    &ctx,
                    &mut canvas,
                    if con_superposicion { &overlay } else { vacia },
                );
                let imagen = canvas.texture_id().expect("textura de la imagen");
                for (id, datos) in subidas {
                    if id == imagen {
                        subidas_de_la_imagen += 1;
                        let ImageData::Color(color) = datos;
                        assert_eq!(
                            color.pixels, esperado.pixels,
                            "{nombre}: lo que sube el lienzo es la imagen, sin el damero"
                        );
                    }
                }
            }
        }
    }
    assert_eq!(
        subidas_de_la_imagen, 1,
        "{nombre}: la imagen se sube una sola vez con todos los fondos y superposiciones"
    );

    // El PNG: los mismos bytes antes y después, y leídos de vuelta son la imagen original.
    let png_despues = write_png(image).expect("PNG después");
    assert_eq!(png_despues, png_antes, "{nombre}: mismos bytes del PNG");
    let leido = read_png(&png_despues).expect("el PNG se lee");
    assert!(leido.is_exact(), "{nombre}: sin conversiones al leer");
    // Una RGBA vuelve idéntica; una indexada, con su índice transparente convertido por el formato
    // en alfa 0 de la paleta (es lo que ya hacía `write_png`: no es cosa del damero), así que lo
    // que se compara es el contenido RGBA de cada píxel.
    if original.mode() == Mode::Rgba {
        assert_eq!(leido.image, original, "{nombre}: la imagen original");
    }
    let leidos = to_rgba8(&leido.image);
    assert_eq!(leidos, bytes_de_la_imagen, "{nombre}: mismos bytes RGBA");
    for (i, pixel) in leidos.as_chunks::<4>().0.iter().enumerate() {
        assert!(
            !es_gris_del_damero(pixel),
            "{nombre}: el píxel {i} del PNG leído es un gris del damero"
        );
    }
    // Alfa 0 donde era 0.
    for y in 0..ALTO {
        for x in 0..4 {
            let i = ((y * ANCHO + x) * 4 + 3) as usize;
            assert_eq!(leidos[i], 0, "{nombre}: ({x}, {y}) sigue transparente");
        }
    }

    // La imagen, `to_rgba8` y el documento, intactos.
    assert_eq!(*image, original, "{nombre}: la imagen no cambia");
    assert_eq!(to_rgba8(image), bytes_de_la_imagen);
    assert_eq!(doc.state_id(), state_id, "{nombre}: state_id");
    assert!(!doc.can_undo());
}

#[test]
fn fila_11_el_damero_no_esta_en_el_png_de_una_rgba_con_alfa_0_y_alfa_128() {
    comprueba("RGBA", &rgba_con_transparencia());
}

#[test]
fn fila_11_el_damero_no_esta_en_el_png_de_una_indexada_con_indice_transparente() {
    comprueba("indexada", &indexada_con_transparente());
}

#[test]
fn fila_11_las_imagenes_de_la_prueba_tienen_alfa_0_y_alfa_128_y_ningun_gris_del_damero() {
    // Control de las imágenes de partida: sin esto la prueba de arriba no probaría nada.
    for (nombre, image) in [
        ("RGBA", rgba_con_transparencia()),
        ("indexada", indexada_con_transparente()),
    ] {
        let bytes = to_rgba8(&image);
        assert!(
            bytes.as_chunks::<4>().0.iter().any(|p| p[3] == 0),
            "{nombre}: alfa 0"
        );
        assert!(
            bytes
                .as_chunks::<4>()
                .0
                .iter()
                .all(|p| !es_gris_del_damero(p)),
            "{nombre}"
        );
    }
    let rgba = to_rgba8(&rgba_con_transparencia());
    assert!(
        rgba.as_chunks::<4>().0.iter().any(|p| p[3] == 128),
        "RGBA: alfa 128"
    );
}
