//! Barra de menú (specs/hito-2-menu-herramientas.md, «Piezas» y «Geometría»): la tabla de
//! entradas con su acción y su atajo por plataforma es lógica pura; `show_menu` solo la pinta.

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

/// Una entrada de un desplegable.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct MenuItem {
    pub label: &'static str,
    pub action: AppAction,
    /// Una línea separadora antes de la entrada.
    pub separator_before: bool,
}

/// Un submenú dentro de un desplegable: su rótulo y sus entradas.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Submenu {
    pub label: &'static str,
    pub items: &'static [MenuItem],
}

/// Un menú de la barra: su título, sus entradas y sus submenús (que se pintan después de las
/// entradas).
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

/// Los seis menús. Solo hay entradas de funciones que existen.
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

/// `true` si `action` es la del fondo `current`: la entrada del submenú que lleva la marca. Con un
/// color propio la lleva `Color…`, no la entrada del fondo liso que tenga el mismo color.
pub fn background_checked(action: AppAction, current: CanvasBackground) -> bool {
    match current {
        CanvasBackground::Custom(_) => action == AppAction::BackgroundColor,
        _ => action == AppAction::CanvasBackground(current),
    }
}

/// Un atajo para mostrarlo: la tecla, si lleva `command` (`Ctrl` o `Cmd`) y si lleva `Mayús`. Es
/// el [`Chord`] de la tabla de teclas.
pub type Shortcut = Chord;

/// El atajo que muestra `keymap` para `action` en `platform`, si lo tiene: la primera fila de la
/// tabla marcada como mostrada en esa plataforma (`Redo` enseña `Ctrl+Y` y, en macOS,
/// `Cmd+Mayús+Z`; `Salir` solo tiene atajo en Linux).
pub fn shortcut_in(keymap: &Keymap, action: AppAction, platform: Platform) -> Option<Shortcut> {
    keymap.shown(action, platform)
}

/// Como [`shortcut_in`] con la tabla por defecto.
pub fn shortcut_for(action: AppAction, platform: Platform) -> Option<Shortcut> {
    shortcut_in(default_keymap(), action, platform)
}

/// El atajo fijo de una entrada que no sale de la tabla de teclas: `F1` en `Ayuda de la
/// herramienta` (`F1` es de la help y no se reasigna).
pub fn fixed_shortcut(action: AppAction) -> Option<&'static str> {
    matches!(action, AppAction::ShowHelp(_)).then_some("F1")
}

/// Texto de un atajo: `Ctrl+O`, `Cmd+Mayús+S`, `Start`, `>`, `d`.
pub fn shortcut_text(shortcut: Shortcut, platform: Platform) -> String {
    shortcut.text(platform)
}

/// Margen del texto de un botón del menú, en `u`, a cada lado.
const PAD_X_U: i64 = 6;
/// Margen a la izquierda de la barra, en `u`.
const BAR_MARGIN_U: i64 = 4;

/// Ancho de un desplegable en `u`: márgenes y el mayor `rótulo + 2 + shortcut_2`, a 7 u por carácter
/// (con la tabla de teclas por defecto).
pub fn popup_width_u(menu: &Menu, platform: Platform) -> i64 {
    popup_width_in(menu, platform, default_keymap())
}

/// Como [`popup_width_u`] con los atajos de `keymap`.
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

/// Caracteres del valor de un color propio (`#rrggbb`).
const VALUE_CHARS: i64 = 7;

/// Ancho de un submenú en `u`: márgenes, la marca (`· `), el rótulo más largo, un hueco y la
/// columna del valor, que se reserva **siempre** para que el submenú no cambie de ancho al elegir.
pub fn submenu_width_u(submenu: &Submenu) -> i64 {
    2 * PAD_X_U + (submenu_label_chars(submenu) + 2 + 2 + VALUE_CHARS) * i64::from(ADVANCE_PX)
}

/// Como [`show_menu_with_background`] con el fondo por defecto marcado.
pub fn show_menu(ui: &mut Ui, state: &UiState, platform: Platform) -> Option<AppAction> {
    show_menu_with_background(ui, state, platform, CanvasBackground::default())
}

/// Pinta la barra en el borde superior de `ui` y devuelve la entrada elegida, si la hay, con la
/// marca del submenú de fondo en `background`. Hay que llamarla **antes** de lo que ocupa el
/// resto de la ventana.
pub fn show_menu_with_background(
    ui: &mut Ui,
    state: &UiState,
    platform: Platform,
    background: CanvasBackground,
) -> Option<AppAction> {
    show_menu_in(ui, state, platform, background, default_keymap())
}

/// Como [`show_menu_with_background`] con los atajos de `keymap` (la app pasa su tabla).
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

/// Como [`show_menu_in`] con la pila de capas, que atenúa `Remove`, `Subir`, `Bajar`, `Nueva layer_2` y
/// `Duplicar` cuando no se pueden hacer (la app pasa la suya).
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
    // Botones de 16 u con 6 u de margen a cada lado del texto y 1 u arriba, sin separación.
    let style = move |style: &mut Style| {
        menu_style(style);
        style.spacing.button_padding = vec2(PAD_X_U as f32 * u, TEXT_BUTTON_TEXT_Y_U as f32 * u);
        style.spacing.item_spacing = vec2(0.0, 0.0);
        style.spacing.interact_size.y = TEXT_BUTTON_H_U as f32 * u;
        style.visuals.widgets.noninteractive.bg_stroke = egui::Stroke::NONE;
        // Los atajos se pintan «débiles» (60 % de alfa): con el ratón encima, sobre `SURFACE_3`,
        // dan 3,84:1. En el menú van en `TEXT`, sin atenuar.
        style.visuals.weak_text_alpha = 1.0;
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

/// Una fila del desplegable: de 16 u, el rótulo a la izquierda y el atajo a la derecha. Devuelve si
/// se ha pulsado estando habilitada.
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

/// Caracteres del rótulo más largo de las entradas de un submenú.
fn submenu_label_chars(submenu: &Submenu) -> i64 {
    submenu
        .items
        .iter()
        .map(|item| item.label.chars().count())
        .max()
        .unwrap_or(0) as i64
}

/// Una fila de un submenú con marca: `· ` delante del rótulo de la activa (dos espacios si no, en
/// la fuente monoespaciada el texto no se mueve) y la fila resaltada. Devuelve si se ha pulsado
/// estando habilitada.
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
    // `override_text_color` (TEXT) gana al trazo de la selección: sin color propio el rótulo de la
    // fila marcada (relleno `ACCENT`) sale a 1,47:1. `SURFACE_0` da 8,98:1.
    let label = egui::RichText::new(printable(ctx, &text));
    let label = if checked {
        label.color(SURFACE_0)
    } else {
        label
    };
    let mut button = Button::new(label)
        .selected(checked)
        .min_size(vec2(width, TEXT_BUTTON_H_U as f32 * u));
    // El valor del color propio, a la derecha como un atajo, también en `SURFACE_0`.
    if let Some(value) = value {
        button = button.shortcut_text(egui::RichText::new(printable(ctx, &value)).color(SURFACE_0));
    }
    ui.add_enabled(on, button).clicked()
}
