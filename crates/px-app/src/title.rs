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
}

/// `Pixoko: {w}×{h} · {zoom}`, y a continuación, cada uno solo si existe, ` · ({x}, {y})`,
/// ` · {herramienta} · {color}` (o solo uno de los dos), ` · deshacer` y ` · rehacer`.
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

/// Título completo: es `title_text`. Existe solo porque varias specs y pruebas citan
/// `window_title` (filas 22 y 10, propiedad 5); debe quitarse cuando dejen de citarlo, y sus
/// llamadas pasan a `title_text` (specs/hito-2-limpieza-api.md).
pub fn window_title(info: &TitleInfo) -> String {
    title_text(info)
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

/// Texto de la barra de estado: el título de `window_title` sin el prefijo `Pixoko: ` y sin el
/// fichero (specs/hito-2-texto.md). `window_title` es, pues, `Pixoko: ` más esto.
pub fn status_line(info: &TitleInfo) -> String {
    let title = window_title(info);
    match title.strip_prefix("Pixoko: ") {
        Some(line) => line.to_string(),
        None => title,
    }
}

/// Antepone al título el fichero del documento (solo el nombre, sin directorios) y, si hay
/// cambios sin guardar, un `*`: `casa.png* — {title}`. Sin fichero y con cambios, `sin título*`;
/// sin fichero y sin cambios, el título tal cual (specs/hito-2-abrir-guardar.md).
pub fn title_with_file(file: Option<&str>, modified: bool, title: &str) -> String {
    match (file, modified) {
        (Some(name), _) => format!("{name}{} — {title}", if modified { "*" } else { "" }),
        (None, true) => format!("sin título* — {title}"),
        (None, false) => title.to_string(),
    }
}

#[cfg(test)]
mod tests {
    //! Fila 17 de specs/hito-2-lapiz-deshacer.md: el título de la ventana se construye con una
    //! función pura (`title_text`) que recibe los datos ya calculados, sin `egui`.
    //!
    //! Formato: `Pixoko: {w}×{h} · {zoom}`, y a continuación, cada uno solo si existe,
    //! ` · ({x}, {y})`, ` · {color}`, ` · deshacer` y ` · rehacer`. Con error, el texto es
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
    fn fila_17_deshacer_y_rehacer_aparecen_solo_si_se_puede() {
        let casos = [
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
        for (can_undo, can_redo, esperado) in casos {
            let info = TitleInfo {
                can_undo,
                can_redo,
                ..base()
            };
            assert_eq!(
                title_text(&info),
                esperado,
                "undo {can_undo}, redo {can_redo}"
            );
        }
    }

    #[test]
    fn fila_17_el_error_sustituye_al_estado() {
        for (can_undo, can_redo) in [(false, false), (true, false), (false, true), (true, true)] {
            let info = TitleInfo {
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
            size: None,
            hover: None,
            color: None,
            error: Some("la imagen no cabe".to_string()),
            ..base()
        };
        assert_eq!(title_text(&info), "Pixoko: la imagen no cabe");
    }

    #[test]
    fn sin_cursor_ni_color_el_titulo_es_tamano_y_zoom() {
        let info = TitleInfo {
            hover: None,
            color: None,
            ..base()
        };
        assert_eq!(title_text(&info), "Pixoko: 64×48 · ×8");
    }

    #[test]
    fn cada_parte_opcional_aparece_solo_si_existe() {
        let solo_cursor = TitleInfo {
            color: None,
            ..base()
        };
        assert_eq!(title_text(&solo_cursor), "Pixoko: 64×48 · ×8 · (3, 4)");
        let solo_color = TitleInfo {
            hover: None,
            ..base()
        };
        assert_eq!(
            title_text(&solo_color),
            "Pixoko: 64×48 · ×8 · color 3 #ff0000"
        );
        // Deshacer y rehacer no dependen de lo demás.
        let solo_historia = TitleInfo {
            hover: None,
            color: None,
            can_undo: true,
            can_redo: true,
            ..base()
        };
        assert_eq!(
            title_text(&solo_historia),
            "Pixoko: 64×48 · ×8 · deshacer · rehacer"
        );
    }

    #[test]
    fn sin_tamano_se_conserva_la_cadena_vacia_de_title_for() {
        // Lo que daba `title_for` sin imagen: `Pixoko: {} · {zoom}` con el tamaño vacío.
        let info = TitleInfo {
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
        let con_cursor = TitleInfo {
            hover: Some((2, 5)),
            ..info
        };
        assert_eq!(title_text(&con_cursor), "Pixoko:  · ×1 · (2, 5)");
    }

    #[test]
    fn el_titulo_anterior_se_conserva_con_cursor_y_sin_cursor() {
        // Las dos formas de `title_for` en `main.rs` antes de la unidad 6.
        let con = TitleInfo {
            color: None,
            ..base()
        };
        assert_eq!(title_text(&con), "Pixoko: 64×48 · ×8 · (3, 4)");
        let sin = TitleInfo {
            hover: None,
            color: None,
            zoom: zoom(12),
            ..base()
        };
        assert_eq!(title_text(&sin), "Pixoko: 64×48 · ×12");
    }
}

#[cfg(test)]
mod window_title_tests {
    //! Fila 22 de specs/hito-2-linea-relleno-app.md: `window_title(info)` da el título completo,
    //! con la herramienta en el campo `tool` de `TitleInfo` y `status_text(tool, color)` dentro de
    //! `title_text`. No toca los tests del módulo `tests`.

    use super::*;
    use px_ui::tool::Tool;
    use px_ui::view::Zoom;

    fn zoom(n: u32) -> Zoom {
        Zoom::from_int(n).expect("el zoom existe")
    }

    /// `info` completo: tamaño, zoom, cursor, color, y sin deshacer ni rehacer.
    fn completo() -> TitleInfo {
        TitleInfo {
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
    fn fila_22_titulo_completo_con_la_linea() {
        let info = TitleInfo {
            can_undo: true,
            can_redo: true,
            ..completo()
        };
        assert_eq!(
            window_title(&info),
            "Pixoko: 64×48 · ×8 · (3, 4) · línea · color 3 #ff0000 · deshacer · rehacer"
        );
    }

    #[test]
    fn fila_22_sin_color_solo_sale_la_herramienta() {
        let mut sin_cursor = TitleInfo {
            hover: None,
            color: None,
            ..completo()
        };
        sin_cursor.tool = Some(Tool::Pencil);
        assert_eq!(window_title(&sin_cursor), "Pixoko: 64×48 · ×8 · lápiz");
        sin_cursor.tool = Some(Tool::Line);
        assert_eq!(window_title(&sin_cursor), "Pixoko: 64×48 · ×8 · línea");
        sin_cursor.tool = Some(Tool::Fill);
        assert_eq!(window_title(&sin_cursor), "Pixoko: 64×48 · ×8 · relleno");
        let con_cursor = TitleInfo {
            tool: Some(Tool::Pencil),
            color: None,
            ..completo()
        };
        assert_eq!(
            window_title(&con_cursor),
            "Pixoko: 64×48 · ×8 · (3, 4) · lápiz"
        );
    }

    #[test]
    fn fila_22_deshacer_y_rehacer_aparecen_solo_si_se_puede() {
        let casos = [
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
        for (can_undo, can_redo, esperado) in casos {
            let info = TitleInfo {
                can_undo,
                can_redo,
                ..completo()
            };
            assert_eq!(
                window_title(&info),
                esperado,
                "undo {can_undo}, redo {can_redo}"
            );
        }
    }

    #[test]
    fn fila_22_con_error_el_titulo_es_pixoko_y_el_error_y_nada_mas() {
        for tool in [Tool::Pencil, Tool::Line, Tool::Fill] {
            for (can_undo, can_redo) in [(false, false), (true, false), (false, true), (true, true)]
            {
                let info = TitleInfo {
                    tool: Some(tool),
                    can_undo,
                    can_redo,
                    error: Some("no hay memoria para guardar el paso en la historia".to_string()),
                    ..completo()
                };
                assert_eq!(
                    window_title(&info),
                    "Pixoko: no hay memoria para guardar el paso en la historia",
                    "{tool:?}, undo {can_undo}, redo {can_redo}"
                );
            }
        }
    }

    #[test]
    fn el_relleno_con_la_ranura_transparente_y_la_herramienta_van_juntos() {
        let info = TitleInfo {
            tool: Some(Tool::Fill),
            color: Some("color 9 transparente".to_string()),
            ..completo()
        };
        assert_eq!(
            window_title(&info),
            "Pixoko: 64×48 · ×8 · (3, 4) · relleno · color 9 transparente"
        );
    }

    #[test]
    fn sin_tamano_se_conserva_la_cadena_vacia_y_se_anade_la_herramienta() {
        let info = TitleInfo {
            size: None,
            zoom: Zoom::default(),
            hover: None,
            tool: Some(Tool::Pencil),
            color: None,
            can_undo: false,
            can_redo: false,
            error: None,
        };
        assert_eq!(window_title(&info), "Pixoko:  · ×1 · lápiz");
    }
}

#[cfg(test)]
mod file_title_tests {
    //! Fila 21 de specs/hito-2-abrir-guardar.md (parte `title_with_file`): el nombre del fichero
    //! y el asterisco de modificado van delante del título normal. Sin fichero y sin cambios, el
    //! título no cambia; sin fichero y con cambios, el nombre es `sin título`.

    use super::*;

    /// Un título cualquiera (`T` de la spec).
    const T: &str = "Pixoko: 16×16 · ×8";

    #[test]
    fn fila_21_sin_fichero_y_sin_cambios_es_el_titulo_de_hoy() {
        assert_eq!(title_with_file(None, false, T), T);
    }

    #[test]
    fn fila_21_con_fichero_sin_cambios() {
        assert_eq!(
            title_with_file(Some("a.png"), false, T),
            format!("a.png — {T}")
        );
    }

    #[test]
    fn fila_21_con_fichero_y_cambios_lleva_asterisco() {
        assert_eq!(
            title_with_file(Some("a.png"), true, T),
            format!("a.png* — {T}")
        );
    }

    #[test]
    fn fila_21_sin_fichero_y_con_cambios_es_sin_titulo() {
        assert_eq!(title_with_file(None, true, T), format!("sin título* — {T}"));
    }

    #[test]
    fn el_titulo_con_error_tambien_lleva_el_nombre() {
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
    fn completo() -> TitleInfo {
        TitleInfo {
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
    fn comprueba_titulo(info: &TitleInfo, linea: &str) {
        assert_eq!(status_line(info), linea);
        let titulo = window_title(info);
        assert_eq!(titulo, format!("Pixoko: {linea}"));
        assert_eq!(
            title_with_file(Some("casa.png"), true, &titulo),
            format!("casa.png* — Pixoko: {linea}")
        );
        assert_eq!(
            title_with_file(Some("casa.png"), false, &titulo),
            format!("casa.png — Pixoko: {linea}")
        );
        assert_eq!(
            title_with_file(None, true, &titulo),
            format!("sin título* — Pixoko: {linea}")
        );
        assert_eq!(title_with_file(None, false, &titulo), titulo);
    }

    #[test]
    fn fila_10_la_linea_de_estado_completa() {
        comprueba_titulo(
            &completo(),
            "64×48 · ×8 · (3, 4) · línea · color 3 #ff0000 · deshacer · rehacer",
        );
    }

    #[test]
    fn fila_10_sin_color_solo_sale_la_herramienta() {
        let info = TitleInfo {
            color: None,
            ..completo()
        };
        comprueba_titulo(&info, "64×48 · ×8 · (3, 4) · línea · deshacer · rehacer");
    }

    #[test]
    fn fila_10_sin_cursor_no_sale_la_posicion() {
        let info = TitleInfo {
            hover: None,
            ..completo()
        };
        comprueba_titulo(
            &info,
            "64×48 · ×8 · línea · color 3 #ff0000 · deshacer · rehacer",
        );
    }

    #[test]
    fn fila_10_con_error_solo_sale_el_error() {
        let info = TitleInfo {
            error: Some("no hay memoria para guardar el paso en la historia".to_string()),
            ..completo()
        };
        comprueba_titulo(&info, "no hay memoria para guardar el paso en la historia");
    }

    #[test]
    fn fila_10_cada_herramienta_pone_su_etiqueta() {
        for (tool, etiqueta) in [
            (Tool::Pencil, "lápiz"),
            (Tool::Line, "línea"),
            (Tool::Fill, "relleno"),
        ] {
            let info = TitleInfo {
                tool: Some(tool),
                can_undo: false,
                can_redo: false,
                ..completo()
            };
            comprueba_titulo(
                &info,
                &format!("64×48 · ×8 · (3, 4) · {etiqueta} · color 3 #ff0000"),
            );
        }
    }

    #[test]
    fn fila_10_sin_tamano_se_conserva_la_cadena_vacia_del_titulo() {
        // `window_title` da `Pixoko:  · ×1 · lápiz` (tests de la unidad 7) y sale idéntico:
        // la línea de estado es ese título sin el prefijo `Pixoko: `.
        let info = TitleInfo {
            size: None,
            zoom: Zoom::default(),
            hover: None,
            tool: Some(Tool::Pencil),
            color: None,
            can_undo: false,
            can_redo: false,
            error: None,
        };
        assert_eq!(window_title(&info), "Pixoko:  · ×1 · lápiz");
        assert_eq!(status_line(&info), " · ×1 · lápiz");
    }

    // -----------------------------------------------------------------------------------------
    // Propiedad 5
    // -----------------------------------------------------------------------------------------

    fn herramienta() -> impl Strategy<Value = Tool> {
        prop::sample::select(vec![Tool::Pencil, Tool::Line, Tool::Fill])
    }

    /// Los zoom enteros posibles, y el de por defecto.
    fn zoom_cualquiera() -> impl Strategy<Value = Zoom> {
        let mut todos: Vec<Zoom> = (1..=64).filter_map(Zoom::from_int).collect();
        todos.push(Zoom::default());
        prop::sample::select(todos)
    }

    /// Los textos de color que produce `slot_label`: `color 3 #ff0000`, `color 2 índice 1`,
    /// `color 4 gris 96` y `color 9 transparente`.
    fn color_cualquiera() -> impl Strategy<Value = Option<String>> {
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
        fn propiedad_5_el_titulo_es_pixoko_mas_la_linea_de_estado(
            tool in herramienta(),
            size in prop::option::of((0_u32..=16384, 0_u32..=16384)),
            zoom in zoom_cualquiera(),
            hover in prop::option::of((0_u32..=16384, 0_u32..=16384)),
            color in color_cualquiera(),
            can_undo in any::<bool>(),
            can_redo in any::<bool>(),
        ) {
            let info = TitleInfo { size, zoom, hover, tool: Some(tool), color, can_undo, can_redo, error: None };
            let linea = status_line(&info);
            prop_assert_eq!(window_title(&info), format!("Pixoko: {linea}"));
            prop_assert!(!linea.contains('\n') && !linea.contains('\r'), "{:?}", linea);
            for c in linea.chars() {
                prop_assert!(
                    INTERFACE_CHARSET.contains(c),
                    "{:?} (U+{:04X}) no está en INTERFACE_CHARSET: {:?}",
                    c,
                    c as u32,
                    linea
                );
            }
        }
    }
}

#[cfg(test)]
mod tool_field_tests {
    //! Unidad 14-limpieza (specs/hito-2-limpieza-api.md, filas 1 a 8 y propiedades P1 y P2):
    //! `TitleInfo` tiene un campo propio `tool: Option<Tool>`; `title_text` escribe la herramienta
    //! (entre el cursor y deshacer) solo si es `Some`; `window_title(&info)` y `status_line(&info)`
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
    const HERRAMIENTAS: [(Tool, &str); 5] = [
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
    fn fila_1_sin_herramienta_no_sale_la_herramienta() {
        assert_eq!(
            title_text(&base()),
            "Pixoko: 64×48 · ×8 · (3, 4) · color 3 #ff0000"
        );
    }

    #[test]
    fn fila_2_con_la_linea_sale_antes_del_color() {
        let info = TitleInfo {
            tool: Some(Tool::Line),
            ..base()
        };
        assert_eq!(
            title_text(&info),
            "Pixoko: 64×48 · ×8 · (3, 4) · línea · color 3 #ff0000"
        );
    }

    #[test]
    fn fila_3_con_la_linea_y_sin_color_solo_sale_la_herramienta() {
        let info = TitleInfo {
            tool: Some(Tool::Line),
            color: None,
            ..base()
        };
        assert_eq!(title_text(&info), "Pixoko: 64×48 · ×8 · (3, 4) · línea");
    }

    #[test]
    fn fila_4_las_cinco_herramientas_ponen_su_etiqueta() {
        let casos = [
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
        for (tool, esperado) in casos {
            let info = TitleInfo {
                hover: None,
                tool: Some(tool),
                ..base()
            };
            assert_eq!(title_text(&info), esperado, "{tool:?}");
        }
    }

    #[test]
    fn fila_5_el_borrador_con_color_transparente_y_la_historia_van_en_orden() {
        let info = TitleInfo {
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
    fn fila_6_con_error_el_titulo_es_pixoko_y_el_error_con_cualquier_herramienta() {
        let herramientas = [
            None,
            Some(Tool::Pencil),
            Some(Tool::Line),
            Some(Tool::Fill),
            Some(Tool::Dropper),
            Some(Tool::Eraser),
        ];
        for tool in herramientas {
            for (can_undo, can_redo) in [(false, false), (true, true)] {
                let info = TitleInfo {
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
    fn fila_7_window_title_y_status_line_leen_la_herramienta_del_campo() {
        let sin = base();
        assert_eq!(window_title(&sin), title_text(&sin));
        assert_eq!(
            window_title(&sin),
            "Pixoko: 64×48 · ×8 · (3, 4) · color 3 #ff0000"
        );
        assert_eq!(status_line(&sin), "64×48 · ×8 · (3, 4) · color 3 #ff0000");

        let con = TitleInfo {
            tool: Some(Tool::Line),
            ..base()
        };
        assert_eq!(window_title(&con), title_text(&con));
        assert_eq!(
            window_title(&con),
            "Pixoko: 64×48 · ×8 · (3, 4) · línea · color 3 #ff0000"
        );
        assert_eq!(
            status_line(&con),
            "64×48 · ×8 · (3, 4) · línea · color 3 #ff0000"
        );
    }

    #[test]
    fn fila_8_app_title_info_siempre_pone_la_herramienta() {
        let imagen = Image::new(64, 48, Mode::Rgba).expect("la imagen cabe");
        let canvas = CanvasInfo {
            zoom: zoom(8),
            hover: Some((3, 4)),
            image_rect: None,
            error: None,
        };
        for (tool, etiqueta) in HERRAMIENTAS {
            for image in [Some(&imagen), None] {
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
                let titulo = window_title(&info);
                assert!(
                    titulo.contains(etiqueta),
                    "{tool:?}, imagen {}: {titulo:?}",
                    image.is_some()
                );
            }
        }
        // Guarda de texto: `main.rs` no construye el literal `TitleInfo { .. }` por otro camino
        // que `app_title_info`. Es frágil a propósito (mira el texto fuente, no el
        // comportamiento): si falla, hay que mirar por qué antes de tocar la prueba.
        assert!(!include_str!("main.rs").contains("TitleInfo {"));
    }

    // -----------------------------------------------------------------------------------------
    // Propiedades
    // -----------------------------------------------------------------------------------------

    fn herramienta() -> impl Strategy<Value = Option<Tool>> {
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
    fn zoom_cualquiera() -> impl Strategy<Value = Zoom> {
        let mut todos: Vec<Zoom> = (1..=64).filter_map(Zoom::from_int).collect();
        todos.push(Zoom::default());
        prop::sample::select(todos)
    }

    /// Textos de color como los de `slot_label`, o ninguno.
    fn color_cualquiera() -> impl Strategy<Value = Option<String>> {
        prop_oneof![
            1 => Just(None),
            3 => "color [0-9]{1,3} #[0-9a-f]{6}".prop_map(Some),
            1 => "color [0-9]{1,3} índice [0-9]{1,3}".prop_map(Some),
            1 => "color [0-9]{1,3} gris [0-9]{1,3}".prop_map(Some),
            1 => "color [0-9]{1,3} transparente".prop_map(Some),
        ]
    }

    /// Un error cualquiera (o ninguno), solo con letras minúsculas y espacios.
    fn error_cualquiera() -> impl Strategy<Value = Option<String>> {
        prop_oneof![
            3 => Just(None),
            1 => "[a-z]{1,8}( [a-z]{1,8}){0,3}".prop_map(Some),
        ]
    }

    /// Oráculo de P1, escrito a mano con `format!`: la fórmula de antes del campo `tool`.
    fn oraculo(info: &TitleInfo) -> String {
        if let Some(error) = &info.error {
            return format!("Pixoko: {error}");
        }
        let tamano = match info.size {
            Some((w, h)) => format!("{w}×{h}"),
            None => String::new(),
        };
        let cursor = match info.hover {
            Some((x, y)) => format!(" · ({x}, {y})"),
            None => String::new(),
        };
        let etiqueta = info.tool.map(|tool| {
            HERRAMIENTAS
                .iter()
                .find(|(t, _)| *t == tool)
                .map(|(_, etiqueta)| *etiqueta)
                .expect("la herramienta está en la tabla")
        });
        let estado = match (etiqueta, &info.color) {
            (Some(etiqueta), Some(color)) => format!(" · {etiqueta} · {color}"),
            (Some(etiqueta), None) => format!(" · {etiqueta}"),
            (None, Some(color)) => format!(" · {color}"),
            (None, None) => String::new(),
        };
        let deshacer = if info.can_undo { " · deshacer" } else { "" };
        let rehacer = if info.can_redo { " · rehacer" } else { "" };
        format!(
            "Pixoko: {tamano} · {}{cursor}{estado}{deshacer}{rehacer}",
            info.zoom
        )
    }

    proptest! {
        #![proptest_config(ProptestConfig::with_cases(256))]

        #[test]
        fn propiedad_1_title_text_coincide_con_la_formula_anterior(
            tool in herramienta(),
            size in prop::option::of((0_u32..=16384, 0_u32..=16384)),
            zoom in zoom_cualquiera(),
            hover in prop::option::of((0_u32..=16384, 0_u32..=16384)),
            color in color_cualquiera(),
            can_undo in any::<bool>(),
            can_redo in any::<bool>(),
            error in error_cualquiera(),
        ) {
            let info = TitleInfo { size, zoom, hover, tool, color, can_undo, can_redo, error };
            prop_assert_eq!(title_text(&info), oraculo(&info));
        }

        #[test]
        fn propiedad_2_la_barra_solo_usa_caracteres_de_la_interfaz_con_las_cinco_herramientas(
            tool in herramienta(),
            size in prop::option::of((0_u32..=16384, 0_u32..=16384)),
            zoom in zoom_cualquiera(),
            hover in prop::option::of((0_u32..=16384, 0_u32..=16384)),
            color in color_cualquiera(),
            can_undo in any::<bool>(),
            can_redo in any::<bool>(),
            error in error_cualquiera(),
        ) {
            let info = TitleInfo { size, zoom, hover, tool, color, can_undo, can_redo, error };
            let linea = status_line(&info);
            prop_assert!(!linea.contains('\n') && !linea.contains('\r'), "{:?}", linea);
            for c in linea.chars() {
                prop_assert!(
                    INTERFACE_CHARSET.contains(c),
                    "{:?} (U+{:04X}) no está en INTERFACE_CHARSET: {:?}",
                    c,
                    c as u32,
                    linea
                );
            }
        }
    }
}
