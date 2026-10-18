//! Barra de menú (specs/hito-2-menu-herramientas.md, «Piezas» y «Geometría»): la tabla de
//! entradas con su acción y su atajo por plataforma es lógica pura; `show_menu` solo la pinta.

use egui::containers::menu::{MenuBar, MenuConfig, menu_style};
use egui::{Button, Frame, Key, Panel, Sense, Style, Ui, vec2};

use crate::actions::{AppAction, Platform, UiState, enabled};
use crate::chrome::MENU_H_U;
use crate::edit_layout::{BUTTON_H_U, BUTTON_TEXT_Y_U};
use crate::status::ADVANCE_PX;
use crate::text::{printable, ui_unit};
use crate::theme::{SURFACE_1, SURFACE_3};

/// Una entrada de un desplegable.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct MenuItem {
    pub label: &'static str,
    pub action: AppAction,
    /// Una línea separadora antes de la entrada.
    pub separator_before: bool,
}

/// Un menú de la barra: su título y sus entradas.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Menu {
    pub title: &'static str,
    pub items: &'static [MenuItem],
}

const fn item(label: &'static str, action: AppAction) -> MenuItem {
    MenuItem {
        label,
        action,
        separator_before: false,
    }
}

/// Los cuatro menús. Solo hay entradas de funciones que existen: «Nueva imagen» (unidad 19) y
/// «Convertir modo» (unidad 20) se añaden en sus unidades.
pub const MENUS: [Menu; 4] = [
    Menu {
        title: "Archivo",
        items: &[
            item("Abrir…", AppAction::Open),
            item("Guardar", AppAction::Save),
            item("Guardar como…", AppAction::SaveAs),
            MenuItem {
                label: "Salir",
                action: AppAction::Quit,
                separator_before: true,
            },
        ],
    },
    Menu {
        title: "Editar",
        items: &[
            item("Deshacer", AppAction::Undo),
            item("Rehacer", AppAction::Redo),
        ],
    },
    Menu {
        title: "Ver",
        items: &[
            item("Acercar", AppAction::ZoomIn),
            item("Alejar", AppAction::ZoomOut),
            item("Ajustar a la ventana", AppAction::ZoomFit),
            item("1:1", AppAction::ZoomActual),
        ],
    },
    Menu {
        title: "Ayuda",
        items: &[item("Licencias", AppAction::ShowLicenses)],
    },
];

/// Un atajo para mostrarlo: la tecla, si lleva `command` (`Ctrl` o `Cmd`) y si lleva `Mayús`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Shortcut {
    pub key: Key,
    pub command: bool,
    pub shift: bool,
}

/// El atajo que se muestra para `action` en `platform`, si lo tiene. `Rehacer` muestra
/// `Ctrl+Y` y, en macOS, `Cmd+Mayús+Z` (las dos teclas funcionan en todos los sistemas); `Salir`
/// solo tiene atajo en Linux.
pub fn shortcut_for(action: AppAction, platform: Platform) -> Option<Shortcut> {
    let with_command = |key, shift| Shortcut {
        key,
        command: true,
        shift,
    };
    let plain = |key| Shortcut {
        key,
        command: false,
        shift: false,
    };
    match action {
        AppAction::Open => Some(with_command(Key::O, false)),
        AppAction::Save => Some(with_command(Key::S, false)),
        AppAction::SaveAs => Some(with_command(Key::S, true)),
        AppAction::Quit => (platform == Platform::Linux).then(|| with_command(Key::Q, false)),
        AppAction::Undo => Some(with_command(Key::Z, false)),
        AppAction::Redo if platform == Platform::Mac => Some(with_command(Key::Z, true)),
        AppAction::Redo => Some(with_command(Key::Y, false)),
        AppAction::ZoomIn => Some(plain(Key::Plus)),
        AppAction::ZoomOut => Some(plain(Key::Minus)),
        AppAction::ZoomFit => Some(plain(Key::Home)),
        AppAction::ZoomActual => Some(with_command(Key::Num1, false)),
        _ => None,
    }
}

/// Texto de un atajo: `Ctrl+O`, `Cmd+Mayús+S`, `Inicio`, `+`, `-`.
pub fn shortcut_text(shortcut: Shortcut, platform: Platform) -> String {
    let key = match shortcut.key {
        Key::Home => "Inicio".to_string(),
        Key::Plus => "+".to_string(),
        Key::Minus => "-".to_string(),
        Key::Num1 => "1".to_string(),
        other => other.name().to_string(),
    };
    let mut text = String::new();
    if shortcut.command {
        text.push_str(if platform == Platform::Mac {
            "Cmd+"
        } else {
            "Ctrl+"
        });
    }
    if shortcut.shift {
        text.push_str("Mayús+");
    }
    text.push_str(&key);
    text
}

/// Margen del texto de un botón del menú, en `u`, a cada lado.
const PAD_X_U: i64 = 6;
/// Margen a la izquierda de la barra, en `u`.
const BAR_MARGIN_U: i64 = 4;

/// Ancho de un desplegable en `u`: márgenes y el mayor `rótulo + 2 + atajo`, a 7 u por carácter.
pub fn popup_width_u(menu: &Menu, platform: Platform) -> i64 {
    let widest = menu
        .items
        .iter()
        .map(|item| {
            let shortcut = shortcut_for(item.action, platform)
                .map_or(0, |s| shortcut_text(s, platform).chars().count());
            item.label.chars().count() + 2 + shortcut
        })
        .max()
        .unwrap_or(0) as i64;
    2 * PAD_X_U + widest * i64::from(ADVANCE_PX)
}

/// Pinta la barra en el borde superior de `ui` y devuelve la entrada elegida, si la hay. Hay que
/// llamarla **antes** de lo que ocupa el resto de la ventana.
pub fn show_menu(ui: &mut Ui, state: &UiState, platform: Platform) -> Option<AppAction> {
    let ctx = ui.ctx().clone();
    let ppp = ctx.pixels_per_point();
    let u = ui_unit(ppp);
    let mut chosen = None;
    // Botones de 16 u con 6 u de margen a cada lado del texto y 1 u arriba, sin separación.
    let style = move |style: &mut Style| {
        menu_style(style);
        style.spacing.button_padding = vec2(PAD_X_U as f32 * u, BUTTON_TEXT_Y_U as f32 * u);
        style.spacing.item_spacing = vec2(0.0, 0.0);
        style.spacing.interact_size.y = BUTTON_H_U as f32 * u;
        style.visuals.widgets.noninteractive.bg_stroke = egui::Stroke::NONE;
    };
    Panel::top("menu")
        .exact_size(MENU_H_U as f32 * u)
        .show_separator_line(false)
        .frame(Frame::new().fill(SURFACE_1))
        .show(ui, |ui| {
            // La fila de la barra mide `interact_size.y`: sin esto el panel crece a 18 pt.
            style(ui.style_mut());
            MenuBar::new()
                .style(style)
                .config(MenuConfig::new().style(style))
                .ui(ui, |ui| {
                    ui.add_space(BAR_MARGIN_U as f32 * u);
                    for menu in &MENUS {
                        let width = popup_width_u(menu, platform) as f32 * u;
                        ui.menu_button(printable(&ctx, menu.title), |ui| {
                            ui.set_min_width(width);
                            ui.set_max_width(width);
                            for item in menu.items {
                                if item.separator_before {
                                    let (rect, _) =
                                        ui.allocate_exact_size(vec2(width, u), Sense::hover());
                                    ui.painter().rect_filled(rect, 0.0, SURFACE_3);
                                }
                                if menu_entry(ui, &ctx, state, platform, item, width, u) {
                                    chosen = Some(item.action);
                                }
                            }
                        });
                    }
                });
        });
    chosen
}

/// Una fila del desplegable: de 16 u, el rótulo a la izquierda y el atajo a la derecha. Devuelve si
/// se ha pulsado estando habilitada.
fn menu_entry(
    ui: &mut Ui,
    ctx: &egui::Context,
    state: &UiState,
    platform: Platform,
    item: &MenuItem,
    width: f32,
    u: f32,
) -> bool {
    let on = enabled(item.action, state);
    let label = printable(ctx, item.label);
    let shortcut = shortcut_for(item.action, platform).map(|s| shortcut_text(s, platform));
    let mut button = Button::new(label).min_size(vec2(width, BUTTON_H_U as f32 * u));
    if let Some(text) = shortcut {
        button = button.shortcut_text(printable(ctx, &text));
    }
    ui.add_enabled(on, button).clicked()
}
