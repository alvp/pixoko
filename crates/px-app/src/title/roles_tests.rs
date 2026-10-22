//! Unidad I.2a, fila 9 (specs/interfaz-dos-colores.md y visto bueno 5): la barra y el título
//! llevan los dos colores y, con poco ancho, la línea de estado pierde primero el tamaño y luego
//! el zoom, nunca la herramienta ni los dos colores.

use super::*;
use px_ui::view::Zoom;

fn info(tool: Tool, color: Option<String>) -> TitleInfo {
    TitleInfo {
        layers: None,
        size: Some((64, 48)),
        zoom: Zoom::from_int(8).expect("zoom"),
        hover: Some((3, 4)),
        tool: Some(tool),
        color,
        can_undo: true,
        can_redo: false,
        error: None,
    }
}

const FG: &str = "color 3 #ff0000";
const BG: &str = "color 9 transparente";

#[test]
fn row_9_the_label_carries_the_foreground_and_the_background() {
    for tool in [Tool::Pencil, Tool::Line, Tool::Fill, Tool::Dropper] {
        assert_eq!(
            colors_label(tool, Some(FG.to_string()), Some(BG.to_string())),
            Some("color 3 #ff0000 · fondo color 9 transparente".to_string()),
            "{tool:?}"
        );
    }
    assert_eq!(
        colors_label(
            Tool::Line,
            Some("índice 12 #ff8800".into()),
            Some("índice 0 #00000000".into())
        ),
        Some("índice 12 #ff8800 · fondo índice 0 #00000000".to_string())
    );
}

#[test]
fn row_9_the_eraser_does_not_show_the_colors() {
    assert_eq!(
        colors_label(
            Tool::Eraser,
            Some("transparente".into()),
            Some(BG.to_string())
        ),
        Some("transparente".to_string())
    );
}

#[test]
fn row_9_without_color_or_without_background_the_label_adapts() {
    assert_eq!(colors_label(Tool::Pencil, None, None), None);
    assert_eq!(
        colors_label(Tool::Pencil, Some(FG.to_string()), None),
        Some(FG.to_string())
    );
}

#[test]
fn row_9_the_title_and_the_bar_say_tool_and_the_two_colors() {
    let color = colors_label(Tool::Pencil, Some(FG.into()), Some(BG.into()));
    let info = info(Tool::Pencil, color);
    assert_eq!(
        status_line(&info),
        "64×48 · ×8 · (3, 4) · lápiz · color 3 #ff0000 · fondo color 9 transparente · deshacer"
    );
    assert_eq!(title_text(&info), format!("Pixoko: {}", status_line(&info)));
}

#[test]
fn row_9_if_it_fits_the_fitted_line_is_the_whole_line() {
    let color = colors_label(Tool::Pencil, Some(FG.into()), Some(BG.into()));
    let info = info(Tool::Pencil, color);
    let whole = status_line(&info);
    assert_eq!(status_line_fit(&info, whole.chars().count()), whole);
    assert_eq!(status_line_fit(&info, 500), whole);
}

#[test]
fn row_9_with_little_width_the_size_is_lost_first_and_then_the_zoom() {
    let color = colors_label(Tool::Pencil, Some(FG.into()), Some(BG.into()));
    let info = info(Tool::Pencil, color);
    let whole = status_line(&info);
    let without_size =
        "×8 · (3, 4) · lápiz · color 3 #ff0000 · fondo color 9 transparente · deshacer";
    let without_zoom = "(3, 4) · lápiz · color 3 #ff0000 · fondo color 9 transparente · deshacer";
    assert_eq!(
        status_line_fit(&info, whole.chars().count() - 1),
        without_size
    );
    assert_eq!(
        status_line_fit(&info, without_size.chars().count()),
        without_size
    );
    assert_eq!(
        status_line_fit(&info, without_size.chars().count() - 1),
        without_zoom
    );
    assert_eq!(
        status_line_fit(&info, without_zoom.chars().count()),
        without_zoom
    );
}

#[test]
fn row_9_the_tool_and_the_two_colors_are_never_lost_and_there_is_no_panic() {
    for tool in [Tool::Pencil, Tool::Line, Tool::Fill, Tool::Dropper] {
        let color = colors_label(tool, Some(FG.into()), Some(BG.into()));
        let info = info(tool, color);
        for columns in (0..=200).rev() {
            let line = status_line_fit(&info, columns);
            assert!(line.contains(tool.label()), "{columns}: {line}");
            assert!(line.contains(FG), "{columns}: {line}");
            assert!(line.contains(&format!("fondo {BG}")), "{columns}: {line}");
            // El tamaño solo está si el zoom también.
            if line.contains("64×48") {
                assert!(line.contains("×8"), "{columns}: {line}");
            }
        }
    }
}

#[test]
fn row_9_an_error_replaces_everything_and_is_not_fitted() {
    let mut info = info(Tool::Pencil, Some(FG.to_string()));
    info.error = Some("no se pudo".to_string());
    assert_eq!(status_line_fit(&info, 3), "no se pudo");
}
