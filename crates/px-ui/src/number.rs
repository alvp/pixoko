//! Campo numérico de los diálogos (specs/hito-2-dialogos-modales.md, decisión 4): analizador de
//! solo dígitos ASCII con un tope de cifras antes de convertir, y los pasos de `−` y `+`.
//!
//! Se miran como mucho los primeros `MAX_NUMBER_CHARS + 1` caracteres: una cadena larguísima ni se
//! recorre entera ni reserva más. Diez cifras caben en `u64`, así que la conversión no desborda.

/// Cifras que admite el campo (las de `u32::MAX`).
pub const MAX_NUMBER_CHARS: usize = 10;

/// Por qué no vale el texto de un campo numérico.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum NumberError {
    /// Texto vacío.
    Empty,
    /// Algún carácter (de los primeros 11) no es un dígito ASCII.
    NotDigit,
    /// Más de `MAX_NUMBER_CHARS` caracteres.
    TooLong,
    Below {
        min: u32,
    },
    Above {
        max: u32,
    },
}

/// Texto del campo acotado a `MAX_NUMBER_CHARS + 1` caracteres, cortando en frontera de carácter.
pub fn bounded_text(text: &str) -> String {
    text.chars().take(MAX_NUMBER_CHARS + 1).collect()
}

/// Lee el texto como un número entre `min` y `max`. Errores, en este orden: `Empty`, `NotDigit`,
/// `TooLong`, `Below`, `Above`. No se recorta nada (los espacios son `NotDigit`).
pub fn parse_number(text: &str, min: u32, max: u32) -> Result<u32, NumberError> {
    let head: Vec<char> = text.chars().take(MAX_NUMBER_CHARS + 1).collect();
    if head.is_empty() {
        return Err(NumberError::Empty);
    }
    if head.iter().any(|c| !c.is_ascii_digit()) {
        return Err(NumberError::NotDigit);
    }
    if head.len() > MAX_NUMBER_CHARS {
        return Err(NumberError::TooLong);
    }
    let value = head
        .iter()
        .fold(0u64, |acc, c| acc * 10 + u64::from(*c as u8 - b'0'));
    if value < u64::from(min) {
        return Err(NumberError::Below { min });
    }
    if value > u64::from(max) {
        return Err(NumberError::Above { max });
    }
    Ok(value as u32)
}

/// Texto del error, para el motivo en línea («escribe un número», «máximo 8192»…).
pub fn number_error_text(error: &NumberError) -> String {
    match error {
        NumberError::Empty => "escribe un número".to_string(),
        NumberError::NotDigit => "solo dígitos 0-9".to_string(),
        NumberError::TooLong => format!("máximo {MAX_NUMBER_CHARS} cifras"),
        NumberError::Below { min } => format!("mínimo {min}"),
        NumberError::Above { max } => format!("máximo {max}"),
    }
}

/// El texto tras pulsar `−` (`delta < 0`) o `+`: suma o resta 1 al valor válido sin salirse del
/// rango y sin ceros a la izquierda. Con el texto inválido o fuera de rango da `default` (recortado
/// al rango) en los dos sentidos.
pub fn step_number(text: &str, min: u32, max: u32, default: u32, delta: i8) -> String {
    match parse_number(text, min, max) {
        Ok(value) => {
            let moved = if delta < 0 {
                value.saturating_sub(1)
            } else {
                value.saturating_add(1)
            };
            moved.clamp(min, max).to_string()
        }
        Err(_) => default.clamp(min, max).to_string(),
    }
}

/// Si el botón `−` o `+` está habilitado: solo no lo está con un valor válido ya en el límite que
/// toca.
pub fn step_enabled(text: &str, min: u32, max: u32, delta: i8) -> bool {
    match parse_number(text, min, max) {
        Ok(value) if delta < 0 => value > min,
        Ok(value) => value < max,
        Err(_) => true,
    }
}
