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
use common::{PPPS, boton, contexto, fotograma, modificadores, mover, pintado, tolerancia_de_egui};

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
        stroke: false,
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
fn fila_12_ver_gana_un_submenu_fondo_del_lienzo_con_las_cinco_opciones_en_orden() {
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
            ("Color…", AppAction::BackgroundColor),
        ]
    );
    for item in fondo.items {
        assert!(!item.separator_before, "{}", item.label);
    }
}

#[test]
fn fila_12_el_resto_de_menus_no_tiene_submenus_y_son_cinco_en_orden() {
    let titulos: Vec<&str> = MENUS.iter().map(|m| m.title).collect();
    assert_eq!(titulos, ["Archivo", "Editar", "Imagen", "Ver", "Ayuda"]);
    for titulo in ["Archivo", "Editar", "Imagen", "Ayuda"] {
        assert!(el_menu(titulo).submenus.is_empty(), "{titulo}");
    }
}

#[test]
fn fila_12_menu_items_son_las_11_entradas_de_la_17_nueva_imagen_de_la_19_y_convertir_de_la_20() {
    let acciones: Vec<AppAction> = MENUS
        .iter()
        .flat_map(|m| m.items.iter().map(|i| i.action))
        .collect();
    // `Nueva imagen…` (unidad 19) es la primera y `Convertir modo…` (unidad 20) va tras
    // `Rehacer`; las otras 11 son las de la 17, sin cambios.
    let mut esperado = vec![AppAction::NewImage];
    esperado.extend(&ACCIONES_DE_LA_TABLA[..6]);
    // `Borrar página` (I.2a) cierra `Editar`, antes de `Imagen`.
    esperado.push(AppAction::ClearPage);
    esperado.push(AppAction::ConvertMode);
    esperado.extend(&ACCIONES_DE_LA_TABLA[6..10]);
    // `Ayuda` (I.5): la ayuda de la herramienta y los atajos preceden a las licencias.
    esperado.push(AppAction::ShowHelp(HelpTarget::Active));
    esperado.push(AppAction::ShowShortcuts);
    esperado.push(AppAction::ShowLicenses);
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
        [
            ("Deshacer", AppAction::Undo),
            ("Rehacer", AppAction::Redo),
            ("Borrar página", AppAction::ClearPage),
        ]
    );
    assert_eq!(
        rotulos_y_acciones(el_menu("Imagen").items),
        [("Convertir modo…", AppAction::ConvertMode)]
    );
    assert_eq!(
        rotulos_y_acciones(el_menu("Ayuda").items),
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
fn fila_12_las_cuatro_acciones_del_submenu_son_distintas_y_no_estan_en_items() {
    let ver = el_menu("Ver");
    let acciones: Vec<AppAction> = ver.submenus[0].items.iter().map(|i| i.action).collect();
    // Las cuatro fijas y, al final, `Color…` (unidad 19b).
    let mut esperado = las_cuatro().to_vec();
    esperado.push(AppAction::BackgroundColor);
    assert_eq!(acciones, esperado);
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
            // Con un color propio la marca va en `Color…` (unidad 19b).
            CanvasBackground::Custom(_) => "Color…",
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
        n_anteriores + 6,
        "el submenú y sus cinco entradas"
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

// ---------------------------------------------------------------------------------------------
// Unidad 19b, fila 2: la quinta entrada `Color…`
// ---------------------------------------------------------------------------------------------
//
// Suposiciones: `AppAction::BackgroundColor` es `Clone, Copy, Debug, PartialEq, Eq` como las demás;
// `submenu_width_u(&Submenu) -> i64` y `background_value_text(CanvasBackground) -> Option<String>`
// son las de «Piezas». El banco abre el submenú **con clics** (`Ver`, `Fondo del lienzo`), como lo
// hace `dialog_open_focus.rs` con `Ayuda → Licencias`, y mira lo pintado en orden.

const AZUL_ACERO: Rgba = Rgba::opaque(0x33, 0x66, 0x99);
const NEGRO: Rgba = Rgba::opaque(0, 0, 0);

/// Los seis fondos activos de la fila 2: los cuatro de siempre y dos colores propios (el segundo,
/// negro: no salta a `Negro`).
const ACTIVOS: [CanvasBackground; 6] = [
    CanvasBackground::Checker,
    CanvasBackground::Black,
    CanvasBackground::Gray,
    CanvasBackground::White,
    CanvasBackground::Custom(AZUL_ACERO),
    CanvasBackground::Custom(NEGRO),
];

fn el_submenu_de_fondo() -> &'static Submenu {
    &el_menu("Ver").submenus[0]
}

/// La acción cuya entrada lleva la marca con `activo`.
fn accion_marcada(activo: CanvasBackground) -> AppAction {
    match activo {
        CanvasBackground::Custom(_) => AppAction::BackgroundColor,
        otro => accion(otro),
    }
}

/// Las acciones de antes, las cuatro del fondo y la nueva.
fn todas_con_color() -> Vec<AppAction> {
    let mut v = todas_las_acciones();
    v.push(AppAction::BackgroundColor);
    v
}

#[test]
fn fila_2_la_accion_nueva_es_un_tipo_plano_y_distinta_de_las_otras() {
    assert_plano::<AppAction>();
    for otra in todas_las_acciones() {
        assert_ne!(AppAction::BackgroundColor, otra);
    }
}

#[test]
fn fila_2_fondo_del_lienzo_tiene_cinco_entradas_y_la_quinta_es_color_sin_separador() {
    let fondo = el_submenu_de_fondo();
    assert_eq!(fondo.label, "Fondo del lienzo");
    let rotulos: Vec<&str> = fondo.items.iter().map(|i| i.label).collect();
    assert_eq!(rotulos, ["Damero", "Negro", "Gris", "Blanco", "Color…"]);
    let ultima = fondo.items.last().expect("hay entradas");
    assert_eq!(ultima.action, AppAction::BackgroundColor);
    assert!(fondo.items.iter().all(|i| !i.separator_before));
    // Los puntos suspensivos son un solo carácter (U+2026) del juego, no tres puntos ni `?`.
    assert_eq!(ultima.label.chars().count(), 6);
    assert!(ultima.label.ends_with('…'));
    assert!(!ultima.label.contains('?'));
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
fn fila_2_con_cada_fondo_activo_hay_exactamente_una_marca_y_con_custom_es_color() {
    let fondo = el_submenu_de_fondo();
    for activo in ACTIVOS {
        let marcadas: Vec<&str> = fondo
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
            CanvasBackground::Custom(_) => "Color…",
        };
        assert_eq!(marcadas, [esperado], "{activo:?} activo");
    }
}

#[test]
fn fila_2_background_checked_solo_es_cierto_para_la_accion_marcada_con_cualquier_fondo() {
    for activo in ACTIVOS {
        for action in todas_con_color() {
            assert_eq!(
                background_checked(action, activo),
                action == accion_marcada(activo),
                "{action:?} con {activo:?} activo"
            );
        }
    }
}

#[test]
fn fila_2_con_un_color_propio_no_se_marca_ninguna_de_las_cuatro_fijas_ni_siquiera_con_el_mismo_color()
 {
    for propio in [
        CanvasBackground::Custom(AZUL_ACERO),
        CanvasBackground::Custom(NEGRO),
        CanvasBackground::Custom(Rgba::opaque(255, 255, 255)),
        CanvasBackground::Custom(Rgba::opaque(40, 40, 40)),
    ] {
        for fija in las_cuatro() {
            assert!(!background_checked(fija, propio), "{fija:?} con {propio:?}");
        }
        assert!(background_checked(AppAction::BackgroundColor, propio));
    }
    // Y al revés: con una fija activa, `Color…` no se marca.
    for activo in FONDOS {
        assert!(!background_checked(AppAction::BackgroundColor, activo));
    }
}

#[test]
fn fila_2_color_no_tiene_atajo_en_ninguna_plataforma_y_ninguna_tecla_la_da() {
    for platform in PLATAFORMAS {
        assert_eq!(
            shortcut_for(AppAction::BackgroundColor, platform),
            None,
            "{platform:?}"
        );
        for key in Key::ALL {
            for bits in 0..32u8 {
                assert_ne!(
                    action_for_shortcut(*key, modificadores(bits), platform),
                    Some(AppAction::BackgroundColor),
                    "{key:?} con modificadores {bits:#07b} en {platform:?}"
                );
            }
        }
    }
}

#[test]
fn fila_2_el_rotulo_color_esta_en_el_juego_sin_interrogante_y_printable_lo_deja_igual() {
    let color = "Color…";
    assert!(color.chars().all(|c| INTERFACE_CHARSET.contains(c)));
    let tiene = |c: char| INTERFACE_CHARSET.contains(c);
    assert_eq!(replace_missing(color, tiene), color);
    assert!(!color.contains('?'));
    for ppp in PPPS {
        let ctx = contexto(ppp);
        assert_eq!(printable(&ctx, color), color, "ppp {ppp}");
    }
    // Está en la tabla y no se repite con ningún otro rótulo de la barra.
    assert!(rotulos_nuevos().contains(&color));
    let mut todos: Vec<&str> = MENUS.iter().map(|m| m.title).collect();
    todos.extend(MENUS.iter().flat_map(|m| m.items.iter().map(|i| i.label)));
    assert!(!todos.contains(&color));
}

#[test]
fn fila_2_submenu_width_u_es_131_con_el_rotulo_mas_largo_mas_la_columna_del_valor() {
    let fondo = el_submenu_de_fondo();
    // 12 + (2 + rótulo más largo + 2 + 7) · 7 con «Color…» (6 caracteres, como `Damero`).
    let mas_largo = fondo
        .items
        .iter()
        .map(|i| i.label.chars().count() as i64)
        .max()
        .expect("hay entradas");
    assert_eq!(mas_largo, 6);
    assert_eq!(submenu_width_u(fondo), 12 + (2 + mas_largo + 2 + 7) * 7);
    assert_eq!(submenu_width_u(fondo), 131);
}

/// Lo que se ve de una forma, en orden de pintado.
#[derive(Clone, Debug)]
enum Forma {
    Texto {
        texto: String,
        rect: Rect,
        clip: Rect,
        colores: Vec<Color32>,
        opacidad: f32,
        /// Color del primer vértice de glifo tal como lo pinta la fuente: lo que cambia al atenuar.
        glifo: Option<Color32>,
    },
    Relleno {
        rect: Rect,
        fill: Color32,
        clip: Rect,
    },
}

impl Forma {
    fn aspecto(&self) -> (Vec<Color32>, u32) {
        match self {
            Forma::Texto {
                colores,
                opacidad,
                glifo,
                ..
            } => {
                let mut v = colores.clone();
                v.extend(glifo);
                (v, opacidad.to_bits())
            }
            Forma::Relleno { .. } => (vec![], 0),
        }
    }
}

fn aplana(shape: &Shape, clip: Rect, fuera: &mut Vec<Forma>) {
    match shape {
        Shape::Text(t) => {
            let job = &t.galley.job;
            let colores = job
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
            let glifo = t.galley.rows.iter().find_map(|fila| {
                let visuales = &fila.row.visuals;
                visuales.mesh.vertices[visuales.glyph_vertex_range.clone()]
                    .first()
                    .map(|v| v.color)
            });
            fuera.push(Forma::Texto {
                texto: t.galley.text().to_owned(),
                rect: Rect::from_min_size(t.pos, t.galley.size()),
                clip,
                colores,
                opacidad: t.opacity_factor,
                glifo,
            });
        }
        Shape::Rect(r) => fuera.push(Forma::Relleno {
            rect: r.rect,
            fill: r.fill,
            clip,
        }),
        Shape::Vec(formas) => {
            for forma in formas {
                aplana(forma, clip, fuera);
            }
        }
        _ => {}
    }
}

fn formas_en_orden(formas: &[ClippedShape]) -> Vec<Forma> {
    let mut fuera = Vec::new();
    for clipped in formas {
        aplana(&clipped.shape, clipped.clip_rect, &mut fuera);
    }
    fuera
}

/// `src` (premultiplicado, como egui) por encima de `dst` opaco.
fn encima(dst: Color32, src: Color32) -> Color32 {
    let k = 1.0 - f32::from(src.a()) / 255.0;
    let canal = |s: u8, d: u8| (f32::from(s) + f32::from(d) * k).round().clamp(0.0, 255.0) as u8;
    Color32::from_rgb(
        canal(src.r(), dst.r()),
        canal(src.g(), dst.g()),
        canal(src.b(), dst.b()),
    )
}

/// Contraste mínimo de texto (WCAG AA).
const MIN_TEXTO: f64 = 4.5;

const PANTALLA_DEL_MENU: Vec2 = vec2(600.0, 400.0);

/// La barra con el menú `Ver` y su submenú `Fondo del lienzo` abiertos con el ratón, un fotograma
/// tras otro como `App::ui`: `show_menu_with_background` con el fondo activo y los bloqueos dados.
struct BancoDelMenu {
    ctx: Context,
    fondo: CanvasBackground,
    locks: Locks,
    formas: Vec<Forma>,
    elegida: Option<AppAction>,
    /// El reloj de egui, en segundos: avanza en cada fotograma para que terminen las animaciones de
    /// los desplegables (con el tiempo parado se verían a medio aparecer, translúcidos).
    tiempo: f64,
}

impl BancoDelMenu {
    fn nuevo(ppp: f32, fondo: CanvasBackground, locks: Locks) -> BancoDelMenu {
        let ctx = contexto(ppp);
        ctx.set_visuals_of(egui::Theme::Dark, visuals(ppp));
        let mut banco = BancoDelMenu {
            ctx,
            fondo,
            locks,
            formas: Vec::new(),
            elegida: None,
            tiempo: 0.0,
        };
        banco.reposo();
        banco
    }

    /// Con `Ver → Fondo del lienzo` abierto con dos clics reales.
    fn abierto(ppp: f32, fondo: CanvasBackground, locks: Locks) -> BancoDelMenu {
        let mut banco = BancoDelMenu::nuevo(ppp, fondo, locks);
        let ver = banco.centro_de("Ver");
        banco.clic(ver);
        banco.reposo();
        let submenu = banco.centro_de("Fondo del lienzo");
        banco.clic(submenu);
        banco.reposo();
        banco
    }

    fn fotograma(&mut self, events: Vec<Event>) {
        let estado = UiState {
            can_undo: true,
            can_redo: true,
            zoom: Zoom::from_int(5),
            tool: Tool::Pencil,
            locks: self.locks,
        };
        let fondo = self.fondo;
        let mut elegida = None;
        self.tiempo += 0.25;
        let entrada = RawInput {
            screen_rect: Some(Rect::from_min_size(Pos2::ZERO, PANTALLA_DEL_MENU)),
            events,
            time: Some(self.tiempo),
            ..RawInput::default()
        };
        let mut salida = self.ctx.run_ui(entrada, |ui| {
            apply_scale(ui.ctx());
            elegida = show_menu_with_background(ui, &estado, Platform::Linux, fondo);
        });
        salida.textures_delta.clear();
        self.formas = formas_en_orden(&salida.shapes);
        if elegida.is_some() {
            self.elegida = elegida;
        }
    }

    /// Unos fotogramas sin eventos (1,5 s de reloj): lo que tarda en asentarse la animación.
    fn reposo(&mut self) {
        for _ in 0..6 {
            self.fotograma(vec![]);
        }
    }

    /// Mueve, pulsa y suelta en `pos`: tres fotogramas, como los entrega winit.
    fn clic(&mut self, pos: Pos2) {
        self.fotograma(vec![mover(pos)]);
        self.fotograma(vec![boton(pos, true)]);
        self.fotograma(vec![boton(pos, false)]);
    }

    /// El índice de la primera forma de texto que contiene `buscado`.
    fn indice_de(&self, buscado: &str) -> Option<usize> {
        self.formas
            .iter()
            .position(|f| matches!(f, Forma::Texto { texto, .. } if texto.contains(buscado)))
    }

    fn texto(&self, buscado: &str) -> Option<&Forma> {
        self.indice_de(buscado).map(|i| &self.formas[i])
    }

    fn rect_de(&self, buscado: &str) -> Rect {
        match self.texto(buscado) {
            Some(Forma::Texto { rect, .. }) => *rect,
            _ => panic!(
                "no se pinta {buscado:?}; hay {:?}",
                self.formas
                    .iter()
                    .filter_map(|f| match f {
                        Forma::Texto { texto, .. } => Some(texto.as_str()),
                        Forma::Relleno { .. } => None,
                    })
                    .collect::<Vec<_>>()
            ),
        }
    }

    fn centro_de(&self, buscado: &str) -> Pos2 {
        self.rect_de(buscado).center()
    }

    /// La fila donde está el texto `buscado` (el último relleno anterior que lo contiene, aunque
    /// sea transparente) y el color real que hay debajo: todos los rellenos anteriores que lo
    /// contienen, mezclados en orden sobre el fondo del lienzo (el que hay detrás del menú).
    fn fondo_bajo(&self, buscado: &str) -> Option<(Rect, Color32)> {
        let i = self.indice_de(buscado)?;
        let Forma::Texto { rect, clip, .. } = &self.formas[i] else {
            return None;
        };
        let visible = rect.intersect(*clip);
        let sonda = visible.shrink2(vec2(0.0, visible.height() * 0.25));
        let mut fila = None;
        let mut fondo = BACKGROUND;
        for forma in &self.formas[..i] {
            if let Forma::Relleno { rect, fill, clip } = forma
                && rect.intersect(*clip).expand(0.01).contains_rect(sonda)
            {
                fila = Some(*rect);
                fondo = encima(fondo, *fill);
            }
        }
        fila.map(|fila| (fila, fondo))
    }

    /// El color con el que se pinta el texto `buscado`: el del primer glifo (lo que sale por
    /// pantalla, con el atenuado y el color forzado ya aplicados).
    fn color_pintado(&self, buscado: &str) -> Color32 {
        match self.texto(buscado) {
            Some(Forma::Texto { glifo, colores, .. }) => glifo
                .or_else(|| colores.first().copied())
                .unwrap_or(Color32::PLACEHOLDER),
            _ => panic!("no se pinta {buscado:?}"),
        }
    }
}

fn sin_bloqueos() -> Locks {
    Locks::default()
}

#[test]
fn fila_2_el_submenu_se_abre_con_clics_y_pinta_sus_cinco_rotulos() {
    for ppp in PPPS {
        let banco = BancoDelMenu::abierto(ppp, CanvasBackground::Checker, sin_bloqueos());
        for rotulo in ["Damero", "Negro", "Gris", "Blanco", "Color…"] {
            assert!(
                banco.texto(rotulo).is_some(),
                "ppp {ppp}: falta {rotulo:?} en el submenú abierto"
            );
        }
    }
}

#[test]
fn fila_2_el_valor_solo_se_pinta_con_un_color_propio_y_a_la_derecha_de_color() {
    for ppp in PPPS {
        // Sin color propio no hay valor en ninguna fila: ni `#` ni dígitos hexadecimales sueltos.
        for fijo in FONDOS {
            let banco = BancoDelMenu::abierto(ppp, fijo, sin_bloqueos());
            assert!(banco.texto("Color…").is_some(), "ppp {ppp} {fijo:?}");
            assert!(
                banco.indice_de("#").is_none(),
                "ppp {ppp} {fijo:?}: sin color propio no se pinta ningún valor"
            );
        }
        for (c, texto) in [(AZUL_ACERO, "#336699"), (NEGRO, "#000000")] {
            let activo = CanvasBackground::Custom(c);
            assert_eq!(background_value_text(activo).as_deref(), Some(texto));
            let banco = BancoDelMenu::abierto(ppp, activo, sin_bloqueos());
            let valor = banco.rect_de(texto);
            let etiqueta = banco.rect_de("Color…");
            // A la derecha de la etiqueta y en la misma fila.
            assert!(
                valor.min.x >= etiqueta.max.x - 0.01,
                "ppp {ppp}: el valor {valor:?} va a la derecha de {etiqueta:?}"
            );
            assert!(
                (valor.center().y - etiqueta.center().y).abs() <= 1.0,
                "ppp {ppp}: el valor {valor:?} está en la fila de {etiqueta:?}"
            );
            // Dentro de la fila (el relleno de la entrada marcada) y sin salirse por la derecha.
            let (fila, _) = banco
                .fondo_bajo(texto)
                .unwrap_or_else(|| panic!("ppp {ppp}: el valor no tiene relleno debajo"));
            assert!(
                valor.max.x <= fila.max.x + 0.01,
                "ppp {ppp}: el valor {valor:?} cabe en la fila {fila:?}"
            );
            // Un solo valor pintado.
            let n = banco
                .formas
                .iter()
                .filter(|f| matches!(f, Forma::Texto { texto: t, .. } if t.contains(texto)))
                .count();
            assert_eq!(n, 1, "ppp {ppp}: {texto} se pinta una vez");
            // NO PROBADO: que el valor vaya en `TEXT` (D3). La fila marcada se rellena con
            // `ACCENT` y `TEXT` sobre `ACCENT` da un contraste de 1,47, que contradice el
            // «≥ 4,5 sobre la fila marcada» de la misma D3: se prueba el contraste (la prueba de
            // abajo) y no el color concreto.
        }
    }
}

#[test]
fn fila_2_el_valor_tiene_contraste_al_menos_4_5_sobre_su_fila_marcada_y_con_el_raton() {
    // Con un color propio la entrada `Color…` es siempre la marcada: el valor se ve sobre la fila
    // marcada y, con el puntero encima, sobre la marcada resaltada. El relleno «normal» (sin marca ni
    // puntero) nunca lleva el valor: solo aparece con `Custom`, que marca la fila. NO PROBADO ese
    // tercer estado por eso. El color que se mide es el que sale por pantalla (el del glifo).
    for ppp in PPPS {
        for (c, texto) in [(AZUL_ACERO, "#336699"), (NEGRO, "#000000")] {
            let activo = CanvasBackground::Custom(c);
            let mut banco = BancoDelMenu::abierto(ppp, activo, sin_bloqueos());
            for estado in ["marcada", "con el ratón"] {
                if estado == "con el ratón" {
                    let encima = banco.centro_de("Color…");
                    banco.fotograma(vec![mover(encima)]);
                    banco.reposo();
                }
                let (_, fondo) = banco
                    .fondo_bajo(texto)
                    .unwrap_or_else(|| panic!("ppp {ppp} {estado}: sin relleno bajo el valor"));
                let color = banco.color_pintado(texto);
                let ratio = contrast_ratio(color, fondo);
                assert!(
                    ratio >= MIN_TEXTO,
                    "ppp {ppp} {estado}: el valor {texto} ({color:?}) sobre {fondo:?} da {ratio:.2}"
                );
            }
        }
    }
}

#[test]
fn fila_2_el_submenu_mide_lo_mismo_a_las_cinco_escalas_con_y_sin_color_propio() {
    for ppp in PPPS {
        let u = ui_unit(ppp);
        // 131 u, el de la spec, y el que dice `submenu_width_u`: lo pintado cumple los dos.
        assert_eq!(submenu_width_u(el_submenu_de_fondo()), 131);
        let esperado = 131.0 * u;
        for activo in ACTIVOS {
            let banco = BancoDelMenu::abierto(ppp, activo, sin_bloqueos());
            let marcada = match activo {
                CanvasBackground::Checker => "Damero",
                CanvasBackground::Black => "Negro",
                CanvasBackground::Gray => "Gris",
                CanvasBackground::White => "Blanco",
                CanvasBackground::Custom(_) => "Color…",
            };
            // El ancho del submenú es el del relleno de la fila marcada (una fila mide el ancho
            // del desplegable).
            let (fila, _) = banco
                .fondo_bajo(marcada)
                .unwrap_or_else(|| panic!("ppp {ppp} {activo:?}: sin fila marcada"));
            let tolerancia = tolerancia_de_egui(ppp) / ppp;
            assert!(
                (fila.width() - esperado).abs() <= tolerancia,
                "ppp {ppp} {activo:?}: la fila mide {} pt y debía medir {esperado} pt (131 u)",
                fila.width()
            );
        }
    }
}

#[test]
fn fila_2_color_se_atenua_con_un_gesto_o_una_quita_y_las_cuatro_fijas_no() {
    let con_gesto = Locks {
        gesture: true,
        ..Locks::default()
    };
    let con_quita = Locks {
        removal: true,
        ..Locks::default()
    };
    for ppp in PPPS {
        let libre = BancoDelMenu::abierto(ppp, CanvasBackground::Checker, sin_bloqueos());
        for (nombre, locks) in [("gesto", con_gesto), ("quita", con_quita)] {
            let banco = BancoDelMenu::abierto(ppp, CanvasBackground::Checker, locks);
            let aspecto = |b: &BancoDelMenu, r: &str| b.texto(r).expect("se pinta").aspecto();
            assert_ne!(
                aspecto(&banco, "Color…"),
                aspecto(&libre, "Color…"),
                "ppp {ppp}, {nombre}: `Color…` se pinta atenuada"
            );
            for fija in ["Damero", "Negro", "Gris", "Blanco"] {
                assert_eq!(
                    aspecto(&banco, fija),
                    aspecto(&libre, fija),
                    "ppp {ppp}, {nombre}: {fija} sigue habilitada"
                );
            }
        }
    }
}

#[test]
fn fila_2_enabled_de_color_es_falso_con_un_gesto_o_una_quita_y_el_de_las_cuatro_fijas_cierto() {
    let base = estado();
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
        for fija in las_cuatro() {
            assert!(enabled(fija, &state), "{fija:?} con {locks:?}");
        }
    }
    // Sin bloqueos, las cinco están habilitadas.
    for action in las_cuatro().into_iter().chain([AppAction::BackgroundColor]) {
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
        for action in las_cuatro().into_iter().chain([AppAction::BackgroundColor]) {
            assert!(!enabled(action, &state), "{action:?} con {locks:?}");
        }
    }
}

// ---------------------------------------------------------------------------------------------
// Unidad 19b, fila 3 (parte pura): `BackgroundColor` abre un diálogo y se bloquea como los demás
// ---------------------------------------------------------------------------------------------

#[test]
fn fila_3_background_color_abre_un_dialogo_y_no_repite_ni_es_zoom() {
    assert!(AppAction::BackgroundColor.opens_dialog());
    assert!(!AppAction::BackgroundColor.ignores_repeat());
    assert!(!AppAction::BackgroundColor.is_zoom());
    // Las cuatro fijas no abren ningún diálogo.
    for fija in las_cuatro() {
        assert!(!fija.opens_dialog(), "{fija:?}");
    }
}

#[test]
fn fila_3_background_color_libre_con_todo_en_falso_y_bloqueada_con_cada_bloqueo_solo() {
    let solo = |f: fn(&mut Locks)| {
        let mut l = Locks::default();
        f(&mut l);
        l
    };
    let casos: [(&str, Locks, bool); 6] = [
        ("ninguno", Locks::default(), false),
        ("gesture", solo(|l| l.gesture = true), true),
        ("dialog", solo(|l| l.dialog = true), true),
        ("text_focus", solo(|l| l.text_focus = true), true),
        ("removal", solo(|l| l.removal = true), true),
        ("modal", solo(|l| l.modal = true), true),
    ];
    for (nombre, locks, bloqueada) in casos {
        for origin in ORIGENES {
            assert_eq!(
                is_blocked(AppAction::BackgroundColor, locks, origin),
                bloqueada,
                "{nombre} y {origin:?}"
            );
        }
        // Y la regla de la 18 para abrir un diálogo da lo mismo.
        assert_eq!(can_open_dialog(locks), !bloqueada, "{nombre}");
    }
}

#[test]
fn fila_3_con_las_32_combinaciones_se_bloquea_exactamente_con_cualquier_bloqueo() {
    for bits in 0..32u8 {
        let locks = bloqueos(bits);
        let alguno =
            locks.gesture || locks.dialog || locks.text_focus || locks.removal || locks.modal;
        for origin in ORIGENES {
            assert_eq!(
                is_blocked(AppAction::BackgroundColor, locks, origin),
                alguno,
                "{locks:?} y {origin:?}"
            );
        }
        assert_eq!(can_open_dialog(locks), !alguno, "{locks:?}");
        // Con cualquier combinación, bloquea igual que las otras que abren un diálogo.
        for otra in [
            AppAction::NewImage,
            AppAction::ConvertMode,
            AppAction::ShowLicenses,
        ] {
            assert_eq!(
                is_blocked(AppAction::BackgroundColor, locks, Origin::Menu),
                is_blocked(otra, locks, Origin::Menu),
                "{otra:?} con {locks:?}"
            );
        }
    }
}

#[test]
fn fila_3_enabled_de_background_color_es_lo_contrario_de_is_blocked_con_el_origen_menu() {
    for state in estados() {
        assert_eq!(
            enabled(AppAction::BackgroundColor, &state),
            !is_blocked(AppAction::BackgroundColor, state.locks, Origin::Menu),
            "{state:?}"
        );
    }
}

#[test]
fn fila_3_las_cuatro_fijas_siguen_sin_bloquearse_con_un_gesto_ni_con_una_quita() {
    // Lo que cambia con la nueva acción es solo ella: con un gesto en curso o una quita pendiente,
    // `Negro` se puede elegir y `Color…` no.
    for solo in [
        Locks {
            gesture: true,
            ..Locks::default()
        },
        Locks {
            removal: true,
            ..Locks::default()
        },
    ] {
        for origin in ORIGENES {
            for fija in las_cuatro() {
                assert!(!is_blocked(fija, solo, origin), "{fija:?} con {solo:?}");
            }
            assert!(is_blocked(AppAction::BackgroundColor, solo, origin));
        }
    }
    // Y las cuatro fijas se bloquean exactamente como antes con las 32 combinaciones.
    for bits in 0..32u8 {
        let locks = bloqueos(bits);
        for fija in las_cuatro() {
            for origin in ORIGENES {
                assert_eq!(
                    is_blocked(fija, locks, origin),
                    locks.dialog || locks.modal || locks.text_focus,
                    "{fija:?} con {locks:?} y {origin:?}"
                );
            }
        }
    }
}

// ---------------------------------------------------------------------------------------------
// Unidad 19c (specs/hito-2-contraste-menu.md): contraste de las entradas de los menús
// ---------------------------------------------------------------------------------------------
//
// Todo lo de abajo es un módulo de este ejecutable (D5) y reutiliza `BancoDelMenu`, `Forma`,
// `encima`, `fondo_bajo` (por índice, para no confundir «Guardar» con «Guardar como…») y los
// colores del tema. Reglas de medida:
//
// - El color de un texto es el del primer vértice de glifo (`Forma::Texto::glifo`), que es el de
//   pantalla pero **premultiplicado** (el atajo débil de egui es `#86868699`, lo desactivado lleva
//   alfa 128): se mezcla con `encima` sobre el relleno real de debajo antes de `contrast_ratio`.
// - El relleno real es el de `fondo_bajo`: todos los rellenos anteriores que contienen el texto,
//   mezclados en orden sobre el fondo del lienzo.
// - El banco abre los menús con clics (barra y submenú) y mueve el ratón con eventos reales. Aquí
//   se construye con un envoltorio (`Mesa`) que también admite la plataforma (el banco fija Linux),
//   así que la fila 3 prueba las tres plataformas pintadas de verdad, no solo `shortcut_text`.
// - «Fondos activos»: los seis de `ACTIVOS` (cuatro fijos, `Custom(#336699)` y `Custom(#000000)`).
// - La fila 5 compara posiciones y tamaños con las invariantes de la geometría de la 17 (margen de
//   6 u, fila de 16 u, texto de 14 u y 7 u por carácter, anchos de `popup_width_u` y
//   `submenu_width_u`), medidas sobre el código actual: no hay una captura «de antes» que comparar,
//   y un cambio de color no puede alterarlas. Es lo que se puede probar sin inventar cifras.
mod contraste_del_menu {
    use super::*;
    use proptest::prelude::*;
    use px_ui::menu::popup_width_u;
    use px_ui::theme::{ACCENT, SURFACE_0, TEXT};
    use std::collections::{BTreeMap, BTreeSet};

    /// Suelo de contraste del texto desactivado (visto bueno de D4): se compara con `>=`.
    const SUELO_DESACTIVADO: f64 = 2.0;
    /// Pares texto-fondo mínimos que debe recorrer la fila 1 (que no sea vacía por un fallo del
    /// banco): seis escenas, todas las filas y los seis fondos a las cinco escalas.
    const PARES_MINIMOS_FILA_1: usize = 10_000;

    // -----------------------------------------------------------------------------------------
    // Escenas
    // -----------------------------------------------------------------------------------------

    #[derive(Clone, Copy, Debug)]
    struct Escena {
        nombre: &'static str,
        menu: &'static str,
        /// Con `Ver → Fondo del lienzo` abierto además del desplegable.
        submenu: bool,
    }

    const fn escena(nombre: &'static str, menu: &'static str, submenu: bool) -> Escena {
        Escena {
            nombre,
            menu,
            submenu,
        }
    }

    const ESC_ARCHIVO: Escena = escena("Archivo", "Archivo", false);
    const ESC_EDITAR: Escena = escena("Editar", "Editar", false);
    const ESC_IMAGEN: Escena = escena("Imagen", "Imagen", false);
    const ESC_VER: Escena = escena("Ver", "Ver", false);
    const ESC_AYUDA: Escena = escena("Ayuda", "Ayuda", false);
    const ESC_FONDO: Escena = escena("Ver → Fondo del lienzo", "Ver", true);
    /// Archivo, Editar, Imagen, Ver y su submenú (spec) y, de propina, Ayuda.
    const ESCENAS: [Escena; 6] = [
        ESC_ARCHIVO,
        ESC_EDITAR,
        ESC_IMAGEN,
        ESC_VER,
        ESC_FONDO,
        ESC_AYUDA,
    ];

    fn marca(fondo: CanvasBackground, item: &MenuItem) -> String {
        format!(
            "{} {}",
            if background_checked(item.action, fondo) {
                '·'
            } else {
                ' '
            },
            item.label
        )
    }

    /// Los rótulos de las filas sobre las que se pone el ratón en la escena, uno a uno.
    fn filas_de(escena: Escena, fondo: CanvasBackground) -> Vec<String> {
        if escena.submenu {
            el_submenu_de_fondo()
                .items
                .iter()
                .map(|i| marca(fondo, i))
                .collect()
        } else {
            let menu = el_menu(escena.menu);
            let mut v: Vec<String> = menu.items.iter().map(|i| i.label.to_string()).collect();
            v.extend(menu.submenus.iter().map(|s| s.label.to_string()));
            v
        }
    }

    /// La acción de una fila a partir de su rótulo pintado (con o sin marca).
    fn accion_de_rotulo(texto: &str) -> Option<AppAction> {
        let de_menu = MENUS
            .iter()
            .flat_map(|m| m.items.iter())
            .find(|i| i.label == texto);
        if let Some(i) = de_menu {
            return Some(i.action);
        }
        let sin_marca = texto
            .strip_prefix("· ")
            .or_else(|| texto.strip_prefix("  "))?;
        el_submenu_de_fondo()
            .items
            .iter()
            .find(|i| i.label == sin_marca)
            .map(|i| i.action)
    }

    // -----------------------------------------------------------------------------------------
    // El banco con plataforma
    // -----------------------------------------------------------------------------------------

    fn idx_exacto(b: &BancoDelMenu, buscado: &str) -> Option<usize> {
        b.formas
            .iter()
            .position(|f| matches!(f, Forma::Texto { texto, .. } if texto == buscado))
    }

    fn rect_exacto(b: &BancoDelMenu, buscado: &str) -> Rect {
        match idx_exacto(b, buscado) {
            Some(i) => match &b.formas[i] {
                Forma::Texto { rect, .. } => *rect,
                Forma::Relleno { .. } => unreachable!(),
            },
            None => panic!(
                "no se pinta {buscado:?}; hay {:?}",
                b.formas
                    .iter()
                    .filter_map(|f| match f {
                        Forma::Texto { texto, .. } => Some(texto.as_str()),
                        Forma::Relleno { .. } => None,
                    })
                    .collect::<Vec<_>>()
            ),
        }
    }

    /// Como `BancoDelMenu::fondo_bajo`, pero por índice de forma.
    fn fondo_en(b: &BancoDelMenu, i: usize) -> Option<(Rect, Color32)> {
        let Forma::Texto { rect, clip, .. } = &b.formas[i] else {
            return None;
        };
        let visible = rect.intersect(*clip);
        let sonda = visible.shrink2(vec2(0.0, visible.height() * 0.25));
        let mut fila = None;
        let mut fondo = BACKGROUND;
        for forma in &b.formas[..i] {
            if let Forma::Relleno { rect, fill, clip } = forma
                && rect.intersect(*clip).expand(0.01).contains_rect(sonda)
            {
                fila = Some(*rect);
                fondo = encima(fondo, *fill);
            }
        }
        fila.map(|fila| (fila, fondo))
    }

    /// Un texto pintado con su relleno real y su contraste.
    #[derive(Clone, Debug)]
    struct Medida {
        texto: String,
        rect: Rect,
        /// El color del glifo tal cual lo pinta egui (premultiplicado).
        pintado: Color32,
        /// La fila (relleno más cercano) y el color real de debajo; `None` si no hay relleno.
        fila: Option<Rect>,
        fondo: Option<Color32>,
        /// `pintado` mezclado sobre `fondo`: lo que ve el usuario.
        pantalla: Option<Color32>,
        ratio: Option<f64>,
    }

    impl Medida {
        fn describe(&self) -> String {
            match (self.fondo, self.pantalla, self.ratio) {
                (Some(f), Some(p), Some(r)) => format!(
                    "{:?} pintado {:?} (en pantalla {p:?}) sobre {f:?} da {r:.2}",
                    self.texto, self.pintado
                ),
                _ => format!(
                    "{:?} pintado {:?} sin relleno debajo",
                    self.texto, self.pintado
                ),
            }
        }
    }

    fn medir(b: &BancoDelMenu) -> Vec<Medida> {
        let mut v = Vec::new();
        for (i, f) in b.formas.iter().enumerate() {
            let Forma::Texto {
                texto,
                rect,
                glifo,
                colores,
                ..
            } = f
            else {
                continue;
            };
            // «Unaligned» es el rótulo de depuración que egui pinta en las compilaciones de
            // depuración cuando un rectángulo no cae en la rejilla de píxeles (a 1,25 el menú
            // no cae): no es texto de la interfaz y no lleva relleno.
            if texto.trim().is_empty() || texto == "Unaligned" {
                continue;
            }
            let pintado = glifo
                .or_else(|| colores.first().copied())
                .unwrap_or(Color32::PLACEHOLDER);
            let (fila, fondo, pantalla, ratio) = match fondo_en(b, i) {
                Some((fila, fondo)) => {
                    let pantalla = encima(fondo, pintado);
                    (
                        Some(fila),
                        Some(fondo),
                        Some(pantalla),
                        Some(contrast_ratio(pantalla, fondo)),
                    )
                }
                None => (None, None, None, None),
            };
            v.push(Medida {
                texto: texto.clone(),
                rect: *rect,
                pintado,
                fila,
                fondo,
                pantalla,
                ratio,
            });
        }
        v
    }

    /// El estado de un recorrido: la escena, dónde está el ratón y todo lo pintado.
    #[derive(Clone, Debug)]
    struct Estado {
        escena: Escena,
        /// Rótulo de la fila bajo el ratón; `None` es el ratón fuera de la ventana (reposo).
        raton: Option<String>,
        medidas: Vec<Medida>,
    }

    struct Mesa {
        b: BancoDelMenu,
        platform: Platform,
    }

    impl Mesa {
        fn nueva(ppp: f32, fondo: CanvasBackground, locks: Locks, platform: Platform) -> Mesa {
            let ctx = contexto(ppp);
            ctx.set_visuals_of(egui::Theme::Dark, visuals(ppp));
            let mut mesa = Mesa {
                b: BancoDelMenu {
                    ctx,
                    fondo,
                    locks,
                    formas: Vec::new(),
                    elegida: None,
                    tiempo: 0.0,
                },
                platform,
            };
            mesa.reposo();
            mesa
        }

        /// Un fotograma como el de `BancoDelMenu::fotograma`, con la plataforma de la mesa.
        fn fotograma(&mut self, events: Vec<Event>) {
            let b = &mut self.b;
            let estado = UiState {
                can_undo: true,
                can_redo: true,
                zoom: Zoom::from_int(5),
                tool: Tool::Pencil,
                locks: b.locks,
            };
            let fondo = b.fondo;
            let platform = self.platform;
            b.tiempo += 0.25;
            let entrada = RawInput {
                screen_rect: Some(Rect::from_min_size(Pos2::ZERO, PANTALLA_DEL_MENU)),
                events,
                time: Some(b.tiempo),
                ..RawInput::default()
            };
            let mut salida = b.ctx.run_ui(entrada, |ui| {
                apply_scale(ui.ctx());
                show_menu_with_background(ui, &estado, platform, fondo);
            });
            salida.textures_delta.clear();
            b.formas = formas_en_orden(&salida.shapes);
        }

        fn reposo(&mut self) {
            for _ in 0..6 {
                self.fotograma(vec![]);
            }
        }

        fn clic(&mut self, pos: Pos2) {
            self.fotograma(vec![mover(pos)]);
            self.fotograma(vec![boton(pos, true)]);
            self.fotograma(vec![boton(pos, false)]);
        }

        fn centro(&self, texto: &str) -> Pos2 {
            rect_exacto(&self.b, texto).center()
        }

        /// Barra con `escena` abierta con clics reales (el menú y, si toca, el submenú).
        fn abierta(
            ppp: f32,
            fondo: CanvasBackground,
            locks: Locks,
            platform: Platform,
            escena: Escena,
        ) -> Mesa {
            let mut mesa = Mesa::nueva(ppp, fondo, locks, platform);
            let titulo = mesa.centro(escena.menu);
            mesa.clic(titulo);
            mesa.reposo();
            if escena.submenu {
                let submenu = mesa.centro("Fondo del lienzo");
                mesa.clic(submenu);
                mesa.reposo();
            }
            mesa
        }

        /// Mide la escena con el ratón sobre la fila `raton` (o fuera, si es `None`).
        fn estado(&mut self, escena: Escena, raton: Option<&str>) -> Estado {
            match raton {
                None => self.fotograma(vec![Event::PointerGone]),
                Some(fila) => {
                    let pos = self.centro(fila);
                    self.fotograma(vec![mover(pos)]);
                }
            }
            self.fotograma(vec![]);
            self.fotograma(vec![]);
            Estado {
                escena,
                raton: raton.map(str::to_owned),
                medidas: medir(&self.b),
            }
        }
    }

    /// Reposo (ratón fuera) y luego el ratón sobre cada fila, una a una.
    fn recorrer_escena(
        ppp: f32,
        fondo: CanvasBackground,
        locks: Locks,
        platform: Platform,
        escena: Escena,
    ) -> Vec<Estado> {
        let mut mesa = Mesa::abierta(ppp, fondo, locks, platform, escena);
        let mut estados = vec![mesa.estado(escena, None)];
        for fila in filas_de(escena, fondo) {
            estados.push(mesa.estado(escena, Some(&fila)));
        }
        estados
    }

    fn recorrer(
        ppp: f32,
        fondo: CanvasBackground,
        locks: Locks,
        platform: Platform,
    ) -> Vec<Estado> {
        ESCENAS
            .iter()
            .flat_map(|e| recorrer_escena(ppp, fondo, locks, platform, *e))
            .collect()
    }

    /// Reparte las escalas en hilos (cada una con su contexto de egui).
    fn en_paralelo<T: Sync, R: Send>(datos: &[T], f: impl Fn(&T) -> R + Sync) -> Vec<R> {
        std::thread::scope(|s| {
            let hilos: Vec<_> = datos.iter().map(|d| s.spawn(|| f(d))).collect();
            hilos
                .into_iter()
                .map(|h| h.join().expect("un hilo del recorrido ha fallado"))
                .collect()
        })
    }

    fn estado_de_ui(locks: Locks) -> UiState {
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
    struct Fallos {
        /// Clave del par (texto, relleno, resultado) -> (veces, primer sitio donde ocurre).
        por_par: BTreeMap<String, (usize, String)>,
    }

    impl Fallos {
        fn anota(&mut self, donde: String, m: &Medida) {
            self.por_par
                .entry(m.describe())
                .and_modify(|(n, _)| *n += 1)
                .or_insert((1, donde));
        }

        fn anota_texto(&mut self, donde: String, mensaje: String) {
            self.por_par
                .entry(mensaje)
                .and_modify(|(n, _)| *n += 1)
                .or_insert((1, donde));
        }

        fn mezcla(&mut self, otro: Fallos) {
            for (k, (n, donde)) in otro.por_par {
                self.por_par
                    .entry(k)
                    .and_modify(|(m, _)| *m += n)
                    .or_insert((n, donde));
            }
        }

        fn esta_vacio(&self) -> bool {
            self.por_par.is_empty()
        }

        fn resumen(&self) -> String {
            let total: usize = self.por_par.values().map(|(n, _)| n).sum();
            let mut s = format!(
                "{} pares distintos fallan ({total} apariciones):",
                self.por_par.len()
            );
            for (par, (n, donde)) in self.por_par.iter().take(14) {
                s.push_str(&format!("\n  x{n}  {par}  [p. ej. {donde}]"));
            }
            s
        }
    }

    fn donde(ppp: f32, fondo: CanvasBackground, e: &Estado) -> String {
        format!(
            "ppp {ppp}, fondo {fondo:?}, {}, ratón en {:?}",
            e.escena.nombre, e.raton
        )
    }

    // -----------------------------------------------------------------------------------------
    // Fila 1: recorrido de todo texto, en reposo y con el ratón
    // -----------------------------------------------------------------------------------------

    #[test]
    fn fila_1_todo_texto_de_los_menus_tiene_contraste_4_5_sobre_su_relleno_real() {
        let resultados = en_paralelo(&PPPS, |&ppp| {
            let mut fallos = Fallos::default();
            let mut pares = 0usize;
            let mut distintos = BTreeSet::new();
            for fondo in ACTIVOS {
                for e in recorrer(ppp, fondo, sin_bloqueos(), Platform::Linux) {
                    for m in &e.medidas {
                        pares += 1;
                        distintos.insert((
                            m.texto.clone(),
                            m.fondo.map(|f| f.to_array()),
                            m.pintado.to_array(),
                        ));
                        match m.ratio {
                            None => fallos.anota(donde(ppp, fondo, &e), m),
                            Some(r) if r < MIN_TEXTO => fallos.anota(donde(ppp, fondo, &e), m),
                            Some(_) => {}
                        }
                    }
                }
            }
            (fallos, pares, distintos)
        });
        let mut fallos = Fallos::default();
        let mut pares = 0;
        let mut distintos = BTreeSet::new();
        for (f, n, d) in resultados {
            fallos.mezcla(f);
            pares += n;
            distintos.extend(d);
        }
        eprintln!(
            "fila 1: {pares} medidas texto-fondo, {} pares (texto, relleno) distintos, 6 fondos x 5 escalas",
            distintos.len()
        );
        assert!(
            pares >= PARES_MINIMOS_FILA_1,
            "el recorrido solo midió {pares} pares: el banco no está abriendo los menús"
        );
        assert!(fallos.esta_vacio(), "{}", fallos.resumen());
    }

    // -----------------------------------------------------------------------------------------
    // Fila 2: la entrada marcada
    // -----------------------------------------------------------------------------------------

    #[test]
    fn fila_2_la_marcada_es_surface_0_sobre_accent_las_demas_text_y_el_valor_surface_0() {
        let resultados = en_paralelo(&PPPS, |&ppp| {
            let mut fallos = Fallos::default();
            let mut estados_vistos = 0usize;
            for fondo in ACTIVOS {
                let estados =
                    recorrer_escena(ppp, fondo, sin_bloqueos(), Platform::Linux, ESC_FONDO);
                for e in &estados {
                    estados_vistos += 1;
                    let sitio = donde(ppp, fondo, e);
                    // Una sola fila sobre ACCENT en todo lo pintado.
                    let filas_accent: BTreeSet<[u32; 4]> = e
                        .medidas
                        .iter()
                        .filter(|m| m.fondo == Some(ACCENT))
                        .filter_map(|m| m.fila)
                        .map(|r| [r.min.x, r.min.y, r.max.x, r.max.y].map(f32::to_bits))
                        .collect();
                    if filas_accent.len() != 1 {
                        fallos.anota_texto(
                            sitio.clone(),
                            format!(
                                "hay {} filas sobre ACCENT y debía haber 1",
                                filas_accent.len()
                            ),
                        );
                    }
                    for item in el_submenu_de_fondo().items {
                        let rotulo = marca(fondo, item);
                        let marcada = background_checked(item.action, fondo);
                        let Some(m) = e.medidas.iter().find(|m| m.texto == rotulo) else {
                            fallos.anota_texto(sitio.clone(), format!("no se pinta {rotulo:?}"));
                            continue;
                        };
                        let (color, base) = if marcada {
                            (SURFACE_0, Some(ACCENT))
                        } else {
                            (TEXT, None)
                        };
                        if m.pantalla != Some(color) || m.pintado != color {
                            fallos.anota(sitio.clone(), m);
                        }
                        if let Some(base) = base
                            && m.fondo != Some(base)
                        {
                            fallos.anota(sitio.clone(), m);
                        }
                        match m.ratio {
                            Some(r) if r >= MIN_TEXTO => {}
                            _ => fallos.anota(sitio.clone(), m),
                        }
                        if marcada {
                            // SURFACE_0 sobre ACCENT: 8,98.
                            match m.ratio {
                                Some(r) if (r - 8.98).abs() < 0.01 => {}
                                _ => fallos.anota(sitio.clone(), m),
                            }
                        }
                    }
                    // El valor de `Color…`, solo con un color propio, en SURFACE_0.
                    if let Some(valor) = background_value_text(fondo) {
                        match e.medidas.iter().find(|m| m.texto == valor) {
                            Some(m) => {
                                if m.pantalla != Some(SURFACE_0) || m.fondo != Some(ACCENT) {
                                    fallos.anota(sitio.clone(), m);
                                }
                            }
                            None => fallos.anota_texto(
                                sitio.clone(),
                                format!("no se pinta el valor {valor:?}"),
                            ),
                        }
                    }
                }
            }
            (fallos, estados_vistos)
        });
        let mut fallos = Fallos::default();
        for (f, n) in resultados {
            assert_eq!(n, 6 * 6, "seis fondos x (reposo + cinco filas)");
            fallos.mezcla(f);
        }
        assert!(fallos.esta_vacio(), "{}", fallos.resumen());
    }

    // -----------------------------------------------------------------------------------------
    // Fila 3: los atajos, en TEXT opaco
    // -----------------------------------------------------------------------------------------

    fn escena_del_menu(titulo: &str) -> Escena {
        *ESCENAS
            .iter()
            .find(|e| e.menu == titulo && !e.submenu)
            .expect("escena del menú")
    }

    #[test]
    fn fila_3_los_atajos_se_pintan_en_text_opaco_con_4_5_en_reposo_y_con_el_raton() {
        // `show_menu_with_background` recibe la plataforma, así que las tres se pintan de verdad.
        let resultados = en_paralelo(&PPPS, |&ppp| {
            let mut fallos = Fallos::default();
            let mut por_plataforma = Vec::new();
            for platform in PLATAFORMAS {
                let mut n = 0usize;
                for titulo in ["Archivo", "Editar", "Ver"] {
                    let escena = escena_del_menu(titulo);
                    let mut mesa = Mesa::abierta(
                        ppp,
                        CanvasBackground::Checker,
                        sin_bloqueos(),
                        platform,
                        escena,
                    );
                    for item in el_menu(titulo).items {
                        let Some(atajo) = shortcut_for(item.action, platform) else {
                            continue;
                        };
                        let texto = shortcut_text(atajo, platform);
                        n += 1;
                        for raton in [None, Some(item.label)] {
                            let e = mesa.estado(escena, raton);
                            let sitio = format!("{platform:?}, {}", donde(ppp, e_fondo(), &e));
                            let Some(m) = e.medidas.iter().find(|m| m.texto == texto) else {
                                fallos
                                    .anota_texto(sitio, format!("no se pinta el atajo {texto:?}"));
                                continue;
                            };
                            if m.pintado != TEXT || m.pintado.a() != 255 {
                                fallos.anota(sitio.clone(), m);
                            }
                            match m.ratio {
                                Some(r) if r >= MIN_TEXTO => {}
                                _ => fallos.anota(sitio, m),
                            }
                        }
                    }
                }
                por_plataforma.push((platform, n));
            }
            (fallos, por_plataforma)
        });
        let mut fallos = Fallos::default();
        for (f, por_plataforma) in resultados {
            fallos.mezcla(f);
            // Los atajos de la tabla: 12 en Linux (con `Salir`), 11 en Windows y en macOS (con
            // `Borrar página`, de I.2a).
            for (platform, n) in por_plataforma {
                let esperados = if platform == Platform::Linux { 12 } else { 11 };
                assert_eq!(n, esperados, "atajos de la tabla en {platform:?}");
            }
        }
        assert!(fallos.esta_vacio(), "{}", fallos.resumen());
    }

    fn e_fondo() -> CanvasBackground {
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

    fn bloqueos_de_la_fila_4() -> [(&'static str, Locks); 3] {
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
    const FONDOS_DE_LA_FILA_4: [CanvasBackground; 2] = [
        CanvasBackground::Checker,
        CanvasBackground::Custom(AZUL_ACERO),
    ];

    /// `m` está en una fila cuyo rótulo es de una acción deshabilitada con `locks`.
    fn desactivada(m: &Medida, e: &Estado, state: &UiState) -> bool {
        let Some(fila) = m.fila else {
            return false;
        };
        e.medidas.iter().any(|o| {
            o.fila == Some(fila) && accion_de_rotulo(&o.texto).is_some_and(|a| !enabled(a, state))
        })
    }

    /// `m` está en una fila marcada del submenú de fondo (su rótulo lleva la marca `· `).
    fn de_fila_marcada(m: &Medida, e: &Estado) -> bool {
        let Some(fila) = m.fila else {
            return false;
        };
        e.medidas
            .iter()
            .any(|o| o.fila == Some(fila) && o.texto.starts_with("· "))
    }

    /// Recorre los tres bloqueos y compara cada par desactivado (de filas marcadas o de las no
    /// marcadas, según `marcadas`) con el mismo par habilitado, y con el suelo de D4.
    fn comprueba_desactivados(marcadas: bool) -> (Fallos, usize) {
        let resultados = en_paralelo(&PPPS, |&ppp| {
            let mut fallos = Fallos::default();
            let mut desactivados = 0usize;
            for fondo in FONDOS_DE_LA_FILA_4 {
                let libre = recorrer(ppp, fondo, sin_bloqueos(), Platform::Linux);
                for (nombre, locks) in bloqueos_de_la_fila_4() {
                    let state = estado_de_ui(locks);
                    let con = recorrer(ppp, fondo, locks, Platform::Linux);
                    assert_eq!(libre.len(), con.len());
                    for (a, b) in libre.iter().zip(&con) {
                        assert_eq!(a.medidas.len(), b.medidas.len(), "{nombre}");
                        for (habilitado, m) in a.medidas.iter().zip(&b.medidas) {
                            assert_eq!(
                                habilitado.texto, m.texto,
                                "{nombre}: mismo orden de textos"
                            );
                            if !desactivada(m, b, &state) || de_fila_marcada(m, b) != marcadas {
                                continue;
                            }
                            desactivados += 1;
                            let sitio = format!("{nombre}: {}", donde(ppp, fondo, b));
                            let (Some(r), Some(rh)) = (m.ratio, habilitado.ratio) else {
                                fallos.anota(sitio, m);
                                continue;
                            };
                            if r >= rh {
                                fallos.anota_texto(
                                    sitio.clone(),
                                    format!(
                                        "{:?} desactivado da {r:.2} y habilitado {rh:.2}: no se nota atenuado",
                                        m.texto
                                    ),
                                );
                            }
                            // Suelo de D4 (comparación `>=`, matiz del visto bueno).
                            if r < SUELO_DESACTIVADO {
                                fallos.anota_texto(
                                    sitio,
                                    format!(
                                        "{:?} desactivado da {r:.2}, por debajo del suelo de {SUELO_DESACTIVADO}",
                                        m.texto
                                    ),
                                );
                            }
                        }
                    }
                }
            }
            (fallos, desactivados)
        });
        let mut fallos = Fallos::default();
        let mut total = 0;
        for (f, n) in resultados {
            fallos.mezcla(f);
            total += n;
        }
        (fallos, total)
    }

    #[test]
    fn fila_4_cada_rotulo_y_atajo_desactivado_es_menor_que_el_habilitado_y_al_menos_2_0() {
        let (fallos, n) = comprueba_desactivados(false);
        assert!(
            n > 1000,
            "el recorrido solo encontró {n} pares desactivados"
        );
        assert!(fallos.esta_vacio(), "{}", fallos.resumen());
    }

    #[test]
    fn fila_4_la_fila_marcada_desactivada_es_menor_que_la_habilitada_y_al_menos_2_0() {
        let (fallos, n) = comprueba_desactivados(true);
        assert!(
            n > 100,
            "el recorrido solo encontró {n} pares marcados desactivados"
        );
        assert!(fallos.esta_vacio(), "{}", fallos.resumen());
    }

    #[test]
    fn fila_4_los_pares_bajo_4_5_estan_solo_entre_los_desactivados() {
        let resultados = en_paralelo(&PPPS, |&ppp| {
            let mut fallos = Fallos::default();
            for fondo in FONDOS_DE_LA_FILA_4 {
                for (nombre, locks) in bloqueos_de_la_fila_4() {
                    let state = estado_de_ui(locks);
                    for e in recorrer(ppp, fondo, locks, Platform::Linux) {
                        for m in &e.medidas {
                            let bajo = m.ratio.is_none_or(|r| r < MIN_TEXTO);
                            if bajo && !desactivada(m, &e, &state) {
                                fallos.anota(format!("{nombre}: {}", donde(ppp, fondo, &e)), m);
                            }
                        }
                    }
                }
            }
            fallos
        });
        let mut fallos = Fallos::default();
        for f in resultados {
            fallos.mezcla(f);
        }
        assert!(
            fallos.esta_vacio(),
            "pares habilitados bajo 4,5 con un bloqueo puesto: {}",
            fallos.resumen()
        );
    }

    // -----------------------------------------------------------------------------------------
    // Fila 5: nada más cambia
    // -----------------------------------------------------------------------------------------

    #[test]
    fn fila_5_override_text_color_sigue_siendo_text_y_el_submenu_mide_131_u() {
        for ppp in PPPS {
            assert_eq!(
                visuals(ppp).override_text_color,
                Some(TEXT),
                "ppp {ppp}: el color de texto forzado no cambia"
            );
        }
        assert_eq!(submenu_width_u(el_submenu_de_fondo()), 131);
    }

    #[test]
    fn fila_5_posiciones_y_tamanos_de_cada_texto_siguen_la_geometria_de_la_17() {
        let fallos = en_paralelo(&PPPS, |&ppp| {
            let u = ui_unit(ppp);
            // A 1,25 egui coloca el panel y las filas fuera de la rejilla de píxeles (por eso pinta
            // «Unaligned»): las filas miden hasta medio píxel físico más y los márgenes un cuarto.
            // Es el comportamiento de siempre; a ese ppp se tolera un píxel físico.
            let tol = if ppp == 1.25 {
                1.0 / ppp
            } else {
                tolerancia_de_egui(ppp) / ppp
            };
            let mut fallos = Fallos::default();
            let mut vistos = 0usize;
            for fondo in FONDOS_DE_LA_FILA_4 {
                for escena in ESCENAS {
                    let mut mesa =
                        Mesa::abierta(ppp, fondo, sin_bloqueos(), Platform::Linux, escena);
                    let e = mesa.estado(escena, None);
                    for m in &e.medidas {
                        vistos += 1;
                        let sitio = donde(ppp, fondo, &e);
                        let Some(fila) = m.fila else {
                            fallos.anota_texto(sitio, format!("{:?} sin fila", m.texto));
                            continue;
                        };
                        let es_titulo = MENUS.iter().any(|menu| menu.title == m.texto);
                        let es_rotulo = es_titulo
                            || m.texto == "Fondo del lienzo"
                            || accion_de_rotulo(&m.texto).is_some();
                        let caracteres = m.texto.chars().count() as f32;
                        let mut comprueba = |nombre: &str, real: f32, esperado: f32| {
                            if (real - esperado).abs() > tol {
                                fallos.anota_texto(
                                    sitio.clone(),
                                    format!(
                                        "{:?}: {nombre} mide {real} y debía medir {esperado} (u = {u})",
                                        m.texto
                                    ),
                                );
                            }
                        };
                        // La caja del texto: 7 u por carácter y 14 u de alto.
                        comprueba("ancho del texto", m.rect.width(), caracteres * 7.0 * u);
                        comprueba("alto del texto", m.rect.height(), 14.0 * u);
                        // La fila: 16 u de alto y el texto 1 u por debajo de su borde.
                        // (La fila del submenú `Fondo del lienzo` a 1,25 mide 16 pt y no 12,8: la
                        // flecha y el redondeo de egui; es así hoy y solo a ese ppp.)
                        if !(ppp == 1.25 && m.texto == "Fondo del lienzo") {
                            comprueba("alto de la fila", fila.height(), 16.0 * u);
                            comprueba("margen superior", m.rect.min.y - fila.min.y, u);
                        }
                        // Rótulos con 6 u a la izquierda; atajos y valor con 6 u a la derecha.
                        if es_rotulo {
                            comprueba("margen izquierdo", m.rect.min.x - fila.min.x, 6.0 * u);
                        } else {
                            comprueba("margen derecho", fila.max.x - m.rect.max.x, 6.0 * u);
                        }
                        // El ancho de la fila.
                        let ancho_u = if es_titulo {
                            Some(12.0 + caracteres * 7.0)
                        } else {
                            match accion_de_rotulo(&m.texto) {
                                Some(
                                    AppAction::CanvasBackground(_) | AppAction::BackgroundColor,
                                ) => Some(submenu_width_u(el_submenu_de_fondo()) as f32),
                                Some(_) => {
                                    Some(popup_width_u(el_menu(escena.menu), Platform::Linux) as f32)
                                }
                                None if m.texto == "Fondo del lienzo" => {
                                    Some(popup_width_u(el_menu("Ver"), Platform::Linux) as f32)
                                }
                                None => None,
                            }
                        };
                        if let Some(ancho_u) = ancho_u {
                            comprueba("ancho de la fila", fila.width(), ancho_u * u);
                        }
                    }
                }
            }
            (fallos, vistos)
        });
        let mut todos = Fallos::default();
        for (f, vistos) in fallos {
            assert!(vistos > 100, "solo se midieron {vistos} textos");
            todos.mezcla(f);
        }
        assert!(todos.esta_vacio(), "{}", todos.resumen());
    }

    // -----------------------------------------------------------------------------------------
    // Propiedades (32 casos; `PROPTEST_CASES` baja o sube el número al desarrollar)
    // -----------------------------------------------------------------------------------------

    fn casos() -> u32 {
        std::env::var("PROPTEST_CASES")
            .ok()
            .and_then(|v| v.parse().ok())
            .unwrap_or(32)
    }

    fn fondo_cualquiera() -> impl Strategy<Value = CanvasBackground> {
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
    /// `fila` (módulo el número de filas de cada uno).
    fn dos_menus(ppp: f32, fondo: CanvasBackground, fila: usize) -> Vec<Estado> {
        [ESC_FONDO, ESC_ARCHIVO]
            .into_iter()
            .map(|escena| {
                let filas = filas_de(escena, fondo);
                let raton = filas[fila % filas.len()].clone();
                let mut mesa = Mesa::abierta(ppp, fondo, sin_bloqueos(), Platform::Linux, escena);
                mesa.estado(escena, Some(&raton))
            })
            .collect()
    }

    proptest! {
        #![proptest_config(ProptestConfig {
            cases: casos(),
            failure_persistence: None,
            ..ProptestConfig::default()
        })]

        /// Contraste >= 4,5 de todo par texto-fondo habilitado con cualquier fondo activo (con
        /// `Custom` de RGB opaco aleatorio), cualquier escala y cualquier fila bajo el ratón.
        #[test]
        fn propiedad_1_el_contraste_habilitado_es_al_menos_4_5_con_cualquier_fondo(
            fondo in fondo_cualquiera(),
            escala in 0..PPPS.len(),
            fila in 0usize..64,
        ) {
            let ppp = PPPS[escala];
            let mut fallos = Fallos::default();
            for e in dos_menus(ppp, fondo, fila) {
                for m in &e.medidas {
                    if m.ratio.is_none_or(|r| r < MIN_TEXTO) {
                        fallos.anota(donde(ppp, fondo, &e), m);
                    }
                }
            }
            prop_assert!(fallos.esta_vacio(), "{}", fallos.resumen());
        }

        /// El color pintado depende solo del estado: `SURFACE_0` el rótulo marcado y el valor,
        /// `TEXT` todo lo demás (rótulos, atajos, títulos), sin mirar fondo, escala ni fila.
        #[test]
        fn propiedad_2_el_color_del_texto_depende_solo_del_estado(
            fondo in fondo_cualquiera(),
            escala in 0..PPPS.len(),
            fila in 0usize..64,
        ) {
            let ppp = PPPS[escala];
            let marcada = el_submenu_de_fondo()
                .items
                .iter()
                .find(|i| background_checked(i.action, fondo))
                .map(|i| marca(fondo, i))
                .expect("siempre hay una marcada");
            let valor = background_value_text(fondo);
            let mut fallos = Fallos::default();
            for e in dos_menus(ppp, fondo, fila) {
                for m in &e.medidas {
                    let esperado = if m.texto == marcada || valor.as_deref() == Some(m.texto.as_str()) {
                        SURFACE_0
                    } else {
                        TEXT
                    };
                    if m.pintado != esperado || m.pintado.a() != 255 {
                        fallos.anota(donde(ppp, fondo, &e), m);
                    }
                }
            }
            prop_assert!(fallos.esta_vacio(), "debían ser {SURFACE_0:?} o {TEXT:?}: {}", fallos.resumen());
        }
    }
}
