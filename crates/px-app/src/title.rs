//! Texto del título de la ventana, como función pura (specs/hito-2-lapiz-deshacer.md, fila 17).
//! (Sin fuente en egui todavía, el estado va en el título.)

use px_core::Image;
use px_ui::canvas::CanvasInfo;
use px_ui::tool::{Tool, status_text};
use px_ui::view::Zoom;

/// Datos del título, ya calculados.
pub struct TitleInfo {
    pub size: Option<(u32, u32)>,
    pub zoom: Zoom,
    /// Píxel de la imagen bajo el cursor.
    pub hover: Option<(u32, u32)>,
    /// Herramienta activa; `None` no escribe herramienta. En la app siempre es `Some`
    /// (`app_title_info`).
    pub tool: Option<Tool>,
    /// Color activo, ya con su ranura (`color 3 #ff0000`).
    pub color: Option<String>,
    pub can_undo: bool,
    pub can_redo: bool,
    /// Error que sustituye al estado.
    pub error: Option<String>,
    /// Posición (desde 1) de la capa activa y número de capas, solo con más de una
    /// (specs/hito-3-capas-vista.md, decisión 6).
    pub layers: Option<(usize, usize)>,
}

/// `Pixoko: {w}×{h} · {zoom}`, y a continuación, cada uno solo si existe, ` · ({x}, {y})`,
/// ` · layer_2 {posición}/{total}` (solo con más de una capa),
/// ` · {tool_2} · {color}` (o solo uno de los dos), ` · undo` y ` · redo`.
/// Con error, `Pixoko: {error}` y nada más.
pub fn title_text(info: &TitleInfo) -> String {
    if let Some(error) = &info.error {
        return format!("Pixoko: {error}");
    }
    let size = info
        .size
        .map(|(width, height)| format!("{width}×{height}"))
        .unwrap_or_default();
    let mut title = format!("Pixoko: {size} · {}", info.zoom);
    if let Some((x, y)) = info.hover {
        title.push_str(&format!(" · ({x}, {y})"));
    }
    if let Some((position, total)) = info.layers {
        title.push_str(&format!(" · capa {position}/{total}"));
    }
    match (info.tool, info.color.as_deref()) {
        (Some(tool), color) => title.push_str(&format!(" · {}", status_text(tool, color))),
        (None, Some(color)) => title.push_str(&format!(" · {color}")),
        (None, None) => {}
    }
    if info.can_undo {
        title.push_str(" · deshacer");
    }
    if info.can_redo {
        title.push_str(" · rehacer");
    }
    title
}

/// `TitleInfo` de la app: el único sitio de producción que lo construye, siempre con la
/// herramienta (`tool: Some(tool)`). Una prueba vigila que `main.rs` no lo construya aparte.
pub fn app_title_info(
    tool: Tool,
    image: Option<&Image>,
    canvas: &CanvasInfo,
    color: Option<String>,
    can_undo: bool,
    can_redo: bool,
    error: Option<String>,
) -> TitleInfo {
    TitleInfo {
        layers: None,
        size: image.map(|image| (image.width(), image.height())),
        zoom: canvas.zoom,
        hover: canvas.hover,
        tool: Some(tool),
        color,
        can_undo,
        can_redo,
        error,
    }
}

/// Texto de la barra de estado: el título de `title_text` sin el prefijo `Pixoko: ` y sin el
/// fichero (specs/hito-2-texto.md). `title_text` es, pues, `Pixoko: ` más esto.
pub fn status_line(info: &TitleInfo) -> String {
    let title = title_text(info);
    match title.strip_prefix("Pixoko: ") {
        Some(line) => line.to_string(),
        None => title,
    }
}

/// Lo que acompaña a la herramienta en el título y la barra: el color de primer plano y, tras
/// `background_2`, el de fondo (specs/interfaz-dos-colores.md, decisión 10). El borrador no usa los
/// colores y deja solo su propio texto.
pub fn colors_label(
    tool: Tool,
    foreground: Option<String>,
    background: Option<String>,
) -> Option<String> {
    if tool == Tool::Eraser {
        return foreground;
    }
    match (foreground, background) {
        (Some(foreground), Some(background)) => Some(format!("{foreground} · fondo {background}")),
        (foreground, _) => foreground,
    }
}

/// Texto de la barra de estado en `columns` caracteres: con poco ancho la línea pierde primero el
/// tamaño, después el zoom y por último la posición del cursor y deshacer y rehacer; nunca la
/// herramienta ni los dos colores. Con un error, el texto del error tal cual.
pub fn status_line_fit(info: &TitleInfo, columns: usize) -> String {
    let full = status_line(info);
    if info.error.is_some() || full.chars().count() <= columns {
        return full;
    }
    let tool = match (info.tool, info.color.as_deref()) {
        (Some(tool), color) => Some(status_text(tool, color)),
        (None, Some(color)) => Some(color.to_string()),
        (None, None) => None,
    };
    let line = |size: bool, zoom: bool, extras: bool| -> String {
        let mut parts: Vec<String> = Vec::new();
        if size && let Some((width, height)) = info.size {
            parts.push(format!("{width}×{height}"));
        }
        if zoom {
            parts.push(info.zoom.to_string());
        }
        if extras && let Some((x, y)) = info.hover {
            parts.push(format!("({x}, {y})"));
        }
        parts.extend(tool.clone());
        if extras && info.can_undo {
            parts.push("deshacer".to_string());
        }
        if extras && info.can_redo {
            parts.push("rehacer".to_string());
        }
        parts.join(" · ")
    };
    [
        line(false, true, true),
        line(false, false, true),
        line(false, false, false),
    ]
    .into_iter()
    .find(|candidate| candidate.chars().count() <= columns)
    .unwrap_or_else(|| line(false, false, false))
}

/// Antepone al título el fichero del documento (solo el nombre, sin directorios) y, si hay
/// cambios sin guardar, un `*`: `matches.png* — {title}`. Sin fichero y con cambios, `without título*`;
/// sin fichero y sin cambios, el título tal cual (specs/hito-2-abrir-guardar.md).
pub fn title_with_file(file: Option<&str>, modified: bool, title: &str) -> String {
    match (file, modified) {
        (Some(name), _) => format!("{name}{} — {title}", if modified { "*" } else { "" }),
        (None, true) => format!("sin título* — {title}"),
        (None, false) => title.to_string(),
    }
}

#[cfg(test)]
mod roles_tests;

#[cfg(test)]
mod tests {
    //! Fila 17 de specs/hito-2-lapiz-deshacer.md: el título de la ventana se construye con una
    //! función pura (`title_text`) que recibe los datos ya calculados, sin `egui`.
    //!
    //! Formato: `Pixoko: {w}×{h} · {zoom}`, y a continuación, cada uno solo si existe,
    //! ` · ({x}, {y})`, ` · {color}`, ` · undo` y ` · redo`. Con error, el texto es
    //! `Pixoko: {error}` y nada más. Sin tamaño se conserva lo que ya hacía `title_for` de
    //! `main.rs`: la cadena vacía en su lugar (`Pixoko:  · ×1`).

    use super::*;
    use px_ui::view::Zoom;

    fn zoom(n: u32) -> Zoom {
        Zoom::from_int(n).expect("el zoom existe")
    }

    /// El título completo de la spec, sin deshacer ni rehacer.
    fn base() -> TitleInfo {
        TitleInfo {
            layers: None,
            size: Some((64, 48)),
            zoom: zoom(8),
            hover: Some((3, 4)),
            tool: None,
            color: Some("color 3 #ff0000".to_string()),
            can_undo: false,
            can_redo: false,
            error: None,
        }
    }

    #[test]
    fn row_17_undo_and_redo_appear_only_if_possible() {
        let cases = [
            (
                false,
                false,
                "Pixoko: 64×48 · ×8 · (3, 4) · color 3 #ff0000",
            ),
            (
                true,
                false,
                "Pixoko: 64×48 · ×8 · (3, 4) · color 3 #ff0000 · deshacer",
            ),
            (
                false,
                true,
                "Pixoko: 64×48 · ×8 · (3, 4) · color 3 #ff0000 · rehacer",
            ),
            (
                true,
                true,
                "Pixoko: 64×48 · ×8 · (3, 4) · color 3 #ff0000 · deshacer · rehacer",
            ),
        ];
        for (can_undo, can_redo, expected_2) in cases {
            let info = TitleInfo {
                layers: None,
                can_undo,
                can_redo,
                ..base()
            };
            assert_eq!(
                title_text(&info),
                expected_2,
                "undo {can_undo}, redo {can_redo}"
            );
        }
    }

    #[test]
    fn row_17_the_error_replaces_the_state() {
        for (can_undo, can_redo) in [(false, false), (true, false), (false, true), (true, true)] {
            let info = TitleInfo {
                layers: None,
                can_undo,
                can_redo,
                error: Some("no hay memoria para guardar el paso en la historia".to_string()),
                ..base()
            };
            assert_eq!(
                title_text(&info),
                "Pixoko: no hay memoria para guardar el paso en la historia",
                "undo {can_undo}, redo {can_redo}"
            );
        }
        // También sin tamaño, sin cursor y sin color.
        let info = TitleInfo {
            layers: None,
            size: None,
            hover: None,
            color: None,
            error: Some("la imagen no cabe".to_string()),
            ..base()
        };
        assert_eq!(title_text(&info), "Pixoko: la imagen no cabe");
    }

    #[test]
    fn without_cursor_or_color_the_title_is_size_and_zoom() {
        let info = TitleInfo {
            layers: None,
            hover: None,
            color: None,
            ..base()
        };
        assert_eq!(title_text(&info), "Pixoko: 64×48 · ×8");
    }

    #[test]
    fn each_optional_part_appears_only_if_it_exists() {
        let only_cursor = TitleInfo {
            layers: None,
            color: None,
            ..base()
        };
        assert_eq!(title_text(&only_cursor), "Pixoko: 64×48 · ×8 · (3, 4)");
        let only_color = TitleInfo {
            layers: None,
            hover: None,
            ..base()
        };
        assert_eq!(
            title_text(&only_color),
            "Pixoko: 64×48 · ×8 · color 3 #ff0000"
        );
        // Deshacer y rehacer no dependen de lo demás.
        let only_history = TitleInfo {
            layers: None,
            hover: None,
            color: None,
            can_undo: true,
            can_redo: true,
            ..base()
        };
        assert_eq!(
            title_text(&only_history),
            "Pixoko: 64×48 · ×8 · deshacer · rehacer"
        );
    }

    #[test]
    fn without_size_the_empty_string_of_title_for_is_kept() {
        // Lo que daba `title_for` sin imagen: `Pixoko: {} · {zoom}` con el tamaño vacío.
        let info = TitleInfo {
            layers: None,
            size: None,
            zoom: Zoom::default(),
            hover: None,
            tool: None,
            color: None,
            can_undo: false,
            can_redo: false,
            error: None,
        };
        assert_eq!(title_text(&info), "Pixoko:  · ×1");
        // Y con cursor, como antes: `Pixoko:  · ×1 · (2, 5)`.
        let with_cursor = TitleInfo {
            layers: None,
            hover: Some((2, 5)),
            ..info
        };
        assert_eq!(title_text(&with_cursor), "Pixoko:  · ×1 · (2, 5)");
    }

    #[test]
    fn the_previous_title_is_kept_with_and_without_cursor() {
        // Las dos formas de `title_for` en `main.rs` antes de la unidad 6.
        let with = TitleInfo {
            layers: None,
            color: None,
            ..base()
        };
        assert_eq!(title_text(&with), "Pixoko: 64×48 · ×8 · (3, 4)");
        let without = TitleInfo {
            layers: None,
            hover: None,
            color: None,
            zoom: zoom(12),
            ..base()
        };
        assert_eq!(title_text(&without), "Pixoko: 64×48 · ×12");
    }
}

#[cfg(test)]
mod window_title_tests {
    //! Fila 22 de specs/hito-2-linea-relleno-app.md: `title_text(info)` da el título completo,
    //! con la herramienta en el campo `tool` de `TitleInfo` y `status_text(tool, color)` dentro de
    //! `title_text`. No toca los tests del módulo `tests`.

    use super::*;
    use px_ui::tool::Tool;
    use px_ui::view::Zoom;

    fn zoom(n: u32) -> Zoom {
        Zoom::from_int(n).expect("el zoom existe")
    }

    /// `info` completo: tamaño, zoom, cursor, color, y sin deshacer ni rehacer.
    fn complete() -> TitleInfo {
        TitleInfo {
            layers: None,
            size: Some((64, 48)),
            zoom: zoom(8),
            hover: Some((3, 4)),
            tool: Some(Tool::Line),
            color: Some("color 3 #ff0000".to_string()),
            can_undo: false,
            can_redo: false,
            error: None,
        }
    }

    #[test]
    fn row_22_full_title_with_the_line() {
        let info = TitleInfo {
            layers: None,
            can_undo: true,
            can_redo: true,
            ..complete()
        };
        assert_eq!(
            title_text(&info),
            "Pixoko: 64×48 · ×8 · (3, 4) · línea · color 3 #ff0000 · deshacer · rehacer"
        );
    }

    #[test]
    fn row_22_without_color_only_the_tool_shows() {
        let mut without_cursor = TitleInfo {
            layers: None,
            hover: None,
            color: None,
            ..complete()
        };
        without_cursor.tool = Some(Tool::Pencil);
        assert_eq!(title_text(&without_cursor), "Pixoko: 64×48 · ×8 · lápiz");
        without_cursor.tool = Some(Tool::Line);
        assert_eq!(title_text(&without_cursor), "Pixoko: 64×48 · ×8 · línea");
        without_cursor.tool = Some(Tool::Fill);
        assert_eq!(title_text(&without_cursor), "Pixoko: 64×48 · ×8 · relleno");
        let with_cursor = TitleInfo {
            layers: None,
            tool: Some(Tool::Pencil),
            color: None,
            ..complete()
        };
        assert_eq!(
            title_text(&with_cursor),
            "Pixoko: 64×48 · ×8 · (3, 4) · lápiz"
        );
    }

    #[test]
    fn row_22_undo_and_redo_appear_only_if_possible() {
        let cases = [
            (
                false,
                false,
                "Pixoko: 64×48 · ×8 · (3, 4) · línea · color 3 #ff0000",
            ),
            (
                true,
                false,
                "Pixoko: 64×48 · ×8 · (3, 4) · línea · color 3 #ff0000 · deshacer",
            ),
            (
                false,
                true,
                "Pixoko: 64×48 · ×8 · (3, 4) · línea · color 3 #ff0000 · rehacer",
            ),
            (
                true,
                true,
                "Pixoko: 64×48 · ×8 · (3, 4) · línea · color 3 #ff0000 · deshacer · rehacer",
            ),
        ];
        for (can_undo, can_redo, expected_2) in cases {
            let info = TitleInfo {
                layers: None,
                can_undo,
                can_redo,
                ..complete()
            };
            assert_eq!(
                title_text(&info),
                expected_2,
                "undo {can_undo}, redo {can_redo}"
            );
        }
    }

    #[test]
    fn row_22_with_an_error_the_title_is_pixoko_and_the_error_and_nothing_more() {
        for tool in [Tool::Pencil, Tool::Line, Tool::Fill] {
            for (can_undo, can_redo) in [(false, false), (true, false), (false, true), (true, true)]
            {
                let info = TitleInfo {
                    layers: None,
                    tool: Some(tool),
                    can_undo,
                    can_redo,
                    error: Some("no hay memoria para guardar el paso en la historia".to_string()),
                    ..complete()
                };
                assert_eq!(
                    title_text(&info),
                    "Pixoko: no hay memoria para guardar el paso en la historia",
                    "{tool:?}, undo {can_undo}, redo {can_redo}"
                );
            }
        }
    }

    #[test]
    fn the_fill_with_the_transparent_slot_and_the_tool_go_together() {
        let info = TitleInfo {
            layers: None,
            tool: Some(Tool::Fill),
            color: Some("color 9 transparente".to_string()),
            ..complete()
        };
        assert_eq!(
            title_text(&info),
            "Pixoko: 64×48 · ×8 · (3, 4) · relleno · color 9 transparente"
        );
    }

    #[test]
    fn without_size_the_empty_string_is_kept_and_the_tool_is_added() {
        let info = TitleInfo {
            layers: None,
            size: None,
            zoom: Zoom::default(),
            hover: None,
            tool: Some(Tool::Pencil),
            color: None,
            can_undo: false,
            can_redo: false,
            error: None,
        };
        assert_eq!(title_text(&info), "Pixoko:  · ×1 · lápiz");
    }
}

#[cfg(test)]
mod file_title_tests {
    //! Fila 21 de specs/hito-2-abrir-guardar.md (parte `title_with_file`): el nombre del fichero
    //! y el asterisco de modificado van delante del título normal. Sin fichero y sin cambios, el
    //! título no cambia; sin fichero y con cambios, el nombre es `without título`.

    use super::*;

    /// Un título cualquiera (`T` de la spec).
    const T: &str = "Pixoko: 16×16 · ×8";

    #[test]
    fn row_21_without_a_file_and_without_changes_it_is_todays_title() {
        assert_eq!(title_with_file(None, false, T), T);
    }

    #[test]
    fn row_21_with_a_file_without_changes() {
        assert_eq!(
            title_with_file(Some("a.png"), false, T),
            format!("a.png — {T}")
        );
    }

    #[test]
    fn row_21_with_a_file_and_changes_it_carries_an_asterisk() {
        assert_eq!(
            title_with_file(Some("a.png"), true, T),
            format!("a.png* — {T}")
        );
    }

    #[test]
    fn row_21_without_a_file_and_with_changes_it_is_untitled() {
        assert_eq!(title_with_file(None, true, T), format!("sin título* — {T}"));
    }

    #[test]
    fn the_title_with_an_error_also_carries_the_name() {
        // `title` ya viene calculado (con `window_title`), también cuando es un error.
        let error = "Pixoko: no se pudo guardar";
        assert_eq!(
            title_with_file(Some("casa.png"), true, error),
            format!("casa.png* — {error}")
        );
    }
}

#[cfg(test)]
mod status_line_tests {
    //! Fila 10 y propiedad 5 de specs/hito-2-texto.md: `status_line(info)` es el título de la
    //! ventana **sin** el prefijo `Pixoko: ` (la barra de estado repite lo que va en el título);
    //! `window_title` pasa a ser `format!("Pixoko: {}", status_line(..))` y `title_with_file` no
    //! cambia, así que el título sale idéntico. La herramienta va en el campo `tool` de
    //! `TitleInfo`. Con error o aviso, solo ese texto.
    //!
    //! La propiedad 5 exige además que `status_line` use solo caracteres de `INTERFACE_CHARSET`
    //! (la fuente de la interfaz tiene glifo para todos) y que no lleve saltos de línea.

    use super::*;
    use proptest::prelude::*;
    use px_ui::text::INTERFACE_CHARSET;
    use px_ui::tool::Tool;
    use px_ui::view::Zoom;

    fn zoom(n: u32) -> Zoom {
        Zoom::from_int(n).expect("el zoom existe")
    }

    /// `info` completo: tamaño, zoom, cursor, color, deshacer y rehacer.
    fn complete() -> TitleInfo {
        TitleInfo {
            layers: None,
            size: Some((64, 48)),
            zoom: zoom(8),
            hover: Some((3, 4)),
            tool: Some(Tool::Line),
            color: Some("color 3 #ff0000".to_string()),
            can_undo: true,
            can_redo: true,
            error: None,
        }
    }

    /// Comprueba `window_title == "Pixoko: " + status_line` y que `title_with_file` no cambia.
    fn checks_title(info: &TitleInfo, line: &str) {
        assert_eq!(status_line(info), line);
        let title = title_text(info);
        assert_eq!(title, format!("Pixoko: {line}"));
        assert_eq!(
            title_with_file(Some("casa.png"), true, &title),
            format!("casa.png* — Pixoko: {line}")
        );
        assert_eq!(
            title_with_file(Some("casa.png"), false, &title),
            format!("casa.png — Pixoko: {line}")
        );
        assert_eq!(
            title_with_file(None, true, &title),
            format!("sin título* — Pixoko: {line}")
        );
        assert_eq!(title_with_file(None, false, &title), title);
    }

    #[test]
    fn row_10_the_complete_status_line() {
        checks_title(
            &complete(),
            "64×48 · ×8 · (3, 4) · línea · color 3 #ff0000 · deshacer · rehacer",
        );
    }

    #[test]
    fn row_10_without_color_only_the_tool_shows() {
        let info = TitleInfo {
            layers: None,
            color: None,
            ..complete()
        };
        checks_title(&info, "64×48 · ×8 · (3, 4) · línea · deshacer · rehacer");
    }

    #[test]
    fn row_10_without_cursor_the_position_does_not_show() {
        let info = TitleInfo {
            layers: None,
            hover: None,
            ..complete()
        };
        checks_title(
            &info,
            "64×48 · ×8 · línea · color 3 #ff0000 · deshacer · rehacer",
        );
    }

    #[test]
    fn row_10_with_an_error_only_the_error_shows() {
        let info = TitleInfo {
            layers: None,
            error: Some("no hay memoria para guardar el paso en la historia".to_string()),
            ..complete()
        };
        checks_title(&info, "no hay memoria para guardar el paso en la historia");
    }

    #[test]
    fn row_10_each_tool_sets_its_label() {
        for (tool, label_2) in [
            (Tool::Pencil, "lápiz"),
            (Tool::Line, "línea"),
            (Tool::Fill, "relleno"),
        ] {
            let info = TitleInfo {
                layers: None,
                tool: Some(tool),
                can_undo: false,
                can_redo: false,
                ..complete()
            };
            checks_title(
                &info,
                &format!("64×48 · ×8 · (3, 4) · {label_2} · color 3 #ff0000"),
            );
        }
    }

    #[test]
    fn row_10_without_size_the_empty_string_of_the_title_is_kept() {
        // `window_title` da `Pixoko:  · ×1 · lápiz` (tests de la unidad 7) y sale idéntico:
        // la línea de estado es ese título sin el prefijo `Pixoko: `.
        let info = TitleInfo {
            layers: None,
            size: None,
            zoom: Zoom::default(),
            hover: None,
            tool: Some(Tool::Pencil),
            color: None,
            can_undo: false,
            can_redo: false,
            error: None,
        };
        assert_eq!(title_text(&info), "Pixoko:  · ×1 · lápiz");
        assert_eq!(status_line(&info), " · ×1 · lápiz");
    }

    // -----------------------------------------------------------------------------------------
    // Propiedad 5
    // -----------------------------------------------------------------------------------------

    fn tool_2() -> impl Strategy<Value = Tool> {
        prop::sample::select(vec![Tool::Pencil, Tool::Line, Tool::Fill])
    }

    /// Los zoom enteros posibles, y el de por defecto.
    fn zoom_any() -> impl Strategy<Value = Zoom> {
        let mut all_2: Vec<Zoom> = (1..=64).filter_map(Zoom::from_int).collect();
        all_2.push(Zoom::default());
        prop::sample::select(all_2)
    }

    /// Los textos de color que produce `slot_label`: `color 3 #ff0000`, `color 2 índice 1`,
    /// `color 4 gray 96` y `color 9 transparent`.
    fn color_any() -> impl Strategy<Value = Option<String>> {
        prop_oneof![
            1 => Just(None),
            3 => (0_u8..=255, any::<u8>(), any::<u8>(), any::<u8>())
                .prop_map(|(slot, r, g, b)| Some(format!("color {slot} #{r:02x}{g:02x}{b:02x}"))),
            1 => (0_u8..=255, any::<u8>())
                .prop_map(|(slot, i)| Some(format!("color {slot} índice {i}"))),
            1 => (0_u8..=255, any::<u8>())
                .prop_map(|(slot, g)| Some(format!("color {slot} gris {g}"))),
            1 => (0_u8..=255).prop_map(|slot| Some(format!("color {slot} transparente"))),
        ]
    }

    proptest! {
        #[test]
        fn property_5_the_title_is_pixoko_plus_the_status_line(
            tool in tool_2(),
            size in prop::option::of((0_u32..=16384, 0_u32..=16384)),
            zoom in zoom_any(),
            hover in prop::option::of((0_u32..=16384, 0_u32..=16384)),
            color in color_any(),
            can_undo in any::<bool>(),
            can_redo in any::<bool>(),
        ) {
            let info = TitleInfo { layers: None, size, zoom, hover, tool: Some(tool), color, can_undo, can_redo, error: None };
            let line = status_line(&info);
            prop_assert_eq!(title_text(&info), format!("Pixoko: {line}"));
            prop_assert!(!line.contains('\n') && !line.contains('\r'), "{:?}", line);
            for c in line.chars() {
                prop_assert!(
                    INTERFACE_CHARSET.contains(c),
                    "{:?} (U+{:04X}) no está en INTERFACE_CHARSET: {:?}",
                    c,
                    c as u32,
                    line
                );
            }
        }
    }
}

#[cfg(test)]
mod tool_field_tests {
    //! Unidad 14-limpieza (specs/hito-2-limpieza-api.md, filas 1 a 8 y propiedades P1 y P2):
    //! `TitleInfo` tiene un campo propio `tool: Option<Tool>`; `title_text` escribe la herramienta
    //! (entre el cursor y deshacer) solo si es `Some`; `title_text(&info)` y `status_line(&info)`
    //! ya no reciben la herramienta; y `app_title_info` construye el `TitleInfo` de producción con
    //! `tool` siempre `Some`.

    use super::*;
    use proptest::prelude::*;
    use px_core::{Image, Mode};
    use px_ui::canvas::CanvasInfo;
    use px_ui::text::INTERFACE_CHARSET;
    use px_ui::tool::Tool;
    use px_ui::view::Zoom;

    fn zoom(n: u32) -> Zoom {
        Zoom::from_int(n).expect("el zoom existe")
    }

    /// Las cinco herramientas con su etiqueta, escritas a mano (no con `Tool::label()`).
    const TOOLS: [(Tool, &str); 5] = [
        (Tool::Pencil, "lápiz"),
        (Tool::Line, "línea"),
        (Tool::Fill, "relleno"),
        (Tool::Dropper, "cuentagotas"),
        (Tool::Eraser, "borrador"),
    ];

    /// `base` de la spec: 64×48, zoom ×8, cursor (3, 4), `color 3 #ff0000`, sin deshacer ni
    /// rehacer y sin herramienta.
    fn base() -> TitleInfo {
        TitleInfo {
            layers: None,
            size: Some((64, 48)),
            zoom: zoom(8),
            hover: Some((3, 4)),
            tool: None,
            color: Some("color 3 #ff0000".to_string()),
            can_undo: false,
            can_redo: false,
            error: None,
        }
    }

    #[test]
    fn row_1_without_a_tool_the_tool_does_not_show() {
        assert_eq!(
            title_text(&base()),
            "Pixoko: 64×48 · ×8 · (3, 4) · color 3 #ff0000"
        );
    }

    #[test]
    fn row_2_with_the_line_it_comes_out_before_the_color() {
        let info = TitleInfo {
            layers: None,
            tool: Some(Tool::Line),
            ..base()
        };
        assert_eq!(
            title_text(&info),
            "Pixoko: 64×48 · ×8 · (3, 4) · línea · color 3 #ff0000"
        );
    }

    #[test]
    fn row_3_with_the_line_and_without_color_only_the_tool_shows() {
        let info = TitleInfo {
            layers: None,
            tool: Some(Tool::Line),
            color: None,
            ..base()
        };
        assert_eq!(title_text(&info), "Pixoko: 64×48 · ×8 · (3, 4) · línea");
    }

    #[test]
    fn row_4_the_five_tools_set_their_label() {
        let cases = [
            (Tool::Pencil, "Pixoko: 64×48 · ×8 · lápiz · color 3 #ff0000"),
            (Tool::Line, "Pixoko: 64×48 · ×8 · línea · color 3 #ff0000"),
            (Tool::Fill, "Pixoko: 64×48 · ×8 · relleno · color 3 #ff0000"),
            (
                Tool::Dropper,
                "Pixoko: 64×48 · ×8 · cuentagotas · color 3 #ff0000",
            ),
            (
                Tool::Eraser,
                "Pixoko: 64×48 · ×8 · borrador · color 3 #ff0000",
            ),
        ];
        for (tool, expected_2) in cases {
            let info = TitleInfo {
                layers: None,
                hover: None,
                tool: Some(tool),
                ..base()
            };
            assert_eq!(title_text(&info), expected_2, "{tool:?}");
        }
    }

    #[test]
    fn row_5_the_eraser_with_a_transparent_color_and_the_history_go_in_order() {
        let info = TitleInfo {
            layers: None,
            tool: Some(Tool::Eraser),
            color: Some("color 0 transparente".to_string()),
            can_undo: true,
            can_redo: true,
            ..base()
        };
        assert_eq!(
            title_text(&info),
            "Pixoko: 64×48 · ×8 · (3, 4) · borrador · color 0 transparente · deshacer · rehacer"
        );
    }

    #[test]
    fn row_6_with_an_error_the_title_is_pixoko_and_the_error_with_any_tool() {
        let tools_2 = [
            None,
            Some(Tool::Pencil),
            Some(Tool::Line),
            Some(Tool::Fill),
            Some(Tool::Dropper),
            Some(Tool::Eraser),
        ];
        for tool in tools_2 {
            for (can_undo, can_redo) in [(false, false), (true, true)] {
                let info = TitleInfo {
                    layers: None,
                    tool,
                    can_undo,
                    can_redo,
                    error: Some("no se pudo guardar".to_string()),
                    ..base()
                };
                assert_eq!(
                    title_text(&info),
                    "Pixoko: no se pudo guardar",
                    "{tool:?}, undo {can_undo}, redo {can_redo}"
                );
            }
        }
    }

    #[test]
    fn row_7_window_title_and_status_line_read_the_tool_from_the_field() {
        let without = base();
        assert_eq!(
            title_text(&without),
            "Pixoko: 64×48 · ×8 · (3, 4) · color 3 #ff0000"
        );
        assert_eq!(
            status_line(&without),
            "64×48 · ×8 · (3, 4) · color 3 #ff0000"
        );

        let with = TitleInfo {
            layers: None,
            tool: Some(Tool::Line),
            ..base()
        };
        assert_eq!(
            title_text(&with),
            "Pixoko: 64×48 · ×8 · (3, 4) · línea · color 3 #ff0000"
        );
        assert_eq!(
            status_line(&with),
            "64×48 · ×8 · (3, 4) · línea · color 3 #ff0000"
        );
    }

    #[test]
    fn row_8_app_title_info_always_puts_the_tool() {
        let image_2 = Image::new(64, 48, Mode::Rgba).expect("la imagen cabe");
        let canvas = CanvasInfo {
            zoom: zoom(8),
            hover: Some((3, 4)),
            image_rect: None,
            error: None,
        };
        for (tool, label_2) in TOOLS {
            for image in [Some(&image_2), None] {
                let info = app_title_info(
                    tool,
                    image,
                    &canvas,
                    Some("color 3 #ff0000".to_string()),
                    false,
                    false,
                    None,
                );
                assert_eq!(
                    info.tool,
                    Some(tool),
                    "{tool:?}, imagen {}",
                    image.is_some()
                );
                let title = title_text(&info);
                assert!(
                    title.contains(label_2),
                    "{tool:?}, imagen {}: {title:?}",
                    image.is_some()
                );
            }
        }
        // Guarda de texto: `lib.rs` (el cableado de la app) no construye el literal `TitleInfo { layers: None, .. }` por otro camino
        // que `app_title_info`. Es frágil a propósito (mira el texto fuente, no el
        // comportamiento): si falla, hay que mirar por qué antes de tocar la prueba.
        assert!(!include_str!("lib.rs").contains("TitleInfo { layers: None,"));
    }

    // -----------------------------------------------------------------------------------------
    // Propiedades
    // -----------------------------------------------------------------------------------------

    fn tool_2() -> impl Strategy<Value = Option<Tool>> {
        prop::sample::select(vec![
            None,
            Some(Tool::Pencil),
            Some(Tool::Line),
            Some(Tool::Fill),
            Some(Tool::Dropper),
            Some(Tool::Eraser),
        ])
    }

    /// Los zoom enteros posibles, y el de por defecto.
    fn zoom_any() -> impl Strategy<Value = Zoom> {
        let mut all_2: Vec<Zoom> = (1..=64).filter_map(Zoom::from_int).collect();
        all_2.push(Zoom::default());
        prop::sample::select(all_2)
    }

    /// Textos de color como los de `slot_label`, o ninguno.
    fn color_any() -> impl Strategy<Value = Option<String>> {
        prop_oneof![
            1 => Just(None),
            3 => "color [0-9]{1,3} #[0-9a-f]{6}".prop_map(Some),
            1 => "color [0-9]{1,3} índice [0-9]{1,3}".prop_map(Some),
            1 => "color [0-9]{1,3} gris [0-9]{1,3}".prop_map(Some),
            1 => "color [0-9]{1,3} transparente".prop_map(Some),
        ]
    }

    /// Un error cualquiera (o ninguno), solo con letras minúsculas y espacios.
    fn error_any() -> impl Strategy<Value = Option<String>> {
        prop_oneof![
            3 => Just(None),
            1 => "[a-z]{1,8}( [a-z]{1,8}){0,3}".prop_map(Some),
        ]
    }

    /// Oráculo de P1, escrito a mano con `format!`: la fórmula de antes del campo `tool`.
    fn oracle(info: &TitleInfo) -> String {
        if let Some(error) = &info.error {
            return format!("Pixoko: {error}");
        }
        let size = match info.size {
            Some((w, h)) => format!("{w}×{h}"),
            None => String::new(),
        };
        let cursor = match info.hover {
            Some((x, y)) => format!(" · ({x}, {y})"),
            None => String::new(),
        };
        let label_2 = info.tool.map(|tool| {
            TOOLS
                .iter()
                .find(|(t, _)| *t == tool)
                .map(|(_, label_2)| *label_2)
                .expect("la herramienta está en la tabla")
        });
        let state_2 = match (label_2, &info.color) {
            (Some(label_2), Some(color)) => format!(" · {label_2} · {color}"),
            (Some(label_2), None) => format!(" · {label_2}"),
            (None, Some(color)) => format!(" · {color}"),
            (None, None) => String::new(),
        };
        let undo = if info.can_undo { " · deshacer" } else { "" };
        let redo = if info.can_redo { " · rehacer" } else { "" };
        format!(
            "Pixoko: {size} · {}{cursor}{state_2}{undo}{redo}",
            info.zoom
        )
    }

    proptest! {
        #![proptest_config(ProptestConfig::with_cases(256))]

        #[test]
        fn property_1_title_text_matches_the_previous_formula(
            tool in tool_2(),
            size in prop::option::of((0_u32..=16384, 0_u32..=16384)),
            zoom in zoom_any(),
            hover in prop::option::of((0_u32..=16384, 0_u32..=16384)),
            color in color_any(),
            can_undo in any::<bool>(),
            can_redo in any::<bool>(),
            error in error_any(),
        ) {
            let info = TitleInfo { layers: None, size, zoom, hover, tool, color, can_undo, can_redo, error };
            prop_assert_eq!(title_text(&info), oracle(&info));
        }

        #[test]
        fn property_2_the_bar_only_uses_interface_characters_with_the_five_tools(
            tool in tool_2(),
            size in prop::option::of((0_u32..=16384, 0_u32..=16384)),
            zoom in zoom_any(),
            hover in prop::option::of((0_u32..=16384, 0_u32..=16384)),
            color in color_any(),
            can_undo in any::<bool>(),
            can_redo in any::<bool>(),
            error in error_any(),
        ) {
            let info = TitleInfo { layers: None, size, zoom, hover, tool, color, can_undo, can_redo, error };
            let line = status_line(&info);
            prop_assert!(!line.contains('\n') && !line.contains('\r'), "{:?}", line);
            for c in line.chars() {
                prop_assert!(
                    INTERFACE_CHARSET.contains(c),
                    "{:?} (U+{:04X}) no está en INTERFACE_CHARSET: {:?}",
                    c,
                    c as u32,
                    line
                );
            }
        }
    }
}
