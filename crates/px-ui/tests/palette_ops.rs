//! Tests de la parte pura de la unidad 11d (specs/hito-2-paleta-operaciones.md): añadir, quitar,
//! mover y rotar entradas de la paleta, sin `egui` ni ventana.
//!
//! Cubre las filas 3 a 8, 10 y 11 en su parte pura, la parte de longitudes de la fila 12 y las
//! propiedades 1 a 3. La fila 8 es solo `clamp_choice` (lo de `Session` es de `px-app`) y la
//! propiedad 4 es de `px-app`.
//!
//! Los números esperados salen de la spec, escritos como literales. Las entradas se numeran
//! desde 0. `n` = 1, 2 y 3 salvo que se diga. Paletas de la spec: `P5` = `[R, G, B, X, Y]` con
//! píxeles `[0, 1, 2, 3, 3]` y `F` = `[R, G, B, K]` con píxeles `[0, 1, 2, 3]`.
//!
//! Los modelos de las propiedades (destino del arrastre con `i128`, máquina del arrastre y
//! vector de identificadores de la propiedad 3) están escritos en el propio test y no llaman a
//! `entry_fate` ni a `choice_after`.

use proptest::collection::vec;
use proptest::prelude::*;
use proptest::test_runner::TestCaseError;
use px_core::{Command, Document, Image, ImageId, Mode, Palette, Pixel, Rgba};
use px_ui::color::{ColorChoice, clamp_choice, info_flags_alpha0};
use px_ui::color_edit::{ColorEditor, EditTarget, StaleReason, stale_notice};
use px_ui::edit_layout::PaletteButton;
use px_ui::palette_grid::{
    CELL_U, COLUMNS, DRAG_START_U, DROP_HALF_GAP_U, DROP_MARK_W_U, GAP_U, GRID_X_U, GRID_Y_U, Grid,
    PITCH_U, RING_DEST_U, RING_RANGE_U, drop_mark_rect, edges_u, max_first_row, total_rows,
};
use px_ui::palette_ops::{
    DragEnd, Fate, MoveDrag, NOTHING_TO_ROTATE_NOTICE, PaletteOp, PaletteOps,
    REMOVAL_CANCELLED_NOTICE, Removal, RemovalWarning, blocked_reason, choice_after,
    default_destination, drag_labels, entry_fate, removal_prompt, removal_warning, rotation_range,
    summary, uses_entry,
};
use px_ui::status::columns;
use px_ui::text::{INTERFACE_CHARSET, fit_text, text_scale};
use px_ui::theme::{
    ACCENT, DROP_MARK, NOTICE_PREFIX, RING_DEST, RING_RANGE, SURFACE_1, TEXT, WARNING,
    contrast_ratio,
};
use px_ui::view::ScreenRect;

const SCALES: [u32; 3] = [1, 2, 3];
const PPPS: [f32; 5] = [1.0, 1.25, 1.5, 2.0, 3.0];

const RED: Rgba = Rgba::opaque(255, 0, 0);
const GREEN: Rgba = Rgba::opaque(0, 255, 0);
const BLUE: Rgba = Rgba::opaque(0, 0, 255);
const TEST_BLACK: Rgba = Rgba::opaque(0, 0, 0);
const X: Rgba = Rgba::opaque(200, 30, 30);
const Y: Rgba = Rgba::opaque(10, 10, 240);

// Apoyo común.

fn rect(x: i64, y: i64, width: i64, height: i64) -> ScreenRect {
    ScreenRect {
        x,
        y,
        width,
        height,
    }
}

fn palette(colors_2: &[Rgba], transparent: Option<u8>) -> Palette {
    let mut palette = Palette::from_colors(colors_2.iter().copied()).expect("paleta válida");
    palette
        .set_transparent_index(transparent)
        .expect("índice transparente válido");
    palette
}

/// Imagen indexada de una fila con esos píxeles.
fn image_with(colors_2: &[Rgba], transparent: Option<u8>, pixels_2: &[u8]) -> Image {
    let width_2 = u32::try_from(pixels_2.len()).expect("pocos píxeles");
    let mut image_2 =
        Image::new_indexed(width_2, 1, palette(colors_2, transparent)).expect("imagen válida");
    for (x, &index_2) in pixels_2.iter().enumerate() {
        image_2
            .set_pixel(x as u32, 0, Pixel::Indexed(index_2))
            .expect("píxel válido");
    }
    image_2
}

/// `P5`: paleta `[R, G, B, X, Y]`, píxeles `[0, 1, 2, 3, 3]`.
fn image_p_5(transparent: Option<u8>) -> Image {
    image_with(&[RED, GREEN, BLUE, X, Y], transparent, &[0, 1, 2, 3, 3])
}

/// `F`: paleta `[R, G, B, K]`, píxeles `[0, 1, 2, 3]`.
fn image_f(transparent: Option<u8>) -> Image {
    image_with(&[RED, GREEN, BLUE, TEST_BLACK], transparent, &[0, 1, 2, 3])
}

fn pixels_2(image_2: &Image) -> Vec<u8> {
    (0..image_2.width())
        .map(|x| match image_2.get_pixel(x, 0).expect("píxel dentro") {
            Pixel::Indexed(index_2) => index_2,
            other_2 => panic!("píxel no indexado: {other_2:?}"),
        })
        .collect()
}

fn image_of(doc: &Document, id: ImageId) -> &Image {
    doc.image(id).expect("la imagen existe")
}

fn colors_of(doc: &Document, id: ImageId) -> Vec<Rgba> {
    image_of(doc, id)
        .palette()
        .expect("indexada")
        .colors()
        .to_vec()
}

fn in_the_set(text_2: &str) -> bool {
    text_2.chars().all(|c| INTERFACE_CHARSET.contains(c))
}

fn characters(text_2: &str) -> usize {
    text_2.chars().count()
}

const BUTTONS: [PaletteButton; 6] = [
    PaletteButton::Add,
    PaletteButton::Remove,
    PaletteButton::Confirm,
    PaletteButton::Cancel,
    PaletteButton::RotateLeft,
    PaletteButton::RotateRight,
];

// Tipos y constantes.

#[test]
fn the_types_are_copy_debug_and_comparable() {
    fn checks<T: Clone + Copy + std::fmt::Debug + PartialEq + Eq>() {}
    checks::<PaletteOp>();
    checks::<Fate>();
    checks::<RemovalWarning>();
    checks::<DragEnd>();
    checks::<Removal>();
    assert_ne!(
        PaletteOp::Move { from: 1, to: 2 },
        PaletteOp::Move { from: 2, to: 1 }
    );
}

#[test]
fn the_grid_constants_are_worth_what_the_spec_says() {
    assert_eq!(DRAG_START_U, 4);
    assert_eq!(DROP_HALF_GAP_U, 2);
    assert_eq!(RING_RANGE_U, 1);
    assert_eq!(RING_DEST_U, 2);
    assert_eq!(DROP_MARK_W_U, 2);
}

#[test]
fn the_grid_constants_are_consistent_with_the_spacing() {
    // La media separación de captura por cada lado cubre toda la separación de 4 u.
    assert_eq!(2 * DROP_HALF_GAP_U, GAP_U);
    // La barra cabe en la separación y no toca las muestras.
    const { assert!(DROP_MARK_W_U <= GAP_U) };
}

#[test]
fn the_new_colors_of_the_theme_are_those_of_the_spec() {
    assert_eq!(RING_RANGE, ACCENT);
    assert_eq!(RING_DEST, WARNING);
    assert_eq!(DROP_MARK, TEXT);
}

#[test]
fn the_new_colors_of_the_theme_have_contrast_of_3_to_1_with_surface_1() {
    for (name_2, color) in [
        ("RING_RANGE", RING_RANGE),
        ("RING_DEST", RING_DEST),
        ("DROP_MARK", DROP_MARK),
    ] {
        let contrast = contrast_ratio(color, SURFACE_1);
        assert!(contrast >= 3.0, "{name_2}: {contrast}");
    }
}

#[test]
fn edges_u_includes_the_edges_of_the_range_ring_of_the_target_and_of_the_bar() {
    let borders = edges_u();
    for step in 0..COLUMNS as i64 {
        for (source_2, name_2) in [(GRID_X_U, "x"), (GRID_Y_U, "y")] {
            let start_2 = source_2 + step * PITCH_U;
            let mut expected_2_2 = vec![
                // Anillo de rango (1 u) y de destino (2 u).
                start_2 - RING_RANGE_U,
                start_2 + CELL_U + RING_RANGE_U,
                start_2 - RING_DEST_U,
                start_2 + CELL_U + RING_DEST_U,
            ];
            if name_2 == "x" {
                // Barra de 2 u: a 3 u a la izquierda o a 11 u a la derecha del origen.
                expected_2_2.extend([start_2 - 3, start_2 - 1, start_2 + 11, start_2 + 13]);
            } else {
                // La barra empieza 2 u por encima de la muestra y mide 14 u.
                expected_2_2.extend([start_2 - 2, start_2 + 12]);
            }
            for border in expected_2_2 {
                assert!(
                    borders.contains(&border),
                    "falta el borde {border} ({name_2}, paso {step})"
                );
            }
        }
    }
}

// Fila 3: Añadir.

#[test]
fn row_3_blocked_reason_of_add_with_256_255_and_1_entries() {
    assert_eq!(
        blocked_reason(PaletteButton::Add, 256, false),
        Some("paleta llena: 256 entradas")
    );
    assert_eq!(
        blocked_reason(PaletteButton::Add, 256, true),
        Some("paleta llena: 256 entradas")
    );
    assert_eq!(blocked_reason(PaletteButton::Add, 255, false), None);
    assert_eq!(blocked_reason(PaletteButton::Add, 1, false), None);
}

#[test]
fn row_3_add_summary() {
    assert_eq!(
        summary(&PaletteOp::Add, 1, 4, false, false),
        "entrada 4 añadida (copia de la 1)"
    );
    assert_eq!(
        summary(&PaletteOp::Add, 3, 5, false, false),
        "entrada 5 añadida (copia de la 3)"
    );
    // La spec de Quitar: «entrada 5 añadida (copia de la 4)».
    assert_eq!(
        summary(&PaletteOp::Add, 4, 5, false, false),
        "entrada 5 añadida (copia de la 4)"
    );
}

#[test]
fn row_3_the_selection_after_adding_is_the_new_one() {
    for (active_2, length_2) in [(0u8, 1usize), (1, 4), (3, 4), (254, 255)] {
        assert_eq!(
            choice_after(&PaletteOp::Add, active_2, length_2),
            length_2 as u8,
            "activa {active_2}, largo {length_2}"
        );
    }
}

#[test]
fn row_3_add_over_f_copies_the_active_one_and_undo_clips_the_selection() {
    let mut doc = Document::new(image_f(None));
    let id = ImageId::from_raw(0);
    let active_2 = 1u8;
    let color = image_of(&doc, id)
        .palette()
        .unwrap()
        .resolved(active_2)
        .unwrap();
    assert!(
        doc.apply(Command::AddPaletteEntry { image: id, color })
            .unwrap()
    );
    let colors_2 = colors_of(&doc, id);
    assert_eq!(colors_2.len(), 5);
    assert_eq!(colors_2[4], GREEN);

    let choice_2 = ColorChoice::Entry(choice_after(&PaletteOp::Add, active_2, 4));
    assert_eq!(choice_2, ColorChoice::Entry(4));

    doc.undo().unwrap();
    assert_eq!(colors_of(&doc, id).len(), 4);
    let after_undo = clamp_choice(image_of(&doc, id), choice_2);
    assert_eq!(after_undo, ColorChoice::Entry(3), "se recorta a la última");

    doc.redo().unwrap();
    assert_eq!(colors_of(&doc, id).len(), 5);
    let after_redo = clamp_choice(image_of(&doc, id), after_undo);
    assert_eq!(
        after_redo,
        ColorChoice::Entry(3),
        "no persigue a la entrada"
    );
}

#[test]
fn row_3_add_the_copy_of_the_transparent_is_alpha_0_and_is_not_the_transparent() {
    let mut doc = Document::new(image_f(Some(3)));
    let id = ImageId::from_raw(0);
    let color = image_of(&doc, id).palette().unwrap().resolved(3).unwrap();
    assert_eq!(color, Rgba::new(0, 0, 0, 0));
    doc.apply(Command::AddPaletteEntry { image: id, color })
        .unwrap();
    let palette = image_of(&doc, id).palette().unwrap();
    assert_eq!(palette.len(), 5);
    assert_eq!(palette.get(4).unwrap(), Rgba::new(0, 0, 0, 0));
    assert_eq!(palette.transparent_index(), Some(3));
}

#[test]
fn row_3_with_255_entries_256_is_reached_and_the_next_one_is_deactivated() {
    assert_eq!(blocked_reason(PaletteButton::Add, 255, true), None);
    // Tras añadir, la paleta tiene 256 y el botón queda desactivado.
    assert_eq!(
        choice_after(&PaletteOp::Add, 10, 255),
        255,
        "la nueva es la 255"
    );
    assert!(blocked_reason(PaletteButton::Add, 256, true).is_some());
}

#[test]
fn row_11_with_one_entry_add_gives_the_1() {
    assert_eq!(blocked_reason(PaletteButton::Add, 1, false), None);
    assert_eq!(choice_after(&PaletteOp::Add, 0, 1), 1);
}

// Filas 3 y 4: avisos de los botones desactivados.

#[test]
fn row_4_blocked_reason_of_remove_and_confirm() {
    assert_eq!(
        blocked_reason(PaletteButton::Remove, 1, false),
        Some("no se puede quitar la única entrada")
    );
    assert_eq!(
        blocked_reason(PaletteButton::Remove, 0, false),
        Some("no se puede quitar la única entrada")
    );
    assert_eq!(blocked_reason(PaletteButton::Remove, 2, false), None);
    assert_eq!(blocked_reason(PaletteButton::Remove, 256, false), None);
    assert_eq!(
        blocked_reason(PaletteButton::Confirm, 5, false),
        Some("elige un destino pulsando una muestra")
    );
    assert_eq!(blocked_reason(PaletteButton::Confirm, 5, true), None);
}

#[test]
fn rotate_and_cancel_are_never_disabled() {
    for length_2 in [0usize, 1, 2, 255, 256] {
        for target_2 in [false, true] {
            for button_2 in [
                PaletteButton::RotateLeft,
                PaletteButton::RotateRight,
                PaletteButton::Cancel,
            ] {
                assert_eq!(
                    blocked_reason(button_2, length_2, target_2),
                    None,
                    "{button_2:?}, largo {length_2}, destino {target_2}"
                );
            }
        }
    }
}

#[test]
fn blocked_reason_depends_only_on_what_the_spec_says() {
    for length_2 in [1usize, 2, 100, 255, 256] {
        for target_2 in [false, true] {
            let add = blocked_reason(PaletteButton::Add, length_2, target_2).is_some();
            assert_eq!(add, length_2 >= 256, "Add, largo {length_2}");
            let remove = blocked_reason(PaletteButton::Remove, length_2, target_2).is_some();
            assert_eq!(remove, length_2 <= 1, "Remove, largo {length_2}");
            let confirm = blocked_reason(PaletteButton::Confirm, length_2, target_2).is_some();
            assert_eq!(confirm, !target_2, "Confirm, destino {target_2}");
        }
    }
}

#[test]
fn the_notice_constants_are_those_of_the_spec() {
    assert_eq!(
        REMOVAL_CANCELLED_NOTICE,
        "quitar cancelado: el documento cambió"
    );
    assert_eq!(NOTHING_TO_ROTATE_NOTICE, "nada que rotar: una sola entrada");
    assert_eq!(
        stale_notice(StaleReason::Moved),
        "texto descartado: la entrada se movió"
    );
}

// Fila 4: Quitar.

#[test]
fn row_4_default_destination_of_p5() {
    let p5 = image_p_5(None);
    let palette = p5.palette().unwrap();
    assert_eq!(default_destination(palette, 3), Some(0), "X → R");
    assert_eq!(default_destination(palette, 4), Some(2), "Y → B");
    assert_eq!(
        default_destination(palette, 0),
        Some(3),
        "R → X, en el resto [G, B, X, Y] gana X (índice 3 antes de quitar)"
    );
}

#[test]
fn row_4_default_destination_of_rgb_removing_the_1_the_lowest_index_wins_the_tie() {
    let rgb = palette(&[RED, GREEN, BLUE], None);
    assert_eq!(default_destination(&rgb, 1), Some(0));
}

#[test]
fn row_4_uses_entry_is_false_only_for_the_4_in_p5() {
    let p5 = image_p_5(None);
    for (index_2, expected_2) in [(0u8, true), (1, true), (2, true), (3, true), (4, false)] {
        assert_eq!(uses_entry(&p5, index_2), expected_2, "entrada {index_2}");
    }
    assert!(!uses_entry(&p5, 200), "una entrada que no existe no se usa");
}

#[test]
fn row_4_uses_entry_finds_a_pixel_at_any_position() {
    let palette_of_3 = palette(&[RED, GREEN, BLUE], None);
    let mut image_2 = Image::new_indexed(7, 5, palette_of_3).unwrap();
    assert!(uses_entry(&image_2, 0), "todos los píxeles empiezan en 0");
    assert!(!uses_entry(&image_2, 2));
    image_2.set_pixel(6, 4, Pixel::Indexed(2)).unwrap();
    assert!(uses_entry(&image_2, 2), "último píxel");
    image_2.set_pixel(6, 4, Pixel::Indexed(0)).unwrap();
    image_2.set_pixel(0, 0, Pixel::Indexed(1)).unwrap();
    assert!(uses_entry(&image_2, 1), "primer píxel");
    assert!(uses_entry(&image_2, 0));
    image_2.set_pixel(0, 0, Pixel::Indexed(0)).unwrap();
    assert!(!uses_entry(&image_2, 1));
}

#[test]
fn row_4_removing_the_3_of_p5_moves_the_pixels_to_the_2_and_an_undo_restores_it() {
    let mut doc = Document::new(image_p_5(None));
    let id = ImageId::from_raw(0);
    let initial = image_of(&doc, id).clone();
    let (index_2, target_2) = (3u8, 2u8);
    let op = PaletteOp::Remove {
        index: index_2,
        into: target_2,
    };

    assert!(
        doc.apply(Command::RemovePaletteEntry {
            image: id,
            index: index_2,
            into: target_2
        })
        .unwrap()
    );
    assert_eq!(colors_of(&doc, id), vec![RED, GREEN, BLUE, Y]);
    assert_eq!(pixels_2(image_of(&doc, id)), vec![0, 1, 2, 2, 2]);
    assert_eq!(choice_after(&op, 3, 5), 3, "la selección es la 3 (Y)");
    assert_eq!(
        summary(&op, 3, 5, false, false),
        "entrada 3 quitada; píxeles pasan a la 2"
    );

    doc.undo().unwrap();
    assert_eq!(image_of(&doc, id), &initial);
    assert!(!doc.can_undo(), "un solo paso");
}

#[test]
fn row_4_removing_the_4_without_pixels_is_applied_with_the_default_target() {
    let mut doc = Document::new(image_p_5(None));
    let id = ImageId::from_raw(0);
    let target_2 = default_destination(image_of(&doc, id).palette().unwrap(), 4).unwrap();
    assert_eq!(target_2, 2);
    let op = PaletteOp::Remove {
        index: 4,
        into: target_2,
    };
    doc.apply(Command::RemovePaletteEntry {
        image: id,
        index: 4,
        into: target_2,
    })
    .unwrap();
    assert_eq!(colors_of(&doc, id), vec![RED, GREEN, BLUE, X]);
    assert_eq!(choice_after(&op, 4, 5), 3, "se recorta a la última");
    assert_eq!(
        summary(&op, 4, 5, false, true),
        "entrada 4 quitada (sin píxeles)",
        "con `unused` verdadero"
    );
}

#[test]
fn row_4_remove_summary_with_pixels_uses_the_later_numbering() {
    // `into` por debajo de `index`: no cambia.
    assert_eq!(
        summary(&PaletteOp::Remove { index: 3, into: 0 }, 3, 5, false, false),
        "entrada 3 quitada; píxeles pasan a la 0"
    );
    assert_eq!(
        summary(&PaletteOp::Remove { index: 1, into: 0 }, 1, 5, false, false),
        "entrada 1 quitada; píxeles pasan a la 0"
    );
    // `into` por encima de `index`: baja un lugar.
    assert_eq!(
        summary(&PaletteOp::Remove { index: 3, into: 4 }, 3, 5, false, false),
        "entrada 3 quitada; píxeles pasan a la 3"
    );
    assert_eq!(
        summary(&PaletteOp::Remove { index: 0, into: 1 }, 0, 5, false, false),
        "entrada 0 quitada; píxeles pasan a la 0"
    );
    assert_eq!(
        summary(&PaletteOp::Remove { index: 0, into: 4 }, 0, 5, false, false),
        "entrada 0 quitada; píxeles pasan a la 3"
    );
}

#[test]
fn row_4_remove_summary_without_pixels() {
    assert_eq!(
        summary(&PaletteOp::Remove { index: 4, into: 2 }, 4, 5, false, true),
        "entrada 4 quitada (sin píxeles)"
    );
    assert_eq!(
        summary(&PaletteOp::Remove { index: 0, into: 1 }, 0, 5, false, true),
        "entrada 0 quitada (sin píxeles)"
    );
}

#[test]
fn row_4_the_editor_lines_with_a_pending_removal() {
    assert_eq!(
        removal_prompt(3, Some(0), None),
        [
            "quitar 3; destino 0".to_string(),
            "elige otra muestra o confirma".to_string()
        ]
    );
    assert_eq!(
        removal_prompt(3, Some(2), None),
        [
            "quitar 3; destino 2".to_string(),
            "elige otra muestra o confirma".to_string()
        ]
    );
}

// Fila 5: Quitar con la transparente, la única y los duplicados.

#[test]
fn row_5_the_transparent_has_no_default_target() {
    let p5 = image_p_5(Some(3));
    assert_eq!(default_destination(p5.palette().unwrap(), 3), None);
    // En 0 y en la última también.
    let at_0 = palette(&[RED, GREEN, BLUE], Some(0));
    assert_eq!(default_destination(&at_0, 0), None);
    let in_the_last = palette(&[RED, GREEN, BLUE], Some(2));
    assert_eq!(default_destination(&in_the_last, 2), None);
}

#[test]
fn row_5_a_palette_of_fewer_than_two_entries_has_no_target() {
    assert_eq!(default_destination(&palette(&[RED], None), 0), None);
    assert_eq!(default_destination(&Palette::new(), 0), None);
}

#[test]
fn row_5_removing_the_transparent_warns_that_its_pixels_stay_opaque() {
    let p5 = image_p_5(Some(3));
    let palette = p5.palette().unwrap();
    for target_2 in [None, Some(0), Some(4)] {
        assert_eq!(
            removal_warning(palette, 3, target_2),
            Some(RemovalWarning::PixelsOpaque),
            "destino {target_2:?}"
        );
    }
}

#[test]
fn row_5_with_the_transparent_as_target_the_notice_is_transparent_target() {
    let p5 = image_p_5(Some(3));
    let palette = p5.palette().unwrap();
    assert_eq!(
        removal_warning(palette, 0, Some(3)),
        Some(RemovalWarning::DestinationTransparent)
    );
    assert_eq!(removal_warning(palette, 0, Some(4)), None);
    assert_eq!(
        removal_warning(palette, 0, None),
        None,
        "sin destino y sin ser la transparente no hay aviso"
    );
}

#[test]
fn row_5_without_a_transparent_there_is_never_a_notice() {
    let p5 = image_p_5(None);
    let palette = p5.palette().unwrap();
    for index_2 in 0..5u8 {
        for target_2 in [None, Some(0), Some(1), Some(4)] {
            assert_eq!(removal_warning(palette, index_2, target_2), None);
        }
    }
}

#[test]
fn row_5_the_editor_lines_with_notices() {
    assert_eq!(
        removal_prompt(3, None, Some(RemovalWarning::PixelsOpaque)),
        [
            "quitar 3: pulsa un destino".to_string(),
            "aviso: sus píxeles quedan opacos".to_string()
        ]
    );
    // Al pulsar la muestra 4 (con el aviso de que sus píxeles quedan opacos).
    let [line_1, line_2] = removal_prompt(3, Some(4), Some(RemovalWarning::PixelsOpaque));
    assert_eq!(line_1, "quitar 3; destino 4");
    assert_eq!(line_2, "aviso: sus píxeles quedan opacos");
    assert_eq!(
        removal_prompt(0, Some(3), Some(RemovalWarning::DestinationTransparent)),
        [
            "quitar 0; destino 3".to_string(),
            "aviso: destino transparente".to_string()
        ]
    );
}

#[test]
fn row_5_remove_the_first_line_without_a_target_says_press_a_target() {
    for index_2 in [0u8, 3, 255] {
        let [line_1, _] = removal_prompt(index_2, None, Some(RemovalWarning::PixelsOpaque));
        assert_eq!(line_1, format!("quitar {index_2}: pulsa un destino"));
    }
}

#[test]
fn row_5_removing_the_transparent_with_the_4_as_target_leaves_the_and_opaque() {
    let mut doc = Document::new(image_p_5(Some(3)));
    let id = ImageId::from_raw(0);
    let initial = image_of(&doc, id).clone();
    assert!(
        doc.apply(Command::RemovePaletteEntry {
            image: id,
            index: 3,
            into: 4
        })
        .unwrap()
    );
    let image_2 = image_of(&doc, id);
    let palette = image_2.palette().unwrap();
    assert_eq!(palette.transparent_index(), None);
    assert_eq!(palette.colors(), &[RED, GREEN, BLUE, Y]);
    assert_eq!(pixels_2(image_2), vec![0, 1, 2, 3, 3], "ahora Y opaco");
    doc.undo().unwrap();
    assert_eq!(image_of(&doc, id), &initial);
}

#[test]
fn row_5_removing_the_transparent_without_pixels_says_it_was_the_transparent() {
    let op = PaletteOp::Remove { index: 5, into: 0 };
    assert_eq!(
        summary(&op, 5, 6, true, true),
        "entrada 5 quitada; era la transparente"
    );
    // Con píxeles que la usan, el resumen es el normal.
    assert_eq!(
        summary(&op, 5, 6, true, false),
        "entrada 5 quitada; píxeles pasan a la 0"
    );
    // Sin ser la transparente, `unused` da el de sin píxeles.
    assert_eq!(
        summary(&op, 5, 6, false, true),
        "entrada 5 quitada (sin píxeles)"
    );
}

#[test]
fn row_5_with_duplicates_the_target_is_the_first_and_without_a_notice() {
    let aab = palette(&[RED, RED, BLUE], None);
    assert_eq!(default_destination(&aab, 1), Some(0), "distancia 0");
    assert_eq!(default_destination(&aab, 0), Some(1));
    assert_eq!(removal_warning(&aab, 1, Some(0)), None);
    assert_eq!(removal_warning(&aab, 0, Some(1)), None);
}

#[test]
fn row_5_with_three_duplicates_the_target_of_any_is_the_first_of_the_others() {
    let aaa = palette(&[RED, RED, RED, BLUE], None);
    assert_eq!(default_destination(&aaa, 2), Some(0));
    assert_eq!(default_destination(&aaa, 1), Some(0));
    assert_eq!(default_destination(&aaa, 0), Some(1));
}

#[test]
fn row_5_with_256_entries_removing_the_255_leaves_the_254() {
    let op = PaletteOp::Remove {
        index: 255,
        into: 0,
    };
    assert_eq!(choice_after(&op, 255, 256), 254);
    assert_eq!(blocked_reason(PaletteButton::Remove, 256, false), None);
}

#[test]
fn row_5_the_transparent_is_readjusted_in_the_rest_when_looking_for_the_target() {
    // [D, T (transparente, guardada blanca), W]: quitando la 0 el resto es [T, W] y la
    // transparente es la primera del resto. Gana T (índice 1 antes de quitar). Si no se
    // reajustara, la transparente sería W (índice 2).
    let dark = Rgba::opaque(5, 5, 5);
    let white = Rgba::opaque(255, 255, 255);
    let after = palette(&[dark, white, white], Some(1));
    assert_eq!(default_destination(&after, 0), Some(1));

    // La transparente antes de la quitada: [T, D, W], quitando la 1.
    let before = palette(&[white, dark, white], Some(0));
    assert_eq!(default_destination(&before, 1), Some(0));
}

#[test]
fn row_5_the_default_target_can_be_a_transparent_one_and_then_there_is_a_notice() {
    // [D, W, T]: quitando D (50, 0, 0), la transparente (distancia 67525·255²) queda más cerca
    // que W (172075·255²).
    let dark = Rgba::opaque(50, 0, 0);
    let white = Rgba::opaque(255, 255, 255);
    let with_transparent = palette(&[dark, white, white], Some(2));
    let target_2 = default_destination(&with_transparent, 0);
    assert_eq!(target_2, Some(2));
    assert_eq!(
        removal_warning(&with_transparent, 0, target_2),
        Some(RemovalWarning::DestinationTransparent)
    );
}

// Fila 6: Rotar.

#[test]
fn row_6_rotation_range_of_the_spec() {
    assert_eq!(rotation_range(4, 1, Some(3)), (1, 3));
    assert_eq!(rotation_range(4, 3, Some(1)), (1, 3));
    assert_eq!(rotation_range(256, 0, None), (0, 256));
    assert_eq!(rotation_range(4, 2, Some(2)), (2, 1));
}

#[test]
fn row_6_without_a_range_the_whole_palette_is_rotated() {
    for length_2 in [1usize, 2, 5, 255, 256] {
        assert_eq!(rotation_range(length_2, 0, None), (0, length_2 as u16));
        assert_eq!(
            rotation_range(length_2, (length_2 - 1) as u8, None),
            (0, length_2 as u16),
            "la activa no importa sin rango"
        );
    }
}

#[test]
fn row_11_a_range_of_the_whole_palette_is_the_same_as_without_a_range() {
    assert_eq!(rotation_range(4, 0, Some(3)), rotation_range(4, 0, None));
    assert_eq!(rotation_range(4, 3, Some(0)), rotation_range(4, 0, None));
    assert_eq!(rotation_range(256, 0, Some(255)), (0, 256));
}

#[test]
fn row_11_a_one_entry_range_or_a_one_entry_palette_gives_length_1() {
    assert_eq!(rotation_range(1, 0, None), (0, 1));
    assert_eq!(rotation_range(4, 2, Some(2)), (2, 1));
    assert_eq!(rotation_range(256, 255, Some(255)), (255, 1));
}

proptest! {
    #[test]
    fn rotation_range_is_the_minimum_and_the_length_of_the_span(
        length_2 in 1usize..=256,
        active_raw in any::<u8>(),
        other_raw in proptest::option::of(any::<u8>()),
    ) {
        let active_2 = (usize::from(active_raw) % length_2) as u8;
        let other_2 = other_raw.map(|raw_2| (usize::from(raw_2) % length_2) as u8);
        let (start_2, length) = rotation_range(length_2, active_2, other_2);
        match other_2 {
            None => prop_assert_eq!((start_2, length), (0, length_2 as u16)),
            Some(other_2) => {
                prop_assert_eq!(start_2, active_2.min(other_2));
                prop_assert_eq!(length, u16::from(active_2.max(other_2) - active_2.min(other_2)) + 1);
            }
        }
        prop_assert!(length >= 1);
        prop_assert!(usize::from(start_2) + usize::from(length) <= length_2);
    }
}

#[test]
fn row_6_the_sign_of_rotate_colors_right_is_plus_1_over_f() {
    let mut doc = Document::new(image_f(None));
    let id = ImageId::from_raw(0);
    let initial = image_of(&doc, id).clone();
    let [a, b, c, d] = [RED, GREEN, BLUE, TEST_BLACK];

    assert!(
        doc.apply(Command::RotateColors {
            image: id,
            start: 0,
            len: 4,
            steps: 1
        })
        .unwrap()
    );
    assert_eq!(colors_of(&doc, id), vec![d, a, b, c], "+1: [D, A, B, C]");
    assert_eq!(
        pixels_2(image_of(&doc, id)),
        vec![0, 1, 2, 3],
        "los píxeles no cambian"
    );
    doc.undo().unwrap();
    assert_eq!(image_of(&doc, id), &initial);

    assert!(
        doc.apply(Command::RotateColors {
            image: id,
            start: 0,
            len: 4,
            steps: -1
        })
        .unwrap()
    );
    assert_eq!(colors_of(&doc, id), vec![b, c, d, a], "−1: [B, C, D, A]");
}

#[test]
fn row_6_rotating_without_a_range_with_the_active_2_gives_the_same_as_the_direct_command() {
    for (steps_2, expected_2) in [
        (1, vec![TEST_BLACK, RED, GREEN, BLUE]),
        (-1, vec![GREEN, BLUE, TEST_BLACK, RED]),
    ] {
        let mut doc = Document::new(image_f(None));
        let id = ImageId::from_raw(0);
        let (start_2, length) = rotation_range(4, 2, None);
        doc.apply(Command::RotateColors {
            image: id,
            start: start_2,
            len: length,
            steps: steps_2,
        })
        .unwrap();
        assert_eq!(colors_of(&doc, id), expected_2, "steps {steps_2}");
        // La selección se queda en la 2.
        let op = PaletteOp::Rotate {
            start: start_2,
            len: length,
            steps: steps_2,
        };
        assert_eq!(choice_after(&op, 2, 4), 2);
    }
}

#[test]
fn row_6_with_a_range_from_1_to_3_from_the_1_and_the_active_one_1() {
    // `Mayús`+clic en la 3 desde `Entry(1)`: rango (1, 3).
    let (start_2, length) = rotation_range(4, 1, Some(3));
    for (steps_2, expected_2) in [
        (1, vec![RED, TEST_BLACK, GREEN, BLUE]),
        (-1, vec![RED, BLUE, TEST_BLACK, GREEN]),
    ] {
        let mut doc = Document::new(image_f(None));
        let id = ImageId::from_raw(0);
        doc.apply(Command::RotateColors {
            image: id,
            start: start_2,
            len: length,
            steps: steps_2,
        })
        .unwrap();
        assert_eq!(colors_of(&doc, id), expected_2, "steps {steps_2}");
        assert_eq!(pixels_2(image_of(&doc, id)), vec![0, 1, 2, 3]);
    }
}

#[test]
fn row_6_with_the_transparent_inside_the_range_it_stays_in_its_slot() {
    let mut doc = Document::new(image_f(Some(3)));
    let id = ImageId::from_raw(0);
    doc.apply(Command::RotateColors {
        image: id,
        start: 0,
        len: 4,
        steps: 1,
    })
    .unwrap();
    let palette = image_of(&doc, id).palette().unwrap();
    assert_eq!(palette.transparent_index(), Some(3));
    assert_eq!(palette.colors(), &[TEST_BLACK, RED, GREEN, BLUE]);
}

#[test]
fn row_6_rotate_summaries() {
    let range = PaletteOp::Rotate {
        start: 1,
        len: 3,
        steps: 1,
    };
    assert_eq!(
        summary(&range, 1, 4, false, false),
        "rotadas entradas 1 a 3 a la derecha"
    );
    let range_left = PaletteOp::Rotate {
        start: 1,
        len: 3,
        steps: -1,
    };
    assert_eq!(
        summary(&range_left, 1, 4, false, false),
        "rotadas entradas 1 a 3 a la izquierda"
    );
    let all = PaletteOp::Rotate {
        start: 0,
        len: 4,
        steps: 1,
    };
    assert_eq!(
        summary(&all, 2, 4, false, false),
        "paleta rotada a la derecha"
    );
    let all_left = PaletteOp::Rotate {
        start: 0,
        len: 4,
        steps: -1,
    };
    assert_eq!(
        summary(&all_left, 2, 4, false, false),
        "paleta rotada a la izquierda"
    );
}

#[test]
fn row_6_a_span_that_starts_at_0_but_does_not_reach_the_end_is_not_the_whole_palette() {
    let op = PaletteOp::Rotate {
        start: 0,
        len: 3,
        steps: -1,
    };
    assert_eq!(
        summary(&op, 0, 4, false, false),
        "rotadas entradas 0 a 2 a la izquierda"
    );
    let op = PaletteOp::Rotate {
        start: 1,
        len: 3,
        steps: 1,
    };
    assert_eq!(
        summary(&op, 0, 4, false, false),
        "rotadas entradas 1 a 3 a la derecha",
        "empieza en 1: no es toda"
    );
    let all = PaletteOp::Rotate {
        start: 0,
        len: 256,
        steps: 1,
    };
    assert_eq!(
        summary(&all, 0, 256, false, false),
        "paleta rotada a la derecha"
    );
}

#[test]
fn row_6_the_sign_of_the_summary_comes_from_steps() {
    for (steps_2, side) in [
        (1, "derecha"),
        (5, "derecha"),
        (-1, "izquierda"),
        (-7, "izquierda"),
    ] {
        let op = PaletteOp::Rotate {
            start: 0,
            len: 4,
            steps: steps_2,
        };
        assert_eq!(
            summary(&op, 0, 4, false, false),
            format!("paleta rotada a la {side}"),
            "steps {steps_2}"
        );
    }
}

#[test]
fn row_6_the_selection_after_rotating_is_the_same() {
    for active_2 in [0u8, 2, 3] {
        let op = PaletteOp::Rotate {
            start: 1,
            len: 3,
            steps: 1,
        };
        assert_eq!(choice_after(&op, active_2, 4), active_2);
    }
}

#[test]
fn row_13_rotating_a_one_entry_range_gives_ok_false_without_a_step() {
    let mut doc = Document::new(image_f(None));
    let id = ImageId::from_raw(0);
    let state_2 = doc.state_id();
    for steps_2 in [1, -1] {
        let (start_2, length) = rotation_range(4, 2, Some(2));
        assert_eq!(length, 1);
        assert!(
            !doc.apply(Command::RotateColors {
                image: id,
                start: start_2,
                len: length,
                steps: steps_2
            })
            .unwrap()
        );
    }
    // Una paleta de una entrada, igual.
    let mut a = Document::new(image_with(&[RED], None, &[0, 0]));
    let id_a = ImageId::from_raw(0);
    let state_a = a.state_id();
    let (start_2, length) = rotation_range(1, 0, None);
    assert!(
        !a.apply(Command::RotateColors {
            image: id_a,
            start: start_2,
            len: length,
            steps: 1
        })
        .unwrap()
    );
    assert_eq!(a.state_id(), state_a);
    assert!(!a.can_undo());
    assert_eq!(doc.state_id(), state_2);
    assert!(!doc.can_undo());
    assert!(!doc.can_redo());
}

// Fila 7: Mover.

#[test]
fn row_7_drop_index_with_256_entries_and_16_rows() {
    let cases: [((i64, i64), Option<usize>); 10] = [
        ((-3, 0), None),
        ((-2, 0), Some(0)),
        ((0, 0), Some(0)),
        ((11, 0), Some(0)),
        ((12, 0), Some(1)),
        ((221, 0), Some(15)),
        ((222, 0), None),
        ((0, 12), Some(16)),
        ((0, 221), Some(240)),
        ((0, 222), None),
    ];
    for n in SCALES {
        let k = i64::from(n);
        let grid = Grid::new(n, 256, 16, 0);
        for ((x, y), expected_2) in cases {
            assert_eq!(
                grid.drop_index(x * k, y * k),
                expected_2,
                "n={n}, punto ({x}, {y})"
            );
        }
    }
}

#[test]
fn row_7_drop_index_with_9_entries_the_last_row_counts_as_the_end() {
    for n in SCALES {
        let k = i64::from(n);
        let grid = Grid::new(n, 9, 16, 0);
        assert_eq!(grid.drop_index(126 * k, 0), Some(8), "n={n}");
        assert_eq!(grid.drop_index(0, 14 * k), None, "n={n}: no hay fila 1");
        // Las muestras que existen se encuentran por su esquina.
        for i in 0..9i64 {
            assert_eq!(
                grid.drop_index(i * 14 * k, 0),
                Some(i as usize),
                "n={n}, i={i}"
            );
        }
    }
}

#[test]
fn row_7_drop_index_with_first_row_3_and_5_visible_rows() {
    for n in SCALES {
        let k = i64::from(n);
        let grid = Grid::new(n, 256, 5, 3);
        assert_eq!(grid.drop_index(0, 0), Some(48), "n={n}");
        assert_eq!(grid.drop_index(0, 67 * k), Some(112), "n={n}");
        assert_eq!(grid.drop_index(0, 68 * k), None, "n={n}");
    }
}

#[test]
fn row_7_drop_index_with_20_entries_the_extra_column_is_the_last() {
    for n in SCALES {
        let k = i64::from(n);
        let grid = Grid::new(n, 20, 16, 0);
        assert_eq!(grid.drop_index(42 * k, 14 * k), Some(19), "n={n}");
        assert_eq!(grid.drop_index(56 * k, 14 * k), Some(19), "n={n}");
        assert_eq!(grid.drop_index(210 * k, 14 * k), Some(19), "n={n}");
        assert_eq!(grid.drop_index(0, 14 * k), Some(16), "n={n}");
    }
}

#[test]
fn row_7_drop_index_without_swatches_or_without_visible_rows_is_none() {
    for n in SCALES {
        let k = i64::from(n);
        for (x, y) in [(0, 0), (-2 * k, -2 * k), (5 * k, 5 * k)] {
            assert_eq!(
                Grid::new(n, 0, 16, 0).drop_index(x, y),
                None,
                "sin muestras"
            );
            assert_eq!(Grid::new(n, 256, 0, 0).drop_index(x, y), None, "sin filas");
        }
    }
}

#[test]
fn row_7_drop_index_of_a_single_entry_always_lands_on_it() {
    for n in SCALES {
        let k = i64::from(n);
        let grid = Grid::new(n, 1, 16, 0);
        for x in [-2 * k, 0, 11 * k, 12 * k, 100 * k, 221 * k] {
            assert_eq!(grid.drop_index(x, 0), Some(0), "n={n}, x={x}");
        }
        assert_eq!(grid.drop_index(222 * k, 0), None);
    }
}

#[test]
fn row_7_drop_index_of_the_swatch_found_by_index_at_is_the_same() {
    for n in SCALES {
        let grid = Grid::new(n, 100, 8, 2);
        for i in 0..100usize {
            if let Some(cel) = grid.cell_rect(i) {
                for (x, y) in [
                    (cel.x, cel.y),
                    (cel.x + cel.width - 1, cel.y + cel.height - 1),
                ] {
                    assert_eq!(grid.drop_index(x, y), Some(i), "n={n}, i={i}");
                }
            }
        }
    }
}

#[test]
fn row_7_drop_mark_rect_on_the_left_and_on_the_right() {
    for n in SCALES {
        let k = i64::from(n);
        let grid = Grid::new(n, 256, 16, 0);
        for i in [0usize, 1, 5, 15, 16, 100, 255] {
            let cel = grid.cell_rect(i).unwrap();
            assert_eq!(
                drop_mark_rect(cel, false, n),
                rect(cel.x - 3 * k, cel.y - 2 * k, 2 * k, 14 * k),
                "izquierda, n={n}, i={i}"
            );
            assert_eq!(
                drop_mark_rect(cel, true, n),
                rect(cel.x + 11 * k, cel.y - 2 * k, 2 * k, 14 * k),
                "derecha, n={n}, i={i}"
            );
        }
    }
}

#[test]
fn row_7_the_bar_to_the_right_of_a_swatch_is_the_one_to_the_left_of_the_next() {
    for n in SCALES {
        let grid = Grid::new(n, 256, 16, 0);
        for i in [0usize, 1, 7, 14, 16, 100, 238] {
            if i % COLUMNS == COLUMNS - 1 {
                continue;
            }
            let (cel, next) = (grid.cell_rect(i).unwrap(), grid.cell_rect(i + 1).unwrap());
            assert_eq!(
                drop_mark_rect(cel, true, n),
                drop_mark_rect(next, false, n),
                "n={n}, i={i}"
            );
        }
    }
}

#[test]
fn row_7_the_bar_fits_in_the_gap_and_covers_the_height_of_the_swatch() {
    for n in SCALES {
        let grid = Grid::new(n, 256, 16, 0);
        let cel = grid.cell_rect(37).unwrap();
        for after_2 in [false, true] {
            let bar = drop_mark_rect(cel, after_2, n);
            let (left, right) = (bar.x, bar.x + bar.width);
            if after_2 {
                assert!(left >= cel.x + cel.width && right <= cel.x + 14 * i64::from(n));
            } else {
                assert!(right <= cel.x && left >= cel.x - 4 * i64::from(n));
            }
            assert!(bar.y <= cel.y && bar.y + bar.height >= cel.y + cel.height);
        }
    }
}

#[test]
fn row_7_drag_labels() {
    assert_eq!(
        drag_labels(5, Some(9)),
        ["mover 5 a 9".to_string(), "Escape cancela".to_string()]
    );
    assert_eq!(
        drag_labels(5, None),
        [
            "mover 5: suelta en la rejilla".to_string(),
            "Escape cancela".to_string()
        ]
    );
    assert_eq!(drag_labels(0, Some(255))[0], "mover 0 a 255");
}

#[test]
fn row_7_move_summary() {
    assert_eq!(
        summary(&PaletteOp::Move { from: 5, to: 9 }, 5, 12, false, false),
        "entrada 5 movida a la posición 9"
    );
    assert_eq!(
        summary(&PaletteOp::Move { from: 0, to: 2 }, 0, 4, false, false),
        "entrada 0 movida a la posición 2"
    );
}

#[test]
fn row_7_the_selection_after_moving_is_to() {
    assert_eq!(choice_after(&PaletteOp::Move { from: 0, to: 2 }, 0, 4), 2);
    assert_eq!(choice_after(&PaletteOp::Move { from: 9, to: 1 }, 9, 12), 1);
    assert_eq!(choice_after(&PaletteOp::Move { from: 4, to: 4 }, 4, 12), 4);
}

#[test]
fn row_7_moving_from_0_to_2_in_f_gives_g_b_r_k_and_a_single_undo() {
    let mut doc = Document::new(image_f(None));
    let id = ImageId::from_raw(0);
    let initial = image_of(&doc, id).clone();
    assert!(
        doc.apply(Command::MovePaletteEntry {
            image: id,
            from: 0,
            to: 2
        })
        .unwrap()
    );
    assert_eq!(colors_of(&doc, id), vec![GREEN, BLUE, RED, TEST_BLACK]);
    assert_eq!(pixels_2(image_of(&doc, id)), vec![2, 0, 1, 3]);
    doc.undo().unwrap();
    assert_eq!(image_of(&doc, id), &initial);
    assert!(!doc.can_undo(), "un solo paso");
    // A la misma posición no hay paso.
    assert!(
        !doc.apply(Command::MovePaletteEntry {
            image: id,
            from: 1,
            to: 1
        })
        .unwrap()
    );
    assert!(!doc.can_undo());
}

#[test]
fn row_7_move_drag_a_3_px_click_is_click_and_a_4_px_one_starts() {
    let mut click = MoveDrag::press(2, (10, 10));
    assert!(!click.dragging());
    click.update((13, 10), 1, Some(5));
    assert!(!click.dragging(), "3 px: bajo el umbral");
    assert_eq!(click.preview(), None);
    assert_eq!(click.release(), DragEnd::Click);

    let mut drag = MoveDrag::press(2, (10, 10));
    drag.update((14, 10), 1, Some(5));
    assert!(drag.dragging(), "4 px: umbral alcanzado");
    assert_eq!(drag.preview(), Some((2, 5)));
    assert_eq!(drag.release(), DragEnd::Move { from: 2, to: 5 });
}

#[test]
fn row_7_move_drag_the_threshold_is_euclidean_and_scales_with_n() {
    // dx = dy = 3: 18 >= 16.
    let mut diagonal = MoveDrag::press(0, (0, 0));
    diagonal.update((3, 3), 1, Some(1));
    assert!(diagonal.dragging());
    // dx = 3, dy = 2: 13 < 16.
    let mut short = MoveDrag::press(0, (0, 0));
    short.update((3, 2), 1, Some(1));
    assert!(!short.dragging());
    // Hacia atrás también cuenta.
    let mut back = MoveDrag::press(0, (10, 10));
    back.update((6, 10), 1, Some(1));
    assert!(back.dragging());
    let mut back_short = MoveDrag::press(0, (10, 10));
    back_short.update((7, 10), 1, Some(1));
    assert!(!back_short.dragging());
    // Con n = 2 el umbral es de 8 px; con n = 3, de 12.
    for (n, just_before, just) in [(2u32, 7i64, 8i64), (3, 11, 12)] {
        let mut before = MoveDrag::press(0, (0, 0));
        before.update((just_before, 0), n, Some(1));
        assert!(!before.dragging(), "n={n}, {just_before} px");
        let mut now = MoveDrag::press(0, (0, 0));
        now.update((0, just), n, Some(1));
        assert!(now.dragging(), "n={n}, {just} px");
    }
}

#[test]
fn row_7_move_drag_to_the_same_one_without_a_target_or_outside_is_nothing() {
    let mut same = MoveDrag::press(4, (0, 0));
    same.update((20, 0), 1, Some(4));
    assert!(same.dragging());
    assert_eq!(same.preview(), None, "to == from: sin vista previa");
    assert_eq!(same.release(), DragEnd::Nothing);

    let mut outside = MoveDrag::press(4, (0, 0));
    outside.update((20, 0), 1, None);
    assert_eq!(outside.preview(), None);
    assert_eq!(outside.release(), DragEnd::Nothing);

    // Se pasó por un destino pero se soltó fuera: manda el último.
    let mut after_2 = MoveDrag::press(4, (0, 0));
    after_2.update((20, 0), 1, Some(7));
    after_2.update((40, 0), 1, None);
    assert_eq!(after_2.release(), DragEnd::Nothing);
}

#[test]
fn row_7_move_drag_stores_the_last_target() {
    let mut drag = MoveDrag::press(1, (0, 0));
    drag.update((20, 0), 1, Some(3));
    drag.update((30, 0), 1, Some(8));
    assert_eq!(drag.preview(), Some((1, 8)));
    drag.update((40, 0), 1, Some(1));
    assert_eq!(drag.preview(), None);
    drag.update((50, 0), 1, Some(6));
    assert_eq!(drag.release(), DragEnd::Move { from: 1, to: 6 });
}

#[test]
fn row_7_move_drag_does_not_go_back_when_returning_to_the_press_point() {
    let mut drag = MoveDrag::press(1, (100, 100));
    drag.update((120, 100), 1, Some(3));
    assert!(drag.dragging());
    drag.update((100, 100), 1, Some(5));
    assert!(drag.dragging(), "volver al punto inicial no desarma");
    assert_eq!(drag.release(), DragEnd::Move { from: 1, to: 5 });
}

#[test]
fn row_7_move_drag_before_the_threshold_a_target_is_worth_nothing() {
    let mut click = MoveDrag::press(1, (0, 0));
    click.update((1, 1), 1, Some(9));
    click.update((0, 2), 1, Some(4));
    assert_eq!(click.preview(), None);
    assert_eq!(click.release(), DragEnd::Click);
}

#[test]
fn row_7_move_drag_without_any_update_is_click() {
    assert_eq!(MoveDrag::press(7, (5, 5)).release(), DragEnd::Click);
}

#[test]
fn row_7_move_drag_with_large_coordinates_does_not_overflow() {
    let mut drag = MoveDrag::press(0, (-1_000_000_000, -1_000_000_000));
    drag.update((1_000_000_000, 1_000_000_000), 8, Some(1));
    assert!(drag.dragging());
    assert_eq!(drag.release(), DragEnd::Move { from: 0, to: 1 });
}

#[test]
fn row_11_a_drag_with_a_single_entry_can_only_land_on_it() {
    let grid = Grid::new(1, 1, 16, 0);
    let target_2 = grid.drop_index(100, 0);
    assert_eq!(target_2, Some(0));
    let mut drag = MoveDrag::press(0, (0, 0));
    drag.update((100, 0), 1, target_2.map(|i| i as u8));
    assert_eq!(drag.release(), DragEnd::Nothing);
}

#[test]
fn row_11_with_256_entries_the_drag_from_0_to_255_is_move() {
    let grid = Grid::new(1, 256, 16, 0);
    let target_2 = grid.drop_index(210, 210).map(|i| i as u8);
    assert_eq!(target_2, Some(255));
    let mut drag = MoveDrag::press(0, (0, 0));
    drag.update((210, 210), 1, target_2);
    assert_eq!(drag.release(), DragEnd::Move { from: 0, to: 255 });
    assert!(blocked_reason(PaletteButton::Add, 256, false).is_some());
}

// Fila 8: clamp_choice.

#[test]
fn row_8_clamp_choice_in_indexed() {
    let five = image_with(&[RED, GREEN, BLUE, X, Y], None, &[0, 1, 2, 3, 4]);
    assert_eq!(
        clamp_choice(&five, ColorChoice::Entry(7)),
        ColorChoice::Entry(4)
    );
    assert_eq!(
        clamp_choice(&five, ColorChoice::Entry(2)),
        ColorChoice::Entry(2)
    );
    assert_eq!(
        clamp_choice(&five, ColorChoice::Slot(3)),
        ColorChoice::Entry(2)
    );
    let two = image_with(&[RED, GREEN], None, &[0, 1]);
    assert_eq!(
        clamp_choice(&two, ColorChoice::Slot(3)),
        ColorChoice::Entry(1)
    );
    assert_eq!(
        clamp_choice(&two, ColorChoice::Entry(255)),
        ColorChoice::Entry(1)
    );
    let a = image_with(&[RED], None, &[0]);
    assert_eq!(
        clamp_choice(&a, ColorChoice::Entry(9)),
        ColorChoice::Entry(0)
    );
    assert_eq!(
        clamp_choice(&a, ColorChoice::Slot(9)),
        ColorChoice::Entry(0)
    );
}

#[test]
fn row_8_clamp_choice_without_a_palette() {
    for mode_2 in [Mode::Rgba, Mode::Gray, Mode::GrayAlpha] {
        let image_2 = Image::new(4, 4, mode_2).unwrap();
        assert_eq!(
            clamp_choice(&image_2, ColorChoice::Slot(3)),
            ColorChoice::Slot(3),
            "{mode_2:?}"
        );
        assert_eq!(
            clamp_choice(&image_2, ColorChoice::Slot(9)),
            ColorChoice::Slot(9),
            "{mode_2:?}"
        );
        assert_eq!(
            clamp_choice(&image_2, ColorChoice::Entry(0)),
            ColorChoice::Slot(1),
            "{mode_2:?}: la inicial"
        );
        assert_eq!(
            clamp_choice(&image_2, ColorChoice::Entry(200)),
            ColorChoice::Slot(1),
            "{mode_2:?}: la inicial"
        );
    }
}

fn choice_any() -> impl Strategy<Value = ColorChoice> {
    prop_oneof![
        any::<u8>().prop_map(ColorChoice::Entry),
        any::<u8>().prop_map(ColorChoice::Slot),
    ]
}

proptest! {
    #[test]
    fn clamp_choice_in_indexed_gives_a_valid_idempotent_entry_and_respects_what_is_valid(
        length_2 in 1usize..=256,
        choice_2 in choice_any(),
    ) {
        let colors_2: Vec<Rgba> = (0..length_2).map(|i| Rgba::opaque(i as u8, 0, 0)).collect();
        let image_2 = image_with(&colors_2, None, &[0]);
        let result = clamp_choice(&image_2, choice_2);
        let ColorChoice::Entry(index_2) = result else {
            return Err(TestCaseError::fail(format!("no es Entry: {result:?}")));
        };
        prop_assert!(usize::from(index_2) < length_2);
        prop_assert_eq!(clamp_choice(&image_2, result), result);
        if let ColorChoice::Entry(i) = choice_2
            && usize::from(i) < length_2
        {
            prop_assert_eq!(result, choice_2);
        }
    }
}

// Fila 10: destino de la entrada con texto pendiente.

#[test]
fn row_10_entry_fate_of_move_with_entry_3() {
    assert_eq!(
        entry_fate(&PaletteOp::Move { from: 3, to: 5 }, 3),
        Fate::Moved
    );
    assert_eq!(
        entry_fate(&PaletteOp::Move { from: 1, to: 4 }, 3),
        Fate::Moved
    );
    assert_eq!(
        entry_fate(&PaletteOp::Move { from: 5, to: 0 }, 3),
        Fate::Moved
    );
    assert_eq!(
        entry_fate(&PaletteOp::Move { from: 6, to: 7 }, 3),
        Fate::Same
    );
}

#[test]
fn row_10_entry_fate_of_remove_with_entry_3() {
    assert_eq!(
        entry_fate(&PaletteOp::Remove { index: 3, into: 0 }, 3),
        Fate::Gone
    );
    assert_eq!(
        entry_fate(&PaletteOp::Remove { index: 1, into: 0 }, 3),
        Fate::Moved
    );
    assert_eq!(
        entry_fate(&PaletteOp::Remove { index: 5, into: 0 }, 3),
        Fate::Same
    );
}

#[test]
fn row_10_entry_fate_of_rotate_with_entry_3() {
    assert_eq!(
        entry_fate(
            &PaletteOp::Rotate {
                start: 2,
                len: 3,
                steps: 1
            },
            3
        ),
        Fate::Changed,
        "entradas 2 a 4"
    );
    assert_eq!(
        entry_fate(
            &PaletteOp::Rotate {
                start: 5,
                len: 2,
                steps: 1
            },
            3
        ),
        Fate::Same
    );
}

#[test]
fn row_10_entry_fate_of_add_is_same_for_any_entry() {
    for input_2 in [0u8, 3, 255] {
        assert_eq!(entry_fate(&PaletteOp::Add, input_2), Fate::Same);
    }
}

#[test]
fn row_10_entry_fate_at_the_edges() {
    // Mover una entrada a su sitio no la cambia.
    assert_eq!(
        entry_fate(&PaletteOp::Move { from: 3, to: 3 }, 3),
        Fate::Same
    );
    // Rotar un número de pasos múltiplo de la longitud no cambia nada.
    for steps_2 in [0, 3, -3, 6] {
        assert_eq!(
            entry_fate(
                &PaletteOp::Rotate {
                    start: 2,
                    len: 3,
                    steps: steps_2
                },
                3
            ),
            Fate::Same,
            "steps {steps_2}"
        );
    }
    // Rotar a la izquierda cambia lo mismo.
    assert_eq!(
        entry_fate(
            &PaletteOp::Rotate {
                start: 2,
                len: 3,
                steps: -1
            },
            3
        ),
        Fate::Changed
    );
    // Los extremos del rango están dentro; los vecinos, fuera.
    for (input_2, expected_2) in [
        (1u8, Fate::Same),
        (2, Fate::Changed),
        (4, Fate::Changed),
        (5, Fate::Same),
    ] {
        assert_eq!(
            entry_fate(
                &PaletteOp::Rotate {
                    start: 2,
                    len: 3,
                    steps: 1
                },
                input_2
            ),
            expected_2,
            "entrada {input_2}"
        );
    }
}

#[test]
fn row_10_the_pending_text_resolved_before_does_not_remain_and_discard_pending_deletes_it() {
    let color = Rgba::opaque(10, 20, 30);
    let mut editor = ColorEditor::default();
    editor.sync(Some((EditTarget::Entry(3), color)));
    assert_eq!(editor.pending_target(), None);
    assert!(!editor.discard_pending(), "nada pendiente");

    editor.type_hex(EditTarget::Entry(3), color, "ff0000".to_string());
    assert!(editor.has_pending());
    assert_eq!(editor.pending_target(), Some(EditTarget::Entry(3)));
    assert!(editor.discard_pending(), "había texto");
    assert!(!editor.has_pending());
    assert_eq!(editor.pending_target(), None);
    assert_eq!(editor.hex_display(), None);
    assert!(!editor.discard_pending(), "ya no hay");
}

#[test]
fn row_10_discard_pending_gives_no_commit_afterwards() {
    use px_ui::color_edit::HexOutcome;
    let color = Rgba::opaque(10, 20, 30);
    let mut editor = ColorEditor::default();
    editor.sync(Some((EditTarget::Entry(3), color)));
    editor.type_hex(EditTarget::Entry(3), color, "ff0000".to_string());
    editor.discard_pending();
    assert!(matches!(
        editor.confirm_hex(Mode::Indexed, true),
        HexOutcome::Nothing
    ));
}

// Fila 11: banderas de la línea 2 del panel.

#[test]
fn row_11_info_flags_range_of_the_spec() {
    assert_eq!(info_flags_alpha0(false, false, true, false, false), "rango");
    assert_eq!(
        info_flags_alpha0(true, false, true, false, false),
        "cursor · rango"
    );
    assert_eq!(
        info_flags_alpha0(true, false, true, true, false),
        "cursor · rango · transparente"
    );
    assert_eq!(
        info_flags_alpha0(true, true, false, true, false),
        "cursor · activa · transparente"
    );
    assert_eq!(info_flags_alpha0(false, false, false, false, false), "");
}

#[test]
fn row_11_the_range_flag_does_not_come_out_on_the_active_one() {
    assert_eq!(info_flags_alpha0(false, true, true, false, false), "activa");
    assert_eq!(
        info_flags_alpha0(true, true, true, false, false),
        "cursor · activa"
    );
    assert_eq!(
        info_flags_alpha0(true, true, true, true, false),
        "cursor · activa · transparente"
    );
}

#[test]
fn row_11_info_flags_range_matches_a_model_in_the_16_combinations() {
    for bits in 0..16u8 {
        let (cursor, active_2, range, transparent) =
            (bits & 1 != 0, bits & 2 != 0, bits & 4 != 0, bits & 8 != 0);
        let mut expected_2 = Vec::new();
        if cursor {
            expected_2.push("cursor");
        }
        if active_2 {
            expected_2.push("activa");
        }
        if range && !active_2 {
            expected_2.push("rango");
        }
        if transparent {
            expected_2.push("transparente");
        }
        let obtained = info_flags_alpha0(cursor, active_2, range, transparent, false);
        assert_eq!(obtained, expected_2.join(" · "), "bits {bits:04b}");
        assert!(characters(&obtained) <= 30, "la más larga son 30 columnas");
        assert!(in_the_set(&obtained));
    }
    assert_eq!(
        characters("cursor · activa · transparente"),
        30,
        "es la línea más larga"
    );
}

// Fila 12: textos.

/// Todos los avisos de la unidad (barra de estado), con los números más largos.
fn notices() -> Vec<String> {
    let mut notices: Vec<String> = vec![
        REMOVAL_CANCELLED_NOTICE.to_string(),
        NOTHING_TO_ROTATE_NOTICE.to_string(),
        stale_notice(StaleReason::Moved).to_string(),
        stale_notice(StaleReason::Gone).to_string(),
        stale_notice(StaleReason::Changed).to_string(),
    ];
    for button_2 in BUTTONS {
        for length_2 in [0usize, 1, 2, 255, 256] {
            for target_2 in [false, true] {
                if let Some(reason_2) = blocked_reason(button_2, length_2, target_2) {
                    notices.push(reason_2.to_string());
                }
            }
        }
    }
    // Añadir: todos los números de entrada.
    for length_2 in 1..=255usize {
        for active_2 in [0u8, (length_2 - 1) as u8] {
            notices.push(summary(&PaletteOp::Add, active_2, length_2, false, false));
        }
    }
    // Quitar: todas las parejas, con las cuatro combinaciones de `unused` y `was_transparent`.
    for index_2 in 0..=255u8 {
        for target_2 in 0..=255u8 {
            if target_2 == index_2 {
                continue;
            }
            for (without_pixels, was_transparent) in
                [(false, false), (true, false), (true, true), (false, true)]
            {
                notices.push(summary(
                    &PaletteOp::Remove {
                        index: index_2,
                        into: target_2,
                    },
                    index_2,
                    256,
                    was_transparent,
                    without_pixels,
                ));
            }
        }
    }
    // Mover.
    for from_2 in 0..=255u8 {
        for until in 0..=255u8 {
            notices.push(summary(
                &PaletteOp::Move {
                    from: from_2,
                    to: until,
                },
                from_2,
                256,
                false,
                false,
            ));
        }
    }
    // Rotar: todos los tramos posibles de una paleta de 256 y de una de 255.
    for long_palette in [255usize, 256] {
        for start_2 in 0..long_palette {
            for length in 1..=(long_palette - start_2) {
                for steps_2 in [1, -1] {
                    notices.push(summary(
                        &PaletteOp::Rotate {
                            start: start_2 as u8,
                            len: length as u16,
                            steps: steps_2,
                        },
                        0,
                        long_palette,
                        false,
                        false,
                    ));
                }
            }
        }
    }
    notices
}

#[test]
fn row_12_the_longest_remove_summary_is_43_characters() {
    let text_2 = summary(
        &PaletteOp::Remove {
            index: 255,
            into: 254,
        },
        255,
        256,
        false,
        false,
    );
    assert_eq!(text_2, "entrada 255 quitada; píxeles pasan a la 254");
    assert_eq!(characters(&text_2), 43);
}

#[test]
fn row_12_no_notice_passes_43_characters_or_leaves_the_character_set() {
    let all_2 = notices();
    assert!(all_2.len() > 100_000, "se probaron muchos: {}", all_2.len());
    let more_long_2 = all_2.iter().map(|text_2| characters(text_2)).max().unwrap();
    assert_eq!(
        more_long_2, 43,
        "el más largo es el de tres cifras de Quitar"
    );
    for text_2 in &all_2 {
        assert!(
            characters(text_2) <= 43,
            "{text_2:?}: {} caracteres",
            characters(text_2)
        );
        assert!(in_the_set(text_2), "{text_2:?} sale de INTERFACE_CHARSET");
        assert!(!text_2.is_empty());
    }
}

#[test]
fn row_12_with_the_notice_prefix_they_fit_in_the_50_columns_of_361_u_and_fit_text_does_not_clip() {
    assert_eq!(characters(NOTICE_PREFIX), 7);
    for ppp in PPPS {
        let n = text_scale(ppp);
        let width_px = (361 - 6) as f32 * n as f32;
        let maximum_2 = columns(width_px, ppp);
        assert_eq!(maximum_2, 50, "ppp {ppp}");
        assert_eq!(maximum_2 - characters(NOTICE_PREFIX), 43);
    }
    let all_2 = notices();
    for text_2 in all_2.iter().step_by(97) {
        let complete = format!("{NOTICE_PREFIX}{text_2}");
        let fits = fit_text(&complete, 50, |c| INTERFACE_CHARSET.contains(c));
        assert_eq!(fits, complete, "se recortó {complete:?}");
    }
    let the_more_long = format!(
        "{NOTICE_PREFIX}{}",
        summary(
            &PaletteOp::Remove {
                index: 255,
                into: 254
            },
            255,
            256,
            false,
            false
        )
    );
    assert_eq!(characters(&the_more_long), 50);
    assert_eq!(
        fit_text(&the_more_long, 50, |c| INTERFACE_CHARSET.contains(c)),
        the_more_long
    );
}

#[test]
fn row_12_the_editor_and_panel_lines_fit_in_32_columns() {
    for ppp in PPPS {
        let n = text_scale(ppp);
        assert_eq!(columns(225.0 * n as f32, ppp), 32, "ppp {ppp}");
    }
    let mut lines: Vec<String> = Vec::new();
    for index_2 in 0..=255u8 {
        for target_2 in std::iter::once(None).chain((0..=255u8).map(Some)) {
            for notice in [
                None,
                Some(RemovalWarning::PixelsOpaque),
                Some(RemovalWarning::DestinationTransparent),
            ] {
                lines.extend(removal_prompt(index_2, target_2, notice));
            }
        }
    }
    for from_2 in 0..=255u8 {
        for until in std::iter::once(None).chain((0..=255u8).map(Some)) {
            lines.extend(drag_labels(from_2, until));
        }
    }
    for bits in 0..16u8 {
        lines.push(info_flags_alpha0(
            bits & 1 != 0,
            bits & 2 != 0,
            bits & 4 != 0,
            bits & 8 != 0,
            false,
        ));
    }
    let more_long = lines.iter().map(|line| characters(line)).max().unwrap();
    assert!(more_long <= 32, "la más larga tiene {more_long}");
    for line in &lines {
        assert!(characters(line) <= 32, "{line:?}");
        assert!(in_the_set(line), "{line:?} sale de INTERFACE_CHARSET");
    }
}

#[test]
fn row_12_the_button_labels_fit_and_are_in_the_character_set() {
    // Texto en (6, 1) dentro del botón; 7 u por carácter.
    for (label, width_u) in [
        ("Añadir", 110),
        ("Quitar", 110),
        ("Confirmar", 110),
        ("Cancelar", 110),
        ("Rotar a la izquierda", 225),
        ("Rotar a la derecha", 225),
    ] {
        assert!(in_the_set(label), "{label}");
        let used = 6 + 7 * characters(label) as i64;
        assert!(used <= width_u, "{label}: {used} u en {width_u} u");
    }
    assert_eq!(6 + 140, 146, "Rotar a la izquierda: 6 + 140 u");
}

// PaletteOps.

#[test]
fn palette_ops_new_is_empty() {
    let mut ops = PaletteOps::default();
    assert!(ops.drag().is_none());
    assert_eq!(ops.range_end(), None);
    assert_eq!(ops.removal(), None);
    assert!(!ops.busy());
    assert_eq!(ops.end_drag(), None);
}

#[test]
fn palette_ops_sync_without_a_pending_removal_does_not_warn() {
    let mut ops = PaletteOps::default();
    assert!(!ops.sync(0, 5));
    assert!(!ops.sync(1, 5));
    assert!(!ops.sync(99, 1));
}

#[test]
fn palette_ops_sync_with_the_same_state_keeps_the_removal() {
    let mut ops = PaletteOps::default();
    ops.sync(7, 5);
    ops.begin_removal(3, Some(0));
    assert_eq!(
        ops.removal(),
        Some(Removal {
            index: 3,
            into: Some(0)
        })
    );
    for _ in 0..3 {
        assert!(!ops.sync(7, 5));
    }
    assert_eq!(
        ops.removal(),
        Some(Removal {
            index: 3,
            into: Some(0)
        })
    );
}

#[test]
fn palette_ops_sync_cancels_the_removal_if_the_document_changes_and_warns_once() {
    for new_2 in [8u64, 6, 0, u64::MAX] {
        let mut ops = PaletteOps::default();
        ops.sync(7, 5);
        ops.begin_removal(3, Some(0));
        assert!(ops.sync(new_2, 5), "estado {new_2}: hay que avisar");
        assert_eq!(ops.removal(), None);
        assert!(!ops.sync(new_2, 5), "solo avisa una vez");
        assert!(
            !ops.sync(new_2.wrapping_add(1), 5),
            "sin quita no hay aviso"
        );
    }
}

#[test]
fn palette_ops_sync_cancels_the_removal_without_a_target_if_the_document_changes() {
    let mut ops = PaletteOps::default();
    ops.sync(1, 5);
    ops.begin_removal(3, None);
    assert!(ops.sync(2, 5));
    assert_eq!(ops.removal(), None);
}

#[test]
fn palette_ops_sync_cancels_the_removal_if_its_indices_are_no_longer_valid() {
    // `index` fuera de la paleta.
    let mut ops = PaletteOps::default();
    ops.sync(1, 5);
    ops.begin_removal(4, Some(0));
    assert!(!ops.sync(1, 5), "4 < 5 vale");
    assert!(ops.sync(1, 4), "4 >= 4 ya no vale");
    assert_eq!(ops.removal(), None);

    // `into` fuera de la paleta.
    let mut ops = PaletteOps::default();
    ops.sync(1, 5);
    ops.begin_removal(1, Some(4));
    assert!(ops.sync(1, 4));
    assert_eq!(ops.removal(), None);

    // Sin destino solo importa `index`.
    let mut ops = PaletteOps::default();
    ops.sync(1, 5);
    ops.begin_removal(3, None);
    assert!(!ops.sync(1, 4), "3 < 4 vale");
    assert_eq!(ops.removal().map(|removes| removes.index), Some(3));
    assert!(ops.sync(1, 3));
    assert_eq!(ops.removal(), None);
}

#[test]
fn palette_ops_begin_removal_is_tied_to_the_state_of_the_last_sync() {
    let mut ops = PaletteOps::default();
    ops.sync(10, 5);
    ops.sync(11, 5);
    ops.begin_removal(2, Some(1));
    assert!(!ops.sync(11, 5), "el de la última sync");
    ops.cancel_removal();
    assert_eq!(ops.removal(), None);

    ops.begin_removal(2, Some(1));
    assert!(
        ops.sync(10, 5),
        "el estado anterior ya no es el de la quita"
    );
}

#[test]
fn palette_ops_sync_clips_the_other_end_of_the_range() {
    let mut ops = PaletteOps::default();
    ops.sync(1, 10);
    ops.set_range_end(8);
    assert_eq!(ops.range_end(), Some(8));
    assert!(!ops.sync(1, 10));
    assert_eq!(ops.range_end(), Some(8), "cabe");
    ops.sync(1, 6);
    assert_eq!(ops.range_end(), Some(5), "se recorta a la última");
    ops.sync(1, 5);
    assert_eq!(ops.range_end(), Some(4));
    ops.sync(1, 20);
    assert_eq!(ops.range_end(), Some(4), "no vuelve a crecer");
}

#[test]
fn palette_ops_range_set_and_clear() {
    let mut ops = PaletteOps::default();
    ops.set_range_end(3);
    assert_eq!(ops.range_end(), Some(3));
    ops.set_range_end(7);
    assert_eq!(ops.range_end(), Some(7));
    ops.clear_range();
    assert_eq!(ops.range_end(), None);
    ops.clear_range();
    assert_eq!(ops.range_end(), None);
}

#[test]
fn palette_ops_set_destination() {
    let mut ops = PaletteOps::default();
    assert!(!ops.set_destination(2, 5), "sin quita pendiente");
    assert_eq!(ops.removal(), None);

    ops.sync(1, 5);
    ops.begin_removal(3, Some(0));
    assert!(!ops.set_destination(3, 5), "la propia se ignora");
    assert_eq!(ops.removal().map(|removes| removes.into), Some(Some(0)));
    assert!(!ops.set_destination(5, 5), "fuera de la paleta");
    assert!(!ops.set_destination(200, 5));
    assert_eq!(ops.removal().map(|removes| removes.into), Some(Some(0)));

    assert!(ops.set_destination(2, 5));
    assert_eq!(
        ops.removal(),
        Some(Removal {
            index: 3,
            into: Some(2)
        })
    );
    assert!(ops.set_destination(4, 5), "el último vale");
    assert_eq!(ops.removal().map(|removes| removes.into), Some(Some(4)));
}

#[test]
fn palette_ops_set_destination_gives_a_target_to_a_removal_without_it() {
    let mut ops = PaletteOps::default();
    ops.sync(1, 5);
    ops.begin_removal(3, None);
    assert!(ops.set_destination(4, 5));
    assert_eq!(
        ops.removal(),
        Some(Removal {
            index: 3,
            into: Some(4)
        })
    );
    // El destino no cambia a qué estado está atada.
    assert!(!ops.sync(1, 5));
    assert!(ops.sync(2, 5));
}

#[test]
fn palette_ops_cancel_removal() {
    let mut ops = PaletteOps::default();
    ops.cancel_removal();
    ops.sync(1, 5);
    ops.begin_removal(3, Some(0));
    ops.cancel_removal();
    assert_eq!(ops.removal(), None);
    assert!(!ops.sync(2, 5), "cancelada a mano: no hay aviso");
}

#[test]
fn palette_ops_busy_only_with_a_real_drag() {
    let mut ops = PaletteOps::default();
    assert!(!ops.busy());
    ops.begin_drag(2, (10, 10));
    assert!(ops.drag().is_some());
    assert!(!ops.busy(), "armado, antes del umbral");
    ops.drag().unwrap().update((13, 10), 1, Some(5));
    assert!(!ops.busy(), "3 px");
    ops.drag().unwrap().update((14, 10), 1, Some(5));
    assert!(ops.busy(), "4 px");
    assert!(ops.drag().is_some_and(|drag| drag.dragging()));
    ops.drag().unwrap().update((10, 10), 1, Some(5));
    assert!(ops.busy(), "no vuelve atrás");

    assert_eq!(ops.end_drag(), Some(DragEnd::Move { from: 2, to: 5 }));
    assert!(!ops.busy());
    assert!(ops.drag().is_none());
    assert_eq!(ops.end_drag(), None, "una sola salida");
}

#[test]
fn palette_ops_an_armed_click_ends_in_click() {
    let mut ops = PaletteOps::default();
    ops.begin_drag(4, (0, 0));
    ops.drag().unwrap().update((2, 1), 1, Some(9));
    assert!(!ops.busy());
    assert_eq!(ops.end_drag(), Some(DragEnd::Click));
    assert!(ops.drag().is_none());
}

#[test]
fn palette_ops_cancel_drag_gives_no_output() {
    let mut ops = PaletteOps::default();
    ops.cancel_drag();
    ops.begin_drag(4, (0, 0));
    ops.drag().unwrap().update((40, 0), 1, Some(9));
    assert!(ops.busy());
    ops.cancel_drag();
    assert!(ops.drag().is_none());
    assert!(!ops.busy());
    assert_eq!(ops.end_drag(), None);
}

#[test]
fn palette_ops_reset_clears_everything() {
    let mut ops = PaletteOps::default();
    ops.sync(1, 5);
    ops.begin_removal(3, Some(0));
    ops.set_range_end(2);
    ops.begin_drag(1, (0, 0));
    ops.drag().unwrap().update((40, 0), 1, Some(9));
    assert!(ops.busy());
    ops.reset();
    assert_eq!(ops.removal(), None);
    assert_eq!(ops.range_end(), None);
    assert!(ops.drag().is_none());
    assert!(!ops.busy());
    assert!(!ops.sync(2, 5), "reset no deja una quita a medias");
}

// Propiedad 1: destino del arrastre.

fn coordinate(selector: u8, raw: i64, step: i64, delta: i64, n: i64, end: usize) -> i64 {
    const ENDS: [i64; 6] = [i64::MIN, i64::MIN + 1, i64::MAX - 1, i64::MAX, -1, 0];
    match selector % 8 {
        // Alrededor de los bordes de las zonas de captura.
        0..=3 => step * 14 * n - 2 * n + delta,
        4 => ENDS[end % ENDS.len()],
        _ => raw,
    }
}

/// Modelo de `drop_index` con `i128` y `div_euclid`.
fn model_drop_index(
    n: i128,
    count: usize,
    visible_2: usize,
    first: usize,
    x: i64,
    y: i64,
) -> Option<usize> {
    let (x, y) = (i128::from(x), i128::from(y));
    let total = count.div_ceil(COLUMNS);
    let with_swatches = visible_2.min(total.saturating_sub(first)) as i128;
    let step = 14 * n;
    let in_x = x >= -2 * n && x < COLUMNS as i128 * step - 2 * n;
    let in_y = y >= -2 * n && y < with_swatches * step - 2 * n;
    if !(in_x && in_y) || count == 0 {
        return None;
    }
    let column = (x + 2 * n).div_euclid(step);
    let row_2 = (y + 2 * n).div_euclid(step);
    let index_2 = (first as i128 + row_2) * COLUMNS as i128 + column;
    let last_2 = count as i128 - 1;
    Some(usize::try_from(index_2.min(last_2)).expect("índice dentro"))
}

proptest! {
    #[test]
    fn property_1_drop_index_is_total_and_matches_the_model(
        n in 1u32..=8,
        count in 0usize..=256,
        visible_2 in 0usize..=20,
        row_raw in any::<usize>(),
        (sx, cx, px, dx, ex) in (any::<u8>(), -40i64..3600, -1i64..=17, -3i64..=3, any::<usize>()),
        (sy, cy, py, dy, ey) in (any::<u8>(), -40i64..3600, -1i64..=17, -3i64..=3, any::<usize>()),
        (sx2, cx2, px2, dx2, ex2) in (any::<u8>(), -40i64..3600, -1i64..=17, -3i64..=3, any::<usize>()),
    ) {
        let total = total_rows(count);
        let first = row_raw % (max_first_row(total, visible_2) + 1);
        let grid = Grid::new(n, count, visible_2, first);
        let k = i64::from(n);
        let x = coordinate(sx, cx, px, dx, k, ex);
        let y = coordinate(sy, cy, py, dy, k, ey);
        let x2 = coordinate(sx2, cx2, px2, dx2, k, ex2);

        let obtained = grid.drop_index(x, y);
        prop_assert_eq!(obtained, model_drop_index(i128::from(n), count, visible_2, first, x, y));
        if let Some(i) = obtained {
            prop_assert!(i < count);
        }

        // Si `index_at` encuentra una muestra, `drop_index` da la misma.
        if let Some(i) = grid.index_at(x, y) {
            prop_assert_eq!(obtained, Some(i));
        }

        // Monótona en la fila: con `y` fijo, `x` mayor no da un índice menor.
        let (smaller, greater) = if x <= x2 { (x, x2) } else { (x2, x) };
        if let (Some(a), Some(b)) = (grid.drop_index(smaller, y), grid.drop_index(greater, y)) {
            prop_assert!(a <= b, "x={} da {}, x={} da {}", smaller, a, greater, b);
        }
    }

    #[test]
    fn property_1_the_center_of_each_visible_swatch_is_its_target(
        n in 1u32..=8,
        count in 1usize..=256,
        visible_2 in 1usize..=20,
        row_raw in any::<usize>(),
    ) {
        let total = total_rows(count);
        let first = row_raw % (max_first_row(total, visible_2) + 1);
        let grid = Grid::new(n, count, visible_2, first);
        for i in 0..count {
            if let Some(cel) = grid.cell_rect(i) {
                let center = (cel.x + cel.width / 2, cel.y + cel.height / 2);
                prop_assert_eq!(grid.drop_index(center.0, center.1), Some(i));
            }
        }
    }
}

// Propiedad 2: máquina del arrastre.

#[derive(Clone, Debug)]
enum TestAction {
    TestPress { from: u8, at: (i64, i64) },
    Move { at: (i64, i64), target: Option<u8> },
    Release,
    Cancel,
}

fn point() -> impl Strategy<Value = (i64, i64)> {
    (-40i64..40, -40i64..40)
}

fn action_2() -> impl Strategy<Value = TestAction> {
    prop_oneof![
        2 => (0u8..6, point()).prop_map(|(from, at)| TestAction::TestPress { from, at }),
        6 => (point(), proptest::option::of(0u8..6))
            .prop_map(|(at, target)| TestAction::Move { at, target }),
        2 => Just(TestAction::Release),
        1 => Just(TestAction::Cancel),
    ]
}

/// Modelo del arrastre: el punto de pulsación, si alguna vez se pasó el umbral y el último
/// destino dado con el arrastre vivo.
struct ModelDrag {
    from: u8,
    start_2: (i64, i64),
    crossed: bool,
    target_2: Option<u8>,
}

impl ModelDrag {
    fn move_pointer(&mut self, at: (i64, i64), n: u32, target: Option<u8>) {
        let dx = i128::from(at.0) - i128::from(self.start_2.0);
        let dy = i128::from(at.1) - i128::from(self.start_2.1);
        let threshold = 4 * i128::from(n);
        if dx * dx + dy * dy >= threshold * threshold {
            self.crossed = true;
        }
        if self.crossed {
            self.target_2 = target;
        }
    }

    fn output_2(&self) -> DragEnd {
        if !self.crossed {
            return DragEnd::Click;
        }
        match self.target_2 {
            Some(to) if to != self.from => DragEnd::Move {
                from: self.from,
                to,
            },
            _ => DragEnd::Nothing,
        }
    }

    fn preview(&self) -> Option<(u8, u8)> {
        match self.target_2 {
            Some(to) if self.crossed && to != self.from => Some((self.from, to)),
            _ => None,
        }
    }
}

proptest! {
    #[test]
    fn property_2_move_drag_follows_the_model(
        n in 1u32..=8,
        actions in vec(action_2(), 0..40),
    ) {
        let mut drag: Option<MoveDrag> = None;
        let mut model: Option<ModelDrag> = None;
        for action_2 in actions {
            match action_2 {
                TestAction::TestPress { from, at } => {
                    if drag.is_none() {
                        drag = Some(MoveDrag::press(from, at));
                        model = Some(ModelDrag { from, start_2: at, crossed: false, target_2: None });
                    }
                }
                TestAction::Move { at, target } => {
                    if let (Some(real), Some(expected_2)) = (drag.as_mut(), model.as_mut()) {
                        real.update(at, n, target);
                        expected_2.move_pointer(at, n, target);
                    }
                }
                TestAction::Release => {
                    if let (Some(real), Some(expected_2)) = (drag.take(), model.take()) {
                        prop_assert_eq!(real.release(), expected_2.output_2());
                    }
                }
                TestAction::Cancel => {
                    drag = None;
                    model = None;
                }
            }
            match (&drag, &model) {
                (Some(real), Some(expected_2)) => {
                    prop_assert_eq!(real.dragging(), expected_2.crossed);
                    prop_assert_eq!(real.preview(), expected_2.preview());
                }
                (None, None) => {}
                _ => prop_assert!(false, "real y modelo no coinciden en si hay arrastre"),
            }
        }
    }

    #[test]
    fn property_2_palette_ops_follows_the_model_and_gives_no_extra_outputs(
        n in 1u32..=8,
        actions in vec(action_2(), 0..40),
    ) {
        let mut ops = PaletteOps::default();
        let mut model: Option<ModelDrag> = None;
        for action_2 in actions {
            match action_2 {
                TestAction::TestPress { from, at } => {
                    if model.is_none() {
                        ops.begin_drag(from, at);
                        model = Some(ModelDrag { from, start_2: at, crossed: false, target_2: None });
                    }
                }
                TestAction::Move { at, target } => {
                    if let Some(expected_2) = model.as_mut() {
                        ops.drag().expect("hay arrastre").update(at, n, target);
                        expected_2.move_pointer(at, n, target);
                    }
                }
                TestAction::Release => {
                    let expected = model.take().map(|m| m.output_2());
                    prop_assert_eq!(ops.end_drag(), expected);
                    // Después de soltar no hay más salidas.
                    prop_assert_eq!(ops.end_drag(), None);
                }
                TestAction::Cancel => {
                    ops.cancel_drag();
                    model = None;
                    prop_assert_eq!(ops.end_drag(), None);
                }
            }
            prop_assert_eq!(ops.drag().is_some(), model.is_some());
            let crossed = model.as_ref().is_some_and(|m| m.crossed);
            prop_assert_eq!(ops.busy(), crossed);
            if let (Some(drag), Some(expected_2)) = (ops.drag(), model.as_ref()) {
                prop_assert_eq!(drag.preview(), expected_2.preview());
            }
        }
    }
}

// Propiedad 3: destino de la entrada y selección, con un modelo de identificadores.

/// Identificador de la entrada que añade `Add`.
const NEW: u32 = 1000;

fn strategy_steps() -> impl Strategy<Value = i32> {
    prop_oneof![
        4 => any::<i32>(),
        4 => -10i32..10,
        1 => Just(i32::MIN),
        1 => Just(i32::MAX),
    ]
}

/// Una operación válida para una paleta de `length_2` entradas, o `None` si el tipo no se puede.
fn operation_valid(length_2: usize, kind: u8, a: u8, b: u8, steps_2: i32) -> Option<PaletteOp> {
    match kind % 4 {
        0 => (length_2 < 256).then_some(PaletteOp::Add),
        1 => {
            if length_2 < 2 {
                return None;
            }
            let index_2 = usize::from(a) % length_2;
            let target_2 = (index_2 + 1 + usize::from(b) % (length_2 - 1)) % length_2;
            Some(PaletteOp::Remove {
                index: index_2 as u8,
                into: target_2 as u8,
            })
        }
        2 => Some(PaletteOp::Move {
            from: (usize::from(a) % length_2) as u8,
            to: (usize::from(b) % length_2) as u8,
        }),
        _ => {
            let start_2 = usize::from(a) % length_2;
            let length = 1 + usize::from(b) % (length_2 - start_2);
            Some(PaletteOp::Rotate {
                start: start_2 as u8,
                len: length as u16,
                steps: steps_2,
            })
        }
    }
}

/// Aplica la operación a un vector de identificadores.
fn model_apply(op: &PaletteOp, ids: &[u32]) -> Vec<u32> {
    let mut new_2 = ids.to_vec();
    match *op {
        PaletteOp::Add => new_2.push(NEW),
        PaletteOp::Remove { index, .. } => {
            new_2.remove(usize::from(index));
        }
        PaletteOp::Move { from, to } => {
            let id = new_2.remove(usize::from(from));
            new_2.insert(usize::from(to), id);
        }
        PaletteOp::Rotate { start, len, steps } => {
            let (start_2, fin) = (usize::from(start), usize::from(start) + usize::from(len));
            let k = i64::from(steps).rem_euclid(i64::from(len));
            new_2[start_2..fin].rotate_right(usize::try_from(k).expect("k pequeño"));
        }
        // `operation_valid` (`kind % 4`) no genera `SetTransparent`: la unidad 21 tiene su
        // propia propiedad (conmutador exacto) y la estrategia de 11d no se amplía.
        PaletteOp::SetTransparent { .. } => {
            unreachable!("operacion_valida no genera SetTransparent (propiedad propia, unidad 21)")
        }
    }
    new_2
}

fn target_model(op: &PaletteOp, ids: &[u32], new_2: &[u32], input_2: usize) -> Fate {
    let id = ids[input_2];
    if matches!(op, PaletteOp::Rotate { .. }) {
        // Mismo índice, otro color.
        return if new_2[input_2] == id {
            Fate::Same
        } else {
            Fate::Changed
        };
    }
    match new_2.iter().position(|&other_2| other_2 == id) {
        None => Fate::Gone,
        Some(position) if position == input_2 => Fate::Same,
        Some(_) => Fate::Moved,
    }
}

fn selection_model(op: &PaletteOp, active_2: u8, new_2: &[u32]) -> usize {
    match *op {
        PaletteOp::Add => new_2.iter().position(|&id| id == NEW).expect("la nueva"),
        PaletteOp::Remove { index, .. } => usize::from(index).min(new_2.len() - 1),
        PaletteOp::Move { from, .. } => new_2
            .iter()
            .position(|&id| id == u32::from(from))
            .expect("la movida"),
        PaletteOp::Rotate { .. } => usize::from(active_2),
        // Inalcanzable por la misma razón que en `model_apply`.
        PaletteOp::SetTransparent { .. } => {
            unreachable!("operacion_valida no genera SetTransparent (propiedad propia, unidad 21)")
        }
    }
}

proptest! {
    #[test]
    fn property_3_entry_fate_and_choice_after_follow_the_identifier_model(
        length_2 in 1usize..=256,
        kind in any::<u8>(),
        a in any::<u8>(),
        b in any::<u8>(),
        steps_2 in strategy_steps(),
        active_raw in any::<u8>(),
    ) {
        let Some(op) = operation_valid(length_2, kind, a, b, steps_2) else {
            return Ok(());
        };
        let active_2 = (usize::from(active_raw) % length_2) as u8;
        let ids: Vec<u32> = (0..length_2 as u32).collect();
        let new_2 = model_apply(&op, &ids);

        for input_2 in 0..length_2 {
            prop_assert_eq!(
                entry_fate(&op, input_2 as u8),
                target_model(&op, &ids, &new_2, input_2),
                "{:?}, entrada {}", op, input_2
            );
        }
        let chosen_2 = choice_after(&op, active_2, length_2);
        prop_assert_eq!(usize::from(chosen_2), selection_model(&op, active_2, &new_2), "{:?}", op);
        prop_assert!(usize::from(chosen_2) < new_2.len());
    }

    #[test]
    fn property_3_the_model_matches_what_px_core_does(
        length_2 in 1usize..=256,
        kind in any::<u8>(),
        a in any::<u8>(),
        b in any::<u8>(),
        steps_2 in strategy_steps(),
    ) {
        let Some(op) = operation_valid(length_2, kind, a, b, steps_2) else {
            return Ok(());
        };
        let color_of = |id: u32| {
            if id == NEW {
                Rgba::new(0, 0, 0, 1)
            } else {
                Rgba::opaque(id as u8, 255 - id as u8, 7)
            }
        };
        let ids: Vec<u32> = (0..length_2 as u32).collect();
        let colors_2: Vec<Rgba> = ids.iter().map(|&id| color_of(id)).collect();
        let mut doc = Document::new(image_with(&colors_2, None, &[0]));
        let id = ImageId::from_raw(0);
        let command = match op {
            PaletteOp::Add => Command::AddPaletteEntry { image: id, color: color_of(NEW) },
            PaletteOp::Remove { index, into } => {
                Command::RemovePaletteEntry { image: id, index, into }
            }
            PaletteOp::Move { from, to } => Command::MovePaletteEntry { image: id, from, to },
            PaletteOp::Rotate { start, len, steps } => {
                Command::RotateColors { image: id, start, len, steps }
            }
            // Inalcanzable por la misma razón que en `model_apply`.
            PaletteOp::SetTransparent { .. } => {
                unreachable!("operacion_valida no genera SetTransparent (propiedad propia, unidad 21)")
            }
        };
        doc.apply(command).expect("operación válida");
        let expected_2: Vec<Rgba> = model_apply(&op, &ids)
            .into_iter()
            .map(color_of)
            .collect();
        prop_assert_eq!(colors_of(&doc, id), expected_2, "{:?}", op);
    }

    #[test]
    fn default_destination_matches_a_brute_force_model(
        colors_2 in vec((any::<u8>(), any::<u8>(), any::<u8>(), any::<u8>()), 2..=12),
        transparent_raw in any::<usize>(),
        removed_raw in any::<usize>(),
    ) {
        let colors_2: Vec<Rgba> = colors_2.into_iter().map(|(r, g, b, a)| Rgba::new(r, g, b, a)).collect();
        let length_2 = colors_2.len();
        let transparent = {
            let t = transparent_raw % (length_2 + 1);
            (t < length_2).then_some(t as u8)
        };
        let index_2 = (removed_raw % length_2) as u8;
        let palette = palette(&colors_2, transparent);

        let expected_2 = if transparent == Some(index_2) {
            None
        } else {
            let premultiplicado = |c: Rgba| {
                let alpha = i64::from(c.a);
                [i64::from(c.r) * alpha, i64::from(c.g) * alpha, i64::from(c.b) * alpha, 255 * alpha]
            };
            let resolved = |j: usize| {
                let color = colors_2[j];
                if transparent == Some(j as u8) { Rgba { a: 0, ..color } } else { color }
            };
            let goal = premultiplicado(resolved(usize::from(index_2)));
            let mut better: Option<(i64, usize)> = None;
            for j in (0..length_2).filter(|&j| j != usize::from(index_2)) {
                let other_2 = premultiplicado(resolved(j));
                let distance: i64 = goal.iter().zip(other_2).map(|(&p, q)| (p - q) * (p - q)).sum();
                if better.is_none_or(|(d, _)| distance < d) {
                    better = Some((distance, j));
                }
            }
            better.map(|(_, j)| j as u8)
        };
        prop_assert_eq!(default_destination(&palette, index_2), expected_2);
        if let Some(target_2) = default_destination(&palette, index_2) {
            prop_assert_ne!(target_2, index_2);
            prop_assert!(usize::from(target_2) < length_2);
        }
    }

    #[test]
    fn uses_entry_matches_counting_the_pixels(
        width_2 in 1u32..8,
        height_2 in 1u32..8,
        length_2 in 1usize..8,
        raw_2_2 in vec(any::<u8>(), 64),
    ) {
        let colors_2: Vec<Rgba> = (0..length_2).map(|i| Rgba::opaque(i as u8, 0, 0)).collect();
        let mut image_2 = Image::new_indexed(width_2, height_2, palette(&colors_2, None)).unwrap();
        let mut used_2 = vec![false; length_2];
        for y in 0..height_2 {
            for x in 0..width_2 {
                let index_2 = (usize::from(raw_2_2[(y * 8 + x) as usize]) % length_2) as u8;
                image_2.set_pixel(x, y, Pixel::Indexed(index_2)).unwrap();
                used_2[usize::from(index_2)] = true;
            }
        }
        for (index_2, &used) in used_2.iter().enumerate() {
            prop_assert_eq!(uses_entry(&image_2, index_2 as u8), used, "entrada {}", index_2);
        }
    }
}

// Unidad 21: botón «Fijar/Quitar transparente» (specs/hito-2-fijar-transparente.md), en su parte
// pura: filas 3, 4, 6, 8 y 10 y la propiedad 1. El comando es `Command::SetTransparentIndex`
// (unidad 10); lo nuevo es `PaletteOp::SetTransparent`, `transparent_toggle`,
// `transparent_button_label`, `transparent_summary`, `entry_fate` y `choice_after`.

mod set_transparent {
    use super::*;
    use px_ui::palette_ops::{transparent_button_label, transparent_summary, transparent_toggle};
    use px_ui::pick::{EraseError, eraser_label, eraser_pixel, is_erasable};

    const SET: &str = "Fijar transparente";
    const REMOVE: &str = "Quitar transparente";

    /// Lo que hace pulsar el botón con `active_2`: el `PaletteOp` y el comando de `px-core`.
    fn press(doc: &mut Document, active_2: u8) -> Result<bool, px_core::DocumentError> {
        let id = ImageId::from_raw(0);
        let palette = image_of(doc, id).palette().expect("indexada");
        let op = PaletteOp::SetTransparent {
            index: transparent_toggle(palette, active_2),
        };
        let PaletteOp::SetTransparent { index } = op else {
            unreachable!()
        };
        doc.apply(Command::SetTransparentIndex { image: id, index })
    }

    fn mark(doc: &Document) -> Option<u8> {
        image_of(doc, ImageId::from_raw(0))
            .palette()
            .expect("indexada")
            .transparent_index()
    }

    fn label(doc: &Document, active_2: u8) -> &'static str {
        let palette = image_of(doc, ImageId::from_raw(0))
            .palette()
            .expect("indexada");
        transparent_button_label(palette, active_2)
    }

    fn resolved(doc: &Document, index_2: u8) -> Rgba {
        image_of(doc, ImageId::from_raw(0))
            .palette()
            .expect("indexada")
            .resolved(index_2)
            .expect("entrada válida")
    }

    /// Color visible de cada píxel, calculado a mano (sin `Palette::resolved`).
    fn visible_2(image_2: &Image) -> Vec<Rgba> {
        let palette = image_2.palette().expect("indexada");
        (0..image_2.width())
            .map(|x| {
                let Pixel::Indexed(i) = image_2.get_pixel(x, 0).expect("píxel") else {
                    panic!("no indexado")
                };
                let saved = palette.colors()[usize::from(i)];
                if palette.transparent_index() == Some(i) {
                    Rgba { a: 0, ..saved }
                } else {
                    saved
                }
            })
            .collect()
    }

    // Tipos.

    #[test]
    fn set_transparent_es_copy_debug_y_comparable() {
        let op = PaletteOp::SetTransparent { index: Some(2) };
        let copy = op;
        assert_eq!(op, copy);
        assert_ne!(op, PaletteOp::SetTransparent { index: None });
        assert_ne!(op, PaletteOp::SetTransparent { index: Some(3) });
        assert_ne!(op, PaletteOp::Add);
        assert!(format!("{op:?}").contains("SetTransparent"));
    }

    // Fila 3: fijar y quitar.

    #[test]
    fn row_3_set_over_f_with_the_2_is_a_single_step_that_only_moves_the_index() {
        let original = image_f(None);
        let mut doc = Document::new(original.clone());
        let id = ImageId::from_raw(0);
        let palette = original.palette().unwrap();
        assert_eq!(transparent_button_label(palette, 2), SET);
        assert_eq!(transparent_toggle(palette, 2), Some(2));

        assert_eq!(press(&mut doc, 2), Ok(true));
        assert_eq!(mark(&doc), Some(2));
        assert_eq!(pixels_2(image_of(&doc, id)), vec![0, 1, 2, 3]);
        assert_eq!(colors_of(&doc, id), vec![RED, GREEN, BLUE, TEST_BLACK]);
        assert_eq!(resolved(&doc, 2), Rgba::new(0, 0, 255, 0));
        assert_eq!(resolved(&doc, 1), GREEN);
        assert_eq!(label(&doc, 2), REMOVE);
        assert_ne!(doc.state_id(), 0);

        // Un solo paso.
        doc.undo().unwrap();
        assert_eq!(mark(&doc), None);
        assert_eq!(image_of(&doc, id), &original);
        assert_eq!(doc.state_id(), 0);
        assert!(!doc.can_undo());
        assert_eq!(label(&doc, 2), SET);
        doc.redo().unwrap();
        assert_eq!(mark(&doc), Some(2));
        assert!(!doc.can_redo());
    }

    #[test]
    fn row_3_the_selection_and_the_set_notice() {
        let op = PaletteOp::SetTransparent { index: Some(2) };
        assert_eq!(choice_after(&op, 2, 4), 2);
        assert_eq!(entry_fate(&op, 2), Fate::Same);
        assert_eq!(
            ColorChoice::Entry(choice_after(&op, 2, 4)),
            ColorChoice::Entry(2)
        );
        assert_eq!(
            transparent_summary(Some(2), 2, None),
            "entrada 2 transparente"
        );
        assert_eq!(blocked_reason(PaletteButton::Transparent, 4, false), None);
    }

    #[test]
    fn row_3_remove_with_the_2_marked_leaves_the_stored_color() {
        let original = image_f(Some(2));
        let mut doc = Document::new(original.clone());
        let id = ImageId::from_raw(0);
        assert_eq!(label(&doc, 2), REMOVE);
        assert_eq!(
            transparent_toggle(original.palette().unwrap(), 2),
            None,
            "con la marcada activa el botón quita"
        );
        assert_eq!(resolved(&doc, 2), Rgba::new(0, 0, 255, 0));

        assert_eq!(press(&mut doc, 2), Ok(true));
        assert_eq!(mark(&doc), None);
        assert_eq!(colors_of(&doc, id), vec![RED, GREEN, BLUE, TEST_BLACK]);
        assert_eq!(pixels_2(image_of(&doc, id)), vec![0, 1, 2, 3]);
        assert_eq!(resolved(&doc, 2), BLUE, "vuelve a su alfa guardado");
        assert_eq!(label(&doc, 2), SET);
        assert_eq!(
            transparent_summary(None, 2, None),
            "entrada 2 ya no es transparente"
        );

        doc.undo().unwrap();
        assert_eq!(mark(&doc), Some(2));
        assert_eq!(image_of(&doc, id), &original);
        assert_eq!(label(&doc, 2), REMOVE, "el rótulo sale siempre del estado");
        doc.redo().unwrap();
        assert_eq!(mark(&doc), None);
        assert_eq!(label(&doc, 2), SET);
    }

    #[test]
    fn row_3_the_label_depends_only_on_whether_the_active_one_is_the_marked_one() {
        let palette = palette(&[RED, GREEN, BLUE, TEST_BLACK], Some(1));
        for active_2 in 0..4u8 {
            let expected_2 = if active_2 == 1 { REMOVE } else { SET };
            assert_eq!(
                transparent_button_label(&palette, active_2),
                expected_2,
                "{active_2}"
            );
            assert_eq!(
                transparent_toggle(&palette, active_2),
                if active_2 == 1 { None } else { Some(active_2) },
                "{active_2}"
            );
        }
        let without = self::palette(&[RED, GREEN, BLUE, TEST_BLACK], None);
        for active_2 in 0..4u8 {
            assert_eq!(transparent_button_label(&without, active_2), SET);
            assert_eq!(transparent_toggle(&without, active_2), Some(active_2));
        }
    }

    #[test]
    fn row_3_the_three_summaries() {
        assert_eq!(
            transparent_summary(Some(0), 0, None),
            "entrada 0 transparente"
        );
        assert_eq!(
            transparent_summary(Some(7), 7, None),
            "entrada 7 transparente"
        );
        assert_eq!(
            transparent_summary(Some(3), 3, Some(1)),
            "entrada 3 transparente (antes la 1)"
        );
        assert_eq!(
            transparent_summary(None, 3, Some(3)),
            "entrada 3 ya no es transparente"
        );
        assert_eq!(
            transparent_summary(None, 255, None),
            "entrada 255 ya no es transparente"
        );
    }

    // Fila 4: reemplazo.

    #[test]
    fn row_4_setting_the_3_with_the_1_marked_replaces_and_the_1_is_seen_again() {
        let original = image_f(Some(1));
        let mut doc = Document::new(original.clone());
        let id = ImageId::from_raw(0);
        assert_eq!(resolved(&doc, 1), Rgba::new(0, 255, 0, 0));
        assert_eq!(label(&doc, 3), SET);

        assert_eq!(press(&mut doc, 3), Ok(true));
        assert_eq!(mark(&doc), Some(3));
        assert_eq!(resolved(&doc, 1), Rgba::new(0, 255, 0, 255));
        assert_eq!(resolved(&doc, 3), Rgba::new(0, 0, 0, 0));
        assert_eq!(
            transparent_summary(Some(3), 3, Some(1)),
            "entrada 3 transparente (antes la 1)"
        );

        // Un solo paso y un solo `undo`.
        doc.undo().unwrap();
        assert_eq!(mark(&doc), Some(1));
        assert_eq!(image_of(&doc, id), &original);
        assert!(!doc.can_undo());
    }

    #[test]
    fn row_4_with_stored_alpha_0_in_the_previous_one_it_stays_with_alpha_0() {
        let green_alpha_0 = Rgba::new(0, 255, 0, 0);
        let image_2 = image_with(
            &[RED, green_alpha_0, BLUE, TEST_BLACK],
            Some(1),
            &[0, 1, 2, 3],
        );
        let mut doc = Document::new(image_2);
        assert_eq!(press(&mut doc, 3), Ok(true));
        assert_eq!(mark(&doc), Some(3));
        assert_eq!(resolved(&doc, 1), green_alpha_0);
        doc.undo().unwrap();
        assert_eq!(mark(&doc), Some(1));
    }

    #[test]
    fn row_4_the_notice_with_three_digits_is_39_characters_and_no_notice_passes_43() {
        let notice = transparent_summary(Some(255), 255, Some(254));
        assert_eq!(notice, "entrada 255 transparente (antes la 254)");
        assert_eq!(characters(&notice), 39);

        let mut more_long_2 = 0;
        let mut check = |text_2: String| {
            let n = characters(&text_2);
            more_long_2 = more_long_2.max(n);
            assert!(n <= 43, "{text_2:?}: {n} caracteres");
            assert!(in_the_set(&text_2), "{text_2:?} sale de INTERFACE_CHARSET");
            let complete = format!("{NOTICE_PREFIX}{text_2}");
            assert_eq!(
                fit_text(&complete, 50, |c| INTERFACE_CHARSET.contains(c)),
                complete,
                "fit_text recortó {complete:?}"
            );
        };
        for active_2 in 0..=255u8 {
            check(transparent_summary(None, active_2, None));
            check(transparent_summary(None, active_2, Some(active_2)));
            check(transparent_summary(Some(active_2), active_2, None));
            for preview in (0..=255u8).filter(|&p| p != active_2) {
                check(transparent_summary(Some(active_2), active_2, Some(preview)));
            }
        }
        assert_eq!(more_long_2, 39);
    }

    // Fila 6: borrador (ya existe; la regla no cambia).

    /// `[0 = alpha 0, G, B, K]` con píxeles `[0, 1, 2, 3]`: una entrada anterior de alfa 0.
    fn with_one_of_alpha_0() -> Document {
        Document::new(image_with(
            &[Rgba::new(5, 5, 5, 0), GREEN, BLUE, TEST_BLACK],
            None,
            &[0, 1, 2, 3],
        ))
    }

    #[test]
    fn row_6_with_set_at_j_the_eraser_uses_j_even_if_there_is_an_earlier_one_of_alpha_0() {
        let mut doc = with_one_of_alpha_0();
        let id = ImageId::from_raw(0);
        // Antes de fijar: la primera de alfa 0.
        assert_eq!(eraser_pixel(image_of(&doc, id)), Ok(Pixel::Indexed(0)));
        assert_eq!(eraser_label(image_of(&doc, id)), "transparente (entrada 0)");

        press(&mut doc, 2).unwrap();
        let image_2 = image_of(&doc, id);
        assert_eq!(eraser_pixel(image_2), Ok(Pixel::Indexed(2)));
        assert_eq!(eraser_label(image_2), "transparente");
        // Los píxeles `Indexed(2)` ya son transparentes: el borrador los omite.
        assert!(!is_erasable(image_2, 2, 0));
        assert!(is_erasable(image_2, 1, 0));
        assert!(is_erasable(image_2, 3, 0));
        assert!(!is_erasable(image_2, 0, 0), "alfa guardado 0");

        // `Remove`: vuelve a la primera de alfa 0 y los `Indexed(2)` (alfa guardado 255) cuentan.
        press(&mut doc, 2).unwrap();
        let image_2 = image_of(&doc, id);
        assert_eq!(eraser_pixel(image_2), Ok(Pixel::Indexed(0)));
        assert_eq!(eraser_label(image_2), "transparente (entrada 0)");
        assert!(is_erasable(image_2, 2, 0));
    }

    #[test]
    fn row_6_removing_without_any_alpha_0_entry_leaves_the_eraser_without_a_value() {
        let mut doc = Document::new(image_f(None));
        let id = ImageId::from_raw(0);
        assert_eq!(
            eraser_pixel(image_of(&doc, id)),
            Err(EraseError::NoTransparentIndex)
        );
        assert_eq!(eraser_label(image_of(&doc, id)), "sin índice transparente");

        press(&mut doc, 3).unwrap();
        assert_eq!(eraser_pixel(image_of(&doc, id)), Ok(Pixel::Indexed(3)));
        assert_eq!(eraser_label(image_of(&doc, id)), "transparente");
        assert!(!is_erasable(image_of(&doc, id), 3, 0));

        press(&mut doc, 3).unwrap();
        assert_eq!(
            eraser_pixel(image_of(&doc, id)),
            Err(EraseError::NoTransparentIndex)
        );
        assert_eq!(eraser_label(image_of(&doc, id)), "sin índice transparente");
        assert!(!is_erasable(image_of(&doc, id), 3, 0));
    }

    // Fila 8: la paleta inicial de «Nueva imagen» (entrada 0 marcada con #00000000).

    /// Réplica de la forma de `initial_palette()`: 32 entradas, la 0 transparente y marcada, la
    /// 25 negra.
    fn palette_initial() -> Palette {
        let mut colors_2 = vec![Rgba::TRANSPARENT];
        colors_2.extend((1..32u8).map(|i| Rgba::opaque(i * 8, 255 - i * 8, i)));
        colors_2[25] = TEST_BLACK;
        palette(&colors_2, Some(0))
    }

    #[test]
    fn row_8_removing_the_mark_of_entry_0_changes_no_visible_pixel() {
        let mut image_2 = Image::new_indexed(4, 1, palette_initial()).unwrap();
        image_2.set_pixel(3, 0, Pixel::Indexed(25)).unwrap();
        let mut doc = Document::new(image_2);
        let id = ImageId::from_raw(0);
        let before = visible_2(image_of(&doc, id));
        assert_eq!(label(&doc, 0), REMOVE);
        assert_eq!(label(&doc, 25), SET);
        assert_eq!(
            transparent_toggle(image_of(&doc, id).palette().unwrap(), 25),
            Some(25)
        );

        assert_eq!(press(&mut doc, 0), Ok(true));
        assert_eq!(mark(&doc), None);
        let palette = image_of(&doc, id).palette().unwrap();
        assert_eq!(palette.get(0).unwrap(), Rgba::new(0, 0, 0, 0));
        assert_eq!(visible_2(image_of(&doc, id)), before, "mismo aspecto");
        assert_eq!(label(&doc, 0), SET);
        // El borrador usa ahora la entrada 0 por la segunda rama.
        assert_eq!(eraser_pixel(image_of(&doc, id)), Ok(Pixel::Indexed(0)));
        assert_eq!(eraser_label(image_of(&doc, id)), "transparente (entrada 0)");

        doc.undo().unwrap();
        assert_eq!(mark(&doc), Some(0));
        assert_eq!(label(&doc, 0), REMOVE);
        assert_eq!(visible_2(image_of(&doc, id)), before);
    }

    // Fila 10: límites.

    #[test]
    fn row_10_with_a_single_entry_set_and_remove_work() {
        let mut doc = Document::new(image_with(&[RED], None, &[0, 0, 0]));
        let id = ImageId::from_raw(0);
        assert_eq!(label(&doc, 0), SET);
        assert_eq!(press(&mut doc, 0), Ok(true));
        assert_eq!(mark(&doc), Some(0));
        assert_eq!(label(&doc, 0), REMOVE);
        assert!(visible_2(image_of(&doc, id)).iter().all(|c| c.a == 0));
        assert_eq!(press(&mut doc, 0), Ok(true));
        assert_eq!(mark(&doc), None);
        assert!(visible_2(image_of(&doc, id)).iter().all(|c| *c == RED));
        assert_eq!(
            choice_after(&PaletteOp::SetTransparent { index: None }, 0, 1),
            0
        );
        assert_eq!(blocked_reason(PaletteButton::Transparent, 1, false), None);
    }

    fn palette_of_256() -> Vec<Rgba> {
        (0..=255u8).map(|i| Rgba::opaque(i, 255 - i, 9)).collect()
    }

    #[test]
    fn row_10_with_256_entries_set_at_255_and_remove_the_marked_at_0_or_at_the_last() {
        let colors_2 = palette_of_256();
        let mut doc = Document::new(image_with(&colors_2, None, &[0, 255]));
        assert_eq!(press(&mut doc, 255), Ok(true));
        assert_eq!(mark(&doc), Some(255));
        assert_eq!(label(&doc, 255), REMOVE);
        assert_eq!(label(&doc, 254), SET);
        assert_eq!(press(&mut doc, 255), Ok(true));
        assert_eq!(mark(&doc), None);

        for end in [0u8, 255] {
            let mut doc = Document::new(image_with(&colors_2, Some(end), &[0, 255]));
            assert_eq!(label(&doc, end), REMOVE, "marcada en {end}");
            assert_eq!(press(&mut doc, end), Ok(true));
            assert_eq!(mark(&doc), None);
            doc.undo().unwrap();
            assert_eq!(mark(&doc), Some(end));
        }
        // La marcada en 0 con la 255 activa: fijar reemplaza.
        let mut doc = Document::new(image_with(&colors_2, Some(0), &[0, 255]));
        assert_eq!(press(&mut doc, 255), Ok(true));
        assert_eq!(mark(&doc), Some(255));
        assert_eq!(
            transparent_summary(Some(255), 255, Some(0)),
            "entrada 255 transparente (antes la 0)"
        );
    }

    #[test]
    fn row_10_entry_fate_is_same_and_choice_after_is_the_active_one_in_the_whole_range() {
        for index in [None, Some(0u8), Some(255)] {
            let op = PaletteOp::SetTransparent { index };
            for input_2 in 0..=255u8 {
                assert_eq!(entry_fate(&op, input_2), Fate::Same, "{index:?}, {input_2}");
            }
            for length_2 in [1usize, 2, 4, 255, 256] {
                for active_2 in [0u8, 1, 3, 254, 255] {
                    assert_eq!(choice_after(&op, active_2, length_2), active_2, "{index:?}");
                }
            }
        }
        for length_2 in 0..=257usize {
            for target_2 in [false, true] {
                assert_eq!(
                    blocked_reason(PaletteButton::Transparent, length_2, target_2),
                    None,
                    "len {length_2}"
                );
            }
        }
    }

    #[test]
    fn row_10_the_range_and_the_selection_are_kept_and_a_pending_removal_is_cancelled() {
        let mut doc = Document::new(image_f(None));
        let mut ops = PaletteOps::default();
        ops.sync(doc.state_id(), 4);
        ops.set_range_end(3);
        ops.begin_removal(2, Some(0));
        assert!(ops.removal().is_some());

        press(&mut doc, 1).unwrap();
        let op = PaletteOp::SetTransparent { index: Some(1) };
        // Selección intacta: la misma activa.
        assert_eq!(
            ColorChoice::Entry(choice_after(&op, 1, 4)),
            ColorChoice::Entry(1)
        );
        // El siguiente fotograma cancela la quita (el documento cambió) pero conserva el rango.
        assert!(ops.sync(doc.state_id(), 4), "la quita se cancela");
        assert!(ops.removal().is_none());
        assert_eq!(ops.range_end(), Some(3));
        assert!(!ops.sync(doc.state_id(), 4), "una sola vez");
        assert_eq!(ops.range_end(), Some(3));
    }

    #[test]
    fn row_10_undo_and_redo_do_not_move_the_selection_and_the_chosen_one_past_the_end_is_clipped() {
        let mut doc = Document::new(image_f(None));
        let id = ImageId::from_raw(0);
        press(&mut doc, 2).unwrap();
        for step in 0..2 {
            for chosen_2 in [0u8, 2, 3, 7, 200] {
                let image_2 = image_of(&doc, id);
                let expected = ColorChoice::Entry(chosen_2.min(3));
                assert_eq!(
                    clamp_choice(image_2, ColorChoice::Entry(chosen_2)),
                    expected,
                    "paso {step}, elegida {chosen_2}"
                );
            }
            if step == 0 {
                doc.undo().unwrap();
            }
        }
        doc.redo().unwrap();
        assert_eq!(mark(&doc), Some(2));
        assert_eq!(label(&doc, 2), REMOVE);
    }

    // Propiedad 1: conmutador exacto.

    fn channel() -> impl Strategy<Value = u8> {
        prop_oneof![Just(0u8), Just(255u8), any::<u8>()]
    }

    /// Color visible de la entrada `i` según el oráculo: el guardado, con alfa 0 en la marcada.
    fn visible_model(colors_2: &[Rgba], mark: Option<u8>, i: u8) -> Rgba {
        let saved = colors_2[usize::from(i)];
        if mark == Some(i) {
            Rgba { a: 0, ..saved }
        } else {
            saved
        }
    }

    proptest! {
        #[test]
        fn property_1_the_button_toggles_the_mark_and_touches_nothing_else(
            colors_2 in vec((channel(), channel(), channel(), channel()), 1..=256),
            mark_raw in any::<usize>(),
            active_raw in any::<usize>(),
            raw_2_2 in vec(any::<u8>(), 1..=24),
        ) {
            let colors_2: Vec<Rgba> =
                colors_2.into_iter().map(|(r, g, b, a)| Rgba::new(r, g, b, a)).collect();
            let length_2 = colors_2.len();
            let mark_initial = {
                let m = mark_raw % (length_2 + 1);
                (m < length_2).then_some(m as u8)
            };
            let active_2 = (active_raw % length_2) as u8;
            let indices: Vec<u8> = raw_2_2.iter().map(|&c| (usize::from(c) % length_2) as u8).collect();

            // Oráculo propio: rótulo e índice resultante.
            let is_the_marked = mark_initial == Some(active_2);
            let label_expected = if is_the_marked { REMOVE } else { SET };
            let mark_expected = if is_the_marked { None } else { Some(active_2) };

            let original = image_with(&colors_2, mark_initial, &indices);
            let palette_original = original.palette().unwrap().clone();
            prop_assert_eq!(transparent_button_label(&palette_original, active_2), label_expected);
            prop_assert_eq!(transparent_toggle(&palette_original, active_2), mark_expected);

            let mut doc = Document::new(original.clone());
            let id = ImageId::from_raw(0);
            prop_assert_eq!(press(&mut doc, active_2), Ok(true));
            let after_2 = image_of(&doc, id).clone();

            // Solo cambia el índice: bytes de píxeles y colores guardados iguales.
            prop_assert_eq!(after_2.palette().unwrap().transparent_index(), mark_expected);
            prop_assert_eq!(after_2.bytes(), original.bytes());
            prop_assert_eq!(after_2.palette().unwrap().colors(), &colors_2[..]);

            // El color resuelto cambia exactamente en los píxeles de la activa y de la marca
            // anterior (si el alfa guardado no era ya 0), y esa vuelve a su guardado.
            let visible_before = visible_2(&original);
            let visible_after = visible_2(&after_2);
            for (x, &i) in indices.iter().enumerate() {
                let affected = i == active_2 || mark_initial == Some(i);
                let changes = affected && colors_2[usize::from(i)].a != 0;
                prop_assert_eq!(
                    visible_before[x] != visible_after[x],
                    changes,
                    "píxel {} (entrada {})", x, i
                );
                prop_assert_eq!(visible_before[x], visible_model(&colors_2, mark_initial, i));
                prop_assert_eq!(visible_after[x], visible_model(&colors_2, mark_expected, i));
                if mark_initial == Some(i) && i != active_2 {
                    prop_assert_eq!(visible_after[x], colors_2[usize::from(i)]);
                }
            }

            // Undo y redo exactos.
            doc.undo().unwrap();
            prop_assert_eq!(image_of(&doc, id), &original);
            prop_assert!(!doc.can_undo());
            doc.redo().unwrap();
            prop_assert_eq!(image_of(&doc, id), &after_2);

            // Una segunda pulsación con la misma activa: el rótulo y el índice salen del estado.
            let second_removes = mark_expected == Some(active_2);
            prop_assert_eq!(label(&doc, active_2), if second_removes { REMOVE } else { SET });
            prop_assert_eq!(press(&mut doc, active_2), Ok(true));
            let second_expected = if second_removes { None } else { Some(active_2) };
            prop_assert_eq!(mark(&doc), second_expected);
            // Vuelve a la original si no había otra marca distinta de la activa.
            if mark_initial.is_none() || mark_initial == Some(active_2) {
                prop_assert_eq!(image_of(&doc, id), &original);
            } else {
                prop_assert_eq!(mark(&doc), None);
                prop_assert_eq!(image_of(&doc, id).bytes(), original.bytes());
                prop_assert_eq!(image_of(&doc, id).palette().unwrap().colors(), &colors_2[..]);
            }
        }
    }
}

// Unidad 21: `summary` con la rama `SetTransparent` (delega en `transparent_summary(index, active,
// None)`; la signature no changes).

mod summary_transparent {
    use super::*;
    use px_ui::palette_ops::transparent_summary;

    fn summary_2(index: Option<u8>, active_2: u8, length_2: usize) -> String {
        summary(
            &PaletteOp::SetTransparent { index },
            active_2,
            length_2,
            false,
            false,
        )
    }

    #[test]
    fn summary_of_set_transparent_gives_the_two_texts_of_the_spec() {
        assert_eq!(summary_2(Some(2), 2, 4), "entrada 2 transparente");
        assert_eq!(summary_2(Some(0), 0, 1), "entrada 0 transparente");
        assert_eq!(summary_2(Some(255), 255, 256), "entrada 255 transparente");
        assert_eq!(summary_2(None, 2, 4), "entrada 2 ya no es transparente");
        assert_eq!(
            summary_2(None, 255, 256),
            "entrada 255 ya no es transparente"
        );
    }

    #[test]
    fn summary_delegates_to_transparent_summary_without_a_previous_one() {
        for active_2 in 0..=255u8 {
            for index in [None, Some(active_2)] {
                assert_eq!(
                    summary_2(index, active_2, 256),
                    transparent_summary(index, active_2, None),
                    "{index:?}, activa {active_2}"
                );
            }
        }
        // Los demás argumentos no cuentan.
        for (was_transparent, without_pixels) in [(false, true), (true, false), (true, true)] {
            assert_eq!(
                summary(
                    &PaletteOp::SetTransparent { index: Some(3) },
                    3,
                    4,
                    was_transparent,
                    without_pixels
                ),
                "entrada 3 transparente"
            );
        }
    }

    #[test]
    fn transparent_summary_with_a_different_previous_one() {
        assert_eq!(
            transparent_summary(Some(3), 3, Some(1)),
            "entrada 3 transparente (antes la 1)"
        );
        assert_eq!(
            transparent_summary(Some(255), 255, Some(254)),
            "entrada 255 transparente (antes la 254)"
        );
        assert_eq!(
            transparent_summary(Some(0), 0, Some(255)),
            "entrada 0 transparente (antes la 255)"
        );
    }

    #[test]
    fn the_summaries_with_three_digits_are_39_and_33_characters_and_fit_in_43() {
        let with_preview = transparent_summary(Some(255), 255, Some(254));
        let remove = summary_2(None, 255, 256);
        assert_eq!(characters(&with_preview), 39);
        assert_eq!(characters(&remove), 33);
        for text_2 in [&with_preview, &remove] {
            assert!(characters(text_2) <= 43);
            assert!(in_the_set(text_2), "{text_2:?}");
            let complete = format!("{NOTICE_PREFIX}{text_2}");
            assert!(characters(&complete) <= 50);
            assert_eq!(
                fit_text(&complete, 50, |c| INTERFACE_CHARSET.contains(c)),
                complete
            );
        }
    }
}
