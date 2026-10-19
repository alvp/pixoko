//! Licencias de terceros: el diálogo (specs/hito-2-dialogos-modales.md, decisión 9) que sustituye
//! al panel de la unidad 17, y el ajuste de línea y el recorte del desplazamiento de la zona de
//! texto de solo lectura, que son funciones puras.

use crate::dialog::{FieldId, FieldKind, FieldSpec, FormSpec, Rows};

const CLOSE_LABEL: &str = "Cerrar";

/// Ajusta `text` a `columns` caracteres por línea. Corta por el último espacio de la línea si lo
/// hay (con algo delante) y, si no, en seco. Conserva las líneas vacías. Con `columns = 0` no hay
/// sitio y da vacío.
pub fn wrap_text(text: &str, columns: usize) -> Vec<String> {
    let mut out = Vec::new();
    if columns == 0 {
        return out;
    }
    for line in text.lines() {
        let mut rest: Vec<char> = line.chars().collect();
        if rest.is_empty() {
            out.push(String::new());
            continue;
        }
        while rest.len() > columns {
            let cut = (1..=columns)
                .rev()
                .find(|&i| rest[i] == ' ' && rest[..i].iter().any(|c| *c != ' '));
            match cut {
                Some(i) => {
                    let piece: String = rest[..i].iter().collect();
                    out.push(piece.trim_end().to_string());
                    let skip = rest[i..].iter().take_while(|c| **c == ' ').count();
                    rest.drain(..i + skip);
                }
                None => {
                    out.push(rest[..columns].iter().collect());
                    rest.drain(..columns);
                }
            }
        }
        if !rest.is_empty() {
            out.push(rest.iter().collect());
        }
    }
    out
}

/// Recorta la primera línea visible a `0..=total − visibles`.
pub fn clamp_first_line(first: usize, total: usize, visible: usize) -> usize {
    first.min(total.saturating_sub(visible))
}

/// Id del único campo del diálogo de Licencias: la zona de texto.
pub const LICENSES_FIELD: FieldId = "texto";

/// El diálogo de Licencias: una zona de solo lectura que ocupa el cuerpo y un botón `Cerrar`
/// (specs/hito-2-dialogos-modales.md, decisión 9).
pub fn licenses_form(text: &str) -> FormSpec {
    FormSpec {
        title: "Licencias",
        accept: CLOSE_LABEL,
        cancel: None,
        fields: vec![FieldSpec {
            id: LICENSES_FIELD,
            label: "Texto",
            kind: FieldKind::ReadOnly {
                text: text.to_string(),
                rows: Rows::Fill,
            },
            shown_when: None,
        }],
        check: None,
    }
}
