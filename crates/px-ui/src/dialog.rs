//! Estado puro de los diálogos modales (specs/hito-2-dialogos-modales.md, decisiones 1, 3, 4 y 6):
//! una descripción del formulario (`FormSpec`), el estado de cada campo, su validez y la del
//! conjunto, y los eventos (`Event`) a los que reacciona. Sin egui: se prueba sin ventana.

use px_core::{Mode, Rgba};

use crate::actions::{Locks, can_open_dialog};
use crate::color_edit::{hex_notice, hex_text, parse_hex};
use crate::dialog_keys::{FocusTarget, initial_focus};
use crate::dialog_layout::{
    BLOCK_GAP_U, CHOICE_GAP_U, CHOICE_ROW_H_U, ERROR_H_U, FIELD_H_U, FILL_CONTENT_H_U, PAD_U,
    ZONE_COLS, ZONE_FRAME_U,
};
use crate::edit_layout::ROW_H_U;
use crate::licenses_view::wrap_text;
use crate::number::{NumberError, bounded_text, number_error_text, parse_number, step_number};
use crate::text::INTERFACE_CHARSET;

/// Identificador de un campo.
pub type FieldId = &'static str;

/// Longitud máxima del motivo del conjunto (con `error: ` caben en las 44 columnas).
pub const REASON_MAX_CHARS: usize = 37;
/// Columnas del pie: el motivo con su prefijo (`error: ` o `falta: `) nunca pasa de aquí.
pub const FOOTER_COLUMNS: usize = 44;
/// Prefijo de una lista de requisitos pendientes: sustituye a `error: ` (no es un error).
pub const MISSING_PREFIX: &str = "falta: ";
/// Prefijo de la forma con cuenta de esa lista (`faltan 3: …`).
pub const MISSING_COUNT_PREFIX: &str = "faltan ";

/// Si un motivo es una lista de requisitos pendientes (lleva su propio prefijo, no `error: `).
pub fn is_missing_list(reason: &str) -> bool {
    reason.starts_with(MISSING_PREFIX) || reason.starts_with(MISSING_COUNT_PREFIX)
}

/// Recorta un motivo para que el pie entero quepa: 37 caracteres (más `error: `) o 44 si es una
/// lista de pendientes (que lleva su propio prefijo).
fn clip_reason(text: &str) -> String {
    let max = if is_missing_list(text) {
        FOOTER_COLUMNS
    } else {
        REASON_MAX_CHARS
    };
    text.chars().take(max).collect()
}
/// Caracteres máximos del texto pendiente de un hexadecimal que se guardan.
const HEX_TEXT_MAX_CHARS: usize = 64;

/// Alto de una zona de solo lectura.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Rows {
    /// Muestra `min(filas, líneas)` filas y se desplaza por dentro si hay más.
    Fixed(usize),
    /// Ocupa todo el cuerpo del diálogo (solo vale como único campo).
    Fill,
}

#[derive(Clone, Debug, PartialEq)]
pub enum FieldKind {
    Number {
        min: u32,
        max: u32,
        default: u32,
    },
    Choice {
        options: &'static [&'static str],
        default: usize,
    },
    Check {
        default: bool,
    },
    Hex {
        mode: Mode,
        default: Rgba,
    },
    ReadOnly {
        text: String,
        rows: Rows,
    },
}

/// Un campo se muestra solo si el campo `Choice` `field` vale `choice`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ShowWhen {
    pub field: FieldId,
    pub choice: usize,
}

#[derive(Clone, Debug, PartialEq)]
pub struct FieldSpec {
    pub id: FieldId,
    pub label: &'static str,
    pub kind: FieldKind,
    pub shown_when: Option<ShowWhen>,
}

/// Comprobación cruzada del conjunto: un motivo si los valores (todos válidos) no valen.
pub type CheckFn = fn(&Values) -> Option<String>;

#[derive(Clone, Debug)]
pub struct FormSpec {
    pub title: &'static str,
    pub accept: &'static str,
    pub cancel: Option<&'static str>,
    pub fields: Vec<FieldSpec>,
    pub check: Option<CheckFn>,
}

impl PartialEq for FormSpec {
    fn eq(&self, other: &FormSpec) -> bool {
        self.title == other.title
            && self.accept == other.accept
            && self.cancel == other.cancel
            && self.fields == other.fields
            && self.check.map(|f| f as usize) == other.check.map(|f| f as usize)
    }
}

/// Por qué una descripción de formulario no vale.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SpecError {
    DuplicateId(FieldId),
    MinAboveMax(FieldId),
    DefaultOutOfRange(FieldId),
    NoOptions(FieldId),
    LabelTooLong(FieldId),
    TextOutsideCharset(&'static str),
    TitleTooLong,
    ButtonTooLong(&'static str),
    BadShownWhen(FieldId),
    FillNotAlone,
}

/// Longitud máxima del título y de los rótulos de una fila entera.
const WIDE_LABEL_MAX: usize = 44;
/// Rótulo de un numérico o un hexadecimal (a la izquierda del campo, en 98 u).
const FIELD_LABEL_MAX: usize = 12;
/// Rótulo de una casilla o de una opción (tras la marca de 10 u y el hueco).
const CHECK_LABEL_MAX: usize = 42;
/// Rótulo de un botón: 8 caracteres.
const BUTTON_LABEL_MAX: usize = 8;

fn in_charset(text: &str) -> bool {
    text.chars().all(|c| INTERFACE_CHARSET.contains(c))
}

impl FormSpec {
    /// Comprueba ids únicos, rangos, valores por defecto, longitudes de los rótulos, juego de
    /// caracteres de todo texto fijo, `shown_when` y que `Fill` esté solo.
    pub fn validate(&self) -> Result<(), SpecError> {
        let text = |t: &'static str| {
            if in_charset(t) {
                Ok(())
            } else {
                Err(SpecError::TextOutsideCharset(t))
            }
        };
        text(self.title)?;
        if self.title.chars().count() > WIDE_LABEL_MAX {
            return Err(SpecError::TitleTooLong);
        }
        for button in std::iter::once(self.accept).chain(self.cancel) {
            text(button)?;
            if button.chars().count() > BUTTON_LABEL_MAX {
                return Err(SpecError::ButtonTooLong(button));
            }
        }
        for (i, field) in self.fields.iter().enumerate() {
            if self.fields[..i].iter().any(|f| f.id == field.id) {
                return Err(SpecError::DuplicateId(field.id));
            }
            text(field.label)?;
            let label_chars = field.label.chars().count();
            let max_label = match field.kind {
                FieldKind::Number { .. } | FieldKind::Hex { .. } => FIELD_LABEL_MAX,
                FieldKind::Check { .. } => CHECK_LABEL_MAX,
                FieldKind::Choice { .. } | FieldKind::ReadOnly { .. } => WIDE_LABEL_MAX,
            };
            if label_chars > max_label {
                return Err(SpecError::LabelTooLong(field.id));
            }
            match &field.kind {
                FieldKind::Number { min, max, default } => {
                    if min > max {
                        return Err(SpecError::MinAboveMax(field.id));
                    }
                    if default < min || default > max {
                        return Err(SpecError::DefaultOutOfRange(field.id));
                    }
                }
                FieldKind::Choice { options, default } => {
                    if options.is_empty() {
                        return Err(SpecError::NoOptions(field.id));
                    }
                    if *default >= options.len() {
                        return Err(SpecError::DefaultOutOfRange(field.id));
                    }
                    for option in *options {
                        text(option)?;
                        if option.chars().count() > CHECK_LABEL_MAX {
                            return Err(SpecError::LabelTooLong(field.id));
                        }
                    }
                }
                _ => {}
            }
            if let Some(show) = &field.shown_when {
                let target = self.fields[..i].iter().find(|f| f.id == show.field);
                let valid = matches!(
                    target.map(|f| &f.kind),
                    Some(FieldKind::Choice { options, .. }) if show.choice < options.len()
                );
                if !valid {
                    return Err(SpecError::BadShownWhen(field.id));
                }
            }
        }
        let has_fill = self.fields.iter().any(|f| {
            matches!(
                f.kind,
                FieldKind::ReadOnly {
                    rows: Rows::Fill,
                    ..
                }
            )
        });
        if has_fill && self.fields.len() != 1 {
            return Err(SpecError::FillNotAlone);
        }
        Ok(())
    }
}

/// El valor de un campo.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Value {
    Number(u32),
    Choice(usize),
    Check(bool),
    Color(Rgba),
}

/// Los valores de los campos visibles que tienen valor (no las zonas de texto).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Values {
    entries: Vec<(FieldId, Value)>,
}

impl Values {
    pub fn get(&self, id: FieldId) -> Option<&Value> {
        self.entries
            .iter()
            .find(|(entry, _)| *entry == id)
            .map(|(_, value)| value)
    }

    pub fn number(&self, id: FieldId) -> Option<u32> {
        match self.get(id)? {
            Value::Number(value) => Some(*value),
            _ => None,
        }
    }

    pub fn choice(&self, id: FieldId) -> Option<usize> {
        match self.get(id)? {
            Value::Choice(value) => Some(*value),
            _ => None,
        }
    }

    pub fn check(&self, id: FieldId) -> Option<bool> {
        match self.get(id)? {
            Value::Check(value) => Some(*value),
            _ => None,
        }
    }

    pub fn color(&self, id: FieldId) -> Option<Rgba> {
        match self.get(id)? {
            Value::Color(value) => Some(*value),
            _ => None,
        }
    }
}

/// Si un campo es válido, o por qué no (sin la etiqueta).
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Validity {
    Valid,
    Invalid(String),
}

/// Tipo de diálogo (la 19 y la 20 añaden los suyos).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum DialogKind {
    Licenses,
    /// El diálogo de prueba de `PIXOKO_DIALOGO_PRUEBA=1` (specs/hito-2-dialogos-modales.md,
    /// decisión 10): no cambia el documento.
    Demo,
    /// `Archivo → Nueva imagen…` (specs/hito-2-nueva-imagen.md).
    NewImage,
    /// `Imagen → Convertir modo…` (specs/hito-2-convertir-modo.md).
    ConvertMode,
}

/// Lo que le pasa a un diálogo abierto.
#[derive(Clone, Debug, PartialEq)]
pub enum Event {
    /// El usuario cambió el texto de un campo numérico.
    Number {
        field: FieldId,
        text: String,
    },
    /// `−` (`delta < 0`) o `+`.
    Step {
        field: FieldId,
        delta: i8,
    },
    Choice {
        field: FieldId,
        index: usize,
    },
    Check {
        field: FieldId,
        on: bool,
    },
    /// El usuario cambió el texto de un hexadecimal (queda pendiente).
    Hex {
        field: FieldId,
        text: String,
    },
    /// `Intro` (`blur` falso) o pérdida del foco (`blur` verdadero) en un hexadecimal.
    HexConfirm {
        field: FieldId,
        blur: bool,
    },
    /// `Escape` en un hexadecimal: descarta el texto pendiente.
    HexRevert {
        field: FieldId,
    },
    Accept,
    Cancel,
}

/// La respuesta de un diálogo a un evento.
#[derive(Clone, Debug, PartialEq)]
pub enum Reaction {
    None,
    /// `Aceptar` con algo inválido: el motivo.
    Rejected(String),
    Accepted(Values),
    Cancelled,
}

#[derive(Clone, Debug, PartialEq)]
enum FieldState {
    Number {
        text: String,
    },
    Choice(usize),
    Check(bool),
    Hex {
        committed: Rgba,
        pending: Option<String>,
    },
    Text {
        first_line: usize,
    },
}

/// Un diálogo abierto: la descripción y el estado de cada campo.
#[derive(Clone, Debug, PartialEq)]
pub struct Dialog {
    kind: DialogKind,
    spec: FormSpec,
    states: Vec<FieldState>,
    scroll_u: i64,
    focus_taken: bool,
    /// Nota del límite de un campo numérico: su error `Above` dice `máximo N (nota)`.
    limit_notes: Vec<(FieldId, &'static str)>,
    /// Motivo externo que hace inválido el conjunto (`set_block`).
    block: Option<String>,
    /// Resumen de varios requisitos (`set_summary`): sustituye a todo lo demás en `reason()`.
    summary: Option<String>,
}

impl Dialog {
    pub fn new(kind: DialogKind, spec: FormSpec) -> Result<Dialog, SpecError> {
        spec.validate()?;
        let states = spec
            .fields
            .iter()
            .map(|field| match &field.kind {
                FieldKind::Number { default, .. } => FieldState::Number {
                    text: default.to_string(),
                },
                FieldKind::Choice { default, .. } => FieldState::Choice(*default),
                FieldKind::Check { default } => FieldState::Check(*default),
                FieldKind::Hex { default, .. } => FieldState::Hex {
                    committed: *default,
                    pending: None,
                },
                FieldKind::ReadOnly { .. } => FieldState::Text { first_line: 0 },
            })
            .collect();
        Ok(Dialog {
            kind,
            spec,
            states,
            scroll_u: 0,
            focus_taken: false,
            limit_notes: Vec::new(),
            block: None,
            summary: None,
        })
    }

    pub fn kind(&self) -> DialogKind {
        self.kind
    }

    pub fn spec(&self) -> &FormSpec {
        &self.spec
    }

    fn index_of(&self, id: FieldId) -> Option<usize> {
        self.spec.fields.iter().position(|f| f.id == id)
    }

    /// Un campo se muestra si su control (un campo anterior) vale la opción **y el control se
    /// muestra** (transitivo: un control oculto, aunque conserve un valor, oculta a sus
    /// dependientes).
    fn shown_at(&self, index: usize) -> bool {
        match &self.spec.fields[index].shown_when {
            None => true,
            Some(show) => match self.index_of(show.field) {
                Some(j) if j < index => {
                    matches!(&self.states[j], FieldState::Choice(selected) if *selected == show.choice)
                        && self.shown_at(j)
                }
                _ => false,
            },
        }
    }

    /// Si el campo existe y se muestra.
    pub fn is_shown(&self, id: FieldId) -> bool {
        self.index_of(id).is_some_and(|i| self.shown_at(i))
    }

    /// Índices de los campos visibles, en orden.
    pub(crate) fn shown_indices(&self) -> Vec<usize> {
        (0..self.spec.fields.len())
            .filter(|i| self.shown_at(*i))
            .collect()
    }

    fn field_validity(&self, index: usize) -> Validity {
        let field = &self.spec.fields[index];
        match (&field.kind, &self.states[index]) {
            (FieldKind::Number { min, max, .. }, FieldState::Number { text }) => {
                match parse_number(text, *min, *max) {
                    Ok(_) => Validity::Valid,
                    Err(error) => {
                        let mut reason = number_error_text(&error);
                        if matches!(error, NumberError::Above { .. })
                            && let Some((_, note)) =
                                self.limit_notes.iter().find(|(id, _)| *id == field.id)
                        {
                            reason = format!("{reason} ({note})");
                        }
                        Validity::Invalid(reason)
                    }
                }
            }
            (
                FieldKind::Hex { mode, .. },
                FieldState::Hex {
                    committed,
                    pending: Some(text),
                },
            ) => match parse_hex(text, *mode, *committed) {
                Ok(_) => Validity::Valid,
                Err(error) => Validity::Invalid(hex_notice(&error).to_string()),
            },
            _ => Validity::Valid,
        }
    }

    /// Si el campo es válido, o por qué no. `None` si no existe o está oculto.
    pub fn validity(&self, id: FieldId) -> Option<Validity> {
        let index = self.index_of(id)?;
        self.shown_at(index).then(|| self.field_validity(index))
    }

    /// Los valores de los campos visibles, suponiendo que todos son válidos. Un hexadecimal con
    /// texto pendiente válido da el color escrito.
    fn collect_values(&self) -> Values {
        let mut entries = Vec::new();
        for index in self.shown_indices() {
            let field = &self.spec.fields[index];
            let value = match (&field.kind, &self.states[index]) {
                (FieldKind::Number { min, max, .. }, FieldState::Number { text }) => {
                    match parse_number(text, *min, *max) {
                        Ok(value) => Value::Number(value),
                        Err(_) => continue,
                    }
                }
                (_, FieldState::Choice(selected)) => Value::Choice(*selected),
                (_, FieldState::Check(on)) => Value::Check(*on),
                (FieldKind::Hex { mode, .. }, FieldState::Hex { committed, pending }) => {
                    match pending {
                        Some(text) => match parse_hex(text, *mode, *committed) {
                            Ok(color) => Value::Color(color),
                            Err(_) => continue,
                        },
                        None => Value::Color(*committed),
                    }
                }
                _ => continue,
            };
            entries.push((field.id, value));
        }
        Values { entries }
    }

    /// El resumen, si lo hay; si no, el motivo del primer campo visible inválido
    /// (`etiqueta: motivo`) o, con todos válidos, el de la comprobación cruzada (recortado) y, por
    /// último, el motivo externo. `None` si todo vale.
    pub fn reason(&self) -> Option<String> {
        if let Some(summary) = &self.summary {
            return Some(summary.clone());
        }
        for index in self.shown_indices() {
            if let Validity::Invalid(reason) = self.field_validity(index) {
                let text = format!("{}: {}", self.spec.fields[index].label, reason);
                return Some(clip_reason(&text));
            }
        }
        if let Some(check) = self.spec.check
            && let Some(text) = check(&self.collect_values())
        {
            return Some(clip_reason(&text));
        }
        self.block.clone()
    }

    /// Los motivos de los campos visibles inválidos (`etiqueta: motivo`, en el orden de los
    /// campos) y, con todos válidos, el de la comprobación cruzada; sin recortar. No incluye el
    /// motivo externo ni el resumen.
    pub fn problems(&self) -> Vec<String> {
        let mut problems = Vec::new();
        for index in self.shown_indices() {
            if let Validity::Invalid(reason) = self.field_validity(index) {
                problems.push(format!("{}: {}", self.spec.fields[index].label, reason));
            }
        }
        if problems.is_empty()
            && let Some(check) = self.spec.check
            && let Some(text) = check(&self.collect_values())
        {
            problems.push(text);
        }
        problems
    }

    /// Pone (o quita, con `None`) un resumen de requisitos: mientras exista es el motivo del
    /// conjunto, antes que los campos, la comprobación cruzada y el motivo externo. Lo usa quien ya
    /// ha reunido esos motivos (`problems`) en una sola línea. Se recorta como el motivo externo.
    pub fn set_summary(&mut self, summary: Option<String>) {
        self.summary = summary.map(|text| clip_reason(&text));
    }

    /// El resumen, si lo hay.
    pub fn summary(&self) -> Option<&str> {
        self.summary.as_deref()
    }

    /// Pone (o quita, con `None`) un motivo externo: mientras exista, el conjunto no es válido,
    /// después de los campos y de la comprobación cruzada. Lo usa quien tiene datos que el
    /// formulario no ve (el informe de una conversión). Se recorta a 37 caracteres.
    pub fn set_block(&mut self, reason: Option<String>) {
        self.block = reason.map(|text| clip_reason(&text));
    }

    /// El motivo externo, si lo hay.
    pub fn block(&self) -> Option<&str> {
        self.block.as_deref()
    }

    pub fn is_valid(&self) -> bool {
        self.reason().is_none()
    }

    /// Los valores, solo si el conjunto es válido.
    pub fn values(&self) -> Option<Values> {
        self.is_valid().then(|| self.collect_values())
    }

    /// Texto que muestra un campo numérico o hexadecimal.
    pub fn text(&self, id: FieldId) -> Option<String> {
        match &self.states[self.index_of(id)?] {
            FieldState::Number { text } => Some(text.clone()),
            FieldState::Hex { committed, pending } => {
                Some(pending.clone().unwrap_or_else(|| hex_text(*committed)))
            }
            _ => None,
        }
    }

    /// Si el hexadecimal tiene texto escrito sin confirmar.
    pub fn hex_pending(&self, id: FieldId) -> bool {
        self.index_of(id).is_some_and(|i| {
            matches!(
                &self.states[i],
                FieldState::Hex {
                    pending: Some(_),
                    ..
                }
            )
        })
    }

    /// Si algún hexadecimal visible tiene texto pendiente.
    pub fn any_hex_pending(&self) -> bool {
        self.shown_indices().into_iter().any(|i| {
            matches!(
                &self.states[i],
                FieldState::Hex {
                    pending: Some(_),
                    ..
                }
            )
        })
    }

    /// Si una casilla está marcada, aunque el formulario no sea válido.
    pub fn checked(&self, id: FieldId) -> Option<bool> {
        match &self.states[self.index_of(id)?] {
            FieldState::Check(on) => Some(*on),
            _ => None,
        }
    }

    /// La opción elegida de un grupo, aunque el formulario no sea válido (la necesita quien
    /// traslada valores entre grupos, como `Nueva imagen`).
    pub fn chosen(&self, id: FieldId) -> Option<usize> {
        match &self.states[self.index_of(id)?] {
            FieldState::Choice(selected) => Some(*selected),
            _ => None,
        }
    }

    /// Cambia el texto de una zona de solo lectura (`content_h_u` lo recalcula). `false` si el
    /// campo no existe o no es de solo lectura.
    pub fn set_report(&mut self, id: FieldId, text: String) -> bool {
        let Some(index) = self.index_of(id) else {
            return false;
        };
        match &mut self.spec.fields[index].kind {
            FieldKind::ReadOnly { text: stored, .. } => {
                *stored = text;
                true
            }
            _ => false,
        }
    }

    /// Pone (o quita, con `None`) la nota del límite de un campo numérico: su error `Above` pasa
    /// a decir `máximo N (nota)`. `false` si el campo no existe o no es numérico.
    pub fn set_limit_note(&mut self, id: FieldId, note: Option<&'static str>) -> bool {
        let numeric = self
            .index_of(id)
            .is_some_and(|i| matches!(self.spec.fields[i].kind, FieldKind::Number { .. }));
        if !numeric {
            return false;
        }
        self.limit_notes.retain(|(field, _)| *field != id);
        if let Some(note) = note {
            self.limit_notes.push((id, note));
        }
        true
    }

    /// Primera línea visible de una zona de texto (sin recortar al total: lo hace el pintado).
    pub fn first_line(&self, id: FieldId) -> usize {
        match self.index_of(id).map(|i| &self.states[i]) {
            Some(FieldState::Text { first_line }) => *first_line,
            _ => 0,
        }
    }

    pub fn set_first_line(&mut self, id: FieldId, line: usize) {
        if let Some(index) = self.index_of(id)
            && let FieldState::Text { first_line } = &mut self.states[index]
        {
            *first_line = line;
        }
    }

    /// Desplazamiento del cuerpo, en `u` (así sobrevive a un cambio de escala).
    pub fn scroll_u(&self) -> i64 {
        self.scroll_u
    }

    pub fn set_scroll_u(&mut self, scroll: i64) {
        self.scroll_u = scroll;
    }

    /// Si el diálogo es una zona de texto que ocupa todo el cuerpo.
    pub fn is_fill(&self) -> bool {
        self.spec.fields.iter().any(|f| {
            matches!(
                f.kind,
                FieldKind::ReadOnly {
                    rows: Rows::Fill,
                    ..
                }
            )
        })
    }

    /// Alto del bloque de un campo visible, en `u`.
    pub(crate) fn block_h_u(&self, index: usize) -> i64 {
        match &self.spec.fields[index].kind {
            FieldKind::Number { .. } | FieldKind::Hex { .. } => FIELD_H_U + ERROR_H_U,
            FieldKind::Check { .. } => CHOICE_ROW_H_U,
            FieldKind::Choice { options, .. } => {
                let n = options.len() as i64;
                ROW_H_U + n * CHOICE_ROW_H_U + (n - 1).max(0) * CHOICE_GAP_U
            }
            FieldKind::ReadOnly {
                text,
                rows: Rows::Fixed(rows),
            } => {
                let lines = wrap_text(text, ZONE_COLS).len().max(1);
                ZONE_FRAME_U + ROW_H_U * (*rows).min(lines) as i64
            }
            FieldKind::ReadOnly {
                rows: Rows::Fill, ..
            } => FILL_CONTENT_H_U - 2 * PAD_U,
        }
    }

    /// Alto del contenido del cuerpo, en `u`: el relleno, los bloques visibles y 4 entre ellos.
    pub fn content_h_u(&self) -> i64 {
        if self.is_fill() {
            return FILL_CONTENT_H_U;
        }
        let blocks: Vec<i64> = self
            .shown_indices()
            .into_iter()
            .map(|i| self.block_h_u(i))
            .collect();
        let gaps = (blocks.len() as i64 - 1).max(0) * BLOCK_GAP_U;
        2 * PAD_U + blocks.iter().sum::<i64>() + gaps
    }

    /// El foco inicial, una sola vez tras `new` (después, `None`).
    pub fn take_focus_request(&mut self) -> Option<FocusTarget> {
        if self.focus_taken {
            return None;
        }
        self.focus_taken = true;
        Some(initial_focus(self))
    }

    /// Aplica un evento. `Accept` y `Cancel` dan la respuesta que cierra el diálogo; el resto,
    /// `Reaction::None` y, si el campo no existe o no es de esa clase, no cambian nada.
    pub fn event(&mut self, event: Event) -> Reaction {
        match event {
            Event::Number { field, text } => {
                if let Some(index) = self.index_of(field)
                    && let FieldState::Number { text: stored } = &mut self.states[index]
                {
                    *stored = bounded_text(&text);
                }
            }
            Event::Step { field, delta } => {
                if let Some(index) = self.index_of(field)
                    && let FieldKind::Number { min, max, default } = self.spec.fields[index].kind
                    && let FieldState::Number { text } = &mut self.states[index]
                {
                    *text = step_number(text, min, max, default, delta);
                }
            }
            Event::Choice {
                field,
                index: option,
            } => {
                if let Some(index) = self.index_of(field)
                    && let FieldKind::Choice { options, .. } = self.spec.fields[index].kind
                    && option < options.len()
                {
                    self.states[index] = FieldState::Choice(option);
                }
            }
            Event::Check { field, on } => {
                if let Some(index) = self.index_of(field)
                    && let FieldState::Check(stored) = &mut self.states[index]
                {
                    *stored = on;
                }
            }
            Event::Hex { field, text } => {
                if let Some(index) = self.index_of(field)
                    && let FieldState::Hex { pending, .. } = &mut self.states[index]
                {
                    *pending = Some(text.chars().take(HEX_TEXT_MAX_CHARS).collect());
                }
            }
            Event::HexConfirm { field, blur } => {
                if let Some(index) = self.index_of(field) {
                    self.confirm_hex(index, blur);
                }
            }
            Event::HexRevert { field } => {
                if let Some(index) = self.index_of(field)
                    && let FieldState::Hex { pending, .. } = &mut self.states[index]
                {
                    *pending = None;
                }
            }
            Event::Cancel => return Reaction::Cancelled,
            Event::Accept => {
                if let Some(reason) = self.reason() {
                    return Reaction::Rejected(reason);
                }
                for index in self.shown_indices() {
                    self.confirm_hex(index, false);
                }
                return Reaction::Accepted(self.collect_values());
            }
        }
        Reaction::None
    }

    /// Confirma o revierte el texto pendiente de un hexadecimal (regla de 11c).
    fn confirm_hex(&mut self, index: usize, blur: bool) {
        let FieldKind::Hex { mode, .. } = self.spec.fields[index].kind else {
            return;
        };
        let FieldState::Hex { committed, pending } = &mut self.states[index] else {
            return;
        };
        let Some(text) = pending.as_ref() else {
            return;
        };
        match parse_hex(text, mode, *committed) {
            Ok(color) => {
                *committed = color;
                *pending = None;
            }
            // Con `Intro` se conserva el texto (y el error, que sale de analizarlo); al perder el
            // foco se revierte al último color confirmado.
            Err(_) if blur => *pending = None,
            Err(_) => {}
        }
    }
}

/// Estado de los diálogos de la app: ninguno abierto, o uno.
#[derive(Clone, Debug, Default, PartialEq)]
pub enum Modal {
    #[default]
    Closed,
    Open(Dialog),
}

impl Modal {
    pub fn is_open(&self) -> bool {
        matches!(self, Modal::Open(_))
    }

    pub fn kind(&self) -> Option<DialogKind> {
        match self {
            Modal::Open(dialog) => Some(dialog.kind()),
            Modal::Closed => None,
        }
    }

    /// Abre un diálogo. `false` (y sin cambios) si hay un bloqueo (`can_open_dialog`), ya hay uno
    /// abierto o la descripción no vale.
    pub fn open(&mut self, kind: DialogKind, spec: FormSpec, locks: Locks) -> bool {
        if self.is_open() || !can_open_dialog(locks) {
            return false;
        }
        match Dialog::new(kind, spec) {
            Ok(dialog) => {
                *self = Modal::Open(dialog);
                true
            }
            Err(_) => false,
        }
    }

    /// Aplica un evento; `Accepted` y `Cancelled` cierran el diálogo. Cerrado, no hace nada.
    pub fn event(&mut self, event: Event) -> Reaction {
        let Modal::Open(dialog) = self else {
            return Reaction::None;
        };
        let reaction = dialog.event(event);
        if matches!(reaction, Reaction::Accepted(_) | Reaction::Cancelled) {
            *self = Modal::Closed;
        }
        reaction
    }

    /// Cierra el diálogo sin respuesta (el cierre de la ventana). `true` si había uno.
    pub fn abandon(&mut self) -> bool {
        let was_open = self.is_open();
        *self = Modal::Closed;
        was_open
    }

    /// Cierra el diálogo solo si es del tipo pedido.
    pub fn close_kind(&mut self, kind: DialogKind) -> bool {
        if self.kind() == Some(kind) {
            *self = Modal::Closed;
            true
        } else {
            false
        }
    }
}
