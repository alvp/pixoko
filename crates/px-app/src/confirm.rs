//! Confirmación por repetición, con la caducidad como función pura del tiempo que da quien
//! llama (specs/hito-2-abrir-guardar.md, secciones 2 y 3).

use std::path::PathBuf;

/// Segundos que dura una confirmación armada.
pub const CONFIRM_SECS: f64 = 5.0;

/// Lo que pide confirmación.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Request {
    Open(PathBuf),
    Close,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Confirmation {
    /// Primera petición: queda armada y hay que avisar.
    Armed,
    /// Segunda petición igual dentro de `CONFIRM_SECS`: hacerlo.
    Confirmed,
}

#[derive(Clone, Debug, Default)]
pub struct Confirm {
    pending: Option<(Request, f64)>,
}

impl Confirm {
    pub fn new() -> Confirm {
        Confirm::default()
    }

    /// `Confirmed` si la petición pendiente es igual a esta y `armado ≤ now < armado + 5`; en
    /// ese caso se vacía. Si no, esta queda armada (sustituye a la anterior) y da `Armed`.
    pub fn request(&mut self, request: Request, now: f64) -> Confirmation {
        if let Some((pending, armed)) = self.pending.take()
            && pending == request
            && (armed..armed + CONFIRM_SECS).contains(&now)
        {
            return Confirmation::Confirmed;
        }
        self.pending = Some((request, now));
        Confirmation::Armed
    }

    pub fn clear(&mut self) {
        self.pending = None;
    }
}

/// Aviso al soltar un fichero con cambios sin guardar.
pub fn open_warning(name: &str) -> String {
    format!(
        "hay cambios sin guardar: suelta {name} otra vez en {CONFIRM_SECS:.0} s para descartarlos, o guarda con Ctrl+S"
    )
}

/// Aviso al cerrar con cambios sin guardar.
pub fn close_warning() -> String {
    format!(
        "hay cambios sin guardar: cierra otra vez en {CONFIRM_SECS:.0} s para descartarlos, o guarda con Ctrl+S"
    )
}

#[cfg(test)]
mod tests;
