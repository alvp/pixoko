//! Caja de herramientas: botones de herramienta y de zoom y, debajo, el recuadro fijo de opciones
//! de la herramienta activa (specs/hito-2-menu-herramientas.md, «Geometría» y «Piezas»,
//! specs/interfaz-caja-derecha.md y specs/interfaz-iconos.md): botones de icono de 24 × 24 u con
//! relieve, en una rejilla de cuatro columnas, en una caja de 107 u en el borde derecho, a la
//! derecha del panel de paletas. Los rótulos, las acciones y el clic derecho son tablas puras;
//! `show_toolbox` solo las pinta.

use egui::{
    Align2, Event, FontFamily, FontId, Frame, Label, Panel, RichText, Role, Sense, Ui, WidgetInfo,
    pos2, vec2,
};
use egui::{MouseWheelUnit, Rect};

use crate::actions::{AppAction, Platform, UiState, enabled};
use crate::chrome::{
    BUTTON_PAD_U, OPTIONS_BODY_H_U, OPTIONS_BODY_W_U, OPTIONS_BODY_X_U, OPTIONS_BODY_Y_U,
    OPTIONS_BORDER_U, OPTIONS_FOCUS_BORDER_U, OPTIONS_H_U, OPTIONS_KEY_Y_U, OPTIONS_ROW_H_U,
    OPTIONS_TITLE_X_U, OPTIONS_TITLE_Y_U, OPTIONS_W_U, OPTIONS_X_U, OPTIONS_Y_U, TOOLBAR_MARGIN_U,
    TOOLBAR_SEPARATOR_Y_U, TOOLBOX_BUTTONS, ZOOM_LABEL_Y_U, toolbar_button_width_u,
    toolbar_width_u, toolbox_button_rect_u,
};
use crate::icon_button::{look, paint_icon};
use crate::icons::{Icon, IconSet};
use crate::keymap::Keymap;
use crate::options::{
    NO_OPTIONS, OptionsState, SCROLL_ROWS_PER_NOTCH, key_line, options_row_count, title,
};
use crate::text::{font_size_points, printable, ui_unit};
use crate::theme::{ACCENT, SURFACE_0, SURFACE_1, SURFACE_3, TEXT, TEXT_DIM};
use crate::tool::Tool;
use crate::view::Zoom;

/// Qué hace el clic derecho en un botón (tabla de datos; el hito 8 puede unirla a `GestureMap`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Secondary {
    /// Nada.
    None,
    /// Otra acción (en `Acercar` y `Alejar`, la contraria).
    Action(AppAction),
    /// Elegir la herramienta del botón y llevar el foco al recuadro de opciones.
    FocusOptions,
}

/// Un botón de la caja: rótulo (tooltip y texto accesible), icono, acción del clic izquierdo y qué
/// hace el derecho. El atajo de las herramientas sale de la tabla de teclas
/// (`Keymap::shown_text`) y solo se dice en el tooltip.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Button {
    pub label: &'static str,
    pub icon: Icon,
    pub action: AppAction,
    pub secondary: Secondary,
}

const fn tool_button(label: &'static str, icon: Icon, tool: Tool) -> Button {
    Button {
        label,
        icon,
        action: AppAction::SelectTool(tool),
        secondary: Secondary::FocusOptions,
    }
}

/// Los cinco botones de herramienta, de arriba abajo. Los rótulos son los de `chrome::TOOL_LABELS`.
pub const TOOL_BUTTONS: [Button; 5] = [
    tool_button("Lápiz", Icon::Pencil, Tool::Pencil),
    tool_button("Línea", Icon::Line, Tool::Line),
    tool_button("Relleno", Icon::Fill, Tool::Fill),
    tool_button("Cuentagotas", Icon::Dropper, Tool::Dropper),
    tool_button("Borrador", Icon::Eraser, Tool::Eraser),
];

/// Los cuatro botones de zoom: el atajo se dice en el tooltip y en el menú Ver. `Acercar` y
/// `Alejar` se invierten con el clic derecho; `Ajustar` y `1:1` no hacen nada con él.
pub const ZOOM_BUTTONS: [Button; 4] = [
    Button {
        label: "Acercar",
        icon: Icon::ZoomIn,
        action: AppAction::ZoomIn,
        secondary: Secondary::Action(AppAction::ZoomOut),
    },
    Button {
        label: "Alejar",
        icon: Icon::ZoomOut,
        action: AppAction::ZoomOut,
        secondary: Secondary::Action(AppAction::ZoomIn),
    },
    Button {
        label: "Ajustar",
        icon: Icon::ZoomFit,
        action: AppAction::ZoomFit,
        secondary: Secondary::None,
    },
    Button {
        label: "1:1",
        icon: Icon::ZoomActual,
        action: AppAction::ZoomActual,
        secondary: Secondary::None,
    },
];

/// Lo que da un clic derecho en un botón, con el estado de la interfaz.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SecondaryOutcome {
    Nothing,
    /// Una acción de vista (el zoom contrario).
    Action(AppAction),
    /// Elegir esta herramienta y dar el foco al recuadro de opciones.
    SelectAndFocus(AppAction),
}

/// Resultado del clic derecho en `button`: **se habilita por la acción efectiva** (la del
/// derecho, no la del izquierdo) con las mismas reglas que el menú, así que en ×64 `Acercar` está
/// atenuado para el izquierdo y el derecho aleja.
pub fn secondary_outcome(button: &Button, state: &UiState) -> SecondaryOutcome {
    match button.secondary {
        Secondary::None => SecondaryOutcome::Nothing,
        Secondary::Action(action) if enabled(action, state) => SecondaryOutcome::Action(action),
        Secondary::FocusOptions if enabled(button.action, state) => {
            SecondaryOutcome::SelectAndFocus(button.action)
        }
        _ => SecondaryOutcome::Nothing,
    }
}

/// Texto del nivel de zoom: `Zoom ×12`, `Zoom ×1/8` o `Zoom —` si no hay imagen.
pub fn zoom_label(zoom: Option<Zoom>) -> String {
    match zoom {
        Some(zoom) => format!("Zoom {zoom}"),
        None => "Zoom —".to_string(),
    }
}

/// Tooltip de un botón, leído de la tabla del teclado: `Nombre (atajo)` (solo el nombre si no hay
/// atajo) y, en una segunda línea, lo que hace el clic derecho.
pub fn tooltip_text(button: &Button, keymap: &Keymap, platform: Platform) -> String {
    let mut text = match keymap.shown_text(button.action, platform) {
        Some(shortcut) => format!("{} ({shortcut})", button.label),
        None => button.label.to_string(),
    };
    let second = match button.secondary {
        Secondary::None => None,
        Secondary::FocusOptions => Some("opciones"),
        Secondary::Action(AppAction::ZoomIn) => Some("acercar"),
        Secondary::Action(AppAction::ZoomOut) => Some("alejar"),
        Secondary::Action(_) => None,
    };
    if let Some(second) = second {
        text.push_str("\nclic derecho: ");
        text.push_str(second);
    }
    text
}

/// Lo que la caja necesita para pintarse.
pub struct ToolboxView<'a> {
    pub state: &'a UiState,
    /// Texto del nivel de zoom (de [`zoom_label`]).
    pub zoom_label: &'a str,
    pub options: &'a OptionsState,
    /// Las texturas de los iconos (vacío: los botones se pintan sin icono).
    pub icons: &'a IconSet,
}

/// Lo que pasó en la caja en el fotograma.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ToolboxOutput {
    /// La acción del botón pulsado (clic izquierdo, o el derecho en `Acercar`, `Alejar` y las
    /// herramientas).
    pub action: Option<AppAction>,
    /// Un clic derecho permitido en una herramienta: el foco va al recuadro de opciones.
    pub focus_options: bool,
    /// Se pulsó un botón del ratón fuera del recuadro de opciones (quita el foco).
    pub pressed_outside_options: bool,
    /// Filas que pide desplazar la rueda sobre el cuerpo del recuadro (positivo = hacia abajo).
    pub options_scroll_rows: i64,
}

/// Pinta la caja en el borde **derecho** de `ui` y devuelve lo que pasó en ella. Hay que llamarla
/// **antes** del panel de paletas (que se apila a su izquierda). Lo que no cabe se recorta a la
/// caja: no se desplaza. Cada botón es un icono sobre una cara con relieve; su rótulo, su atajo y
/// lo que hace el clic derecho van en el tooltip, y el rótulo en el texto accesible.
pub fn show_toolbox(
    ui: &mut Ui,
    view: &ToolboxView,
    keymap: &Keymap,
    platform: Platform,
) -> ToolboxOutput {
    let ctx = ui.ctx().clone();
    let ppp = ctx.pixels_per_point();
    let u = ui_unit(ppp);
    let font = FontId::new(font_size_points(ppp), FontFamily::Proportional);
    let state = view.state;
    let mut out = ToolboxOutput::default();
    Panel::right("herramientas")
        .exact_size(toolbar_width_u() as f32 * u)
        .show_separator_line(false)
        .frame(Frame::new().fill(SURFACE_1))
        .show(ui, |ui| {
            let (rect, _) = ui.allocate_exact_size(ui.available_size(), Sense::hover());
            let at = |x_u: i64, y_u: i64| rect.min + vec2(x_u as f32 * u, y_u as f32 * u);
            let buttons = TOOL_BUTTONS.iter().chain(ZOOM_BUTTONS.iter());
            debug_assert_eq!(buttons.clone().count(), TOOLBOX_BUTTONS);
            for (index, button) in buttons.enumerate() {
                let (x_u, y_u, w_u, h_u) = toolbox_button_rect_u(index);
                let button_rect = Rect::from_min_size(
                    at(x_u, y_u),
                    vec2(w_u as f32 * u, h_u as f32 * u),
                );
                let on = enabled(button.action, state);
                let secondary = secondary_outcome(button, state);
                let active = button.action == AppAction::SelectTool(state.tool);
                let sense = if on || secondary != SecondaryOutcome::Nothing {
                    Sense::click()
                } else {
                    Sense::hover()
                };
                let response =
                    ui.interact(button_rect, ui.unique_id().with(("boton", index)), sense);
                paint_icon(
                    ui.painter(),
                    button_rect,
                    u,
                    view.icons.get(button.icon),
                    &look(active, on, response.hovered()),
                );
                if on && response.clicked() {
                    out.action = Some(button.action);
                }
                if response.secondary_clicked() {
                    match secondary {
                        SecondaryOutcome::Nothing => {}
                        SecondaryOutcome::Action(action) => out.action = Some(action),
                        SecondaryOutcome::SelectAndFocus(action) => {
                            out.action = Some(action);
                            out.focus_options = true;
                        }
                    }
                }
                response.widget_info(|| WidgetInfo::labeled(Role::Button, on, button.label));
                let tooltip = tooltip_text(button, keymap, platform);
                response.on_hover_ui(|ui| {
                    for line in tooltip.lines() {
                        ui.add(
                            Label::new(
                                RichText::new(printable(&ctx, line))
                                    .font(font.clone())
                                    .color(TEXT),
                            )
                            .extend(),
                        );
                    }
                });
            }
            let separator = Rect::from_min_size(
                at(TOOLBAR_MARGIN_U, TOOLBAR_SEPARATOR_Y_U),
                vec2(toolbar_button_width_u() as f32 * u, u),
            );
            ui.painter().rect_filled(separator, 0.0, SURFACE_3);
            ui.painter().text(
                at(TOOLBAR_MARGIN_U + BUTTON_PAD_U, ZOOM_LABEL_Y_U),
                Align2::LEFT_TOP,
                view.zoom_label,
                font.clone(),
                TEXT,
            );
            show_options(ui, view, keymap, platform, rect.min, u, &font, &mut out);
        });
    out
}

/// El recuadro de opciones: relleno, borde (de 1 u, o de 2 u `ACCENT` con el foco), nombre de la
/// herramienta, línea del atajo y cuerpo recortado con su desplazamiento por filas.
#[allow(clippy::too_many_arguments)]
fn show_options(
    ui: &mut Ui,
    view: &ToolboxView,
    keymap: &Keymap,
    platform: Platform,
    origin: egui::Pos2,
    u: f32,
    font: &FontId,
    out: &mut ToolboxOutput,
) {
    let ctx = ui.ctx().clone();
    let at = |x_u: i64, y_u: i64| origin + vec2(x_u as f32 * u, y_u as f32 * u);
    let size = |w_u: i64, h_u: i64| vec2(w_u as f32 * u, h_u as f32 * u);
    let panel = Rect::from_min_size(at(OPTIONS_X_U, OPTIONS_Y_U), size(OPTIONS_W_U, OPTIONS_H_U));
    let body = Rect::from_min_size(
        at(OPTIONS_BODY_X_U, OPTIONS_BODY_Y_U),
        size(OPTIONS_BODY_W_U, OPTIONS_BODY_H_U),
    );
    let tool = view.state.tool;
    let painter = ui.painter().clone();
    painter.rect_filled(panel, 0.0, SURFACE_0);
    let (border, color) = if view.options.focused {
        (OPTIONS_FOCUS_BORDER_U as f32 * u, ACCENT)
    } else {
        (OPTIONS_BORDER_U as f32 * u, SURFACE_3)
    };
    for strip in [
        Rect::from_min_size(panel.min, vec2(panel.width(), border)),
        Rect::from_min_size(
            pos2(panel.min.x, panel.max.y - border),
            vec2(panel.width(), border),
        ),
        Rect::from_min_size(
            pos2(panel.min.x, panel.min.y + border),
            vec2(border, panel.height() - 2.0 * border),
        ),
        Rect::from_min_size(
            pos2(panel.max.x - border, panel.min.y + border),
            vec2(border, panel.height() - 2.0 * border),
        ),
    ] {
        painter.rect_filled(strip, 0.0, color);
    }
    painter.text(
        at(OPTIONS_TITLE_X_U, OPTIONS_TITLE_Y_U),
        Align2::LEFT_TOP,
        printable(&ctx, title(tool)),
        font.clone(),
        TEXT,
    );
    painter.text(
        at(OPTIONS_TITLE_X_U, OPTIONS_KEY_Y_U),
        Align2::LEFT_TOP,
        printable(&ctx, &key_line(tool, keymap, platform)),
        font.clone(),
        TEXT_DIM,
    );
    // El cuerpo se recorta a su rectángulo: con filas de más (hito 4) se desplaza por filas.
    let body_clip = body.intersect(painter.clip_rect());
    let body_painter = painter.with_clip_rect(body_clip);
    // Con la caja recortada por debajo del cuerpo (respaldo B) no hay nada que pintar.
    if options_row_count(tool) == 0 && body_clip.is_positive() {
        body_painter.text(
            body.min,
            Align2::LEFT_TOP,
            NO_OPTIONS,
            font.clone(),
            TEXT_DIM,
        );
    }
    ui.input(|input| {
        let hover = input.pointer.hover_pos();
        for event in &input.events {
            match event {
                Event::PointerButton {
                    pos, pressed: true, ..
                } if !panel.contains(*pos) => out.pressed_outside_options = true,
                Event::MouseWheel { unit, delta, .. }
                    if hover.is_some_and(|p| body.contains(p)) =>
                {
                    let rows = match unit {
                        MouseWheelUnit::Line | MouseWheelUnit::Page => {
                            -delta.y * SCROLL_ROWS_PER_NOTCH as f32
                        }
                        // Píxeles: una fila son `14 u` puntos.
                        MouseWheelUnit::Point => -delta.y / (OPTIONS_ROW_H_U as f32 * u),
                    };
                    out.options_scroll_rows += rows.round() as i64;
                }
                _ => {}
            }
        }
    });
}
