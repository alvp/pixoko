//! Tests de `ColorChoice` y las piezas de selección de `color` (specs/hito-2-panel-paletas.md,
//! unidad 11b): `pixel_for_choice`, `initial_choice`, `cell_count`, `choice_for_cell`,
//! `cell_of_choice`, `key_choice`, `cell_color`, `is_transparent_cell`, `cell_label`,
//! `info_flags` y `choice_label`.
//!
//! Cubre las filas 6 y 7 (la parte de `px-ui`: elegir y celdas inexistentes), la 8 (solo
//! `initial_choice`; lo de `Session` es de `px-app`), la 9 (textos), la parte de selección de la
//! 11 (transparente en 0, 17, 255 y ninguno; paletas con colores repetidos; las nueve ranuras de
//! los modos sin paleta) y la propiedad 4 en su parte pura.
//!
//! Las muestras («celdas») se numeran desde 0; las ranuras y las teclas, desde 1. Un clic en la
//! celda `k` y la tecla `k + 1` dan la misma `ColorChoice`. Los textos de las filas 9 y 11 están
//! escritos con las cadenas literales de la spec.

use proptest::collection::vec;
use proptest::prelude::*;
use px_core::{Image, Mode, Palette, Pixel, Rgba};
use px_ui::color::{
    ColorChoice, SLOTS, cell_color, cell_count, cell_label, cell_of_choice, choice_for_cell,
    choice_label, info_flags_alpha0, initial_choice, is_transparent_cell, key_choice,
    pixel_for_choice, pixel_for_slot,
};

fn image_2(mode: Mode) -> Image {
    Image::new(4, 4, mode).unwrap()
}

/// Indexada con los colores dados y el índice transparente indicado.
fn indexed_with(colors_2: Vec<Rgba>, transparent: Option<u8>) -> Image {
    let mut palette = Palette::from_colors(colors_2).unwrap();
    palette.set_transparent_index(transparent).unwrap();
    Image::new_indexed(4, 4, palette).unwrap()
}

/// Indexada de `n` entradas opacas y distintas entre sí, sin índice transparente.
fn indexed(n: usize) -> Image {
    indexed_with(
        (0..n)
            .map(|i| Rgba::opaque(i as u8, 255 - i as u8, 7))
            .collect(),
        None,
    )
}

fn rgba(r: u8, g: u8, b: u8) -> Rgba {
    Rgba::opaque(r, g, b)
}

/// Las cuatro formas de imagen que se prueban siempre: tres sin paleta, y indexadas de 3 y 256.
fn all_2() -> Vec<(&'static str, Image)> {
    vec![
        ("rgba", image_2(Mode::Rgba)),
        ("gray", image_2(Mode::Gray)),
        ("gray alpha", image_2(Mode::GrayAlpha)),
        ("indexada de 1", indexed(1)),
        ("indexada de 3", indexed(3)),
        ("indexada de 9", indexed(9)),
        ("indexada de 256", indexed(256)),
    ]
}

fn without_palette() -> Vec<(&'static str, Image)> {
    vec![
        ("rgba", image_2(Mode::Rgba)),
        ("gray", image_2(Mode::Gray)),
        ("gray alpha", image_2(Mode::GrayAlpha)),
    ]
}

// `ColorChoice` y `pixel_for_choice`.

#[test]
fn color_choice_es_copy_debug_y_comparable() {
    fn checks<T: Clone + Copy + std::fmt::Debug + PartialEq + Eq>() {}
    checks::<ColorChoice>();
    assert_ne!(ColorChoice::Slot(3), ColorChoice::Entry(3));
    assert_eq!(ColorChoice::Entry(3), ColorChoice::Entry(3));
}

#[test]
fn from_u8_gives_a_slot() {
    assert_eq!(ColorChoice::from(3u8), ColorChoice::Slot(3));
    assert_eq!(ColorChoice::from(0u8), ColorChoice::Slot(0));
    assert_eq!(ColorChoice::from(255u8), ColorChoice::Slot(255));
    let choice_2: ColorChoice = 9u8.into();
    assert_eq!(choice_2, ColorChoice::Slot(9));
}

#[test]
fn pixel_for_choice_with_a_slot_is_pixel_for_slot_in_the_four_modes() {
    let images = [
        image_2(Mode::Rgba),
        image_2(Mode::Gray),
        image_2(Mode::GrayAlpha),
        indexed(3),
        indexed(9),
        indexed(256),
    ];
    for image in &images {
        for n in 0..=10u8 {
            assert_eq!(
                pixel_for_choice(image, ColorChoice::Slot(n)),
                pixel_for_slot(image, n),
                "{:?}, ranura {n}",
                image.mode()
            );
        }
    }
    // Con ejemplos concretos: la ranura vale también en las indexadas, como hoy.
    assert_eq!(
        pixel_for_choice(&indexed(3), ColorChoice::Slot(3)),
        Some(Pixel::Indexed(2))
    );
    assert_eq!(pixel_for_choice(&indexed(3), ColorChoice::Slot(4)), None);
}

#[test]
fn pixel_for_choice_with_an_entry_in_an_indexed_one() {
    let image = indexed(256);
    for i in [0u8, 1, 8, 200, 255] {
        assert_eq!(
            pixel_for_choice(&image, ColorChoice::Entry(i)),
            Some(Pixel::Indexed(i))
        );
    }
    let three = indexed(3);
    assert_eq!(
        pixel_for_choice(&three, ColorChoice::Entry(2)),
        Some(Pixel::Indexed(2))
    );
    assert_eq!(pixel_for_choice(&three, ColorChoice::Entry(3)), None);
    assert_eq!(pixel_for_choice(&three, ColorChoice::Entry(255)), None);
}

#[test]
fn pixel_for_choice_with_an_entry_in_a_non_indexed_image_is_none() {
    for (name_2, image) in without_palette() {
        for i in [0u8, 1, 8, 200, 255] {
            assert_eq!(
                pixel_for_choice(&image, ColorChoice::Entry(i)),
                None,
                "{name_2}, Entry({i})"
            );
        }
    }
}

// Fila 8 (parte de px-ui): `initial_choice`.

#[test]
fn row_8_initial_choice() {
    assert_eq!(initial_choice(&image_2(Mode::Rgba)), ColorChoice::Slot(1));
    assert_eq!(initial_choice(&image_2(Mode::Gray)), ColorChoice::Slot(1));
    assert_eq!(
        initial_choice(&image_2(Mode::GrayAlpha)),
        ColorChoice::Slot(1)
    );
    for n in [1usize, 2, 3, 9, 256] {
        assert_eq!(
            initial_choice(&indexed(n)),
            ColorChoice::Entry(0),
            "indexada de {n}"
        );
    }
}

#[test]
fn the_initial_choice_is_valid_for_its_image() {
    for (name_2, image) in all_2() {
        let initial = initial_choice(&image);
        assert!(
            pixel_for_choice(&image, initial).is_some(),
            "{name_2}: {initial:?}"
        );
        assert_eq!(cell_of_choice(&image, initial), Some(0), "{name_2}");
        assert_eq!(choice_for_cell(&image, 0), Some(initial), "{name_2}");
    }
}

// `cell_count`.

#[test]
fn cell_count_is_the_palette_or_nine() {
    assert_eq!(SLOTS, 9);
    for (name_2, image) in without_palette() {
        assert_eq!(cell_count(&image), 9, "{name_2}");
    }
    for n in [1usize, 2, 3, 9, 10, 255, 256] {
        assert_eq!(cell_count(&indexed(n)), n, "indexada de {n}");
    }
}

// Fila 6: elegir (parte de px-ui).

#[test]
fn row_6_indexed_of_256_click_on_the_200_and_on_the_8_and_keys_1_and_9() {
    let image = indexed(256);
    assert_eq!(choice_for_cell(&image, 200), Some(ColorChoice::Entry(200)));
    assert_eq!(
        pixel_for_choice(&image, ColorChoice::Entry(200)),
        Some(Pixel::Indexed(200))
    );
    assert_eq!(choice_for_cell(&image, 8), Some(ColorChoice::Entry(8)));
    assert_eq!(key_choice(&image, 9), Some(ColorChoice::Entry(8)));
    assert_eq!(key_choice(&image, 1), Some(ColorChoice::Entry(0)));
    // `cell_of_choice` da la muestra pulsada (o la de la tecla menos 1).
    assert_eq!(cell_of_choice(&image, ColorChoice::Entry(200)), Some(200));
    assert_eq!(cell_of_choice(&image, ColorChoice::Entry(8)), Some(8));
    assert_eq!(cell_of_choice(&image, ColorChoice::Entry(0)), Some(0));
    // `choice_label`: `índice 200 #…` con el color de la entrada 200 (Rgba::opaque(200, 55, 7)).
    assert_eq!(
        choice_label(&image, ColorChoice::Entry(200)).as_deref(),
        Some("índice 200 #c83707")
    );
}

#[test]
fn row_6_rgba_click_on_the_2_and_key_3() {
    let image = image_2(Mode::Rgba);
    assert_eq!(choice_for_cell(&image, 2), Some(ColorChoice::Slot(3)));
    assert_eq!(key_choice(&image, 3), Some(ColorChoice::Slot(3)));
    assert_eq!(
        pixel_for_choice(&image, ColorChoice::Slot(3)),
        Some(Pixel::Rgba(rgba(0xff, 0, 0)))
    );
    assert_eq!(cell_of_choice(&image, ColorChoice::Slot(3)), Some(2));
    assert_eq!(
        choice_label(&image, ColorChoice::Slot(3)).as_deref(),
        Some("color 3 #ff0000")
    );
}

#[test]
fn row_6_gray_click_on_the_8() {
    let image = image_2(Mode::Gray);
    assert_eq!(choice_for_cell(&image, 8), Some(ColorChoice::Slot(9)));
    assert_eq!(key_choice(&image, 9), Some(ColorChoice::Slot(9)));
    assert_eq!(
        pixel_for_choice(&image, ColorChoice::Slot(9)),
        Some(Pixel::Gray(255))
    );
    assert_eq!(cell_of_choice(&image, ColorChoice::Slot(9)), Some(8));
}

#[test]
fn row_6_indexed_of_3_the_9_key_does_not_exist() {
    let image = indexed(3);
    assert_eq!(key_choice(&image, 9), None);
    for n in 4..=9u8 {
        assert_eq!(key_choice(&image, n), None, "tecla {n}");
    }
    for n in 1..=3u8 {
        assert_eq!(
            key_choice(&image, n),
            Some(ColorChoice::Entry(n - 1)),
            "tecla {n}"
        );
    }
    // La selección no cambia: se modela como «si es `None`, se conserva».
    let mut selection = ColorChoice::Entry(1);
    if let Some(new) = key_choice(&image, 9) {
        selection = new;
    }
    assert_eq!(selection, ColorChoice::Entry(1));
}

#[test]
fn row_6_a_click_on_k_and_the_k_key_plus_1_give_the_same_choice_and_the_same_pixel() {
    for (name_2, image) in all_2() {
        for k in 0..9usize {
            let per_click = choice_for_cell(&image, k);
            let per_key = key_choice(&image, (k + 1) as u8);
            assert_eq!(per_click, per_key, "{name_2}, celda {k}");
            if let (Some(click), Some(key_2)) = (per_click, per_key) {
                assert_eq!(
                    pixel_for_choice(&image, click),
                    pixel_for_choice(&image, key_2),
                    "{name_2}, celda {k}"
                );
            }
        }
    }
}

#[test]
fn the_normal_form_is_entry_in_indexed_and_slot_in_the_others() {
    for (name_2, image) in without_palette() {
        for i in 0..9usize {
            assert_eq!(
                choice_for_cell(&image, i),
                Some(ColorChoice::Slot(i as u8 + 1)),
                "{name_2}, celda {i}"
            );
        }
    }
    let image = indexed(20);
    for i in 0..20usize {
        assert_eq!(
            choice_for_cell(&image, i),
            Some(ColorChoice::Entry(i as u8)),
            "celda {i}"
        );
    }
}

// Fila 7 (parte de px-ui): celdas inexistentes.

#[test]
fn row_7_the_nonexistent_cels_do_not_choose() {
    for (name_2, image) in all_2() {
        let how_many = cell_count(&image);
        assert!(choice_for_cell(&image, how_many - 1).is_some(), "{name_2}");
        for cell in [
            how_many,
            how_many + 1,
            how_many + 100,
            256,
            1000,
            usize::MAX,
        ] {
            assert_eq!(
                choice_for_cell(&image, cell),
                None,
                "{name_2}, celda {cell}"
            );
            assert_eq!(cell_color(&image, cell), None, "{name_2}, celda {cell}");
            assert_eq!(cell_label(&image, cell), None, "{name_2}, celda {cell}");
        }
    }
}

#[test]
fn row_7_the_keys_without_a_swatch_are_ignored() {
    // Una sola entrada: las teclas 2 a 9 se ignoran (fila 11).
    let a = indexed(1);
    assert_eq!(key_choice(&a, 1), Some(ColorChoice::Entry(0)));
    for n in 2..=9u8 {
        assert_eq!(key_choice(&a, n), None, "tecla {n}");
    }
    // Paletas de 5 entradas: de la 6 a la 9 no existen.
    let five = indexed(5);
    for n in 6..=9u8 {
        assert_eq!(key_choice(&five, n), None, "tecla {n}");
    }
    // La tecla 0 no es de ninguna muestra.
    for (name_2, image) in all_2() {
        assert_eq!(key_choice(&image, 0), None, "{name_2}, tecla 0");
    }
}

#[test]
fn row_7_cell_of_choice_is_none_if_it_is_not_valid_for_the_image() {
    // Entrada en una imagen sin paleta.
    for (name_2, image) in without_palette() {
        assert_eq!(
            cell_of_choice(&image, ColorChoice::Entry(0)),
            None,
            "{name_2}"
        );
        assert_eq!(
            cell_of_choice(&image, ColorChoice::Entry(200)),
            None,
            "{name_2}"
        );
        // Ranuras fuera de 1 a 9.
        assert_eq!(
            cell_of_choice(&image, ColorChoice::Slot(0)),
            None,
            "{name_2}"
        );
        assert_eq!(
            cell_of_choice(&image, ColorChoice::Slot(10)),
            None,
            "{name_2}"
        );
        // Las nueve ranuras dan su muestra.
        for n in 1..=9u8 {
            assert_eq!(
                cell_of_choice(&image, ColorChoice::Slot(n)),
                Some(usize::from(n) - 1),
                "{name_2}, ranura {n}"
            );
        }
    }
    // Entrada fuera de la paleta.
    let three = indexed(3);
    assert_eq!(cell_of_choice(&three, ColorChoice::Entry(2)), Some(2));
    assert_eq!(cell_of_choice(&three, ColorChoice::Entry(3)), None);
    assert_eq!(cell_of_choice(&three, ColorChoice::Entry(255)), None);
}

// Fila 9: textos.

/// `[#000000, #ff0000, #00ff00 (índice transparent), #0000ff80, …, #ffffff (255)]`.
fn palette_of_row_9() -> Image {
    let mut colors_2 = vec![Rgba::opaque(10, 20, 30); 256];
    colors_2[0] = rgba(0, 0, 0);
    colors_2[1] = rgba(0xff, 0, 0);
    colors_2[2] = rgba(0, 0xff, 0);
    colors_2[3] = Rgba::new(0, 0, 0xff, 0x80);
    colors_2[255] = rgba(0xff, 0xff, 0xff);
    indexed_with(colors_2, Some(2))
}

#[test]
fn row_9_cell_label_indexed() {
    let image = palette_of_row_9();
    assert_eq!(cell_label(&image, 1).as_deref(), Some("índice 1 #ff0000"));
    // Resuelto: la transparente da alfa 0, aunque se guardó opaca.
    assert_eq!(cell_label(&image, 2).as_deref(), Some("índice 2 #00ff0000"));
    assert_eq!(cell_label(&image, 3).as_deref(), Some("índice 3 #0000ff80"));
    assert_eq!(
        cell_label(&image, 255).as_deref(),
        Some("índice 255 #ffffff")
    );
    assert_eq!(cell_label(&image, 0).as_deref(), Some("índice 0 #000000"));
    assert_eq!(cell_label(&image, 256), None);
}

#[test]
fn row_9_cell_label_indexed_with_the_example_of_index_12() {
    let mut colors_2 = vec![rgba(1, 2, 3); 20];
    colors_2[12] = rgba(0xff, 0x88, 0x00);
    let image = indexed_with(colors_2, None);
    assert_eq!(cell_label(&image, 12).as_deref(), Some("índice 12 #ff8800"));
}

#[test]
fn row_9_cell_label_rgba() {
    // Las muestras 3 y 9 de la spec son las ranuras `color 3` y `color 9` (celdas 2 y 8).
    let image = image_2(Mode::Rgba);
    assert_eq!(cell_label(&image, 2).as_deref(), Some("color 3 #ff0000"));
    assert_eq!(cell_label(&image, 8).as_deref(), Some("color 9 #00000000"));
    assert_eq!(cell_label(&image, 0).as_deref(), Some("color 1 #000000"));
    assert_eq!(cell_label(&image, 7).as_deref(), Some("color 8 #00ffff"));
    assert_eq!(cell_label(&image, 9), None);
}

#[test]
fn row_9_cell_label_gray_alpha() {
    let image = image_2(Mode::GrayAlpha);
    assert_eq!(cell_label(&image, 2).as_deref(), Some("color 3 gris 64"));
    assert_eq!(
        cell_label(&image, 8).as_deref(),
        Some("color 9 gris 0 alfa 0")
    );
    assert_eq!(cell_label(&image, 9), None);
}

#[test]
fn row_9_cell_label_gray() {
    let image = image_2(Mode::Gray);
    assert_eq!(cell_label(&image, 3).as_deref(), Some("color 4 gris 96"));
    assert_eq!(cell_label(&image, 9), None);
}

#[test]
fn row_9_info_flags_in_the_eight_combinations() {
    // (hover, activa, transparente) -> texto.
    let cases = [
        (false, false, false, ""),
        (false, true, false, "activa"),
        (false, false, true, "transparente"),
        (false, true, true, "activa · transparente"),
        (true, false, false, "cursor"),
        (true, true, false, "cursor · activa"),
        (true, false, true, "cursor · transparente"),
        (true, true, true, "cursor · activa · transparente"),
    ];
    for (hover, active_2, transparent, expected_2) in cases {
        assert_eq!(
            info_flags_alpha0(hover, active_2, false, transparent, false),
            expected_2,
            "hover {hover}, activa {active_2}, transparente {transparent}"
        );
    }
    // El más largo, de 30 caracteres (fila 12).
    assert_eq!(
        info_flags_alpha0(true, true, false, true, false)
            .chars()
            .count(),
        30,
        "cursor · activa · transparente"
    );
}

#[test]
fn row_9_choice_label() {
    let indexed = palette_of_row_9();
    assert_eq!(
        choice_label(&indexed, ColorChoice::Entry(2)).as_deref(),
        Some("índice 2 #00ff0000 transparente")
    );
    assert_eq!(
        choice_label(&indexed, ColorChoice::Entry(1)).as_deref(),
        Some("índice 1 #ff0000")
    );
    assert_eq!(
        choice_label(&indexed, ColorChoice::Entry(3)).as_deref(),
        Some("índice 3 #0000ff80")
    );
    // Entrada que no existe: no hay etiqueta.
    assert_eq!(choice_label(&indexed, ColorChoice::Slot(0)), None);

    let color = image_2(Mode::Rgba);
    assert_eq!(
        choice_label(&color, ColorChoice::Slot(9)).as_deref(),
        Some("color 9 transparente")
    );
    assert_eq!(
        choice_label(&color, ColorChoice::Slot(3)).as_deref(),
        Some("color 3 #ff0000")
    );
    // Una entrada en una imagen sin paleta no vale.
    assert_eq!(choice_label(&color, ColorChoice::Entry(2)), None);
}

#[test]
fn choice_label_with_a_slot_is_slot_label_as_today() {
    for (name_2, image) in without_palette() {
        for n in 1..=9u8 {
            assert_eq!(
                choice_label(&image, ColorChoice::Slot(n)),
                px_ui::color::slot_label(&image, n),
                "{name_2}, ranura {n}"
            );
        }
    }
    // En una indexada, `Slot` mantiene el texto de hoy (`color 2 índice 1`).
    assert_eq!(
        choice_label(&indexed(3), ColorChoice::Slot(2)).as_deref(),
        Some("color 2 índice 1")
    );
}

#[test]
fn choice_label_exists_exactly_when_there_is_color() {
    for (name_2, image) in all_2() {
        for choice in [
            ColorChoice::Slot(0),
            ColorChoice::Slot(1),
            ColorChoice::Slot(9),
            ColorChoice::Slot(10),
            ColorChoice::Entry(0),
            ColorChoice::Entry(2),
            ColorChoice::Entry(8),
            ColorChoice::Entry(255),
        ] {
            assert_eq!(
                choice_label(&image, choice).is_some(),
                pixel_for_choice(&image, choice).is_some(),
                "{name_2}, {choice:?}"
            );
        }
    }
}

// Fila 11 (parte de selección).

fn indexed_of_256_with_transparent(t: Option<u8>) -> Image {
    indexed_with(
        (0..256usize)
            .map(|i| Rgba::opaque(i as u8, 100, 200))
            .collect(),
        t,
    )
}

#[test]
fn row_11_the_transparent_index_at_0_17_and_255() {
    for t in [0u8, 17, 255] {
        let image = indexed_of_256_with_transparent(Some(t));
        for cell in 0..256usize {
            let is_the_transparent = cell == usize::from(t);
            assert_eq!(
                is_transparent_cell(&image, cell),
                is_the_transparent,
                "t={t}, celda {cell}"
            );
            let color = cell_color(&image, cell).unwrap();
            assert_eq!(color.a == 0, is_the_transparent, "t={t}, celda {cell}");
        }
        // La transparente se ve con alfa 0 (su RGB guardado sigue ahí) y se elige como las otras.
        let saved = Rgba::opaque(t, 100, 200);
        assert_eq!(
            cell_color(&image, usize::from(t)),
            Some(Rgba { a: 0, ..saved })
        );
        assert_eq!(
            pixel_for_choice(&image, ColorChoice::Entry(t)),
            Some(Pixel::Indexed(t))
        );
        assert_eq!(
            choice_for_cell(&image, usize::from(t)),
            Some(ColorChoice::Entry(t))
        );
        assert_eq!(
            cell_of_choice(&image, ColorChoice::Entry(t)),
            Some(usize::from(t))
        );
        // El resto conserva su color opaco.
        let other = usize::from(t ^ 1);
        assert_eq!(
            cell_color(&image, other),
            Some(Rgba::opaque(other as u8, 100, 200))
        );
        // El texto de la entrada transparente dice alfa 0.
        assert_eq!(
            cell_label(&image, usize::from(t)),
            Some(format!("índice {t} #{t:02x}64c800"))
        );
    }
}

#[test]
fn row_11_without_a_transparent_index_no_cell_is_transparent() {
    let image = indexed_of_256_with_transparent(None);
    for cell in 0..256usize {
        assert!(!is_transparent_cell(&image, cell), "celda {cell}");
        assert_eq!(
            cell_color(&image, cell),
            Some(Rgba::opaque(cell as u8, 100, 200)),
            "celda {cell}"
        );
    }
}

#[test]
fn row_11_partial_alpha_colors_are_not_transparent() {
    // Un alfa de 128 guardado no es el índice transparente: solo cuenta `transparent_index`.
    let a = rgba(0x12, 0x34, 0x56);
    let image = indexed_with(vec![a, Rgba { a: 128, ..a }], None);
    assert!(!is_transparent_cell(&image, 0));
    assert!(!is_transparent_cell(&image, 1));
    assert_eq!(cell_color(&image, 0), Some(a));
    assert_eq!(cell_color(&image, 1), Some(Rgba { a: 128, ..a }));
    assert_eq!(cell_label(&image, 1).as_deref(), Some("índice 1 #12345680"));
}

#[test]
fn row_11_duplicate_colors_can_be_chosen_separately() {
    let a = rgba(0x12, 0x34, 0x56);
    for colors_2 in [vec![a, a, a], vec![a, Rgba { a: 128, ..a }]] {
        let how_many_2 = colors_2.len();
        let image = indexed_with(colors_2, None);
        assert_eq!(cell_count(&image), how_many_2);
        let mut choices = Vec::new();
        for i in 0..how_many_2 {
            let choice_2 = choice_for_cell(&image, i).unwrap();
            assert_eq!(choice_2, ColorChoice::Entry(i as u8));
            assert_eq!(
                pixel_for_choice(&image, choice_2),
                Some(Pixel::Indexed(i as u8))
            );
            // `cell_of_choice` devuelve la celda, no la primera con ese color.
            assert_eq!(cell_of_choice(&image, choice_2), Some(i), "celda {i}");
            choices.push(choice_2);
        }
        for (i, a) in choices.iter().enumerate() {
            for b in &choices[i + 1..] {
                assert_ne!(a, b);
            }
        }
    }
    // En concreto, el clic en la 1 da `Entry(1)` y `cell_of_choice` da 1.
    let image = indexed_with(vec![a, a, a], None);
    assert_eq!(choice_for_cell(&image, 1), Some(ColorChoice::Entry(1)));
    assert_eq!(cell_of_choice(&image, ColorChoice::Entry(1)), Some(1));
}

#[test]
fn row_11_the_nine_slots_of_rgba() {
    let image = image_2(Mode::Rgba);
    let expected_2 = [
        Rgba::opaque(0x00, 0x00, 0x00),
        Rgba::opaque(0xff, 0xff, 0xff),
        Rgba::opaque(0xff, 0x00, 0x00),
        Rgba::opaque(0x00, 0xff, 0x00),
        Rgba::opaque(0x00, 0x00, 0xff),
        Rgba::opaque(0xff, 0xff, 0x00),
        Rgba::opaque(0xff, 0x00, 0xff),
        Rgba::opaque(0x00, 0xff, 0xff),
        Rgba::new(0, 0, 0, 0),
    ];
    assert_eq!(cell_count(&image), 9);
    for (i, expected_2) in expected_2.into_iter().enumerate() {
        assert_eq!(cell_color(&image, i), Some(expected_2), "celda {i}");
        assert_eq!(is_transparent_cell(&image, i), i == 8, "celda {i}");
    }
    assert_eq!(cell_color(&image, 9), None);
    assert_eq!(choice_for_cell(&image, 9), None);
}

#[test]
fn row_11_the_nine_slots_of_gray() {
    let image = image_2(Mode::Gray);
    assert_eq!(cell_count(&image), 9);
    for i in 0..8usize {
        let gray_2 = (i as u8) * 32;
        assert_eq!(
            cell_color(&image, i),
            Some(Rgba::opaque(gray_2, gray_2, gray_2)),
            "celda {i}"
        );
        assert!(!is_transparent_cell(&image, i), "celda {i}");
    }
    // La novena es gris 255 opaco: Gray no tiene transparente.
    assert_eq!(cell_color(&image, 8), Some(Rgba::opaque(255, 255, 255)));
    assert!(!is_transparent_cell(&image, 8));
    assert_eq!(cell_color(&image, 9), None);
    assert_eq!(choice_for_cell(&image, 9), None);
}

#[test]
fn row_11_the_nine_slots_of_gray_alpha() {
    let image = image_2(Mode::GrayAlpha);
    assert_eq!(cell_count(&image), 9);
    for i in 0..8usize {
        let gray_2 = (i as u8) * 32;
        assert_eq!(
            cell_color(&image, i),
            Some(Rgba::new(gray_2, gray_2, gray_2, 255)),
            "celda {i}"
        );
        assert!(!is_transparent_cell(&image, i), "celda {i}");
    }
    assert_eq!(cell_color(&image, 8), Some(Rgba::new(0, 0, 0, 0)));
    assert!(is_transparent_cell(&image, 8));
    assert_eq!(cell_color(&image, 9), None);
    assert_eq!(choice_for_cell(&image, 9), None);
}

#[test]
fn row_11_palette_of_one_entry() {
    let image = indexed(1);
    assert_eq!(cell_count(&image), 1);
    assert_eq!(choice_for_cell(&image, 0), Some(ColorChoice::Entry(0)));
    assert_eq!(choice_for_cell(&image, 1), None);
    assert_eq!(cell_color(&image, 0), Some(Rgba::opaque(0, 255, 7)));
    assert_eq!(cell_color(&image, 1), None);
    assert_eq!(cell_of_choice(&image, ColorChoice::Entry(0)), Some(0));
}

#[test]
fn row_11_palette_of_256_entries() {
    let image = indexed(256);
    assert_eq!(cell_count(&image), 256);
    assert_eq!(choice_for_cell(&image, 255), Some(ColorChoice::Entry(255)));
    assert_eq!(choice_for_cell(&image, 256), None);
    assert_eq!(
        pixel_for_choice(&image, ColorChoice::Entry(255)),
        Some(Pixel::Indexed(255))
    );
    assert_eq!(cell_of_choice(&image, ColorChoice::Entry(255)), Some(255));
}

#[test]
fn cell_color_is_what_pixel_rgba_paints_for_each_choice() {
    // El color con el que se ve la celda es el de su `Pixel`.
    for (name_2, image) in all_2() {
        for cell in 0..cell_count(&image) {
            let choice_2 = choice_for_cell(&image, cell).unwrap();
            let pixel = pixel_for_choice(&image, choice_2).unwrap();
            assert_eq!(
                cell_color(&image, cell),
                px_ui::texture::pixel_rgba(&image, pixel),
                "{name_2}, celda {cell}"
            );
        }
    }
}

// Propiedad 4 (parte pura): la selección que se mantiene siempre vale.

#[derive(Clone, Copy, Debug)]
enum TestAction {
    /// Clic en una celda cualquiera. Con `near`, relativa al número de celdas (incluye las que
    /// no existen justo después de la última).
    Click { cel: usize, near: bool },
    /// Tecla 0 a 9.
    TestKey(u8),
}

fn action_2() -> impl Strategy<Value = TestAction> {
    prop_oneof![
        (0usize..300, any::<bool>()).prop_map(|(cel, near)| TestAction::Click { cel, near }),
        (0u8..=9).prop_map(TestAction::TestKey),
    ]
}

fn image_for(mode_2: usize, colors_2: usize) -> Image {
    match mode_2 {
        0 => image_2(Mode::Rgba),
        1 => image_2(Mode::Gray),
        2 => image_2(Mode::GrayAlpha),
        _ => indexed(colors_2),
    }
}

proptest! {
    #[test]
    fn property_4_the_selection_that_is_kept_always_has_a_pixel(
        mode_2 in 0usize..4,
        colors_2 in 1usize..=256,
        actions in vec(action_2(), 0..60),
    ) {
        let image = image_for(mode_2, colors_2);
        let how_many = cell_count(&image);
        let mut selection = initial_choice(&image);
        prop_assert!(pixel_for_choice(&image, selection).is_some());

        for action_2 in actions {
            match action_2 {
                TestAction::Click { cel, near } => {
                    let cel = if near { cel % (how_many + 4) } else { cel };
                    let choice_2 = choice_for_cell(&image, cel);
                    // Solo hay elección en las celdas que existen.
                    prop_assert_eq!(choice_2.is_some(), cel < how_many);
                    if let Some(new) = choice_2 {
                        selection = new;
                        // Un clic en una celda que existe deja `cell_of_choice == Some(cel)`.
                        prop_assert_eq!(cell_of_choice(&image, selection), Some(cel));
                    }
                }
                TestAction::TestKey(n) => {
                    let choice_2 = key_choice(&image, n);
                    prop_assert_eq!(
                        choice_2.is_some(),
                        (1..=9).contains(&n) && usize::from(n) <= how_many.min(9)
                    );
                    if let Some(new) = choice_2 {
                        selection = new;
                        prop_assert_eq!(
                            cell_of_choice(&image, selection),
                            Some(usize::from(n) - 1)
                        );
                        // La tecla y el clic en la celda `n - 1` coinciden.
                        prop_assert_eq!(
                            choice_for_cell(&image, usize::from(n) - 1),
                            Some(selection)
                        );
                    }
                }
            }
            prop_assert!(
                pixel_for_choice(&image, selection).is_some(),
                "la selección {:?} no tiene color en {:?} de {} celdas",
                selection, image.mode(), how_many
            );
            prop_assert!(cell_of_choice(&image, selection).is_some());
            prop_assert!(choice_label(&image, selection).is_some());
        }
    }
}
