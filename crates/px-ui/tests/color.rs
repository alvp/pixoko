//! Tests de `color` (specs/hito-2-lapiz-deshacer.md, sección 4; fila 11 y propiedad 5):
//! `SLOTS`, `pixel_for_slot(image, slot)` y `slot_label(image, slot)`.
//!
//! Las tablas de la spec se comprueban a mano, ranura por ranura; la propiedad 5 comprueba que
//! el color nunca es de otro modo ni de fuera de la paleta, para cualquier ranura de 0 a 255.

use proptest::prelude::*;
use px_core::{Document, Image, Mode, Palette, Pixel, Rgba};
use px_ui::color::{SLOTS, pixel_for_slot, slot_label};
use px_ui::stroke::Stroke;

fn imagen(mode: Mode) -> Image {
    Image::new(4, 4, mode).unwrap()
}

fn indexada(colores: usize) -> Image {
    let paleta =
        Palette::from_colors((0..colores).map(|i| Rgba::opaque((i % 20) as u8 * 10, 0, 0)))
            .unwrap();
    Image::new_indexed(4, 4, paleta).unwrap()
}

fn rgba(r: u8, g: u8, b: u8) -> Pixel {
    Pixel::Rgba(Rgba::opaque(r, g, b))
}

#[test]
fn hay_nueve_ranuras() {
    assert_eq!(SLOTS, 9);
}

#[test]
fn fila_11_ejemplos_de_la_spec() {
    let color = imagen(Mode::Rgba);
    assert_eq!(pixel_for_slot(&color, 1), Some(rgba(0, 0, 0)));
    assert_eq!(
        pixel_for_slot(&color, 9),
        Some(Pixel::Rgba(Rgba::new(0, 0, 0, 0)))
    );

    let gris = imagen(Mode::Gray);
    assert_eq!(pixel_for_slot(&gris, 8), Some(Pixel::Gray(224)));
    assert_eq!(pixel_for_slot(&gris, 9), Some(Pixel::Gray(255)));

    let gris_alfa = imagen(Mode::GrayAlpha);
    assert_eq!(
        pixel_for_slot(&gris_alfa, 9),
        Some(Pixel::GrayAlpha { gray: 0, alpha: 0 })
    );

    // Paleta de 3: las ranuras sobrantes se ignoran sin error.
    let tres = indexada(3);
    assert_eq!(pixel_for_slot(&tres, 3), Some(Pixel::Indexed(2)));
    assert_eq!(pixel_for_slot(&tres, 4), None);

    for image in [
        imagen(Mode::Rgba),
        imagen(Mode::Gray),
        imagen(Mode::GrayAlpha),
        indexada(9),
    ] {
        assert_eq!(pixel_for_slot(&image, 0), None, "{:?}", image.mode());
        assert_eq!(pixel_for_slot(&image, 10), None, "{:?}", image.mode());
    }
}

#[test]
fn rgba_tabla_completa() {
    let image = imagen(Mode::Rgba);
    let esperado = [
        rgba(0x00, 0x00, 0x00),
        rgba(0xff, 0xff, 0xff),
        rgba(0xff, 0x00, 0x00),
        rgba(0x00, 0xff, 0x00),
        rgba(0x00, 0x00, 0xff),
        rgba(0xff, 0xff, 0x00),
        rgba(0xff, 0x00, 0xff),
        rgba(0x00, 0xff, 0xff),
        Pixel::Rgba(Rgba::new(0, 0, 0, 0)),
    ];
    for (i, esperado) in esperado.into_iter().enumerate() {
        let slot = i as u8 + 1;
        assert_eq!(
            pixel_for_slot(&image, slot),
            Some(esperado),
            "ranura {slot}"
        );
    }
}

#[test]
fn gray_tabla_completa_y_la_nueve_es_un_gris_distinto() {
    let image = imagen(Mode::Gray);
    let mut vistos = Vec::new();
    for slot in 1..=9u8 {
        let esperado = Pixel::Gray(((u32::from(slot) - 1) * 32).min(255) as u8);
        assert_eq!(
            pixel_for_slot(&image, slot),
            Some(esperado),
            "ranura {slot}"
        );
        vistos.push(esperado);
    }
    assert_eq!(vistos[0], Pixel::Gray(0));
    assert_eq!(vistos[7], Pixel::Gray(224));
    assert_eq!(vistos[8], Pixel::Gray(255));
    for (i, a) in vistos.iter().enumerate() {
        for b in &vistos[i + 1..] {
            assert_ne!(a, b, "las ranuras de Gray son todas distintas");
        }
    }
}

#[test]
fn gray_alpha_tabla_completa() {
    let image = imagen(Mode::GrayAlpha);
    for slot in 1..=8u8 {
        let gray = ((u32::from(slot) - 1) * 32).min(255) as u8;
        assert_eq!(
            pixel_for_slot(&image, slot),
            Some(Pixel::GrayAlpha { gray, alpha: 255 }),
            "ranura {slot}"
        );
    }
    assert_eq!(
        pixel_for_slot(&image, 9),
        Some(Pixel::GrayAlpha { gray: 0, alpha: 0 })
    );
}

#[test]
fn indexed_usa_el_indice_n_menos_uno_mientras_haya_paleta() {
    for colores in [1usize, 2, 5, 9, 10, 256] {
        let image = indexada(colores);
        for slot in 1..=9u8 {
            let esperado = if usize::from(slot) <= colores {
                Some(Pixel::Indexed(slot - 1))
            } else {
                None
            };
            assert_eq!(
                pixel_for_slot(&image, slot),
                esperado,
                "paleta de {colores}, ranura {slot}"
            );
        }
    }
}

#[test]
fn slot_label_con_los_formatos_de_la_spec() {
    let color = imagen(Mode::Rgba);
    assert_eq!(slot_label(&color, 3).as_deref(), Some("color 3 #ff0000"));
    assert_eq!(slot_label(&color, 1).as_deref(), Some("color 1 #000000"));
    assert_eq!(slot_label(&color, 6).as_deref(), Some("color 6 #ffff00"));
    assert_eq!(slot_label(&color, 8).as_deref(), Some("color 8 #00ffff"));
    assert_eq!(
        slot_label(&color, 9).as_deref(),
        Some("color 9 transparente")
    );

    let paleta = indexada(3);
    assert_eq!(slot_label(&paleta, 2).as_deref(), Some("color 2 índice 1"));
    assert_eq!(slot_label(&paleta, 1).as_deref(), Some("color 1 índice 0"));

    let gris = imagen(Mode::Gray);
    assert_eq!(slot_label(&gris, 4).as_deref(), Some("color 4 gris 96"));
    assert_eq!(slot_label(&gris, 9).as_deref(), Some("color 9 gris 255"));

    // GrayAlpha: gris con alfa 255 en 1 a 8 y transparente en la 9.
    let gris_alfa = imagen(Mode::GrayAlpha);
    assert_eq!(
        slot_label(&gris_alfa, 4).as_deref(),
        Some("color 4 gris 96")
    );
    assert_eq!(
        slot_label(&gris_alfa, 9).as_deref(),
        Some("color 9 transparente")
    );
}

#[test]
fn slot_label_es_none_cuando_la_ranura_es_none() {
    let tres = indexada(3);
    assert_eq!(slot_label(&tres, 4), None);
    assert_eq!(slot_label(&tres, 9), None);
    for image in [
        imagen(Mode::Rgba),
        imagen(Mode::Gray),
        imagen(Mode::GrayAlpha),
        indexada(9),
    ] {
        assert_eq!(slot_label(&image, 0), None, "{:?}", image.mode());
        assert_eq!(slot_label(&image, 10), None, "{:?}", image.mode());
        assert_eq!(slot_label(&image, 255), None, "{:?}", image.mode());
    }
}

// Propiedad 5: el color nunca es del modo equivocado.

fn imagen_para(modo: usize, colores: usize, width: u32, height: u32) -> Image {
    match modo {
        0 => Image::new(width, height, Mode::Rgba).unwrap(),
        1 => Image::new(width, height, Mode::Gray).unwrap(),
        2 => Image::new(width, height, Mode::GrayAlpha).unwrap(),
        _ => {
            let paleta =
                Palette::from_colors((0..colores).map(|i| Rgba::opaque(i as u8 * 20, 1, 2)))
                    .unwrap();
            Image::new_indexed(width, height, paleta).unwrap()
        }
    }
}

proptest! {
    #[test]
    fn el_color_de_una_ranura_es_siempre_del_modo_de_la_imagen(
        modo in 0usize..4,
        colores in 1usize..=12,
        width in 1u32..=16,
        height in 1u32..=16,
        slot in any::<u8>(),
    ) {
        let image = imagen_para(modo, colores, width, height);
        let pixel = pixel_for_slot(&image, slot);

        // `None` solo con la ranura fuera de 1 a 9 o con indexado y `n > paleta.len()`.
        let sobrante = image.mode() == Mode::Indexed && usize::from(slot) > colores;
        prop_assert_eq!(pixel.is_none(), !(1..=9).contains(&slot) || sobrante);
        // La etiqueta existe exactamente cuando hay color.
        prop_assert_eq!(slot_label(&image, slot).is_some(), pixel.is_some());

        if let Some(pixel) = pixel {
            prop_assert_eq!(image.check_pixel(pixel), Ok(()));

            // Un trazo con ese color nunca da `Err(Pixel { .. })`.
            let mut doc = Document::new(image);
            let id = doc.main_image();
            let mut stroke = Stroke::new(width, height);
            stroke.move_to((0, 0), pixel);
            stroke.move_to((width as i32 - 1, height as i32 - 1), pixel);
            let resultado = stroke.commit(&mut doc, id);
            prop_assert!(
                resultado.is_ok(),
                "el trazo con la ranura {} dio {:?}",
                slot,
                resultado
            );
        }
    }
}
