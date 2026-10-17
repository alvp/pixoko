//! Tests de `texture::to_rgba8`, la conversión de `Image` a bytes RGBA
//! (specs/hito-2-ventana.md, «De `Image` a textura», primer punto).
//!
//! Contrato: `to_rgba8(&Image) -> Vec<u8>` devuelve `ancho · alto · 4` bytes, fila a fila
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

const TODOS: [Mode; 4] = [Mode::Rgba, Mode::Indexed, Mode::Gray, Mode::GrayAlpha];

fn paleta(colores: &[Rgba]) -> Palette {
    Palette::from_colors(colores.iter().copied()).unwrap()
}

/// Longitud esperada de los bytes de una imagen de `width` x `height`.
fn longitud(width: u32, height: u32) -> usize {
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
fn paleta_de(n: usize) -> Palette {
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
fn rellena(mode: Mode, width: u32, height: u32, n: usize) -> Image {
    let mut imagen = match mode {
        Mode::Indexed => Image::new_indexed(width, height, paleta_de(n)).unwrap(),
        _ => Image::new(width, height, mode).unwrap(),
    };
    for y in 0..height {
        for x in 0..width {
            let k = y as usize * width as usize + x as usize;
            imagen.set_pixel(x, y, pixel_de(mode, k, n)).unwrap();
        }
    }
    imagen
}

/// Los cuatro bytes que debe dar un píxel; salida de referencia escrita aparte de la función
/// probada, a partir de `get_pixel` y de la paleta de la imagen.
fn esperado_de(imagen: &Image, x: u32, y: u32) -> [u8; 4] {
    match imagen.get_pixel(x, y).unwrap() {
        Pixel::Rgba(c) => [c.r, c.g, c.b, c.a],
        Pixel::Indexed(i) => {
            let c = imagen.palette().unwrap().resolved(i).unwrap();
            [c.r, c.g, c.b, c.a]
        }
        Pixel::Gray(g) => [g, g, g, 255],
        Pixel::GrayAlpha { gray, alpha } => [gray, gray, gray, alpha],
    }
}

/// Comprueba longitud y, píxel a píxel, que los bytes de `to_rgba8` son los esperados.
fn comprueba_contra_get_pixel(imagen: &Image) -> Result<(), TestCaseError> {
    let (w, h) = (imagen.width(), imagen.height());
    let bytes = to_rgba8(imagen);
    prop_assert_eq!(bytes.len(), longitud(w, h));
    for y in 0..h {
        for x in 0..w {
            let at = (y as usize * w as usize + x as usize) * 4;
            prop_assert_eq!(
                &bytes[at..at + 4],
                &esperado_de(imagen, x, y)[..],
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
fn rgba_3x2_sale_tal_cual() {
    let mut imagen = Image::new(3, 2, Mode::Rgba).unwrap();
    // Fila de arriba: (1,2,3,4) (5,6,7,8) (9,10,11,12); fila de abajo: 13..=24.
    for y in 0..2u32 {
        for x in 0..3u32 {
            let b = ((y * 3 + x) * 4) as u8;
            let c = Rgba::new(b + 1, b + 2, b + 3, b + 4);
            imagen.set_pixel(x, y, Pixel::Rgba(c)).unwrap();
        }
    }
    let esperado: Vec<u8> = (1..=24).collect();
    assert_eq!(to_rgba8(&imagen), esperado);
}

#[test]
fn rgba_conserva_el_rgb_de_los_pixeles_transparentes_y_semitransparentes() {
    // Alfa directo: no se premultiplica ni se borra el RGB cuando a = 0.
    let mut imagen = Image::new(3, 1, Mode::Rgba).unwrap();
    imagen
        .set_pixel(0, 0, Pixel::Rgba(Rgba::new(200, 100, 50, 0)))
        .unwrap();
    imagen
        .set_pixel(1, 0, Pixel::Rgba(Rgba::new(200, 100, 50, 128)))
        .unwrap();
    imagen
        .set_pixel(2, 0, Pixel::Rgba(Rgba::new(200, 100, 50, 255)))
        .unwrap();
    assert_eq!(
        to_rgba8(&imagen),
        vec![200, 100, 50, 0, 200, 100, 50, 128, 200, 100, 50, 255]
    );
}

#[test]
fn gris_3x2_da_g_g_g_y_alfa_opaco() {
    let mut imagen = Image::new(3, 2, Mode::Gray).unwrap();
    let grises = [10u8, 20, 30, 40, 50, 60];
    for (k, &g) in grises.iter().enumerate() {
        imagen
            .set_pixel(k as u32 % 3, k as u32 / 3, Pixel::Gray(g))
            .unwrap();
    }
    let esperado = vec![
        10, 10, 10, 255, 20, 20, 20, 255, 30, 30, 30, 255, // fila de arriba
        40, 40, 40, 255, 50, 50, 50, 255, 60, 60, 60, 255, // fila de abajo
    ];
    assert_eq!(to_rgba8(&imagen), esperado);
}

#[test]
fn gris_extremos_0_y_255() {
    let mut imagen = Image::new(2, 1, Mode::Gray).unwrap();
    imagen.set_pixel(0, 0, Pixel::Gray(0)).unwrap();
    imagen.set_pixel(1, 0, Pixel::Gray(255)).unwrap();
    assert_eq!(to_rgba8(&imagen), vec![0, 0, 0, 255, 255, 255, 255, 255]);
}

#[test]
fn gris_con_alfa_3x2_da_g_g_g_a() {
    let mut imagen = Image::new(3, 2, Mode::GrayAlpha).unwrap();
    let pares = [
        (10u8, 200u8),
        (20, 190),
        (30, 0),
        (40, 255),
        (50, 128),
        (60, 1),
    ];
    for (k, &(gray, alpha)) in pares.iter().enumerate() {
        imagen
            .set_pixel(k as u32 % 3, k as u32 / 3, Pixel::GrayAlpha { gray, alpha })
            .unwrap();
    }
    let esperado = vec![
        10, 10, 10, 200, 20, 20, 20, 190, 30, 30, 30, 0, // fila de arriba
        40, 40, 40, 255, 50, 50, 50, 128, 60, 60, 60, 1, // fila de abajo
    ];
    assert_eq!(to_rgba8(&imagen), esperado);
}

#[test]
fn indexado_3x2_usa_la_paleta() {
    let p = paleta(&[
        Rgba::new(1, 2, 3, 255),
        Rgba::new(4, 5, 6, 128),
        Rgba::new(7, 8, 9, 64),
        Rgba::new(10, 11, 12, 255),
    ]);
    let mut imagen = Image::new_indexed(3, 2, p).unwrap();
    let indices = [3u8, 0, 2, 1, 1, 3];
    for (k, &i) in indices.iter().enumerate() {
        imagen
            .set_pixel(k as u32 % 3, k as u32 / 3, Pixel::Indexed(i))
            .unwrap();
    }
    let esperado = vec![
        10, 11, 12, 255, 1, 2, 3, 255, 7, 8, 9, 64, // fila de arriba
        4, 5, 6, 128, 4, 5, 6, 128, 10, 11, 12, 255, // fila de abajo
    ];
    assert_eq!(to_rgba8(&imagen), esperado);
}

#[test]
fn indexado_sin_indice_transparente_conserva_el_alfa_guardado_incluso_cero() {
    let p = paleta(&[Rgba::new(9, 8, 7, 0), Rgba::new(1, 2, 3, 77)]);
    assert_eq!(p.transparent_index(), None);
    let mut imagen = Image::new_indexed(2, 1, p).unwrap();
    imagen.set_pixel(1, 0, Pixel::Indexed(1)).unwrap();
    assert_eq!(to_rgba8(&imagen), vec![9, 8, 7, 0, 1, 2, 3, 77]);
}

// Índice transparente.

#[test]
fn indexado_con_indice_transparente_en_medio_da_a_cero_y_conserva_el_rgb() {
    let mut p = paleta(&[
        Rgba::new(1, 2, 3, 255),
        Rgba::new(40, 50, 60, 255), // el transparente: guardado opaco
        Rgba::new(7, 8, 9, 100),
    ]);
    p.set_transparent_index(Some(1)).unwrap();
    let mut imagen = Image::new_indexed(3, 1, p).unwrap();
    imagen.set_pixel(0, 0, Pixel::Indexed(0)).unwrap();
    imagen.set_pixel(1, 0, Pixel::Indexed(1)).unwrap();
    imagen.set_pixel(2, 0, Pixel::Indexed(2)).unwrap();
    assert_eq!(
        to_rgba8(&imagen),
        vec![1, 2, 3, 255, 40, 50, 60, 0, 7, 8, 9, 100]
    );
}

#[test]
fn indexado_el_resto_de_indices_conserva_su_alfa_guardado_con_transparente_fijado() {
    let mut p = paleta(&[
        Rgba::new(1, 2, 3, 0), // alfa guardado 0, pero no es el transparente
        Rgba::new(4, 5, 6, 200),
        Rgba::new(7, 8, 9, 255),
    ]);
    p.set_transparent_index(Some(2)).unwrap();
    let mut imagen = Image::new_indexed(3, 1, p).unwrap();
    for x in 0..3u32 {
        imagen.set_pixel(x, 0, Pixel::Indexed(x as u8)).unwrap();
    }
    assert_eq!(
        to_rgba8(&imagen),
        vec![1, 2, 3, 0, 4, 5, 6, 200, 7, 8, 9, 0]
    );
}

#[test]
fn indexado_el_transparente_en_el_primero_y_en_el_ultimo() {
    for transparente in [0u8, 3] {
        let mut p = paleta(&[
            Rgba::new(10, 11, 12, 255),
            Rgba::new(20, 21, 22, 255),
            Rgba::new(30, 31, 32, 255),
            Rgba::new(40, 41, 42, 255),
        ]);
        p.set_transparent_index(Some(transparente)).unwrap();
        let mut imagen = Image::new_indexed(4, 1, p.clone()).unwrap();
        for x in 0..4u32 {
            imagen.set_pixel(x, 0, Pixel::Indexed(x as u8)).unwrap();
        }
        let bytes = to_rgba8(&imagen);
        for x in 0..4usize {
            let c = p.get(x as u8).unwrap();
            let a = if x as u8 == transparente { 0 } else { 255 };
            assert_eq!(
                &bytes[x * 4..x * 4 + 4],
                &[c.r, c.g, c.b, a],
                "transparente {transparente}, columna {x}"
            );
        }
    }
}

#[test]
fn indexado_sigue_la_paleta_actual_de_la_imagen_sin_usar_valores_antiguos() {
    // Los índices no cambian pero la paleta sí: los bytes salen de la paleta de ese momento.
    let mut imagen = Image::new_indexed(1, 1, paleta(&[Rgba::new(1, 1, 1, 255)])).unwrap();
    assert_eq!(to_rgba8(&imagen), vec![1, 1, 1, 255]);
    let mut p = Palette::new();
    p.push(Rgba::new(9, 9, 9, 255)).unwrap();
    p.set_transparent_index(Some(0)).unwrap();
    // `Image` no expone la paleta mutable: se reconstruye con la nueva.
    imagen = Image::new_indexed(1, 1, p).unwrap();
    assert_eq!(to_rgba8(&imagen), vec![9, 9, 9, 0]);
}

// Paleta de 256 entradas.

#[test]
fn indexado_con_paleta_de_256_entradas_usa_todos_los_indices() {
    let colores: Vec<Rgba> = (0..256usize)
        .map(|i| {
            let b = i as u8;
            Rgba::new(b, 255 - b, b ^ 0x5a, b.wrapping_mul(3).wrapping_add(1))
        })
        .collect();
    let mut p = Palette::from_colors(colores.iter().copied()).unwrap();
    assert_eq!(p.len(), 256);
    p.set_transparent_index(Some(128)).unwrap();
    // 16 x 16: el píxel (x, y) usa el índice y * 16 + x, así que salen los 256 índices.
    let mut imagen = Image::new_indexed(16, 16, p).unwrap();
    for y in 0..16u32 {
        for x in 0..16u32 {
            imagen
                .set_pixel(x, y, Pixel::Indexed((y * 16 + x) as u8))
                .unwrap();
        }
    }
    let bytes = to_rgba8(&imagen);
    assert_eq!(bytes.len(), 16 * 16 * 4);
    for (i, c) in colores.iter().enumerate() {
        let a = if i == 128 { 0 } else { c.a };
        assert_eq!(&bytes[i * 4..i * 4 + 4], &[c.r, c.g, c.b, a], "índice {i}");
    }
}

#[test]
fn indexado_con_indice_maximo_255_y_transparente_255() {
    let colores: Vec<Rgba> = (0..256usize)
        .map(|i| Rgba::new(i as u8, 0, 0, 255))
        .collect();
    let mut p = Palette::from_colors(colores).unwrap();
    p.set_transparent_index(Some(255)).unwrap();
    let mut imagen = Image::new_indexed(2, 1, p).unwrap();
    imagen.set_pixel(0, 0, Pixel::Indexed(254)).unwrap();
    imagen.set_pixel(1, 0, Pixel::Indexed(255)).unwrap();
    assert_eq!(to_rgba8(&imagen), vec![254, 0, 0, 255, 255, 0, 0, 0]);
}

// Imagen 1 x 1.

#[test]
fn imagen_1x1_de_cada_modo() {
    for mode in TODOS {
        let imagen = rellena(mode, 1, 1, 3);
        let bytes = to_rgba8(&imagen);
        assert_eq!(bytes.len(), 4, "{mode:?}");
        assert_eq!(&bytes[..], &esperado_de(&imagen, 0, 0)[..], "{mode:?}");
    }
}

#[test]
fn imagen_1x1_valores_concretos() {
    let mut rgba = Image::new(1, 1, Mode::Rgba).unwrap();
    rgba.set_pixel(0, 0, Pixel::Rgba(Rgba::new(11, 22, 33, 44)))
        .unwrap();
    assert_eq!(to_rgba8(&rgba), vec![11, 22, 33, 44]);

    let mut gris = Image::new(1, 1, Mode::Gray).unwrap();
    gris.set_pixel(0, 0, Pixel::Gray(99)).unwrap();
    assert_eq!(to_rgba8(&gris), vec![99, 99, 99, 255]);

    let mut ga = Image::new(1, 1, Mode::GrayAlpha).unwrap();
    ga.set_pixel(0, 0, Pixel::GrayAlpha { gray: 99, alpha: 7 })
        .unwrap();
    assert_eq!(to_rgba8(&ga), vec![99, 99, 99, 7]);

    // Imagen nueva: todos los píxeles a cero; en indexado, el índice 0 de la paleta.
    let nueva = Image::new(1, 1, Mode::Rgba).unwrap();
    assert_eq!(to_rgba8(&nueva), vec![0, 0, 0, 0]);
    let nueva_gris = Image::new(1, 1, Mode::Gray).unwrap();
    assert_eq!(to_rgba8(&nueva_gris), vec![0, 0, 0, 255]);
    let nueva_indexada = Image::new_indexed(1, 1, paleta(&[Rgba::new(5, 6, 7, 8)])).unwrap();
    assert_eq!(to_rgba8(&nueva_indexada), vec![5, 6, 7, 8]);
}

// Imágenes anchas y estrechas.

#[test]
fn imagen_ancha_17x1_de_cada_modo() {
    for mode in TODOS {
        let imagen = rellena(mode, 17, 1, 5);
        let bytes = to_rgba8(&imagen);
        assert_eq!(bytes.len(), longitud(17, 1), "{mode:?}");
        for x in 0..17u32 {
            let at = x as usize * 4;
            assert_eq!(
                &bytes[at..at + 4],
                &esperado_de(&imagen, x, 0)[..],
                "{mode:?}, columna {x}"
            );
        }
    }
}

#[test]
fn imagen_estrecha_1x17_de_cada_modo() {
    for mode in TODOS {
        let imagen = rellena(mode, 1, 17, 5);
        let bytes = to_rgba8(&imagen);
        assert_eq!(bytes.len(), longitud(1, 17), "{mode:?}");
        for y in 0..17u32 {
            let at = y as usize * 4;
            assert_eq!(
                &bytes[at..at + 4],
                &esperado_de(&imagen, 0, y)[..],
                "{mode:?}, fila {y}"
            );
        }
    }
}

#[test]
fn imagen_ancha_rgba_17x1_valores_concretos() {
    // Un solo píxel distinto: si se confundieran filas y columnas, la salida no cambiaría de
    // tamaño pero este píxel saldría en otro sitio.
    let mut imagen = Image::new(17, 1, Mode::Rgba).unwrap();
    imagen
        .set_pixel(16, 0, Pixel::Rgba(Rgba::new(1, 2, 3, 4)))
        .unwrap();
    let bytes = to_rgba8(&imagen);
    assert_eq!(bytes.len(), 68);
    assert!(bytes[..64].iter().all(|&b| b == 0));
    assert_eq!(&bytes[64..], &[1, 2, 3, 4]);
}

#[test]
fn imagen_estrecha_rgba_1x17_valores_concretos() {
    let mut imagen = Image::new(1, 17, Mode::Rgba).unwrap();
    imagen
        .set_pixel(0, 16, Pixel::Rgba(Rgba::new(1, 2, 3, 4)))
        .unwrap();
    let bytes = to_rgba8(&imagen);
    assert_eq!(bytes.len(), 68);
    assert!(bytes[..64].iter().all(|&b| b == 0));
    assert_eq!(&bytes[64..], &[1, 2, 3, 4]);
}

#[test]
fn la_fila_de_arriba_va_primero_en_todos_los_modos() {
    // 2 x 2 con un píxel distinto solo en la esquina superior derecha (x = 1, y = 0): sus
    // bytes son los segundos de la salida, no los de la fila de abajo.
    for mode in TODOS {
        let mut imagen = rellena(mode, 2, 2, 4);
        let marca = pixel_de(mode, 200, 4);
        imagen.set_pixel(1, 0, marca).unwrap();
        let bytes = to_rgba8(&imagen);
        assert_eq!(bytes.len(), 16, "{mode:?}");
        assert_eq!(&bytes[4..8], &esperado_de(&imagen, 1, 0)[..], "{mode:?}");
        assert_eq!(&bytes[8..12], &esperado_de(&imagen, 0, 1)[..], "{mode:?}");
        assert_eq!(&bytes[12..16], &esperado_de(&imagen, 1, 1)[..], "{mode:?}");
    }
}

// Propiedades.

fn dimensiones() -> impl Strategy<Value = (u32, u32)> {
    (1u32..=16, 1u32..=16)
}

/// Imagen RGBA aleatoria de hasta 16 x 16, con los píxeles escritos con `set_pixel`.
fn imagen_rgba() -> impl Strategy<Value = Image> {
    dimensiones()
        .prop_flat_map(|(w, h)| {
            (
                Just((w, h)),
                vec(any::<[u8; 4]>(), (w * h) as usize..=(w * h) as usize),
            )
        })
        .prop_map(|((w, h), pixeles)| {
            let mut imagen = Image::new(w, h, Mode::Rgba).unwrap();
            for (k, p) in pixeles.iter().enumerate() {
                let c = Rgba::new(p[0], p[1], p[2], p[3]);
                imagen
                    .set_pixel(k as u32 % w, k as u32 / w, Pixel::Rgba(c))
                    .unwrap();
            }
            imagen
        })
}

/// Imagen indexada aleatoria de hasta 16 x 16: paleta de 1 a 256 colores, índice transparente
/// opcional e índices de píxel siempre dentro de la paleta.
fn imagen_indexada() -> impl Strategy<Value = Image> {
    (
        dimensiones(),
        vec(any::<[u8; 4]>(), 1..=256usize),
        proptest::option::of(any::<u8>()),
    )
        .prop_flat_map(|((w, h), colores, transparente)| {
            let n = colores.len();
            (
                Just((w, h)),
                Just(colores),
                Just(transparente.map(|t| (usize::from(t) % n) as u8)),
                vec(0..n, (w * h) as usize..=(w * h) as usize),
            )
        })
        .prop_map(|((w, h), colores, transparente, indices)| {
            let mut p =
                Palette::from_colors(colores.iter().map(|c| Rgba::new(c[0], c[1], c[2], c[3])))
                    .unwrap();
            p.set_transparent_index(transparente).unwrap();
            let mut imagen = Image::new_indexed(w, h, p).unwrap();
            for (k, &i) in indices.iter().enumerate() {
                imagen
                    .set_pixel(k as u32 % w, k as u32 / w, Pixel::Indexed(i as u8))
                    .unwrap();
            }
            imagen
        })
}

/// Imagen de grises o de grises con alfa aleatoria de hasta 16 x 16.
fn imagen_gris(mode: Mode) -> impl Strategy<Value = Image> {
    dimensiones()
        .prop_flat_map(|(w, h)| {
            (
                Just((w, h)),
                vec(any::<[u8; 2]>(), (w * h) as usize..=(w * h) as usize),
            )
        })
        .prop_map(move |((w, h), pixeles)| {
            let mut imagen = Image::new(w, h, mode).unwrap();
            for (k, p) in pixeles.iter().enumerate() {
                let pixel = match mode {
                    Mode::Gray => Pixel::Gray(p[0]),
                    _ => Pixel::GrayAlpha {
                        gray: p[0],
                        alpha: p[1],
                    },
                };
                imagen.set_pixel(k as u32 % w, k as u32 / w, pixel).unwrap();
            }
            imagen
        })
}

proptest! {
    #[test]
    fn rgba_aleatoria_coincide_con_get_pixel(imagen in imagen_rgba()) {
        comprueba_contra_get_pixel(&imagen)?;
        // En RGBA, además, los bytes son exactamente los de get_pixel sin transformar.
        let bytes = to_rgba8(&imagen);
        for y in 0..imagen.height() {
            for x in 0..imagen.width() {
                let Pixel::Rgba(c) = imagen.get_pixel(x, y).unwrap() else {
                    panic!("imagen RGBA con un píxel de otro modo");
                };
                let at = (y as usize * imagen.width() as usize + x as usize) * 4;
                prop_assert_eq!(&bytes[at..at + 4], &[c.r, c.g, c.b, c.a][..]);
            }
        }
    }

    #[test]
    fn indexada_aleatoria_coincide_con_la_paleta_resuelta(imagen in imagen_indexada()) {
        let paleta = imagen.palette().unwrap();
        let bytes = to_rgba8(&imagen);
        prop_assert_eq!(bytes.len(), longitud(imagen.width(), imagen.height()));
        for y in 0..imagen.height() {
            for x in 0..imagen.width() {
                let Pixel::Indexed(i) = imagen.get_pixel(x, y).unwrap() else {
                    panic!("imagen indexada con un píxel de otro modo");
                };
                let c = paleta.resolved(i).unwrap();
                let at = (y as usize * imagen.width() as usize + x as usize) * 4;
                prop_assert_eq!(
                    &bytes[at..at + 4],
                    &[c.r, c.g, c.b, c.a][..],
                    "píxel ({}, {}), índice {}", x, y, i
                );
            }
        }
    }

    #[test]
    fn gris_aleatoria_coincide_con_get_pixel(imagen in imagen_gris(Mode::Gray)) {
        comprueba_contra_get_pixel(&imagen)?;
    }

    #[test]
    fn gris_con_alfa_aleatoria_coincide_con_get_pixel(imagen in imagen_gris(Mode::GrayAlpha)) {
        comprueba_contra_get_pixel(&imagen)?;
    }

    #[test]
    fn la_longitud_es_ancho_por_alto_por_4_en_todos_los_modos(
        (w, h) in dimensiones(),
        n in 1usize..=256,
    ) {
        for mode in TODOS {
            let imagen = rellena(mode, w, h, n);
            prop_assert_eq!(to_rgba8(&imagen).len(), longitud(w, h), "{:?}", mode);
        }
    }

    #[test]
    fn la_conversion_no_modifica_la_imagen_y_es_determinista(imagen in imagen_indexada()) {
        let antes = imagen.clone();
        let a = to_rgba8(&imagen);
        let b = to_rgba8(&imagen);
        prop_assert_eq!(&imagen, &antes);
        prop_assert_eq!(a, b);
    }
}
