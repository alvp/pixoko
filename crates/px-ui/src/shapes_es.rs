//! The fixed Spanish texts of the shapes (specs/hito-4-formas.md, unit 4.4): the rows of the
//! options window and the six help entries. Only data: they are edited without touching code. The
//! limits checked by the tests are the same as for the other help files: name <= 20 characters,
//! description <= 8 lines of 42 columns and everything inside `INTERFACE_CHARSET`.

use crate::help_texts::HelpEntry;
use crate::tool::{ShapeFill, ShapeKind};

pub fn kind_text(kind: ShapeKind) -> &'static str {
    match kind {
        ShapeKind::Rectangle => "Tipo: Rectángulo",
        ShapeKind::Circle => "Tipo: Círculo",
        ShapeKind::Ellipse => "Tipo: Elipse",
        ShapeKind::Polygon => "Tipo: Polígono",
        ShapeKind::Freeform => "Tipo: Forma libre",
    }
}

pub fn fill_text(fill: ShapeFill) -> &'static str {
    match fill {
        ShapeFill::Outline => "Relleno: Contorno",
        ShapeFill::Filled => "Relleno: Solo relleno",
        ShapeFill::Both => "Relleno: Ambos",
    }
}

const fn entry(
    key: &'static str,
    name: &'static str,
    description: &'static str,
    mouse_left: Option<&'static str>,
    mouse_right: Option<&'static str>,
) -> HelpEntry {
    HelpEntry {
        key,
        name,
        description,
        mouse_left,
        mouse_right,
        options: &[],
    }
}

/// The six entries, after those of the brush.
pub(crate) const HELP_SHAPES_ES: [HelpEntry; 6] = [
    entry(
        "tool.shape",
        "Formas",
        "Dibuja rectángulos, círculos, elipses, polígonos y formas libres. El clic derecho abre su ventana: ahí eliges el tipo y si lleva solo contorno, solo relleno o ambos. El contorno usa el pincel y su espaciado; el relleno, no. Cada forma es un solo paso de deshacer.",
        Some("en el lienzo, dibuja con el primer plano"),
        Some("en el lienzo, dibuja con el fondo"),
    ),
    entry(
        "tool.rectangle",
        "Rectángulo",
        "Elige Formas con el tipo Rectángulo. Arrastra de una esquina a la opuesta. Con Mayús sale un cuadrado. Conserva si la forma lleva contorno, relleno o ambos.",
        Some("en el lienzo, dibuja con el primer plano"),
        Some("en el lienzo, dibuja con el fondo"),
    ),
    entry(
        "tool.circle",
        "Círculo",
        "Elige Formas con el tipo Círculo. Pulsa en el centro y arrastra hasta el borde: la distancia es el radio. Los círculos pequeños salen con la máscara propia de Pixoko.",
        Some("en el lienzo, dibuja con el primer plano"),
        Some("en el lienzo, dibuja con el fondo"),
    ),
    entry(
        "tool.ellipse",
        "Elipse",
        "Elige Formas con el tipo Elipse. Pulsa en el centro y arrastra: la distancia en horizontal y en vertical son los dos radios. Con Mayús sale un círculo.",
        Some("en el lienzo, dibuja con el primer plano"),
        Some("en el lienzo, dibuja con el fondo"),
    ),
    entry(
        "tool.polygon",
        "Polígono",
        "Elige Formas con el tipo Polígono. Cada clic fija una esquina; al arrastrar ves el lado que se forma. Intro cierra el polígono y lo dibuja; también cierra un clic sobre la primera esquina. Esc lo descarta. Espacio sigue moviendo el lienzo.",
        Some("en el lienzo, fija una esquina con el primer plano"),
        Some("en el lienzo, fija una esquina con el fondo"),
    ),
    entry(
        "tool.freeform",
        "Forma libre",
        "Elige Formas con el tipo Forma libre. Dibuja a mano alzada; al soltar se cierra con una recta del final al principio y se rellena si la forma lleva relleno. Si el trazo se cruza, el relleno alterna: lo que queda dentro dos veces, vacío.",
        Some("en el lienzo, dibuja con el primer plano"),
        Some("en el lienzo, dibuja con el fondo"),
    ),
];
