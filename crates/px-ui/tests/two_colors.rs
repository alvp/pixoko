//! Unidad I.2a, parte pura (specs/interfaz-dos-colores.md): fila 1 (fondo por defecto), fila 3
//! (bloqueos del color con un trazo), fila 5 (banderas y anillos), fila 6 (geometría del
//! indicador), fila 7 (teclas y menú) y la propiedad 3 (geometría de una sola
//! fuente). Sin ventana.

use egui::Color32;
use px_core::{Image, Mode, Palette, Pixel, Rgba};
use px_ui::actions::{AppAction, Locks, Origin, Platform, is_blocked};
use px_ui::color::{
    ColorChoice, RoleFlags, default_background, info_flags_roles, pixel_for_choice,
};
use px_ui::edit_layout::{block_height_px, min_window_height_pt, min_window_width_pt};
use px_ui::keymap::{Binding, Conflict, KeyId, Keymap, Platforms, Press};
use px_ui::menu::{MENUS, popup_width_u, shortcut_for, shortcut_text};
use px_ui::palette_grid::{CELL_U, GAP_U, corner_rects, ring_rects};
use px_ui::text::text_scale;
use px_ui::theme::{RING_ACTIVE, RING_BACKGROUND, SURFACE_1, contrast_ratio};
use px_ui::view::ScreenRect;

const PPPS: [f32; 5] = [1.0, 1.25, 1.5, 2.0, 3.0];

// ---------------------------------------------------------------------------------------------
// Fila 1: fondo por defecto
// ---------------------------------------------------------------------------------------------

fn indexed(colors_2: Vec<Rgba>, transparent: Option<u8>) -> Image {
    let mut palette = Palette::from_colors(colors_2).expect("paleta");
    palette.set_transparent_index(transparent).expect("existe");
    Image::new_indexed(4, 4, palette).expect("imagen")
}

fn opaque(n: usize) -> Vec<Rgba> {
    (0..n).map(|i| Rgba::opaque(i as u8 * 10, 1, 2)).collect()
}

#[test]
fn row_1_rgba_gray_alpha_and_gray_use_slot_9() {
    for mode in [Mode::Rgba, Mode::GrayAlpha, Mode::Gray] {
        let image = Image::new(4, 4, mode).expect("imagen");
        assert_eq!(default_background(&image), ColorChoice::Slot(9), "{mode:?}");
    }
}

#[test]
fn row_1_slot_9_is_transparent_in_rgba_and_gray_alpha_and_white_in_gray() {
    let rgba = Image::new(4, 4, Mode::Rgba).expect("imagen");
    assert_eq!(
        pixel_for_choice(&rgba, default_background(&rgba)),
        Some(Pixel::Rgba(Rgba::TRANSPARENT))
    );
    let ga = Image::new(4, 4, Mode::GrayAlpha).expect("imagen");
    assert_eq!(
        pixel_for_choice(&ga, default_background(&ga)),
        Some(Pixel::GrayAlpha { gray: 0, alpha: 0 })
    );
    let gray = Image::new(4, 4, Mode::Gray).expect("imagen");
    assert_eq!(
        pixel_for_choice(&gray, default_background(&gray)),
        Some(Pixel::Gray(255))
    );
}

#[test]
fn row_1_indexed_with_transparent_index_uses_that_entry() {
    let mut colors_2 = opaque(5);
    colors_2[2] = Rgba::new(0, 255, 0, 0);
    let image = indexed(colors_2, Some(3));
    assert_eq!(default_background(&image), ColorChoice::Entry(3));
}

#[test]
fn row_1_indexed_without_an_index_uses_the_first_alpha_0_entry() {
    let mut colors_2 = opaque(5);
    colors_2[2] = Rgba::new(0, 255, 0, 0);
    colors_2[4] = Rgba::new(9, 9, 9, 0);
    assert_eq!(
        default_background(&indexed(colors_2.clone(), None)),
        ColorChoice::Entry(2)
    );
    colors_2[2] = Rgba::opaque(0, 255, 0);
    assert_eq!(
        default_background(&indexed(colors_2, None)),
        ColorChoice::Entry(4)
    );
}

#[test]
fn row_1_indexed_without_any_and_palette_of_one_entry_use_entry_0() {
    assert_eq!(
        default_background(&indexed(opaque(5), None)),
        ColorChoice::Entry(0)
    );
    assert_eq!(
        default_background(&indexed(opaque(1), None)),
        ColorChoice::Entry(0)
    );
}

// ---------------------------------------------------------------------------------------------
// Fila 3 (parte pura): bloqueos del color con un trazo
// ---------------------------------------------------------------------------------------------

const SOURCES: [Origin; 3] = [Origin::Menu, Origin::Button, Origin::Key];

const ACTIONS_OF_COLOR: [AppAction; 5] = [
    AppAction::ColorNext,
    AppAction::ColorPrev,
    AppAction::BackgroundNext,
    AppAction::BackgroundPrev,
    AppAction::SelectColor(3),
];

fn only(f: impl FnOnce(&mut Locks)) -> Locks {
    let mut locks = Locks::default();
    f(&mut locks);
    locks
}

fn with_stroke() -> Locks {
    Locks {
        gesture: true,
        stroke: true,
        ..Locks::default()
    }
}

#[test]
fn row_3_the_color_actions_with_a_stroke_in_progress_are_still_allowed() {
    for action in ACTIONS_OF_COLOR {
        for origin in SOURCES {
            assert!(
                !is_blocked(action, with_stroke(), origin),
                "{action:?} con {origin:?}"
            );
        }
    }
}

#[test]
fn row_3_color_with_the_slider_or_a_swatch_being_dragged_stays_blocked() {
    // `gesture` sin `stroke`: el deslizador del editor o el arrastre de una muestra.
    for action in ACTIONS_OF_COLOR {
        for origin in SOURCES {
            assert!(is_blocked(action, only(|l| l.gesture = true), origin));
        }
    }
}

#[test]
fn row_3_color_is_blocked_with_a_modal_dialog_field_and_pending_removal() {
    for action in ACTIONS_OF_COLOR {
        for origin in SOURCES {
            for locks in [
                only(|l| l.dialog = true),
                only(|l| l.modal = true),
                only(|l| l.text_focus = true),
                only(|l| l.removal = true),
                Locks {
                    removal: true,
                    ..with_stroke()
                },
            ] {
                assert!(is_blocked(action, locks, origin), "{action:?} {locks:?}");
            }
        }
    }
}

#[test]
fn row_3_the_rest_of_the_actions_stays_blocked_with_a_stroke() {
    for action in [
        AppAction::Undo,
        AppAction::Redo,
        AppAction::Open,
        AppAction::NewImage,
        AppAction::ClearPage,
        AppAction::SelectTool(px_ui::tool::Tool::Line),
    ] {
        for origin in SOURCES {
            assert!(is_blocked(action, with_stroke(), origin), "{action:?}");
        }
    }
}

#[test]
fn row_8_clear_page_is_blocked_with_gesture_dialog_modal_and_field_but_not_with_removal() {
    for origin in SOURCES {
        for locks in [
            only(|l| l.gesture = true),
            with_stroke(),
            only(|l| l.dialog = true),
            only(|l| l.modal = true),
            only(|l| l.text_focus = true),
        ] {
            assert!(is_blocked(AppAction::ClearPage, locks, origin), "{locks:?}");
        }
        assert!(!is_blocked(
            AppAction::ClearPage,
            only(|l| l.removal = true),
            origin
        ));
        assert!(!is_blocked(AppAction::ClearPage, Locks::default(), origin));
    }
}

// ---------------------------------------------------------------------------------------------
// Fila 5: banderas y anillos
// ---------------------------------------------------------------------------------------------

fn flags_2(f: impl FnOnce(&mut RoleFlags)) -> RoleFlags {
    let mut flags = RoleFlags::default();
    f(&mut flags);
    flags
}

#[test]
fn row_5_flags_of_each_role_and_their_combinations() {
    let cases = [
        (RoleFlags::default(), ""),
        (flags_2(|f| f.foreground = true), "frente"),
        (flags_2(|f| f.background = true), "fondo"),
        (
            flags_2(|f| {
                f.foreground = true;
                f.background = true;
            }),
            "frente · fondo",
        ),
        (
            flags_2(|f| {
                f.hover = true;
                f.foreground = true;
                f.background = true;
            }),
            "cursor · frente · fondo",
        ),
        (
            flags_2(|f| {
                f.background = true;
                f.transparent = true;
            }),
            "fondo · transparente",
        ),
        (flags_2(|f| f.alpha0 = true), "alfa 0"),
        (
            flags_2(|f| {
                f.alpha0 = true;
                f.transparent = true;
            }),
            "transparente",
        ),
        (flags_2(|f| f.in_range = true), "rango"),
        (
            flags_2(|f| {
                f.in_range = true;
                f.foreground = true;
            }),
            "frente",
        ),
    ];
    for (flags, expected_2) in cases {
        assert_eq!(info_flags_roles(flags, 32), expected_2, "{flags:?}");
    }
}

#[test]
fn row_5_if_it_does_not_fit_in_32_columns_cursor_is_omitted() {
    let worst = flags_2(|f| {
        f.hover = true;
        f.foreground = true;
        f.background = true;
        f.transparent = true;
    });
    assert_eq!(
        info_flags_roles(worst, 32),
        "frente · fondo · transparente",
        "sin cursor"
    );
    assert_eq!(
        info_flags_roles(worst, 40),
        "cursor · frente · fondo · transparente",
        "con sitio, cursor se queda"
    );
    for columns in 29..=64 {
        assert!(info_flags_roles(worst, columns).chars().count() <= columns.max(29));
    }
}

#[test]
fn row_5_the_background_ring_contrasts_3_to_1_and_is_not_the_foreground_one() {
    assert!(contrast_ratio(RING_BACKGROUND, SURFACE_1) >= 3.0);
    assert_ne!(RING_BACKGROUND, RING_ACTIVE);
    let _: Color32 = RING_BACKGROUND;
}

fn cel(n: i64) -> ScreenRect {
    ScreenRect {
        x: 0,
        y: 0,
        width: CELL_U * n,
        height: CELL_U * n,
    }
}

fn overlap_2(a: &ScreenRect, b: &ScreenRect) -> bool {
    a.x < b.x + b.width && b.x < a.x + a.width && a.y < b.y + b.height && b.y < a.y + a.height
}

#[test]
fn row_5_the_background_ring_is_8_disjoint_rectangles_in_4_corners_outside_the_swatch() {
    for n in 1..=3 {
        let cell = cel(n);
        let rects = corner_rects(cell, 0, 2 * n, 5 * n);
        assert_eq!(rects.len(), 8);
        for (i, a) in rects.iter().enumerate() {
            assert!(!overlap_2(a, &cell), "{a:?} tapa la muestra");
            for b in rects.iter().skip(i + 1) {
                assert!(!overlap_2(a, b), "{a:?} y {b:?}");
            }
            // Dentro del hueco de 4 u.
            assert!(a.x >= -GAP_U * n && a.x + a.width <= (CELL_U + GAP_U) * n);
            assert!(a.y >= -GAP_U * n && a.y + a.height <= (CELL_U + GAP_U) * n);
        }
        // Cada esquina de la muestra tiene su L (dos rectángulos); no hay anillo en el medio de
        // los lados (forma distinta a la del primer plano).
        let middle = ScreenRect {
            x: 4 * n,
            y: -2 * n,
            width: 2 * n,
            height: 2 * n,
        };
        assert!(rects.iter().all(|r| !overlap_2(r, &middle)));
    }
}

#[test]
fn row_5_with_the_same_swatch_the_foreground_one_goes_inside_and_the_background_one_outside_without_overlapping()
 {
    for n in 1..=3 {
        let cell = cel(n);
        let inside = ring_rects(cell, 2 * n);
        let outside = corner_rects(cell, 2 * n, 2 * n, 5 * n);
        for a in &inside {
            for b in &outside {
                assert!(!overlap_2(a, b), "{a:?} y {b:?}");
            }
        }
        for b in &outside {
            assert!(b.x >= -4 * n && b.x + b.width <= (CELL_U + 4) * n);
            assert!(!overlap_2(b, &cell));
        }
    }
}

// ---------------------------------------------------------------------------------------------
// Fila 7: teclas y menú
// ---------------------------------------------------------------------------------------------

fn presses_2(key: KeyId) -> Press {
    Press {
        key: Some(key),
        command: false,
        shift: false,
        alt: false,
        other_ctrl: false,
        repeat: false,
    }
}

const PLATFORMS: [Platform; 3] = [Platform::Linux, Platform::Windows, Platform::Mac];

fn action_2(press: &Press, platform: Platform) -> Option<AppAction> {
    Keymap::default_scheme().action_for(press, platform)
}

#[test]
fn row_7_background_braces_and_brackets_with_any_shift_or_alt() {
    for platform in PLATFORMS {
        for (c, expected) in [
            ('}', AppAction::BackgroundNext),
            ('{', AppAction::BackgroundPrev),
        ] {
            for shift in [false, true] {
                for alt in [false, true] {
                    let press = Press {
                        shift,
                        alt,
                        ..presses_2(KeyId::Char(c))
                    };
                    assert_eq!(
                        action_2(&press, platform),
                        Some(expected),
                        "{c} {shift} {alt}"
                    );
                }
            }
            let with_ctrl = Press {
                command: true,
                ..presses_2(KeyId::Char(c))
            };
            assert_eq!(action_2(&with_ctrl, platform), None);
        }
    }
}

#[test]
fn row_7_capital_k_clears_the_page_k_without_shift_chooses_the_eraser_and_with_ctrl_or_alt_does_nothing()
 {
    for platform in PLATFORMS {
        let k = KeyId::Char('k');
        let with_shift = Press {
            shift: true,
            ..presses_2(k)
        };
        assert_eq!(action_2(&with_shift, platform), Some(AppAction::ClearPage));
        assert_eq!(
            action_2(&presses_2(k), platform),
            Some(AppAction::SelectTool(px_ui::tool::Tool::Eraser))
        );
        for press in [
            Press {
                command: true,
                shift: true,
                ..presses_2(k)
            },
            Press {
                command: true,
                ..presses_2(k)
            },
            Press {
                alt: true,
                shift: true,
                ..presses_2(k)
            },
            Press {
                other_ctrl: true,
                shift: true,
                ..presses_2(k)
            },
        ] {
            assert_eq!(action_2(&press, platform), None, "{press:?}");
        }
    }
}

#[test]
fn row_7_shift_page_up_and_page_down_are_the_background_and_without_shift_they_are_still_the_foreground()
 {
    for platform in PLATFORMS {
        let with = |key, shift, alt| Press {
            shift,
            alt,
            ..presses_2(key)
        };
        assert_eq!(
            action_2(&with(KeyId::PageDown, true, false), platform),
            Some(AppAction::BackgroundNext)
        );
        assert_eq!(
            action_2(&with(KeyId::PageUp, true, true), platform),
            Some(AppAction::BackgroundPrev)
        );
        assert_eq!(
            action_2(&with(KeyId::PageDown, false, false), platform),
            Some(AppAction::ColorNext)
        );
        assert_eq!(
            action_2(&with(KeyId::PageUp, false, true), platform),
            Some(AppAction::ColorPrev)
        );
        // `Start` no cambia con `Mayús`.
        for shift in [false, true] {
            assert_eq!(
                action_2(&with(KeyId::Home, shift, false), platform),
                Some(AppAction::ZoomFit)
            );
        }
    }
}

#[test]
fn row_7_the_table_has_38_rows_without_clashes_and_only_the_nine_modern_ones_carry_command() {
    let keymap = Keymap::default_scheme();
    assert_eq!(keymap.rows().len(), 43);
    assert_eq!(keymap.conflicts(), Vec::<Conflict>::new());
    let with_command = keymap.rows().iter().filter(|row| row.command).count();
    assert_eq!(with_command, 9);
}

#[test]
fn row_7_two_page_up_rows_with_different_shift_do_not_clash_but_equal_ones_do() {
    let row_2 = |shift, action| Binding {
        key: KeyId::PageUp,
        command: false,
        shift,
        action,
        active: Platforms::All,
        shown: Platforms::Never,
    };
    let well = Keymap::new(vec![
        row_2(false, AppAction::ColorPrev),
        row_2(true, AppAction::BackgroundPrev),
    ]);
    assert_eq!(well.conflicts(), Vec::<Conflict>::new());
    let badly = Keymap::new(vec![
        row_2(true, AppAction::ColorPrev),
        row_2(true, AppAction::BackgroundPrev),
    ]);
    assert_eq!(badly.conflicts(), vec![Conflict::Duplicate(0, 1)]);
}

#[test]
fn row_7_edit_wins_clear_page_and_shows_k_read_from_the_table() {
    let edit = MENUS
        .iter()
        .find(|menu| menu.title == "Editar")
        .expect("menú Editar");
    let input_2 = edit
        .items
        .iter()
        .find(|item| item.action == AppAction::ClearPage)
        .expect("Borrar página");
    assert_eq!(input_2.label, "Borrar página");
    for platform in PLATFORMS {
        let shortcut_2 = shortcut_for(AppAction::ClearPage, platform).expect("tiene atajo");
        assert_eq!(shortcut_text(shortcut_2, platform), "K");
        // El desplegable cabe: el rótulo más largo más su atajo.
        let width_2 = popup_width_u(edit, platform);
        let more_long_2 = edit
            .items
            .iter()
            .map(|item| item.label.chars().count())
            .max()
            .unwrap_or(0) as i64;
        assert!(width_2 >= (more_long_2 + 2 + 1) * i64::from(px_ui::status::ADVANCE_PX));
    }
    // Deshacer y Rehacer siguen donde estaban.
    assert_eq!(edit.items[0].action, AppAction::Undo);
    assert_eq!(edit.items[1].action, AppAction::Redo);
}

// ---------------------------------------------------------------------------------------------
// Propiedad 3: geometría de una sola fuente
// ---------------------------------------------------------------------------------------------

fn ppp_multiples_of_0_05() -> impl Iterator<Item = f32> {
    (0..=60).map(|i| (100 + 5 * i) as f32 / 100.0)
}

#[test]
fn property_3_window_minimums_at_the_five_ppp_and_the_60_percent_criterion() {
    for (ppp, height_2) in PPPS.into_iter().zip([313.0, 250.4, 417.33, 313.0, 313.0]) {
        assert!(
            (min_window_height_pt(ppp) - height_2).abs() < 0.01,
            "alto con {ppp}: {}",
            min_window_height_pt(ppp)
        );
    }
    for (ppp, width_2) in PPPS.into_iter().zip([468.0, 374.4, 624.0, 468.0, 468.0]) {
        assert!(
            (min_window_width_pt(ppp) - width_2).abs() < 0.01,
            "ancho con {ppp}"
        );
    }
}

#[test]
fn property_3_the_indexed_block_measures_308_n_and_the_minimum_height_follows_the_formula() {
    for ppp in ppp_multiples_of_0_05() {
        let n = i64::from(text_scale(ppp));
        assert_eq!(block_height_px(Mode::Indexed, n as u32), 308 * n);
        let height_2 = ((20 + 16 + 277) * n) as f32 / ppp;
        assert!(
            (min_window_height_pt(ppp) - height_2).abs() < 0.01,
            "ppp {ppp}"
        );
        assert!(min_window_height_pt(ppp) <= 432.0 + 0.01, "alto con {ppp}");
        assert!(min_window_width_pt(ppp) <= 683.0, "ancho con {ppp}");
    }
}
