//! Columna de herramientas y botones de zoom (specs/hito-2-menu-herramientas.md, «Geometría» y
//! «Piezas»): botones de texto de 99 × 16 u en una columna de 107 u, a la izquierda del panel de
//! paletas. Los rótulos y las acciones son tablas puras; `show_toolbar` solo las pinta.

use egui::{Align2, FontFamily, FontId, Frame, Panel, Rect, Sense, Ui, pos2, vec2};

use crate::actions::{AppAction, Platform, UiState, enabled};
use crate::chrome::{
    BUTTON_PAD_U, TOOLBAR_MARGIN_U, TOOLBAR_SEPARATOR_Y_U, TOOLBAR_STEP_U, TOOLS_Y_U,
    ZOOM_LABEL_Y_U, ZOOM_Y_U, toolbar_button_width_u, toolbar_width_u,
};
use crate::edit_layout::{BUTTON_H_U, BUTTON_TEXT_Y_U};
use crate::keymap::{Keymap, default_keymap};
use crate::text::{font_size_points, ui_unit};
use crate::theme::{
    ACCENT, SURFACE_0, SURFACE_1, SURFACE_2, SURFACE_3, TEXT, TEXT_DIM, TEXT_DISABLED,
};
use crate::tool::Tool;
use crate::view::Zoom;

/// Un botón de la columna: rótulo y acción. La letra del atajo de las herramientas sale de la
/// tabla de teclas (`Keymap::shown_text`), no de aquí.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Button {
    pub label: &'static str,
    pub action: AppAction,
}

/// Los cinco botones de herramienta, de arriba abajo. Los rótulos son los de `chrome::TOOL_LABELS`.
pub const TOOL_BUTTONS: [Button; 5] = [
    Button {
        label: "Lápiz",
        action: AppAction::SelectTool(Tool::Pencil),
    },
    Button {
        label: "Línea",
        action: AppAction::SelectTool(Tool::Line),
    },
    Button {
        label: "Relleno",
        action: AppAction::SelectTool(Tool::Fill),
    },
    Button {
        label: "Cuentagotas",
        action: AppAction::SelectTool(Tool::Dropper),
    },
    Button {
        label: "Borrador",
        action: AppAction::SelectTool(Tool::Eraser),
    },
];

/// Los cuatro botones de zoom, sin letra: el menú Ver muestra sus atajos.
pub const ZOOM_BUTTONS: [Button; 4] = [
    Button {
        label: "Acercar",
        action: AppAction::ZoomIn,
    },
    Button {
        label: "Alejar",
        action: AppAction::ZoomOut,
    },
    Button {
        label: "Ajustar",
        action: AppAction::ZoomFit,
    },
    Button {
        label: "1:1",
        action: AppAction::ZoomActual,
    },
];

/// Texto del nivel de zoom: `Zoom ×12`, `Zoom ×1/8` o `Zoom —` si no hay imagen.
pub fn zoom_label(zoom: Option<Zoom>) -> String {
    match zoom {
        Some(zoom) => format!("Zoom {zoom}"),
        None => "Zoom —".to_string(),
    }
}

/// Pinta la columna en el borde izquierdo de `ui` y devuelve la acción del botón pulsado, si la
/// hay. Hay que llamarla **antes** del panel de paletas. `zoom_label` es el texto del nivel (de
/// [`zoom_label`]). Lo que no cabe se recorta a la columna: no se desplaza.
pub fn show_toolbar(ui: &mut Ui, state: &UiState, zoom_label: &str) -> Option<AppAction> {
    show_toolbar_with(ui, state, zoom_label, default_keymap(), Platform::current())
}

/// Como [`show_toolbar`] con la letra de cada herramienta leída de `keymap` (la app pasa su tabla).
/// Los botones sin atajo (el Borrador) y los de zoom no llevan letra.
pub fn show_toolbar_with(
    ui: &mut Ui,
    state: &UiState,
    zoom_label: &str,
    keymap: &Keymap,
    platform: Platform,
) -> Option<AppAction> {
    let ctx = ui.ctx().clone();
    let ppp = ctx.pixels_per_point();
    let u = ui_unit(ppp);
    let font = FontId::new(font_size_points(ppp), FontFamily::Proportional);
    let mut chosen = None;
    Panel::left("herramientas")
        .exact_size(toolbar_width_u() as f32 * u)
        .show_separator_line(false)
        .frame(Frame::new().fill(SURFACE_1))
        .show(ui, |ui| {
            let (rect, _) = ui.allocate_exact_size(ui.available_size(), Sense::hover());
            let at = |x_u: i64, y_u: i64| rect.min + vec2(x_u as f32 * u, y_u as f32 * u);
            let button_w = toolbar_button_width_u();
            let mut draw =
                |ui: &mut Ui, index: usize, y_u: i64, button: &Button, letter: Option<String>| {
                    let button_rect = Rect::from_min_size(
                        at(TOOLBAR_MARGIN_U, y_u),
                        vec2(button_w as f32 * u, BUTTON_H_U as f32 * u),
                    );
                    let on = enabled(button.action, state);
                    let active = button.action == AppAction::SelectTool(state.tool);
                    let sense = if on { Sense::click() } else { Sense::hover() };
                    let response =
                        ui.interact(button_rect, ui.unique_id().with(("boton", index)), sense);
                    let fill = if active {
                        ACCENT
                    } else if on && response.hovered() {
                        SURFACE_3
                    } else {
                        SURFACE_2
                    };
                    let (label_color, key_color) = match (on, active) {
                        (false, _) => (TEXT_DISABLED, TEXT_DISABLED),
                        (true, true) => (SURFACE_0, SURFACE_0),
                        (true, false) => (TEXT, TEXT_DIM),
                    };
                    let painter = ui.painter();
                    painter.rect_filled(button_rect, 0.0, fill);
                    let text_y = button_rect.min.y + BUTTON_TEXT_Y_U as f32 * u;
                    painter.text(
                        pos2(button_rect.min.x + BUTTON_PAD_U as f32 * u, text_y),
                        Align2::LEFT_TOP,
                        button.label,
                        font.clone(),
                        label_color,
                    );
                    if let Some(letter) = letter {
                        painter.text(
                            pos2(button_rect.max.x - BUTTON_PAD_U as f32 * u, text_y),
                            Align2::RIGHT_TOP,
                            letter,
                            font.clone(),
                            key_color,
                        );
                    }
                    if on && response.clicked() {
                        chosen = Some(button.action);
                    }
                };
            for (i, button) in TOOL_BUTTONS.iter().enumerate() {
                let letter = keymap.shown_text(button.action, platform);
                draw(ui, i, TOOLS_Y_U + TOOLBAR_STEP_U * i as i64, button, letter);
            }
            let separator = Rect::from_min_size(
                at(TOOLBAR_MARGIN_U, TOOLBAR_SEPARATOR_Y_U),
                vec2(button_w as f32 * u, u),
            );
            ui.painter().rect_filled(separator, 0.0, SURFACE_3);
            ui.painter().text(
                at(TOOLBAR_MARGIN_U + BUTTON_PAD_U, ZOOM_LABEL_Y_U),
                Align2::LEFT_TOP,
                zoom_label,
                font.clone(),
                TEXT,
            );
            for (i, button) in ZOOM_BUTTONS.iter().enumerate() {
                draw(
                    ui,
                    TOOL_BUTTONS.len() + i,
                    ZOOM_Y_U + TOOLBAR_STEP_U * i as i64,
                    button,
                    None,
                );
            }
        });
    chosen
}
