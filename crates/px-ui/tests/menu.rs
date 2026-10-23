//! Tests de la tabla del menú `px_ui::menu` (specs/hito-2-menu-herramientas.md, «Casos» filas 1 y 2
//! y propiedad 1). Sin ventana: `MENUS`, `shortcut_for`, `shortcut_text` y `enabled` son lógica
//! pura; solo la comprobación de que ningún rótulo pierde un carácter con la fuente real usa un
//! `egui::Context` (sin GPU).
//!
//! - Fila 1: los cinco menús (con `TestImage` de la unidad 20) con sus rótulos y acciones, el separador solo antes de `Salir`, los
//!   atajos por `Platform::{Linux, Windows, Mac}` (`Salir` solo con `Ctrl+Q` en Linux; `Cmd` en
//!   macOS y `Redo` = `Cmd+Mayús+Z`), los rótulos contra `INTERFACE_CHARSET`, ningún `?` por
//!   `replace_missing` y ninguna acción repetida.
//! - Fila 2: `enabled` de las 11 acciones de la tabla con historia vacía, solo deshacer, solo
//!   rehacer y zoom `None`, `Zoom::MIN`, ×5 y `Zoom::MAX`.
//! - Propiedad 1: tabla coherente (menú, botones de herramienta y botones de zoom).
//!
//! Suposiciones sobre las firmas de «Piezas» (derives no listados): `Shortcut`, `Menu` y
//! `MenuItem` son `Clone, Copy, Debug, PartialEq, Eq`; `Platform`, `Origin` y `Locks` también.

use egui::Context;
use proptest::prelude::*;
use px_ui::actions::{AppAction, Locks, Origin, Platform, UiState, enabled, is_blocked};
use px_ui::help::HelpTarget;
use px_ui::keymap::{KeyId, Keymap, Press};
use px_ui::menu::{MENUS, Menu, MenuItem, Shortcut, shortcut_for, shortcut_text};
use px_ui::text::{INTERFACE_CHARSET, install_fonts, printable, replace_missing};
use px_ui::tool::Tool;
use px_ui::toolbar::{TOOL_BUTTONS, ZOOM_BUTTONS};
use px_ui::view::Zoom;

const PLATFORMS: [Platform; 3] = [Platform::Linux, Platform::Windows, Platform::Mac];
const TEST_TOOLS: [Tool; 5] = [
    Tool::Pencil,
    Tool::Line,
    Tool::Fill,
    Tool::Dropper,
    Tool::Eraser,
];

/// Las 11 acciones de la tabla, en el orden del menú.
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
    AppAction::ZoomActualSize,
    AppAction::ShowLicenses,
];

/// Las entradas de todos los menús, aplanadas.
fn entries() -> Vec<MenuItem> {
    MENUS.iter().flat_map(|m| m.items.iter().copied()).collect()
}

fn title(menu: &Menu) -> &'static str {
    menu.title
}

fn shortcut_2(key: KeyId, command: bool, shift: bool) -> Option<Shortcut> {
    Some(Shortcut {
        key,
        command,
        shift,
    })
}

// ---------------------------------------------------------------------------------------------
// Fila 1: la tabla
// ---------------------------------------------------------------------------------------------

#[test]
fn row_1_five_menus_in_order_with_their_titles() {
    let titles: Vec<&str> = MENUS.iter().map(title).collect();
    assert_eq!(
        titles,
        ["Archivo", "Editar", "Imagen", "Capa", "Ver", "Ayuda"]
    );
}

#[test]
fn row_1_each_menu_has_the_labels_and_actions_of_decision_2() {
    let expected_2: [(&str, Vec<(&str, AppAction)>); 6] = [
        (
            "Archivo",
            vec![
                ("Nueva imagen…", AppAction::NewImage),
                ("Abrir…", AppAction::Open),
                ("Guardar", AppAction::Save),
                ("Guardar como…", AppAction::SaveAs),
                ("Exportar PNG aplanado…", AppAction::ExportFlat),
                ("Salir", AppAction::Quit),
            ],
        ),
        (
            "Editar",
            vec![
                ("Deshacer", AppAction::Undo),
                ("Rehacer", AppAction::Redo),
                ("Borrar página", AppAction::ClearPage),
                ("Intercambiar colores", AppAction::SwapColors),
            ],
        ),
        (
            "Imagen",
            vec![
                ("Convertir modo…", AppAction::ConvertMode),
                ("Añadir entrada", AppAction::AddPaletteEntry),
                ("Quitar entrada", AppAction::RemovePaletteEntry),
                ("Tamaño de la paleta…", AppAction::PaletteSize),
            ],
        ),
        (
            "Capa",
            vec![
                ("Nueva capa", AppAction::NewLayer),
                ("Duplicar", AppAction::DuplicateLayer),
                ("Quitar", AppAction::RemoveLayer),
                ("Subir", AppAction::LayerUp),
                ("Bajar", AppAction::LayerDown),
                ("Combinar abajo", AppAction::MergeLayerDown),
                ("Combinar visibles", AppAction::MergeVisibleLayers),
                ("Aplanar", AppAction::FlattenLayers),
                ("Renombrar", AppAction::RenameLayer),
                ("Propiedades", AppAction::LayerProperties),
                ("Mostrar panel", AppAction::ToggleLayerPanel),
            ],
        ),
        (
            "Ver",
            vec![
                ("Acercar", AppAction::ZoomIn),
                ("Alejar", AppAction::ZoomOut),
                ("Ajustar a la ventana", AppAction::ZoomFit),
                ("1:1", AppAction::ZoomActualSize),
                ("Editor de color", AppAction::ToggleColorEditor),
            ],
        ),
        (
            "Ayuda",
            vec![
                (
                    "Ayuda de la herramienta",
                    AppAction::ShowHelp(HelpTarget::Active),
                ),
                ("Atajos de teclado…", AppAction::ShowShortcuts),
                ("Licencias", AppAction::ShowLicenses),
            ],
        ),
    ];
    for (menu, (title, items)) in MENUS.iter().zip(expected_2) {
        assert_eq!(menu.title, title);
        let real: Vec<(&str, AppAction)> = menu.items.iter().map(|i| (i.label, i.action)).collect();
        assert_eq!(real, items, "menú {title}");
    }
}

#[test]
fn row_1_the_separator_goes_only_before_quit() {
    for menu in MENUS.iter() {
        for item in menu.items {
            assert_eq!(
                item.separator_before,
                item.action == AppAction::Quit,
                "{} / {}",
                menu.title,
                item.label
            );
        }
    }
}

#[test]
fn row_1_there_are_thirty_three_entries_and_no_action_repeats() {
    let actions: Vec<AppAction> = entries().iter().map(|i| i.action).collect();
    assert_eq!(actions.len(), 33);
    for (i, a) in actions.iter().enumerate() {
        assert!(
            !actions[i + 1..].contains(a),
            "la acción {a:?} aparece dos veces"
        );
    }
    // `Nueva image_2…` (unidad 19) es la primera y `Convertir mode_2…` (unidad 20) va tras
    // `Redo`; las otras 11 siguen siendo las de la 17.
    let mut expected_2 = vec![AppAction::NewImage];
    // `Exportar PNG aplanado…` (3.5) va tras `Guardar as_word…`, antes de `Salir`.
    expected_2.extend(&TABLE_ACTIONS[..3]);
    expected_2.push(AppAction::ExportFlat);
    expected_2.extend(&TABLE_ACTIONS[3..6]);
    // `Borrar página` (I.2a) cierra `Editar`, antes de `TestImage`.
    expected_2.push(AppAction::ClearPage);
    // `Intercambiar colores` (C4) follows `Borrar página`, still in `Editar`.
    expected_2.push(AppAction::SwapColors);
    expected_2.push(AppAction::ConvertMode);
    // The three palette actions of C3 follow `Convertir modo…`.
    expected_2.extend([
        AppAction::AddPaletteEntry,
        AppAction::RemovePaletteEntry,
        AppAction::PaletteSize,
    ]);
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
    // `Editor de color` (C1) closes `Ver`.
    expected_2.push(AppAction::ToggleColorEditor);
    // `Ayuda` (I.5): la ayuda de la herramienta y los atajos preceden a las licencias.
    expected_2.push(AppAction::ShowHelp(HelpTarget::Active));
    expected_2.push(AppAction::ShowShortcuts);
    expected_2.push(AppAction::ShowLicenses);
    assert_eq!(actions, expected_2);
}

#[test]
fn row_1_no_entry_or_title_is_empty_or_repeated() {
    let mut labels: Vec<&str> = MENUS.iter().map(title).collect();
    labels.extend(entries().iter().map(|i| i.label));
    for (i, r) in labels.iter().enumerate() {
        assert!(!r.is_empty());
        assert!(!labels[i + 1..].contains(r), "rótulo repetido: {r}");
    }
}

// ---------------------------------------------------------------------------------------------
// Fila 1: atajos por plataforma
// ---------------------------------------------------------------------------------------------

/// Texto del atajo de cada entrada, en el orden de `TABLE_ACTIONS`.
fn texts_of_shortcut(platform: Platform) -> Vec<Option<String>> {
    entries()
        .iter()
        .map(|i| shortcut_for(i.action, platform).map(|s| shortcut_text(s, platform)))
        .collect()
}

fn some(texts: [Option<&str>; 33]) -> Vec<Option<String>> {
    texts.iter().map(|t| t.map(str::to_string)).collect()
}

#[test]
fn row_1_shortcuts_on_linux() {
    assert_eq!(
        texts_of_shortcut(Platform::Linux),
        some([
            Some("Ctrl+N"),
            Some("Ctrl+O"),
            Some("Ctrl+S"),
            Some("Ctrl+Mayús+S"),
            None,
            Some("Ctrl+Q"),
            Some("Ctrl+Z"),
            Some("Ctrl+Y"),
            Some("K"),
            None,
            None,
            None,
            None,
            None,
            Some(";"),
            None,
            None,
            Some(")"),
            Some("("),
            None,
            None,
            None,
            None,
            None,
            Some("'"),
            Some(">"),
            Some("<"),
            Some("Inicio"),
            Some("Ctrl+1"),
            Some("p"),
            None,
            None,
            None,
        ])
    );
}

#[test]
fn row_1_shortcuts_on_windows_are_those_of_linux_except_quit_which_has_none() {
    assert_eq!(
        texts_of_shortcut(Platform::Windows),
        some([
            Some("Ctrl+N"),
            Some("Ctrl+O"),
            Some("Ctrl+S"),
            Some("Ctrl+Mayús+S"),
            None,
            None,
            Some("Ctrl+Z"),
            Some("Ctrl+Y"),
            Some("K"),
            None,
            None,
            None,
            None,
            None,
            Some(";"),
            None,
            None,
            Some(")"),
            Some("("),
            None,
            None,
            None,
            None,
            None,
            Some("'"),
            Some(">"),
            Some("<"),
            Some("Inicio"),
            Some("Ctrl+1"),
            Some("p"),
            None,
            None,
            None,
        ])
    );
    assert_eq!(shortcut_for(AppAction::Quit, Platform::Windows), None);
}

#[test]
fn row_1_shortcuts_on_mac_use_cmd_redo_is_cmd_shift_z_and_quit_has_none() {
    assert_eq!(
        texts_of_shortcut(Platform::Mac),
        some([
            Some("Cmd+N"),
            Some("Cmd+O"),
            Some("Cmd+S"),
            Some("Cmd+Mayús+S"),
            None,
            None,
            Some("Cmd+Z"),
            Some("Cmd+Mayús+Z"),
            Some("K"),
            None,
            None,
            None,
            None,
            None,
            Some(";"),
            None,
            None,
            Some(")"),
            Some("("),
            None,
            None,
            None,
            None,
            None,
            Some("'"),
            Some(">"),
            Some("<"),
            Some("Inicio"),
            Some("Cmd+1"),
            Some("p"),
            None,
            None,
            None,
        ])
    );
    assert_eq!(shortcut_for(AppAction::Quit, Platform::Mac), None);
}

#[test]
fn row_1_licenses_has_no_shortcut_on_any_platform() {
    for platform in PLATFORMS {
        assert_eq!(shortcut_for(AppAction::ShowLicenses, platform), None);
    }
}

#[test]
fn row_1_shortcut_for_gives_the_key_and_the_modifiers_of_each_entry() {
    for platform in PLATFORMS {
        let redo = if platform == Platform::Mac {
            shortcut_2(KeyId::Char('z'), true, true)
        } else {
            shortcut_2(KeyId::Char('y'), true, false)
        };
        let quit = (platform == Platform::Linux)
            .then(|| shortcut_2(KeyId::Char('q'), true, false))
            .flatten();
        let expected_2 = [
            (AppAction::Open, shortcut_2(KeyId::Char('o'), true, false)),
            (AppAction::Save, shortcut_2(KeyId::Char('s'), true, false)),
            (AppAction::SaveAs, shortcut_2(KeyId::Char('s'), true, true)),
            (AppAction::Quit, quit),
            (AppAction::Undo, shortcut_2(KeyId::Char('z'), true, false)),
            (AppAction::Redo, redo),
            (
                AppAction::ZoomIn,
                shortcut_2(KeyId::Char('>'), false, false),
            ),
            (
                AppAction::ZoomOut,
                shortcut_2(KeyId::Char('<'), false, false),
            ),
            (AppAction::ZoomFit, shortcut_2(KeyId::Home, false, false)),
            (
                AppAction::ZoomActualSize,
                shortcut_2(KeyId::Char('1'), true, false),
            ),
            (AppAction::ShowLicenses, None),
        ];
        for (action, shortcut) in expected_2 {
            assert_eq!(
                shortcut_for(action, platform),
                shortcut,
                "{action:?} en {platform:?}"
            );
        }
    }
}

#[test]
fn row_1_shortcut_text_of_each_form() {
    // Con `command`: `Ctrl+` en Linux y Windows, `Cmd+` en macOS; con `shift`, `Mayús+`.
    for (platform, command) in [
        (Platform::Linux, "Ctrl"),
        (Platform::Windows, "Ctrl"),
        (Platform::Mac, "Cmd"),
    ] {
        assert_eq!(
            shortcut_text(shortcut_2(KeyId::Char('o'), true, false).unwrap(), platform),
            format!("{command}+O")
        );
        assert_eq!(
            shortcut_text(shortcut_2(KeyId::Char('s'), true, true).unwrap(), platform),
            format!("{command}+Mayús+S")
        );
        assert_eq!(
            shortcut_text(shortcut_2(KeyId::Char('1'), true, false).unwrap(), platform),
            format!("{command}+1")
        );
        assert_eq!(
            shortcut_text(shortcut_2(KeyId::Char('z'), true, true).unwrap(), platform),
            format!("{command}+Mayús+Z")
        );
        // Sin `command` la tecla va sola, con su nombre en español o su signo.
        assert_eq!(
            shortcut_text(shortcut_2(KeyId::Home, false, false).unwrap(), platform),
            "Inicio"
        );
        assert_eq!(
            shortcut_text(
                shortcut_2(KeyId::Char('+'), false, false).unwrap(),
                platform
            ),
            "+"
        );
        assert_eq!(
            shortcut_text(
                shortcut_2(KeyId::Char('-'), false, false).unwrap(),
                platform
            ),
            "-"
        );
    }
}

// ---------------------------------------------------------------------------------------------
// Fila 1: rótulos y fuente
// ---------------------------------------------------------------------------------------------

/// Todo el texto fijo del menú: títulos, rótulos y los atajos de cada plataforma.
fn texts_fixed() -> Vec<String> {
    let mut v: Vec<String> = MENUS.iter().map(|m| m.title.to_string()).collect();
    for item in entries() {
        v.push(item.label.to_string());
        for platform in PLATFORMS {
            if let Some(s) = shortcut_for(item.action, platform) {
                v.push(shortcut_text(s, platform));
            }
        }
    }
    v
}

#[test]
fn row_1_all_the_menu_text_is_inside_interface_charset() {
    for text_2 in texts_fixed() {
        for c in text_2.chars() {
            assert!(
                INTERFACE_CHARSET.contains(c),
                "{c:?} de {text_2:?} no está en INTERFACE_CHARSET"
            );
        }
    }
}

#[test]
fn row_1_no_label_loses_a_character_through_replace_missing() {
    let has = |c: char| INTERFACE_CHARSET.contains(c);
    for text_2 in texts_fixed() {
        assert_eq!(replace_missing(&text_2, has), text_2);
        assert!(!text_2.contains('?'), "{text_2:?} lleva un `?`");
    }
}

#[test]
fn row_1_with_the_real_font_printable_changes_no_label() {
    let ctx = Context::default();
    install_fonts(&ctx);
    // Los cambios de fuente se aplican al empezar el pase siguiente.
    for _ in 0..2 {
        ctx.run_ui(egui::RawInput::default(), |_| {})
            .drop_without_applying_deltas();
    }
    for text_2 in texts_fixed() {
        assert_eq!(printable(&ctx, &text_2), text_2);
    }
}

// ---------------------------------------------------------------------------------------------
// Fila 2: habilitación
// ---------------------------------------------------------------------------------------------

fn state_2(can_undo: bool, can_redo: bool, zoom: Option<Zoom>) -> UiState {
    UiState {
        can_undo,
        can_redo,
        zoom,
        tool: Tool::Pencil,
        locks: Locks::default(),
    }
}

fn x5() -> Option<Zoom> {
    Zoom::from_int(5)
}

/// `enabled` de las 11 acciones de la tabla, en el orden de `TABLE_ACTIONS`.
fn enabled_2(state: &UiState) -> Vec<bool> {
    TABLE_ACTIONS.iter().map(|a| enabled(*a, state)).collect()
}

#[test]
fn row_2_with_the_history_empty_undo_and_redo_are_disabled() {
    // Orden: Open, Save, SaveAs, Quit, Undo, Redo, ZoomIn, ZoomOut, ZoomFit, ZoomActual, Licencias.
    assert_eq!(
        enabled_2(&state_2(false, false, x5())),
        [
            true, true, true, true, false, false, true, true, true, true, true
        ]
    );
}

#[test]
fn row_2_only_can_undo_enables_undo() {
    assert_eq!(
        enabled_2(&state_2(true, false, x5())),
        [
            true, true, true, true, true, false, true, true, true, true, true
        ]
    );
}

#[test]
fn row_2_only_can_redo_enables_redo() {
    assert_eq!(
        enabled_2(&state_2(false, true, x5())),
        [
            true, true, true, true, false, true, true, true, true, true, true
        ]
    );
}

#[test]
fn row_2_without_a_displayable_image_the_three_zoom_ones_and_1_to_1_are_disabled() {
    assert_eq!(
        enabled_2(&state_2(true, true, None)),
        [
            true, true, true, true, true, true, false, false, false, false, true
        ]
    );
}

#[test]
fn row_2_at_zoom_min_only_zoom_out_is_disabled() {
    // `Zoom::MIN` es 1/64 (antes ×1): ahí `Alejar` sigue deshabilitado, pero `1:1` ya vale
    // (el zoom no es ×1). La comparación usa la constante, no el número.
    assert_eq!(
        enabled_2(&state_2(true, true, Some(Zoom::MIN))),
        [
            true, true, true, true, true, true, true, false, true, true, true
        ]
    );
}

#[test]
fn row_2_at_x1_zoom_out_is_enabled_and_1_to_1_is_not() {
    // El estado que antes era el mínimo: ahora hay un nivel por debajo (1/2).
    assert_eq!(
        enabled_2(&state_2(true, true, Zoom::from_int(1))),
        [
            true, true, true, true, true, true, true, true, true, false, true
        ]
    );
}

#[test]
fn row_2_at_x5_all_the_zoom_is_enabled() {
    assert_eq!(enabled_2(&state_2(true, true, x5())), [true; 11]);
}

#[test]
fn row_2_at_zoom_max_only_zoom_in_is_disabled() {
    assert_eq!(
        enabled_2(&state_2(true, true, Some(Zoom::MAX))),
        [
            true, true, true, true, true, true, false, true, true, true, true
        ]
    );
    assert_eq!(Zoom::MAX, Zoom::from_int(64).expect("×64"));
}

#[test]
fn row_2_save_is_always_enabled_also_without_changes_or_history() {
    for can_undo in [false, true] {
        for can_redo in [false, true] {
            for zoom in [None, Some(Zoom::MIN), x5(), Some(Zoom::MAX)] {
                let state = state_2(can_undo, can_redo, zoom);
                assert!(enabled(AppAction::Save, &state), "{zoom:?}");
                for action in [
                    AppAction::Open,
                    AppAction::SaveAs,
                    AppAction::Quit,
                    AppAction::ShowLicenses,
                ] {
                    assert!(enabled(action, &state), "{action:?} con {zoom:?}");
                }
            }
        }
    }
}

#[test]
fn row_2_select_tool_depends_only_on_the_lock_also_the_active_tool() {
    for active_2 in TEST_TOOLS {
        let state = UiState {
            tool: active_2,
            ..state_2(false, false, None)
        };
        for tool in TEST_TOOLS {
            assert!(enabled(AppAction::SelectTool(tool), &state));
        }
    }
}

#[test]
fn row_2_zoom_in_is_false_only_at_x64_at_all_the_integer_levels() {
    for n in 1..=64u32 {
        let zoom = Zoom::from_int(n).expect("nivel válido");
        let state = state_2(true, true, Some(zoom));
        assert_eq!(enabled(AppAction::ZoomIn, &state), n != 64, "×{n}");
        assert_eq!(
            enabled(AppAction::ZoomOut, &state),
            zoom > Zoom::MIN,
            "×{n}"
        );
        assert_eq!(enabled(AppAction::ZoomActualSize, &state), n != 1, "×{n}");
        assert!(enabled(AppAction::ZoomFit, &state), "×{n}");
    }
}

// ---------------------------------------------------------------------------------------------
// Propiedad 1: tabla coherente
// ---------------------------------------------------------------------------------------------

/// La pulsación que hace un `Shortcut` (la tecla que muestra, con `command` y `Mayús` si los lleva).
fn pressed_2(shortcut: Shortcut) -> Press {
    Press {
        key: Some(shortcut.key),
        command: shortcut.command,
        shift: shortcut.shift,
        alt: false,
        other_ctrl: false,
        repeat: false,
    }
}

/// Las acciones de cada entrada de menú y de las dos tablas de la columna.
fn menu_and_column_actions() -> Vec<AppAction> {
    let mut v: Vec<AppAction> = entries().iter().map(|i| i.action).collect();
    v.extend(TOOL_BUTTONS.iter().map(|b| b.action));
    v.extend(ZOOM_BUTTONS.iter().map(|b| b.action));
    v
}

fn platform_2(i: u8) -> Platform {
    PLATFORMS[usize::from(i) % 3]
}

fn state_of(
    can_undo: bool,
    can_redo: bool,
    zoom: Option<u32>,
    tool: u8,
    locks: [bool; 5],
) -> UiState {
    UiState {
        can_undo,
        can_redo,
        zoom: zoom.and_then(Zoom::from_int),
        tool: TEST_TOOLS[usize::from(tool) % 5],
        locks: Locks {
            gesture: locks[0],
            stroke: false,
            dialog: locks[1],
            text_focus: locks[2],
            removal: locks[3],
            modal: locks[4],
        },
    }
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(512))]

    #[test]
    fn property_1_enabled_implies_not_blocked_and_the_locks_turn_off_what_they_should(
        can_undo: bool,
        can_redo: bool,
        zoom in prop::option::of(1u32..=64),
        tool in 0u8..5,
        locks in prop::array::uniform5(any::<bool>()),
    ) {
        let state = state_of(can_undo, can_redo, zoom, tool, locks);
        for action in menu_and_column_actions() {
            let on = enabled(action, &state);
            // `enabled` implica `!is_blocked(.., Origin::Menu)`.
            if on {
                prop_assert!(!is_blocked(action, state.locks, Origin::Menu), "{:?}", action);
            }
            // Con un diálogo abierto, falso en todas.
            if state.locks.dialog {
                prop_assert!(!on, "{:?} con diálogo", action);
            }
            // With a gesture or a text field, false in all but the four zoom ones, `Mostrar panel`
            // (3.4: opening or closing the layers panel does not modify the document) and
            // `Editor de color` (C1: neither does opening or closing the color editor).
            if (state.locks.gesture || state.locks.text_focus)
                && !action.is_zoom()
                && action != AppAction::ToggleLayerPanel
                && action != AppAction::ToggleColorEditor
            {
                prop_assert!(!on, "{:?} con gesto o texto", action);
            }
            // Las acciones de zoom no se apagan por gesto, quita ni campo de texto (con el menú).
            if action.is_zoom() {
                let without_nothing_more = !state.locks.dialog && !state.locks.modal;
                if without_nothing_more && state.zoom.is_some() {
                    let per_level = match action {
                        AppAction::ZoomIn => state.zoom < Some(Zoom::MAX),
                        AppAction::ZoomOut => state.zoom > Some(Zoom::MIN),
                        AppAction::ZoomActualSize => state.zoom != Zoom::from_int(1),
                        _ => true,
                    };
                    prop_assert_eq!(on, per_level, "{:?}", action);
                }
            }
            // Ninguna entrada depende del origen: menú y botón bloquean igual.
            for bits in 0..32u8 {
                let locks = Locks {
                    gesture: bits & 1 != 0,
                    stroke: false,
                    dialog: bits & 2 != 0,
                    text_focus: bits & 4 != 0,
                    removal: bits & 8 != 0,
                    modal: bits & 16 != 0,
                };
                prop_assert_eq!(
                    is_blocked(action, locks, Origin::Menu),
                    is_blocked(action, locks, Origin::Button),
                    "{:?} con {:?}", action, locks
                );
            }
        }
    }

    #[test]
    fn property_1_the_shortcut_of_each_entry_translated_gives_its_action(platform_i in 0u8..3) {
        let platform = platform_2(platform_i);
        for item in entries() {
            if let Some(shortcut) = shortcut_for(item.action, platform) {
                prop_assert_eq!(
                    Keymap::default_scheme().action_for(&pressed_2(shortcut), platform),
                    Some(item.action),
                    "{}", item.label
                );
            }
        }
        // La letra de la columna sale de la tabla y es la tecla de herramienta (el Borrador, `k`).
        for button in TOOL_BUTTONS.iter() {
            let letter = Keymap::default_scheme().shown(button.action, platform);
            let letter = letter.expect("los botones de herramienta la muestran");
            prop_assert_eq!(
                Keymap::default_scheme().action_for(&pressed_2(letter), platform),
                Some(button.action),
                "{}", button.label
            );
        }
    }
}

// ---------------------------------------------------------------------------------------------
// Unidad 19: «Nueva imagen…» (specs/hito-2-nueva-imagen.md, fila 9, parte del menú)
// ---------------------------------------------------------------------------------------------
//
// El menú ya está adaptado en las pruebas de la fila 1 (rótulos, orden y texto de los atajos): aquí
// solo `shortcut_for` y `shortcut_text` de la acción nueva por plataforma y su `enabled`.

#[test]
fn row_9_shortcut_for_new_image_is_command_n_on_the_three_platforms() {
    for (platform, text_2) in [
        (Platform::Linux, "Ctrl+N"),
        (Platform::Windows, "Ctrl+N"),
        (Platform::Mac, "Cmd+N"),
    ] {
        let shortcut = shortcut_for(AppAction::NewImage, platform)
            .unwrap_or_else(|| panic!("{platform:?}: Nueva imagen… tiene atajo"));
        assert_eq!(
            shortcut,
            shortcut_2(KeyId::Char('n'), true, false).unwrap(),
            "{platform:?}"
        );
        assert_eq!(shortcut_text(shortcut, platform), text_2, "{platform:?}");
    }
}

#[test]
fn row_9_new_image_is_in_file_the_first_one_with_its_action() {
    let file = MENUS
        .iter()
        .find(|m| m.title == "Archivo")
        .expect("Archivo");
    let first = file.items.first().expect("Archivo tiene entradas");
    assert_eq!(first.label, "Nueva imagen…");
    assert_eq!(first.action, AppAction::NewImage);
    assert!(!first.separator_before);
    // En ningún otro menú.
    let times = MENUS
        .iter()
        .flat_map(|m| m.items.iter())
        .filter(|item| item.action == AppAction::NewImage)
        .count();
    assert_eq!(times, 1);
}

#[test]
fn row_9_enabled_of_new_image_does_not_depend_on_the_history_or_the_zoom() {
    for can_undo in [false, true] {
        for can_redo in [false, true] {
            for zoom in [
                None,
                Zoom::from_int(1),
                x5(),
                Some(Zoom::MIN),
                Some(Zoom::MAX),
            ] {
                let state = state_2(can_undo, can_redo, zoom);
                assert!(
                    enabled(AppAction::NewImage, &state),
                    "undo {can_undo}, redo {can_redo}, zoom {zoom:?}"
                );
            }
        }
    }
}

#[test]
fn row_9_enabled_of_new_image_is_disabled_with_each_lock() {
    for (name_2, locks) in [
        (
            "gesture",
            Locks {
                gesture: true,
                ..Locks::default()
            },
        ),
        (
            "dialog",
            Locks {
                dialog: true,
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
        (
            "removal",
            Locks {
                removal: true,
                ..Locks::default()
            },
        ),
        (
            "modal",
            Locks {
                modal: true,
                ..Locks::default()
            },
        ),
    ] {
        let state = UiState {
            locks,
            ..state_2(true, true, x5())
        };
        assert!(!enabled(AppAction::NewImage, &state), "{name_2}");
        assert!(
            is_blocked(AppAction::NewImage, locks, Origin::Menu),
            "{name_2}"
        );
    }
}

// ---------------------------------------------------------------------------------------------
// Unidad 20: «Imagen → Convertir modo…» (specs/hito-2-convertir-modo.md, fila 10, parte del menú)
// ---------------------------------------------------------------------------------------------
//
// La tabla ya está adaptada en las pruebas de la fila 1 (cinco menús, rótulos, orden y texto de los
// atajos): aquí el menú `TestImage` por sí solo, la falta de atajo de `ConvertMode`, su `enabled` con
// los cinco bloqueos y el ancho de los desplegables (`TestImage` mide 131 u; `Ver`, 208, sigue siendo
// el más ancho).

use px_ui::menu::popup_width_u;

fn menu_image() -> Menu {
    *MENUS
        .iter()
        .find(|m| m.title == "Imagen")
        .expect("la barra tiene el menú Imagen")
}

#[test]
fn row_10_image_is_the_third_menu_between_edit_and_view_with_convert_and_three_palette_entries() {
    let titles: Vec<&str> = MENUS.iter().map(title).collect();
    assert_eq!(titles.iter().position(|t| *t == "Imagen"), Some(2));
    assert_eq!(titles[1], "Editar");
    assert_eq!(titles[3], "Capa");
    assert_eq!(titles[4], "Ver");

    let image_2 = menu_image();
    assert_eq!(
        image_2.items.len(),
        4,
        "convertir y las tres de la paleta (C3)"
    );
    let input_2 = image_2.items[0];
    assert_eq!(input_2.label, "Convertir modo…");
    assert_eq!(input_2.action, AppAction::ConvertMode);
    assert!(!input_2.separator_before, "sin separador");
    assert!(image_2.submenus.is_empty(), "sin submenús");
}

#[test]
fn row_10_convert_mode_is_in_the_bar_only_once_and_only_in_image() {
    let times: Vec<&str> = MENUS
        .iter()
        .filter(|m| m.items.iter().any(|i| i.action == AppAction::ConvertMode))
        .map(title)
        .collect();
    assert_eq!(times, ["Imagen"]);
    let total = entries()
        .iter()
        .filter(|i| i.action == AppAction::ConvertMode)
        .count();
    assert_eq!(total, 1);
    // Ningún submenú la repite.
    for menu in MENUS.iter() {
        for sub in menu.submenus {
            assert!(
                sub.items.iter().all(|i| i.action != AppAction::ConvertMode),
                "{}",
                sub.label
            );
        }
    }
}

#[test]
fn row_10_convert_mode_has_neither_shortcut_nor_shortcut_text_on_the_three_platforms() {
    for platform in PLATFORMS {
        assert_eq!(
            shortcut_for(AppAction::ConvertMode, platform),
            None,
            "{platform:?}"
        );
    }
    // Y en la tabla del menú, la posición de la entrada nueva no lleva texto de atajo.
    for platform in PLATFORMS {
        let position = entries()
            .iter()
            .position(|i| i.action == AppAction::ConvertMode)
            .expect("la entrada existe");
        assert_eq!(texts_of_shortcut(platform)[position], None, "{platform:?}");
    }
}

#[test]
fn row_10_enabled_of_convert_mode_does_not_depend_on_the_history_or_the_zoom() {
    for can_undo in [false, true] {
        for can_redo in [false, true] {
            for zoom in [
                None,
                Zoom::from_int(1),
                x5(),
                Some(Zoom::MIN),
                Some(Zoom::MAX),
            ] {
                let state = state_2(can_undo, can_redo, zoom);
                assert!(
                    enabled(AppAction::ConvertMode, &state),
                    "undo {can_undo}, redo {can_redo}, zoom {zoom:?}"
                );
            }
        }
    }
}

#[test]
fn row_10_enabled_of_convert_mode_is_disabled_with_each_of_the_five_locks() {
    for (name_2, locks) in [
        (
            "gesture",
            Locks {
                gesture: true,
                ..Locks::default()
            },
        ),
        (
            "dialog",
            Locks {
                dialog: true,
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
        // Regla literal de la 18: la quita pendiente también frena abrir un diálogo.
        (
            "removal",
            Locks {
                removal: true,
                ..Locks::default()
            },
        ),
        (
            "modal",
            Locks {
                modal: true,
                ..Locks::default()
            },
        ),
    ] {
        let state = UiState {
            locks,
            ..state_2(true, true, x5())
        };
        assert!(!enabled(AppAction::ConvertMode, &state), "{name_2}");
        assert!(
            is_blocked(AppAction::ConvertMode, locks, Origin::Menu),
            "{name_2}"
        );
    }
    // Sin bloqueos, habilitada.
    assert!(enabled(AppAction::ConvertMode, &state_2(true, true, x5())));
}

#[test]
fn row_10_the_image_dropdown_measures_166_u_and_the_view_one_208_the_widest() {
    for platform in PLATFORMS {
        assert_eq!(popup_width_u(&menu_image(), platform), 166, "{platform:?}");
        let see = MENUS.iter().find(|m| m.title == "Ver").expect("Ver");
        assert_eq!(popup_width_u(see, platform), 208, "{platform:?}");
        assert!(
            popup_width_u(&menu_image(), platform) < popup_width_u(see, platform),
            "{platform:?}: Imagen es más estrecho que Ver"
        );
        // `Ver` sigue siendo el más ancho de los cinco.
        for menu in MENUS.iter().filter(|m| m.title != "Ver") {
            assert!(
                popup_width_u(menu, platform) < 208,
                "{platform:?}: {} mide {}",
                menu.title,
                popup_width_u(menu, platform)
            );
        }
    }
}
