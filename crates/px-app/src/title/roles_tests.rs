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
fn fila_9_la_etiqueta_lleva_el_primer_plano_y_el_fondo() {
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
fn fila_9_el_borrador_no_muestra_los_colores() {
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
fn fila_9_sin_color_o_sin_fondo_la_etiqueta_se_adapta() {
    assert_eq!(colors_label(Tool::Pencil, None, None), None);
    assert_eq!(
        colors_label(Tool::Pencil, Some(FG.to_string()), None),
        Some(FG.to_string())
    );
}

#[test]
fn fila_9_el_titulo_y_la_barra_dicen_herramienta_y_los_dos_colores() {
    let color = colors_label(Tool::Pencil, Some(FG.into()), Some(BG.into()));
    let info = info(Tool::Pencil, color);
    assert_eq!(
        status_line(&info),
        "64×48 · ×8 · (3, 4) · lápiz · color 3 #ff0000 · fondo color 9 transparente · deshacer"
    );
    assert_eq!(title_text(&info), format!("Pixoko: {}", status_line(&info)));
}

#[test]
fn fila_9_si_cabe_la_linea_ajustada_es_la_linea_entera() {
    let color = colors_label(Tool::Pencil, Some(FG.into()), Some(BG.into()));
    let info = info(Tool::Pencil, color);
    let entera = status_line(&info);
    assert_eq!(status_line_fit(&info, entera.chars().count()), entera);
    assert_eq!(status_line_fit(&info, 500), entera);
}

#[test]
fn fila_9_con_poco_ancho_se_pierde_primero_el_tamano_y_luego_el_zoom() {
    let color = colors_label(Tool::Pencil, Some(FG.into()), Some(BG.into()));
    let info = info(Tool::Pencil, color);
    let entera = status_line(&info);
    let sin_tamano =
        "×8 · (3, 4) · lápiz · color 3 #ff0000 · fondo color 9 transparente · deshacer";
    let sin_zoom = "(3, 4) · lápiz · color 3 #ff0000 · fondo color 9 transparente · deshacer";
    assert_eq!(
        status_line_fit(&info, entera.chars().count() - 1),
        sin_tamano
    );
    assert_eq!(
        status_line_fit(&info, sin_tamano.chars().count()),
        sin_tamano
    );
    assert_eq!(
        status_line_fit(&info, sin_tamano.chars().count() - 1),
        sin_zoom
    );
    assert_eq!(status_line_fit(&info, sin_zoom.chars().count()), sin_zoom);
}

#[test]
fn fila_9_nunca_se_pierde_la_herramienta_ni_los_dos_colores_ni_hay_panico() {
    for tool in [Tool::Pencil, Tool::Line, Tool::Fill, Tool::Dropper] {
        let color = colors_label(tool, Some(FG.into()), Some(BG.into()));
        let info = info(tool, color);
        for columns in (0..=200).rev() {
            let linea = status_line_fit(&info, columns);
            assert!(linea.contains(tool.label()), "{columns}: {linea}");
            assert!(linea.contains(FG), "{columns}: {linea}");
            assert!(linea.contains(&format!("fondo {BG}")), "{columns}: {linea}");
            // El tamaño solo está si el zoom también.
            if linea.contains("64×48") {
                assert!(linea.contains("×8"), "{columns}: {linea}");
            }
        }
    }
}

#[test]
fn fila_9_un_error_sustituye_a_todo_y_no_se_ajusta() {
    let mut info = info(Tool::Pencil, Some(FG.to_string()));
    info.error = Some("no se pudo".to_string());
    assert_eq!(status_line_fit(&info, 3), "no se pudo");
}
