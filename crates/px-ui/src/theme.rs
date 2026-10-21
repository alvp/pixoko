//! Tema mínimo: fondo neutro y color solo para acentos y avisos (specs/hito-2-texto.md).
//!
//! Los grises son neutros (`gris(p) = round(p·255/100)`). Los dos tonos salen de una regla en
//! HSL elegida aquí: acento 168° (62 %, 52 %) y aviso 28° (85 %, 58 %).

use egui::{Color32, CornerRadius, Shadow, Stroke, Visuals};

/// Campos de texto.
pub const SURFACE_0: Color32 = Color32::from_rgb(0x1a, 0x1a, 0x1a);
/// Paneles y barra de estado.
pub const SURFACE_1: Color32 = Color32::from_rgb(0x24, 0x24, 0x24);
/// Ventanas y botones.
pub const SURFACE_2: Color32 = Color32::from_rgb(0x30, 0x30, 0x30);
/// Botón con el ratón encima.
pub const SURFACE_3: Color32 = Color32::from_rgb(0x42, 0x42, 0x42);
pub const TEXT: Color32 = Color32::from_rgb(0xe0, 0xe0, 0xe0);
pub const TEXT_DIM: Color32 = Color32::from_rgb(0xa3, 0xa3, 0xa3);
/// Texto de lo deshabilitado (menú y columna de herramientas).
pub const TEXT_DISABLED: Color32 = Color32::from_rgb(0x6b, 0x6b, 0x6b);
/// Selección y enlaces.
pub const ACCENT: Color32 = Color32::from_rgb(0x39, 0xd0, 0xb2);
/// Avisos (siempre con `NOTICE_PREFIX`: el color solo refuerza).
pub const WARNING: Color32 = Color32::from_rgb(0xef, 0x8e, 0x39);

/// Relieve de los botones de icono (specs/interfaz-iconos.md, decisión 3): el lado claro (arriba y a
/// la izquierda, o abajo y a la derecha si el botón está hundido) da el contorno con ≥ 3:1 contra la
/// cara, el fondo de la caja y el lado oscuro; el oscuro no puede medirse contra una cara oscura
/// (ni el negro puro pasa de 1,59:1) y solo da la sombra.
pub const BEVEL_LIGHT: Color32 = TEXT_DIM;
pub const BEVEL_DARK: Color32 = Color32::from_rgb(0x10, 0x10, 0x10);

/// Anillo de la muestra elegida en el panel de paletas (contraste ≥ 3:1 con `SURFACE_1`).
pub const RING_ACTIVE: Color32 = ACCENT;
/// Anillo (en esquinas) de la muestra elegida como color de fondo: contraste ≥ 3:1 con
/// `SURFACE_1`, distinto del verde del primer plano y del naranja del destino de una quita.
pub const RING_BACKGROUND: Color32 = Color32::from_rgb(0xf2, 0xd0, 0x3c);
/// Anillo de la muestra bajo el cursor.
pub const RING_HOVER: Color32 = TEXT;

/// Anillo de las muestras del rango de rotación (1 u; la elegida lleva uno de 2 u).
pub const RING_RANGE: Color32 = ACCENT;
/// Anillo del destino de una quita pendiente.
pub const RING_DEST: Color32 = WARNING;
/// Barra que marca dónde caerá una muestra arrastrada.
pub const DROP_MARK: Color32 = TEXT;

/// Prefijo de texto de los avisos: no dependen solo del color.
pub const NOTICE_PREFIX: &str = "aviso: ";

/// Luminancia relativa de WCAG 2.x.
fn luminance(color: Color32) -> f64 {
    let linear = |v: u8| {
        let v = f64::from(v) / 255.0;
        if v <= 0.03928 {
            v / 12.92
        } else {
            ((v + 0.055) / 1.055).powf(2.4)
        }
    };
    0.2126 * linear(color.r()) + 0.7152 * linear(color.g()) + 0.0722 * linear(color.b())
}

/// Contraste WCAG 2.x entre dos colores: `(L1 + 0,05) / (L2 + 0,05)` con `L1 ≥ L2`. Simétrico.
pub fn contrast_ratio(a: Color32, b: Color32) -> f64 {
    let (la, lb) = (luminance(a), luminance(b));
    let (light, dark) = if la >= lb { (la, lb) } else { (lb, la) };
    (light + 0.05) / (dark + 0.05)
}

/// Estilo visual: oscuro, neutro, esquinas y sombras a 0 y trazos de `u` puntos (un píxel de
/// interfaz, `n` píxeles físicos).
pub fn visuals(ppp: f32) -> Visuals {
    let u = crate::text::ui_unit(ppp);
    let mut v = Visuals::dark();
    v.override_text_color = Some(TEXT);
    v.panel_fill = SURFACE_1;
    v.window_fill = SURFACE_2;
    v.extreme_bg_color = SURFACE_0;
    v.faint_bg_color = SURFACE_2;
    v.window_stroke = Stroke::new(u, SURFACE_3);
    v.window_corner_radius = CornerRadius::ZERO;
    v.menu_corner_radius = CornerRadius::ZERO;
    v.window_shadow = Shadow::NONE;
    v.popup_shadow = Shadow::NONE;
    v.hyperlink_color = ACCENT;
    v.warn_fg_color = WARNING;
    v.error_fg_color = WARNING;
    // El texto seleccionado se pinta con el trazo de la selección: oscuro sobre el acento.
    v.selection.bg_fill = ACCENT;
    v.selection.stroke = Stroke::new(u, SURFACE_0);

    let widgets = &mut v.widgets;
    for (state, fill, edge) in [
        (&mut widgets.noninteractive, SURFACE_1, SURFACE_3),
        (&mut widgets.inactive, SURFACE_2, SURFACE_3),
        (&mut widgets.hovered, SURFACE_3, ACCENT),
        (&mut widgets.active, SURFACE_3, ACCENT),
        (&mut widgets.open, SURFACE_2, ACCENT),
    ] {
        state.bg_fill = fill;
        state.weak_bg_fill = fill;
        state.bg_stroke = Stroke::new(u, edge);
        state.fg_stroke = Stroke::new(u, TEXT);
        state.corner_radius = CornerRadius::ZERO;
        state.expansion = 0.0;
    }
    v
}
