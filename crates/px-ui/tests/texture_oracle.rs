//! Oráculo de `to_rgba8` y `to_rgba8_with_entry`
//! (specs/hito-2-image-filas-bytes.md, migraciones M2 y M3).
//!
//! `to_rgba8_antigua` y `to_rgba8_with_entry_antigua` son copias, dentro de la prueba, de las
//! implementaciones anteriores a la migración (recorren la imagen con `get_pixel`). La salida de
//! las funciones públicas debe ser idéntica byte a byte. Estas pruebas pasan con la
//! implementación antigua y han de seguir pasando con la que trabaja sobre `bytes()`.

use proptest::collection::vec;
use proptest::prelude::*;
use px_core::{Image, Mode, Palette, Pixel, Rgba};
use px_ui::texture::{pixel_rgba, to_rgba8, to_rgba8_with_entry};

// ---------------------------------------------------------------------------------------
// Oráculos: copias de las implementaciones antiguas.
// ---------------------------------------------------------------------------------------

fn to_rgba8_antigua(image: &Image) -> Vec<u8> {
    let (width, height) = (image.width(), image.height());
    let mut bytes = Vec::with_capacity(width as usize * height as usize * 4);
    for y in 0..height {
        for x in 0..width {
            let pixel = image
                .get_pixel(x, y)
                .expect("las coordenadas del bucle están dentro de la imagen");
            let color =
                pixel_rgba(image, pixel).expect("un píxel indexado está en la paleta de su imagen");
            bytes.extend_from_slice(&[color.r, color.g, color.b, color.a]);
        }
    }
    bytes
}

fn to_rgba8_with_entry_antigua(image: &Image, index: u8, color: Rgba) -> Vec<u8> {
    let Some(palette) = image
        .palette()
        .filter(|palette| image.mode() == Mode::Indexed && usize::from(index) < palette.len())
    else {
        return to_rgba8_antigua(image);
    };
    // Tabla índice → color resuelto: una consulta por píxel en vez de una a la paleta.
    let mut table = [Rgba::TRANSPARENT; 256];
    for (slot, entry) in (0..=u8::MAX).zip(table.iter_mut()) {
        if let Ok(resolved) = palette.resolved(slot) {
            *entry = resolved;
        }
    }
    table[usize::from(index)] = if palette.transparent_index() == Some(index) {
        Rgba { a: 0, ..color }
    } else {
        color
    };
    let (width, height) = (image.width(), image.height());
    let mut bytes = Vec::with_capacity(width as usize * height as usize * 4);
    for y in 0..height {
        for x in 0..width {
            let color = match image.get_pixel(x, y) {
                Ok(Pixel::Indexed(i)) => table[usize::from(i)],
                _ => Rgba::TRANSPARENT,
            };
            bytes.extend_from_slice(&[color.r, color.g, color.b, color.a]);
        }
    }
    bytes
}

// ---------------------------------------------------------------------------------------
// Ayudantes.
// ---------------------------------------------------------------------------------------

fn rellena(mut img: Image, pixeles: impl IntoIterator<Item = Pixel>) -> Image {
    let w = img.width();
    for (i, p) in pixeles.into_iter().enumerate() {
        img.set_pixel(i as u32 % w, i as u32 / w, p).unwrap();
    }
    img
}

fn rgba_arbitrario() -> impl Strategy<Value = Rgba> {
    (
        any::<u8>(),
        any::<u8>(),
        any::<u8>(),
        // Con frecuencia a = 0 (con RGB casi siempre distinto de 0) y a = 255.
        prop_oneof![Just(0u8), Just(255u8), any::<u8>()],
    )
        .prop_map(|(r, g, b, a)| Rgba::new(r, g, b, a))
}

/// Imagen de 1 a 16 de lado en cualquiera de los cuatro modos. El indexado lleva una paleta de
/// 1 a 256 entradas, con colores repetidos (entradas duplicadas), alfa 0 con RGB distinto de
/// cero y a veces un índice transparente fijado.
fn imagen_arbitraria() -> BoxedStrategy<Image> {
    (1u32..=16, 1u32..=16)
        .prop_flat_map(|(w, h)| {
            let n = (w * h) as usize;
            prop_oneof![
                vec(rgba_arbitrario(), n).prop_map(move |px| rellena(
                    Image::new(w, h, Mode::Rgba).unwrap(),
                    px.into_iter().map(Pixel::Rgba)
                )),
                vec(any::<u8>(), n).prop_map(move |px| rellena(
                    Image::new(w, h, Mode::Gray).unwrap(),
                    px.into_iter().map(Pixel::Gray)
                )),
                vec(any::<(u8, u8)>(), n).prop_map(move |px| rellena(
                    Image::new(w, h, Mode::GrayAlpha).unwrap(),
                    px.into_iter()
                        .map(|(gray, alpha)| Pixel::GrayAlpha { gray, alpha })
                )),
                (1usize..=256)
                    .prop_flat_map(move |len| {
                        (
                            // Pocas semillas de color, repetidas, para forzar duplicados.
                            vec(rgba_arbitrario(), 1..=4usize),
                            vec(rgba_arbitrario(), len),
                            vec(any::<bool>(), len),
                            proptest::option::of(0..len),
                            vec(0..len, n),
                        )
                    })
                    .prop_map(move |(semillas, colores, duplica, transparente, px)| {
                        let colores = colores.into_iter().zip(duplica).enumerate().map(
                            |(i, (color, dup))| {
                                if dup {
                                    semillas[i % semillas.len()]
                                } else {
                                    color
                                }
                            },
                        );
                        let mut p = Palette::from_colors(colores).unwrap();
                        p.set_transparent_index(transparente.map(|t| t as u8))
                            .unwrap();
                        rellena(
                            Image::new_indexed(w, h, p).unwrap(),
                            px.into_iter().map(|i| Pixel::Indexed(i as u8)),
                        )
                    }),
            ]
        })
        .boxed()
}

// ---------------------------------------------------------------------------------------
// Casos.
// ---------------------------------------------------------------------------------------

/// Una imagen de 1 x 1 de cada modo (el indexado con alfa 0 y RGB distinto de cero).
fn una_de_cada_modo() -> Vec<Image> {
    let mut indexada =
        Palette::from_colors([Rgba::new(10, 20, 30, 0), Rgba::new(10, 20, 30, 0)]).unwrap();
    indexada.set_transparent_index(Some(1)).unwrap();
    vec![
        rellena(
            Image::new(1, 1, Mode::Rgba).unwrap(),
            [Pixel::Rgba(Rgba::new(1, 2, 3, 4))],
        ),
        rellena(
            Image::new_indexed(1, 1, indexada).unwrap(),
            [Pixel::Indexed(1)],
        ),
        rellena(Image::new(1, 1, Mode::Gray).unwrap(), [Pixel::Gray(77)]),
        rellena(
            Image::new(1, 1, Mode::GrayAlpha).unwrap(),
            [Pixel::GrayAlpha {
                gray: 9,
                alpha: 200,
            }],
        ),
    ]
}

#[test]
fn una_imagen_de_1x1_de_cada_modo_da_lo_mismo_que_la_antigua() {
    for img in una_de_cada_modo() {
        assert_eq!(
            to_rgba8(&img),
            to_rgba8_antigua(&img),
            "to_rgba8 {:?}",
            img.mode()
        );
        for index in [0u8, 1, 2, 255] {
            let color = Rgba::new(200, 100, 50, 25);
            assert_eq!(
                to_rgba8_with_entry(&img, index, color),
                to_rgba8_with_entry_antigua(&img, index, color),
                "to_rgba8_with_entry {:?} índice {index}",
                img.mode()
            );
        }
    }
}

// ---------------------------------------------------------------------------------------
// Propiedades.
// ---------------------------------------------------------------------------------------

proptest! {
    #![proptest_config(ProptestConfig::with_cases(256))]

    // `to_rgba8` es idéntica a la antigua en los cuatro modos.
    #[test]
    fn p_to_rgba8_igual_a_la_antigua(img in imagen_arbitraria()) {
        prop_assert_eq!(to_rgba8(&img), to_rgba8_antigua(&img));
    }

    // `to_rgba8_with_entry` es idéntica a la antigua para cualquier índice (también fuera de
    // la paleta y el transparente) y cualquier color; sin paleta vale `to_rgba8`.
    #[test]
    fn p_to_rgba8_with_entry_igual_a_la_antigua(
        img in imagen_arbitraria(),
        index in any::<u8>(),
        color in rgba_arbitrario(),
    ) {
        let nuevo = to_rgba8_with_entry(&img, index, color);
        prop_assert_eq!(&nuevo, &to_rgba8_with_entry_antigua(&img, index, color));
        if img.mode() != Mode::Indexed {
            prop_assert_eq!(&nuevo, &to_rgba8(&img));
        }
    }

    // Con el índice de un píxel y un índice transparente de la propia paleta, ambos caminos
    // (dentro de la paleta) coinciden con la antigua.
    #[test]
    fn p_to_rgba8_with_entry_en_la_paleta_igual_a_la_antigua(
        img in imagen_arbitraria(),
        color in rgba_arbitrario(),
    ) {
        if let Some(palette) = img.palette() {
            let candidatos = [Some(0u8), palette.transparent_index(), u8::try_from(palette.len() - 1).ok()];
            for index in candidatos.into_iter().flatten() {
                prop_assert_eq!(
                    to_rgba8_with_entry(&img, index, color),
                    to_rgba8_with_entry_antigua(&img, index, color)
                );
            }
        }
    }
}
