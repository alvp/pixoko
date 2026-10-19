//! Fila 11 de la unidad 17b (specs/hito-2-damero-lienzo.md): **el damero no está en el fichero
//! guardado**. Es solo visualización: pintar el lienzo con cada fondo y con la superposición no
//! cambia el documento, ni lo que hay en la textura de la imagen, ni los bytes de `write_png`.
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
//! - `to_rgba8` de la imagen y el `state_id` del documento, intactos, también **durante** la
//!   superposición;
//! - la textura de la imagen, intacta: la **textura compuesta** (los deltas completos y parciales
//!   que egui entrega, aplicados en orden) es `from_rgba_unmultiplied(to_rgba8(documento))` sin el
//!   damero, con cada fondo y cada camino; la imagen se sube una sola vez (un único delta completo
//!   y nada más mientras no hay superposición); con la superposición, fuera de los píxeles
//!   pendientes la textura sigue siendo el documento; y al vaciarla (`sync`, o `refresh`) la
//!   textura vuelve a ser el documento.
//!
//! Unidad 14a-2: la superposición ya no entra por `paint` sino por `Canvas::set_overlay`, llamado
//! tras `interact` y antes de `sync` y `paint`; es parte de la textura y no hay formas extra.
//!
//! Suposición sobre «Piezas»: el fondo y el camino se eligen con `Canvas::set_background` y
//! `Canvas::set_checker_mesh`.

use std::collections::HashMap;

use eframe::egui::{
    Color32, ColorImage, Context, ImageData, Pos2, RawInput, Rect, TextureId, TexturesDelta, vec2,
};
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

/// Lo que egui entrega a la GPU, aplicado en orden (completos y parciales, `free` incluidos): la
/// textura compuesta de cada `TextureId`. (El ayudante equivalente de `px-ui/tests/common` no se
/// puede usar desde `px-app`.)
#[derive(Default)]
struct Texturas {
    mapa: HashMap<TextureId, ([usize; 2], Vec<Color32>)>,
}

impl Texturas {
    fn aplica(&mut self, delta: &TexturesDelta) {
        for (id, deltas) in &delta.set {
            for delta in deltas {
                let ImageData::Color(imagen) = &delta.image;
                match delta.pos {
                    None => {
                        self.mapa.insert(*id, (imagen.size, imagen.pixels.clone()));
                    }
                    Some([x0, y0]) => {
                        let (tamano, texels) = self
                            .mapa
                            .get_mut(id)
                            .expect("un parche llega a una textura ya creada");
                        let [w, h] = imagen.size;
                        assert!(
                            x0 + w <= tamano[0] && y0 + h <= tamano[1],
                            "el parche {w}×{h} en ({x0}, {y0}) se sale de la textura {tamano:?}"
                        );
                        for y in 0..h {
                            let destino = (y0 + y) * tamano[0] + x0;
                            texels[destino..destino + w]
                                .copy_from_slice(&imagen.pixels[y * w..(y + 1) * w]);
                        }
                    }
                }
            }
        }
        for id in &delta.free {
            self.mapa.remove(id);
        }
    }

    fn get(&self, id: TextureId) -> Option<&([usize; 2], Vec<Color32>)> {
        self.mapa.get(&id)
    }
}

/// Las subidas de un fotograma a la textura de la imagen (la baldosa 0, la única de estas
/// imágenes): deltas completos (`pos == None`) y parciales.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
struct Subidas {
    completas: usize,
    parciales: usize,
}

/// Un fotograma sin ventana, en el orden de la app: `interact` → `set_overlay` → `sync` → `paint`
/// (con `overlay` vacío, `set_overlay` es `clear_overlay`). Aplica los deltas de texturas a
/// `texturas` y devuelve lo subido a la textura de la imagen.
fn fotograma(
    ctx: &Context,
    canvas: &mut Canvas,
    documento: &Image,
    overlay: &[(u32, u32, Rgba)],
    texturas: &mut Texturas,
) -> Subidas {
    let input = RawInput {
        screen_rect: Some(Rect::from_min_size(Pos2::ZERO, vec2(200.0, 150.0))),
        ..RawInput::default()
    };
    let mut output = ctx.run_ui(input, |ui| {
        canvas.interact(ui);
        canvas.set_overlay(ui.ctx(), documento, overlay);
        canvas.sync(ui.ctx(), documento).expect("sync");
        canvas.paint(ui);
    });
    let imagen = canvas.texture_id().expect("textura de la imagen");
    let mut subidas = Subidas::default();
    for (id, deltas) in &output.textures_delta.set {
        if *id != imagen {
            continue;
        }
        for delta in deltas {
            if delta.is_whole() {
                subidas.completas += 1;
            } else {
                subidas.parciales += 1;
            }
        }
    }
    texturas.aplica(&output.textures_delta);
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

/// La textura de la imagen compuesta, píxel a píxel, frente a lo esperado; los `salvo` (píxeles de
/// la superposición) se comprueban aparte.
fn textura_es(
    texturas: &Texturas,
    canvas: &Canvas,
    esperado: &[Color32],
    salvo: &[(u32, u32, Rgba)],
    contexto: &str,
) {
    let id = canvas.texture_id().expect("textura de la imagen");
    let (tamano, texels) = texturas.get(id).expect("la textura existe");
    assert_eq!(
        *tamano,
        [ANCHO as usize, ALTO as usize],
        "{contexto}: tamaño"
    );
    for y in 0..ALTO {
        for x in 0..ANCHO {
            let i = (y * ANCHO + x) as usize;
            // Con varios puntos en el mismo píxel gana el último, como al pintar el trazo.
            let pendiente = salvo.iter().rev().find(|(px, py, _)| (*px, *py) == (x, y));
            let debe = match pendiente {
                Some((_, _, c)) => Color32::from_rgba_unmultiplied(c.r, c.g, c.b, c.a),
                None => esperado[i],
            };
            assert_eq!(texels[i], debe, "{contexto}: texel ({x}, {y})");
        }
    }
}

fn comprueba(nombre: &str, image: &Image) {
    let original = image.clone();
    let doc = Document::new(image.clone());
    let documento = doc.image(doc.main_image()).expect("imagen del documento");
    let state_id = doc.state_id();
    let bytes_de_la_imagen = to_rgba8(image);
    let png_antes = write_png(image).expect("PNG antes");

    let ctx = Context::default();
    let mut texturas = Texturas::default();
    let mut canvas = Canvas::new();
    canvas.set_image(documento);
    let esperado: Vec<Color32> =
        ColorImage::from_rgba_unmultiplied([ANCHO as usize, ALTO as usize], &bytes_de_la_imagen)
            .pixels;
    let overlay = superposicion();
    let vacia: &[(u32, u32, Rgba)] = &[];

    // Sin superposición: la imagen se sube una sola vez (un único delta completo) y después, con
    // cada fondo y cada camino, no se sube nada; la textura es la imagen, sin el damero.
    let mut subidas_de_la_imagen = Subidas::default();
    for (fondo, malla) in MODOS {
        canvas.set_background(fondo);
        canvas.set_checker_mesh(malla);
        for _ in 0..2 {
            let subidas = fotograma(&ctx, &mut canvas, documento, vacia, &mut texturas);
            subidas_de_la_imagen.completas += subidas.completas;
            subidas_de_la_imagen.parciales += subidas.parciales;
            textura_es(
                &texturas,
                &canvas,
                &esperado,
                &[],
                &format!("{nombre}: {fondo:?} malla {malla}, sin superposición"),
            );
        }
    }
    assert_eq!(
        subidas_de_la_imagen,
        Subidas {
            completas: 1,
            parciales: 0
        },
        "{nombre}: la imagen se sube una sola vez con todos los fondos"
    );

    // Con la superposición del borrador (alfa 0) y del lápiz (alfa parcial): el documento no
    // cambia y, fuera de los píxeles pendientes, la textura sigue siendo el documento; los
    // pendientes llevan su color (premultiplicado).
    for (fondo, malla) in MODOS {
        canvas.set_background(fondo);
        canvas.set_checker_mesh(malla);
        for _ in 0..2 {
            fotograma(&ctx, &mut canvas, documento, &overlay, &mut texturas);
            textura_es(
                &texturas,
                &canvas,
                &esperado,
                &overlay,
                &format!("{nombre}: {fondo:?} malla {malla}, con superposición"),
            );
            assert_eq!(doc.state_id(), state_id, "{nombre}: state_id durante");
            assert_eq!(
                to_rgba8(documento),
                bytes_de_la_imagen,
                "{nombre}: documento durante"
            );
        }
    }

    // Al vaciarla, `sync` regenera los bloques tocados desde el documento: la textura vuelve a ser
    // `from_rgba_unmultiplied(to_rgba8(documento))`; y lo mismo con `refresh`.
    fotograma(&ctx, &mut canvas, documento, vacia, &mut texturas);
    textura_es(
        &texturas,
        &canvas,
        &esperado,
        &[],
        &format!("{nombre}: tras vaciar la superposición y sync"),
    );
    fotograma(&ctx, &mut canvas, documento, &overlay, &mut texturas);
    canvas.clear_overlay();
    canvas.refresh(&ctx, documento);
    fotograma(&ctx, &mut canvas, documento, vacia, &mut texturas);
    textura_es(
        &texturas,
        &canvas,
        &esperado,
        &[],
        &format!("{nombre}: tras la superposición y refresh"),
    );
    // En reposo no se sube nada más.
    assert_eq!(
        fotograma(&ctx, &mut canvas, documento, vacia, &mut texturas),
        Subidas::default(),
        "{nombre}: en reposo, cero subidas"
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
