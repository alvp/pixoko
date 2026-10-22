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
//!   que egui entrega, aplicados en orden) es `from_rgba_unmultiplied(to_rgba8(document))` sin el
//!   damero, con cada fondo y cada camino; la imagen se sube una sola vez (un único delta completo
//!   y nada más mientras no hay superposición); con la superposición, fuera de los píxeles
//!   pendientes la textura sigue siendo el documento; y al vaciarla (`sync`, o `refresh`) la
//!   textura vuelve a ser el documento.
//!
//! Unidad 14a-2: la superposición ya no entra por `paint` sino por `Canvas::set_overlay`, llamado
//! tras `interact_roles` y antes de `sync` y `paint`; es parte de la textura y no hay formas extra.
//!
//! Suposición sobre «Piezas»: el fondo y el camino se eligen con `Canvas::set_background` y
//! `Canvas::set_checker_mesh`.

use px_core::ImageId;
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

const TEST_WIDTH: u32 = 16;
const TEST_HEIGHT: u32 = 12;

/// Los cinco modos de pintar el fondo: los cuatro fondos con la textura y el respaldo por malla.
const TEST_MODES: [(CanvasBackground, bool); 5] = [
    (CanvasBackground::Checker, false),
    (CanvasBackground::Black, false),
    (CanvasBackground::Gray, false),
    (CanvasBackground::White, false),
    (CanvasBackground::Checker, true),
];

/// RGBA de 16×12: las cuatro primeras columnas transparentes (alfa 0), las cuatro siguientes con
/// alfa 128 y el resto opacas; ningún píxel opaco tiene un gris del damero.
fn rgba_with_transparency() -> Image {
    let mut image = Image::new(TEST_WIDTH, TEST_HEIGHT, Mode::Rgba).expect("imagen RGBA");
    for y in 0..TEST_HEIGHT {
        for x in 0..TEST_WIDTH {
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
fn indexed_with_transparent() -> Image {
    let mut palette = Palette::from_colors([
        Rgba::opaque(10, 20, 30),
        Rgba::opaque(255, 0, 0),
        Rgba::opaque(0, 255, 0),
        Rgba::opaque(0, 0, 255),
    ])
    .expect("paleta");
    palette
        .set_transparent_index(Some(0))
        .expect("índice transparente");
    let mut image = Image::new_indexed(TEST_WIDTH, TEST_HEIGHT, palette).expect("imagen indexada");
    for y in 0..TEST_HEIGHT {
        for x in 0..TEST_WIDTH {
            let index_2 = if x < 4 { 0 } else { 1 + ((x + y) % 3) as u8 };
            image
                .set_pixel(x, y, Pixel::Indexed(index_2))
                .expect("dentro");
        }
    }
    image
}

/// Superposición del borrador (alfa 0) y del lápiz con alfa parcial sobre píxeles de la imagen.
fn overlay_2() -> Vec<(u32, u32, Rgba)> {
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
struct Textures {
    map: HashMap<TextureId, ([usize; 2], Vec<Color32>)>,
}

impl Textures {
    fn applies(&mut self, delta: &TexturesDelta) {
        for (id, deltas) in &delta.set {
            for delta in deltas {
                let ImageData::Color(image_2) = &delta.image;
                match delta.pos {
                    None => {
                        self.map.insert(*id, (image_2.size, image_2.pixels.clone()));
                    }
                    Some([x0, y0]) => {
                        let (size, texels) = self
                            .map
                            .get_mut(id)
                            .expect("un parche llega a una textura ya creada");
                        let [w, h] = image_2.size;
                        assert!(
                            x0 + w <= size[0] && y0 + h <= size[1],
                            "el parche {w}×{h} en ({x0}, {y0}) se sale de la textura {size:?}"
                        );
                        for y in 0..h {
                            let target_2 = (y0 + y) * size[0] + x0;
                            texels[target_2..target_2 + w]
                                .copy_from_slice(&image_2.pixels[y * w..(y + 1) * w]);
                        }
                    }
                }
            }
        }
        for id in &delta.free {
            self.map.remove(id);
        }
    }

    fn get(&self, id: TextureId) -> Option<&([usize; 2], Vec<Color32>)> {
        self.map.get(&id)
    }
}

/// Las subidas de un fotograma a la textura de la imagen (la baldosa 0, la única de estas
/// imágenes): deltas completos (`pos == None`) y parciales.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
struct Uploads {
    complete: usize,
    partial: usize,
}

/// Un fotograma sin ventana, en el orden de la app: `interact_roles` → `set_overlay` → `sync` → `paint`
/// (con `overlay` vacío, `set_overlay` es `clear_overlay`). Aplica los deltas de texturas a
/// `textures` y devuelve lo subido a la textura de la imagen.
fn frame(
    ctx: &Context,
    canvas: &mut Canvas,
    document: &Image,
    overlay: &[(u32, u32, Rgba)],
    textures: &mut Textures,
) -> Uploads {
    let input = RawInput {
        screen_rect: Some(Rect::from_min_size(Pos2::ZERO, vec2(200.0, 150.0))),
        ..RawInput::default()
    };
    let mut output = ctx.run_ui(input, |ui| {
        canvas.interact_roles(ui);
        canvas.set_overlay(ui.ctx(), document, overlay);
        canvas.sync(ui.ctx(), document).expect("sync");
        canvas.paint(ui);
    });
    let image_2 = canvas.texture_id().expect("textura de la imagen");
    let mut uploads = Uploads::default();
    for (id, deltas) in &output.textures_delta.set {
        if *id != image_2 {
            continue;
        }
        for delta in deltas {
            if delta.is_whole() {
                uploads.complete += 1;
            } else {
                uploads.partial += 1;
            }
        }
    }
    textures.applies(&output.textures_delta);
    output.textures_delta.clear();
    uploads
}

fn is_checker_gray(c: &[u8; 4]) -> bool {
    let (r, g, b, a) = (c[0], c[1], c[2], c[3]);
    a == 255
        && [CHECK_A, CHECK_B]
            .iter()
            .any(|gray| (gray.r, gray.g, gray.b) == (r, g, b))
}

/// La textura de la imagen compuesta, píxel a píxel, frente a lo esperado; los `salvo` (píxeles de
/// la superposición) se comprueban aparte.
fn texture_is(
    textures: &Textures,
    canvas: &Canvas,
    expected_2: &[Color32],
    salvo: &[(u32, u32, Rgba)],
    context: &str,
) {
    let id = canvas.texture_id().expect("textura de la imagen");
    let (size, texels) = textures.get(id).expect("la textura existe");
    assert_eq!(
        *size,
        [TEST_WIDTH as usize, TEST_HEIGHT as usize],
        "{context}: tamaño"
    );
    for y in 0..TEST_HEIGHT {
        for x in 0..TEST_WIDTH {
            let i = (y * TEST_WIDTH + x) as usize;
            // Con varios puntos en el mismo píxel gana el último, como al pintar el trazo.
            let pending_2 = salvo.iter().rev().find(|(px, py, _)| (*px, *py) == (x, y));
            let must = match pending_2 {
                Some((_, _, c)) => Color32::from_rgba_unmultiplied(c.r, c.g, c.b, c.a),
                None => expected_2[i],
            };
            assert_eq!(texels[i], must, "{context}: texel ({x}, {y})");
        }
    }
}

fn checks(name_2: &str, image: &Image) {
    let original = image.clone();
    let doc = Document::new(image.clone());
    let document = doc
        .image(ImageId::from_raw(0))
        .expect("imagen del documento");
    let state_id = doc.state_id();
    let image_bytes = to_rgba8(image);
    let png_antes = write_png(image).expect("PNG antes");

    let ctx = Context::default();
    let mut textures = Textures::default();
    let mut canvas = Canvas::new();
    canvas.set_image(document);
    let expected_2: Vec<Color32> = ColorImage::from_rgba_unmultiplied(
        [TEST_WIDTH as usize, TEST_HEIGHT as usize],
        &image_bytes,
    )
    .pixels;
    let overlay = overlay_2();
    let empty: &[(u32, u32, Rgba)] = &[];

    // Sin superposición: la imagen se sube una sola vez (un único delta completo) y después, con
    // cada fondo y cada camino, no se sube nada; la textura es la imagen, sin el damero.
    let mut image_uploads = Uploads::default();
    for (background_2, mesh) in TEST_MODES {
        canvas.set_background(background_2);
        canvas.set_checker_mesh(mesh);
        for _ in 0..2 {
            let uploads = frame(&ctx, &mut canvas, document, empty, &mut textures);
            image_uploads.complete += uploads.complete;
            image_uploads.partial += uploads.partial;
            texture_is(
                &textures,
                &canvas,
                &expected_2,
                &[],
                &format!("{name_2}: {background_2:?} malla {mesh}, sin superposición"),
            );
        }
    }
    assert_eq!(
        image_uploads,
        Uploads {
            complete: 1,
            partial: 0
        },
        "{name_2}: la imagen se sube una sola vez con todos los fondos"
    );

    // Con la superposición del borrador (alfa 0) y del lápiz (alfa parcial): el documento no
    // cambia y, fuera de los píxeles pendientes, la textura sigue siendo el documento; los
    // pendientes llevan su color (premultiplicado).
    for (background_2, mesh) in TEST_MODES {
        canvas.set_background(background_2);
        canvas.set_checker_mesh(mesh);
        for _ in 0..2 {
            frame(&ctx, &mut canvas, document, &overlay, &mut textures);
            texture_is(
                &textures,
                &canvas,
                &expected_2,
                &overlay,
                &format!("{name_2}: {background_2:?} malla {mesh}, con superposición"),
            );
            assert_eq!(doc.state_id(), state_id, "{name_2}: state_id durante");
            assert_eq!(
                to_rgba8(document),
                image_bytes,
                "{name_2}: documento durante"
            );
        }
    }

    // Al vaciarla, `sync` regenera los bloques tocados desde el documento: la textura vuelve a ser
    // `from_rgba_unmultiplied(to_rgba8(document))`; y lo mismo con `refresh`.
    frame(&ctx, &mut canvas, document, empty, &mut textures);
    texture_is(
        &textures,
        &canvas,
        &expected_2,
        &[],
        &format!("{name_2}: tras vaciar la superposición y sync"),
    );
    frame(&ctx, &mut canvas, document, &overlay, &mut textures);
    canvas.clear_overlay();
    canvas.refresh(&ctx, document);
    frame(&ctx, &mut canvas, document, empty, &mut textures);
    texture_is(
        &textures,
        &canvas,
        &expected_2,
        &[],
        &format!("{name_2}: tras la superposición y refresh"),
    );
    // En reposo no se sube nada más.
    assert_eq!(
        frame(&ctx, &mut canvas, document, empty, &mut textures),
        Uploads::default(),
        "{name_2}: en reposo, cero subidas"
    );

    // El PNG: los mismos bytes antes y después, y leídos de vuelta son la imagen original.
    let png_after = write_png(image).expect("PNG después");
    assert_eq!(png_after, png_antes, "{name_2}: mismos bytes del PNG");
    let read_2 = read_png(&png_after).expect("el PNG se lee");
    assert!(read_2.is_exact(), "{name_2}: sin conversiones al leer");
    // Una RGBA vuelve idéntica; una indexada, con su índice transparente convertido por el formato
    // en alfa 0 de la paleta (es lo que ya hacía `write_png`: no es cosa del damero), así que lo
    // que se compara es el contenido RGBA de cada píxel.
    if original.mode() == Mode::Rgba {
        assert_eq!(read_2.image, original, "{name_2}: la imagen original");
    }
    let read_2_2 = to_rgba8(&read_2.image);
    assert_eq!(read_2_2, image_bytes, "{name_2}: mismos bytes RGBA");
    for (i, pixel) in read_2_2.as_chunks::<4>().0.iter().enumerate() {
        assert!(
            !is_checker_gray(pixel),
            "{name_2}: el píxel {i} del PNG leído es un gris del damero"
        );
    }
    // Alfa 0 donde era 0.
    for y in 0..TEST_HEIGHT {
        for x in 0..4 {
            let i = ((y * TEST_WIDTH + x) * 4 + 3) as usize;
            assert_eq!(read_2_2[i], 0, "{name_2}: ({x}, {y}) sigue transparente");
        }
    }

    // La imagen, `to_rgba8` y el documento, intactos.
    assert_eq!(*image, original, "{name_2}: la imagen no cambia");
    assert_eq!(to_rgba8(image), image_bytes);
    assert_eq!(doc.state_id(), state_id, "{name_2}: state_id");
    assert!(!doc.can_undo());
}

#[test]
fn row_11_the_checker_is_not_in_the_png_of_an_rgba_with_alpha_0_and_alpha_128() {
    checks("RGBA", &rgba_with_transparency());
}

#[test]
fn row_11_the_checker_is_not_in_the_png_of_an_indexed_one_with_transparent_index() {
    checks("indexada", &indexed_with_transparent());
}

#[test]
fn row_11_the_test_images_have_alpha_0_and_alpha_128_and_no_checker_gray() {
    // Control de las imágenes de partida: sin esto la prueba de arriba no probaría nada.
    for (name_2, image) in [
        ("RGBA", rgba_with_transparency()),
        ("indexada", indexed_with_transparent()),
    ] {
        let bytes = to_rgba8(&image);
        assert!(
            bytes.as_chunks::<4>().0.iter().any(|p| p[3] == 0),
            "{name_2}: alfa 0"
        );
        assert!(
            bytes.as_chunks::<4>().0.iter().all(|p| !is_checker_gray(p)),
            "{name_2}"
        );
    }
    let rgba = to_rgba8(&rgba_with_transparency());
    assert!(
        rgba.as_chunks::<4>().0.iter().any(|p| p[3] == 128),
        "RGBA: alfa 128"
    );
}
