//! Tests de la vista previa de la edición de un color (specs/hito-2-editar-color.md: decisión 7,
//! filas 4, 13 y 19 de «Casos límite» y propiedad 4a).
//!
//! Dos partes:
//!
//! - `texture::to_rgba8_with_entry`, función pura: los bytes que daría `to_rgba8` si la entrada
//!   `index` de la paleta valiera `color`, sin clonar ni tocar la imagen. La referencia de las
//!   pruebas es `to_rgba8` de una copia con `Command::SetPaletteColor` aplicado por `Document`.
//! - `Canvas::preview_entry`, `end_preview` y `preview_uploads`, con un `egui::Context` sin
//!   ventana (como `tests/canvas.rs` y `tests/palette_panel.rs`).
//!
//! Cómo se lee la textura: `egui` no deja leer el contenido de una textura desde el `Context`
//! (`Context::tex_manager` solo guarda metadatos: nombre, tamaño y bytes por píxel). Lo que sí
//! se ve es el `TexturesDelta` de cada fotograma (`FullOutput::textures_delta`): lo que `egui`
//! entregaría a la GPU. El banco los aplica **en orden** con `common::textures::Textures` (el
//! ayudante de la unidad 14a: componen la textura con deltas completos y parciales, y una textura
//! por baldosa) y así se puede comparar el contenido de la textura del lienzo con el esperado, y
//! contar cuántos deltas llegaron en un fotograma (cero = no se subió nada). Con las imágenes de
//! este fichero (una sola baldosa, visible entera) `texture_id()` es la textura de la única
//! baldosa y un cambio de valor sigue siendo un delta completo. Los píxeles de un `ColorImage`
//! son `Color32` con alfa premultiplicado, así que la comparación se hace contra
//! `ColorImage::from_rgba_unmultiplied` de los bytes esperados (un píxel de alfa 0 pierde su RGB
//! en la textura: por eso la fila 13 mira los bytes de `to_rgba8_with_entry` y, aparte, el alfa
//! del píxel de la textura).

use egui::{Color32, ColorImage, Context, FullOutput, Pos2, RawInput, Rect, Vec2, vec2};
use proptest::collection::vec;
use proptest::prelude::*;
use px_core::ImageId;
use px_core::{Command, Document, Image, Mode, Palette, Pixel, Rgba};
use px_ui::canvas::Canvas;
use px_ui::texture::{to_rgba8, to_rgba8_with_entry};

mod common;
use common::textures::Textures;

// ---------------------------------------------------------------------------------------------
// Imágenes de prueba
// ---------------------------------------------------------------------------------------------

const RED: Rgba = Rgba::opaque(255, 0, 0);
const GREEN: Rgba = Rgba::opaque(0, 255, 0);
const BLUE: Rgba = Rgba::opaque(0, 0, 255);
const TEST_BLACK: Rgba = Rgba::opaque(0, 0, 0);
const YELLOW: Rgba = Rgba::opaque(255, 255, 0);
const GREEN_YELLOWISH: Rgba = Rgba::opaque(128, 255, 0);

/// Indexada de `width_2` × `height_2` con esa paleta, ese índice transparente y esos índices por píxel
/// (fila a fila).
fn image_indexed(
    width_2: u32,
    height_2: u32,
    colors_2: &[Rgba],
    transparent: Option<u8>,
    indices: &[u8],
) -> Image {
    assert_eq!(indices.len(), (width_2 * height_2) as usize);
    let mut palette = Palette::from_colors(colors_2.iter().copied()).expect("paleta de 1 a 256");
    palette
        .set_transparent_index(transparent)
        .expect("el índice transparente existe");
    let mut image_2 = Image::new_indexed(width_2, height_2, palette).expect("imagen indexada");
    for (k, &index_2) in indices.iter().enumerate() {
        let (x, y) = (k as u32 % width_2, k as u32 / width_2);
        image_2
            .set_pixel(x, y, Pixel::Indexed(index_2))
            .expect("el índice está en la paleta");
    }
    image_2
}

/// `F` de la spec: indexada de 4×1, paleta `[R, G, B, K]`, píxeles `[0, 1, 2, 3]`, sin
/// transparente.
fn image_f() -> Image {
    image_indexed(4, 1, &[RED, GREEN, BLUE, TEST_BLACK], None, &[0, 1, 2, 3])
}

/// La imagen que queda al aplicar `SetPaletteColor` con `Document` (la referencia del test).
fn with_set_palette_color(image_2: &Image, index: u8, color: Rgba) -> Image {
    let mut document = Document::new(image_2.clone());
    let id = ImageId::from_raw(0);
    document
        .apply(Command::SetPaletteColor {
            image: id,
            index,
            color,
        })
        .expect("la entrada existe");
    document.image(id).expect("la imagen principal").clone()
}

/// Los cuatro bytes del píxel `k` de unos bytes RGBA.
fn pixel_k(bytes: &[u8], k: usize) -> [u8; 4] {
    bytes[4 * k..4 * k + 4].try_into().expect("4 bytes")
}

/// Bytes de `to_rgba8(image_2)` con el píxel `k` sustituido por `color`.
fn bytes_with_pixel(image_2: &Image, k: usize, color: Rgba) -> Vec<u8> {
    let mut bytes = to_rgba8(image_2);
    bytes[4 * k..4 * k + 4].copy_from_slice(&[color.r, color.g, color.b, color.a]);
    bytes
}

// ---------------------------------------------------------------------------------------------
// Fila 4 (parte de bytes): un arrastre sobre F
// ---------------------------------------------------------------------------------------------

#[test]
fn row_04_the_bytes_are_those_of_to_rgba8_except_pixel_1_yellow() {
    let image_2 = image_f();
    let obtained = to_rgba8_with_entry(&image_2, 1, YELLOW);
    assert_eq!(obtained.len(), 4 * 4);
    assert_eq!(obtained, bytes_with_pixel(&image_2, 1, YELLOW));
    // Los píxeles 0, 2 y 3 son los de siempre.
    let base = to_rgba8(&image_2);
    for k in [0, 2, 3] {
        assert_eq!(pixel_k(&obtained, k), pixel_k(&base, k), "píxel {k}");
    }
    assert_eq!(pixel_k(&obtained, 1), [255, 255, 0, 255]);
}

#[test]
fn row_04_with_the_intermediate_value_of_the_drag() {
    // El provisional del arrastre pasa por (128, 255, 0, 255).
    let image_2 = image_f();
    assert_eq!(
        to_rgba8_with_entry(&image_2, 1, GREEN_YELLOWISH),
        bytes_with_pixel(&image_2, 1, GREEN_YELLOWISH)
    );
}

#[test]
fn row_04_matches_to_rgba8_of_a_copy_with_set_palette_color() {
    let image_2 = image_f();
    let copy = with_set_palette_color(&image_2, 1, YELLOW);
    assert_eq!(to_rgba8_with_entry(&image_2, 1, YELLOW), to_rgba8(&copy));
    // La paleta de la copia es [R, Y, B, K] y los píxeles siguen siendo [0, 1, 2, 3].
    assert_eq!(
        copy.palette().expect("indexada").colors(),
        &[RED, YELLOW, BLUE, TEST_BLACK]
    );
}

#[test]
fn row_04_the_used_image_does_not_change() {
    let image_2 = image_f();
    let before = image_2.clone();
    let _ = to_rgba8_with_entry(&image_2, 1, YELLOW);
    let _ = to_rgba8_with_entry(&image_2, 0, Rgba::new(1, 2, 3, 4));
    assert_eq!(image_2, before);
    assert_eq!(image_2.palette().expect("indexada").get(1), Ok(GREEN));
}

#[test]
fn row_04_with_the_color_the_entry_already_has_it_is_the_document_bytes() {
    let image_2 = image_f();
    assert_eq!(to_rgba8_with_entry(&image_2, 1, GREEN), to_rgba8(&image_2));
}

// ---------------------------------------------------------------------------------------------
// Fila 13 (parte de bytes): duplicadas y transparente
// ---------------------------------------------------------------------------------------------

#[test]
fn row_13a_duplicate_entries_are_not_touched_it_is_replaced_by_index() {
    // [A, A, A], píxeles [0, 1, 2], elegida la 1: se arrastra R.
    let a = Rgba::opaque(10, 20, 30);
    let a_prime = Rgba::opaque(200, 20, 30);
    let image_2 = image_indexed(3, 1, &[a, a, a], None, &[0, 1, 2]);
    let obtained = to_rgba8_with_entry(&image_2, 1, a_prime);

    assert_eq!(
        pixel_k(&obtained, 0),
        [10, 20, 30, 255],
        "el píxel 0 sigue en A"
    );
    assert_eq!(
        pixel_k(&obtained, 1),
        [200, 20, 30, 255],
        "el píxel 1 pasa a A'"
    );
    assert_eq!(
        pixel_k(&obtained, 2),
        [10, 20, 30, 255],
        "el píxel 2 sigue en A"
    );

    // Es la paleta [A, A', A] de SetPaletteColor.
    let copy = with_set_palette_color(&image_2, 1, a_prime);
    assert_eq!(copy.palette().expect("indexada").colors(), &[a, a_prime, a]);
    assert_eq!(obtained, to_rgba8(&copy));
    // Y la imagen original no cambia (el `undo` del documento la restaura: eso es de px-app).
    assert_eq!(to_rgba8(&image_2), to_rgba8_with_entry(&image_2, 1, a));
}

#[test]
fn row_13b_the_transparent_index_stays_with_alpha_0() {
    // [R, G, (0,0,255,255)] con transparente Some(2) y la 2 elegida: se escribe #090909.
    let image_2 = image_indexed(3, 1, &[RED, GREEN, BLUE], Some(2), &[0, 1, 2]);
    let new_2 = Rgba::opaque(9, 9, 9);
    let obtained = to_rgba8_with_entry(&image_2, 2, new_2);

    // Guardado (9,9,9,255), resuelto (9,9,9,0): el píxel 2 tiene alfa 0 y el RGB nuevo.
    assert_eq!(pixel_k(&obtained, 2), [9, 9, 9, 0]);
    assert_eq!(pixel_k(&obtained, 0), [255, 0, 0, 255]);
    assert_eq!(pixel_k(&obtained, 1), [0, 255, 0, 255]);

    let copy = with_set_palette_color(&image_2, 2, new_2);
    assert_eq!(
        copy.palette().expect("indexada").get(2),
        Ok(new_2),
        "lo guardado es (9,9,9,255)"
    );
    assert_eq!(obtained, to_rgba8(&copy));
}

#[test]
fn row_13b_a_different_stored_alpha_on_the_transparent_does_not_change_the_bytes() {
    // El alfa guardado de la transparente no se ve: el resuelto siempre es alfa 0.
    let image_2 = image_indexed(3, 1, &[RED, GREEN, BLUE], Some(2), &[0, 1, 2]);
    let opaque = to_rgba8_with_entry(&image_2, 2, Rgba::new(7, 8, 9, 255));
    let middle = to_rgba8_with_entry(&image_2, 2, Rgba::new(7, 8, 9, 100));
    assert_eq!(opaque, middle);
    assert_eq!(pixel_k(&opaque, 2), [7, 8, 9, 0]);
}

#[test]
fn row_13_a_color_with_alpha_in_a_normal_entry_is_seen_with_that_alpha() {
    // Fila 12 de la spec: A 0 en la entrada 0 de F se ve con alfa 0 (no es la transparente).
    let image_2 = image_f();
    let transparent = Rgba::new(255, 0, 0, 0);
    let obtained = to_rgba8_with_entry(&image_2, 0, transparent);
    assert_eq!(pixel_k(&obtained, 0), [255, 0, 0, 0]);
    assert_eq!(
        obtained,
        to_rgba8(&with_set_palette_color(&image_2, 0, transparent))
    );
}

// ---------------------------------------------------------------------------------------------
// Sin paleta o fuera de ella: igual a to_rgba8
// ---------------------------------------------------------------------------------------------

#[test]
fn a_non_indexed_image_gives_the_bytes_of_to_rgba8() {
    let color = Rgba::new(1, 2, 3, 4);
    let mut rgba = Image::new(3, 2, Mode::Rgba).expect("imagen");
    rgba.set_pixel(1, 1, Pixel::Rgba(Rgba::new(9, 8, 7, 6)))
        .expect("dentro");
    let mut gray_2 = Image::new(3, 2, Mode::Gray).expect("imagen");
    gray_2.set_pixel(2, 0, Pixel::Gray(77)).expect("dentro");
    let mut gray_alpha = Image::new(3, 2, Mode::GrayAlpha).expect("imagen");
    gray_alpha
        .set_pixel(0, 1, Pixel::GrayAlpha { gray: 5, alpha: 6 })
        .expect("dentro");
    for image_2 in [rgba, gray_2, gray_alpha] {
        let before = image_2.clone();
        for index in [0u8, 1, 255] {
            assert_eq!(
                to_rgba8_with_entry(&image_2, index, color),
                to_rgba8(&image_2),
                "{:?}, índice {index}",
                image_2.mode()
            );
        }
        assert_eq!(image_2, before);
    }
}

#[test]
fn an_index_outside_the_palette_gives_the_bytes_of_to_rgba8() {
    let image_2 = image_f();
    for index in [4u8, 5, 100, 255] {
        assert_eq!(
            to_rgba8_with_entry(&image_2, index, YELLOW),
            to_rgba8(&image_2),
            "índice {index} con la paleta de 4 entradas"
        );
    }
}

// ---------------------------------------------------------------------------------------------
// Propiedad 4a
// ---------------------------------------------------------------------------------------------

/// Un color cualquiera, o uno de un grupo pequeño para que haya muchas entradas duplicadas.
fn color() -> impl Strategy<Value = Rgba> {
    prop_oneof![
        (any::<u8>(), any::<u8>(), any::<u8>(), any::<u8>())
            .prop_map(|(r, g, b, a)| Rgba::new(r, g, b, a)),
        (0u8..2, 0u8..2, prop::sample::select(vec![0u8, 255]))
            .prop_map(|(r, g, a)| { Rgba::new(r * 255, g * 255, 0, a) }),
    ]
}

/// Imagen indexada de hasta 8×8 con 1 a 16 entradas, con o sin transparente, más un índice de la
/// paleta y un color.
fn case() -> impl Strategy<Value = (Image, u8, Rgba)> {
    (vec(color(), 1..=16), 1u32..=8, 1u32..=8).prop_flat_map(|(colors_2, width_2, height_2)| {
        let len = colors_2.len() as u8;
        (
            Just(colors_2),
            Just(width_2),
            Just(height_2),
            vec(0..len, (width_2 * height_2) as usize),
            prop::option::of(0..len),
            0..len,
            color(),
        )
            .prop_map(
                |(colors_2, width_2, height_2, indices, transparent, index, color)| {
                    (
                        image_indexed(width_2, height_2, &colors_2, transparent, &indices),
                        index,
                        color,
                    )
                },
            )
    })
}

proptest! {
    /// 4a: `to_rgba8_with_entry` es `to_rgba8` de una copia con `SetPaletteColor` aplicado; la
    /// imagen no cambia; y los píxeles de otro índice son los de `to_rgba8(image)`.
    #[test]
    fn property_4a_equal_to_to_rgba8_of_the_copy_with_set_palette_color(
        (image_2, index, color) in case()
    ) {
        let before = image_2.clone();
        let obtained = to_rgba8_with_entry(&image_2, index, color);
        prop_assert_eq!(&image_2, &before, "la imagen usada no cambia");

        let copy = with_set_palette_color(&image_2, index, color);
        prop_assert_eq!(&obtained, &to_rgba8(&copy));

        let base = to_rgba8(&image_2);
        prop_assert_eq!(obtained.len(), base.len());
        for y in 0..image_2.height() {
            for x in 0..image_2.width() {
                let k = (y * image_2.width() + x) as usize;
                let used = image_2.get_pixel(x, y).expect("dentro");
                if used != Pixel::Indexed(index) {
                    prop_assert_eq!(
                        pixel_k(&obtained, k),
                        pixel_k(&base, k),
                        "el píxel ({}, {}) no usa la entrada {}", x, y, index
                    );
                }
            }
        }
    }
}

// ---------------------------------------------------------------------------------------------
// Banco del lienzo (fila 19)
// ---------------------------------------------------------------------------------------------

const TEST_SCREEN: Vec2 = vec2(800.0, 600.0);

struct Bench {
    ctx: Context,
    canvas: Canvas,
    /// Contenido de cada textura, compuesto en orden con los `TexturesDelta` de cada fotograma.
    textures_2: Textures,
    /// Deltas que llegaron en el último fotograma para la textura del lienzo.
    deltas_last: usize,
}

impl Bench {
    /// Lienzo con `image_2` puesta y subida (dos pases, como pide el banco de `egui`).
    fn with_image(image_2: &Image) -> Bench {
        let mut bench = Bench {
            ctx: Context::default(),
            canvas: Canvas::new(),
            textures_2: Textures::new(),
            deltas_last: 0,
        };
        bench.canvas.set_image(image_2);
        bench.frame();
        bench.frame();
        assert_eq!(
            bench.texture_2(),
            expected(image_2),
            "tras subirla, la textura es la del documento"
        );
        bench
    }

    /// Un fotograma: el lienzo se dibuja y se recogen los cambios de textura.
    fn frame(&mut self) {
        let input_2 = RawInput {
            screen_rect: Some(Rect::from_min_size(Pos2::ZERO, TEST_SCREEN)),
            ..RawInput::default()
        };
        let canvas = &mut self.canvas;
        let mut output_2: FullOutput = self.ctx.run_ui(input_2, |ui| {
            canvas.show(ui);
        });
        let id = self.canvas.texture_id();
        self.deltas_last = 0;
        for (texture_2, deltas) in &output_2.textures_delta.set {
            for delta in deltas {
                // Una sola baldosa con la imagen entera visible: cada cambio de valor reconstruye
                // y sube la baldosa entera (un delta completo), como antes de las baldosas.
                assert_eq!(delta.pos, None, "el lienzo sube la textura entera");
                if Some(*texture_2) == id {
                    self.deltas_last += 1;
                }
            }
        }
        self.textures_2.applies(&output_2.textures_delta);
        // `egui` entra en pánico si se sueltan los cambios sin aplicar.
        output_2.textures_delta.clear();
    }

    /// Contenido actual de la textura del lienzo.
    fn texture_2(&self) -> Vec<Color32> {
        let id = self.canvas.texture_id().expect("la textura está subida");
        self.textures_2
            .get(id)
            .expect("se ha subido su contenido")
            .texels
            .clone()
    }

    /// `preview_entry` y un fotograma, para que el cambio llegue al banco.
    fn previews(&mut self, image_2: &Image, index: u8, color: Rgba) {
        self.canvas.preview_entry(&self.ctx, image_2, index, color);
        self.frame();
    }

    fn the_preview_ends(&mut self, image_2: &Image) {
        self.canvas.end_preview(&self.ctx, image_2);
        self.frame();
    }
}

/// Lo que habría en la textura para unos bytes RGBA de `image_2`.
fn texture_of_bytes(image_2: &Image, bytes: &[u8]) -> Vec<Color32> {
    ColorImage::from_rgba_unmultiplied([image_2.width() as usize, image_2.height() as usize], bytes)
        .pixels
}

/// Lo que habría en la textura para `image_2` tal cual.
fn expected(image_2: &Image) -> Vec<Color32> {
    texture_of_bytes(image_2, &to_rgba8(image_2))
}

/// Lo que habría en la textura con la entrada `index` valiendo `color`.
fn expected_with(image_2: &Image, index: u8, color: Rgba) -> Vec<Color32> {
    texture_of_bytes(image_2, &to_rgba8_with_entry(image_2, index, color))
}

// ---------------------------------------------------------------------------------------------
// Fila 19
// ---------------------------------------------------------------------------------------------

#[test]
fn row_19_without_a_preview_the_counter_is_zero() {
    let image_2 = image_f();
    let bench = Bench::with_image(&image_2);
    assert_eq!(bench.canvas.preview_uploads(), 0);
}

#[test]
fn row_19_three_equal_frames_upload_the_new_color_once_another_and_going_back_another() {
    let image_2 = image_f();
    let mut bench = Bench::with_image(&image_2);

    // Amarillo, tres fotogramas seguidos: una sola subida.
    bench.previews(&image_2, 1, YELLOW);
    assert_eq!(bench.canvas.preview_uploads(), 1);
    assert_eq!(bench.deltas_last, 1, "el primer fotograma sube la textura");
    assert_eq!(bench.texture_2(), expected_with(&image_2, 1, YELLOW));
    for _ in 0..2 {
        bench.previews(&image_2, 1, YELLOW);
        assert_eq!(bench.canvas.preview_uploads(), 1, "el valor se repite");
        assert_eq!(bench.deltas_last, 0, "y no llega nada a la GPU");
    }
    assert_eq!(bench.texture_2(), expected_with(&image_2, 1, YELLOW));

    // Fotogramas sin llamar a `preview_entry`: nada se vuelve a subir.
    bench.frame();
    assert_eq!(bench.deltas_last, 0);
    assert_eq!(bench.canvas.preview_uploads(), 1);

    // Un color nuevo: +1.
    bench.previews(&image_2, 1, GREEN_YELLOWISH);
    assert_eq!(bench.canvas.preview_uploads(), 2);
    assert_eq!(
        bench.texture_2(),
        expected_with(&image_2, 1, GREEN_YELLOWISH)
    );

    // De nuevo amarillo (volver): +1.
    bench.previews(&image_2, 1, YELLOW);
    assert_eq!(bench.canvas.preview_uploads(), 3);
    assert_eq!(bench.texture_2(), expected_with(&image_2, 1, YELLOW));
}

#[test]
fn row_19_the_preview_only_changes_the_chosen_entry() {
    let image_2 = image_f();
    let mut bench = Bench::with_image(&image_2);
    bench.previews(&image_2, 1, YELLOW);
    let texture_2 = bench.texture_2();
    let base = expected(&image_2);
    for k in [0, 2, 3] {
        assert_eq!(texture_2[k], base[k], "píxel {k}");
    }
    assert_ne!(texture_2[1], base[1]);
}

#[test]
fn row_19_releasing_with_a_step_the_refresh_leaves_the_document_texture() {
    let image_2 = image_f();
    let mut bench = Bench::with_image(&image_2);
    bench.previews(&image_2, 1, GREEN_YELLOWISH);
    bench.previews(&image_2, 1, YELLOW);
    let uploads = bench.canvas.preview_uploads();
    assert_eq!(uploads, 2);

    // `Ok(true)`: el documento ya tiene el amarillo y la app llama a `refresh`.
    let document = with_set_palette_color(&image_2, 1, YELLOW);
    bench.canvas.refresh(&bench.ctx, &document);
    bench.frame();
    assert_eq!(bench.texture_2(), expected(&document));
    assert_eq!(
        bench.canvas.preview_uploads(),
        uploads,
        "refresh no cuenta como subida de la vista previa"
    );

    // Y un `end_preview` posterior no estropea nada (la textura sigue siendo la del documento).
    bench.the_preview_ends(&document);
    assert_eq!(bench.texture_2(), expected(&document));
    assert_eq!(bench.canvas.preview_uploads(), uploads);
}

#[test]
fn row_19_cancelling_with_end_preview_restores_the_document_bytes() {
    let image_2 = image_f();
    let mut bench = Bench::with_image(&image_2);
    bench.previews(&image_2, 1, YELLOW);
    assert_ne!(bench.texture_2(), expected(&image_2));

    bench.the_preview_ends(&image_2);
    assert_eq!(bench.texture_2(), expected(&image_2));
    assert_eq!(bench.canvas.preview_uploads(), 1, "end_preview no cuenta");

    // Un segundo `end_preview` ya no hay vista previa: no sube nada.
    bench.the_preview_ends(&image_2);
    assert_eq!(bench.deltas_last, 0);
    assert_eq!(bench.texture_2(), expected(&image_2));
    assert_eq!(bench.canvas.preview_uploads(), 1);
}

#[test]
fn row_19_a_drag_whose_final_value_is_the_initial_ends_without_a_step() {
    let image_2 = image_f();
    let mut bench = Bench::with_image(&image_2);
    bench.previews(&image_2, 1, YELLOW);
    // Vuelve al color del documento (VERDE): sigue siendo un cambio respecto a lo subido.
    bench.previews(&image_2, 1, GREEN);
    assert_eq!(bench.canvas.preview_uploads(), 2);
    assert_eq!(bench.texture_2(), expected(&image_2));

    // Soltar sin paso: `end_preview`.
    bench.the_preview_ends(&image_2);
    assert_eq!(bench.texture_2(), expected(&image_2));
    assert_eq!(bench.canvas.preview_uploads(), 2);
}

#[test]
fn row_19_without_a_previous_preview_entry_end_preview_uploads_nothing() {
    let image_2 = image_f();
    let mut bench = Bench::with_image(&image_2);
    bench.the_preview_ends(&image_2);
    assert_eq!(bench.deltas_last, 0, "no llega ningún delta a la GPU");
    assert_eq!(bench.canvas.preview_uploads(), 0);
    assert_eq!(bench.texture_2(), expected(&image_2));
    // Varias veces, también sin pánico.
    for _ in 0..3 {
        bench.the_preview_ends(&image_2);
        assert_eq!(bench.deltas_last, 0);
    }
}

#[test]
fn row_19_set_image_returns_the_texture_to_the_document() {
    let image_2 = image_f();
    let mut bench = Bench::with_image(&image_2);
    bench.previews(&image_2, 1, YELLOW);

    bench.canvas.set_image(&image_2);
    bench.frame();
    bench.frame();
    assert_eq!(bench.texture_2(), expected(&image_2));

    // Ya no hay vista previa que terminar.
    bench.the_preview_ends(&image_2);
    assert_eq!(bench.deltas_last, 0);
    assert_eq!(bench.texture_2(), expected(&image_2));

    // Y el amarillo vuelve a ser un cambio respecto a lo último subido.
    bench.previews(&image_2, 1, YELLOW);
    assert_eq!(bench.canvas.preview_uploads(), 2);
    assert_eq!(bench.texture_2(), expected_with(&image_2, 1, YELLOW));
}

#[test]
fn row_19_the_duplicates_keep_their_color_in_the_preview_and_after_confirming() {
    // Fila 13a, por el lienzo: [A, A, A], elegida la 1.
    let a = Rgba::opaque(10, 20, 30);
    let a_prime = Rgba::opaque(200, 20, 30);
    let image_2 = image_indexed(3, 1, &[a, a, a], None, &[0, 1, 2]);
    let mut bench = Bench::with_image(&image_2);

    bench.previews(&image_2, 1, a_prime);
    let texture_2 = bench.texture_2();
    let base = expected(&image_2);
    assert_eq!(texture_2[0], base[0], "el píxel 0 conserva A");
    assert_eq!(texture_2[2], base[2], "el píxel 2 conserva A");
    assert_ne!(texture_2[1], base[1]);

    let document = with_set_palette_color(&image_2, 1, a_prime);
    bench.canvas.refresh(&bench.ctx, &document);
    bench.frame();
    let texture_2 = bench.texture_2();
    assert_eq!(texture_2, expected(&document));
    assert_eq!(texture_2[0], base[0]);
    assert_eq!(texture_2[2], base[2]);
}

#[test]
fn row_19_on_the_transparent_the_pixel_stays_with_alpha_0() {
    let image_2 = image_indexed(3, 1, &[RED, GREEN, BLUE], Some(2), &[0, 1, 2]);
    let mut bench = Bench::with_image(&image_2);

    // Un color con otro RGB: cambia el color resuelto (el RGB) y el píxel sigue con alfa 0.
    bench.previews(&image_2, 2, Rgba::opaque(9, 9, 9));
    assert_eq!(bench.canvas.preview_uploads(), 1);
    assert_eq!(bench.texture_2()[2].a(), 0, "el píxel 2 sigue transparente");
    assert_eq!(
        bench.texture_2(),
        expected_with(&image_2, 2, Rgba::opaque(9, 9, 9))
    );

    // Solo cambia el RGB otra vez: el color resuelto es otro, así que sube (la textura, al
    // premultiplicar un alfa 0, queda igual; el contador sigue la regla de la spec: «solo
    // cuando cambia el color resuelto»).
    bench.previews(&image_2, 2, Rgba::opaque(10, 10, 10));
    assert_eq!(bench.canvas.preview_uploads(), 2);
    assert_eq!(bench.texture_2()[2].a(), 0);

    // Solo cambia el alfa guardado: el resuelto (10,10,10,0) es el mismo, no sube.
    bench.previews(&image_2, 2, Rgba::new(10, 10, 10, 7));
    assert_eq!(
        bench.canvas.preview_uploads(),
        2,
        "el alfa guardado de la transparente no cambia lo que se ve"
    );
    assert_eq!(bench.deltas_last, 0);
    assert_eq!(bench.texture_2()[2].a(), 0);
}
