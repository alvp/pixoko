//! Texto de la interfaz: fuente de píxeles, escala sin desenfoque y texto que viene de fuera
//! (specs/hito-2-texto.md).
//!
//! La fuente es Departure Mono (SIL OFL 1.1, `docs/THIRD-PARTY.md`): monoespaciada, con una
//! rejilla de 11 píxeles por em y 7 de avance. Es nítida cuando mide un múltiplo entero de 11
//! píxeles **físicos**, así que el tamaño en puntos es `11·n / ppp` con `n` entero.

use std::cell::RefCell;
use std::sync::Arc;

use egui::{Context, FontData, FontDefinitions, FontFamily, FontId, FontTweak, TextStyle};

/// La fuente, sin modificar ni recortar.
pub const FONT_BYTES: &[u8] = include_bytes!("../../../assets/fonts/DepartureMono-Regular.otf");

/// Texto de su licencia (SIL OFL 1.1), literal.
pub const FONT_LICENSE: &str = include_str!("../../../assets/fonts/LICENSE");

/// Píxeles físicos por em en la escala 1.
pub const FONT_EM_PX: f32 = 11.0;

/// Mayor factor entero de escala del texto.
pub const MAX_TEXT_SCALE: u32 = 8;

/// Los 115 caracteres que puede llevar el texto fijo de la interfaz: ASCII imprimible, las
/// letras del español, `¿¡` y `×·—…`.
pub const INTERFACE_CHARSET: &str = concat!(
    " !\"#$%&'()*+,-./0123456789:;<=>?@ABCDEFGHIJKLMNOPQRSTUVWXYZ[\\]^_`abcdefghijklmnopqrstuvwxyz{|}~",
    "ñÑáéíóúÁÉÍÓÚüÜ",
    "¿¡",
    "×·—…",
);

/// Escala válida: un `ppp` que no es finito o es ≤ 0 se trata como 1.
fn sane(ppp: f32) -> f32 {
    if ppp.is_finite() && ppp > 0.0 {
        ppp
    } else {
        1.0
    }
}

/// Factor entero del texto: `ppp` redondeado al entero más cercano (el medio hacia arriba),
/// de 1 a `MAX_TEXT_SCALE`. Un `ppp` no finito o ≤ 0 da 1.
pub fn text_scale(ppp: f32) -> u32 {
    if !(ppp.is_finite() && ppp > 0.0) {
        return 1;
    }
    let rounded = (ppp + 0.5).floor();
    if rounded >= MAX_TEXT_SCALE as f32 {
        MAX_TEXT_SCALE
    } else {
        (rounded as u32).max(1)
    }
}

/// Tamaño de la fuente en puntos: `11·n / ppp` píxeles físicos enteros de rejilla.
pub fn font_size_points(ppp: f32) -> f32 {
    let ppp = sane(ppp);
    let physical = FONT_EM_PX * text_scale(ppp) as f32;
    let mut size = physical / ppp;
    // Con `f32`, `size * ppp` puede quedar un ulp por debajo de `physical` (a 1,3 da 10,999999),
    // y el rasterizador daría alfa 254 en los bordes: se sube hasta que no quede por debajo.
    while size * ppp < physical {
        size = size.next_up();
    }
    size
}

/// Tamaño de los títulos: el doble.
pub fn heading_size_points(ppp: f32) -> f32 {
    2.0 * font_size_points(ppp)
}

/// «Píxel de interfaz» en puntos: `n` píxeles físicos.
pub fn ui_unit(ppp: f32) -> f32 {
    text_scale(ppp) as f32 / sane(ppp)
}

/// Instala la fuente como la única de `Proportional` y de `Monospace`. Sin `hinting` ni
/// `subpixel_binning`: la fuente ya está en la rejilla y así los glifos son exactamente 0 o 255.
pub fn install_fonts(ctx: &Context) {
    let tweak = FontTweak {
        hinting: Some(false),
        subpixel_binning: Some(false),
        ..FontTweak::default()
    };
    let mut definitions = FontDefinitions::empty();
    definitions.font_data.insert(
        "DepartureMono".to_owned(),
        Arc::new(FontData::from_static(FONT_BYTES).tweak(tweak)),
    );
    for family in [FontFamily::Proportional, FontFamily::Monospace] {
        definitions
            .families
            .insert(family, vec!["DepartureMono".to_owned()]);
    }
    ctx.set_fonts(definitions);
}

/// Pone los estilos de texto (`Small`, `Body`, `Button`, `Monospace` y `Heading`) al tamaño que
/// toca con la escala actual de la pantalla. Solo escribe si algo cambia.
pub fn apply_scale(ctx: &Context) {
    let ppp = ctx.pixels_per_point();
    let body = font_size_points(ppp);
    let heading = heading_size_points(ppp);
    let wanted = |style: &TextStyle| match style {
        TextStyle::Heading => FontId::new(heading, FontFamily::Proportional),
        TextStyle::Monospace => FontId::new(body, FontFamily::Monospace),
        _ => FontId::new(body, FontFamily::Proportional),
    };
    let current = ctx.global_style();
    if current
        .text_styles
        .iter()
        .all(|(style, font)| *font == wanted(style))
    {
        return;
    }
    ctx.all_styles_mut(|style| {
        for (kind, font) in style.text_styles.iter_mut() {
            *font = wanted(kind);
        }
    });
}

/// Marcas que no se dibujan o que se combinan con el carácter anterior: bidireccionales y de
/// formato, selectores de variación y marcas combinantes.
fn is_invisible_or_combining(c: char) -> bool {
    matches!(
        u32::from(c),
        0x00AD
            | 0x061C
            | 0x200B..=0x200F
            | 0x202A..=0x202E
            | 0x2060..=0x206F
            | 0xFEFF
            | 0xFFF9..=0xFFFB
            | 0xFE00..=0xFE0F
            | 0xE0100..=0xE01EF
            | 0x0300..=0x036F
            | 0x1AB0..=0x1AFF
            | 0x1DC0..=0x1DFF
            | 0x20D0..=0x20FF
            | 0xFE20..=0xFE2F
    )
}

/// Sustituye lo que la fuente no puede dibujar en texto que viene de fuera: un espacio por cada
/// carácter de control (y por U+2028 y U+2029) y un `?` por cada carácter invisible, combinante
/// o sin glifo. Da siempre el mismo número de caracteres.
pub fn replace_missing(text: &str, has_glyph: impl Fn(char) -> bool) -> String {
    text.chars()
        .map(|c| {
            if c.is_control() || c == '\u{2028}' || c == '\u{2029}' {
                ' '
            } else if is_invisible_or_combining(c) || !has_glyph(c) {
                '?'
            } else {
                c
            }
        })
        .collect()
}

/// Recorta `text` a `max_chars` caracteres. Si no cabe, termina en `…` (solo si la fuente tiene
/// ese glifo) o en `...`; con menos sitio que el sufijo, los primeros caracteres de él. Solo
/// lee `max_chars + 1` caracteres.
pub fn fit_text(text: &str, max_chars: usize, has_glyph: impl Fn(char) -> bool) -> String {
    let ellipsis = if has_glyph('…') { "…" } else { "..." };
    let ellipsis_len = ellipsis.chars().count();
    let mut chars = text.chars();
    let head: Vec<char> = chars.by_ref().take(max_chars).collect();
    if chars.next().is_none() {
        return head.into_iter().collect();
    }
    if max_chars < ellipsis_len {
        return ellipsis.chars().take(max_chars).collect();
    }
    let mut out: String = head[..max_chars - ellipsis_len].iter().collect();
    out.push_str(ellipsis);
    out
}

/// Fuente con la que se pregunta por los glifos: la del cuerpo del texto a la escala actual.
fn body_font(ctx: &Context) -> FontId {
    FontId::new(
        font_size_points(ctx.pixels_per_point()),
        FontFamily::Proportional,
    )
}

/// Predicado «la fuente tiene este glifo» con las fuentes de `ctx`, para `replace_missing` y
/// `fit_text`.
pub fn with_glyph_check<R>(ctx: &Context, f: impl FnOnce(&dyn Fn(char) -> bool) -> R) -> R {
    let font = body_font(ctx);
    ctx.fonts_mut(|fonts| {
        let fonts = RefCell::new(fonts);
        f(&|c| fonts.borrow_mut().has_glyph(&font, c))
    })
}

/// `replace_missing` con las fuentes de `ctx`.
pub fn printable(ctx: &Context, text: &str) -> String {
    with_glyph_check(ctx, |has| replace_missing(text, has))
}
