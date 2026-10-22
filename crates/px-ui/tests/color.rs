//! Tests de `color` (specs/hito-2-lapiz-deshacer.md, sección 4; fila 11 y propiedad 5):
//! `SLOTS`, `pixel_for_slot(image, slot)` y `slot_label(image, slot)`.
//!
//! Las tablas de la spec se comprueban a mano, ranura por ranura; la propiedad 5 comprueba que
//! el color nunca es de otro modo ni de fuera de la paleta, para cualquier ranura de 0 a 255.

use proptest::prelude::*;
use px_core::ImageId;
use px_core::{Document, Image, Mode, Palette, Pixel, Rgba};
use px_ui::color::{SLOTS, pixel_for_slot, slot_label};
use px_ui::stroke::Stroke;

fn image_2(mode: Mode) -> Image {
    Image::new(4, 4, mode).unwrap()
}

fn indexed(colors_2: usize) -> Image {
    let palette =
        Palette::from_colors((0..colors_2).map(|i| Rgba::opaque((i % 20) as u8 * 10, 0, 0)))
            .unwrap();
    Image::new_indexed(4, 4, palette).unwrap()
}

fn rgba(r: u8, g: u8, b: u8) -> Pixel {
    Pixel::Rgba(Rgba::opaque(r, g, b))
}

#[test]
fn there_is_nine_slots() {
    assert_eq!(SLOTS, 9);
}

#[test]
fn row_11_examples_of_the_spec() {
    let color = image_2(Mode::Rgba);
    assert_eq!(pixel_for_slot(&color, 1), Some(rgba(0, 0, 0)));
    assert_eq!(
        pixel_for_slot(&color, 9),
        Some(Pixel::Rgba(Rgba::new(0, 0, 0, 0)))
    );

    let gray_2 = image_2(Mode::Gray);
    assert_eq!(pixel_for_slot(&gray_2, 8), Some(Pixel::Gray(224)));
    assert_eq!(pixel_for_slot(&gray_2, 9), Some(Pixel::Gray(255)));

    let gray_alpha = image_2(Mode::GrayAlpha);
    assert_eq!(
        pixel_for_slot(&gray_alpha, 9),
        Some(Pixel::GrayAlpha { gray: 0, alpha: 0 })
    );

    // Paleta de 3: las ranuras sobrantes se ignoran sin error.
    let three = indexed(3);
    assert_eq!(pixel_for_slot(&three, 3), Some(Pixel::Indexed(2)));
    assert_eq!(pixel_for_slot(&three, 4), None);

    for image in [
        image_2(Mode::Rgba),
        image_2(Mode::Gray),
        image_2(Mode::GrayAlpha),
        indexed(9),
    ] {
        assert_eq!(pixel_for_slot(&image, 0), None, "{:?}", image.mode());
        assert_eq!(pixel_for_slot(&image, 10), None, "{:?}", image.mode());
    }
}

#[test]
fn rgba_table_complete() {
    let image = image_2(Mode::Rgba);
    let expected_2 = [
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
    for (i, expected_2) in expected_2.into_iter().enumerate() {
        let slot = i as u8 + 1;
        assert_eq!(
            pixel_for_slot(&image, slot),
            Some(expected_2),
            "ranura {slot}"
        );
    }
}

#[test]
fn gray_full_table_and_the_nine_is_a_different_gray() {
    let image = image_2(Mode::Gray);
    let mut seen_2 = Vec::new();
    for slot in 1..=9u8 {
        let expected_2 = Pixel::Gray(((u32::from(slot) - 1) * 32).min(255) as u8);
        assert_eq!(
            pixel_for_slot(&image, slot),
            Some(expected_2),
            "ranura {slot}"
        );
        seen_2.push(expected_2);
    }
    assert_eq!(seen_2[0], Pixel::Gray(0));
    assert_eq!(seen_2[7], Pixel::Gray(224));
    assert_eq!(seen_2[8], Pixel::Gray(255));
    for (i, a) in seen_2.iter().enumerate() {
        for b in &seen_2[i + 1..] {
            assert_ne!(a, b, "las ranuras de Gray son todas distintas");
        }
    }
}

#[test]
fn gray_alpha_full_table() {
    let image = image_2(Mode::GrayAlpha);
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
fn indexed_uses_index_n_minus_one_as_long_as_there_is_a_palette() {
    for colors_2 in [1usize, 2, 5, 9, 10, 256] {
        let image = indexed(colors_2);
        for slot in 1..=9u8 {
            let expected_2 = if usize::from(slot) <= colors_2 {
                Some(Pixel::Indexed(slot - 1))
            } else {
                None
            };
            assert_eq!(
                pixel_for_slot(&image, slot),
                expected_2,
                "paleta de {colors_2}, ranura {slot}"
            );
        }
    }
}

#[test]
fn slot_label_with_the_formats_of_the_spec() {
    let color = image_2(Mode::Rgba);
    assert_eq!(slot_label(&color, 3).as_deref(), Some("color 3 #ff0000"));
    assert_eq!(slot_label(&color, 1).as_deref(), Some("color 1 #000000"));
    assert_eq!(slot_label(&color, 6).as_deref(), Some("color 6 #ffff00"));
    assert_eq!(slot_label(&color, 8).as_deref(), Some("color 8 #00ffff"));
    assert_eq!(
        slot_label(&color, 9).as_deref(),
        Some("color 9 transparente")
    );

    let palette = indexed(3);
    assert_eq!(slot_label(&palette, 2).as_deref(), Some("color 2 índice 1"));
    assert_eq!(slot_label(&palette, 1).as_deref(), Some("color 1 índice 0"));

    let gray_2 = image_2(Mode::Gray);
    assert_eq!(slot_label(&gray_2, 4).as_deref(), Some("color 4 gris 96"));
    assert_eq!(slot_label(&gray_2, 9).as_deref(), Some("color 9 gris 255"));

    // GrayAlpha: gris con alfa 255 en 1 a 8 y transparente en la 9.
    let gray_alpha = image_2(Mode::GrayAlpha);
    assert_eq!(
        slot_label(&gray_alpha, 4).as_deref(),
        Some("color 4 gris 96")
    );
    assert_eq!(
        slot_label(&gray_alpha, 9).as_deref(),
        Some("color 9 transparente")
    );
}

#[test]
fn slot_label_is_none_when_the_slot_is_none() {
    let three = indexed(3);
    assert_eq!(slot_label(&three, 4), None);
    assert_eq!(slot_label(&three, 9), None);
    for image in [
        image_2(Mode::Rgba),
        image_2(Mode::Gray),
        image_2(Mode::GrayAlpha),
        indexed(9),
    ] {
        assert_eq!(slot_label(&image, 0), None, "{:?}", image.mode());
        assert_eq!(slot_label(&image, 10), None, "{:?}", image.mode());
        assert_eq!(slot_label(&image, 255), None, "{:?}", image.mode());
    }
}

// Propiedad 5: el color nunca es del modo equivocado.

fn image_for(mode_2: usize, colors_2: usize, width: u32, height: u32) -> Image {
    match mode_2 {
        0 => Image::new(width, height, Mode::Rgba).unwrap(),
        1 => Image::new(width, height, Mode::Gray).unwrap(),
        2 => Image::new(width, height, Mode::GrayAlpha).unwrap(),
        _ => {
            let palette =
                Palette::from_colors((0..colors_2).map(|i| Rgba::opaque(i as u8 * 20, 1, 2)))
                    .unwrap();
            Image::new_indexed(width, height, palette).unwrap()
        }
    }
}

proptest! {
    #[test]
    fn the_color_of_a_slot_is_always_of_the_image_mode(
        mode_2 in 0usize..4,
        colors_2 in 1usize..=12,
        width in 1u32..=16,
        height in 1u32..=16,
        slot in any::<u8>(),
    ) {
        let image = image_for(mode_2, colors_2, width, height);
        let pixel = pixel_for_slot(&image, slot);

        // `None` solo con la ranura fuera de 1 a 9 o con indexado y `n > palette.len()`.
        let leftover = image.mode() == Mode::Indexed && usize::from(slot) > colors_2;
        prop_assert_eq!(pixel.is_none(), !(1..=9).contains(&slot) || leftover);
        // La etiqueta existe exactamente cuando hay color.
        prop_assert_eq!(slot_label(&image, slot).is_some(), pixel.is_some());

        if let Some(pixel) = pixel {
            prop_assert_eq!(image.check_pixel(pixel), Ok(()));

            // Un trazo con ese color nunca da `Err(Pixel { .. })`.
            let mut doc = Document::new(image);
            let id = ImageId::from_raw(0);
            let mut stroke = Stroke::new(width, height);
            stroke.move_to((0, 0), pixel);
            stroke.move_to((width as i32 - 1, height as i32 - 1), pixel);
            let result = stroke.commit(&mut doc, id);
            prop_assert!(
                result.is_ok(),
                "el trazo con la ranura {} dio {:?}",
                slot,
                result
            );
        }
    }
}

// Unidad 21 (specs/hito-2-fijar-transparente.md, fila 5): la bandera `alpha 0` de la línea 2 de
// la muestra y `is_alpha0_cell_with`.

mod alpha_0 {
    use super::*;
    use px_ui::color::{SlotColors, info_flags_alpha0, is_alpha0_cell_with};
    use px_ui::text::INTERFACE_CHARSET;

    /// Oráculo propio: las banderas que apliquen, en orden, unidas con ` · `.
    fn model(hover: bool, active: bool, in_range: bool, transparent: bool, alpha0: bool) -> String {
        let mut parts: Vec<&str> = Vec::new();
        if hover {
            parts.push("cursor");
        }
        if active {
            parts.push("activa");
        }
        if in_range && !active {
            parts.push("rango");
        }
        if transparent {
            parts.push("transparente");
        }
        // Excluyentes: si es la transparente, gana `transparent`.
        if alpha0 && !transparent {
            parts.push("alfa 0");
        }
        parts.join(" · ")
    }

    fn combinations() -> impl Iterator<Item = (bool, bool, bool, bool, bool)> {
        (0..32u8).map(|b| (b & 1 != 0, b & 2 != 0, b & 4 != 0, b & 8 != 0, b & 16 != 0))
    }

    #[test]
    fn info_flags_alpha0_of_the_spec() {
        assert_eq!(info_flags_alpha0(false, false, false, false, false), "");
        assert_eq!(
            info_flags_alpha0(false, false, false, true, false),
            "transparente"
        );
        assert_eq!(
            info_flags_alpha0(false, false, false, false, true),
            "alfa 0"
        );
        assert_eq!(
            info_flags_alpha0(true, true, false, true, false),
            "cursor · activa · transparente"
        );
        assert_eq!(
            info_flags_alpha0(true, true, false, false, true),
            "cursor · activa · alfa 0"
        );
        assert_eq!(
            info_flags_alpha0(true, false, true, false, true),
            "cursor · rango · alfa 0"
        );
        assert_eq!(
            info_flags_alpha0(false, false, true, true, false),
            "rango · transparente"
        );
    }

    #[test]
    fn info_flags_alpha0_follows_the_model_in_the_32_combinations() {
        for (h, a, r, t, z) in combinations() {
            assert_eq!(
                info_flags_alpha0(h, a, r, t, z),
                model(h, a, r, t, z),
                "h={h} a={a} r={r} t={t} z={z}"
            );
        }
    }

    #[test]
    fn transparent_and_alpha_0_are_exclusive_and_transparent_wins() {
        for (h, a, r, t, z) in combinations() {
            let line = info_flags_alpha0(h, a, r, t, z);
            assert!(
                !(line.contains("transparente") && line.contains("alfa 0")),
                "h={h} a={a} r={r} t={t} z={z}: {line:?}"
            );
            if t {
                assert!(line.contains("transparente"), "{line:?}");
                assert!(!line.contains("alfa 0"), "{line:?}");
            } else if z {
                assert!(line.contains("alfa 0"), "{line:?}");
                assert!(!line.contains("transparente"), "{line:?}");
            } else {
                assert!(!line.contains("alfa 0") && !line.contains("transparente"));
            }
        }
        assert_eq!(
            info_flags_alpha0(true, true, false, true, true),
            "cursor · activa · transparente"
        );
        assert_eq!(
            info_flags_alpha0(false, false, false, true, true),
            "transparente"
        );
        assert_eq!(
            info_flags_alpha0(false, false, false, false, true),
            "alfa 0"
        );
    }

    #[test]
    fn the_range_flag_only_comes_out_if_it_is_in_the_range_and_is_not_the_active_one() {
        for t in [false, true] {
            let with = info_flags_alpha0(false, false, true, t, !t);
            assert!(with.contains("rango"), "{with:?}");
            let active_2 = info_flags_alpha0(false, true, true, t, !t);
            assert!(!active_2.contains("rango"), "{active_2:?}");
            let outside = info_flags_alpha0(false, false, false, t, !t);
            assert!(!outside.contains("rango"), "{outside:?}");
        }
    }

    #[test]
    fn info_flags_alpha0_without_alpha_0_gives_the_usual_texts() {
        assert_eq!(
            info_flags_alpha0(true, true, false, true, false),
            "cursor · activa · transparente"
        );
        assert_eq!(
            info_flags_alpha0(true, false, true, true, false),
            "cursor · rango · transparente"
        );
        assert_eq!(info_flags_alpha0(false, false, false, false, false), "");
    }

    #[test]
    fn the_longest_line_is_30_columns_and_everything_comes_from_the_character_set() {
        let mut more_long = 0;
        for (h, a, r, t, z) in combinations() {
            let line = info_flags_alpha0(h, a, r, t, z);
            more_long = more_long.max(line.chars().count());
            assert!(line.chars().count() <= 32, "{line:?}");
            assert!(
                line.chars().all(|c| INTERFACE_CHARSET.contains(c)),
                "{line:?}"
            );
        }
        assert_eq!(more_long, 30);
        assert_eq!("cursor · activa · transparente".chars().count(), 30);
    }

    fn palette_with_alpha_0(transparent: Option<u8>) -> Image {
        // 0 opaca, 1 con alfa guardado 0, 2 opaca, 3 con alfa guardado 0.
        let mut palette = Palette::from_colors([
            Rgba::opaque(255, 0, 0),
            Rgba::new(0, 255, 0, 0),
            Rgba::opaque(0, 0, 255),
            Rgba::new(1, 1, 1, 0),
        ])
        .unwrap();
        palette.set_transparent_index(transparent).unwrap();
        Image::new_indexed(2, 2, palette).unwrap()
    }

    #[test]
    fn is_alpha0_cell_with_marks_those_with_stored_alpha_0_that_are_not_the_transparent_one() {
        let slots = SlotColors::defaults(Mode::Indexed);
        let without = palette_with_alpha_0(None);
        let expected_2 = [false, true, false, true];
        for (cel, expected_2) in expected_2.into_iter().enumerate() {
            assert_eq!(
                is_alpha0_cell_with(&without, cel, &slots),
                expected_2,
                "sin marca, {cel}"
            );
        }
        // Con la 3 marcada: la marcada ya dice `transparent`, no `alpha 0`.
        let marked = palette_with_alpha_0(Some(3));
        for (cel, expected_2) in [false, true, false, false].into_iter().enumerate() {
            assert_eq!(
                is_alpha0_cell_with(&marked, cel, &slots),
                expected_2,
                "con la 3 marcada, {cel}"
            );
        }
        // Una opaca marcada tampoco es alfa 0 (resuelve 0, pero el guardado es 255).
        let opaque = palette_with_alpha_0(Some(0));
        assert!(!is_alpha0_cell_with(&opaque, 0, &slots));
        assert!(is_alpha0_cell_with(&opaque, 1, &slots));
        assert!(is_alpha0_cell_with(&opaque, 3, &slots));
    }

    #[test]
    fn is_alpha0_cell_with_is_exclusive_with_is_transparent_cell_with() {
        use px_ui::color::is_transparent_cell_with;
        let slots = SlotColors::defaults(Mode::Indexed);
        for transparent in [None, Some(0), Some(1), Some(2), Some(3)] {
            let image_2 = palette_with_alpha_0(transparent);
            for cel in 0..4 {
                assert!(
                    !(is_alpha0_cell_with(&image_2, cel, &slots)
                        && is_transparent_cell_with(&image_2, cel, &slots)),
                    "{transparent:?}, celda {cel}"
                );
            }
        }
    }

    #[test]
    fn is_alpha0_cell_with_a_cell_outside_the_palette_is_false() {
        let slots = SlotColors::defaults(Mode::Indexed);
        let image_2 = palette_with_alpha_0(None);
        for cel in [4usize, 5, 255, 256, 1000] {
            assert!(!is_alpha0_cell_with(&image_2, cel, &slots), "{cel}");
        }
    }

    #[test]
    fn is_alpha0_cell_with_in_the_other_modes_is_always_false() {
        for mode_2 in [Mode::Rgba, Mode::Gray, Mode::GrayAlpha] {
            let image_2 = Image::new(2, 2, mode_2).unwrap();
            let slots = SlotColors::defaults(mode_2);
            for cel in 0..12 {
                assert!(
                    !is_alpha0_cell_with(&image_2, cel, &slots),
                    "{mode_2:?}, celda {cel}"
                );
            }
        }
    }
}
