//! Texto del título de la ventana, como función pura (specs/hito-2-lapiz-deshacer.md, fila 17).
//! (Sin fuente en egui todavía, el estado va en el título.)

use px_ui::view::Zoom;

/// Datos del título, ya calculados.
pub struct TitleInfo {
    pub size: Option<(u32, u32)>,
    pub zoom: Zoom,
    /// Píxel de la imagen bajo el cursor.
    pub hover: Option<(u32, u32)>,
    /// Color activo, ya con su ranura (`color 3 #ff0000`).
    pub color: Option<String>,
    pub can_undo: bool,
    pub can_redo: bool,
    /// Error que sustituye al estado.
    pub error: Option<String>,
}

/// `Pixoko: {w}×{h} · {zoom}`, y a continuación, cada uno solo si existe, ` · ({x}, {y})`,
/// ` · {color}`, ` · deshacer` y ` · rehacer`. Con error, `Pixoko: {error}` y nada más.
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
    if let Some(color) = &info.color {
        title.push_str(&format!(" · {color}"));
    }
    if info.can_undo {
        title.push_str(" · deshacer");
    }
    if info.can_redo {
        title.push_str(" · rehacer");
    }
    title
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
