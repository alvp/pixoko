//! Menu bar (specs/hito-2-menu-herramientas.md, "Pieces" and "Geometry"): the table of entries
//! with their action and their shortcut per platform is pure logic; `show_menu` only paints it.

use egui::containers::menu::{MenuBar, MenuConfig, menu_style};
use egui::{Button, Frame, Panel, Sense, Style, Ui, vec2};

use crate::actions::{AppAction, LayerNav, Platform, UiState, enabled, layer_enabled};
use crate::background::{CanvasBackground, background_value_text};
use crate::chrome::MENU_H_U;
use crate::edit_layout::{TEXT_BUTTON_H_U, TEXT_BUTTON_TEXT_Y_U};
use crate::help::HelpTarget;
use crate::keymap::{Chord, Keymap, default_keymap};
use crate::status::ADVANCE_PX;
use crate::text::{printable, ui_unit};
use crate::theme::{SURFACE_0, SURFACE_1, SURFACE_3};

/// An entry of a dropdown.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct MenuItem {
    pub label: &'static str,
    pub action: AppAction,
    /// A separator line before the entry.
    pub separator_before: bool,
}

/// A submenu inside a dropdown: its label and its entries.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Submenu {
    pub label: &'static str,
    pub items: &'static [MenuItem],
}

/// A menu of the bar: its title, its entries and its submenus (painted after the entries).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Menu {
    pub title: &'static str,
    pub items: &'static [MenuItem],
    pub submenus: &'static [Submenu],
}

const fn item(label: &'static str, action: AppAction) -> MenuItem {
    MenuItem {
        label,
        action,
        separator_before: false,
    }
}

/// The six menus. There are only entries for functions that exist.
pub const MENUS: [Menu; 6] = [
    Menu {
        title: "Archivo",
        items: &[
            item("Nueva imagen…", AppAction::NewImage),
            item("Abrir…", AppAction::Open),
            item("Guardar", AppAction::Save),
            item("Guardar como…", AppAction::SaveAs),
            item("Exportar PNG aplanado…", AppAction::ExportFlat),
            MenuItem {
                label: "Salir",
                action: AppAction::Quit,
                separator_before: true,
            },
        ],
        submenus: &[],
    },
    Menu {
        title: "Editar",
        items: &[
            item("Deshacer", AppAction::Undo),
            item("Rehacer", AppAction::Redo),
            item("Borrar página", AppAction::ClearPage),
        ],
        submenus: &[],
    },
    Menu {
        title: "Imagen",
        items: &[item("Convertir modo…", AppAction::ConvertMode)],
        submenus: &[],
    },
    Menu {
        title: "Capa",
        items: &[
            item("Nueva capa", AppAction::NewLayer),
            item("Duplicar", AppAction::DuplicateLayer),
            item("Quitar", AppAction::RemoveLayer),
            item("Subir", AppAction::LayerUp),
            item("Bajar", AppAction::LayerDown),
            item("Combinar abajo", AppAction::MergeLayerDown),
            item("Combinar visibles", AppAction::MergeVisibleLayers),
            item("Aplanar", AppAction::FlattenLayers),
            item("Renombrar", AppAction::RenameLayer),
            item("Propiedades", AppAction::LayerProperties),
            item("Mostrar panel", AppAction::ToggleLayerPanel),
        ],
        submenus: &[],
    },
    Menu {
        title: "Ver",
        items: &[
            item("Acercar", AppAction::ZoomIn),
            item("Alejar", AppAction::ZoomOut),
            item("Ajustar a la ventana", AppAction::ZoomFit),
            item("1:1", AppAction::ZoomActualSize),
            item("Editor de color", AppAction::ToggleColorEditor),
        ],
        submenus: &[Submenu {
            label: "Fondo del lienzo",
            items: &[
                item(
                    "Damero",
                    AppAction::CanvasBackground(CanvasBackground::Checker),
                ),
                item(
                    "Negro",
                    AppAction::CanvasBackground(CanvasBackground::Black),
                ),
                item("Gris", AppAction::CanvasBackground(CanvasBackground::Gray)),
                item(
                    "Blanco",
                    AppAction::CanvasBackground(CanvasBackground::White),
                ),
                item("Color…", AppAction::BackgroundColor),
            ],
        }],
    },
    Menu {
        title: "Ayuda",
        items: &[
            item(
                "Ayuda de la herramienta",
                AppAction::ShowHelp(HelpTarget::Active),
            ),
            item("Atajos de teclado…", AppAction::ShowShortcuts),
            item("Licencias", AppAction::ShowLicenses),
        ],
        submenus: &[],
    },
];

/// `true` if `action` is the one of the background `current`: the entry of the submenu that carries the mark.
/// With a custom color it is `Color…` that carries it, not the plain-background entry of the same color.
pub fn background_checked(action: AppAction, current: CanvasBackground) -> bool {
    match current {
        CanvasBackground::Custom(_) => action == AppAction::BackgroundColor,
        _ => action == AppAction::CanvasBackground(current),
    }
}

/// A shortcut to show it: the key, whether it carries `command` (`Ctrl` or `Cmd`) and whether it carries
/// `Shift`. It is the [`Chord`] of the key table.
pub type Shortcut = Chord;

/// The shortcut `keymap` shows for `action` on `platform`, if it has one: the first row of the table
/// marked as shown on that platform (`Redo` shows `Ctrl+Y` and, on macOS, `Cmd+Shift+Z`; `Quit` only
/// has a shortcut on Linux).
pub fn shortcut_in(keymap: &Keymap, action: AppAction, platform: Platform) -> Option<Shortcut> {
    keymap.shown(action, platform)
}

/// Like [`shortcut_in`] with the default table.
pub fn shortcut_for(action: AppAction, platform: Platform) -> Option<Shortcut> {
    shortcut_in(default_keymap(), action, platform)
}

/// The fixed shortcut of an entry that does not come from the key table: `F1` in `Tool help`
/// (`F1` belongs to the help and is not reassigned).
pub fn fixed_shortcut(action: AppAction) -> Option<&'static str> {
    matches!(action, AppAction::ShowHelp(_)).then_some("F1")
}

/// Text of a shortcut: `Ctrl+O`, `Cmd+Shift+S`, `Home`, `>`, `d`.
pub fn shortcut_text(shortcut: Shortcut, platform: Platform) -> String {
    shortcut.text(platform)
}

/// Margin of the text of a menu button, in `u`, on each side.
const PAD_X_U: i64 = 6;
/// Margin on the left of the bar, in `u`.
const BAR_MARGIN_U: i64 = 4;

/// Width of a dropdown in `u`: margins and the largest `label + 2 + shortcut`, at 7 u per character
/// (with the default key table).
pub fn popup_width_u(menu: &Menu, platform: Platform) -> i64 {
    popup_width_in(menu, platform, default_keymap())
}

/// Like [`popup_width_u`] with the shortcuts of `keymap`.
pub fn popup_width_in(menu: &Menu, platform: Platform, keymap: &Keymap) -> i64 {
    let widest = menu
        .items
        .iter()
        .map(|item| {
            let shortcut = shortcut_in(keymap, item.action, platform)
                .map(|s| shortcut_text(s, platform))
                .or_else(|| fixed_shortcut(item.action).map(str::to_string))
                .map_or(0, |text| text.chars().count());
            item.label.chars().count() + 2 + shortcut
        })
        .chain(
            menu.submenus
                .iter()
                .map(|sub| sub.label.chars().count() + 2),
        )
        .max()
        .unwrap_or(0) as i64;
    2 * PAD_X_U + widest * i64::from(ADVANCE_PX)
}

/// Characters of the value of a custom color (`#rrggbb`).
const VALUE_CHARS: i64 = 7;

/// Width of a submenu in `u`: margins, the mark (`· `), the longest label, a gap and the value column,
/// which is **always** reserved so that the submenu does not change width when choosing.
pub fn submenu_width_u(submenu: &Submenu) -> i64 {
    2 * PAD_X_U + (submenu_label_chars(submenu) + 2 + 2 + VALUE_CHARS) * i64::from(ADVANCE_PX)
}

/// Like [`show_menu_with_background`] with the default background marked.
pub fn show_menu(ui: &mut Ui, state: &UiState, platform: Platform) -> Option<AppAction> {
    show_menu_with_background(ui, state, platform, CanvasBackground::default())
}

/// Paints the bar at the top edge of `ui` and returns the chosen entry, if any, with the mark of the
/// background submenu in `background`. It has to be called **before** what takes up the rest of the
/// window.
pub fn show_menu_with_background(
    ui: &mut Ui,
    state: &UiState,
    platform: Platform,
    background: CanvasBackground,
) -> Option<AppAction> {
    show_menu_in(ui, state, platform, background, default_keymap())
}

/// Like [`show_menu_with_background`] with the shortcuts of `keymap` (the app passes its table).
pub fn show_menu_in(
    ui: &mut Ui,
    state: &UiState,
    platform: Platform,
    background: CanvasBackground,
    keymap: &Keymap,
) -> Option<AppAction> {
    show_menu_full(
        ui,
        state,
        platform,
        background,
        keymap,
        &LayerNav::default(),
    )
}

/// Like [`show_menu_in`] with the layer stack, which dims `Remove`, `Up`, `Down`, `New layer` and
/// `Duplicate` when they cannot be done (the app passes its own).
pub fn show_menu_full(
    ui: &mut Ui,
    state: &UiState,
    platform: Platform,
    background: CanvasBackground,
    keymap: &Keymap,
    layers: &LayerNav,
) -> Option<AppAction> {
    let ctx = ui.ctx().clone();
    let ppp = ctx.pixels_per_point();
    let u = ui_unit(ppp);
    let mut chosen = None;
    // 16 u buttons with 6 u of margin on each side of the text and 1 u above, without separation.
    let style = move |style: &mut Style| {
        menu_style(style);
        style.spacing.button_padding = vec2(PAD_X_U as f32 * u, TEXT_BUTTON_TEXT_Y_U as f32 * u);
        style.spacing.item_spacing = vec2(0.0, 0.0);
        style.spacing.interact_size.y = TEXT_BUTTON_H_U as f32 * u;
        style.visuals.widgets.noninteractive.bg_stroke = egui::Stroke::NONE;
        // Shortcuts are painted "weak" (60 % alpha): with the mouse over, on `SURFACE_3`,
        // they give 3.84:1. In the menu they go in `TEXT`, not dimmed.
        style.visuals.weak_text_alpha = 1.0;
    };
    Panel::top("menu")
        .exact_size(MENU_H_U as f32 * u)
        .resizable(false)
        .show_separator_line(false)
        .frame(Frame::new().fill(SURFACE_1))
        .show(ui, |ui| {
            // The row of the bar measures `interact_size.y`: without this the panel grows to 18 pt.
            style(ui.style_mut());
            MenuBar::new()
                .style(style)
                .config(MenuConfig::new().style(style))
                .ui(ui, |ui| {
                    ui.add_space(BAR_MARGIN_U as f32 * u);
                    for menu in &MENUS {
                        let width = popup_width_in(menu, platform, keymap) as f32 * u;
                        ui.menu_button(printable(&ctx, menu.title), |ui| {
                            ui.set_min_width(width);
                            ui.set_max_width(width);
                            for item in menu.items {
                                if item.separator_before {
                                    let (rect, _) =
                                        ui.allocate_exact_size(vec2(width, u), Sense::hover());
                                    ui.painter().rect_filled(rect, 0.0, SURFACE_3);
                                }
                                if menu_entry(
                                    ui,
                                    &ctx,
                                    (state, layers),
                                    (keymap, platform),
                                    item,
                                    width,
                                    u,
                                ) {
                                    chosen = Some(item.action);
                                }
                            }
                            for submenu in menu.submenus {
                                ui.menu_button(printable(&ctx, submenu.label), |ui| {
                                    let width = submenu_width_u(submenu) as f32 * u;
                                    ui.set_min_width(width);
                                    ui.set_max_width(width);
                                    for item in submenu.items {
                                        if marked_entry(ui, &ctx, state, item, background, width, u)
                                        {
                                            chosen = Some(item.action);
                                        }
                                    }
                                });
                            }
                        });
                    }
                });
        });
    chosen
}

/// A row of the dropdown: 16 u, the label on the left and the shortcut on the right. Returns whether
/// it was clicked.
fn menu_entry(
    ui: &mut Ui,
    ctx: &egui::Context,
    (state, layers): (&UiState, &LayerNav),
    (keymap, platform): (&Keymap, Platform),
    item: &MenuItem,
    width: f32,
    u: f32,
) -> bool {
    let on = enabled(item.action, state) && layer_enabled(item.action, layers);
    let label = printable(ctx, item.label);
    let shortcut = shortcut_in(keymap, item.action, platform)
        .map(|s| shortcut_text(s, platform))
        .or_else(|| fixed_shortcut(item.action).map(str::to_string));
    let mut button = Button::new(label).min_size(vec2(width, TEXT_BUTTON_H_U as f32 * u));
    if let Some(text) = shortcut {
        button = button.shortcut_text(printable(ctx, &text));
    }
    ui.add_enabled(on, button).clicked()
}

/// Characters of the longest label among the entries of a submenu.
fn submenu_label_chars(submenu: &Submenu) -> i64 {
    submenu
        .items
        .iter()
        .map(|item| item.label.chars().count())
        .max()
        .unwrap_or(0) as i64
}

/// A row of a submenu with mark: `· ` in front of the label of the active one (two spaces if not; in
/// the monospaced font the text does not move) and the highlighted row. Returns whether it was clicked.
fn marked_entry(
    ui: &mut Ui,
    ctx: &egui::Context,
    state: &UiState,
    item: &MenuItem,
    background: CanvasBackground,
    width: f32,
    u: f32,
) -> bool {
    let on = enabled(item.action, state);
    let checked = background_checked(item.action, background);
    let value = if item.action == AppAction::BackgroundColor {
        background_value_text(background)
    } else {
        None
    };
    let text = format!("{} {}", if checked { '·' } else { ' ' }, item.label);
    // `override_text_color` (TEXT) wins over the stroke of the selection: without its own color the
    // label of the marked row (`ACCENT` fill) comes out at 1.47:1. `SURFACE_0` gives 8.98:1.
    let label = egui::RichText::new(printable(ctx, &text));
    let label = if checked {
        label.color(SURFACE_0)
    } else {
        label
    };
    let mut button = Button::new(label)
        .selected(checked)
        .min_size(vec2(width, TEXT_BUTTON_H_U as f32 * u));
    // The value of the custom color, on the right like a shortcut, also in `SURFACE_0`.
    if let Some(value) = value {
        button = button.shortcut_text(egui::RichText::new(printable(ctx, &value)).color(SURFACE_0));
    }
    ui.add_enabled(on, button).clicked()
}
