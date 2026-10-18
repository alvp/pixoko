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
//! entregaría a la GPU. El banco lo acumula (`texturas`: el último contenido completo de cada
//! `TextureId`) y así se puede comparar el contenido de la textura del lienzo con el esperado, y
//! contar cuántos deltas llegaron en un fotograma (cero = no se subió nada). Los píxeles de un
//! `ColorImage` son `Color32` con alfa premultiplicado, así que la comparación se hace contra
//! `ColorImage::from_rgba_unmultiplied` de los bytes esperados (un píxel de alfa 0 pierde su RGB
//! en la textura: por eso la fila 13 mira los bytes de `to_rgba8_with_entry` y, aparte, el alfa
//! del píxel de la textura).

use std::collections::HashMap;

use egui::{
    Color32, ColorImage, Context, FullOutput, ImageData, Pos2, RawInput, Rect, TextureId, Vec2,
    vec2,
};
use proptest::collection::vec;
use proptest::prelude::*;
use px_core::{Command, Document, Image, Mode, Palette, Pixel, Rgba};
use px_ui::canvas::Canvas;
use px_ui::texture::{to_rgba8, to_rgba8_with_entry};

// ---------------------------------------------------------------------------------------------
// Imágenes de prueba
// ---------------------------------------------------------------------------------------------

const ROJO: Rgba = Rgba::opaque(255, 0, 0);
const VERDE: Rgba = Rgba::opaque(0, 255, 0);
const AZUL: Rgba = Rgba::opaque(0, 0, 255);
const NEGRO: Rgba = Rgba::opaque(0, 0, 0);
const AMARILLO: Rgba = Rgba::opaque(255, 255, 0);
const VERDE_AMARILLENTO: Rgba = Rgba::opaque(128, 255, 0);

/// Indexada de `ancho` × `alto` con esa paleta, ese índice transparente y esos índices por píxel
/// (fila a fila).
fn imagen_indexada(
    ancho: u32,
    alto: u32,
    colores: &[Rgba],
    transparente: Option<u8>,
    indices: &[u8],
) -> Image {
    assert_eq!(indices.len(), (ancho * alto) as usize);
    let mut paleta = Palette::from_colors(colores.iter().copied()).expect("paleta de 1 a 256");
    paleta
        .set_transparent_index(transparente)
        .expect("el índice transparente existe");
    let mut imagen = Image::new_indexed(ancho, alto, paleta).expect("imagen indexada");
    for (k, &indice) in indices.iter().enumerate() {
        let (x, y) = (k as u32 % ancho, k as u32 / ancho);
        imagen
            .set_pixel(x, y, Pixel::Indexed(indice))
            .expect("el índice está en la paleta");
    }
    imagen
}

/// `F` de la spec: indexada de 4×1, paleta `[R, G, B, K]`, píxeles `[0, 1, 2, 3]`, sin
/// transparente.
fn imagen_f() -> Image {
    imagen_indexada(4, 1, &[ROJO, VERDE, AZUL, NEGRO], None, &[0, 1, 2, 3])
}

/// La imagen que queda al aplicar `SetPaletteColor` con `Document` (la referencia del test).
fn con_set_palette_color(imagen: &Image, index: u8, color: Rgba) -> Image {
    let mut documento = Document::new(imagen.clone());
    let id = documento.main_image();
    documento
        .apply(Command::SetPaletteColor {
            image: id,
            index,
            color,
        })
        .expect("la entrada existe");
    documento.image(id).expect("la imagen principal").clone()
}

/// Los cuatro bytes del píxel `k` de unos bytes RGBA.
fn pixel_k(bytes: &[u8], k: usize) -> [u8; 4] {
    bytes[4 * k..4 * k + 4].try_into().expect("4 bytes")
}

/// Bytes de `to_rgba8(imagen)` con el píxel `k` sustituido por `color`.
fn bytes_con_pixel(imagen: &Image, k: usize, color: Rgba) -> Vec<u8> {
    let mut bytes = to_rgba8(imagen);
    bytes[4 * k..4 * k + 4].copy_from_slice(&[color.r, color.g, color.b, color.a]);
    bytes
}

// ---------------------------------------------------------------------------------------------
// Fila 4 (parte de bytes): un arrastre sobre F
// ---------------------------------------------------------------------------------------------

#[test]
fn fila_04_los_bytes_son_los_de_to_rgba8_salvo_el_pixel_1_amarillo() {
    let imagen = imagen_f();
    let obtenido = to_rgba8_with_entry(&imagen, 1, AMARILLO);
    assert_eq!(obtenido.len(), 4 * 4);
    assert_eq!(obtenido, bytes_con_pixel(&imagen, 1, AMARILLO));
    // Los píxeles 0, 2 y 3 son los de siempre.
    let base = to_rgba8(&imagen);
    for k in [0, 2, 3] {
        assert_eq!(pixel_k(&obtenido, k), pixel_k(&base, k), "píxel {k}");
    }
    assert_eq!(pixel_k(&obtenido, 1), [255, 255, 0, 255]);
}

#[test]
fn fila_04_con_el_valor_intermedio_del_arrastre() {
    // El provisional del arrastre pasa por (128, 255, 0, 255).
    let imagen = imagen_f();
    assert_eq!(
        to_rgba8_with_entry(&imagen, 1, VERDE_AMARILLENTO),
        bytes_con_pixel(&imagen, 1, VERDE_AMARILLENTO)
    );
}

#[test]
fn fila_04_coincide_con_to_rgba8_de_una_copia_con_set_palette_color() {
    let imagen = imagen_f();
    let copia = con_set_palette_color(&imagen, 1, AMARILLO);
    assert_eq!(to_rgba8_with_entry(&imagen, 1, AMARILLO), to_rgba8(&copia));
    // La paleta de la copia es [R, Y, B, K] y los píxeles siguen siendo [0, 1, 2, 3].
    assert_eq!(
        copia.palette().expect("indexada").colors(),
        &[ROJO, AMARILLO, AZUL, NEGRO]
    );
}

#[test]
fn fila_04_la_imagen_usada_no_cambia() {
    let imagen = imagen_f();
    let antes = imagen.clone();
    let _ = to_rgba8_with_entry(&imagen, 1, AMARILLO);
    let _ = to_rgba8_with_entry(&imagen, 0, Rgba::new(1, 2, 3, 4));
    assert_eq!(imagen, antes);
    assert_eq!(imagen.palette().expect("indexada").get(1), Ok(VERDE));
}

#[test]
fn fila_04_con_el_color_que_ya_tiene_la_entrada_son_los_bytes_del_documento() {
    let imagen = imagen_f();
    assert_eq!(to_rgba8_with_entry(&imagen, 1, VERDE), to_rgba8(&imagen));
}

// ---------------------------------------------------------------------------------------------
// Fila 13 (parte de bytes): duplicadas y transparente
// ---------------------------------------------------------------------------------------------

#[test]
fn fila_13a_las_entradas_duplicadas_no_se_tocan_se_sustituye_por_indice() {
    // [A, A, A], píxeles [0, 1, 2], elegida la 1: se arrastra R.
    let a = Rgba::opaque(10, 20, 30);
    let a_prima = Rgba::opaque(200, 20, 30);
    let imagen = imagen_indexada(3, 1, &[a, a, a], None, &[0, 1, 2]);
    let obtenido = to_rgba8_with_entry(&imagen, 1, a_prima);

    assert_eq!(
        pixel_k(&obtenido, 0),
        [10, 20, 30, 255],
        "el píxel 0 sigue en A"
    );
    assert_eq!(
        pixel_k(&obtenido, 1),
        [200, 20, 30, 255],
        "el píxel 1 pasa a A'"
    );
    assert_eq!(
        pixel_k(&obtenido, 2),
        [10, 20, 30, 255],
        "el píxel 2 sigue en A"
    );

    // Es la paleta [A, A', A] de SetPaletteColor.
    let copia = con_set_palette_color(&imagen, 1, a_prima);
    assert_eq!(
        copia.palette().expect("indexada").colors(),
        &[a, a_prima, a]
    );
    assert_eq!(obtenido, to_rgba8(&copia));
    // Y la imagen original no cambia (el `undo` del documento la restaura: eso es de px-app).
    assert_eq!(to_rgba8(&imagen), to_rgba8_with_entry(&imagen, 1, a));
}

#[test]
fn fila_13b_el_indice_transparente_sigue_con_alfa_0() {
    // [R, G, (0,0,255,255)] con transparente Some(2) y la 2 elegida: se escribe #090909.
    let imagen = imagen_indexada(3, 1, &[ROJO, VERDE, AZUL], Some(2), &[0, 1, 2]);
    let nuevo = Rgba::opaque(9, 9, 9);
    let obtenido = to_rgba8_with_entry(&imagen, 2, nuevo);

    // Guardado (9,9,9,255), resuelto (9,9,9,0): el píxel 2 tiene alfa 0 y el RGB nuevo.
    assert_eq!(pixel_k(&obtenido, 2), [9, 9, 9, 0]);
    assert_eq!(pixel_k(&obtenido, 0), [255, 0, 0, 255]);
    assert_eq!(pixel_k(&obtenido, 1), [0, 255, 0, 255]);

    let copia = con_set_palette_color(&imagen, 2, nuevo);
    assert_eq!(
        copia.palette().expect("indexada").get(2),
        Ok(nuevo),
        "lo guardado es (9,9,9,255)"
    );
    assert_eq!(obtenido, to_rgba8(&copia));
}

#[test]
fn fila_13b_un_alfa_guardado_distinto_en_la_transparente_no_cambia_los_bytes() {
    // El alfa guardado de la transparente no se ve: el resuelto siempre es alfa 0.
    let imagen = imagen_indexada(3, 1, &[ROJO, VERDE, AZUL], Some(2), &[0, 1, 2]);
    let opaco = to_rgba8_with_entry(&imagen, 2, Rgba::new(7, 8, 9, 255));
    let medio = to_rgba8_with_entry(&imagen, 2, Rgba::new(7, 8, 9, 100));
    assert_eq!(opaco, medio);
    assert_eq!(pixel_k(&opaco, 2), [7, 8, 9, 0]);
}

#[test]
fn fila_13_un_color_con_alfa_en_una_entrada_normal_se_ve_con_ese_alfa() {
    // Fila 12 de la spec: A 0 en la entrada 0 de F se ve con alfa 0 (no es la transparente).
    let imagen = imagen_f();
    let transparente = Rgba::new(255, 0, 0, 0);
    let obtenido = to_rgba8_with_entry(&imagen, 0, transparente);
    assert_eq!(pixel_k(&obtenido, 0), [255, 0, 0, 0]);
    assert_eq!(
        obtenido,
        to_rgba8(&con_set_palette_color(&imagen, 0, transparente))
    );
}

// ---------------------------------------------------------------------------------------------
// Sin paleta o fuera de ella: igual a to_rgba8
// ---------------------------------------------------------------------------------------------

#[test]
fn una_imagen_no_indexada_da_los_bytes_de_to_rgba8() {
    let color = Rgba::new(1, 2, 3, 4);
    let mut rgba = Image::new(3, 2, Mode::Rgba).expect("imagen");
    rgba.set_pixel(1, 1, Pixel::Rgba(Rgba::new(9, 8, 7, 6)))
        .expect("dentro");
    let mut gris = Image::new(3, 2, Mode::Gray).expect("imagen");
    gris.set_pixel(2, 0, Pixel::Gray(77)).expect("dentro");
    let mut gris_alfa = Image::new(3, 2, Mode::GrayAlpha).expect("imagen");
    gris_alfa
        .set_pixel(0, 1, Pixel::GrayAlpha { gray: 5, alpha: 6 })
        .expect("dentro");
    for imagen in [rgba, gris, gris_alfa] {
        let antes = imagen.clone();
        for index in [0u8, 1, 255] {
            assert_eq!(
                to_rgba8_with_entry(&imagen, index, color),
                to_rgba8(&imagen),
                "{:?}, índice {index}",
                imagen.mode()
            );
        }
        assert_eq!(imagen, antes);
    }
}

#[test]
fn un_indice_fuera_de_la_paleta_da_los_bytes_de_to_rgba8() {
    let imagen = imagen_f();
    for index in [4u8, 5, 100, 255] {
        assert_eq!(
            to_rgba8_with_entry(&imagen, index, AMARILLO),
            to_rgba8(&imagen),
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
fn caso() -> impl Strategy<Value = (Image, u8, Rgba)> {
    (vec(color(), 1..=16), 1u32..=8, 1u32..=8).prop_flat_map(|(colores, ancho, alto)| {
        let len = colores.len() as u8;
        (
            Just(colores),
            Just(ancho),
            Just(alto),
            vec(0..len, (ancho * alto) as usize),
            prop::option::of(0..len),
            0..len,
            color(),
        )
            .prop_map(
                |(colores, ancho, alto, indices, transparente, index, color)| {
                    (
                        imagen_indexada(ancho, alto, &colores, transparente, &indices),
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
    fn propiedad_4a_igual_a_to_rgba8_de_la_copia_con_set_palette_color(
        (imagen, index, color) in caso()
    ) {
        let antes = imagen.clone();
        let obtenido = to_rgba8_with_entry(&imagen, index, color);
        prop_assert_eq!(&imagen, &antes, "la imagen usada no cambia");

        let copia = con_set_palette_color(&imagen, index, color);
        prop_assert_eq!(&obtenido, &to_rgba8(&copia));

        let base = to_rgba8(&imagen);
        prop_assert_eq!(obtenido.len(), base.len());
        for y in 0..imagen.height() {
            for x in 0..imagen.width() {
                let k = (y * imagen.width() + x) as usize;
                let usado = imagen.get_pixel(x, y).expect("dentro");
                if usado != Pixel::Indexed(index) {
                    prop_assert_eq!(
                        pixel_k(&obtenido, k),
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

const PANTALLA: Vec2 = vec2(800.0, 600.0);

struct Banco {
    ctx: Context,
    canvas: Canvas,
    /// Último contenido completo conocido de cada textura, acumulado de los `TexturesDelta`.
    texturas: HashMap<TextureId, Vec<Color32>>,
    /// Deltas que llegaron en el último fotograma para la textura del lienzo.
    deltas_ultimo: usize,
}

impl Banco {
    /// Lienzo con `imagen` puesta y subida (dos pases, como pide el banco de `egui`).
    fn con_imagen(imagen: &Image) -> Banco {
        let mut banco = Banco {
            ctx: Context::default(),
            canvas: Canvas::new(),
            texturas: HashMap::new(),
            deltas_ultimo: 0,
        };
        banco.canvas.set_image(imagen);
        banco.fotograma();
        banco.fotograma();
        assert_eq!(
            banco.textura(),
            esperada(imagen),
            "tras subirla, la textura es la del documento"
        );
        banco
    }

    /// Un fotograma: el lienzo se dibuja y se recogen los cambios de textura.
    fn fotograma(&mut self) {
        let entrada = RawInput {
            screen_rect: Some(Rect::from_min_size(Pos2::ZERO, PANTALLA)),
            ..RawInput::default()
        };
        let canvas = &mut self.canvas;
        let mut salida: FullOutput = self.ctx.run_ui(entrada, |ui| {
            canvas.show(ui);
        });
        let id = self.canvas.texture_id();
        self.deltas_ultimo = 0;
        for (textura, deltas) in &salida.textures_delta.set {
            for delta in deltas {
                assert_eq!(delta.pos, None, "el lienzo sube la textura entera");
                let ImageData::Color(imagen) = &delta.image;
                self.texturas.insert(*textura, imagen.pixels.clone());
                if Some(*textura) == id {
                    self.deltas_ultimo += 1;
                }
            }
        }
        // `egui` entra en pánico si se sueltan los cambios sin aplicar.
        salida.textures_delta.clear();
    }

    /// Contenido actual de la textura del lienzo.
    fn textura(&self) -> Vec<Color32> {
        let id = self.canvas.texture_id().expect("la textura está subida");
        self.texturas
            .get(&id)
            .expect("se ha subido su contenido")
            .clone()
    }

    /// `preview_entry` y un fotograma, para que el cambio llegue al banco.
    fn previsualiza(&mut self, imagen: &Image, index: u8, color: Rgba) {
        self.canvas.preview_entry(&self.ctx, imagen, index, color);
        self.fotograma();
    }

    fn termina_la_vista_previa(&mut self, imagen: &Image) {
        self.canvas.end_preview(&self.ctx, imagen);
        self.fotograma();
    }
}

/// Lo que habría en la textura para unos bytes RGBA de `imagen`.
fn textura_de_bytes(imagen: &Image, bytes: &[u8]) -> Vec<Color32> {
    ColorImage::from_rgba_unmultiplied([imagen.width() as usize, imagen.height() as usize], bytes)
        .pixels
}

/// Lo que habría en la textura para `imagen` tal cual.
fn esperada(imagen: &Image) -> Vec<Color32> {
    textura_de_bytes(imagen, &to_rgba8(imagen))
}

/// Lo que habría en la textura con la entrada `index` valiendo `color`.
fn esperada_con(imagen: &Image, index: u8, color: Rgba) -> Vec<Color32> {
    textura_de_bytes(imagen, &to_rgba8_with_entry(imagen, index, color))
}

// ---------------------------------------------------------------------------------------------
// Fila 19
// ---------------------------------------------------------------------------------------------

#[test]
fn fila_19_sin_vista_previa_el_contador_esta_a_cero() {
    let imagen = imagen_f();
    let banco = Banco::con_imagen(&imagen);
    assert_eq!(banco.canvas.preview_uploads(), 0);
}

#[test]
fn fila_19_tres_fotogramas_iguales_suben_una_vez_el_color_nuevo_otra_y_volver_otra() {
    let imagen = imagen_f();
    let mut banco = Banco::con_imagen(&imagen);

    // Amarillo, tres fotogramas seguidos: una sola subida.
    banco.previsualiza(&imagen, 1, AMARILLO);
    assert_eq!(banco.canvas.preview_uploads(), 1);
    assert_eq!(
        banco.deltas_ultimo, 1,
        "el primer fotograma sube la textura"
    );
    assert_eq!(banco.textura(), esperada_con(&imagen, 1, AMARILLO));
    for _ in 0..2 {
        banco.previsualiza(&imagen, 1, AMARILLO);
        assert_eq!(banco.canvas.preview_uploads(), 1, "el valor se repite");
        assert_eq!(banco.deltas_ultimo, 0, "y no llega nada a la GPU");
    }
    assert_eq!(banco.textura(), esperada_con(&imagen, 1, AMARILLO));

    // Fotogramas sin llamar a `preview_entry`: nada se vuelve a subir.
    banco.fotograma();
    assert_eq!(banco.deltas_ultimo, 0);
    assert_eq!(banco.canvas.preview_uploads(), 1);

    // Un color nuevo: +1.
    banco.previsualiza(&imagen, 1, VERDE_AMARILLENTO);
    assert_eq!(banco.canvas.preview_uploads(), 2);
    assert_eq!(banco.textura(), esperada_con(&imagen, 1, VERDE_AMARILLENTO));

    // De nuevo amarillo (volver): +1.
    banco.previsualiza(&imagen, 1, AMARILLO);
    assert_eq!(banco.canvas.preview_uploads(), 3);
    assert_eq!(banco.textura(), esperada_con(&imagen, 1, AMARILLO));
}

#[test]
fn fila_19_la_vista_previa_solo_cambia_la_entrada_elegida() {
    let imagen = imagen_f();
    let mut banco = Banco::con_imagen(&imagen);
    banco.previsualiza(&imagen, 1, AMARILLO);
    let textura = banco.textura();
    let base = esperada(&imagen);
    for k in [0, 2, 3] {
        assert_eq!(textura[k], base[k], "píxel {k}");
    }
    assert_ne!(textura[1], base[1]);
}

#[test]
fn fila_19_soltar_con_paso_el_refresh_deja_la_textura_del_documento() {
    let imagen = imagen_f();
    let mut banco = Banco::con_imagen(&imagen);
    banco.previsualiza(&imagen, 1, VERDE_AMARILLENTO);
    banco.previsualiza(&imagen, 1, AMARILLO);
    let subidas = banco.canvas.preview_uploads();
    assert_eq!(subidas, 2);

    // `Ok(true)`: el documento ya tiene el amarillo y la app llama a `refresh`.
    let documento = con_set_palette_color(&imagen, 1, AMARILLO);
    banco.canvas.refresh(&banco.ctx, &documento);
    banco.fotograma();
    assert_eq!(banco.textura(), esperada(&documento));
    assert_eq!(
        banco.canvas.preview_uploads(),
        subidas,
        "refresh no cuenta como subida de la vista previa"
    );

    // Y un `end_preview` posterior no estropea nada (la textura sigue siendo la del documento).
    banco.termina_la_vista_previa(&documento);
    assert_eq!(banco.textura(), esperada(&documento));
    assert_eq!(banco.canvas.preview_uploads(), subidas);
}

#[test]
fn fila_19_cancelar_con_end_preview_restaura_los_bytes_del_documento() {
    let imagen = imagen_f();
    let mut banco = Banco::con_imagen(&imagen);
    banco.previsualiza(&imagen, 1, AMARILLO);
    assert_ne!(banco.textura(), esperada(&imagen));

    banco.termina_la_vista_previa(&imagen);
    assert_eq!(banco.textura(), esperada(&imagen));
    assert_eq!(banco.canvas.preview_uploads(), 1, "end_preview no cuenta");

    // Un segundo `end_preview` ya no hay vista previa: no sube nada.
    banco.termina_la_vista_previa(&imagen);
    assert_eq!(banco.deltas_ultimo, 0);
    assert_eq!(banco.textura(), esperada(&imagen));
    assert_eq!(banco.canvas.preview_uploads(), 1);
}

#[test]
fn fila_19_un_arrastre_cuyo_valor_final_es_el_inicial_termina_sin_paso() {
    let imagen = imagen_f();
    let mut banco = Banco::con_imagen(&imagen);
    banco.previsualiza(&imagen, 1, AMARILLO);
    // Vuelve al color del documento (VERDE): sigue siendo un cambio respecto a lo subido.
    banco.previsualiza(&imagen, 1, VERDE);
    assert_eq!(banco.canvas.preview_uploads(), 2);
    assert_eq!(banco.textura(), esperada(&imagen));

    // Soltar sin paso: `end_preview`.
    banco.termina_la_vista_previa(&imagen);
    assert_eq!(banco.textura(), esperada(&imagen));
    assert_eq!(banco.canvas.preview_uploads(), 2);
}

#[test]
fn fila_19_sin_preview_entry_previo_end_preview_no_sube_nada() {
    let imagen = imagen_f();
    let mut banco = Banco::con_imagen(&imagen);
    banco.termina_la_vista_previa(&imagen);
    assert_eq!(banco.deltas_ultimo, 0, "no llega ningún delta a la GPU");
    assert_eq!(banco.canvas.preview_uploads(), 0);
    assert_eq!(banco.textura(), esperada(&imagen));
    // Varias veces, también sin pánico.
    for _ in 0..3 {
        banco.termina_la_vista_previa(&imagen);
        assert_eq!(banco.deltas_ultimo, 0);
    }
}

#[test]
fn fila_19_set_image_devuelve_la_textura_al_documento() {
    let imagen = imagen_f();
    let mut banco = Banco::con_imagen(&imagen);
    banco.previsualiza(&imagen, 1, AMARILLO);

    banco.canvas.set_image(&imagen);
    banco.fotograma();
    banco.fotograma();
    assert_eq!(banco.textura(), esperada(&imagen));

    // Ya no hay vista previa que terminar.
    banco.termina_la_vista_previa(&imagen);
    assert_eq!(banco.deltas_ultimo, 0);
    assert_eq!(banco.textura(), esperada(&imagen));

    // Y el amarillo vuelve a ser un cambio respecto a lo último subido.
    banco.previsualiza(&imagen, 1, AMARILLO);
    assert_eq!(banco.canvas.preview_uploads(), 2);
    assert_eq!(banco.textura(), esperada_con(&imagen, 1, AMARILLO));
}

#[test]
fn fila_19_las_duplicadas_conservan_su_color_en_la_vista_previa_y_tras_confirmar() {
    // Fila 13a, por el lienzo: [A, A, A], elegida la 1.
    let a = Rgba::opaque(10, 20, 30);
    let a_prima = Rgba::opaque(200, 20, 30);
    let imagen = imagen_indexada(3, 1, &[a, a, a], None, &[0, 1, 2]);
    let mut banco = Banco::con_imagen(&imagen);

    banco.previsualiza(&imagen, 1, a_prima);
    let textura = banco.textura();
    let base = esperada(&imagen);
    assert_eq!(textura[0], base[0], "el píxel 0 conserva A");
    assert_eq!(textura[2], base[2], "el píxel 2 conserva A");
    assert_ne!(textura[1], base[1]);

    let documento = con_set_palette_color(&imagen, 1, a_prima);
    banco.canvas.refresh(&banco.ctx, &documento);
    banco.fotograma();
    let textura = banco.textura();
    assert_eq!(textura, esperada(&documento));
    assert_eq!(textura[0], base[0]);
    assert_eq!(textura[2], base[2]);
}

#[test]
fn fila_19_en_la_transparente_el_pixel_sigue_con_alfa_0() {
    let imagen = imagen_indexada(3, 1, &[ROJO, VERDE, AZUL], Some(2), &[0, 1, 2]);
    let mut banco = Banco::con_imagen(&imagen);

    // Un color con otro RGB: cambia el color resuelto (el RGB) y el píxel sigue con alfa 0.
    banco.previsualiza(&imagen, 2, Rgba::opaque(9, 9, 9));
    assert_eq!(banco.canvas.preview_uploads(), 1);
    assert_eq!(banco.textura()[2].a(), 0, "el píxel 2 sigue transparente");
    assert_eq!(
        banco.textura(),
        esperada_con(&imagen, 2, Rgba::opaque(9, 9, 9))
    );

    // Solo cambia el RGB otra vez: el color resuelto es otro, así que sube (la textura, al
    // premultiplicar un alfa 0, queda igual; el contador sigue la regla de la spec: «solo
    // cuando cambia el color resuelto»).
    banco.previsualiza(&imagen, 2, Rgba::opaque(10, 10, 10));
    assert_eq!(banco.canvas.preview_uploads(), 2);
    assert_eq!(banco.textura()[2].a(), 0);

    // Solo cambia el alfa guardado: el resuelto (10,10,10,0) es el mismo, no sube.
    banco.previsualiza(&imagen, 2, Rgba::new(10, 10, 10, 7));
    assert_eq!(
        banco.canvas.preview_uploads(),
        2,
        "el alfa guardado de la transparente no cambia lo que se ve"
    );
    assert_eq!(banco.deltas_ultimo, 0);
    assert_eq!(banco.textura()[2].a(), 0);
}
