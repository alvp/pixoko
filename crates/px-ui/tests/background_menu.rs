//! Tests del menú «Fondo del lienzo» y de los bloqueos de su acción (specs/hito-2-damero-lienzo.md,
//! «Casos» fila 12 y, de la fila 13, la parte de `is_blocked` y `enabled`; el despacho de la fila 13
//! está en `px-app/src/dispatch/tests.rs`). Es lógica pura: la tabla `MENUS`, `Menu::submenus`,
//! `background_checked`, `shortcut_for`, `action_for_shortcut`, `is_blocked` y `enabled` no usan
//! ventana; solo los rótulos con la fuente real y la comprobación de que `show_menu_with_background`
//! se pinta usan un `egui::Context` con `RawInput` sintético (sin GPU ni capturas).
//!
//! - Fila 12: `Ver` conserva sus 4 entradas y gana **un** submenú `Fondo del lienzo` con `Damero`,
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
//! Suposiciones sobre las firmas de «Piezas»: `Submenu` es `Clone, Copy, Debug, PartialEq, Eq` (como
//! `Menu`, que lo contiene) y `AppAction` sigue siendo `Clone, Copy, Debug, PartialEq, Eq`. La marca
//! de la entrada activa dentro del desplegable no se prueba aquí (es de la prueba manual: depende de
//! cómo se dibuje); sí `background_checked`, que decide dónde va.

use egui::{Key, Ui};
use px_ui::actions::{
    AppAction, Locks, Origin, Platform, UiState, action_for_shortcut, enabled, is_blocked,
};
use px_ui::background::CanvasBackground;
use px_ui::menu::{
    MENUS, Menu, MenuItem, Submenu, background_checked, shortcut_for, shortcut_text, show_menu,
    show_menu_with_background,
};
use px_ui::text::{INTERFACE_CHARSET, printable, replace_missing};
use px_ui::tool::Tool;
use px_ui::view::Zoom;

mod common;
use common::{contexto, fotograma, modificadores, pintado};

const FONDOS: [CanvasBackground; 4] = [
    CanvasBackground::Checker,
    CanvasBackground::Black,
    CanvasBackground::Gray,
    CanvasBackground::White,
];
const PLATAFORMAS: [Platform; 3] = [Platform::Linux, Platform::Windows, Platform::Mac];
const ORIGENES: [Origin; 3] = [Origin::Menu, Origin::Button, Origin::Key];
const HERRAMIENTAS: [Tool; 5] = [
    Tool::Pencil,
    Tool::Line,
    Tool::Fill,
    Tool::Dropper,
    Tool::Eraser,
];

/// Las 11 entradas de `Menu::items`, en el orden del menú (las de la unidad 17).
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

fn accion(b: CanvasBackground) -> AppAction {
    AppAction::CanvasBackground(b)
}

fn las_cuatro() -> [AppAction; 4] {
    FONDOS.map(accion)
}

/// Las acciones que ya existían: las 11 de la tabla, `CloseLicenses`, las cinco herramientas y las
/// nueve ranuras de color.
fn acciones_anteriores() -> Vec<AppAction> {
    let mut v = ACCIONES_DE_LA_TABLA.to_vec();
    v.push(AppAction::CloseLicenses);
    v.extend(HERRAMIENTAS.map(AppAction::SelectTool));
    v.extend((1..=9).map(AppAction::SelectColor));
    v
}

fn todas_las_acciones() -> Vec<AppAction> {
    let mut v = acciones_anteriores();
    v.extend(las_cuatro());
    v
}

fn el_menu(titulo: &str) -> &'static Menu {
    MENUS
        .iter()
        .find(|m| m.title == titulo)
        .unwrap_or_else(|| panic!("no hay menú {titulo:?}"))
}

fn rotulos_y_acciones(items: &[MenuItem]) -> Vec<(&'static str, AppAction)> {
    items.iter().map(|i| (i.label, i.action)).collect()
}

fn bloqueos(bits: u8) -> Locks {
    Locks {
        gesture: bits & 1 != 0,
        dialog: bits & 2 != 0,
        text_focus: bits & 4 != 0,
        removal: bits & 8 != 0,
        modal: bits & 16 != 0,
    }
}

fn assert_plano<T: Clone + Copy + std::fmt::Debug + PartialEq + Eq>() {}

// ---------------------------------------------------------------------------------------------
// Fila 12: la tabla
// ---------------------------------------------------------------------------------------------

#[test]
fn fila_12_los_tipos_son_planos() {
    assert_plano::<AppAction>();
    assert_plano::<CanvasBackground>();
    assert_plano::<Menu>();
    assert_plano::<Submenu>();
    assert_plano::<MenuItem>();
}

#[test]
fn fila_12_ver_conserva_sus_4_entradas() {
    let ver = el_menu("Ver");
    assert_eq!(
        rotulos_y_acciones(ver.items),
        [
            ("Acercar", AppAction::ZoomIn),
            ("Alejar", AppAction::ZoomOut),
            ("Ajustar a la ventana", AppAction::ZoomFit),
            ("1:1", AppAction::ZoomActual),
        ]
    );
}

#[test]
fn fila_12_ver_gana_un_submenu_fondo_del_lienzo_con_las_cuatro_opciones_en_orden() {
    let ver = el_menu("Ver");
    assert_eq!(ver.submenus.len(), 1, "un solo submenú");
    let fondo = &ver.submenus[0];
    assert_eq!(fondo.label, "Fondo del lienzo");
    assert_eq!(
        rotulos_y_acciones(fondo.items),
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
        ]
    );
    for item in fondo.items {
        assert!(!item.separator_before, "{}", item.label);
    }
}

#[test]
fn fila_12_el_resto_de_menus_no_tiene_submenus_y_siguen_siendo_cuatro_en_orden() {
    let titulos: Vec<&str> = MENUS.iter().map(|m| m.title).collect();
    assert_eq!(titulos, ["Archivo", "Editar", "Ver", "Ayuda"]);
    for titulo in ["Archivo", "Editar", "Ayuda"] {
        assert!(el_menu(titulo).submenus.is_empty(), "{titulo}");
    }
}

#[test]
fn fila_12_menu_items_son_las_11_entradas_de_la_17_y_nueva_imagen_de_la_19() {
    let acciones: Vec<AppAction> = MENUS
        .iter()
        .flat_map(|m| m.items.iter().map(|i| i.action))
        .collect();
    // `Nueva imagen…` (unidad 19) es la primera; las otras 11 son las de la 17, sin cambios.
    let mut esperado = vec![AppAction::NewImage];
    esperado.extend(ACCIONES_DE_LA_TABLA);
    assert_eq!(acciones, esperado);
    // Ninguna acción del fondo cuela en `items`.
    for b in FONDOS {
        assert!(!acciones.contains(&accion(b)));
    }
    // Los rótulos y los separadores de la 17, como estaban.
    let archivo = el_menu("Archivo");
    assert_eq!(
        rotulos_y_acciones(archivo.items),
        [
            ("Nueva imagen…", AppAction::NewImage),
            ("Abrir…", AppAction::Open),
            ("Guardar", AppAction::Save),
            ("Guardar como…", AppAction::SaveAs),
            ("Salir", AppAction::Quit),
        ]
    );
    assert_eq!(
        rotulos_y_acciones(el_menu("Editar").items),
        [("Deshacer", AppAction::Undo), ("Rehacer", AppAction::Redo)]
    );
    assert_eq!(
        rotulos_y_acciones(el_menu("Ayuda").items),
        [("Licencias", AppAction::ShowLicenses)]
    );
}

#[test]
fn fila_12_las_cuatro_acciones_del_submenu_son_distintas_y_no_estan_en_items() {
    let ver = el_menu("Ver");
    let acciones: Vec<AppAction> = ver.submenus[0].items.iter().map(|i| i.action).collect();
    assert_eq!(acciones, las_cuatro());
    for (i, a) in acciones.iter().enumerate() {
        assert!(!acciones[i + 1..].contains(a), "{a:?} repetida");
    }
}

// ---------------------------------------------------------------------------------------------
// Fila 12: la marca
// ---------------------------------------------------------------------------------------------

#[test]
fn fila_12_background_checked_es_cierto_solo_para_la_accion_del_fondo_activo() {
    for activo in FONDOS {
        for action in todas_las_acciones() {
            assert_eq!(
                background_checked(action, activo),
                action == accion(activo),
                "{action:?} con {activo:?} activo"
            );
        }
    }
}

#[test]
fn fila_12_con_cada_fondo_activo_hay_exactamente_una_marca_en_el_submenu() {
    let ver = el_menu("Ver");
    for activo in FONDOS {
        let marcadas: Vec<&str> = ver.submenus[0]
            .items
            .iter()
            .filter(|i| background_checked(i.action, activo))
            .map(|i| i.label)
            .collect();
        let esperado = match activo {
            CanvasBackground::Checker => "Damero",
            CanvasBackground::Black => "Negro",
            CanvasBackground::Gray => "Gris",
            CanvasBackground::White => "Blanco",
        };
        assert_eq!(marcadas, [esperado], "{activo:?} activo");
    }
}

#[test]
fn fila_12_background_checked_es_falso_para_las_entradas_de_los_menus_de_la_17() {
    for activo in FONDOS {
        for menu in MENUS.iter() {
            for item in menu.items {
                assert!(!background_checked(item.action, activo), "{}", item.label);
            }
        }
    }
}

// ---------------------------------------------------------------------------------------------
// Fila 12: sin atajo
// ---------------------------------------------------------------------------------------------

#[test]
fn fila_12_ningun_atajo_en_ninguna_plataforma() {
    for platform in PLATAFORMAS {
        for b in FONDOS {
            assert_eq!(
                shortcut_for(accion(b), platform),
                None,
                "{b:?} en {platform:?}"
            );
        }
    }
}

#[test]
fn fila_12_ninguna_tecla_con_ningun_modificador_da_una_accion_de_fondo() {
    for platform in PLATAFORMAS {
        for key in Key::ALL {
            for bits in 0..32u8 {
                let resultado = action_for_shortcut(*key, modificadores(bits), platform);
                assert!(
                    !matches!(resultado, Some(AppAction::CanvasBackground(_))),
                    "{key:?} con modificadores {bits:#07b} en {platform:?}: {resultado:?}"
                );
            }
        }
    }
}

// ---------------------------------------------------------------------------------------------
// Fila 12: rótulos
// ---------------------------------------------------------------------------------------------

/// Los rótulos nuevos: el del submenú y los de sus cuatro entradas.
fn rotulos_nuevos() -> Vec<&'static str> {
    let fondo = &el_menu("Ver").submenus[0];
    let mut v = vec![fondo.label];
    v.extend(fondo.items.iter().map(|i| i.label));
    v
}

#[test]
fn fila_12_los_rotulos_nuevos_estan_dentro_de_interface_charset() {
    for texto in rotulos_nuevos() {
        for c in texto.chars() {
            assert!(
                INTERFACE_CHARSET.contains(c),
                "{c:?} de {texto:?} no está en INTERFACE_CHARSET"
            );
        }
    }
}

#[test]
fn fila_12_ningun_rotulo_nuevo_pierde_un_caracter_ni_lleva_un_interrogante() {
    let tiene = |c: char| INTERFACE_CHARSET.contains(c);
    for texto in rotulos_nuevos() {
        assert_eq!(replace_missing(texto, tiene), texto);
        assert!(!texto.contains('?'), "{texto:?} lleva un `?`");
        assert!(!texto.is_empty());
    }
}

#[test]
fn fila_12_con_la_fuente_real_printable_no_cambia_ningun_rotulo_nuevo() {
    let ctx = contexto(1.0);
    for texto in rotulos_nuevos() {
        assert_eq!(printable(&ctx, texto), texto);
    }
}

#[test]
fn fila_12_sin_rotulos_repetidos_con_los_de_menus() {
    // Todos los rótulos de la barra: títulos, entradas y los nuevos.
    let mut todos: Vec<&str> = MENUS.iter().map(|m| m.title).collect();
    todos.extend(MENUS.iter().flat_map(|m| m.items.iter().map(|i| i.label)));
    let n_anteriores = todos.len();
    todos.extend(rotulos_nuevos());
    for (i, r) in todos.iter().enumerate() {
        assert!(!todos[i + 1..].contains(r), "rótulo repetido: {r}");
    }
    assert_eq!(
        todos.len(),
        n_anteriores + 5,
        "el submenú y sus cuatro entradas"
    );
}

#[test]
fn fila_12_el_texto_de_los_atajos_de_la_17_no_cambia() {
    // `shortcut_text` y `shortcut_for` de las entradas de siempre, como estaban.
    let s = shortcut_for(AppAction::ZoomActual, Platform::Linux).expect("tiene atajo");
    assert_eq!(shortcut_text(s, Platform::Linux), "Ctrl+1");
    assert_eq!(shortcut_for(AppAction::ShowLicenses, Platform::Linux), None);
}

// ---------------------------------------------------------------------------------------------
// show_menu y show_menu_with_background
// ---------------------------------------------------------------------------------------------

#[test]
fn show_menu_conserva_su_firma_y_la_version_con_el_fondo_es_nueva() {
    let _: fn(&mut Ui, &UiState, Platform) -> Option<AppAction> = show_menu;
    let _: fn(&mut Ui, &UiState, Platform, CanvasBackground) -> Option<AppAction> =
        show_menu_with_background;
}

fn estado() -> UiState {
    UiState {
        can_undo: true,
        can_redo: false,
        zoom: Zoom::from_int(5),
        tool: Tool::Pencil,
        locks: Locks::default(),
    }
}

#[test]
fn show_menu_with_background_pinta_la_barra_con_cada_fondo_activo_como_show_menu() {
    let pantalla = egui::vec2(400.0, 300.0);
    for ppp in common::PPPS {
        let ctx = contexto(ppp);
        let estado = estado();
        let mut elegida = None;
        let con = fotograma(&ctx, pantalla, vec![], |ui| {
            elegida = show_menu(ui, &estado, Platform::Linux);
        });
        assert_eq!(elegida, None, "sin pulsar nada no se elige nada");
        let con = pintado(&con);
        for titulo in ["Archivo", "Editar", "Ver", "Ayuda"] {
            assert!(con.pinta(titulo), "ppp {ppp}: falta el título {titulo:?}");
        }
        for fondo in FONDOS {
            let mut elegida = None;
            let ctx = contexto(ppp);
            let otro = fotograma(&ctx, pantalla, vec![], |ui| {
                elegida = show_menu_with_background(ui, &estado, Platform::Linux, fondo);
            });
            assert_eq!(elegida, None);
            let otro = pintado(&otro);
            for titulo in ["Archivo", "Editar", "Ver", "Ayuda"] {
                assert!(otro.pinta(titulo), "ppp {ppp} {fondo:?}: falta {titulo:?}");
            }
            // Cerrado, el desplegable no se ve: la barra es la misma con cualquier fondo activo.
            assert_eq!(
                format!("{:?}", otro.todos_los_textos()),
                format!("{:?}", con.todos_los_textos()),
                "ppp {ppp} {fondo:?}"
            );
        }
    }
}

// ---------------------------------------------------------------------------------------------
// Fila 13, parte pura: bloqueos y habilitación
// ---------------------------------------------------------------------------------------------

#[test]
fn fila_13_is_blocked_es_cierto_solo_con_dialog_modal_o_text_focus_con_cualquier_origen() {
    for b in FONDOS {
        for bits in 0..32u8 {
            let locks = bloqueos(bits);
            for origin in ORIGENES {
                assert_eq!(
                    is_blocked(accion(b), locks, origin),
                    locks.dialog || locks.modal || locks.text_focus,
                    "{b:?} con {locks:?} y {origin:?}"
                );
            }
        }
    }
}

#[test]
fn fila_13_cada_bloqueo_solo() {
    let solo = |f: fn(&mut Locks)| {
        let mut l = Locks::default();
        f(&mut l);
        l
    };
    let casos: [(&str, Locks, bool); 6] = [
        ("ninguno", Locks::default(), false),
        ("gesture", solo(|l| l.gesture = true), false),
        ("removal", solo(|l| l.removal = true), false),
        ("dialog", solo(|l| l.dialog = true), true),
        ("modal", solo(|l| l.modal = true), true),
        ("text_focus", solo(|l| l.text_focus = true), true),
    ];
    for b in FONDOS {
        for (nombre, locks, bloqueada) in casos {
            for origin in ORIGENES {
                assert_eq!(
                    is_blocked(accion(b), locks, origin),
                    bloqueada,
                    "{b:?} con {nombre} y {origin:?}"
                );
            }
        }
    }
}

#[test]
fn fila_13_no_la_bloquean_ni_el_gesto_ni_la_quita_ni_los_dos_juntos() {
    let gesto_y_quita = Locks {
        gesture: true,
        removal: true,
        ..Locks::default()
    };
    for b in FONDOS {
        for origin in ORIGENES {
            assert!(
                !is_blocked(accion(b), gesto_y_quita, origin),
                "{b:?} {origin:?}"
            );
        }
    }
}

#[test]
fn fila_13_no_es_zoom_ni_repite() {
    for b in FONDOS {
        assert!(!accion(b).is_zoom(), "{b:?}");
        assert!(!accion(b).ignores_repeat(), "{b:?}");
    }
}

fn estados() -> Vec<UiState> {
    let mut v = Vec::new();
    for can_undo in [false, true] {
        for can_redo in [false, true] {
            for zoom in [None, Some(Zoom::MIN), Zoom::from_int(5), Some(Zoom::MAX)] {
                for tool in HERRAMIENTAS {
                    for bits in 0..32u8 {
                        v.push(UiState {
                            can_undo,
                            can_redo,
                            zoom,
                            tool,
                            locks: bloqueos(bits),
                        });
                    }
                }
            }
        }
    }
    v
}

#[test]
fn fila_13_enabled_es_lo_contrario_de_is_blocked_con_el_origen_menu() {
    for state in estados() {
        for b in FONDOS {
            assert_eq!(
                enabled(accion(b), &state),
                !is_blocked(accion(b), state.locks, Origin::Menu),
                "{b:?} con {state:?}"
            );
        }
    }
}

#[test]
fn fila_13_con_un_campo_de_texto_enfocado_el_submenu_esta_deshabilitado_y_sin_el_habilitado() {
    let con_texto = UiState {
        locks: Locks {
            text_focus: true,
            ..Locks::default()
        },
        ..estado()
    };
    let sin_texto = estado();
    for b in FONDOS {
        assert!(!enabled(accion(b), &con_texto), "{b:?}");
        assert!(enabled(accion(b), &sin_texto), "{b:?}");
    }
}

/// Modelo de `is_blocked` de las acciones de antes, escrito aparte (el de `actions.rs`): (1)
/// `CloseLicenses` nunca; (2) `modal`: todo salvo `Quit`; (3) `dialog`: todo; (4) zoom: con
/// `text_focus` solo si el origen es `Key`; (5) el resto: `gesture` o `text_focus`, y `removal` solo
/// `SelectColor` y las que abren un diálogo (`ShowLicenses`, unidad 18, D2); (6) nada.
fn bloqueada_segun_el_modelo(action: AppAction, locks: Locks, origin: Origin) -> bool {
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
fn fila_13_las_acciones_anteriores_se_bloquean_igual_que_antes() {
    for action in acciones_anteriores() {
        for bits in 0..32u8 {
            let locks = bloqueos(bits);
            for origin in ORIGENES {
                assert_eq!(
                    is_blocked(action, locks, origin),
                    bloqueada_segun_el_modelo(action, locks, origin),
                    "{action:?} con {locks:?} y {origin:?}"
                );
            }
        }
    }
}

#[test]
fn fila_13_las_acciones_anteriores_se_habilitan_igual_que_antes() {
    for state in estados() {
        for action in acciones_anteriores() {
            let mut esperado = !bloqueada_segun_el_modelo(action, state.locks, Origin::Menu);
            if esperado {
                esperado = match action {
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
                esperado,
                "{action:?} con {state:?}"
            );
        }
    }
}
