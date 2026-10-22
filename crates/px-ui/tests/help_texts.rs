//! Tests de la tabla de ayuda y de la ventana de ayuda (specs/interfaz-ayuda.md, unidad I.5).
//! En ROJO hasta la lógica: `help_texts`, `help` y `FieldKind::Button` no existen todavía.
//!
//! - Fila 1: la tabla de textos (22 entradas, claves estables, límites, toda acción de la tabla de
//!   teclado con texto).
//! - Fila 2: `help_form` de cada destino (contenido, botones, fijos, cabe sin desplazar).
//! - Propiedad 4: la ayuda es válida para cualquier tabla alcanzable por el editor.
//! - Propiedad (visto bueno 4c): ninguna acción de la tabla de cualquier tabla alcanzable se queda
//!   sin texto de ayuda.

use proptest::prelude::*;
use px_ui::actions::{AppAction, Platform};
use px_ui::dialog::{Dialog, DialogKind, Event, FieldKind, FormSpec, Reaction, Rows};
use px_ui::dialog_layout::{ZONE_COLS, dialog_layout};
use px_ui::help::{HELP_PRIMARY, HELP_SECONDARY, HELP_ZONE, HelpTarget, help_form, help_form_for};
use px_ui::help_texts::{action_of_key, help_entry, help_key, help_table, is_editable};
use px_ui::key_events::Capture;
use px_ui::key_events::chord_of;
use px_ui::keymap::{Chord, KeyId, Keymap, Press};
use px_ui::licenses_view::wrap_text;
use px_ui::shortcut_editor::{CaptureResult, ShortcutEditor, Slot, slots_of};
use px_ui::text::INTERFACE_CHARSET;
use px_ui::tool::Tool;

const PLATFORMS: [Platform; 3] = [Platform::Linux, Platform::Windows, Platform::Mac];

const TEST_TOOLS: [Tool; 5] = [
    Tool::Pencil,
    Tool::Line,
    Tool::Fill,
    Tool::Dropper,
    Tool::Eraser,
];

const TARGETS_OF_BUTTON: [HelpTarget; 9] = [
    HelpTarget::Tool(Tool::Pencil),
    HelpTarget::Tool(Tool::Line),
    HelpTarget::Tool(Tool::Fill),
    HelpTarget::Tool(Tool::Dropper),
    HelpTarget::Tool(Tool::Eraser),
    HelpTarget::ZoomIn,
    HelpTarget::ZoomOut,
    HelpTarget::ZoomFit,
    HelpTarget::ZoomActualSize,
];

/// Las 22 claves del anexo de la spec, en el orden de la tabla.
const KEYS: [&str; 34] = [
    "tool.pencil",
    "tool.line",
    "tool.fill",
    "tool.dropper",
    "tool.eraser",
    "view.zoom-in",
    "view.zoom-out",
    "view.zoom-fit",
    "view.zoom-actual",
    "view.center",
    "edit.undo",
    "edit.redo",
    "edit.clear-page",
    "color.next",
    "color.prev",
    "color.bg-next",
    "color.bg-prev",
    "file.new",
    "file.open",
    "file.save",
    "file.save-as",
    "file.quit",
    "layer.panel",
    "layer.new",
    "layer.duplicate",
    "layer.remove",
    "layer.up",
    "layer.down",
    "layer.rename",
    "layer.properties",
    "layer.merge-down",
    "layer.merge-visible",
    "layer.flatten",
    "color.editor",
];

/// Las acciones de la tabla por defecto (22 distintas).
fn table_actions() -> Vec<AppAction> {
    let mut v: Vec<AppAction> = Vec::new();
    for row in Keymap::default_scheme().rows() {
        if !v.contains(&row.action) {
            v.push(row.action);
        }
    }
    v
}

fn text_of_the_area(spec: &FormSpec) -> String {
    let field_2 = spec
        .fields
        .iter()
        .find(|f| f.id == HELP_ZONE)
        .expect("la ayuda tiene una zona de texto");
    match &field_2.kind {
        FieldKind::ReadOnly { text, .. } => text.clone(),
        other_2 => panic!("la zona es de solo lectura, no {other_2:?}"),
    }
}

fn button_2(spec: &FormSpec, id: &str) -> Option<String> {
    spec.fields
        .iter()
        .find(|f| f.id == id)
        .map(|f| match &f.kind {
            FieldKind::Button { text, .. } => text.clone(),
            other_2 => panic!("{id} es un botón, no {other_2:?}"),
        })
}

fn chord(c: char) -> Chord {
    Chord {
        key: KeyId::Char(c),
        command: false,
        shift: false,
    }
}

fn press(c: char) -> Press {
    Press {
        key: Some(KeyId::Char(c)),
        command: false,
        shift: false,
        alt: false,
        other_ctrl: false,
        repeat: false,
    }
}

// ---------------------------------------------------------------------------------------------
// Fila 1: la tabla
// ---------------------------------------------------------------------------------------------

#[test]
fn row_1_the_table_has_34_entries_with_unique_keys_and_limits() {
    let table = help_table();
    assert_eq!(table.len(), 34);
    let keys: Vec<&str> = table.iter().map(|e| e.key).collect();
    assert_eq!(keys, KEYS, "claves estables y en el orden del anexo");
    for (i, e) in table.iter().enumerate() {
        assert!(
            !table[..i].iter().any(|o| o.key == e.key),
            "clave repetida {}",
            e.key
        );
        assert!(e.name.chars().count() <= 20, "nombre largo: {}", e.name);
        let lines = wrap_text(e.description, ZONE_COLS);
        assert!(lines.len() <= 8, "{}: {} líneas", e.key, lines.len());
        assert!(!e.description.trim().is_empty(), "{}", e.key);
        let all = [
            Some(e.name),
            Some(e.description),
            e.mouse_left,
            e.mouse_right,
        ];
        for text_2 in all.into_iter().flatten() {
            for c in text_2.chars() {
                assert!(
                    INTERFACE_CHARSET.contains(c),
                    "{}: {c:?} fuera del juego",
                    e.key
                );
            }
        }
        assert!(e.options.is_empty(), "{}: sin opciones hoy", e.key);
    }
}

#[test]
fn row_1_every_action_of_the_keyboard_table_has_text_and_the_keys_are_inverse() {
    for action in table_actions() {
        let key = help_key(action).unwrap_or_else(|| panic!("{action:?} sin clave de ayuda"));
        assert!(help_entry(key).is_some(), "{key}");
        assert_eq!(action_of_key(key), Some(action), "{key}");
    }
    // Las cinco herramientas y los cuatro botones de zoom están en la tabla.
    for target in TARGETS_OF_BUTTON {
        let action = target.action(Tool::Pencil);
        assert!(help_key(action).is_some(), "{target:?}");
    }
    // Las inversas: cada clave de la tabla de ayuda da una acción que da esa clave.
    for e in help_table() {
        let action = action_of_key(e.key).expect(e.key);
        assert_eq!(help_key(action), Some(e.key));
    }
    assert_eq!(action_of_key("no.existe"), None);
    // Editables: 17 de 22; las cinco de archivo son fijas.
    let editable = help_table()
        .iter()
        .filter(|e| is_editable(action_of_key(e.key).unwrap()))
        .count();
    assert_eq!(editable, 29);
    for key in [
        "file.new",
        "file.open",
        "file.save",
        "file.save-as",
        "file.quit",
    ] {
        assert!(!is_editable(action_of_key(key).unwrap()), "{key}");
    }
}

// ---------------------------------------------------------------------------------------------
// Fila 2: la ventana de ayuda
// ---------------------------------------------------------------------------------------------

#[test]
fn row_2_help_form_of_each_target_valid_and_fits_without_scrolling() {
    let table = Keymap::default_scheme();
    for platform in PLATFORMS {
        let mut targets = TARGETS_OF_BUTTON.to_vec();
        targets.push(HelpTarget::Active);
        for target in targets {
            for active in TEST_TOOLS {
                let spec = help_form(target, active, &table, platform);
                spec.validate()
                    .unwrap_or_else(|e| panic!("{target:?}: {e:?}"));
                assert_eq!(spec.accept, "Cerrar");
                assert_eq!(spec.cancel, None);
                let dialog = Dialog::new(DialogKind::Help, spec).expect("valida");
                // A la ventana mínima (468 × 341 u) a cualquier escala: el cuerpo no se desplaza.
                for n in [1u32, 2, 3] {
                    let n64 = i64::from(n);
                    let layout = dialog_layout(
                        (468 * n64, 341 * n64),
                        n,
                        dialog.content_h_u(),
                        false,
                        false,
                    );
                    assert_eq!(layout.max_scroll_u, 0, "{target:?} a n={n}");
                }
            }
        }
        // Todas las claves de la tabla dan una ayuda.
        for e in help_table() {
            let spec = help_form_for(e.key, &table, platform).expect(e.key);
            assert!(spec.validate().is_ok(), "{}", e.key);
            assert_eq!(spec.title, e.name);
        }
        assert!(help_form_for("no.existe", &table, platform).is_none());
    }
}

#[test]
fn row_2_the_title_is_the_name_and_the_active_target_is_that_of_the_tool() {
    let table = Keymap::default_scheme();
    for active in TEST_TOOLS {
        let a = help_form(HelpTarget::Active, active, &table, Platform::Linux);
        let b = help_form(
            HelpTarget::Tool(active),
            Tool::Pencil,
            &table,
            Platform::Linux,
        );
        assert_eq!(a, b, "{active:?}");
    }
    let pencil = help_form(
        HelpTarget::Tool(Tool::Pencil),
        Tool::Line,
        &table,
        Platform::Linux,
    );
    assert_eq!(pencil.title, "Lápiz");
    let text_2 = text_of_the_area(&pencil);
    assert!(text_2.contains("Clic izquierdo: "), "{text_2}");
    assert!(text_2.contains("Clic derecho: "), "{text_2}");
    let fit_2 = help_form(HelpTarget::ZoomFit, Tool::Pencil, &table, Platform::Linux);
    let text_2 = text_of_the_area(&fit_2);
    assert!(
        !text_2.contains("Clic izquierdo"),
        "Ajustar no tiene líneas de clic: {text_2}"
    );
    assert!(!text_2.contains("Clic derecho"), "{text_2}");
}

#[test]
fn row_2_fixed_and_extra_shortcut_buttons() {
    let table = Keymap::default_scheme();
    // Lápiz: principal d, sin secundario.
    let pencil = help_form(
        HelpTarget::Tool(Tool::Pencil),
        Tool::Pencil,
        &table,
        Platform::Linux,
    );
    assert_eq!(button_2(&pencil, HELP_PRIMARY).as_deref(), Some("d"));
    assert_eq!(
        button_2(&pencil, HELP_SECONDARY).as_deref(),
        Some("sin atajo")
    );
    // Deshacer: principal u, fijo Ctrl+Z.
    let undo = help_form_for("edit.undo", &table, Platform::Linux).unwrap();
    assert_eq!(button_2(&undo, HELP_PRIMARY).as_deref(), Some("u"));
    assert_eq!(
        button_2(&undo, HELP_SECONDARY).as_deref(),
        Some("sin atajo")
    );
    assert!(text_of_the_area(&undo).contains("Fijos: Ctrl+Z"));
    // Acercar: `>` y `+`, también `=`.
    let zoom_in = help_form(HelpTarget::ZoomIn, Tool::Pencil, &table, Platform::Linux);
    assert_eq!(button_2(&zoom_in, HELP_PRIMARY).as_deref(), Some(">"));
    assert_eq!(button_2(&zoom_in, HELP_SECONDARY).as_deref(), Some("+"));
    assert!(text_of_the_area(&zoom_in).contains("También: ="));
    // Archivo: sin botones y con el atajo fijo; Salir sin atajo fuera de Linux.
    let save = help_form_for("file.save", &table, Platform::Linux).unwrap();
    assert!(button_2(&save, HELP_PRIMARY).is_none() && button_2(&save, HELP_SECONDARY).is_none());
    assert!(text_of_the_area(&save).contains("Atajo (fijo): Ctrl+S"));
    let exit_linux = help_form_for("file.quit", &table, Platform::Linux).unwrap();
    assert!(text_of_the_area(&exit_linux).contains("Atajo (fijo): Ctrl+Q"));
    for platform in [Platform::Windows, Platform::Mac] {
        let exit = help_form_for("file.quit", &table, platform).unwrap();
        assert!(
            text_of_the_area(&exit).contains("sin atajo"),
            "{platform:?}"
        );
    }
    // Con la tabla modificada, el botón dice lo nuevo.
    let mut editor =
        ShortcutEditor::new(AppAction::SelectTool(Tool::Pencil), &table, Platform::Linux).unwrap();
    editor.begin_capture(Slot::Primary);
    assert_eq!(
        editor.capture(Capture::Press(press('x'))),
        CaptureResult::Set
    );
    let new = editor.accept();
    let pencil = help_form(
        HelpTarget::Tool(Tool::Pencil),
        Tool::Pencil,
        &new,
        Platform::Linux,
    );
    assert_eq!(button_2(&pencil, HELP_PRIMARY).as_deref(), Some("x"));
    assert_eq!(
        chord('x'),
        slots_of(&new, AppAction::SelectTool(Tool::Pencil)).slots[0]
    );
}

#[test]
fn row_2_initial_focus_in_the_area_pressed_does_not_close_and_close_closes() {
    let table = Keymap::default_scheme();
    let spec = help_form(
        HelpTarget::Tool(Tool::Pencil),
        Tool::Pencil,
        &table,
        Platform::Linux,
    );
    let mut dialog = Dialog::new(DialogKind::Help, spec).unwrap();
    assert!(matches!(
        dialog.take_focus_request(),
        Some(px_ui::dialog_keys::FocusTarget::Zone(HELP_ZONE))
    ));
    let reaction = dialog.event(Event::Press {
        field: HELP_PRIMARY,
        aside: false,
    });
    assert_eq!(
        reaction,
        Reaction::Pressed {
            field: HELP_PRIMARY,
            aside: false
        }
    );
    // Un campo desconocido o una zona no son botones.
    assert_eq!(
        dialog.event(Event::Press {
            field: HELP_ZONE,
            aside: false
        }),
        Reaction::None
    );
    assert!(matches!(dialog.event(Event::Accept), Reaction::Accepted(_)));
    let mut other_2 = Dialog::new(
        DialogKind::Help,
        help_form(HelpTarget::ZoomFit, Tool::Pencil, &table, Platform::Linux),
    )
    .unwrap();
    assert!(matches!(other_2.event(Event::Cancel), Reaction::Cancelled));
    // La zona es `Fixed(9)`.
    let spec = help_form(
        HelpTarget::Tool(Tool::Line),
        Tool::Pencil,
        &table,
        Platform::Linux,
    );
    let zone = spec.fields.iter().find(|f| f.id == HELP_ZONE).unwrap();
    assert!(matches!(
        zone.kind,
        FieldKind::ReadOnly {
            rows: Rows::Fixed(9),
            ..
        }
    ));
}

// ---------------------------------------------------------------------------------------------
// Propiedades
// ---------------------------------------------------------------------------------------------

/// Una tabla alcanzable por el editor: una secuencia de asignaciones sobre la tabla por defecto.
fn table_reachable(steps_2: &[(usize, bool, u8, bool)], platform: Platform) -> Keymap {
    let mut table = Keymap::default_scheme();
    let editable: Vec<AppAction> = help_table()
        .iter()
        .filter_map(|e| action_of_key(e.key))
        .filter(|a| is_editable(*a))
        .collect();
    for &(action_2, secondary, character, reassign) in steps_2 {
        let action = editable[action_2 % editable.len()];
        let Some(mut editor) = ShortcutEditor::new(action, &table, platform) else {
            continue;
        };
        editor.begin_capture(if secondary {
            Slot::Secondary
        } else {
            Slot::Primary
        });
        let c = (b'a' + character % 26) as char;
        if let CaptureResult::Pending(_) = editor.capture(Capture::Press(press(c))) {
            editor.resolve(reassign);
        }
        table = editor.accept();
    }
    table
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(256))]

    /// Propiedad 4: `help_form` es válida para cualquier tabla alcanzable, ninguna línea de la zona
    /// pasa de 42 columnas, los botones dicen exactamente las casillas y las de archivo no llevan.
    #[test]
    fn property_4_the_help_is_valid_for_any_table(
        steps_2 in proptest::collection::vec((0usize..17, any::<bool>(), 0u8..26, any::<bool>()), 0..8),
        platform_2 in 0usize..3,
    ) {
        let platform = PLATFORMS[platform_2];
        let table = table_reachable(&steps_2, platform);
        for e in help_table() {
            let action = action_of_key(e.key).unwrap();
            let spec = help_form_for(e.key, &table, platform).unwrap();
            prop_assert!(spec.validate().is_ok());
            for line in wrap_text(&text_of_the_area(&spec), ZONE_COLS) {
                prop_assert!(line.chars().count() <= ZONE_COLS);
            }
            if is_editable(action) {
                let boxes_2 = slots_of(&table, action).slots;
                for (field_2, i) in [(HELP_PRIMARY, 0usize), (HELP_SECONDARY, 1usize)] {
                    let expected_2 = boxes_2
                        .get(i)
                        .map_or("sin atajo".to_string(), |c| c.text(platform));
                    prop_assert_eq!(button_2(&spec, field_2), Some(expected_2));
                }
            } else {
                prop_assert!(button_2(&spec, HELP_PRIMARY).is_none());
                prop_assert!(button_2(&spec, HELP_SECONDARY).is_none());
            }
        }
    }

    /// Visto bueno 4c: ninguna acción de la tabla (de cualquier tabla alcanzable) se queda sin
    /// texto de ayuda.
    #[test]
    fn property_c_no_action_of_the_table_is_left_without_help_text(
        steps_2 in proptest::collection::vec((0usize..17, any::<bool>(), 0u8..26, any::<bool>()), 0..8),
    ) {
        let table = table_reachable(&steps_2, Platform::Linux);
        for row in table.rows() {
            let key = help_key(row.action);
            prop_assert!(key.is_some(), "{:?}", row.action);
            prop_assert!(help_entry(key.unwrap()).is_some());
        }
        // Lo que sale del editor es asignable.
        for row in table.rows().iter().filter(|r| !r.command) {
            let press = Press {
                key: Some(row.key),
                shift: row.shift,
                ..press('a')
            };
            prop_assert!(chord_of(&press).is_ok(), "{row:?}");
        }
    }
}
