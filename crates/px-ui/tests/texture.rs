//! Tests de `texture::to_rgba8`, la conversión de `Image` a bytes RGBA
//! (specs/hito-2-ventana.md, «De `Image` a textura», primer punto).
//!
//! Contrato: `to_rgba8(&Image) -> Vec<u8>` devuelve `width_2 · height_2 · 4` bytes, fila a fila
//! (la de arriba primero), con alfa directo (no premultiplicado). RGBA sale tal cual
//! `r, g, b, a`; indexado sale con `palette.resolved(i)` (el índice transparente da `a = 0`
//! y conserva su RGB; el resto, su alfa guardado); gris sale `(g, g, g, 255)` y gris con
//! alfa `(g, g, g, a)`. No falla ni entra en pánico con ninguna `Image` válida.
//!
//! Lo de egui (`ColorImage`, texturas, `TooLarge`) no se prueba aquí.

use proptest::collection::vec;
use proptest::prelude::*;
use px_core::{Image, Mode, Palette, Pixel, Rgba};
use px_ui::texture::to_rgba8;

// Utilidades de apoyo.

const TEST_ALL: [Mode; 4] = [Mode::Rgba, Mode::Indexed, Mode::Gray, Mode::GrayAlpha];

fn palette(colors_2: &[Rgba]) -> Palette {
    Palette::from_colors(colors_2.iter().copied()).unwrap()
}

/// Longitud esperada de los bytes de una imagen de `width` x `height`.
fn length(width: u32, height: u32) -> usize {
    width as usize * height as usize * 4
}

/// Pixel de modo `mode` distinto para cada `k` (hasta 256 valores distintos en cada canal);
/// el indexado usa `k % n` con una paleta de `n` entradas.
fn pixel_de(mode: Mode, k: usize, n: usize) -> Pixel {
    let k8 = k as u8;
    match mode {
        Mode::Rgba => Pixel::Rgba(Rgba::new(
            k8,
            k8.wrapping_mul(3).wrapping_add(1),
            255 - k8,
            k8.wrapping_add(100),
        )),
        Mode::Indexed => Pixel::Indexed((k % n) as u8),
        Mode::Gray => Pixel::Gray(k8.wrapping_mul(7).wrapping_add(5)),
        Mode::GrayAlpha => Pixel::GrayAlpha {
            gray: k8.wrapping_mul(7).wrapping_add(5),
            alpha: 255 - k8,
        },
    }
}

/// Paleta de `n` entradas con colores y alfas distintos, sin índice transparente.
fn palette_of(n: usize) -> Palette {
    Palette::from_colors((0..n).map(|i| {
        Rgba::new(
            i as u8,
            255 - i as u8,
            (i as u8).wrapping_mul(5),
            (i as u8).wrapping_add(50),
        )
    }))
    .unwrap()
}

/// Imagen de cualquier modo con el píxel `(x, y)` igual a `pixel_de(mode, y * width + x, n)`.
fn fills(mode: Mode, width: u32, height: u32, n: usize) -> Image {
    let mut image_2 = match mode {
        Mode::Indexed => Image::new_indexed(width, height, palette_of(n)).unwrap(),
        _ => Image::new(width, height, mode).unwrap(),
    };
    for y in 0..height {
        for x in 0..width {
            let k = y as usize * width as usize + x as usize;
            image_2.set_pixel(x, y, pixel_de(mode, k, n)).unwrap();
        }
    }
    image_2
}

/// Los cuatro bytes que debe dar un píxel; salida de referencia escrita aparte de la función
/// probada, a partir de `get_pixel` y de la paleta de la imagen.
fn expected_of(image_2: &Image, x: u32, y: u32) -> [u8; 4] {
    match image_2.get_pixel(x, y).unwrap() {
        Pixel::Rgba(c) => [c.r, c.g, c.b, c.a],
        Pixel::Indexed(i) => {
            let c = image_2.palette().unwrap().resolved(i).unwrap();
            [c.r, c.g, c.b, c.a]
        }
        Pixel::Gray(g) => [g, g, g, 255],
        Pixel::GrayAlpha { gray, alpha } => [gray, gray, gray, alpha],
    }
}

/// Comprueba longitud y, píxel a píxel, que los bytes de `to_rgba8` son los esperados.
fn checks_against_get_pixel(image_2: &Image) -> Result<(), TestCaseError> {
    let (w, h) = (image_2.width(), image_2.height());
    let bytes = to_rgba8(image_2);
    prop_assert_eq!(bytes.len(), length(w, h));
    for y in 0..h {
        for x in 0..w {
            let at = (y as usize * w as usize + x as usize) * 4;
            prop_assert_eq!(
                &bytes[at..at + 4],
                &expected_of(image_2, x, y)[..],
                "píxel ({}, {})",
                x,
                y
            );
        }
    }
    Ok(())
}

// Ejemplos por modo, imagen 3 x 2 con todos los valores distintos: detectan un orden de
// filas o columnas equivocado y una permutación de canales.

#[test]
fn rgba_3x2_comes_out_as_is() {
    let mut image_2 = Image::new(3, 2, Mode::Rgba).unwrap();
    // Fila de arriba: (1,2,3,4) (5,6,7,8) (9,10,11,12); fila de abajo: 13..=24.
    for y in 0..2u32 {
        for x in 0..3u32 {
            let b = ((y * 3 + x) * 4) as u8;
            let c = Rgba::new(b + 1, b + 2, b + 3, b + 4);
            image_2.set_pixel(x, y, Pixel::Rgba(c)).unwrap();
        }
    }
    let expected_2: Vec<u8> = (1..=24).collect();
    assert_eq!(to_rgba8(&image_2), expected_2);
}

#[test]
fn rgba_keeps_the_rgb_of_the_transparent_and_semitransparent_pixels() {
    // Alfa directo: no se premultiplica ni se borra el RGB cuando a = 0.
    let mut image_2 = Image::new(3, 1, Mode::Rgba).unwrap();
    image_2
        .set_pixel(0, 0, Pixel::Rgba(Rgba::new(200, 100, 50, 0)))
        .unwrap();
    image_2
        .set_pixel(1, 0, Pixel::Rgba(Rgba::new(200, 100, 50, 128)))
        .unwrap();
    image_2
        .set_pixel(2, 0, Pixel::Rgba(Rgba::new(200, 100, 50, 255)))
        .unwrap();
    assert_eq!(
        to_rgba8(&image_2),
        vec![200, 100, 50, 0, 200, 100, 50, 128, 200, 100, 50, 255]
    );
}

#[test]
fn gray_3x2_gives_g_g_g_and_opaque_alpha() {
    let mut image_2 = Image::new(3, 2, Mode::Gray).unwrap();
    let grays = [10u8, 20, 30, 40, 50, 60];
    for (k, &g) in grays.iter().enumerate() {
        image_2
            .set_pixel(k as u32 % 3, k as u32 / 3, Pixel::Gray(g))
            .unwrap();
    }
    let expected_2 = vec![
        10, 10, 10, 255, 20, 20, 20, 255, 30, 30, 30, 255, // fila de arriba
        40, 40, 40, 255, 50, 50, 50, 255, 60, 60, 60, 255, // fila de abajo
    ];
    assert_eq!(to_rgba8(&image_2), expected_2);
}

#[test]
fn gray_extremes_0_and_255() {
    let mut image_2 = Image::new(2, 1, Mode::Gray).unwrap();
    image_2.set_pixel(0, 0, Pixel::Gray(0)).unwrap();
    image_2.set_pixel(1, 0, Pixel::Gray(255)).unwrap();
    assert_eq!(to_rgba8(&image_2), vec![0, 0, 0, 255, 255, 255, 255, 255]);
}

#[test]
fn gray_alpha_3x2_gives_g_g_g_a() {
    let mut image_2 = Image::new(3, 2, Mode::GrayAlpha).unwrap();
    let pairs = [
        (10u8, 200u8),
        (20, 190),
        (30, 0),
        (40, 255),
        (50, 128),
        (60, 1),
    ];
    for (k, &(gray, alpha)) in pairs.iter().enumerate() {
        image_2
            .set_pixel(k as u32 % 3, k as u32 / 3, Pixel::GrayAlpha { gray, alpha })
            .unwrap();
    }
    let expected_2 = vec![
        10, 10, 10, 200, 20, 20, 20, 190, 30, 30, 30, 0, // fila de arriba
        40, 40, 40, 255, 50, 50, 50, 128, 60, 60, 60, 1, // fila de abajo
    ];
    assert_eq!(to_rgba8(&image_2), expected_2);
}

#[test]
fn indexed_3x2_uses_the_palette() {
    let p = palette(&[
        Rgba::new(1, 2, 3, 255),
        Rgba::new(4, 5, 6, 128),
        Rgba::new(7, 8, 9, 64),
        Rgba::new(10, 11, 12, 255),
    ]);
    let mut image_2 = Image::new_indexed(3, 2, p).unwrap();
    let indices = [3u8, 0, 2, 1, 1, 3];
    for (k, &i) in indices.iter().enumerate() {
        image_2
            .set_pixel(k as u32 % 3, k as u32 / 3, Pixel::Indexed(i))
            .unwrap();
    }
    let expected_2 = vec![
        10, 11, 12, 255, 1, 2, 3, 255, 7, 8, 9, 64, // fila de arriba
        4, 5, 6, 128, 4, 5, 6, 128, 10, 11, 12, 255, // fila de abajo
    ];
    assert_eq!(to_rgba8(&image_2), expected_2);
}

#[test]
fn indexed_without_a_transparent_index_keeps_the_stored_alpha_even_zero() {
    let p = palette(&[Rgba::new(9, 8, 7, 0), Rgba::new(1, 2, 3, 77)]);
    assert_eq!(p.transparent_index(), None);
    let mut image_2 = Image::new_indexed(2, 1, p).unwrap();
    image_2.set_pixel(1, 0, Pixel::Indexed(1)).unwrap();
    assert_eq!(to_rgba8(&image_2), vec![9, 8, 7, 0, 1, 2, 3, 77]);
}

// Índice transparente.

#[test]
fn indexed_with_the_transparent_index_in_the_middle_gives_a_zero_and_keeps_the_rgb() {
    let mut p = palette(&[
        Rgba::new(1, 2, 3, 255),
        Rgba::new(40, 50, 60, 255), // el transparente: guardado opaco
        Rgba::new(7, 8, 9, 100),
    ]);
    p.set_transparent_index(Some(1)).unwrap();
    let mut image_2 = Image::new_indexed(3, 1, p).unwrap();
    image_2.set_pixel(0, 0, Pixel::Indexed(0)).unwrap();
    image_2.set_pixel(1, 0, Pixel::Indexed(1)).unwrap();
    image_2.set_pixel(2, 0, Pixel::Indexed(2)).unwrap();
    assert_eq!(
        to_rgba8(&image_2),
        vec![1, 2, 3, 255, 40, 50, 60, 0, 7, 8, 9, 100]
    );
}

#[test]
fn indexed_the_rest_of_the_indices_keep_their_stored_alpha_with_transparent_set() {
    let mut p = palette(&[
        Rgba::new(1, 2, 3, 0), // alfa guardado 0, pero no es el transparente
        Rgba::new(4, 5, 6, 200),
        Rgba::new(7, 8, 9, 255),
    ]);
    p.set_transparent_index(Some(2)).unwrap();
    let mut image_2 = Image::new_indexed(3, 1, p).unwrap();
    for x in 0..3u32 {
        image_2.set_pixel(x, 0, Pixel::Indexed(x as u8)).unwrap();
    }
    assert_eq!(
        to_rgba8(&image_2),
        vec![1, 2, 3, 0, 4, 5, 6, 200, 7, 8, 9, 0]
    );
}

#[test]
fn indexed_the_transparent_at_the_first_and_at_the_last() {
    for transparent in [0u8, 3] {
        let mut p = palette(&[
            Rgba::new(10, 11, 12, 255),
            Rgba::new(20, 21, 22, 255),
            Rgba::new(30, 31, 32, 255),
            Rgba::new(40, 41, 42, 255),
        ]);
        p.set_transparent_index(Some(transparent)).unwrap();
        let mut image_2 = Image::new_indexed(4, 1, p.clone()).unwrap();
        for x in 0..4u32 {
            image_2.set_pixel(x, 0, Pixel::Indexed(x as u8)).unwrap();
        }
        let bytes = to_rgba8(&image_2);
        for x in 0..4usize {
            let c = p.get(x as u8).unwrap();
            let a = if x as u8 == transparent { 0 } else { 255 };
            assert_eq!(
                &bytes[x * 4..x * 4 + 4],
                &[c.r, c.g, c.b, a],
                "transparente {transparent}, columna {x}"
            );
        }
    }
}

#[test]
fn indexed_follows_the_current_palette_of_the_image_without_using_old_values() {
    // Los índices no cambian pero la paleta sí: los bytes salen de la paleta de ese momento.
    let mut image_2 = Image::new_indexed(1, 1, palette(&[Rgba::new(1, 1, 1, 255)])).unwrap();
    assert_eq!(to_rgba8(&image_2), vec![1, 1, 1, 255]);
    let mut p = Palette::new();
    p.push(Rgba::new(9, 9, 9, 255)).unwrap();
    p.set_transparent_index(Some(0)).unwrap();
    // `Image` no expone la paleta mutable: se reconstruye con la nueva.
    image_2 = Image::new_indexed(1, 1, p).unwrap();
    assert_eq!(to_rgba8(&image_2), vec![9, 9, 9, 0]);
}

// Paleta de 256 entradas.

#[test]
fn indexed_with_a_palette_of_256_entries_uses_all_the_indices() {
    let colors_2: Vec<Rgba> = (0..256usize)
        .map(|i| {
            let b = i as u8;
            Rgba::new(b, 255 - b, b ^ 0x5a, b.wrapping_mul(3).wrapping_add(1))
        })
        .collect();
    let mut p = Palette::from_colors(colors_2.iter().copied()).unwrap();
    assert_eq!(p.len(), 256);
    p.set_transparent_index(Some(128)).unwrap();
    // 16 x 16: el píxel (x, y) usa el índice y * 16 + x, así que salen los 256 índices.
    let mut image_2 = Image::new_indexed(16, 16, p).unwrap();
    for y in 0..16u32 {
        for x in 0..16u32 {
            image_2
                .set_pixel(x, y, Pixel::Indexed((y * 16 + x) as u8))
                .unwrap();
        }
    }
    let bytes = to_rgba8(&image_2);
    assert_eq!(bytes.len(), 16 * 16 * 4);
    for (i, c) in colors_2.iter().enumerate() {
        let a = if i == 128 { 0 } else { c.a };
        assert_eq!(&bytes[i * 4..i * 4 + 4], &[c.r, c.g, c.b, a], "índice {i}");
    }
}

#[test]
fn indexed_with_maximum_index_255_and_transparent_255() {
    let colors_2: Vec<Rgba> = (0..256usize)
        .map(|i| Rgba::new(i as u8, 0, 0, 255))
        .collect();
    let mut p = Palette::from_colors(colors_2).unwrap();
    p.set_transparent_index(Some(255)).unwrap();
    let mut image_2 = Image::new_indexed(2, 1, p).unwrap();
    image_2.set_pixel(0, 0, Pixel::Indexed(254)).unwrap();
    image_2.set_pixel(1, 0, Pixel::Indexed(255)).unwrap();
    assert_eq!(to_rgba8(&image_2), vec![254, 0, 0, 255, 255, 0, 0, 0]);
}

// Imagen 1 x 1.

#[test]
fn image_1x1_of_each_mode() {
    for mode in TEST_ALL {
        let image_2 = fills(mode, 1, 1, 3);
        let bytes = to_rgba8(&image_2);
        assert_eq!(bytes.len(), 4, "{mode:?}");
        assert_eq!(&bytes[..], &expected_of(&image_2, 0, 0)[..], "{mode:?}");
    }
}

#[test]
fn image_1x1_concrete_values() {
    let mut rgba = Image::new(1, 1, Mode::Rgba).unwrap();
    rgba.set_pixel(0, 0, Pixel::Rgba(Rgba::new(11, 22, 33, 44)))
        .unwrap();
    assert_eq!(to_rgba8(&rgba), vec![11, 22, 33, 44]);

    let mut gray_2 = Image::new(1, 1, Mode::Gray).unwrap();
    gray_2.set_pixel(0, 0, Pixel::Gray(99)).unwrap();
    assert_eq!(to_rgba8(&gray_2), vec![99, 99, 99, 255]);

    let mut ga = Image::new(1, 1, Mode::GrayAlpha).unwrap();
    ga.set_pixel(0, 0, Pixel::GrayAlpha { gray: 99, alpha: 7 })
        .unwrap();
    assert_eq!(to_rgba8(&ga), vec![99, 99, 99, 7]);

    // Imagen nueva: todos los píxeles a cero; en indexado, el índice 0 de la paleta.
    let new = Image::new(1, 1, Mode::Rgba).unwrap();
    assert_eq!(to_rgba8(&new), vec![0, 0, 0, 0]);
    let new_gray = Image::new(1, 1, Mode::Gray).unwrap();
    assert_eq!(to_rgba8(&new_gray), vec![0, 0, 0, 255]);
    let new_indexed = Image::new_indexed(1, 1, palette(&[Rgba::new(5, 6, 7, 8)])).unwrap();
    assert_eq!(to_rgba8(&new_indexed), vec![5, 6, 7, 8]);
}

// Imágenes anchas y estrechas.

#[test]
fn wide_17x1_image_of_each_mode() {
    for mode in TEST_ALL {
        let image_2 = fills(mode, 17, 1, 5);
        let bytes = to_rgba8(&image_2);
        assert_eq!(bytes.len(), length(17, 1), "{mode:?}");
        for x in 0..17u32 {
            let at = x as usize * 4;
            assert_eq!(
                &bytes[at..at + 4],
                &expected_of(&image_2, x, 0)[..],
                "{mode:?}, columna {x}"
            );
        }
    }
}

#[test]
fn narrow_1x17_image_of_each_mode() {
    for mode in TEST_ALL {
        let image_2 = fills(mode, 1, 17, 5);
        let bytes = to_rgba8(&image_2);
        assert_eq!(bytes.len(), length(1, 17), "{mode:?}");
        for y in 0..17u32 {
            let at = y as usize * 4;
            assert_eq!(
                &bytes[at..at + 4],
                &expected_of(&image_2, 0, y)[..],
                "{mode:?}, fila {y}"
            );
        }
    }
}

#[test]
fn wide_rgba_17x1_image_concrete_values() {
    // Un solo píxel distinto: si se confundieran filas y columnas, la salida no cambiaría de
    // tamaño pero este píxel saldría en otro sitio.
    let mut image_2 = Image::new(17, 1, Mode::Rgba).unwrap();
    image_2
        .set_pixel(16, 0, Pixel::Rgba(Rgba::new(1, 2, 3, 4)))
        .unwrap();
    let bytes = to_rgba8(&image_2);
    assert_eq!(bytes.len(), 68);
    assert!(bytes[..64].iter().all(|&b| b == 0));
    assert_eq!(&bytes[64..], &[1, 2, 3, 4]);
}

#[test]
fn narrow_rgba_1x17_image_concrete_values() {
    let mut image_2 = Image::new(1, 17, Mode::Rgba).unwrap();
    image_2
        .set_pixel(0, 16, Pixel::Rgba(Rgba::new(1, 2, 3, 4)))
        .unwrap();
    let bytes = to_rgba8(&image_2);
    assert_eq!(bytes.len(), 68);
    assert!(bytes[..64].iter().all(|&b| b == 0));
    assert_eq!(&bytes[64..], &[1, 2, 3, 4]);
}

#[test]
fn the_top_row_goes_first_in_all_the_modes() {
    // 2 x 2 con un píxel distinto solo en la esquina superior derecha (x = 1, y = 0): sus
    // bytes son los segundos de la salida, no los de la fila de abajo.
    for mode in TEST_ALL {
        let mut image_2 = fills(mode, 2, 2, 4);
        let mark = pixel_de(mode, 200, 4);
        image_2.set_pixel(1, 0, mark).unwrap();
        let bytes = to_rgba8(&image_2);
        assert_eq!(bytes.len(), 16, "{mode:?}");
        assert_eq!(&bytes[4..8], &expected_of(&image_2, 1, 0)[..], "{mode:?}");
        assert_eq!(&bytes[8..12], &expected_of(&image_2, 0, 1)[..], "{mode:?}");
        assert_eq!(&bytes[12..16], &expected_of(&image_2, 1, 1)[..], "{mode:?}");
    }
}

// Propiedades.

fn dimensions() -> impl Strategy<Value = (u32, u32)> {
    (1u32..=16, 1u32..=16)
}

/// Imagen RGBA aleatoria de hasta 16 x 16, con los píxeles escritos con `set_pixel`.
fn image_rgba() -> impl Strategy<Value = Image> {
    dimensions()
        .prop_flat_map(|(w, h)| {
            (
                Just((w, h)),
                vec(any::<[u8; 4]>(), (w * h) as usize..=(w * h) as usize),
            )
        })
        .prop_map(|((w, h), pixels_2)| {
            let mut image_2 = Image::new(w, h, Mode::Rgba).unwrap();
            for (k, p) in pixels_2.iter().enumerate() {
                let c = Rgba::new(p[0], p[1], p[2], p[3]);
                image_2
                    .set_pixel(k as u32 % w, k as u32 / w, Pixel::Rgba(c))
                    .unwrap();
            }
            image_2
        })
}

/// Imagen indexada aleatoria de hasta 16 x 16: paleta de 1 a 256 colores, índice transparente
/// opcional e índices de píxel siempre dentro de la paleta.
fn image_indexed() -> impl Strategy<Value = Image> {
    (
        dimensions(),
        vec(any::<[u8; 4]>(), 1..=256usize),
        proptest::option::of(any::<u8>()),
    )
        .prop_flat_map(|((w, h), colors_2, transparent)| {
            let n = colors_2.len();
            (
                Just((w, h)),
                Just(colors_2),
                Just(transparent.map(|t| (usize::from(t) % n) as u8)),
                vec(0..n, (w * h) as usize..=(w * h) as usize),
            )
        })
        .prop_map(|((w, h), colors_2, transparent, indices)| {
            let mut p =
                Palette::from_colors(colors_2.iter().map(|c| Rgba::new(c[0], c[1], c[2], c[3])))
                    .unwrap();
            p.set_transparent_index(transparent).unwrap();
            let mut image_2 = Image::new_indexed(w, h, p).unwrap();
            for (k, &i) in indices.iter().enumerate() {
                image_2
                    .set_pixel(k as u32 % w, k as u32 / w, Pixel::Indexed(i as u8))
                    .unwrap();
            }
            image_2
        })
}

/// Imagen de grises o de grises con alfa aleatoria de hasta 16 x 16.
fn image_gray(mode: Mode) -> impl Strategy<Value = Image> {
    dimensions()
        .prop_flat_map(|(w, h)| {
            (
                Just((w, h)),
                vec(any::<[u8; 2]>(), (w * h) as usize..=(w * h) as usize),
            )
        })
        .prop_map(move |((w, h), pixels_2)| {
            let mut image_2 = Image::new(w, h, mode).unwrap();
            for (k, p) in pixels_2.iter().enumerate() {
                let pixel = match mode {
                    Mode::Gray => Pixel::Gray(p[0]),
                    _ => Pixel::GrayAlpha {
                        gray: p[0],
                        alpha: p[1],
                    },
                };
                image_2
                    .set_pixel(k as u32 % w, k as u32 / w, pixel)
                    .unwrap();
            }
            image_2
        })
}

proptest! {
    #[test]
    fn random_rgba_matches_get_pixel(image_2 in image_rgba()) {
        checks_against_get_pixel(&image_2)?;
        // En RGBA, además, los bytes son exactamente los de get_pixel sin transformar.
        let bytes = to_rgba8(&image_2);
        for y in 0..image_2.height() {
            for x in 0..image_2.width() {
                let Pixel::Rgba(c) = image_2.get_pixel(x, y).unwrap() else {
                    panic!("imagen RGBA con un píxel de otro modo");
                };
                let at = (y as usize * image_2.width() as usize + x as usize) * 4;
                prop_assert_eq!(&bytes[at..at + 4], &[c.r, c.g, c.b, c.a][..]);
            }
        }
    }

    #[test]
    fn random_indexed_matches_the_resolved_palette(image_2 in image_indexed()) {
        let palette = image_2.palette().unwrap();
        let bytes = to_rgba8(&image_2);
        prop_assert_eq!(bytes.len(), length(image_2.width(), image_2.height()));
        for y in 0..image_2.height() {
            for x in 0..image_2.width() {
                let Pixel::Indexed(i) = image_2.get_pixel(x, y).unwrap() else {
                    panic!("imagen indexada con un píxel de otro modo");
                };
                let c = palette.resolved(i).unwrap();
                let at = (y as usize * image_2.width() as usize + x as usize) * 4;
                prop_assert_eq!(
                    &bytes[at..at + 4],
                    &[c.r, c.g, c.b, c.a][..],
                    "píxel ({}, {}), índice {}", x, y, i
                );
            }
        }
    }

    #[test]
    fn random_gray_matches_get_pixel(image_2 in image_gray(Mode::Gray)) {
        checks_against_get_pixel(&image_2)?;
    }

    #[test]
    fn random_gray_alpha_matches_get_pixel(image_2 in image_gray(Mode::GrayAlpha)) {
        checks_against_get_pixel(&image_2)?;
    }

    #[test]
    fn the_length_is_width_times_height_times_4_in_all_modes(
        (w, h) in dimensions(),
        n in 1usize..=256,
    ) {
        for mode in TEST_ALL {
            let image_2 = fills(mode, w, h, n);
            prop_assert_eq!(to_rgba8(&image_2).len(), length(w, h), "{:?}", mode);
        }
    }

    #[test]
    fn the_conversion_does_not_modify_the_image_and_is_deterministic(image_2 in image_indexed()) {
        let before = image_2.clone();
        let a = to_rgba8(&image_2);
        let b = to_rgba8(&image_2);
        prop_assert_eq!(&image_2, &before);
        prop_assert_eq!(a, b);
    }
}
