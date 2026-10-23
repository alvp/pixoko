//! Caja de herramientas: botones de herramienta y de zoom (specs/hito-2-menu-herramientas.md,
//! «Geometría» y «Piezas», specs/interfaz-iconos.md y specs/ui-single-column.md): botones de icono de
//! 24 × 24 u con relieve, en una rejilla de `c(k)` botones por fila, en la parte de arriba de la
//! columna única del borde derecho. El clic derecho en una herramienta la elige y abre o cierra su
//! ventana de opciones. Los rótulos, las acciones y el clic derecho son tablas puras;
//! `paint_toolbox` solo las pinta.

use egui::Rect;
use egui::{
    Align2, FontFamily, FontId, Frame, Label, Panel, Pos2, RichText, Role, Sense, Ui, WidgetInfo,
    vec2,
};

use crate::actions::{AppAction, Platform, UiState, enabled};
use crate::chrome::{
    BUTTON_PAD_U, COLUMN_DEFAULT_COLS, TOOL_LABELS, TOOLBAR_MARGIN_U, TOOLBOX_BUTTONS,
    column_width_u, toolbox_button_rect_u_at, toolbox_height_u, toolbox_separator_y_u,
    zoom_label_y_u,
};
use crate::icon_button::{look, paint_icon};
use crate::icons::{Icon, IconSet};
use crate::keymap::Keymap;
use crate::text::{font_size_points, printable, ui_unit};
use crate::theme::{SURFACE_1, SURFACE_3, TEXT};
use crate::tool::Tool;
use crate::view::Zoom;

/// Qué hace el clic derecho en un botón (tabla de datos; el hito 8 puede unirla a `GestureMap`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Secondary {
    /// Nada.
    None,
    /// Otra acción (en `Acercar` y `Alejar`, la contraria).
    Action(AppAction),
    /// Elegir la herramienta del botón y abrir o cerrar su ventana de opciones.
    ToggleOptions,
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
        secondary: Secondary::ToggleOptions,
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
        icon: Icon::ZoomActualSize,
        action: AppAction::ZoomActualSize,
        secondary: Secondary::None,
    },
];

/// Lo que da un clic derecho en un botón, con el estado de la interfaz.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SecondaryOutcome {
    Nothing,
    /// Una acción de vista (el zoom contrario).
    Action(AppAction),
    /// Elegir esta herramienta y abrir o cerrar su ventana de opciones.
    SelectAndToggle(AppAction),
}

/// Resultado del clic derecho en `button`: **se habilita por la acción efectiva** (la del
/// derecho, no la del izquierdo) con las mismas reglas que el menú, así que en ×64 `Acercar` está
/// atenuado para el izquierdo y el derecho aleja.
pub fn secondary_outcome(button: &Button, state: &UiState) -> SecondaryOutcome {
    match button.secondary {
        Secondary::None => SecondaryOutcome::Nothing,
        Secondary::Action(action) if enabled(action, state) => SecondaryOutcome::Action(action),
        Secondary::ToggleOptions if enabled(button.action, state) => {
            SecondaryOutcome::SelectAndToggle(button.action)
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

/// Tercera línea del tooltip del Borrador: lo que hace el clic derecho **en el lienzo**.
pub const ERASER_RIGHT_CLICK_HINT: &str = "el clic derecho también borra con el fondo";

/// Tooltip de un botón, leído de la tabla del teclado: `Nombre (shortcut_2)` (solo el nombre si no hay
/// atajo) y, en una segunda línea, lo que hace el clic derecho del botón; el Borrador añade una
/// tercera con el clic derecho del lienzo.
pub fn tooltip_text(button: &Button, keymap: &Keymap, platform: Platform) -> String {
    let mut text = match keymap.shown_text(button.action, platform) {
        Some(shortcut) => format!("{} ({shortcut})", button.label),
        None => button.label.to_string(),
    };
    let second = match button.secondary {
        Secondary::None => None,
        Secondary::ToggleOptions => Some("opciones"),
        Secondary::Action(AppAction::ZoomIn) => Some("acercar"),
        Secondary::Action(AppAction::ZoomOut) => Some("alejar"),
        Secondary::Action(_) => None,
    };
    if let Some(second) = second {
        text.push_str("\nclic derecho: ");
        text.push_str(second);
    }
    if button.action == AppAction::SelectTool(Tool::Eraser) {
        text.push('\n');
        text.push_str(ERASER_RIGHT_CLICK_HINT);
    }
    text
}

/// Lo que la caja necesita para pintarse.
pub struct ToolboxView<'a> {
    pub state: &'a UiState,
    /// Texto del nivel de zoom (de [`zoom_label`]).
    pub zoom_label: &'a str,
    /// Las texturas de los iconos (vacío: los botones se pintan sin icono).
    pub icons: &'a IconSet,
}

/// Lo que pasó en la caja en el fotograma.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ToolboxOutput {
    /// La acción del botón pulsado (clic izquierdo, o el derecho en `Acercar`, `Alejar` y las
    /// herramientas).
    pub action: Option<AppAction>,
    /// Un clic derecho permitido en una herramienta (`action` es `SelectTool` de ella): abre o cierra
    /// su ventana de opciones.
    pub toggle_options: bool,
}

/// Dónde se pinta la caja: la esquina `origin` de su rectángulo (por encima de `clip` si el bloque
/// de la columna está desplazado), la parte visible `clip` y las columnas de muestras de la columna.
#[derive(Clone, Copy, Debug)]
pub struct ToolboxPlace {
    pub origin: Pos2,
    pub clip: Rect,
    pub cols: i64,
}

/// Pinta la caja en el borde **derecho** de `ui` con las 16 columnas de hoy y devuelve lo que pasó en
/// ella (la columna única lo hace con [`paint_toolbox`]). Hay que llamarla **antes** de lo que se
/// apila a su izquierda. Lo que no cabe se recorta a la caja. Cada botón es un icono sobre una cara
/// con relieve; su rótulo, su atajo y lo que hace el clic derecho van en el tooltip, y el rótulo en
/// el texto accesible.
pub fn show_toolbox(
    ui: &mut Ui,
    view: &ToolboxView,
    keymap: &Keymap,
    platform: Platform,
) -> ToolboxOutput {
    let ppp = ui.ctx().pixels_per_point();
    let u = ui_unit(ppp);
    Panel::right("herramientas")
        .exact_size(column_width_u(COLUMN_DEFAULT_COLS) as f32 * u)
        .resizable(false)
        .show_separator_line(false)
        .frame(Frame::new().fill(SURFACE_1))
        .show(ui, |ui| {
            let (rect, _) = ui.allocate_exact_size(ui.available_size(), Sense::hover());
            let place = ToolboxPlace {
                origin: rect.min,
                clip: rect,
                cols: COLUMN_DEFAULT_COLS,
            };
            paint_toolbox(ui, view, keymap, platform, &place)
        })
        .inner
}

/// Pinta la caja donde dice `place` y devuelve lo que pasó en ella. Los botones se recolocan en
/// filas de `c(k)` y el zoom queda en una sola fila debajo.
pub fn paint_toolbox(
    ui: &mut Ui,
    view: &ToolboxView,
    keymap: &Keymap,
    platform: Platform,
    place: &ToolboxPlace,
) -> ToolboxOutput {
    let ctx = ui.ctx().clone();
    let ppp = ctx.pixels_per_point();
    let u = ui_unit(ppp);
    let font = FontId::new(font_size_points(ppp), FontFamily::Proportional);
    let state = view.state;
    let tools = TOOL_LABELS.len();
    let cols = place.cols;
    let mut out = ToolboxOutput::default();
    let painter = ui
        .painter()
        .with_clip_rect(place.clip.intersect(ui.clip_rect()));
    let at = |x_u: i64, y_u: i64| place.origin + vec2(x_u as f32 * u, y_u as f32 * u);
    let buttons = TOOL_BUTTONS.iter().chain(ZOOM_BUTTONS.iter());
    debug_assert_eq!(buttons.clone().count(), TOOLBOX_BUTTONS);
    for (index, button) in buttons.enumerate() {
        let (x_u, y_u, w_u, h_u) = toolbox_button_rect_u_at(index, tools, cols);
        let button_rect = Rect::from_min_size(at(x_u, y_u), vec2(w_u as f32 * u, h_u as f32 * u));
        let on = enabled(button.action, state);
        let secondary = secondary_outcome(button, state);
        let active = button.action == AppAction::SelectTool(state.tool);
        let sense = if on || secondary != SecondaryOutcome::Nothing {
            Sense::click()
        } else {
            Sense::hover()
        };
        // Lo que sale por arriba de la columna (bloque desplazado) no se ve ni responde.
        let visible = button_rect.intersect(place.clip);
        let response = if visible.is_positive() {
            ui.interact(visible, ui.unique_id().with(("boton", index)), sense)
        } else {
            ui.interact(
                Rect::from_min_size(place.clip.min, vec2(0.0, 0.0)),
                ui.unique_id().with(("boton", index)),
                Sense::hover(),
            )
        };
        paint_icon(
            &painter,
            button_rect,
            u,
            view.icons.get(button.icon),
            &look(active, on, response.hovered()),
        );
        if !visible.is_positive() {
            continue;
        }
        if on && response.clicked() {
            out.action = Some(button.action);
        }
        if response.secondary_clicked() {
            match secondary {
                SecondaryOutcome::Nothing => {}
                SecondaryOutcome::Action(action) => out.action = Some(action),
                SecondaryOutcome::SelectAndToggle(action) => {
                    out.action = Some(action);
                    out.toggle_options = true;
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
    let width_u = column_width_u(cols);
    let separator = Rect::from_min_size(
        at(TOOLBAR_MARGIN_U, toolbox_separator_y_u(tools, cols)),
        vec2((width_u - 2 * TOOLBAR_MARGIN_U) as f32 * u, u),
    );
    painter.rect_filled(separator, 0.0, SURFACE_3);
    painter.text(
        at(TOOLBAR_MARGIN_U + BUTTON_PAD_U, zoom_label_y_u(tools, cols)),
        Align2::LEFT_TOP,
        view.zoom_label,
        font.clone(),
        TEXT,
    );
    // La línea de 1 u entre la caja y la paleta.
    let line = Rect::from_min_size(
        at(0, toolbox_height_u(tools, cols)),
        vec2(width_u as f32 * u, u),
    );
    painter.rect_filled(line, 0.0, SURFACE_3);
    out
}
