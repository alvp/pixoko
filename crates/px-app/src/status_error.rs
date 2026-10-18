//! Qué texto va a la barra de estado como aviso o error, con su prioridad
//! (specs/hito-2-editar-color.md, decisión 12).

use px_ui::edit_layout::EDITOR_HIDDEN_NOTICE;

/// Error del lienzo, aviso temporal o, si el editor de color no cabe, el aviso de «ventana baja»
/// (`EDITOR_HIDDEN_NOTICE`), en ese orden de prioridad.
pub fn status_error(
    canvas_error: Option<String>,
    notice: Option<String>,
    editor_hidden: bool,
) -> Option<String> {
    canvas_error
        .or(notice)
        .or_else(|| editor_hidden.then(|| EDITOR_HIDDEN_NOTICE.to_string()))
}

#[cfg(test)]
mod tests {
    //! Fila 18 de specs/hito-2-editar-color.md, en su parte pura.

    use super::*;

    use px_ui::edit_layout::EDITOR_HIDDEN_NOTICE;

    fn text(s: &str) -> Option<String> {
        Some(s.to_string())
    }

    #[test]
    fn el_texto_del_aviso_es_el_de_la_spec() {
        assert_eq!(EDITOR_HIDDEN_NOTICE, "ventana baja: editor oculto");
    }

    #[test]
    fn sin_error_ni_aviso_y_con_el_editor_oculto_da_el_aviso_de_ventana_baja() {
        assert_eq!(
            status_error(None, None, true),
            text("ventana baja: editor oculto")
        );
    }

    #[test]
    fn con_el_editor_visible_y_nada_mas_no_hay_texto() {
        assert_eq!(status_error(None, None, false), None);
    }

    #[test]
    fn un_aviso_gana_al_de_ventana_baja() {
        assert_eq!(
            status_error(None, text("guardado x.png"), true),
            text("guardado x.png")
        );
    }

    #[test]
    fn un_error_del_lienzo_gana_al_de_ventana_baja() {
        assert_eq!(
            status_error(text("fallo del lienzo"), None, true),
            text("fallo del lienzo")
        );
    }

    #[test]
    fn el_error_del_lienzo_gana_al_aviso() {
        assert_eq!(
            status_error(text("fallo del lienzo"), text("un aviso"), false),
            text("fallo del lienzo")
        );
        assert_eq!(
            status_error(text("fallo del lienzo"), text("un aviso"), true),
            text("fallo del lienzo")
        );
    }

    #[test]
    fn con_el_editor_visible_el_aviso_y_el_error_pasan_tal_cual() {
        assert_eq!(
            status_error(None, text("un aviso"), false),
            text("un aviso")
        );
        assert_eq!(
            status_error(text("fallo del lienzo"), None, false),
            text("fallo del lienzo")
        );
    }
}
