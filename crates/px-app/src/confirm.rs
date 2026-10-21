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
    /// Ctrl+O con cambios sin guardar (specs/hito-2-dialogos-archivo.md).
    OpenDialog,
    /// `Nueva imagen…` con cambios sin guardar (specs/hito-2-nueva-imagen.md, D6).
    NewImage,
    /// Segunda pulsación de guardar tras un diálogo que no se pudo abrir.
    SaveFallback {
        as_copy: bool,
    },
    /// `Nueva capa` o `Duplicar` en una imagen en gris, repetida en 5 s: abre `Convertir modo…`
    /// con «Gris con alfa» elegido (specs/hito-3-capas-interfaz.md, decisión 13).
    LayersToGrayAlpha,
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

    /// `true` y la vacía si la pendiente es igual a `request` y `armada ≤ now < armada + 5`; si no,
    /// `false` y no cambia nada (no arma).
    pub fn confirm_armed(&mut self, request: &Request, now: f64) -> bool {
        match &self.pending {
            Some((pending, armed))
                if pending == request && (*armed..*armed + CONFIRM_SECS).contains(&now) =>
            {
                self.pending = None;
                true
            }
            _ => false,
        }
    }
}

/// Aviso al soltar un fichero con cambios sin guardar.
pub fn open_warning(name: &str) -> String {
    format!(
        "hay cambios sin guardar: suelta {name} otra vez en {CONFIRM_SECS:.0} s para descartarlos, o guarda con Ctrl+S"
    )
}

/// Aviso al pulsar Ctrl+O con cambios sin guardar.
pub fn open_dialog_warning() -> String {
    format!(
        "hay cambios sin guardar: pulsa Ctrl+O otra vez en {CONFIRM_SECS:.0} s para descartarlos, o guarda con Ctrl+S"
    )
}

/// Aviso al elegir `Nueva imagen…` con cambios sin guardar.
pub fn new_warning() -> String {
    format!(
        "hay cambios sin guardar: elige Nueva imagen otra vez en {CONFIRM_SECS:.0} s para descartarlos, o guarda con Ctrl+S"
    )
}

/// Aviso al cerrar con cambios sin guardar.
pub fn close_warning() -> String {
    format!(
        "hay cambios sin guardar: cierra otra vez en {CONFIRM_SECS:.0} s para descartarlos, o guarda con Ctrl+S"
    )
}

/// La regla de cierre por repetición: sin cambios sin guardar se cierra a la primera; con ellos, la
/// primera petición queda armada (y no cierra) y la segunda, dentro de `CONFIRM_SECS`, cierra.
/// La usan `handle_close` (la `x` de la ventana y `Alt`+`F4`) y `Archivo → Salir` por igual.
pub fn close_allowed(modified: bool, confirm: &mut Confirm, now: f64) -> bool {
    !modified || confirm.request(Request::Close, now) == Confirmation::Confirmed
}

#[cfg(test)]
mod close_tests;
#[cfg(test)]
mod dialog_tests;
#[cfg(test)]
mod new_tests;
#[cfg(test)]
mod tests;
