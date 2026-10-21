//! Tests de la tabla del menú `px_ui::menu` (specs/hito-2-menu-herramientas.md, «Casos» filas 1 y 2
//! y propiedad 1). Sin ventana: `MENUS`, `shortcut_for`, `shortcut_text` y `enabled` son lógica
//! pura; solo la comprobación de que ningún rótulo pierde un carácter con la fuente real usa un
//! `egui::Context` (sin GPU).
//!
//! - Fila 1: los cinco menús (con `Imagen` de la unidad 20) con sus rótulos y acciones, el separador solo antes de `Salir`, los
//!   atajos por `Platform::{Linux, Windows, Mac}` (`Salir` solo con `Ctrl+Q` en Linux; `Cmd` en
//!   macOS y `Rehacer` = `Cmd+Mayús+Z`), los rótulos contra `INTERFACE_CHARSET`, ningún `?` por
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

const PLATAFORMAS: [Platform; 3] = [Platform::Linux, Platform::Windows, Platform::Mac];
const HERRAMIENTAS: [Tool; 5] = [
    Tool::Pencil,
    Tool::Line,
    Tool::Fill,
    Tool::Dropper,
    Tool::Eraser,
];

/// Las 11 acciones de la tabla, en el orden del menú.
const ACCIONES_DE_LA_TABLA: [AppAction; 11] = [
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

/// Las entradas de todos los menús, aplanadas.
fn entradas() -> Vec<MenuItem> {
    MENUS.iter().flat_map(|m| m.items.iter().copied()).collect()
}

fn titulo(menu: &Menu) -> &'static str {
    menu.title
}

fn atajo(key: KeyId, command: bool, shift: bool) -> Option<Shortcut> {
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
fn fila_1_cinco_menus_en_orden_con_sus_titulos() {
    let titulos: Vec<&str> = MENUS.iter().map(titulo).collect();
    assert_eq!(titulos, ["Archivo", "Editar", "Imagen", "Ver", "Ayuda"]);
}

#[test]
fn fila_1_cada_menu_tiene_los_rotulos_y_las_acciones_de_la_decision_2() {
    let esperado: [(&str, Vec<(&str, AppAction)>); 5] = [
        (
            "Archivo",
            vec![
                ("Nueva imagen…", AppAction::NewImage),
                ("Abrir…", AppAction::Open),
                ("Guardar", AppAction::Save),
                ("Guardar como…", AppAction::SaveAs),
                ("Salir", AppAction::Quit),
            ],
        ),
        (
            "Editar",
            vec![
                ("Deshacer", AppAction::Undo),
                ("Rehacer", AppAction::Redo),
                ("Borrar página", AppAction::ClearPage),
            ],
        ),
        ("Imagen", vec![("Convertir modo…", AppAction::ConvertMode)]),
        (
            "Ver",
            vec![
                ("Acercar", AppAction::ZoomIn),
                ("Alejar", AppAction::ZoomOut),
                ("Ajustar a la ventana", AppAction::ZoomFit),
                ("1:1", AppAction::ZoomActual),
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
    for (menu, (titulo, items)) in MENUS.iter().zip(esperado) {
        assert_eq!(menu.title, titulo);
        let reales: Vec<(&str, AppAction)> =
            menu.items.iter().map(|i| (i.label, i.action)).collect();
        assert_eq!(reales, items, "menú {titulo}");
    }
}

#[test]
fn fila_1_el_separador_va_solo_antes_de_salir() {
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
fn fila_1_hay_catorce_entradas_y_ninguna_accion_se_repite() {
    let acciones: Vec<AppAction> = entradas().iter().map(|i| i.action).collect();
    assert_eq!(acciones.len(), 14);
    for (i, a) in acciones.iter().enumerate() {
        assert!(
            !acciones[i + 1..].contains(a),
            "la acción {a:?} aparece dos veces"
        );
    }
    // `Nueva imagen…` (unidad 19) es la primera y `Convertir modo…` (unidad 20) va tras
    // `Rehacer`; las otras 11 siguen siendo las de la 17.
    let mut esperado = vec![AppAction::NewImage];
    esperado.extend(&ACCIONES_DE_LA_TABLA[..6]);
    // `Borrar página` (I.2a) cierra `Editar`, antes de `Imagen`.
    esperado.push(AppAction::ClearPage);
    esperado.push(AppAction::ConvertMode);
    esperado.extend(&ACCIONES_DE_LA_TABLA[6..]);
    assert_eq!(acciones, esperado);
}

#[test]
fn fila_1_ninguna_entrada_ni_titulo_esta_vacio_ni_repetido() {
    let mut rotulos: Vec<&str> = MENUS.iter().map(titulo).collect();
    rotulos.extend(entradas().iter().map(|i| i.label));
    for (i, r) in rotulos.iter().enumerate() {
        assert!(!r.is_empty());
        assert!(!rotulos[i + 1..].contains(r), "rótulo repetido: {r}");
    }
}

// ---------------------------------------------------------------------------------------------
// Fila 1: atajos por plataforma
// ---------------------------------------------------------------------------------------------

/// Texto del atajo de cada entrada, en el orden de `ACCIONES_DE_LA_TABLA`.
fn textos_de_atajo(platform: Platform) -> Vec<Option<String>> {
    entradas()
        .iter()
        .map(|i| shortcut_for(i.action, platform).map(|s| shortcut_text(s, platform)))
        .collect()
}

fn algunos(textos: [Option<&str>; 14]) -> Vec<Option<String>> {
    textos.iter().map(|t| t.map(str::to_string)).collect()
}

#[test]
fn fila_1_atajos_en_linux() {
    assert_eq!(
        textos_de_atajo(Platform::Linux),
        algunos([
            Some("Ctrl+N"),
            Some("Ctrl+O"),
            Some("Ctrl+S"),
            Some("Ctrl+Mayús+S"),
            Some("Ctrl+Q"),
            Some("Ctrl+Z"),
            Some("Ctrl+Y"),
            Some("K"),
            None,
            Some(">"),
            Some("<"),
            Some("Inicio"),
            Some("Ctrl+1"),
            None,
        ])
    );
}

#[test]
fn fila_1_atajos_en_windows_son_los_de_linux_salvo_salir_que_no_tiene() {
    assert_eq!(
        textos_de_atajo(Platform::Windows),
        algunos([
            Some("Ctrl+N"),
            Some("Ctrl+O"),
            Some("Ctrl+S"),
            Some("Ctrl+Mayús+S"),
            None,
            Some("Ctrl+Z"),
            Some("Ctrl+Y"),
            Some("K"),
            None,
            Some(">"),
            Some("<"),
            Some("Inicio"),
            Some("Ctrl+1"),
            None,
        ])
    );
    assert_eq!(shortcut_for(AppAction::Quit, Platform::Windows), None);
}

#[test]
fn fila_1_atajos_en_mac_usan_cmd_rehacer_es_cmd_mayus_z_y_salir_no_tiene() {
    assert_eq!(
        textos_de_atajo(Platform::Mac),
        algunos([
            Some("Cmd+N"),
            Some("Cmd+O"),
            Some("Cmd+S"),
            Some("Cmd+Mayús+S"),
            None,
            Some("Cmd+Z"),
            Some("Cmd+Mayús+Z"),
            Some("K"),
            None,
            Some(">"),
            Some("<"),
            Some("Inicio"),
            Some("Cmd+1"),
            None,
        ])
    );
    assert_eq!(shortcut_for(AppAction::Quit, Platform::Mac), None);
}

#[test]
fn fila_1_licencias_no_tiene_atajo_en_ninguna_plataforma() {
    for platform in PLATAFORMAS {
        assert_eq!(shortcut_for(AppAction::ShowLicenses, platform), None);
    }
}

#[test]
fn fila_1_shortcut_for_da_la_tecla_y_los_modificadores_de_cada_entrada() {
    for platform in PLATAFORMAS {
        let redo = if platform == Platform::Mac {
            atajo(KeyId::Char('z'), true, true)
        } else {
            atajo(KeyId::Char('y'), true, false)
        };
        let quit = (platform == Platform::Linux)
            .then(|| atajo(KeyId::Char('q'), true, false))
            .flatten();
        let esperado = [
            (AppAction::Open, atajo(KeyId::Char('o'), true, false)),
            (AppAction::Save, atajo(KeyId::Char('s'), true, false)),
            (AppAction::SaveAs, atajo(KeyId::Char('s'), true, true)),
            (AppAction::Quit, quit),
            (AppAction::Undo, atajo(KeyId::Char('z'), true, false)),
            (AppAction::Redo, redo),
            (AppAction::ZoomIn, atajo(KeyId::Char('>'), false, false)),
            (AppAction::ZoomOut, atajo(KeyId::Char('<'), false, false)),
            (AppAction::ZoomFit, atajo(KeyId::Home, false, false)),
            (AppAction::ZoomActual, atajo(KeyId::Char('1'), true, false)),
            (AppAction::ShowLicenses, None),
        ];
        for (action, shortcut) in esperado {
            assert_eq!(
                shortcut_for(action, platform),
                shortcut,
                "{action:?} en {platform:?}"
            );
        }
    }
}

#[test]
fn fila_1_shortcut_text_de_cada_forma() {
    // Con `command`: `Ctrl+` en Linux y Windows, `Cmd+` en macOS; con `shift`, `Mayús+`.
    for (platform, command) in [
        (Platform::Linux, "Ctrl"),
        (Platform::Windows, "Ctrl"),
        (Platform::Mac, "Cmd"),
    ] {
        assert_eq!(
            shortcut_text(atajo(KeyId::Char('o'), true, false).unwrap(), platform),
            format!("{command}+O")
        );
        assert_eq!(
            shortcut_text(atajo(KeyId::Char('s'), true, true).unwrap(), platform),
            format!("{command}+Mayús+S")
        );
        assert_eq!(
            shortcut_text(atajo(KeyId::Char('1'), true, false).unwrap(), platform),
            format!("{command}+1")
        );
        assert_eq!(
            shortcut_text(atajo(KeyId::Char('z'), true, true).unwrap(), platform),
            format!("{command}+Mayús+Z")
        );
        // Sin `command` la tecla va sola, con su nombre en español o su signo.
        assert_eq!(
            shortcut_text(atajo(KeyId::Home, false, false).unwrap(), platform),
            "Inicio"
        );
        assert_eq!(
            shortcut_text(atajo(KeyId::Char('+'), false, false).unwrap(), platform),
            "+"
        );
        assert_eq!(
            shortcut_text(atajo(KeyId::Char('-'), false, false).unwrap(), platform),
            "-"
        );
    }
}

// ---------------------------------------------------------------------------------------------
// Fila 1: rótulos y fuente
// ---------------------------------------------------------------------------------------------

/// Todo el texto fijo del menú: títulos, rótulos y los atajos de cada plataforma.
fn textos_fijos() -> Vec<String> {
    let mut v: Vec<String> = MENUS.iter().map(|m| m.title.to_string()).collect();
    for item in entradas() {
        v.push(item.label.to_string());
        for platform in PLATAFORMAS {
            if let Some(s) = shortcut_for(item.action, platform) {
                v.push(shortcut_text(s, platform));
            }
        }
    }
    v
}

#[test]
fn fila_1_todo_el_texto_del_menu_esta_dentro_de_interface_charset() {
    for texto in textos_fijos() {
        for c in texto.chars() {
            assert!(
                INTERFACE_CHARSET.contains(c),
                "{c:?} de {texto:?} no está en INTERFACE_CHARSET"
            );
        }
    }
}

#[test]
fn fila_1_ningun_rotulo_pierde_un_caracter_por_replace_missing() {
    let tiene = |c: char| INTERFACE_CHARSET.contains(c);
    for texto in textos_fijos() {
        assert_eq!(replace_missing(&texto, tiene), texto);
        assert!(!texto.contains('?'), "{texto:?} lleva un `?`");
    }
}

#[test]
fn fila_1_con_la_fuente_real_printable_no_cambia_ningun_rotulo() {
    let ctx = Context::default();
    install_fonts(&ctx);
    // Los cambios de fuente se aplican al empezar el pase siguiente.
    for _ in 0..2 {
        ctx.run_ui(egui::RawInput::default(), |_| {})
            .drop_without_applying_deltas();
    }
    for texto in textos_fijos() {
        assert_eq!(printable(&ctx, &texto), texto);
    }
}

// ---------------------------------------------------------------------------------------------
// Fila 2: habilitación
// ---------------------------------------------------------------------------------------------

fn estado(can_undo: bool, can_redo: bool, zoom: Option<Zoom>) -> UiState {
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

/// `enabled` de las 11 acciones de la tabla, en el orden de `ACCIONES_DE_LA_TABLA`.
fn habilitadas(state: &UiState) -> Vec<bool> {
    ACCIONES_DE_LA_TABLA
        .iter()
        .map(|a| enabled(*a, state))
        .collect()
}

#[test]
fn fila_2_con_la_historia_vacia_deshacer_y_rehacer_estan_deshabilitados() {
    // Orden: Open, Save, SaveAs, Quit, Undo, Redo, ZoomIn, ZoomOut, ZoomFit, ZoomActual, Licencias.
    assert_eq!(
        habilitadas(&estado(false, false, x5())),
        [
            true, true, true, true, false, false, true, true, true, true, true
        ]
    );
}

#[test]
fn fila_2_solo_can_undo_habilita_deshacer() {
    assert_eq!(
        habilitadas(&estado(true, false, x5())),
        [
            true, true, true, true, true, false, true, true, true, true, true
        ]
    );
}

#[test]
fn fila_2_solo_can_redo_habilita_rehacer() {
    assert_eq!(
        habilitadas(&estado(false, true, x5())),
        [
            true, true, true, true, false, true, true, true, true, true, true
        ]
    );
}

#[test]
fn fila_2_sin_imagen_mostrable_los_tres_de_zoom_y_1_a_1_estan_deshabilitados() {
    assert_eq!(
        habilitadas(&estado(true, true, None)),
        [
            true, true, true, true, true, true, false, false, false, false, true
        ]
    );
}

#[test]
fn fila_2_en_zoom_min_solo_alejar_esta_deshabilitado() {
    // `Zoom::MIN` es 1/64 (antes ×1): ahí `Alejar` sigue deshabilitado, pero `1:1` ya vale
    // (el zoom no es ×1). La comparación usa la constante, no el número.
    assert_eq!(
        habilitadas(&estado(true, true, Some(Zoom::MIN))),
        [
            true, true, true, true, true, true, true, false, true, true, true
        ]
    );
}

#[test]
fn fila_2_en_x1_alejar_esta_habilitado_y_1_a_1_no() {
    // El estado que antes era el mínimo: ahora hay un nivel por debajo (1/2).
    assert_eq!(
        habilitadas(&estado(true, true, Zoom::from_int(1))),
        [
            true, true, true, true, true, true, true, true, true, false, true
        ]
    );
}

#[test]
fn fila_2_en_x5_todo_el_zoom_esta_habilitado() {
    assert_eq!(habilitadas(&estado(true, true, x5())), [true; 11]);
}

#[test]
fn fila_2_en_zoom_max_solo_acercar_esta_deshabilitado() {
    assert_eq!(
        habilitadas(&estado(true, true, Some(Zoom::MAX))),
        [
            true, true, true, true, true, true, false, true, true, true, true
        ]
    );
    assert_eq!(Zoom::MAX, Zoom::from_int(64).expect("×64"));
}

#[test]
fn fila_2_guardar_esta_habilitado_siempre_tambien_sin_cambios_ni_historia() {
    for can_undo in [false, true] {
        for can_redo in [false, true] {
            for zoom in [None, Some(Zoom::MIN), x5(), Some(Zoom::MAX)] {
                let state = estado(can_undo, can_redo, zoom);
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
fn fila_2_select_tool_solo_depende_del_bloqueo_tambien_la_herramienta_activa() {
    for activa in HERRAMIENTAS {
        let state = UiState {
            tool: activa,
            ..estado(false, false, None)
        };
        for tool in HERRAMIENTAS {
            assert!(enabled(AppAction::SelectTool(tool), &state));
        }
    }
}

#[test]
fn fila_2_zoom_in_es_falso_solo_en_x64_en_todos_los_niveles_enteros() {
    for n in 1..=64u32 {
        let zoom = Zoom::from_int(n).expect("nivel válido");
        let state = estado(true, true, Some(zoom));
        assert_eq!(enabled(AppAction::ZoomIn, &state), n != 64, "×{n}");
        assert_eq!(
            enabled(AppAction::ZoomOut, &state),
            zoom > Zoom::MIN,
            "×{n}"
        );
        assert_eq!(enabled(AppAction::ZoomActual, &state), n != 1, "×{n}");
        assert!(enabled(AppAction::ZoomFit, &state), "×{n}");
    }
}

// ---------------------------------------------------------------------------------------------
// Propiedad 1: tabla coherente
// ---------------------------------------------------------------------------------------------

/// La pulsación que hace un `Shortcut` (la tecla que muestra, con `command` y `Mayús` si los lleva).
fn pulsado(shortcut: Shortcut) -> Press {
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
fn acciones_de_menu_y_columna() -> Vec<AppAction> {
    let mut v: Vec<AppAction> = entradas().iter().map(|i| i.action).collect();
    v.extend(TOOL_BUTTONS.iter().map(|b| b.action));
    v.extend(ZOOM_BUTTONS.iter().map(|b| b.action));
    v
}

fn plataforma(i: u8) -> Platform {
    PLATAFORMAS[usize::from(i) % 3]
}

fn estado_de(
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
        tool: HERRAMIENTAS[usize::from(tool) % 5],
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
    fn propiedad_1_enabled_implica_no_bloqueada_y_los_bloqueos_apagan_lo_que_toca(
        can_undo: bool,
        can_redo: bool,
        zoom in prop::option::of(1u32..=64),
        tool in 0u8..5,
        locks in prop::array::uniform5(any::<bool>()),
    ) {
        let state = estado_de(can_undo, can_redo, zoom, tool, locks);
        for action in acciones_de_menu_y_columna() {
            let on = enabled(action, &state);
            // `enabled` implica `!is_blocked(.., Origin::Menu)`.
            if on {
                prop_assert!(!is_blocked(action, state.locks, Origin::Menu), "{:?}", action);
            }
            // Con un diálogo abierto, falso en todas.
            if state.locks.dialog {
                prop_assert!(!on, "{:?} con diálogo", action);
            }
            // Con gesto o campo de texto, falso en todas salvo las cuatro de zoom.
            if (state.locks.gesture || state.locks.text_focus) && !action.is_zoom() {
                prop_assert!(!on, "{:?} con gesto o texto", action);
            }
            // Las acciones de zoom no se apagan por gesto, quita ni campo de texto (con el menú).
            if action.is_zoom() {
                let sin_nada_mas = !state.locks.dialog && !state.locks.modal;
                if sin_nada_mas && state.zoom.is_some() {
                    let por_nivel = match action {
                        AppAction::ZoomIn => state.zoom < Some(Zoom::MAX),
                        AppAction::ZoomOut => state.zoom > Some(Zoom::MIN),
                        AppAction::ZoomActual => state.zoom != Zoom::from_int(1),
                        _ => true,
                    };
                    prop_assert_eq!(on, por_nivel, "{:?}", action);
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
    fn propiedad_1_el_atajo_de_cada_entrada_traducido_da_su_accion(platform_i in 0u8..3) {
        let platform = plataforma(platform_i);
        for item in entradas() {
            if let Some(shortcut) = shortcut_for(item.action, platform) {
                prop_assert_eq!(
                    Keymap::default_scheme().action_for(&pulsado(shortcut), platform),
                    Some(item.action),
                    "{}", item.label
                );
            }
        }
        // La letra de la columna sale de la tabla y es la tecla de herramienta (el Borrador, `k`).
        for button in TOOL_BUTTONS.iter() {
            let letra = Keymap::default_scheme().shown(button.action, platform);
            let letra = letra.expect("los botones de herramienta la muestran");
            prop_assert_eq!(
                Keymap::default_scheme().action_for(&pulsado(letra), platform),
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
fn fila_9_shortcut_for_nueva_imagen_es_command_n_en_las_tres_plataformas() {
    for (platform, texto) in [
        (Platform::Linux, "Ctrl+N"),
        (Platform::Windows, "Ctrl+N"),
        (Platform::Mac, "Cmd+N"),
    ] {
        let shortcut = shortcut_for(AppAction::NewImage, platform)
            .unwrap_or_else(|| panic!("{platform:?}: Nueva imagen… tiene atajo"));
        assert_eq!(
            shortcut,
            atajo(KeyId::Char('n'), true, false).unwrap(),
            "{platform:?}"
        );
        assert_eq!(shortcut_text(shortcut, platform), texto, "{platform:?}");
    }
}

#[test]
fn fila_9_nueva_imagen_esta_en_archivo_la_primera_con_su_accion() {
    let archivo = MENUS
        .iter()
        .find(|m| m.title == "Archivo")
        .expect("Archivo");
    let primera = archivo.items.first().expect("Archivo tiene entradas");
    assert_eq!(primera.label, "Nueva imagen…");
    assert_eq!(primera.action, AppAction::NewImage);
    assert!(!primera.separator_before);
    // En ningún otro menú.
    let veces = MENUS
        .iter()
        .flat_map(|m| m.items.iter())
        .filter(|item| item.action == AppAction::NewImage)
        .count();
    assert_eq!(veces, 1);
}

#[test]
fn fila_9_enabled_de_nueva_imagen_no_depende_de_la_historia_ni_del_zoom() {
    for can_undo in [false, true] {
        for can_redo in [false, true] {
            for zoom in [
                None,
                Zoom::from_int(1),
                x5(),
                Some(Zoom::MIN),
                Some(Zoom::MAX),
            ] {
                let state = estado(can_undo, can_redo, zoom);
                assert!(
                    enabled(AppAction::NewImage, &state),
                    "undo {can_undo}, redo {can_redo}, zoom {zoom:?}"
                );
            }
        }
    }
}

#[test]
fn fila_9_enabled_de_nueva_imagen_esta_deshabilitado_con_cada_bloqueo() {
    for (nombre, locks) in [
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
            ..estado(true, true, x5())
        };
        assert!(!enabled(AppAction::NewImage, &state), "{nombre}");
        assert!(
            is_blocked(AppAction::NewImage, locks, Origin::Menu),
            "{nombre}"
        );
    }
}

// ---------------------------------------------------------------------------------------------
// Unidad 20: «Imagen → Convertir modo…» (specs/hito-2-convertir-modo.md, fila 10, parte del menú)
// ---------------------------------------------------------------------------------------------
//
// La tabla ya está adaptada en las pruebas de la fila 1 (cinco menús, rótulos, orden y texto de los
// atajos): aquí el menú `Imagen` por sí solo, la falta de atajo de `ConvertMode`, su `enabled` con
// los cinco bloqueos y el ancho de los desplegables (`Imagen` mide 131 u; `Ver`, 208, sigue siendo
// el más ancho).

use px_ui::menu::popup_width_u;

fn menu_imagen() -> Menu {
    *MENUS
        .iter()
        .find(|m| m.title == "Imagen")
        .expect("la barra tiene el menú Imagen")
}

#[test]
fn fila_10_imagen_es_el_tercer_menu_entre_editar_y_ver_con_una_sola_entrada() {
    let titulos: Vec<&str> = MENUS.iter().map(titulo).collect();
    assert_eq!(titulos.iter().position(|t| *t == "Imagen"), Some(2));
    assert_eq!(titulos[1], "Editar");
    assert_eq!(titulos[3], "Ver");

    let imagen = menu_imagen();
    assert_eq!(imagen.items.len(), 1, "una sola entrada");
    let entrada = imagen.items[0];
    assert_eq!(entrada.label, "Convertir modo…");
    assert_eq!(entrada.action, AppAction::ConvertMode);
    assert!(!entrada.separator_before, "sin separador");
    assert!(imagen.submenus.is_empty(), "sin submenús");
}

#[test]
fn fila_10_convertir_modo_esta_una_sola_vez_en_la_barra_y_solo_en_imagen() {
    let veces: Vec<&str> = MENUS
        .iter()
        .filter(|m| m.items.iter().any(|i| i.action == AppAction::ConvertMode))
        .map(titulo)
        .collect();
    assert_eq!(veces, ["Imagen"]);
    let total = entradas()
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
fn fila_10_convertir_modo_no_tiene_atajo_ni_texto_de_atajo_en_las_tres_plataformas() {
    for platform in PLATAFORMAS {
        assert_eq!(
            shortcut_for(AppAction::ConvertMode, platform),
            None,
            "{platform:?}"
        );
    }
    // Y en la tabla del menú, la posición de la entrada nueva no lleva texto de atajo.
    for platform in PLATAFORMAS {
        let posicion = entradas()
            .iter()
            .position(|i| i.action == AppAction::ConvertMode)
            .expect("la entrada existe");
        assert_eq!(textos_de_atajo(platform)[posicion], None, "{platform:?}");
    }
}

#[test]
fn fila_10_enabled_de_convertir_modo_no_depende_de_la_historia_ni_del_zoom() {
    for can_undo in [false, true] {
        for can_redo in [false, true] {
            for zoom in [
                None,
                Zoom::from_int(1),
                x5(),
                Some(Zoom::MIN),
                Some(Zoom::MAX),
            ] {
                let state = estado(can_undo, can_redo, zoom);
                assert!(
                    enabled(AppAction::ConvertMode, &state),
                    "undo {can_undo}, redo {can_redo}, zoom {zoom:?}"
                );
            }
        }
    }
}

#[test]
fn fila_10_enabled_de_convertir_modo_esta_deshabilitado_con_cada_uno_de_los_cinco_bloqueos() {
    for (nombre, locks) in [
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
            ..estado(true, true, x5())
        };
        assert!(!enabled(AppAction::ConvertMode, &state), "{nombre}");
        assert!(
            is_blocked(AppAction::ConvertMode, locks, Origin::Menu),
            "{nombre}"
        );
    }
    // Sin bloqueos, habilitada.
    assert!(enabled(AppAction::ConvertMode, &estado(true, true, x5())));
}

#[test]
fn fila_10_el_desplegable_de_imagen_mide_131_u_y_el_de_ver_208_el_mas_ancho() {
    for platform in PLATAFORMAS {
        assert_eq!(popup_width_u(&menu_imagen(), platform), 131, "{platform:?}");
        let ver = MENUS.iter().find(|m| m.title == "Ver").expect("Ver");
        assert_eq!(popup_width_u(ver, platform), 208, "{platform:?}");
        assert!(
            popup_width_u(&menu_imagen(), platform) < popup_width_u(ver, platform),
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
