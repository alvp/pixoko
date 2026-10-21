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
use px_ui::help_texts::{
    action_of_key, help_entry, help_key, help_table, is_editable,
};
use px_ui::key_events::chord_of;
use px_ui::keymap::{Chord, KeyId, Keymap, Press};
use px_ui::licenses_view::wrap_text;
use px_ui::shortcut_editor::{CaptureResult, ShortcutEditor, Slot, slots_of};
use px_ui::key_events::Capture;
use px_ui::text::INTERFACE_CHARSET;
use px_ui::tool::Tool;

const PLATAFORMAS: [Platform; 3] = [Platform::Linux, Platform::Windows, Platform::Mac];

const HERRAMIENTAS: [Tool; 5] = [
    Tool::Pencil,
    Tool::Line,
    Tool::Fill,
    Tool::Dropper,
    Tool::Eraser,
];

const DESTINOS_DE_BOTON: [HelpTarget; 9] = [
    HelpTarget::Tool(Tool::Pencil),
    HelpTarget::Tool(Tool::Line),
    HelpTarget::Tool(Tool::Fill),
    HelpTarget::Tool(Tool::Dropper),
    HelpTarget::Tool(Tool::Eraser),
    HelpTarget::ZoomIn,
    HelpTarget::ZoomOut,
    HelpTarget::ZoomFit,
    HelpTarget::ZoomActual,
];

/// Las 22 claves del anexo de la spec, en el orden de la tabla.
const CLAVES: [&str; 22] = [
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
];

/// Las acciones de la tabla por defecto (22 distintas).
fn acciones_de_la_tabla() -> Vec<AppAction> {
    let mut v: Vec<AppAction> = Vec::new();
    for row in Keymap::default_scheme().rows() {
        if !v.contains(&row.action) {
            v.push(row.action);
        }
    }
    v
}

fn texto_de_la_zona(spec: &FormSpec) -> String {
    let campo = spec
        .fields
        .iter()
        .find(|f| f.id == HELP_ZONE)
        .expect("la ayuda tiene una zona de texto");
    match &campo.kind {
        FieldKind::ReadOnly { text, .. } => text.clone(),
        otro => panic!("la zona es de solo lectura, no {otro:?}"),
    }
}

fn boton(spec: &FormSpec, id: &str) -> Option<String> {
    spec.fields.iter().find(|f| f.id == id).map(|f| match &f.kind {
        FieldKind::Button { text, .. } => text.clone(),
        otro => panic!("{id} es un botón, no {otro:?}"),
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
fn fila_1_la_tabla_tiene_22_entradas_con_claves_unicas_y_limites() {
    let tabla = help_table();
    assert_eq!(tabla.len(), 22);
    let claves: Vec<&str> = tabla.iter().map(|e| e.key).collect();
    assert_eq!(claves, CLAVES, "claves estables y en el orden del anexo");
    for (i, e) in tabla.iter().enumerate() {
        assert!(!tabla[..i].iter().any(|o| o.key == e.key), "clave repetida {}", e.key);
        assert!(e.name.chars().count() <= 20, "nombre largo: {}", e.name);
        let lineas = wrap_text(e.description, ZONE_COLS);
        assert!(lineas.len() <= 8, "{}: {} líneas", e.key, lineas.len());
        assert!(!e.description.trim().is_empty(), "{}", e.key);
        let todo = [
            Some(e.name),
            Some(e.description),
            e.mouse_left,
            e.mouse_right,
        ];
        for texto in todo.into_iter().flatten() {
            for c in texto.chars() {
                assert!(INTERFACE_CHARSET.contains(c), "{}: {c:?} fuera del juego", e.key);
            }
        }
        assert!(e.options.is_empty(), "{}: sin opciones hoy", e.key);
    }
}

#[test]
fn fila_1_toda_accion_de_la_tabla_de_teclado_tiene_texto_y_las_claves_son_inversas() {
    for action in acciones_de_la_tabla() {
        let key = help_key(action).unwrap_or_else(|| panic!("{action:?} sin clave de ayuda"));
        assert!(help_entry(key).is_some(), "{key}");
        assert_eq!(action_of_key(key), Some(action), "{key}");
    }
    // Las cinco herramientas y los cuatro botones de zoom están en la tabla.
    for target in DESTINOS_DE_BOTON {
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
    let editables = help_table()
        .iter()
        .filter(|e| is_editable(action_of_key(e.key).unwrap()))
        .count();
    assert_eq!(editables, 17);
    for key in ["file.new", "file.open", "file.save", "file.save-as", "file.quit"] {
        assert!(!is_editable(action_of_key(key).unwrap()), "{key}");
    }
}

// ---------------------------------------------------------------------------------------------
// Fila 2: la ventana de ayuda
// ---------------------------------------------------------------------------------------------

#[test]
fn fila_2_help_form_de_cada_destino_valida_y_cabe_sin_desplazar() {
    let tabla = Keymap::default_scheme();
    for platform in PLATAFORMAS {
        let mut destinos = DESTINOS_DE_BOTON.to_vec();
        destinos.push(HelpTarget::Active);
        for target in destinos {
            for active in HERRAMIENTAS {
                let spec = help_form(target, active, &tabla, platform);
                spec.validate().unwrap_or_else(|e| panic!("{target:?}: {e:?}"));
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
            let spec = help_form_for(e.key, &tabla, platform).expect(e.key);
            assert!(spec.validate().is_ok(), "{}", e.key);
            assert_eq!(spec.title, e.name);
        }
        assert!(help_form_for("no.existe", &tabla, platform).is_none());
    }
}

#[test]
fn fila_2_el_titulo_es_el_nombre_y_el_destino_activo_es_el_de_la_herramienta() {
    let tabla = Keymap::default_scheme();
    for active in HERRAMIENTAS {
        let a = help_form(HelpTarget::Active, active, &tabla, Platform::Linux);
        let b = help_form(HelpTarget::Tool(active), Tool::Pencil, &tabla, Platform::Linux);
        assert_eq!(a, b, "{active:?}");
    }
    let lapiz = help_form(HelpTarget::Tool(Tool::Pencil), Tool::Line, &tabla, Platform::Linux);
    assert_eq!(lapiz.title, "Lápiz");
    let texto = texto_de_la_zona(&lapiz);
    assert!(texto.contains("Clic izquierdo: "), "{texto}");
    assert!(texto.contains("Clic derecho: "), "{texto}");
    let ajustar = help_form(HelpTarget::ZoomFit, Tool::Pencil, &tabla, Platform::Linux);
    let texto = texto_de_la_zona(&ajustar);
    assert!(!texto.contains("Clic izquierdo"), "Ajustar no tiene líneas de clic: {texto}");
    assert!(!texto.contains("Clic derecho"), "{texto}");
}

#[test]
fn fila_2_botones_de_atajo_fijos_y_extras() {
    let tabla = Keymap::default_scheme();
    // Lápiz: principal d, sin secundario.
    let lapiz = help_form(HelpTarget::Tool(Tool::Pencil), Tool::Pencil, &tabla, Platform::Linux);
    assert_eq!(boton(&lapiz, HELP_PRIMARY).as_deref(), Some("d"));
    assert_eq!(boton(&lapiz, HELP_SECONDARY).as_deref(), Some("sin atajo"));
    // Deshacer: principal u, fijo Ctrl+Z.
    let deshacer = help_form_for("edit.undo", &tabla, Platform::Linux).unwrap();
    assert_eq!(boton(&deshacer, HELP_PRIMARY).as_deref(), Some("u"));
    assert_eq!(boton(&deshacer, HELP_SECONDARY).as_deref(), Some("sin atajo"));
    assert!(texto_de_la_zona(&deshacer).contains("Fijos: Ctrl+Z"));
    // Acercar: `>` y `+`, también `=`.
    let acercar = help_form(HelpTarget::ZoomIn, Tool::Pencil, &tabla, Platform::Linux);
    assert_eq!(boton(&acercar, HELP_PRIMARY).as_deref(), Some(">"));
    assert_eq!(boton(&acercar, HELP_SECONDARY).as_deref(), Some("+"));
    assert!(texto_de_la_zona(&acercar).contains("También: ="));
    // Archivo: sin botones y con el atajo fijo; Salir sin atajo fuera de Linux.
    let guardar = help_form_for("file.save", &tabla, Platform::Linux).unwrap();
    assert!(boton(&guardar, HELP_PRIMARY).is_none() && boton(&guardar, HELP_SECONDARY).is_none());
    assert!(texto_de_la_zona(&guardar).contains("Atajo (fijo): Ctrl+S"));
    let salir_linux = help_form_for("file.quit", &tabla, Platform::Linux).unwrap();
    assert!(texto_de_la_zona(&salir_linux).contains("Atajo (fijo): Ctrl+Q"));
    for platform in [Platform::Windows, Platform::Mac] {
        let salir = help_form_for("file.quit", &tabla, platform).unwrap();
        assert!(texto_de_la_zona(&salir).contains("sin atajo"), "{platform:?}");
    }
    // Con la tabla modificada, el botón dice lo nuevo.
    let mut editor = ShortcutEditor::new(
        AppAction::SelectTool(Tool::Pencil),
        &tabla,
        Platform::Linux,
    )
    .unwrap();
    editor.begin_capture(Slot::Primary);
    assert_eq!(editor.capture(Capture::Press(press('x'))), CaptureResult::Set);
    let nueva = editor.accept();
    let lapiz = help_form(HelpTarget::Tool(Tool::Pencil), Tool::Pencil, &nueva, Platform::Linux);
    assert_eq!(boton(&lapiz, HELP_PRIMARY).as_deref(), Some("x"));
    assert_eq!(chord('x'), slots_of(&nueva, AppAction::SelectTool(Tool::Pencil)).slots[0]);
}

#[test]
fn fila_2_foco_inicial_en_la_zona_pressed_no_cierra_y_cerrar_cierra() {
    let tabla = Keymap::default_scheme();
    let spec = help_form(HelpTarget::Tool(Tool::Pencil), Tool::Pencil, &tabla, Platform::Linux);
    let mut dialog = Dialog::new(DialogKind::Help, spec).unwrap();
    assert!(matches!(
        dialog.take_focus_request(),
        Some(px_ui::dialog_keys::FocusTarget::Zone(HELP_ZONE))
    ));
    let reaccion = dialog.event(Event::Press {
        field: HELP_PRIMARY,
        aside: false,
    });
    assert_eq!(
        reaccion,
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
    let mut otro = Dialog::new(
        DialogKind::Help,
        help_form(HelpTarget::ZoomFit, Tool::Pencil, &tabla, Platform::Linux),
    )
    .unwrap();
    assert!(matches!(otro.event(Event::Cancel), Reaction::Cancelled));
    // La zona es `Fixed(9)`.
    let spec = help_form(HelpTarget::Tool(Tool::Line), Tool::Pencil, &tabla, Platform::Linux);
    let zona = spec.fields.iter().find(|f| f.id == HELP_ZONE).unwrap();
    assert!(matches!(
        zona.kind,
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
fn tabla_alcanzable(pasos: &[(usize, bool, u8, bool)], platform: Platform) -> Keymap {
    let mut tabla = Keymap::default_scheme();
    let editables: Vec<AppAction> = help_table()
        .iter()
        .filter_map(|e| action_of_key(e.key))
        .filter(|a| is_editable(*a))
        .collect();
    for &(accion, secundaria, caracter, reasignar) in pasos {
        let action = editables[accion % editables.len()];
        let Some(mut editor) = ShortcutEditor::new(action, &tabla, platform) else {
            continue;
        };
        editor.begin_capture(if secundaria {
            Slot::Secondary
        } else {
            Slot::Primary
        });
        let c = (b'a' + caracter % 26) as char;
        if let CaptureResult::Pending(_) = editor.capture(Capture::Press(press(c))) {
            editor.resolve(reasignar);
        }
        tabla = editor.accept();
    }
    tabla
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(256))]

    /// Propiedad 4: `help_form` es válida para cualquier tabla alcanzable, ninguna línea de la zona
    /// pasa de 42 columnas, los botones dicen exactamente las casillas y las de archivo no llevan.
    #[test]
    fn propiedad_4_la_ayuda_es_valida_para_cualquier_tabla(
        pasos in proptest::collection::vec((0usize..17, any::<bool>(), 0u8..26, any::<bool>()), 0..8),
        plataforma in 0usize..3,
    ) {
        let platform = PLATAFORMAS[plataforma];
        let tabla = tabla_alcanzable(&pasos, platform);
        for e in help_table() {
            let action = action_of_key(e.key).unwrap();
            let spec = help_form_for(e.key, &tabla, platform).unwrap();
            prop_assert!(spec.validate().is_ok());
            for linea in wrap_text(&texto_de_la_zona(&spec), ZONE_COLS) {
                prop_assert!(linea.chars().count() <= ZONE_COLS);
            }
            if is_editable(action) {
                let casillas = slots_of(&tabla, action).slots;
                for (campo, i) in [(HELP_PRIMARY, 0usize), (HELP_SECONDARY, 1usize)] {
                    let esperado = casillas
                        .get(i)
                        .map_or("sin atajo".to_string(), |c| c.text(platform));
                    prop_assert_eq!(boton(&spec, campo), Some(esperado));
                }
            } else {
                prop_assert!(boton(&spec, HELP_PRIMARY).is_none());
                prop_assert!(boton(&spec, HELP_SECONDARY).is_none());
            }
        }
    }

    /// Visto bueno 4c: ninguna acción de la tabla (de cualquier tabla alcanzable) se queda sin
    /// texto de ayuda.
    #[test]
    fn propiedad_c_ninguna_accion_de_la_tabla_se_queda_sin_texto_de_ayuda(
        pasos in proptest::collection::vec((0usize..17, any::<bool>(), 0u8..26, any::<bool>()), 0..8),
    ) {
        let tabla = tabla_alcanzable(&pasos, Platform::Linux);
        for row in tabla.rows() {
            let key = help_key(row.action);
            prop_assert!(key.is_some(), "{:?}", row.action);
            prop_assert!(help_entry(key.unwrap()).is_some());
        }
        // Lo que sale del editor es asignable.
        for row in tabla.rows().iter().filter(|r| !r.command) {
            let press = Press {
                key: Some(row.key),
                shift: row.shift,
                ..press('a')
            };
            prop_assert!(chord_of(&press).is_ok(), "{row:?}");
        }
    }
}
