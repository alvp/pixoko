//! Tests de la lógica pura de cuentagotas y borrador y de sus teclas (specs/hito-2-cuentagotas-
//! borrador.md, «Casos límite» fila 1 en lo que toca a `px-ui` y fila 2, y propiedad 3).
//!
//! `px_ui::pick` no usa egui: `eraser_pixel`, `eraser_label`, `is_erasable`, `picked_at` y
//! `pick_notice` se prueban con imágenes pequeñas hechas a mano, una por modo. Las filas 3 a 8 y
//! las propiedades 1, 2 y 4 están en `tests/eraser.rs` y `tests/dropper.rs`; la fila 10, en
//! `tests/eraser_canvas.rs`.
//!
//! La fila 14 (regla de la entrada de alfa 0 del valor de borrado en indexado) está al final.
//!
//! La propiedad 3 recorre todo `Key::ALL` por las 32 combinaciones de modificadores: se escribe
//! como `proptest!` (muestreo) y, además, como recorrido exhaustivo, que es más fuerte. Sustituye
//! a la propiedad 5 de la unidad 7 (`tests/tool.rs`), cuyo oráculo no conocía `I` ni `E`.

use egui::{Key, Modifiers};
use proptest::prelude::*;
use proptest::test_runner::TestCaseError;
use px_core::ImageId;
use px_core::{Command, Document, Image, Mode, Palette, Pixel, Rgba};
use px_ui::color::{ColorChoice, SlotColors, choice_label_with};
use px_ui::pick::{
    EraseError, Picked, eraser_label, eraser_pixel, is_erasable, pick_notice, picked_at,
};
use px_ui::shortcuts::{action_for_key, file_action_for_key};
use px_ui::tool::{Tool, ToolAction, status_text, tool_action_for_key, tool_color_label};

const TEST_ALL: [Tool; 5] = [
    Tool::Pencil,
    Tool::Line,
    Tool::Fill,
    Tool::Dropper,
    Tool::Eraser,
];

// Apoyo común.

fn rgba(r: u8, g: u8, b: u8, a: u8) -> Rgba {
    Rgba::new(r, g, b, a)
}

/// Imagen RGBA de 8×8 vacía (todo `(0, 0, 0, 0)`).
fn image_rgba() -> Image {
    Image::new(8, 8, Mode::Rgba).expect("imagen válida")
}

fn image_gray() -> Image {
    Image::new(8, 8, Mode::Gray).expect("imagen válida")
}

/// Imagen de gris con alfa de 8×8 vacía (todo `{ gray: 0, alpha: 0 }`).
fn image_gray_alpha() -> Image {
    Image::new(8, 8, Mode::GrayAlpha).expect("imagen válida")
}

fn palette_of_5() -> Palette {
    Palette::from_colors((1..=5).map(|i| Rgba::opaque(10 * i, 0, 0))).expect("paleta válida")
}

/// Indexada de 5 entradas con el índice 3 transparente; todo el lienzo vale `Indexed(0)`.
fn indexed_with_transparent() -> Image {
    let mut palette = palette_of_5();
    palette.set_transparent_index(Some(3)).expect("existe");
    Image::new_indexed(8, 8, palette).expect("imagen válida")
}

fn indexed_without_transparent() -> Image {
    Image::new_indexed(8, 8, palette_of_5()).expect("imagen válida")
}

/// Indexada de 16 entradas, con la 12 en `#ff8800`.
fn indexed_of_16() -> Image {
    let mut colors_2: Vec<Rgba> = (0..16).map(|i| Rgba::opaque(i * 8, 0, 0)).collect();
    colors_2[12] = Rgba::opaque(255, 136, 0);
    let palette = Palette::from_colors(colors_2).expect("paleta válida");
    Image::new_indexed(8, 8, palette).expect("imagen válida")
}

fn with_pixel(mut image: Image, x: u32, y: u32, pixel: Pixel) -> Image {
    image.set_pixel(x, y, pixel).expect("dentro");
    image
}

/// Las cinco combinaciones de modificadores de la fila 1 más `ctrl` y `command` a la vez (lo que
/// da egui en Linux) y `mac_cmd` y `command` a la vez (macOS).
fn modifiers_of_row_1() -> Vec<Modifiers> {
    vec![
        Modifiers::SHIFT,
        Modifiers::ALT,
        Modifiers::CTRL,
        Modifiers::COMMAND,
        Modifiers::MAC_CMD,
        Modifiers {
            ctrl: true,
            command: true,
            ..Modifiers::NONE
        },
        Modifiers {
            mac_cmd: true,
            command: true,
            ..Modifiers::NONE
        },
    ]
}

// Fila 1: teclas y rótulos.

#[test]
fn row_1_comma_is_dropper_and_e_is_no_longer_eraser_without_modifiers() {
    assert_eq!(
        tool_action_for_key(Key::Comma, Modifiers::NONE),
        Some(ToolAction::Select(Tool::Dropper))
    );
    assert_eq!(tool_action_for_key(Key::E, Modifiers::NONE), None);
}

#[test]
fn row_1_with_any_modifier_comma_does_nothing() {
    let key = Key::Comma;
    for modifiers in modifiers_of_row_1() {
        assert_eq!(
            tool_action_for_key(key, modifiers),
            None,
            "{key:?} con {modifiers:?}"
        );
    }
}

#[test]
fn row_1_d_v_f_and_escape_with_the_new_keys() {
    for (key, action_2) in [
        (Key::D, ToolAction::Select(Tool::Pencil)),
        (Key::V, ToolAction::Select(Tool::Line)),
        (Key::F, ToolAction::Select(Tool::Fill)),
        (Key::Escape, ToolAction::Cancel),
    ] {
        assert_eq!(tool_action_for_key(key, Modifiers::NONE), Some(action_2));
    }
}

#[test]
fn row_1_action_for_key_and_file_action_for_key_give_none_for_comma() {
    let mut combinations = modifiers_of_row_1();
    combinations.push(Modifiers::NONE);
    combinations.push(Modifiers::COMMAND | Modifiers::SHIFT);
    let key = Key::Comma;
    for modifiers in combinations {
        assert_eq!(
            action_for_key(key, modifiers),
            None,
            "action_for_key {key:?} con {modifiers:?}"
        );
        assert_eq!(
            file_action_for_key(key, modifiers),
            None,
            "file_action_for_key {key:?} con {modifiers:?}"
        );
    }
}

#[test]
fn row_1_the_names_of_the_new_tools() {
    assert_eq!(Tool::Dropper.label(), "cuentagotas");
    assert_eq!(Tool::Eraser.label(), "borrador");
}

#[test]
fn row_1_status_text_of_the_eraser_and_the_dropper() {
    assert_eq!(
        status_text(Tool::Eraser, Some("transparente")),
        "borrador · transparente"
    );
    assert_eq!(status_text(Tool::Eraser, None), "borrador");
    assert_eq!(
        status_text(Tool::Dropper, Some("color 3 #ff0000")),
        "cuentagotas · color 3 #ff0000"
    );
    assert_eq!(status_text(Tool::Dropper, None), "cuentagotas");
}

#[test]
fn row_1_pencil_line_fill_and_dropper_give_the_active_color_in_rgba() {
    let image = image_rgba();
    let slots = SlotColors::defaults(Mode::Rgba);
    for tool in [Tool::Pencil, Tool::Line, Tool::Fill, Tool::Dropper] {
        assert_eq!(
            tool_color_label(tool, &image, ColorChoice::Slot(3), &slots),
            Some("color 3 #ff0000".to_string()),
            "{tool:?}"
        );
    }
}

#[test]
fn row_1_the_active_color_of_an_indexed_one_is_the_chosen_entry() {
    let image = indexed_of_16();
    let slots = SlotColors::defaults(Mode::Indexed);
    for tool in [Tool::Pencil, Tool::Line, Tool::Fill, Tool::Dropper] {
        assert_eq!(
            tool_color_label(tool, &image, ColorChoice::Entry(12), &slots),
            Some("índice 12 #ff8800".to_string()),
            "{tool:?}"
        );
    }
}

#[test]
fn row_1_the_active_color_uses_the_slots_it_is_given() {
    let image = image_rgba();
    let mut slots = SlotColors::defaults(Mode::Rgba);
    assert!(slots.set(3, rgba(10, 20, 30, 128)));
    for tool in [Tool::Pencil, Tool::Line, Tool::Fill, Tool::Dropper] {
        let label_2 = tool_color_label(tool, &image, ColorChoice::Slot(3), &slots);
        assert_eq!(
            label_2,
            choice_label_with(&image, ColorChoice::Slot(3), &slots),
            "{tool:?}"
        );
        assert_eq!(label_2, Some("color 3 #0a141e80".to_string()), "{tool:?}");
    }
}

#[test]
fn row_1_with_a_choice_that_is_not_valid_the_other_tools_give_none() {
    // `choice_label_with` da `None` para una ranura 0 y para una entrada en una imagen sin paleta.
    let image = image_rgba();
    let slots = SlotColors::defaults(Mode::Rgba);
    for tool in [Tool::Pencil, Tool::Line, Tool::Fill, Tool::Dropper] {
        for choice in [ColorChoice::Slot(0), ColorChoice::Entry(0)] {
            assert_eq!(
                tool_color_label(tool, &image, choice, &slots),
                choice_label_with(&image, choice, &slots),
                "{tool:?} con {choice:?}"
            );
            assert_eq!(
                tool_color_label(tool, &image, choice, &slots),
                None,
                "{tool:?} con {choice:?}"
            );
        }
    }
}

#[test]
fn row_1_the_eraser_gives_what_it_writes_and_not_the_active_color() {
    let cases = [
        (image_rgba(), "transparente"),
        (image_gray_alpha(), "transparente"),
        (indexed_with_transparent(), "transparente"),
        (indexed_without_transparent(), "sin índice transparente"),
        (image_gray(), "sin transparencia"),
    ];
    for (image, expected_2) in cases {
        let slots = SlotColors::defaults(image.mode());
        for choice in [
            ColorChoice::Slot(3),
            ColorChoice::Slot(9),
            ColorChoice::Entry(1),
        ] {
            assert_eq!(
                tool_color_label(Tool::Eraser, &image, choice, &slots),
                Some(expected_2.to_string()),
                "{:?} con {choice:?}",
                image.mode()
            );
        }
    }
}

#[test]
fn row_1_the_eraser_does_not_depend_on_the_choice_being_valid() {
    // Spec: el borrador da `Some(eraser_label(image))` siempre, valga o no la elección.
    let image = image_rgba();
    let slots = SlotColors::defaults(Mode::Rgba);
    for choice in [ColorChoice::Slot(0), ColorChoice::Entry(200)] {
        assert_eq!(
            tool_color_label(Tool::Eraser, &image, choice, &slots),
            Some("transparente".to_string()),
            "{choice:?}"
        );
    }
}

#[test]
fn row_1_the_complete_eraser_line() {
    let image = image_rgba();
    let slots = SlotColors::defaults(Mode::Rgba);
    let label_2 = tool_color_label(Tool::Eraser, &image, ColorChoice::Slot(3), &slots);
    assert_eq!(
        status_text(Tool::Eraser, label_2.as_deref()),
        "borrador · transparente"
    );
}

// Fila 2: `eraser_pixel` y sus textos.

#[test]
fn row_2_eraser_pixel_in_rgba_and_in_gray_alpha() {
    assert_eq!(
        eraser_pixel(&image_rgba()),
        Ok(Pixel::Rgba(Rgba::TRANSPARENT))
    );
    assert_eq!(
        eraser_pixel(&image_gray_alpha()),
        Ok(Pixel::GrayAlpha { gray: 0, alpha: 0 })
    );
}

#[test]
fn row_2_eraser_pixel_in_indexed_with_transparent_3() {
    assert_eq!(
        eraser_pixel(&indexed_with_transparent()),
        Ok(Pixel::Indexed(3))
    );
}

#[test]
fn row_2_eraser_pixel_in_the_modes_that_cannot_erase() {
    assert_eq!(
        eraser_pixel(&indexed_without_transparent()),
        Err(EraseError::NoTransparentIndex)
    );
    assert_eq!(eraser_pixel(&image_gray()), Err(EraseError::NoTransparency));
}

#[test]
fn row_2_eraser_pixel_follows_the_transparent_index_of_the_palette() {
    for t in 0..5u8 {
        let mut palette = palette_of_5();
        palette.set_transparent_index(Some(t)).expect("existe");
        let image = Image::new_indexed(4, 4, palette).expect("imagen válida");
        assert_eq!(eraser_pixel(&image), Ok(Pixel::Indexed(t)));
    }
}

#[test]
fn row_2_the_error_texts() {
    assert_eq!(
        EraseError::NoTransparentIndex.to_string(),
        "esta imagen no tiene índice transparente"
    );
    assert_eq!(
        EraseError::NoTransparency.to_string(),
        "el modo gris no tiene transparencia"
    );
}

#[test]
fn row_2_the_types_derive_what_the_spec_asks() {
    fn copy<T: Clone + Copy + std::fmt::Debug + PartialEq + Eq>() {}
    copy::<EraseError>();
    copy::<Picked>();
    assert_ne!(EraseError::NoTransparentIndex, EraseError::NoTransparency);
    assert_ne!(Picked::Entry(1), Picked::Entry(2));
    assert_ne!(Picked::Entry(1), Picked::Color(Rgba::TRANSPARENT));
    let _ = format!("{:?} {:?}", EraseError::NoTransparency, Picked::Entry(0));
}

#[test]
fn eraser_label_per_mode() {
    assert_eq!(eraser_label(&image_rgba()), "transparente");
    assert_eq!(eraser_label(&image_gray_alpha()), "transparente");
    assert_eq!(eraser_label(&indexed_with_transparent()), "transparente");
    assert_eq!(
        eraser_label(&indexed_without_transparent()),
        "sin índice transparente"
    );
    assert_eq!(eraser_label(&image_gray()), "sin transparencia");
}

// `is_erasable`.

#[test]
fn is_erasable_in_rgba_skips_alpha_0_even_if_the_hidden_rgb_is_not_0() {
    let image = image_rgba();
    let image = with_pixel(image, 1, 1, Pixel::Rgba(rgba(255, 0, 0, 0)));
    let image = with_pixel(image, 2, 1, Pixel::Rgba(rgba(0, 0, 0, 1)));
    let image = with_pixel(image, 3, 1, Pixel::Rgba(rgba(0, 0, 255, 255)));
    assert!(!is_erasable(&image, 0, 0), "(0, 0, 0, 0)");
    assert!(!is_erasable(&image, 1, 1), "(255, 0, 0, 0)");
    assert!(is_erasable(&image, 2, 1), "alfa 1");
    assert!(is_erasable(&image, 3, 1), "opaco");
}

#[test]
fn is_erasable_in_gray_alpha_looks_only_at_the_alpha() {
    let image = image_gray_alpha();
    let image = with_pixel(image, 1, 0, Pixel::GrayAlpha { gray: 7, alpha: 0 });
    let image = with_pixel(image, 2, 0, Pixel::GrayAlpha { gray: 0, alpha: 1 });
    let image = with_pixel(
        image,
        3,
        0,
        Pixel::GrayAlpha {
            gray: 0,
            alpha: 255,
        },
    );
    assert!(!is_erasable(&image, 0, 0), "{{ 0, 0 }}");
    assert!(!is_erasable(&image, 1, 0), "gris 7 con alfa 0");
    assert!(is_erasable(&image, 2, 0));
    assert!(is_erasable(&image, 3, 0));
}

#[test]
fn is_erasable_in_indexed_with_transparent_skips_only_that_index() {
    let image = indexed_with_transparent();
    let image = with_pixel(image, 1, 0, Pixel::Indexed(3));
    let image = with_pixel(image, 2, 0, Pixel::Indexed(4));
    assert!(is_erasable(&image, 0, 0), "Indexed(0)");
    assert!(!is_erasable(&image, 1, 0), "Indexed(3) es el transparente");
    assert!(is_erasable(&image, 2, 0), "Indexed(4)");
}

#[test]
fn is_erasable_is_false_outside_the_image_in_all_modes() {
    let opaque = |image: Image, pixel| {
        let (w, h) = (image.width(), image.height());
        let mut image = image;
        for y in 0..h {
            for x in 0..w {
                image.set_pixel(x, y, pixel).expect("dentro");
            }
        }
        image
    };
    let images = [
        opaque(image_rgba(), Pixel::Rgba(rgba(1, 2, 3, 255))),
        opaque(
            image_gray_alpha(),
            Pixel::GrayAlpha {
                gray: 9,
                alpha: 255,
            },
        ),
        opaque(indexed_with_transparent(), Pixel::Indexed(0)),
    ];
    for image in &images {
        assert!(is_erasable(image, 7, 7), "{:?}: dentro", image.mode());
        for (x, y) in [(8, 0), (0, 8), (8, 8), (u32::MAX, 0), (0, u32::MAX)] {
            assert!(
                !is_erasable(image, x, y),
                "{:?}: ({x}, {y}) fuera",
                image.mode()
            );
        }
    }
}

// Spec: «el borrador cambiaría ese píxel». En gris y en indexada sin transparente no escribe nada,
// así que no cambia ninguno. La página no lo dice con estas palabras.
#[test]
fn is_erasable_is_false_where_the_eraser_cannot_write() {
    let gray_2 = with_pixel(image_gray(), 1, 1, Pixel::Gray(200));
    let without = with_pixel(indexed_without_transparent(), 1, 1, Pixel::Indexed(2));
    for (x, y) in [(0, 0), (1, 1), (7, 7)] {
        assert!(!is_erasable(&gray_2, x, y), "gris ({x}, {y})");
        assert!(
            !is_erasable(&without, x, y),
            "indexada sin transparente ({x}, {y})"
        );
    }
}

// `picked_at` y `pick_notice`.

#[test]
fn picked_at_in_indexed_gives_the_entry_also_the_transparent_one() {
    let image = indexed_with_transparent();
    let image = with_pixel(image, 1, 0, Pixel::Indexed(3));
    let image = with_pixel(image, 2, 0, Pixel::Indexed(4));
    assert_eq!(picked_at(&image, 0, 0), Some(Picked::Entry(0)));
    assert_eq!(picked_at(&image, 1, 0), Some(Picked::Entry(3)));
    assert_eq!(picked_at(&image, 2, 0), Some(Picked::Entry(4)));
}

#[test]
fn picked_at_in_rgba_gives_the_exact_value_with_its_alpha() {
    let image = image_rgba();
    let image = with_pixel(image, 1, 1, Pixel::Rgba(rgba(10, 20, 30, 128)));
    let image = with_pixel(image, 2, 1, Pixel::Rgba(rgba(10, 20, 30, 0)));
    let image = with_pixel(image, 3, 1, Pixel::Rgba(rgba(10, 20, 30, 255)));
    assert_eq!(
        picked_at(&image, 1, 1),
        Some(Picked::Color(rgba(10, 20, 30, 128)))
    );
    assert_eq!(
        picked_at(&image, 2, 1),
        Some(Picked::Color(rgba(10, 20, 30, 0)))
    );
    assert_eq!(
        picked_at(&image, 3, 1),
        Some(Picked::Color(rgba(10, 20, 30, 255)))
    );
}

#[test]
fn picked_at_in_gray_gives_r_equal_g_equal_b_and_alpha_255() {
    let image = with_pixel(image_gray(), 4, 4, Pixel::Gray(96));
    assert_eq!(
        picked_at(&image, 4, 4),
        Some(Picked::Color(rgba(96, 96, 96, 255)))
    );
    assert_eq!(
        picked_at(&image, 0, 0),
        Some(Picked::Color(rgba(0, 0, 0, 255)))
    );
}

#[test]
fn picked_at_in_gray_alpha_keeps_the_alpha_including_the_0() {
    let image = image_gray_alpha();
    let image = with_pixel(
        image,
        1,
        0,
        Pixel::GrayAlpha {
            gray: 7,
            alpha: 200,
        },
    );
    let image = with_pixel(image, 2, 0, Pixel::GrayAlpha { gray: 7, alpha: 0 });
    assert_eq!(
        picked_at(&image, 1, 0),
        Some(Picked::Color(rgba(7, 7, 7, 200)))
    );
    assert_eq!(
        picked_at(&image, 2, 0),
        Some(Picked::Color(rgba(7, 7, 7, 0)))
    );
}

#[test]
fn picked_at_outside_the_image_is_none_in_all_modes() {
    let images = [
        image_rgba(),
        image_gray(),
        image_gray_alpha(),
        indexed_with_transparent(),
        indexed_without_transparent(),
    ];
    for image in &images {
        for (x, y) in [(8, 0), (0, 8), (8, 8), (u32::MAX, 0), (0, u32::MAX)] {
            assert_eq!(
                picked_at(image, x, y),
                None,
                "{:?}: ({x}, {y})",
                image.mode()
            );
        }
        assert!(picked_at(image, 7, 7).is_some(), "{:?}", image.mode());
    }
}

#[test]
fn pick_notice_only_warns_about_a_color_with_alpha_0() {
    assert_eq!(
        pick_notice(&Picked::Color(rgba(10, 20, 30, 0))),
        Some("píxel transparente copiado tal cual (#0a141e00)".to_string())
    );
    assert_eq!(
        pick_notice(&Picked::Color(rgba(7, 7, 7, 0))),
        Some("píxel transparente copiado tal cual (#07070700)".to_string())
    );
    assert_eq!(pick_notice(&Picked::Color(rgba(10, 20, 30, 128))), None);
    assert_eq!(pick_notice(&Picked::Color(rgba(10, 20, 30, 1))), None);
    assert_eq!(pick_notice(&Picked::Color(rgba(10, 20, 30, 255))), None);
}

#[test]
fn pick_notice_does_not_warn_about_an_entry_or_the_transparent_one() {
    for i in [0, 1, 3, 255] {
        assert_eq!(pick_notice(&Picked::Entry(i)), None, "Entry({i})");
    }
}

// Propiedad 3: teclas.

fn modifiers_2(bits: u8) -> Modifiers {
    Modifiers {
        alt: bits & 1 != 0,
        ctrl: bits & 2 != 0,
        shift: bits & 4 != 0,
        mac_cmd: bits & 8 != 0,
        command: bits & 16 != 0,
    }
}

/// Lo que debe dar `tool_action_for_key`, escrito aparte de la implementación.
fn action_expected(key: Key, modifiers: Modifiers) -> Option<ToolAction> {
    let without_modifiers = !(modifiers.alt
        || modifiers.ctrl
        || modifiers.shift
        || modifiers.mac_cmd
        || modifiers.command);
    if !without_modifiers {
        return None;
    }
    match key {
        Key::D => Some(ToolAction::Select(Tool::Pencil)),
        Key::V => Some(ToolAction::Select(Tool::Line)),
        Key::F => Some(ToolAction::Select(Tool::Fill)),
        Key::Comma => Some(ToolAction::Select(Tool::Dropper)),
        Key::K => Some(ToolAction::Select(Tool::Eraser)),
        Key::S => Some(ToolAction::SelectDotted),
        Key::Q => Some(ToolAction::SelectCurve),
        Key::Escape => Some(ToolAction::Cancel),
        _ => None,
    }
}

fn checks_keys(key: Key, modifiers: Modifiers) -> Result<(), TestCaseError> {
    let action_2 = tool_action_for_key(key, modifiers);
    prop_assert_eq!(
        action_2,
        action_expected(key, modifiers),
        "{:?} con {:?}",
        key,
        modifiers
    );
    if action_2.is_some() {
        prop_assert!(
            matches!(
                key,
                Key::D | Key::V | Key::F | Key::Comma | Key::K | Key::S | Key::Q | Key::Escape
            ),
            "{:?} no debería dar acción",
            key
        );
        prop_assert!(modifiers.is_none(), "{:?} con {:?}", key, modifiers);
    }
    if matches!(key, Key::Comma) {
        prop_assert_eq!(
            action_for_key(key, modifiers),
            None,
            "action_for_key {:?} con {:?}",
            key,
            modifiers
        );
        prop_assert_eq!(
            file_action_for_key(key, modifiers),
            None,
            "file_action_for_key {:?} con {:?}",
            key,
            modifiers
        );
    }
    Ok(())
}

proptest! {
    #[test]
    fn property_3_keys_by_sampling(
        key in prop::sample::select(Key::ALL.to_vec()),
        bits in 0u8..32,
    ) {
        checks_keys(key, modifiers_2(bits))?;
    }
}

#[test]
fn property_3_exhaustive_keys_key_all_by_the_32_combinations() {
    assert!(!Key::ALL.is_empty());
    for &key in Key::ALL {
        for bits in 0..32 {
            let modifiers = modifiers_2(bits);
            if let Err(e) = checks_keys(key, modifiers) {
                panic!("{e}");
            }
        }
    }
}

#[test]
fn property_3_each_tool_has_its_key_without_modifiers() {
    // Las cinco herramientas se alcanzan, cada una con una sola tecla distinta (el Borrador, con `k`).
    let mut reached = Vec::new();
    for &key in Key::ALL {
        if let Some(ToolAction::Select(tool)) = tool_action_for_key(key, Modifiers::NONE) {
            reached.push((tool, key));
        }
    }
    assert_eq!(reached.len(), TEST_ALL.len(), "{reached:?}");
    for tool in TEST_ALL {
        assert_eq!(
            reached.iter().filter(|(t, _)| *t == tool).count(),
            1,
            "{tool:?}"
        );
    }
}

// Fila 14: regla de la entrada de alfa 0.
//
// Paleta de 5: `[blue, red, (0,255,0,α0), blue light, (9,9,9,α0)]`. El valor de borrado en
// indexado es, por orden: `transparent_index` si está fijado; si no, la primera entrada cuyo alfa
// guardado es 0; si no, `Err(NoTransparentIndex)`. Se recalcula siempre, no se guarda nada.

const ENTRY_2_ALPHA_0: Rgba = Rgba::new(0, 255, 0, 0);
const ENTRY_4_ALPHA_0: Rgba = Rgba::new(9, 9, 9, 0);

/// La paleta de la fila 14, sin `transparent_index`: las entradas 2 y 4 con alfa 0.
fn palette_row_14() -> Palette {
    Palette::from_colors([
        Rgba::opaque(0, 0, 255),
        Rgba::opaque(255, 0, 0),
        ENTRY_2_ALPHA_0,
        Rgba::opaque(100, 150, 255),
        ENTRY_4_ALPHA_0,
    ])
    .expect("paleta válida")
}

fn image_row_14(palette: Palette) -> Image {
    Image::new_indexed(8, 8, palette).expect("imagen válida")
}

/// Caso (a): sin `transparent_index`, con las entradas 2 y 4 de alfa 0.
fn case_14_a() -> Image {
    image_row_14(palette_row_14())
}

/// Caso (b): `transparent_index = Some(3)` y las entradas 2 y 4 con alfa 0.
fn case_14_b() -> Image {
    let mut palette = palette_row_14();
    palette.set_transparent_index(Some(3)).expect("existe");
    image_row_14(palette)
}

/// Caso (c): `transparent_index = Some(4)` y la entrada 2 con alfa 0 (la 4, con el alfa que se pida).
fn case_14_c(alpha_of_the_4: u8) -> Image {
    let mut palette = palette_row_14();
    palette
        .set(4, Rgba::new(9, 9, 9, alpha_of_the_4))
        .expect("existe");
    palette.set_transparent_index(Some(4)).expect("existe");
    image_row_14(palette)
}

/// Caso (d): sin `transparent_index` y con solo la entrada 4 de alfa 0 (la 2 es opaca).
fn case_14_d() -> Image {
    let mut palette = palette_row_14();
    palette.set(2, Rgba::opaque(0, 255, 0)).expect("existe");
    image_row_14(palette)
}

#[test]
fn row_14a_without_transparent_index_the_first_alpha_0_entry_wins_not_the_last() {
    let image = case_14_a();
    assert_eq!(image.palette().unwrap().transparent_index(), None);
    assert_eq!(eraser_pixel(&image), Ok(Pixel::Indexed(2)));
    assert_eq!(eraser_label(&image), "transparente (entrada 2)");
}

#[test]
fn row_14a_tool_color_label_and_status_text_of_the_eraser() {
    let image = case_14_a();
    let slots = SlotColors::defaults(Mode::Indexed);
    // El borrador no usa la elección: valga o no, el texto es el mismo.
    for choice in [
        ColorChoice::Entry(0),
        ColorChoice::Entry(3),
        ColorChoice::Slot(9),
    ] {
        let label_2 = tool_color_label(Tool::Eraser, &image, choice, &slots);
        assert_eq!(
            label_2,
            Some("transparente (entrada 2)".to_string()),
            "{choice:?}"
        );
        assert_eq!(
            status_text(Tool::Eraser, label_2.as_deref()),
            "borrador · transparente (entrada 2)",
            "{choice:?}"
        );
    }
}

#[test]
fn row_14a_the_other_tools_keep_giving_the_active_color() {
    // La regla nueva solo toca al borrador.
    let image = case_14_a();
    let slots = SlotColors::defaults(Mode::Indexed);
    for tool in [Tool::Pencil, Tool::Line, Tool::Fill, Tool::Dropper] {
        assert_eq!(
            tool_color_label(tool, &image, ColorChoice::Entry(3), &slots),
            choice_label_with(&image, ColorChoice::Entry(3), &slots),
            "{tool:?}"
        );
    }
}

#[test]
fn row_14b_with_transparent_index_it_wins_even_if_an_earlier_entry_has_alpha_0() {
    let image = case_14_b();
    let palette = image.palette().unwrap();
    assert_eq!(palette.get(2).unwrap().a, 0, "la 2 es anterior y de alfa 0");
    assert_eq!(palette.get(4).unwrap().a, 0);
    assert_eq!(eraser_pixel(&image), Ok(Pixel::Indexed(3)));
    // Sin `(input_2 N)`: solo se usa la regla 2 cuando no hay `transparent_index`.
    assert_eq!(eraser_label(&image), "transparente");
    let slots = SlotColors::defaults(Mode::Indexed);
    assert_eq!(
        tool_color_label(Tool::Eraser, &image, ColorChoice::Entry(0), &slots),
        Some("transparente".to_string())
    );
    assert_eq!(
        status_text(
            Tool::Eraser,
            Some(eraser_label(&image).to_string().as_str())
        ),
        "borrador · transparente"
    );
}

#[test]
fn row_14b_with_transparent_index_on_an_opaque_entry_it_also_wins() {
    // El índice fijado manda aunque su alfa guardado no sea 0 (`resolved` lo pone a 0).
    let mut palette = palette_row_14();
    palette.set_transparent_index(Some(0)).expect("existe");
    let image = image_row_14(palette);
    assert_eq!(eraser_pixel(&image), Ok(Pixel::Indexed(0)));
    assert_eq!(eraser_label(&image), "transparente");
}

#[test]
fn row_14c_with_transparent_index_4_the_4_wins_not_the_2() {
    for alpha_of_the_4 in [0, 255] {
        let image = case_14_c(alpha_of_the_4);
        assert_eq!(image.palette().unwrap().get(2).unwrap().a, 0);
        assert_eq!(
            eraser_pixel(&image),
            Ok(Pixel::Indexed(4)),
            "alfa guardado de la 4 = {alpha_of_the_4}"
        );
        assert_eq!(eraser_label(&image), "transparente");
    }
}

#[test]
fn row_14d_without_transparent_index_and_only_entry_4_with_alpha_0() {
    let image = case_14_d();
    assert_eq!(image.palette().unwrap().transparent_index(), None);
    assert_eq!(eraser_pixel(&image), Ok(Pixel::Indexed(4)));
    assert_eq!(eraser_label(&image), "transparente (entrada 4)");
    let slots = SlotColors::defaults(Mode::Indexed);
    let label_2 = tool_color_label(Tool::Eraser, &image, ColorChoice::Entry(1), &slots);
    assert_eq!(label_2.as_deref(), Some("transparente (entrada 4)"));
    assert_eq!(
        status_text(Tool::Eraser, label_2.as_deref()),
        "borrador · transparente (entrada 4)"
    );
}

/// Cambia el color guardado de una entrada de la imagen principal con un comando.
fn edits_input(doc: &mut Document, index: u8, color: Rgba) {
    let image = ImageId::from_raw(0);
    let command = Command::SetPaletteColor {
        image,
        index,
        color,
    };
    assert_eq!(doc.apply(command), Ok(true));
}

#[test]
fn row_14e_after_editing_the_palette_the_erase_value_is_computed_again() {
    let mut doc = Document::new(case_14_a());
    let id = ImageId::from_raw(0);
    assert_eq!(eraser_pixel(doc.image(id).unwrap()), Ok(Pixel::Indexed(2)));

    // La entrada 2 pasa a alfa 255: queda la 4.
    edits_input(&mut doc, 2, Rgba::opaque(0, 255, 0));
    let image = doc.image(id).unwrap();
    assert_eq!(eraser_pixel(image), Ok(Pixel::Indexed(4)));
    assert_eq!(eraser_label(image), "transparente (entrada 4)");

    // Y si ya no queda ninguna: el aviso de siempre.
    edits_input(&mut doc, 4, Rgba::opaque(9, 9, 9));
    let image = doc.image(id).unwrap();
    assert_eq!(eraser_pixel(image), Err(EraseError::NoTransparentIndex));
    assert_eq!(eraser_label(image), "sin índice transparente");
    let slots = SlotColors::defaults(Mode::Indexed);
    assert_eq!(
        tool_color_label(Tool::Eraser, image, ColorChoice::Entry(0), &slots),
        Some("sin índice transparente".to_string())
    );
    assert_eq!(
        status_text(Tool::Eraser, Some(eraser_label(image).to_string().as_str())),
        "borrador · sin índice transparente"
    );

    // Con undo vuelve la regla 2 con la entrada que se recupera, sin nada guardado.
    assert_eq!(doc.undo(), Ok(()));
    assert_eq!(eraser_pixel(doc.image(id).unwrap()), Ok(Pixel::Indexed(4)));
}

#[test]
fn row_14e_a_single_alpha_0_entry_that_becomes_opaque_leaves_none() {
    let mut doc = Document::new(case_14_d());
    let id = ImageId::from_raw(0);
    assert_eq!(eraser_pixel(doc.image(id).unwrap()), Ok(Pixel::Indexed(4)));
    edits_input(&mut doc, 4, Rgba::opaque(9, 9, 9));
    let image = doc.image(id).unwrap();
    assert_eq!(eraser_pixel(image), Err(EraseError::NoTransparentIndex));
    assert_eq!(eraser_label(image), "sin índice transparente");
}

#[test]
fn row_14e_setting_transparent_index_changes_the_value_in_the_next_computation() {
    let mut doc = Document::new(case_14_a());
    let id = ImageId::from_raw(0);
    assert_eq!(eraser_pixel(doc.image(id).unwrap()), Ok(Pixel::Indexed(2)));
    let command = Command::SetTransparentIndex {
        image: id,
        index: Some(3),
    };
    assert_eq!(doc.apply(command), Ok(true));
    let image = doc.image(id).unwrap();
    assert_eq!(eraser_pixel(image), Ok(Pixel::Indexed(3)));
    assert_eq!(eraser_label(image), "transparente");
}

#[test]
fn row_14_a_palette_with_partial_alpha_does_not_count_as_alpha_0() {
    // Solo el alfa guardado exactamente 0 vale: alfa 1 no.
    let palette = Palette::from_colors([
        Rgba::opaque(0, 0, 255),
        Rgba::new(1, 2, 3, 1),
        Rgba::new(1, 2, 3, 128),
    ])
    .expect("paleta válida");
    let image = Image::new_indexed(4, 4, palette).expect("imagen válida");
    assert_eq!(eraser_pixel(&image), Err(EraseError::NoTransparentIndex));
    assert_eq!(eraser_label(&image), "sin índice transparente");
}

#[test]
fn row_14_the_other_modes_do_not_change_with_the_new_rule() {
    assert_eq!(eraser_label(&image_rgba()), "transparente");
    assert_eq!(eraser_label(&image_gray_alpha()), "transparente");
    assert_eq!(eraser_label(&image_gray()), "sin transparencia");
}

// `is_erasable` con la regla nueva (decisión 2: «ya transparente» es el valor resuelto con alfa 0).

#[test]
fn is_erasable_in_indexed_without_transparent_index_skips_the_alpha_0_entries() {
    let image = case_14_a();
    let image = with_pixel(image, 1, 0, Pixel::Indexed(2));
    let image = with_pixel(image, 2, 0, Pixel::Indexed(4));
    let image = with_pixel(image, 3, 0, Pixel::Indexed(1));
    assert!(is_erasable(&image, 0, 0), "Indexed(0), azul");
    assert!(!is_erasable(&image, 1, 0), "Indexed(2), alfa 0");
    assert!(!is_erasable(&image, 2, 0), "Indexed(4), alfa 0");
    assert!(is_erasable(&image, 3, 0), "Indexed(1), rojo");
}

#[test]
fn is_erasable_with_transparent_index_also_skips_other_alpha_0_entries() {
    // Con `transparent_index = 3`: el índice 3 y las entradas 2 y 4 (alfa 0 guardado) ya son
    // transparentes; no pasan a `Indexed(3)`.
    let image = case_14_b();
    let image = with_pixel(image, 1, 0, Pixel::Indexed(2));
    let image = with_pixel(image, 2, 0, Pixel::Indexed(4));
    let image = with_pixel(image, 3, 0, Pixel::Indexed(3));
    let image = with_pixel(image, 4, 0, Pixel::Indexed(1));
    assert!(is_erasable(&image, 0, 0), "Indexed(0)");
    assert!(!is_erasable(&image, 1, 0), "Indexed(2)");
    assert!(!is_erasable(&image, 2, 0), "Indexed(4)");
    assert!(!is_erasable(&image, 3, 0), "Indexed(3), el transparente");
    assert!(is_erasable(&image, 4, 0), "Indexed(1)");
}

#[test]
fn is_erasable_with_transparent_index_set_to_an_opaque_entry_skips_its_index() {
    // El valor resuelto del índice transparente tiene alfa 0 aunque el guardado no.
    let image = case_14_c(255);
    let image = with_pixel(image, 1, 0, Pixel::Indexed(4));
    let image = with_pixel(image, 2, 0, Pixel::Indexed(2));
    assert!(
        !is_erasable(&image, 1, 0),
        "Indexed(4), resuelto con alfa 0"
    );
    assert!(!is_erasable(&image, 2, 0), "Indexed(2), alfa 0 guardado");
    assert!(is_erasable(&image, 0, 0), "Indexed(0)");
}

#[test]
fn is_erasable_without_any_alpha_0_entry_or_index_is_still_false() {
    let image = case_14_d();
    let mut palette = image.palette().unwrap().clone();
    palette.set(4, Rgba::opaque(9, 9, 9)).expect("existe");
    let image = image_row_14(palette);
    for (x, y) in [(0, 0), (3, 3)] {
        assert!(!is_erasable(&image, x, y), "({x}, {y})");
    }
}
