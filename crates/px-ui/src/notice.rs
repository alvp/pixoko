//! Aviso temporal (por ejemplo, un error en el título) con la caducidad como función pura
//! (specs/hito-2-linea-relleno-app.md, sección 3).

/// Segundos que dura un aviso.
pub const NOTICE_SECS: f64 = 5.0;

/// Texto y su instante de creación, en segundos (el reloj lo pone quien llama).
#[derive(Clone, Debug, PartialEq)]
pub struct Notice {
    text: String,
    created: f64,
}

impl Notice {
    pub fn new(text: String, now: f64) -> Notice {
        Notice { text, created: now }
    }

    /// `Some` mientras `now` sea menor que el instante de creación más `NOTICE_SECS`.
    pub fn visible(&self, now: f64) -> Option<&str> {
        (now < self.created + NOTICE_SECS).then_some(self.text.as_str())
    }
}
