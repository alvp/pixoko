//! Tests del menú «Fondo del lienzo» y de los bloqueos de su acción (specs/hito-2-damero-lienzo.md,
//! «Casos» fila 12 y, de la fila 13, la parte de `is_blocked` y `enabled`; el despacho de la fila 13
//! está en `px-app/src/dispatch/tests.rs`). Es lógica pura: la tabla `MENUS`, `Menu::submenus`,
//! `background_checked`, `shortcut_for`, `action_for_shortcut`, `is_blocked` y `enabled` no usan
//! ventana; solo los rótulos con la fuente real y la comprobación de que `show_menu_with_background`
//! se pinta usan un `egui::Context` con `RawInput` sintético (sin GPU ni capturas).
//!
//! - Fila 12: `Ver` conserva sus 4 entradas y gana **un** submenú `Fondo del canvas_2` con `Damero`,
//!   `Negro`, `Gris` y `Blanco` en ese orden; el resto de menús sin submenús; `Menu::items` no
//!   cambia (las 11 entradas de la 17); `background_checked` cierto solo para la acción del fondo
//!   activo; ningún atajo en ninguna plataforma; rótulos dentro de `INTERFACE_CHARSET`, sin `?`,
//!   que `printable` deja igual y sin repetir los de `MENUS`.
//! - Fila 13 (parte pura): `AppAction::CanvasBackground` se bloquea **solo** con `dialog`, con `modal`
//!   y con `text_focus` (desde cualquier origen) y no con `gesture` ni con `removal`; no es zoom ni
//!   repite; `enabled` es lo contrario de `is_blocked` con `Origin::Menu`; las acciones anteriores
//!   se bloquean igual que antes (con el modelo de `actions.rs`).
//! - `show_menu` conserva su firma y es `show_menu_with_background` con el fondo por defecto.
//!
//! Unidad 19b (specs/hito-2-fondo-color.md), al final del fichero: la fila 2 (`Color…` como quinta
//! entrada, la marca siempre en una sola entrada con `Custom`, sin atajo, `submenu_width_u` = 131 u
//! con cualquier fondo activo y a las cinco escalas, el valor `#rrggbb` pintado solo con `Custom` y
//! con contraste) y la parte pura de la fila 3 (`BackgroundColor` es `opens_dialog`: se bloquea con
//! los cinco bloqueos; las cuatro fijas siguen sin bloquearse con `gesture` ni `removal`).
//!
//! Suposiciones sobre las firmas de «Piezas»: `Submenu` es `Clone, Copy, Debug, PartialEq, Eq` (como
//! `Menu`, que lo contiene) y `AppAction` sigue siendo `Clone, Copy, Debug, PartialEq, Eq`. La marca
//! de la entrada activa dentro del desplegable no se prueba aquí (es de la prueba manual: depende de
//! cómo se dibuje); sí `background_checked`, que decide dónde va.

use egui::epaint::{ClippedShape, Shape};
use egui::{Color32, Context, Event, Key, Pos2, RawInput, Rect, Ui, Vec2, vec2};
use px_core::Rgba;
use px_ui::actions::{
    AppAction, Locks, Origin, Platform, UiState, action_for_shortcut, can_open_dialog, enabled,
    is_blocked,
};
use px_ui::background::{CanvasBackground, background_value_text};
use px_ui::canvas::BACKGROUND;
use px_ui::help::HelpTarget;
use px_ui::menu::{
    MENUS, Menu, MenuItem, Submenu, background_checked, shortcut_for, shortcut_text, show_menu,
    show_menu_with_background, submenu_width_u,
};
use px_ui::text::{INTERFACE_CHARSET, apply_scale, printable, replace_missing, ui_unit};
use px_ui::theme::{contrast_ratio, visuals};
use px_ui::tool::Tool;
use px_ui::view::Zoom;

mod common;
use common::{
    PPPS, button_2, context, frame, modifiers_2, move_pointer, painted, tolerance_of_egui,
};

const BACKGROUNDS: [CanvasBackground; 4] = [
    CanvasBackground::Checker,
    CanvasBackground::Black,
    CanvasBackground::Gray,
    CanvasBackground::White,
];
const PLATFORMS: [Platform; 3] = [Platform::Linux, Platform::Windows, Platform::Mac];
const SOURCES: [Origin; 3] = [Origin::Menu, Origin::Button, Origin::Key];
const TEST_TOOLS: [Tool; 5] = [
    Tool::Pencil,
    Tool::Line,
    Tool::Fill,
    Tool::Dropper,
    Tool::Eraser,
];

/// Las 11 entradas de `Menu::items`, en el orden del menú (las de la unidad 17).
const TABLE_ACTIONS: [AppAction; 11] = [
    AppAction::Open,
    AppAction::Save,
    AppAction::SaveAs,
    AppAction::Quit,
    AppAction::Undo,
    AppAction::Redo,
    AppAction::ZoomIn,
    AppAction::ZoomOut,
    AppAction::ZoomFit,
    AppAction::ZoomActual,
    AppAction::ShowLicenses,
];

fn action_2(b: CanvasBackground) -> AppAction {
    AppAction::CanvasBackground(b)
}

fn the_four() -> [AppAction; 4] {
    BACKGROUNDS.map(action_2)
}

/// Las acciones que ya existían: las 11 de la tabla, `CloseLicenses`, las cinco herramientas y las
/// nueve ranuras de color.
fn actions_previous() -> Vec<AppAction> {
    let mut v = TABLE_ACTIONS.to_vec();
    v.push(AppAction::CloseLicenses);
    v.extend(TEST_TOOLS.map(AppAction::SelectTool));
    v.extend((1..=9).map(AppAction::SelectColor));
    v
}

fn all_the_actions() -> Vec<AppAction> {
    let mut v = actions_previous();
    v.extend(the_four());
    v
}

fn el_menu(title: &str) -> &'static Menu {
    MENUS
        .iter()
        .find(|m| m.title == title)
        .unwrap_or_else(|| panic!("no hay menú {title:?}"))
}

fn labels_and_actions(items: &[MenuItem]) -> Vec<(&'static str, AppAction)> {
    items.iter().map(|i| (i.label, i.action)).collect()
}

fn locks_2(bits: u8) -> Locks {
    Locks {
        gesture: bits & 1 != 0,
        stroke: false,
        dialog: bits & 2 != 0,
        text_focus: bits & 4 != 0,
        removal: bits & 8 != 0,
        modal: bits & 16 != 0,
    }
}

fn assert_plane<T: Clone + Copy + std::fmt::Debug + PartialEq + Eq>() {}

// ---------------------------------------------------------------------------------------------
// Fila 12: la tabla
// ---------------------------------------------------------------------------------------------

#[test]
fn row_12_the_types_are_plain() {
    assert_plane::<AppAction>();
    assert_plane::<CanvasBackground>();
    assert_plane::<Menu>();
    assert_plane::<Submenu>();
    assert_plane::<MenuItem>();
}

#[test]
fn row_12_view_keeps_its_4_entries() {
    let see = el_menu("Ver");
    assert_eq!(
        labels_and_actions(see.items),
        [
            ("Acercar", AppAction::ZoomIn),
            ("Alejar", AppAction::ZoomOut),
            ("Ajustar a la ventana", AppAction::ZoomFit),
            ("1:1", AppAction::ZoomActual),
        ]
    );
}

#[test]
fn row_12_view_gains_a_canvas_background_submenu_with_the_five_options_in_order() {
    let see = el_menu("Ver");
    assert_eq!(see.submenus.len(), 1, "un solo submenú");
    let background_2 = &see.submenus[0];
    assert_eq!(background_2.label, "Fondo del lienzo");
    assert_eq!(
        labels_and_actions(background_2.items),
        [
            (
                "Damero",
                AppAction::CanvasBackground(CanvasBackground::Checker)
            ),
            (
                "Negro",
                AppAction::CanvasBackground(CanvasBackground::Black)
            ),
            ("Gris", AppAction::CanvasBackground(CanvasBackground::Gray)),
            (
                "Blanco",
                AppAction::CanvasBackground(CanvasBackground::White)
            ),
            ("Color…", AppAction::BackgroundColor),
        ]
    );
    for item in background_2.items {
        assert!(!item.separator_before, "{}", item.label);
    }
}

#[test]
fn row_12_the_rest_of_the_menus_have_no_submenus_and_there_are_five_in_order() {
    let titles: Vec<&str> = MENUS.iter().map(|m| m.title).collect();
    assert_eq!(
        titles,
        ["Archivo", "Editar", "Imagen", "Capa", "Ver", "Ayuda"]
    );
    for title in ["Archivo", "Editar", "Imagen", "Capa", "Ayuda"] {
        assert!(el_menu(title).submenus.is_empty(), "{title}");
    }
}

#[test]
fn row_12_menu_items_are_the_11_entries_of_the_17_new_image_of_the_19_and_convert_of_the_20() {
    let actions: Vec<AppAction> = MENUS
        .iter()
        .flat_map(|m| m.items.iter().map(|i| i.action))
        .collect();
    // `Nueva image_2…` (unidad 19) es la primera y `Convertir mode_2…` (unidad 20) va tras
    // `Redo`; las otras 11 son las de la 17, sin cambios.
    let mut expected_2 = vec![AppAction::NewImage];
    // `Exportar PNG aplanado…` (3.5) va tras `Guardar as_word…`, antes de `Salir`.
    expected_2.extend(&TABLE_ACTIONS[..3]);
    expected_2.push(AppAction::ExportFlat);
    expected_2.extend(&TABLE_ACTIONS[3..6]);
    // `Borrar página` (I.2a) cierra `Editar`, antes de `TestImage`.
    expected_2.push(AppAction::ClearPage);
    expected_2.push(AppAction::ConvertMode);
    expected_2.extend([
        AppAction::NewLayer,
        AppAction::DuplicateLayer,
        AppAction::RemoveLayer,
        AppAction::LayerUp,
        AppAction::LayerDown,
        AppAction::MergeLayerDown,
        AppAction::MergeVisibleLayers,
        AppAction::FlattenLayers,
        AppAction::RenameLayer,
        AppAction::LayerProperties,
        AppAction::ToggleLayerPanel,
    ]);
    expected_2.extend(&TABLE_ACTIONS[6..10]);
    // `Ayuda` (I.5): la ayuda de la herramienta y los atajos preceden a las licencias.
    expected_2.push(AppAction::ShowHelp(HelpTarget::Active));
    expected_2.push(AppAction::ShowShortcuts);
    expected_2.push(AppAction::ShowLicenses);
    assert_eq!(actions, expected_2);
    // Ninguna acción del fondo cuela en `items`.
    for b in BACKGROUNDS {
        assert!(!actions.contains(&action_2(b)));
    }
    // Los rótulos y los separadores de la 17, como estaban.
    let file = el_menu("Archivo");
    assert_eq!(
        labels_and_actions(file.items),
        [
            ("Nueva imagen…", AppAction::NewImage),
            ("Abrir…", AppAction::Open),
            ("Guardar", AppAction::Save),
            ("Guardar como…", AppAction::SaveAs),
            ("Exportar PNG aplanado…", AppAction::ExportFlat),
            ("Salir", AppAction::Quit),
        ]
    );
    assert_eq!(
        labels_and_actions(el_menu("Editar").items),
        [
            ("Deshacer", AppAction::Undo),
            ("Rehacer", AppAction::Redo),
            ("Borrar página", AppAction::ClearPage),
        ]
    );
    assert_eq!(
        labels_and_actions(el_menu("Imagen").items),
        [("Convertir modo…", AppAction::ConvertMode)]
    );
    assert_eq!(
        labels_and_actions(el_menu("Ayuda").items),
        [
            (
                "Ayuda de la herramienta",
                AppAction::ShowHelp(HelpTarget::Active)
            ),
            ("Atajos de teclado…", AppAction::ShowShortcuts),
            ("Licencias", AppAction::ShowLicenses),
        ]
    );
}

#[test]
fn row_12_the_four_actions_of_the_submenu_are_different_and_are_not_in_items() {
    let see = el_menu("Ver");
    let actions: Vec<AppAction> = see.submenus[0].items.iter().map(|i| i.action).collect();
    // Las cuatro fijas y, al final, `Color…` (unidad 19b).
    let mut expected_2 = the_four().to_vec();
    expected_2.push(AppAction::BackgroundColor);
    assert_eq!(actions, expected_2);
    for (i, a) in actions.iter().enumerate() {
        assert!(!actions[i + 1..].contains(a), "{a:?} repetida");
    }
}

// ---------------------------------------------------------------------------------------------
// Fila 12: la marca
// ---------------------------------------------------------------------------------------------

#[test]
fn row_12_background_checked_is_true_only_for_the_active_background_action() {
    for active_2_2 in BACKGROUNDS {
        for action in all_the_actions() {
            assert_eq!(
                background_checked(action, active_2_2),
                action == action_2(active_2_2),
                "{action:?} con {active_2_2:?} activo"
            );
        }
    }
}

#[test]
fn row_12_with_each_active_background_there_is_exactly_one_mark_in_the_submenu() {
    let see = el_menu("Ver");
    for active_2_2 in BACKGROUNDS {
        let marked_2: Vec<&str> = see.submenus[0]
            .items
            .iter()
            .filter(|i| background_checked(i.action, active_2_2))
            .map(|i| i.label)
            .collect();
        let expected_2 = match active_2_2 {
            CanvasBackground::Checker => "Damero",
            CanvasBackground::Black => "Negro",
            CanvasBackground::Gray => "Gris",
            CanvasBackground::White => "Blanco",
            // Con un color propio la marca va en `Color…` (unidad 19b).
            CanvasBackground::Custom(_) => "Color…",
        };
        assert_eq!(marked_2, [expected_2], "{active_2_2:?} activo");
    }
}

#[test]
fn row_12_background_checked_is_false_for_the_menu_entries_of_the_17() {
    for active_2_2 in BACKGROUNDS {
        for menu in MENUS.iter() {
            for item in menu.items {
                assert!(
                    !background_checked(item.action, active_2_2),
                    "{}",
                    item.label
                );
            }
        }
    }
}

// ---------------------------------------------------------------------------------------------
// Fila 12: sin atajo
// ---------------------------------------------------------------------------------------------

#[test]
fn row_12_no_shortcut_on_any_platform() {
    for platform in PLATFORMS {
        for b in BACKGROUNDS {
            assert_eq!(
                shortcut_for(action_2(b), platform),
                None,
                "{b:?} en {platform:?}"
            );
        }
    }
}

#[test]
fn row_12_no_key_with_any_modifier_gives_a_background_action() {
    for platform in PLATFORMS {
        for key in Key::ALL {
            for bits in 0..32u8 {
                let result = action_for_shortcut(*key, modifiers_2(bits), platform);
                assert!(
                    !matches!(result, Some(AppAction::CanvasBackground(_))),
                    "{key:?} con modificadores {bits:#07b} en {platform:?}: {result:?}"
                );
            }
        }
    }
}

// ---------------------------------------------------------------------------------------------
// Fila 12: rótulos
// ---------------------------------------------------------------------------------------------

/// Los rótulos nuevos: el del submenú y los de sus cuatro entradas.
fn labels_new() -> Vec<&'static str> {
    let background_2 = &el_menu("Ver").submenus[0];
    let mut v = vec![background_2.label];
    v.extend(background_2.items.iter().map(|i| i.label));
    v
}

#[test]
fn row_12_the_new_labels_are_inside_interface_charset() {
    for text_2 in labels_new() {
        for c in text_2.chars() {
            assert!(
                INTERFACE_CHARSET.contains(c),
                "{c:?} de {text_2:?} no está en INTERFACE_CHARSET"
            );
        }
    }
}

#[test]
fn row_12_no_new_label_loses_a_character_or_carries_a_question_mark() {
    let has = |c: char| INTERFACE_CHARSET.contains(c);
    for text_2 in labels_new() {
        assert_eq!(replace_missing(text_2, has), text_2);
        assert!(!text_2.contains('?'), "{text_2:?} lleva un `?`");
        assert!(!text_2.is_empty());
    }
}

#[test]
fn row_12_with_the_real_font_printable_changes_no_new_label() {
    let ctx = context(1.0);
    for text_2 in labels_new() {
        assert_eq!(printable(&ctx, text_2), text_2);
    }
}

#[test]
fn row_12_without_repeated_labels_with_those_of_menus() {
    // Todos los rótulos de la barra: títulos, entradas y los nuevos.
    let mut all_2: Vec<&str> = MENUS.iter().map(|m| m.title).collect();
    all_2.extend(MENUS.iter().flat_map(|m| m.items.iter().map(|i| i.label)));
    let n_previous = all_2.len();
    all_2.extend(labels_new());
    for (i, r) in all_2.iter().enumerate() {
        assert!(!all_2[i + 1..].contains(r), "rótulo repetido: {r}");
    }
    assert_eq!(
        all_2.len(),
        n_previous + 6,
        "el submenú y sus cinco entradas"
    );
}

#[test]
fn row_12_the_text_of_the_shortcuts_of_the_17_does_not_change() {
    // `shortcut_text` y `shortcut_for` de las entradas de siempre, como estaban.
    let s = shortcut_for(AppAction::ZoomActual, Platform::Linux).expect("tiene atajo");
    assert_eq!(shortcut_text(s, Platform::Linux), "Ctrl+1");
    assert_eq!(shortcut_for(AppAction::ShowLicenses, Platform::Linux), None);
}

// ---------------------------------------------------------------------------------------------
// show_menu y show_menu_with_background
// ---------------------------------------------------------------------------------------------

#[test]
fn show_menu_keeps_its_signature_and_the_version_with_the_background_is_new() {
    let _: fn(&mut Ui, &UiState, Platform) -> Option<AppAction> = show_menu;
    let _: fn(&mut Ui, &UiState, Platform, CanvasBackground) -> Option<AppAction> =
        show_menu_with_background;
}

fn state_2() -> UiState {
    UiState {
        can_undo: true,
        can_redo: false,
        zoom: Zoom::from_int(5),
        tool: Tool::Pencil,
        locks: Locks::default(),
    }
}

#[test]
fn show_menu_with_background_paints_the_bar_with_each_active_background_like_show_menu() {
    let screen = egui::vec2(400.0, 300.0);
    for ppp in common::PPPS {
        let ctx = context(ppp);
        let state_2 = state_2();
        let mut chosen_2 = None;
        let with = frame(&ctx, screen, vec![], |ui| {
            chosen_2 = show_menu(ui, &state_2, Platform::Linux);
        });
        assert_eq!(chosen_2, None, "sin pulsar nada no se elige nada");
        let with = painted(&with);
        for title in ["Archivo", "Editar", "Ver", "Ayuda"] {
            assert!(with.paints(title), "ppp {ppp}: falta el título {title:?}");
        }
        for background_2 in BACKGROUNDS {
            let mut chosen_2 = None;
            let ctx = context(ppp);
            let other_2 = frame(&ctx, screen, vec![], |ui| {
                chosen_2 = show_menu_with_background(ui, &state_2, Platform::Linux, background_2);
            });
            assert_eq!(chosen_2, None);
            let other_2 = painted(&other_2);
            for title in ["Archivo", "Editar", "Ver", "Ayuda"] {
                assert!(
                    other_2.paints(title),
                    "ppp {ppp} {background_2:?}: falta {title:?}"
                );
            }
            // Cerrado, el desplegable no se ve: la barra es la misma con cualquier fondo activo.
            assert_eq!(
                format!("{:?}", other_2.all_the_texts()),
                format!("{:?}", with.all_the_texts()),
                "ppp {ppp} {background_2:?}"
            );
        }
    }
}

// ---------------------------------------------------------------------------------------------
// Fila 13, parte pura: bloqueos y habilitación
// ---------------------------------------------------------------------------------------------

#[test]
fn row_13_is_blocked_is_true_only_with_dialog_modal_or_text_focus_with_any_source() {
    for b in BACKGROUNDS {
        for bits in 0..32u8 {
            let locks = locks_2(bits);
            for origin in SOURCES {
                assert_eq!(
                    is_blocked(action_2(b), locks, origin),
                    locks.dialog || locks.modal || locks.text_focus,
                    "{b:?} con {locks:?} y {origin:?}"
                );
            }
        }
    }
}

#[test]
fn row_13_each_lock_alone() {
    let only = |f: fn(&mut Locks)| {
        let mut l = Locks::default();
        f(&mut l);
        l
    };
    let cases: [(&str, Locks, bool); 6] = [
        ("ninguno", Locks::default(), false),
        ("gesture", only(|l| l.gesture = true), false),
        ("removal", only(|l| l.removal = true), false),
        ("dialog", only(|l| l.dialog = true), true),
        ("modal", only(|l| l.modal = true), true),
        ("text_focus", only(|l| l.text_focus = true), true),
    ];
    for b in BACKGROUNDS {
        for (name_2, locks, locked) in cases {
            for origin in SOURCES {
                assert_eq!(
                    is_blocked(action_2(b), locks, origin),
                    locked,
                    "{b:?} con {name_2} y {origin:?}"
                );
            }
        }
    }
}

#[test]
fn row_13_neither_the_gesture_nor_the_removal_nor_the_two_together_block_it() {
    let gesture_and_removes = Locks {
        gesture: true,
        removal: true,
        ..Locks::default()
    };
    for b in BACKGROUNDS {
        for origin in SOURCES {
            assert!(
                !is_blocked(action_2(b), gesture_and_removes, origin),
                "{b:?} {origin:?}"
            );
        }
    }
}

#[test]
fn row_13_it_is_not_zoom_or_repeat() {
    for b in BACKGROUNDS {
        assert!(!action_2(b).is_zoom(), "{b:?}");
        assert!(!action_2(b).ignores_repeat(), "{b:?}");
    }
}

fn states() -> Vec<UiState> {
    let mut v = Vec::new();
    for can_undo in [false, true] {
        for can_redo in [false, true] {
            for zoom in [None, Some(Zoom::MIN), Zoom::from_int(5), Some(Zoom::MAX)] {
                for tool in TEST_TOOLS {
                    for bits in 0..32u8 {
                        v.push(UiState {
                            can_undo,
                            can_redo,
                            zoom,
                            tool,
                            locks: locks_2(bits),
                        });
                    }
                }
            }
        }
    }
    v
}

#[test]
fn row_13_enabled_is_the_opposite_of_is_blocked_with_the_menu_source() {
    for state in states() {
        for b in BACKGROUNDS {
            assert_eq!(
                enabled(action_2(b), &state),
                !is_blocked(action_2(b), state.locks, Origin::Menu),
                "{b:?} con {state:?}"
            );
        }
    }
}

#[test]
fn row_13_with_a_focused_text_field_the_submenu_is_disabled_and_without_it_enabled() {
    let with_text = UiState {
        locks: Locks {
            text_focus: true,
            ..Locks::default()
        },
        ..state_2()
    };
    let without_text = state_2();
    for b in BACKGROUNDS {
        assert!(!enabled(action_2(b), &with_text), "{b:?}");
        assert!(enabled(action_2(b), &without_text), "{b:?}");
    }
}

/// Modelo de `is_blocked` de las acciones de antes, escrito aparte (el de `actions.rs`): (1)
/// `CloseLicenses` nunca; (2) `modal`: todo salvo `Quit`; (3) `dialog`: todo; (4) zoom: con
/// `text_focus` solo si el origen es `Key`; (5) el resto: `gesture` o `text_focus`, y `removal` solo
/// `SelectColor` y las que abren un diálogo (`ShowLicenses`, unidad 18, D2); (6) nada.
fn locked_according_to_the_model(action: AppAction, locks: Locks, origin: Origin) -> bool {
    if action == AppAction::CloseLicenses {
        return false;
    }
    if locks.modal && action != AppAction::Quit {
        return true;
    }
    if locks.dialog {
        return true;
    }
    if action.is_zoom() {
        return locks.text_focus && origin == Origin::Key;
    }
    if locks.gesture || locks.text_focus {
        return true;
    }
    locks.removal && matches!(action, AppAction::SelectColor(_) | AppAction::ShowLicenses)
}

#[test]
fn row_13_the_previous_actions_are_blocked_the_same_as_before() {
    for action in actions_previous() {
        for bits in 0..32u8 {
            let locks = locks_2(bits);
            for origin in SOURCES {
                assert_eq!(
                    is_blocked(action, locks, origin),
                    locked_according_to_the_model(action, locks, origin),
                    "{action:?} con {locks:?} y {origin:?}"
                );
            }
        }
    }
}

#[test]
fn row_13_the_previous_actions_are_enabled_the_same_as_before() {
    for state in states() {
        for action in actions_previous() {
            let mut expected_2 = !locked_according_to_the_model(action, state.locks, Origin::Menu);
            if expected_2 {
                expected_2 = match action {
                    AppAction::Undo => state.can_undo,
                    AppAction::Redo => state.can_redo,
                    AppAction::ZoomIn => state.zoom.is_some_and(|z| z < Zoom::MAX),
                    AppAction::ZoomOut => state.zoom.is_some_and(|z| z > Zoom::MIN),
                    AppAction::ZoomFit => state.zoom.is_some(),
                    AppAction::ZoomActual => state.zoom.is_some_and(|z| z != Zoom::ONE),
                    _ => true,
                };
            }
            assert_eq!(
                enabled(action, &state),
                expected_2,
                "{action:?} con {state:?}"
            );
        }
    }
}

// ---------------------------------------------------------------------------------------------
// Unidad 19b, fila 2: la quinta entrada `Color…`
// ---------------------------------------------------------------------------------------------
//
// Suposiciones: `AppAction::BackgroundColor` es `Clone, Copy, Debug, PartialEq, Eq` como las demás;
// `submenu_width_u(&Submenu) -> i64` y `background_value_text(CanvasBackground) -> Option<String>`
// son las de «Piezas». El banco abre el submenú **con clics** (`Ver`, `Fondo del canvas_2`), como lo
// hace `dialog_open_focus.rs` con `Ayuda → Licencias`, y mira lo pintado en orden.

const BLUE_STEEL: Rgba = Rgba::opaque(0x33, 0x66, 0x99);
const TEST_BLACK: Rgba = Rgba::opaque(0, 0, 0);

/// Los seis fondos activos de la fila 2: los cuatro de siempre y dos colores propios (el segundo,
/// negro: no salta a `Negro`).
const ACTIVE: [CanvasBackground; 6] = [
    CanvasBackground::Checker,
    CanvasBackground::Black,
    CanvasBackground::Gray,
    CanvasBackground::White,
    CanvasBackground::Custom(BLUE_STEEL),
    CanvasBackground::Custom(TEST_BLACK),
];

fn the_background_submenu() -> &'static Submenu {
    &el_menu("Ver").submenus[0]
}

/// La acción cuya entrada lleva la marca con `active_2_2`.
fn action_marked(active_2_2: CanvasBackground) -> AppAction {
    match active_2_2 {
        CanvasBackground::Custom(_) => AppAction::BackgroundColor,
        other_2 => action_2(other_2),
    }
}

/// Las acciones de antes, las cuatro del fondo y la nueva.
fn all_with_color() -> Vec<AppAction> {
    let mut v = all_the_actions();
    v.push(AppAction::BackgroundColor);
    v
}

#[test]
fn row_2_the_new_action_is_a_plain_type_and_different_from_the_others() {
    assert_plane::<AppAction>();
    for other in all_the_actions() {
        assert_ne!(AppAction::BackgroundColor, other);
    }
}

#[test]
fn row_2_canvas_background_has_five_entries_and_the_fifth_is_color_without_a_separator() {
    let background_2 = the_background_submenu();
    assert_eq!(background_2.label, "Fondo del lienzo");
    let labels: Vec<&str> = background_2.items.iter().map(|i| i.label).collect();
    assert_eq!(labels, ["Damero", "Negro", "Gris", "Blanco", "Color…"]);
    let last = background_2.items.last().expect("hay entradas");
    assert_eq!(last.action, AppAction::BackgroundColor);
    assert!(background_2.items.iter().all(|i| !i.separator_before));
    // Los puntos suspensivos son un solo carácter (U+2026) del juego, no tres puntos ni `?`.
    assert_eq!(last.label.chars().count(), 6);
    assert!(last.label.ends_with('…'));
    assert!(!last.label.contains('?'));
    // `Ver` conserva sus cuatro entradas y no gana ninguna: la nueva está solo en el submenú.
    assert_eq!(el_menu("Ver").items.len(), 4);
    for menu in MENUS.iter() {
        assert!(
            menu.items
                .iter()
                .all(|i| i.action != AppAction::BackgroundColor),
            "{}",
            menu.title
        );
    }
}

#[test]
fn row_2_with_each_active_background_there_is_exactly_one_mark_and_with_custom_it_is_color() {
    let background_2 = the_background_submenu();
    for active_2_2 in ACTIVE {
        let marked_2: Vec<&str> = background_2
            .items
            .iter()
            .filter(|i| background_checked(i.action, active_2_2))
            .map(|i| i.label)
            .collect();
        let expected_2 = match active_2_2 {
            CanvasBackground::Checker => "Damero",
            CanvasBackground::Black => "Negro",
            CanvasBackground::Gray => "Gris",
            CanvasBackground::White => "Blanco",
            CanvasBackground::Custom(_) => "Color…",
        };
        assert_eq!(marked_2, [expected_2], "{active_2_2:?} activo");
    }
}

#[test]
fn row_2_background_checked_is_true_only_for_the_marked_action_with_any_background() {
    for active_2_2 in ACTIVE {
        for action in all_with_color() {
            assert_eq!(
                background_checked(action, active_2_2),
                action == action_marked(active_2_2),
                "{action:?} con {active_2_2:?} activo"
            );
        }
    }
}

#[test]
fn row_2_with_a_custom_color_none_of_the_four_fixed_ones_is_marked_not_even_with_the_same_color() {
    for own in [
        CanvasBackground::Custom(BLUE_STEEL),
        CanvasBackground::Custom(TEST_BLACK),
        CanvasBackground::Custom(Rgba::opaque(255, 255, 255)),
        CanvasBackground::Custom(Rgba::opaque(40, 40, 40)),
    ] {
        for fixed in the_four() {
            assert!(!background_checked(fixed, own), "{fixed:?} con {own:?}");
        }
        assert!(background_checked(AppAction::BackgroundColor, own));
    }
    // Y al revés: con una fija activa, `Color…` no se marca.
    for active_2_2 in BACKGROUNDS {
        assert!(!background_checked(AppAction::BackgroundColor, active_2_2));
    }
}

#[test]
fn row_2_color_has_no_shortcut_on_any_platform_and_no_key_gives_it() {
    for platform in PLATFORMS {
        assert_eq!(
            shortcut_for(AppAction::BackgroundColor, platform),
            None,
            "{platform:?}"
        );
        for key in Key::ALL {
            for bits in 0..32u8 {
                assert_ne!(
                    action_for_shortcut(*key, modifiers_2(bits), platform),
                    Some(AppAction::BackgroundColor),
                    "{key:?} con modificadores {bits:#07b} en {platform:?}"
                );
            }
        }
    }
}

#[test]
fn row_2_the_color_label_is_in_the_set_without_a_question_mark_and_printable_leaves_it_the_same() {
    let color = "Color…";
    assert!(color.chars().all(|c| INTERFACE_CHARSET.contains(c)));
    let has = |c: char| INTERFACE_CHARSET.contains(c);
    assert_eq!(replace_missing(color, has), color);
    assert!(!color.contains('?'));
    for ppp in PPPS {
        let ctx = context(ppp);
        assert_eq!(printable(&ctx, color), color, "ppp {ppp}");
    }
    // Está en la tabla y no se repite con ningún otro rótulo de la barra.
    assert!(labels_new().contains(&color));
    let mut all_2: Vec<&str> = MENUS.iter().map(|m| m.title).collect();
    all_2.extend(MENUS.iter().flat_map(|m| m.items.iter().map(|i| i.label)));
    assert!(!all_2.contains(&color));
}

#[test]
fn row_2_submenu_width_u_is_131_with_the_longest_label_plus_the_value_column() {
    let background_2 = the_background_submenu();
    // 12 + (2 + rótulo más largo + 2 + 7) · 7 con «Color…» (6 caracteres, como `Damero`).
    let more_long_2 = background_2
        .items
        .iter()
        .map(|i| i.label.chars().count() as i64)
        .max()
        .expect("hay entradas");
    assert_eq!(more_long_2, 6);
    assert_eq!(
        submenu_width_u(background_2),
        12 + (2 + more_long_2 + 2 + 7) * 7
    );
    assert_eq!(submenu_width_u(background_2), 131);
}

/// Lo que se ve de una forma, en orden de pintado.
#[derive(Clone, Debug)]
enum Shape2 {
    Text {
        text_2: String,
        rect: Rect,
        clip: Rect,
        colors_2: Vec<Color32>,
        opacity_2: f32,
        /// Color del primer vértice de glifo tal como lo pinta la fuente: lo que cambia al atenuar.
        glyph: Option<Color32>,
    },
    TestFill {
        rect: Rect,
        fill: Color32,
        clip: Rect,
    },
}

impl Shape2 {
    fn look(&self) -> (Vec<Color32>, u32) {
        match self {
            Shape2::Text {
                colors_2,
                opacity_2,
                glyph,
                ..
            } => {
                let mut v = colors_2.clone();
                v.extend(glyph);
                (v, opacity_2.to_bits())
            }
            Shape2::TestFill { .. } => (vec![], 0),
        }
    }
}

fn flattens(shape: &Shape, clip: Rect, outside: &mut Vec<Shape2>) {
    match shape {
        Shape::Text(t) => {
            let job = &t.galley.job;
            let colors_2 = job
                .sections
                .iter()
                .map(|s| {
                    t.override_text_color
                        .unwrap_or(if s.format.color == Color32::PLACEHOLDER {
                            t.fallback_color
                        } else {
                            s.format.color
                        })
                })
                .collect();
            let glyph = t.galley.rows.iter().find_map(|row_2| {
                let visual = &row_2.row.visuals;
                visual.mesh.vertices[visual.glyph_vertex_range.clone()]
                    .first()
                    .map(|v| v.color)
            });
            outside.push(Shape2::Text {
                text_2: t.galley.text().to_owned(),
                rect: Rect::from_min_size(t.pos, t.galley.size()),
                clip,
                colors_2,
                opacity_2: t.opacity_factor,
                glyph,
            });
        }
        Shape::Rect(r) => outside.push(Shape2::TestFill {
            rect: r.rect,
            fill: r.fill,
            clip,
        }),
        Shape::Vec(shapes_2) => {
            for shape_2 in shapes_2 {
                flattens(shape_2, clip, outside);
            }
        }
        _ => {}
    }
}

fn shapes_in_order(shapes_2: &[ClippedShape]) -> Vec<Shape2> {
    let mut outside = Vec::new();
    for clipped in shapes_2 {
        flattens(&clipped.shape, clipped.clip_rect, &mut outside);
    }
    outside
}

/// `src` (premultiplicado, como egui) por encima de `dst` opaco.
fn above(dst: Color32, src: Color32) -> Color32 {
    let k = 1.0 - f32::from(src.a()) / 255.0;
    let channel = |s: u8, d: u8| (f32::from(s) + f32::from(d) * k).round().clamp(0.0, 255.0) as u8;
    Color32::from_rgb(
        channel(src.r(), dst.r()),
        channel(src.g(), dst.g()),
        channel(src.b(), dst.b()),
    )
}

/// Contraste mínimo de texto (WCAG AA).
const TEST_MIN_TEXT: f64 = 4.5;

const SCREEN_OF_THE_MENU: Vec2 = vec2(600.0, 400.0);

/// La barra con el menú `Ver` y su submenú `Fondo del canvas_2` abiertos con el ratón, un fotograma
/// tras otro como `App::ui`: `show_menu_with_background` con el fondo activo y los bloqueos dados.
struct BenchOfTheMenu {
    ctx: Context,
    background_2: CanvasBackground,
    locks: Locks,
    shapes_2: Vec<Shape2>,
    chosen_2: Option<AppAction>,
    /// El reloj de egui, en segundos: avanza en cada fotograma para que terminen las animaciones de
    /// los desplegables (con el tiempo parado se verían a medio aparecer, translúcidos).
    time_2: f64,
}

impl BenchOfTheMenu {
    fn new_2(ppp: f32, background_2: CanvasBackground, locks: Locks) -> BenchOfTheMenu {
        let ctx = context(ppp);
        ctx.set_visuals_of(egui::Theme::Dark, visuals(ppp));
        let mut bench = BenchOfTheMenu {
            ctx,
            background_2,
            locks,
            shapes_2: Vec::new(),
            chosen_2: None,
            time_2: 0.0,
        };
        bench.rest_2_2();
        bench
    }

    /// Con `Ver → Fondo del canvas_2` abierto con dos clics reales.
    fn open_2_2(ppp: f32, background_2: CanvasBackground, locks: Locks) -> BenchOfTheMenu {
        let mut bench = BenchOfTheMenu::new_2(ppp, background_2, locks);
        let see = bench.center_of("Ver");
        bench.click(see);
        bench.rest_2_2();
        let submenu = bench.center_of("Fondo del lienzo");
        bench.click(submenu);
        bench.rest_2_2();
        bench
    }

    fn frame(&mut self, events: Vec<Event>) {
        let state_2 = UiState {
            can_undo: true,
            can_redo: true,
            zoom: Zoom::from_int(5),
            tool: Tool::Pencil,
            locks: self.locks,
        };
        let background_2 = self.background_2;
        let mut chosen_2 = None;
        self.time_2 += 0.25;
        let input_2 = RawInput {
            screen_rect: Some(Rect::from_min_size(Pos2::ZERO, SCREEN_OF_THE_MENU)),
            events,
            time: Some(self.time_2),
            ..RawInput::default()
        };
        let mut output_2 = self.ctx.run_ui(input_2, |ui| {
            apply_scale(ui.ctx());
            chosen_2 = show_menu_with_background(ui, &state_2, Platform::Linux, background_2);
        });
        output_2.textures_delta.clear();
        self.shapes_2 = shapes_in_order(&output_2.shapes);
        if chosen_2.is_some() {
            self.chosen_2 = chosen_2;
        }
    }

    /// Unos fotogramas sin eventos (1,5 s de reloj): lo que tarda en asentarse la animación.
    fn rest_2_2(&mut self) {
        for _ in 0..6 {
            self.frame(vec![]);
        }
    }

    /// Mueve, pulsa y suelta en `pos`: tres fotogramas, como los entrega winit.
    fn click(&mut self, pos: Pos2) {
        self.frame(vec![move_pointer(pos)]);
        self.frame(vec![button_2(pos, true)]);
        self.frame(vec![button_2(pos, false)]);
    }

    /// El índice de la primera forma de texto que contiene `searched`.
    fn index_of(&self, searched: &str) -> Option<usize> {
        self.shapes_2
            .iter()
            .position(|f| matches!(f, Shape2::Text { text_2, .. } if text_2.contains(searched)))
    }

    fn text_2(&self, searched: &str) -> Option<&Shape2> {
        self.index_of(searched).map(|i| &self.shapes_2[i])
    }

    fn rect_de(&self, searched: &str) -> Rect {
        match self.text_2(searched) {
            Some(Shape2::Text { rect, .. }) => *rect,
            _ => panic!(
                "no se pinta {searched:?}; hay {:?}",
                self.shapes_2
                    .iter()
                    .filter_map(|f| match f {
                        Shape2::Text { text_2, .. } => Some(text_2.as_str()),
                        Shape2::TestFill { .. } => None,
                    })
                    .collect::<Vec<_>>()
            ),
        }
    }

    fn center_of(&self, searched: &str) -> Pos2 {
        self.rect_de(searched).center()
    }

    /// La fila donde está el texto `searched` (el último relleno anterior que lo contiene, aunque
    /// sea transparente) y el color real que hay debajo: todos los rellenos anteriores que lo
    /// contienen, mezclados en orden sobre el fondo del lienzo (el que hay detrás del menú).
    fn background_under(&self, searched: &str) -> Option<(Rect, Color32)> {
        let i = self.index_of(searched)?;
        let Shape2::Text { rect, clip, .. } = &self.shapes_2[i] else {
            return None;
        };
        let visible = rect.intersect(*clip);
        let probe = visible.shrink2(vec2(0.0, visible.height() * 0.25));
        let mut row_2 = None;
        let mut background_2 = BACKGROUND;
        for shape_2 in &self.shapes_2[..i] {
            if let Shape2::TestFill { rect, fill, clip } = shape_2
                && rect.intersect(*clip).expand(0.01).contains_rect(probe)
            {
                row_2 = Some(*rect);
                background_2 = above(background_2, *fill);
            }
        }
        row_2.map(|row_2| (row_2, background_2))
    }

    /// El color con el que se pinta el texto `searched`: el del primer glifo (lo que sale por
    /// pantalla, con el atenuado y el color forzado ya aplicados).
    fn color_painted(&self, searched: &str) -> Color32 {
        match self.text_2(searched) {
            Some(Shape2::Text {
                glyph, colors_2, ..
            }) => glyph
                .or_else(|| colors_2.first().copied())
                .unwrap_or(Color32::PLACEHOLDER),
            _ => panic!("no se pinta {searched:?}"),
        }
    }
}

fn without_locks() -> Locks {
    Locks::default()
}

#[test]
fn row_2_the_submenu_opens_with_clicks_and_paints_its_five_labels() {
    for ppp in PPPS {
        let bench = BenchOfTheMenu::open_2_2(ppp, CanvasBackground::Checker, without_locks());
        for label in ["Damero", "Negro", "Gris", "Blanco", "Color…"] {
            assert!(
                bench.text_2(label).is_some(),
                "ppp {ppp}: falta {label:?} en el submenú abierto"
            );
        }
    }
}

#[test]
fn row_2_the_value_is_painted_only_with_a_custom_color_and_to_the_right_of_color() {
    for ppp in PPPS {
        // Sin color propio no hay valor en ninguna fila: ni `#` ni dígitos hexadecimales sueltos.
        for fixed_2 in BACKGROUNDS {
            let bench = BenchOfTheMenu::open_2_2(ppp, fixed_2, without_locks());
            assert!(bench.text_2("Color…").is_some(), "ppp {ppp} {fixed_2:?}");
            assert!(
                bench.index_of("#").is_none(),
                "ppp {ppp} {fixed_2:?}: sin color propio no se pinta ningún valor"
            );
        }
        for (c, text_2) in [(BLUE_STEEL, "#336699"), (TEST_BLACK, "#000000")] {
            let active_2_2 = CanvasBackground::Custom(c);
            assert_eq!(background_value_text(active_2_2).as_deref(), Some(text_2));
            let bench = BenchOfTheMenu::open_2_2(ppp, active_2_2, without_locks());
            let value = bench.rect_de(text_2);
            let label_2 = bench.rect_de("Color…");
            // A la derecha de la etiqueta y en la misma fila.
            assert!(
                value.min.x >= label_2.max.x - 0.01,
                "ppp {ppp}: el valor {value:?} va a la derecha de {label_2:?}"
            );
            assert!(
                (value.center().y - label_2.center().y).abs() <= 1.0,
                "ppp {ppp}: el valor {value:?} está en la fila de {label_2:?}"
            );
            // Dentro de la fila (el relleno de la entrada marcada) y sin salirse por la derecha.
            let (row_2, _) = bench
                .background_under(text_2)
                .unwrap_or_else(|| panic!("ppp {ppp}: el valor no tiene relleno debajo"));
            assert!(
                value.max.x <= row_2.max.x + 0.01,
                "ppp {ppp}: el valor {value:?} cabe en la fila {row_2:?}"
            );
            // Un solo valor pintado.
            let n = bench
                .shapes_2
                .iter()
                .filter(|f| matches!(f, Shape2::Text { text_2: t, .. } if t.contains(text_2)))
                .count();
            assert_eq!(n, 1, "ppp {ppp}: {text_2} se pinta una vez");
            // NO PROBADO: que el valor vaya en `TEXT` (D3). La fila marcada se rellena con
            // `ACCENT` y `TEXT` sobre `ACCENT` da un contraste de 1,47, que contradice el
            // «≥ 4,5 sobre la fila marcada» de la misma D3: se prueba el contraste (la prueba de
            // abajo) y no el color concreto.
        }
    }
}

#[test]
fn row_2_the_value_has_contrast_of_at_least_4_5_over_its_marked_row_and_with_the_mouse() {
    // Con un color propio la entrada `Color…` es siempre la marcada: el valor se ve sobre la fila
    // marcada y, con el puntero encima, sobre la marcada resaltada. El relleno «normal» (sin marca ni
    // puntero) nunca lleva el valor: solo aparece con `Custom`, que marca la fila. NO PROBADO ese
    // tercer estado por eso. El color que se mide es el que sale por pantalla (el del glifo).
    for ppp in PPPS {
        for (c, text_2) in [(BLUE_STEEL, "#336699"), (TEST_BLACK, "#000000")] {
            let active_2_2 = CanvasBackground::Custom(c);
            let mut bench = BenchOfTheMenu::open_2_2(ppp, active_2_2, without_locks());
            for state_2 in ["marcada", "con el ratón"] {
                if state_2 == "con el ratón" {
                    let above = bench.center_of("Color…");
                    bench.frame(vec![move_pointer(above)]);
                    bench.rest_2_2();
                }
                let (_, background_2) = bench
                    .background_under(text_2)
                    .unwrap_or_else(|| panic!("ppp {ppp} {state_2}: sin relleno bajo el valor"));
                let color = bench.color_painted(text_2);
                let ratio = contrast_ratio(color, background_2);
                assert!(
                    ratio >= TEST_MIN_TEXT,
                    "ppp {ppp} {state_2}: el valor {text_2} ({color:?}) sobre {background_2:?} da {ratio:.2}"
                );
            }
        }
    }
}

#[test]
fn row_2_the_submenu_measures_the_same_at_the_five_scales_with_and_without_a_custom_color() {
    for ppp in PPPS {
        let u = ui_unit(ppp);
        // 131 u, el de la spec, y el que dice `submenu_width_u`: lo pintado cumple los dos.
        assert_eq!(submenu_width_u(the_background_submenu()), 131);
        let expected_2 = 131.0 * u;
        for active_2_2 in ACTIVE {
            let bench = BenchOfTheMenu::open_2_2(ppp, active_2_2, without_locks());
            let marked = match active_2_2 {
                CanvasBackground::Checker => "Damero",
                CanvasBackground::Black => "Negro",
                CanvasBackground::Gray => "Gris",
                CanvasBackground::White => "Blanco",
                CanvasBackground::Custom(_) => "Color…",
            };
            // El ancho del submenú es el del relleno de la fila marcada (una fila mide el ancho
            // del desplegable).
            let (row_2, _) = bench
                .background_under(marked)
                .unwrap_or_else(|| panic!("ppp {ppp} {active_2_2:?}: sin fila marcada"));
            let tolerance = tolerance_of_egui(ppp) / ppp;
            assert!(
                (row_2.width() - expected_2).abs() <= tolerance,
                "ppp {ppp} {active_2_2:?}: la fila mide {} pt y debía medir {expected_2} pt (131 u)",
                row_2.width()
            );
        }
    }
}

#[test]
fn row_2_color_is_dimmed_with_a_gesture_or_a_removal_and_the_four_fixed_ones_are_not() {
    let with_gesture = Locks {
        gesture: true,
        ..Locks::default()
    };
    let with_removes = Locks {
        removal: true,
        ..Locks::default()
    };
    for ppp in PPPS {
        let free = BenchOfTheMenu::open_2_2(ppp, CanvasBackground::Checker, without_locks());
        for (name_2, locks) in [("gesto", with_gesture), ("quita", with_removes)] {
            let bench = BenchOfTheMenu::open_2_2(ppp, CanvasBackground::Checker, locks);
            let look = |b: &BenchOfTheMenu, r: &str| b.text_2(r).expect("se pinta").look();
            assert_ne!(
                look(&bench, "Color…"),
                look(&free, "Color…"),
                "ppp {ppp}, {name_2}: `Color…` se pinta atenuada"
            );
            for fixed in ["Damero", "Negro", "Gris", "Blanco"] {
                assert_eq!(
                    look(&bench, fixed),
                    look(&free, fixed),
                    "ppp {ppp}, {name_2}: {fixed} sigue habilitada"
                );
            }
        }
    }
}

#[test]
fn row_2_enabled_of_color_is_false_with_a_gesture_or_a_removal_and_that_of_the_four_fixed_ones_true()
 {
    let base = state_2();
    for locks in [
        Locks {
            gesture: true,
            ..Locks::default()
        },
        Locks {
            removal: true,
            ..Locks::default()
        },
        Locks {
            gesture: true,
            removal: true,
            ..Locks::default()
        },
    ] {
        let state = UiState { locks, ..base };
        assert!(!enabled(AppAction::BackgroundColor, &state), "{locks:?}");
        for fixed in the_four() {
            assert!(enabled(fixed, &state), "{fixed:?} con {locks:?}");
        }
    }
    // Sin bloqueos, las cinco están habilitadas.
    for action in the_four().into_iter().chain([AppAction::BackgroundColor]) {
        assert!(enabled(action, &base), "{action:?}");
    }
    // Con un diálogo, un diálogo modal o un campo de texto enfocado, las cinco se atenúan.
    for locks in [
        Locks {
            dialog: true,
            ..Locks::default()
        },
        Locks {
            modal: true,
            ..Locks::default()
        },
        Locks {
            text_focus: true,
            ..Locks::default()
        },
    ] {
        let state = UiState { locks, ..base };
        for action in the_four().into_iter().chain([AppAction::BackgroundColor]) {
            assert!(!enabled(action, &state), "{action:?} con {locks:?}");
        }
    }
}

// ---------------------------------------------------------------------------------------------
// Unidad 19b, fila 3 (parte pura): `BackgroundColor` abre un diálogo y se bloquea como los demás
// ---------------------------------------------------------------------------------------------

#[test]
fn row_3_background_color_opens_a_dialog_and_does_not_repeat_or_is_zoom() {
    assert!(AppAction::BackgroundColor.opens_dialog());
    assert!(!AppAction::BackgroundColor.ignores_repeat());
    assert!(!AppAction::BackgroundColor.is_zoom());
    // Las cuatro fijas no abren ningún diálogo.
    for fixed in the_four() {
        assert!(!fixed.opens_dialog(), "{fixed:?}");
    }
}

#[test]
fn row_3_background_color_free_with_everything_false_and_blocked_with_each_lock_alone() {
    let only = |f: fn(&mut Locks)| {
        let mut l = Locks::default();
        f(&mut l);
        l
    };
    let cases: [(&str, Locks, bool); 6] = [
        ("ninguno", Locks::default(), false),
        ("gesture", only(|l| l.gesture = true), true),
        ("dialog", only(|l| l.dialog = true), true),
        ("text_focus", only(|l| l.text_focus = true), true),
        ("removal", only(|l| l.removal = true), true),
        ("modal", only(|l| l.modal = true), true),
    ];
    for (name_2, locks, locked) in cases {
        for origin in SOURCES {
            assert_eq!(
                is_blocked(AppAction::BackgroundColor, locks, origin),
                locked,
                "{name_2} y {origin:?}"
            );
        }
        // Y la regla de la 18 para abrir un diálogo da lo mismo.
        assert_eq!(can_open_dialog(locks), !locked, "{name_2}");
    }
}

#[test]
fn row_3_with_the_32_combinations_it_is_blocked_exactly_with_any_lock() {
    for bits in 0..32u8 {
        let locks = locks_2(bits);
        let some =
            locks.gesture || locks.dialog || locks.text_focus || locks.removal || locks.modal;
        for origin in SOURCES {
            assert_eq!(
                is_blocked(AppAction::BackgroundColor, locks, origin),
                some,
                "{locks:?} y {origin:?}"
            );
        }
        assert_eq!(can_open_dialog(locks), !some, "{locks:?}");
        // Con cualquier combinación, bloquea igual que las otras que abren un diálogo.
        for other in [
            AppAction::NewImage,
            AppAction::ConvertMode,
            AppAction::ShowLicenses,
        ] {
            assert_eq!(
                is_blocked(AppAction::BackgroundColor, locks, Origin::Menu),
                is_blocked(other, locks, Origin::Menu),
                "{other:?} con {locks:?}"
            );
        }
    }
}

#[test]
fn row_3_enabled_of_background_color_is_the_opposite_of_is_blocked_with_the_menu_source() {
    for state in states() {
        assert_eq!(
            enabled(AppAction::BackgroundColor, &state),
            !is_blocked(AppAction::BackgroundColor, state.locks, Origin::Menu),
            "{state:?}"
        );
    }
}

#[test]
fn row_3_the_four_fixed_ones_are_still_not_blocked_with_a_gesture_or_a_removal() {
    // Lo que cambia con la nueva acción es solo ella: con un gesto en curso o una quita pendiente,
    // `Negro` se puede elegir y `Color…` no.
    for only in [
        Locks {
            gesture: true,
            ..Locks::default()
        },
        Locks {
            removal: true,
            ..Locks::default()
        },
    ] {
        for origin in SOURCES {
            for fixed in the_four() {
                assert!(!is_blocked(fixed, only, origin), "{fixed:?} con {only:?}");
            }
            assert!(is_blocked(AppAction::BackgroundColor, only, origin));
        }
    }
    // Y las cuatro fijas se bloquean exactamente como antes con las 32 combinaciones.
    for bits in 0..32u8 {
        let locks = locks_2(bits);
        for fixed in the_four() {
            for origin in SOURCES {
                assert_eq!(
                    is_blocked(fixed, locks, origin),
                    locks.dialog || locks.modal || locks.text_focus,
                    "{fixed:?} con {locks:?} y {origin:?}"
                );
            }
        }
    }
}

// ---------------------------------------------------------------------------------------------
// Unidad 19c (specs/hito-2-contraste-menu.md): contraste de las entradas de los menús
// ---------------------------------------------------------------------------------------------
//
// Todo lo de abajo es un módulo de este ejecutable (D5) y reutiliza `BenchOfTheMenu`, `Shape2`,
// `above`, `background_under` (por índice, para no confundir «Guardar» con «Guardar como…») y los
// colores del tema. Reglas de medida:
//
// - El color de un texto es el del primer vértice de glifo (`Shape2::Text::glyph`), que es el de
//   pantalla pero **premultiplicado** (el atajo débil de egui es `#86868699`, lo desactivado lleva
//   alfa 128): se mezcla con `above` sobre el relleno real de debajo antes de `contrast_ratio`.
// - El relleno real es el de `background_under`: todos los rellenos anteriores que contienen el texto,
//   mezclados en orden sobre el fondo del lienzo.
// - El banco abre los menús con clics (barra y submenú) y mueve el ratón con eventos reales. Aquí
//   se construye con un envoltorio (`Mesa`) que también admite la plataforma (el banco fija Linux),
//   así que la fila 3 prueba las tres plataformas pintadas de verdad, no solo `shortcut_text`.
// - «Fondos activos»: los seis de `ACTIVE` (cuatro fijos, `Custom(#336699)` y `Custom(#000000)`).
// - La fila 5 compara posiciones y tamaños con las invariantes de la geometría de la 17 (margen de
//   6 u, fila de 16 u, texto de 14 u y 7 u por carácter, anchos de `popup_width_u` y
//   `submenu_width_u`), medidas sobre el código actual: no hay una captura «de antes» que comparar,
//   y un cambio de color no puede alterarlas. Es lo que se puede probar sin inventar cifras.
mod contrast_of_the_menu {
    use super::*;
    use proptest::prelude::*;
    use px_ui::menu::popup_width_u;
    use px_ui::theme::{ACCENT, SURFACE_0, TEXT};
    use std::collections::{BTreeMap, BTreeSet};

    /// Suelo de contraste del texto desactivado (visto bueno de D4): se compara con `>=`.
    const FLOOR_DISABLED: f64 = 2.0;
    /// Pares texto-fondo mínimos que debe recorrer la fila 1 (que no sea vacía por un fallo del
    /// banco): seis escenas, todas las filas y los seis fondos a las cinco escalas.
    const MINIMUM_PAIRS_ROW_1: usize = 10_000;

    // -----------------------------------------------------------------------------------------
    // Escenas
    // -----------------------------------------------------------------------------------------

    #[derive(Clone, Copy, Debug)]
    struct Scene {
        name_2: &'static str,
        menu: &'static str,
        /// Con `Ver → Fondo del canvas_2` abierto además del desplegable.
        submenu: bool,
    }

    const fn scene(name_2: &'static str, menu: &'static str, submenu: bool) -> Scene {
        Scene {
            name_2,
            menu,
            submenu,
        }
    }

    const ESC_FILE: Scene = scene("Archivo", "Archivo", false);
    const ESC_EDIT: Scene = scene("Editar", "Editar", false);
    const ESC_IMAGE: Scene = scene("Imagen", "Imagen", false);
    const ESC_SEE: Scene = scene("Ver", "Ver", false);
    const ESC_HELP: Scene = scene("Ayuda", "Ayuda", false);
    const ESC_BACKGROUND: Scene = scene("Ver → Fondo del lienzo", "Ver", true);
    /// Archivo, Editar, Imagen, Ver y su submenú (spec) y, de propina, Ayuda.
    const ESCENAS: [Scene; 6] = [
        ESC_FILE,
        ESC_EDIT,
        ESC_IMAGE,
        ESC_SEE,
        ESC_BACKGROUND,
        ESC_HELP,
    ];

    fn mark(background_2: CanvasBackground, item: &MenuItem) -> String {
        format!(
            "{} {}",
            if background_checked(item.action, background_2) {
                '·'
            } else {
                ' '
            },
            item.label
        )
    }

    /// Los rótulos de las filas sobre las que se pone el ratón en la escena, uno a uno.
    fn rows_of(scene: Scene, background_2: CanvasBackground) -> Vec<String> {
        if scene.submenu {
            the_background_submenu()
                .items
                .iter()
                .map(|i| mark(background_2, i))
                .collect()
        } else {
            let menu = el_menu(scene.menu);
            let mut v: Vec<String> = menu.items.iter().map(|i| i.label.to_string()).collect();
            v.extend(menu.submenus.iter().map(|s| s.label.to_string()));
            v
        }
    }

    /// La acción de una fila a partir de su rótulo pintado (con o sin marca).
    fn action_of_label(text_2: &str) -> Option<AppAction> {
        let de_menu = MENUS
            .iter()
            .flat_map(|m| m.items.iter())
            .find(|i| i.label == text_2);
        if let Some(i) = de_menu {
            return Some(i.action);
        }
        let without_mark = text_2
            .strip_prefix("· ")
            .or_else(|| text_2.strip_prefix("  "))?;
        the_background_submenu()
            .items
            .iter()
            .find(|i| i.label == without_mark)
            .map(|i| i.action)
    }

    // -----------------------------------------------------------------------------------------
    // El banco con plataforma
    // -----------------------------------------------------------------------------------------

    fn idx_exact(b: &BenchOfTheMenu, searched: &str) -> Option<usize> {
        b.shapes_2
            .iter()
            .position(|f| matches!(f, Shape2::Text { text_2, .. } if text_2 == searched))
    }

    fn rect_exact(b: &BenchOfTheMenu, searched: &str) -> Rect {
        match idx_exact(b, searched) {
            Some(i) => match &b.shapes_2[i] {
                Shape2::Text { rect, .. } => *rect,
                Shape2::TestFill { .. } => unreachable!(),
            },
            None => panic!(
                "no se pinta {searched:?}; hay {:?}",
                b.shapes_2
                    .iter()
                    .filter_map(|f| match f {
                        Shape2::Text { text_2, .. } => Some(text_2.as_str()),
                        Shape2::TestFill { .. } => None,
                    })
                    .collect::<Vec<_>>()
            ),
        }
    }

    /// Como `BenchOfTheMenu::background_under`, pero por índice de forma.
    fn background_in(b: &BenchOfTheMenu, i: usize) -> Option<(Rect, Color32)> {
        let Shape2::Text { rect, clip, .. } = &b.shapes_2[i] else {
            return None;
        };
        let visible = rect.intersect(*clip);
        let probe = visible.shrink2(vec2(0.0, visible.height() * 0.25));
        let mut row_2 = None;
        let mut background_2 = BACKGROUND;
        for shape_2 in &b.shapes_2[..i] {
            if let Shape2::TestFill { rect, fill, clip } = shape_2
                && rect.intersect(*clip).expand(0.01).contains_rect(probe)
            {
                row_2 = Some(*rect);
                background_2 = above(background_2, *fill);
            }
        }
        row_2.map(|row_2| (row_2, background_2))
    }

    /// Un texto pintado con su relleno real y su contraste.
    #[derive(Clone, Debug)]
    struct Measure {
        text_2: String,
        rect: Rect,
        /// El color del glifo tal cual lo pinta egui (premultiplicado).
        painted: Color32,
        /// La fila (relleno más cercano) y el color real de debajo; `None` si no hay relleno.
        row_2: Option<Rect>,
        background_2: Option<Color32>,
        /// `painted` mezclado sobre `background_2`: lo que ve el usuario.
        screen: Option<Color32>,
        ratio: Option<f64>,
    }

    impl Measure {
        fn describe(&self) -> String {
            match (self.background_2, self.screen, self.ratio) {
                (Some(f), Some(p), Some(r)) => format!(
                    "{:?} pintado {:?} (en pantalla {p:?}) sobre {f:?} da {r:.2}",
                    self.text_2, self.painted
                ),
                _ => format!(
                    "{:?} pintado {:?} sin relleno debajo",
                    self.text_2, self.painted
                ),
            }
        }
    }

    fn measure(b: &BenchOfTheMenu) -> Vec<Measure> {
        let mut v = Vec::new();
        for (i, f) in b.shapes_2.iter().enumerate() {
            let Shape2::Text {
                text_2,
                rect,
                glyph,
                colors_2,
                ..
            } = f
            else {
                continue;
            };
            // «Unaligned» es el rótulo de depuración que egui pinta en las compilaciones de
            // depuración cuando un rectángulo no cae en la rejilla de píxeles (a 1,25 el menú
            // no cae): no es texto de la interfaz y no lleva relleno.
            if text_2.trim().is_empty() || text_2 == "Unaligned" {
                continue;
            }
            let painted = glyph
                .or_else(|| colors_2.first().copied())
                .unwrap_or(Color32::PLACEHOLDER);
            let (row_2, background_2, screen, ratio) = match background_in(b, i) {
                Some((row_2, background_2)) => {
                    let screen = above(background_2, painted);
                    (
                        Some(row_2),
                        Some(background_2),
                        Some(screen),
                        Some(contrast_ratio(screen, background_2)),
                    )
                }
                None => (None, None, None, None),
            };
            v.push(Measure {
                text_2: text_2.clone(),
                rect: *rect,
                painted,
                row_2,
                background_2,
                screen,
                ratio,
            });
        }
        v
    }

    /// El estado de un recorrido: la escena, dónde está el ratón y todo lo pintado.
    #[derive(Clone, Debug)]
    struct State {
        scene: Scene,
        /// Rótulo de la fila bajo el ratón; `None` es el ratón fuera de la ventana (reposo).
        mouse: Option<String>,
        measures: Vec<Measure>,
    }

    struct Mesa {
        b: BenchOfTheMenu,
        platform: Platform,
    }

    impl Mesa {
        fn new(ppp: f32, background_2: CanvasBackground, locks: Locks, platform: Platform) -> Mesa {
            let ctx = context(ppp);
            ctx.set_visuals_of(egui::Theme::Dark, visuals(ppp));
            let mut mesa = Mesa {
                b: BenchOfTheMenu {
                    ctx,
                    background_2,
                    locks,
                    shapes_2: Vec::new(),
                    chosen_2: None,
                    time_2: 0.0,
                },
                platform,
            };
            mesa.rest_2_2();
            mesa
        }

        /// Un fotograma como el de `BenchOfTheMenu::frame`, con la plataforma de la mesa.
        fn frame(&mut self, events: Vec<Event>) {
            let b = &mut self.b;
            let state_2 = UiState {
                can_undo: true,
                can_redo: true,
                zoom: Zoom::from_int(5),
                tool: Tool::Pencil,
                locks: b.locks,
            };
            let background_2 = b.background_2;
            let platform = self.platform;
            b.time_2 += 0.25;
            let input_2 = RawInput {
                screen_rect: Some(Rect::from_min_size(Pos2::ZERO, SCREEN_OF_THE_MENU)),
                events,
                time: Some(b.time_2),
                ..RawInput::default()
            };
            let mut output_2 = b.ctx.run_ui(input_2, |ui| {
                apply_scale(ui.ctx());
                show_menu_with_background(ui, &state_2, platform, background_2);
            });
            output_2.textures_delta.clear();
            b.shapes_2 = shapes_in_order(&output_2.shapes);
        }

        fn rest_2_2(&mut self) {
            for _ in 0..6 {
                self.frame(vec![]);
            }
        }

        fn click(&mut self, pos: Pos2) {
            self.frame(vec![move_pointer(pos)]);
            self.frame(vec![button_2(pos, true)]);
            self.frame(vec![button_2(pos, false)]);
        }

        fn center(&self, text_2: &str) -> Pos2 {
            rect_exact(&self.b, text_2).center()
        }

        /// Barra con `scene` abierta con clics reales (el menú y, si toca, el submenú).
        fn open_2(
            ppp: f32,
            background_2: CanvasBackground,
            locks: Locks,
            platform: Platform,
            scene: Scene,
        ) -> Mesa {
            let mut mesa = Mesa::new(ppp, background_2, locks, platform);
            let title = mesa.center(scene.menu);
            mesa.click(title);
            mesa.rest_2_2();
            if scene.submenu {
                let submenu = mesa.center("Fondo del lienzo");
                mesa.click(submenu);
                mesa.rest_2_2();
            }
            mesa
        }

        /// Mide la escena con el ratón sobre la fila `mouse` (o fuera, si es `None`).
        fn state_2(&mut self, scene: Scene, mouse: Option<&str>) -> State {
            match mouse {
                None => self.frame(vec![Event::PointerGone]),
                Some(row_2) => {
                    let pos = self.center(row_2);
                    self.frame(vec![move_pointer(pos)]);
                }
            }
            self.frame(vec![]);
            self.frame(vec![]);
            State {
                scene,
                mouse: mouse.map(str::to_owned),
                measures: measure(&self.b),
            }
        }
    }

    /// Reposo (ratón fuera) y luego el ratón sobre cada fila, una a una.
    fn walk_scene(
        ppp: f32,
        background_2: CanvasBackground,
        locks: Locks,
        platform: Platform,
        scene: Scene,
    ) -> Vec<State> {
        let mut mesa = Mesa::open_2(ppp, background_2, locks, platform, scene);
        let mut states = vec![mesa.state_2(scene, None)];
        for row_2 in rows_of(scene, background_2) {
            states.push(mesa.state_2(scene, Some(&row_2)));
        }
        states
    }

    fn walk(
        ppp: f32,
        background_2: CanvasBackground,
        locks: Locks,
        platform: Platform,
    ) -> Vec<State> {
        ESCENAS
            .iter()
            .flat_map(|e| walk_scene(ppp, background_2, locks, platform, *e))
            .collect()
    }

    /// Reparte las escalas en hilos (cada una con su contexto de egui).
    fn in_parallel<T: Sync, R: Send>(data: &[T], f: impl Fn(&T) -> R + Sync) -> Vec<R> {
        std::thread::scope(|s| {
            let threads: Vec<_> = data.iter().map(|d| s.spawn(|| f(d))).collect();
            threads
                .into_iter()
                .map(|h| h.join().expect("un hilo del recorrido ha fallado"))
                .collect()
        })
    }

    fn state_of_ui(locks: Locks) -> UiState {
        UiState {
            can_undo: true,
            can_redo: true,
            zoom: Zoom::from_int(5),
            tool: Tool::Pencil,
            locks,
        }
    }

    // -----------------------------------------------------------------------------------------
    // Informe de fallos (compacto: sin repetir el mismo par cien veces)
    // -----------------------------------------------------------------------------------------

    #[derive(Default)]
    struct Failures {
        /// Clave del par (texto, relleno, resultado) -> (veces, primer sitio donde ocurre).
        per_par: BTreeMap<String, (usize, String)>,
    }

    impl Failures {
        fn notes(&mut self, location: String, m: &Measure) {
            self.per_par
                .entry(m.describe())
                .and_modify(|(n, _)| *n += 1)
                .or_insert((1, location));
        }

        fn notes_text(&mut self, location: String, message: String) {
            self.per_par
                .entry(message)
                .and_modify(|(n, _)| *n += 1)
                .or_insert((1, location));
        }

        fn blend_2(&mut self, other_2: Failures) {
            for (k, (n, location)) in other_2.per_par {
                self.per_par
                    .entry(k)
                    .and_modify(|(m, _)| *m += n)
                    .or_insert((n, location));
            }
        }

        fn this_empty(&self) -> bool {
            self.per_par.is_empty()
        }

        fn summary_2(&self) -> String {
            let total: usize = self.per_par.values().map(|(n, _)| n).sum();
            let mut s = format!(
                "{} pares distintos fallan ({total} apariciones):",
                self.per_par.len()
            );
            for (par, (n, location)) in self.per_par.iter().take(14) {
                s.push_str(&format!("\n  x{n}  {par}  [p. ej. {location}]"));
            }
            s
        }
    }

    fn location(ppp: f32, background_2: CanvasBackground, e: &State) -> String {
        format!(
            "ppp {ppp}, fondo {background_2:?}, {}, ratón en {:?}",
            e.scene.name_2, e.mouse
        )
    }

    // -----------------------------------------------------------------------------------------
    // Fila 1: recorrido de todo texto, en reposo y con el ratón
    // -----------------------------------------------------------------------------------------

    #[test]
    fn row_1_all_text_of_the_menus_has_contrast_4_5_over_its_real_fill() {
        let results = in_parallel(&PPPS, |&ppp| {
            let mut failures = Failures::default();
            let mut pairs = 0usize;
            let mut different = BTreeSet::new();
            for background_2 in ACTIVE {
                for e in walk(ppp, background_2, without_locks(), Platform::Linux) {
                    for m in &e.measures {
                        pairs += 1;
                        different.insert((
                            m.text_2.clone(),
                            m.background_2.map(|f| f.to_array()),
                            m.painted.to_array(),
                        ));
                        match m.ratio {
                            None => failures.notes(location(ppp, background_2, &e), m),
                            Some(r) if r < TEST_MIN_TEXT => {
                                failures.notes(location(ppp, background_2, &e), m)
                            }
                            Some(_) => {}
                        }
                    }
                }
            }
            (failures, pairs, different)
        });
        let mut failures = Failures::default();
        let mut pairs = 0;
        let mut different = BTreeSet::new();
        for (f, n, d) in results {
            failures.blend_2(f);
            pairs += n;
            different.extend(d);
        }
        eprintln!(
            "fila 1: {pairs} medidas texto-fondo, {} pares (texto, relleno) distintos, 6 fondos x 5 escalas",
            different.len()
        );
        assert!(
            pairs >= MINIMUM_PAIRS_ROW_1,
            "el recorrido solo midió {pairs} pares: el banco no está abriendo los menús"
        );
        assert!(failures.this_empty(), "{}", failures.summary_2());
    }

    // -----------------------------------------------------------------------------------------
    // Fila 2: la entrada marcada
    // -----------------------------------------------------------------------------------------

    #[test]
    fn row_2_the_marked_one_is_surface_0_over_accent_the_others_text_and_the_value_surface_0() {
        let results = in_parallel(&PPPS, |&ppp| {
            let mut failures = Failures::default();
            let mut states_seen = 0usize;
            for background_2 in ACTIVE {
                let states = walk_scene(
                    ppp,
                    background_2,
                    without_locks(),
                    Platform::Linux,
                    ESC_BACKGROUND,
                );
                for e in &states {
                    states_seen += 1;
                    let place = location(ppp, background_2, e);
                    // Una sola fila sobre ACCENT en todo lo pintado.
                    let rows_accent: BTreeSet<[u32; 4]> = e
                        .measures
                        .iter()
                        .filter(|m| m.background_2 == Some(ACCENT))
                        .filter_map(|m| m.row_2)
                        .map(|r| [r.min.x, r.min.y, r.max.x, r.max.y].map(f32::to_bits))
                        .collect();
                    if rows_accent.len() != 1 {
                        failures.notes_text(
                            place.clone(),
                            format!(
                                "hay {} filas sobre ACCENT y debía haber 1",
                                rows_accent.len()
                            ),
                        );
                    }
                    for item in the_background_submenu().items {
                        let label = mark(background_2, item);
                        let marked = background_checked(item.action, background_2);
                        let Some(m) = e.measures.iter().find(|m| m.text_2 == label) else {
                            failures.notes_text(place.clone(), format!("no se pinta {label:?}"));
                            continue;
                        };
                        let (color, base) = if marked {
                            (SURFACE_0, Some(ACCENT))
                        } else {
                            (TEXT, None)
                        };
                        if m.screen != Some(color) || m.painted != color {
                            failures.notes(place.clone(), m);
                        }
                        if let Some(base) = base
                            && m.background_2 != Some(base)
                        {
                            failures.notes(place.clone(), m);
                        }
                        match m.ratio {
                            Some(r) if r >= TEST_MIN_TEXT => {}
                            _ => failures.notes(place.clone(), m),
                        }
                        if marked {
                            // SURFACE_0 sobre ACCENT: 8,98.
                            match m.ratio {
                                Some(r) if (r - 8.98).abs() < 0.01 => {}
                                _ => failures.notes(place.clone(), m),
                            }
                        }
                    }
                    // El valor de `Color…`, solo con un color propio, en SURFACE_0.
                    if let Some(value) = background_value_text(background_2) {
                        match e.measures.iter().find(|m| m.text_2 == value) {
                            Some(m) => {
                                if m.screen != Some(SURFACE_0) || m.background_2 != Some(ACCENT) {
                                    failures.notes(place.clone(), m);
                                }
                            }
                            None => failures.notes_text(
                                place.clone(),
                                format!("no se pinta el valor {value:?}"),
                            ),
                        }
                    }
                }
            }
            (failures, states_seen)
        });
        let mut failures = Failures::default();
        for (f, n) in results {
            assert_eq!(n, 6 * 6, "seis fondos x (reposo + cinco filas)");
            failures.blend_2(f);
        }
        assert!(failures.this_empty(), "{}", failures.summary_2());
    }

    // -----------------------------------------------------------------------------------------
    // Fila 3: los atajos, en TEXT opaco
    // -----------------------------------------------------------------------------------------

    fn scene_of_the_menu(title: &str) -> Scene {
        *ESCENAS
            .iter()
            .find(|e| e.menu == title && !e.submenu)
            .expect("escena del menú")
    }

    #[test]
    fn row_3_the_shortcuts_are_painted_in_opaque_text_with_4_5_at_rest_and_with_the_mouse() {
        // `show_menu_with_background` recibe la plataforma, así que las tres se pintan de verdad.
        let results = in_parallel(&PPPS, |&ppp| {
            let mut failures = Failures::default();
            let mut per_platform = Vec::new();
            for platform in PLATFORMS {
                let mut n = 0usize;
                for title in ["Archivo", "Editar", "Ver"] {
                    let scene = scene_of_the_menu(title);
                    let mut mesa = Mesa::open_2(
                        ppp,
                        CanvasBackground::Checker,
                        without_locks(),
                        platform,
                        scene,
                    );
                    for item in el_menu(title).items {
                        let Some(shortcut_2) = shortcut_for(item.action, platform) else {
                            continue;
                        };
                        let text_2 = shortcut_text(shortcut_2, platform);
                        n += 1;
                        for mouse in [None, Some(item.label)] {
                            let e = mesa.state_2(scene, mouse);
                            let place =
                                format!("{platform:?}, {}", location(ppp, e_background(), &e));
                            let Some(m) = e.measures.iter().find(|m| m.text_2 == text_2) else {
                                failures
                                    .notes_text(place, format!("no se pinta el atajo {text_2:?}"));
                                continue;
                            };
                            if m.painted != TEXT || m.painted.a() != 255 {
                                failures.notes(place.clone(), m);
                            }
                            match m.ratio {
                                Some(r) if r >= TEST_MIN_TEXT => {}
                                _ => failures.notes(place, m),
                            }
                        }
                    }
                }
                per_platform.push((platform, n));
            }
            (failures, per_platform)
        });
        let mut failures = Failures::default();
        for (f, per_platform) in results {
            failures.blend_2(f);
            // Los atajos de la tabla: 12 en Linux (con `Salir`), 11 en Windows y en macOS (con
            // `Borrar página`, de I.2a).
            for (platform, n) in per_platform {
                let expected_2_2 = if platform == Platform::Linux { 12 } else { 11 };
                assert_eq!(n, expected_2_2, "atajos de la tabla en {platform:?}");
            }
        }
        assert!(failures.this_empty(), "{}", failures.summary_2());
    }

    fn e_background() -> CanvasBackground {
        CanvasBackground::Checker
    }

    // -----------------------------------------------------------------------------------------
    // Fila 4: lo desactivado
    // -----------------------------------------------------------------------------------------
    //
    // Cifras medidas en la spec (D4), anotadas aquí y NO fijadas con una igualdad: el rótulo
    // desactivado da 3,72:1, el atajo desactivado 3,72:1 (2,26 hoy; con los atajos en `TEXT`) y el
    // rótulo marcado de `Color…` desactivado 2,00:1 (hoy 1,06, porque va en `TEXT` sobre `ACCENT`
    // al 50 %). Lo que se exige es el suelo de 2,0 (con `>=`), que cada par sea estrictamente
    // menor que el mismo par habilitado y que lo único bajo 4,5 sea desactivado.

    fn locks_of_row_4() -> [(&'static str, Locks); 3] {
        [
            (
                "dialog",
                Locks {
                    dialog: true,
                    ..Locks::default()
                },
            ),
            (
                "gesture",
                Locks {
                    gesture: true,
                    ..Locks::default()
                },
            ),
            (
                "text_focus",
                Locks {
                    text_focus: true,
                    ..Locks::default()
                },
            ),
        ]
    }

    /// Fondos de la fila 4: uno fijo (la marcada es una fila de color) y uno propio (la marcada es
    /// `Color…`, la que más baja da).
    const BACKGROUNDS_OF_ROW_4: [CanvasBackground; 2] = [
        CanvasBackground::Checker,
        CanvasBackground::Custom(BLUE_STEEL),
    ];

    /// `m` está en una fila cuyo rótulo es de una acción deshabilitada con `locks`.
    fn disabled(m: &Measure, e: &State, state: &UiState) -> bool {
        let Some(row_2) = m.row_2 else {
            return false;
        };
        e.measures.iter().any(|o| {
            o.row_2 == Some(row_2) && action_of_label(&o.text_2).is_some_and(|a| !enabled(a, state))
        })
    }

    /// `m` está en una fila marcada del submenú de fondo (su rótulo lleva la marca `· `).
    fn of_row_marked(m: &Measure, e: &State) -> bool {
        let Some(row_2) = m.row_2 else {
            return false;
        };
        e.measures
            .iter()
            .any(|o| o.row_2 == Some(row_2) && o.text_2.starts_with("· "))
    }

    /// Recorre los tres bloqueos y compara cada par desactivado (de filas marcadas o de las no
    /// marcadas, según `marked_2`) con el mismo par habilitado, y con el suelo de D4.
    fn checks_disabled(marked_2: bool) -> (Failures, usize) {
        let results = in_parallel(&PPPS, |&ppp| {
            let mut failures = Failures::default();
            let mut disabled_2 = 0usize;
            for background_2 in BACKGROUNDS_OF_ROW_4 {
                let free = walk(ppp, background_2, without_locks(), Platform::Linux);
                for (name_2, locks) in locks_of_row_4() {
                    let state = state_of_ui(locks);
                    let with = walk(ppp, background_2, locks, Platform::Linux);
                    assert_eq!(free.len(), with.len());
                    for (a, b) in free.iter().zip(&with) {
                        assert_eq!(a.measures.len(), b.measures.len(), "{name_2}");
                        for (enabled_2, m) in a.measures.iter().zip(&b.measures) {
                            assert_eq!(
                                enabled_2.text_2, m.text_2,
                                "{name_2}: mismo orden de textos"
                            );
                            if !disabled(m, b, &state) || of_row_marked(m, b) != marked_2 {
                                continue;
                            }
                            disabled_2 += 1;
                            let place = format!("{name_2}: {}", location(ppp, background_2, b));
                            let (Some(r), Some(rh)) = (m.ratio, enabled_2.ratio) else {
                                failures.notes(place, m);
                                continue;
                            };
                            if r >= rh {
                                failures.notes_text(
                                    place.clone(),
                                    format!(
                                        "{:?} desactivado da {r:.2} y habilitado {rh:.2}: no se nota atenuado",
                                        m.text_2
                                    ),
                                );
                            }
                            // Suelo de D4 (comparación `>=`, matiz del visto bueno).
                            if r < FLOOR_DISABLED {
                                failures.notes_text(
                                    place,
                                    format!(
                                        "{:?} desactivado da {r:.2}, por debajo del suelo de {FLOOR_DISABLED}",
                                        m.text_2
                                    ),
                                );
                            }
                        }
                    }
                }
            }
            (failures, disabled_2)
        });
        let mut failures = Failures::default();
        let mut total = 0;
        for (f, n) in results {
            failures.blend_2(f);
            total += n;
        }
        (failures, total)
    }

    #[test]
    fn row_4_each_label_and_disabled_shortcut_is_less_than_the_enabled_one_and_at_least_2_0() {
        let (failures, n) = checks_disabled(false);
        assert!(
            n > 1000,
            "el recorrido solo encontró {n} pares desactivados"
        );
        assert!(failures.this_empty(), "{}", failures.summary_2());
    }

    #[test]
    fn row_4_the_marked_disabled_row_is_less_than_the_enabled_one_and_at_least_2_0() {
        let (failures, n) = checks_disabled(true);
        assert!(
            n > 100,
            "el recorrido solo encontró {n} pares marcados desactivados"
        );
        assert!(failures.this_empty(), "{}", failures.summary_2());
    }

    #[test]
    fn row_4_the_pairs_below_4_5_are_only_among_the_disabled_ones() {
        let results = in_parallel(&PPPS, |&ppp| {
            let mut failures = Failures::default();
            for background_2 in BACKGROUNDS_OF_ROW_4 {
                for (name_2, locks) in locks_of_row_4() {
                    let state = state_of_ui(locks);
                    for e in walk(ppp, background_2, locks, Platform::Linux) {
                        for m in &e.measures {
                            let under = m.ratio.is_none_or(|r| r < TEST_MIN_TEXT);
                            if under && !disabled(m, &e, &state) {
                                failures.notes(
                                    format!("{name_2}: {}", location(ppp, background_2, &e)),
                                    m,
                                );
                            }
                        }
                    }
                }
            }
            failures
        });
        let mut failures = Failures::default();
        for f in results {
            failures.blend_2(f);
        }
        assert!(
            failures.this_empty(),
            "pares habilitados bajo 4,5 con un bloqueo puesto: {}",
            failures.summary_2()
        );
    }

    // -----------------------------------------------------------------------------------------
    // Fila 5: nada más cambia
    // -----------------------------------------------------------------------------------------

    #[test]
    fn row_5_override_text_color_is_still_text_and_the_submenu_measures_131_u() {
        for ppp in PPPS {
            assert_eq!(
                visuals(ppp).override_text_color,
                Some(TEXT),
                "ppp {ppp}: el color de texto forzado no cambia"
            );
        }
        assert_eq!(submenu_width_u(the_background_submenu()), 131);
    }

    #[test]
    fn row_5_positions_and_sizes_of_each_text_follow_the_geometry_of_the_17() {
        let failures = in_parallel(&PPPS, |&ppp| {
            let u = ui_unit(ppp);
            // A 1,25 egui coloca el panel y las filas fuera de la rejilla de píxeles (por eso pinta
            // «Unaligned»): las filas miden hasta medio píxel físico más y los márgenes un cuarto.
            // Es el comportamiento de siempre; a ese ppp se tolera un píxel físico.
            let tol = if ppp == 1.25 {
                1.0 / ppp
            } else {
                tolerance_of_egui(ppp) / ppp
            };
            let mut failures = Failures::default();
            let mut seen_2 = 0usize;
            for background_2 in BACKGROUNDS_OF_ROW_4 {
                for scene in ESCENAS {
                    let mut mesa =
                        Mesa::open_2(ppp, background_2, without_locks(), Platform::Linux, scene);
                    let e = mesa.state_2(scene, None);
                    for m in &e.measures {
                        seen_2 += 1;
                        let place = location(ppp, background_2, &e);
                        let Some(row_2) = m.row_2 else {
                            failures.notes_text(place, format!("{:?} sin fila", m.text_2));
                            continue;
                        };
                        let is_title = MENUS.iter().any(|menu| menu.title == m.text_2);
                        let is_label = is_title
                            || m.text_2 == "Fondo del lienzo"
                            || action_of_label(&m.text_2).is_some();
                        let characters = m.text_2.chars().count() as f32;
                        let mut checks = |name_2: &str, real: f32, expected_2: f32| {
                            if (real - expected_2).abs() > tol {
                                failures.notes_text(
                                    place.clone(),
                                    format!(
                                        "{:?}: {name_2} mide {real} y debía medir {expected_2} (u = {u})",
                                        m.text_2
                                    ),
                                );
                            }
                        };
                        // La caja del texto: 7 u por carácter y 14 u de alto.
                        checks("ancho del texto", m.rect.width(), characters * 7.0 * u);
                        checks("alto del texto", m.rect.height(), 14.0 * u);
                        // La fila: 16 u de alto y el texto 1 u por debajo de su borde.
                        // (La fila del submenú `Fondo del canvas_2` a 1,25 mide 16 pt y no 12,8: la
                        // flecha y el redondeo de egui; es así hoy y solo a ese ppp.)
                        if !(ppp == 1.25 && m.text_2 == "Fondo del lienzo") {
                            checks("alto de la fila", row_2.height(), 16.0 * u);
                            checks("margen superior", m.rect.min.y - row_2.min.y, u);
                        }
                        // Rótulos con 6 u a la izquierda; atajos y valor con 6 u a la derecha.
                        if is_label {
                            checks("margen izquierdo", m.rect.min.x - row_2.min.x, 6.0 * u);
                        } else {
                            checks("margen derecho", row_2.max.x - m.rect.max.x, 6.0 * u);
                        }
                        // El ancho de la fila.
                        let width_u = if is_title {
                            Some(12.0 + characters * 7.0)
                        } else {
                            match action_of_label(&m.text_2) {
                                Some(
                                    AppAction::CanvasBackground(_) | AppAction::BackgroundColor,
                                ) => Some(submenu_width_u(the_background_submenu()) as f32),
                                Some(_) => {
                                    Some(popup_width_u(el_menu(scene.menu), Platform::Linux) as f32)
                                }
                                None if m.text_2 == "Fondo del lienzo" => {
                                    Some(popup_width_u(el_menu("Ver"), Platform::Linux) as f32)
                                }
                                None => None,
                            }
                        };
                        if let Some(width_u) = width_u {
                            checks("ancho de la fila", row_2.width(), width_u * u);
                        }
                    }
                }
            }
            (failures, seen_2)
        });
        let mut all_2 = Failures::default();
        for (f, seen_2) in failures {
            assert!(seen_2 > 100, "solo se midieron {seen_2} textos");
            all_2.blend_2(f);
        }
        assert!(all_2.this_empty(), "{}", all_2.summary_2());
    }

    // -----------------------------------------------------------------------------------------
    // Propiedades (32 casos; `PROPTEST_CASES` baja o sube el número al desarrollar)
    // -----------------------------------------------------------------------------------------

    fn cases() -> u32 {
        std::env::var("PROPTEST_CASES")
            .ok()
            .and_then(|v| v.parse().ok())
            .unwrap_or(32)
    }

    fn background_any() -> impl Strategy<Value = CanvasBackground> {
        prop_oneof![
            Just(CanvasBackground::Checker),
            Just(CanvasBackground::Black),
            Just(CanvasBackground::Gray),
            Just(CanvasBackground::White),
            (any::<u8>(), any::<u8>(), any::<u8>())
                .prop_map(|(r, g, b)| CanvasBackground::Custom(Rgba::opaque(r, g, b))),
        ]
    }

    /// Dos menús abiertos con clics (el submenú de fondo y `Archivo`), con el ratón sobre la fila
    /// `row_2` (módulo el número de filas de cada uno).
    fn two_menus(ppp: f32, background_2: CanvasBackground, row_2: usize) -> Vec<State> {
        [ESC_BACKGROUND, ESC_FILE]
            .into_iter()
            .map(|scene| {
                let rows_2 = rows_of(scene, background_2);
                let mouse = rows_2[row_2 % rows_2.len()].clone();
                let mut mesa =
                    Mesa::open_2(ppp, background_2, without_locks(), Platform::Linux, scene);
                mesa.state_2(scene, Some(&mouse))
            })
            .collect()
    }

    proptest! {
        #![proptest_config(ProptestConfig {
            cases: cases(),
            failure_persistence: None,
            ..ProptestConfig::default()
        })]

        /// Contraste >= 4,5 de todo par texto-fondo habilitado con cualquier fondo activo (con
        /// `Custom` de RGB opaco aleatorio), cualquier escala y cualquier fila bajo el ratón.
        #[test]
        fn property_1_the_enabled_contrast_is_at_least_4_5_with_any_background(
            background_2 in background_any(),
            scale in 0..PPPS.len(),
            row_2 in 0usize..64,
        ) {
            let ppp = PPPS[scale];
            let mut failures = Failures::default();
            for e in two_menus(ppp, background_2, row_2) {
                for m in &e.measures {
                    if m.ratio.is_none_or(|r| r < TEST_MIN_TEXT) {
                        failures.notes(location(ppp, background_2, &e), m);
                    }
                }
            }
            prop_assert!(failures.this_empty(), "{}", failures.summary_2());
        }

        /// El color pintado depende solo del estado: `SURFACE_0` el rótulo marcado y el valor,
        /// `TEXT` todo lo demás (rótulos, atajos, títulos), sin mirar fondo, escala ni fila.
        #[test]
        fn property_2_the_text_color_depends_only_on_the_state(
            background_2 in background_any(),
            scale in 0..PPPS.len(),
            row_2 in 0usize..64,
        ) {
            let ppp = PPPS[scale];
            let marked = the_background_submenu()
                .items
                .iter()
                .find(|i| background_checked(i.action, background_2))
                .map(|i| mark(background_2, i))
                .expect("siempre hay una marcada");
            let value = background_value_text(background_2);
            let mut failures = Failures::default();
            for e in two_menus(ppp, background_2, row_2) {
                for m in &e.measures {
                    let expected_2 = if m.text_2 == marked || value.as_deref() == Some(m.text_2.as_str()) {
                        SURFACE_0
                    } else {
                        TEXT
                    };
                    if m.painted != expected_2 || m.painted.a() != 255 {
                        failures.notes(location(ppp, background_2, &e), m);
                    }
                }
            }
            prop_assert!(failures.this_empty(), "debían ser {SURFACE_0:?} o {TEXT:?}: {}", failures.summary_2());
        }
    }
}
