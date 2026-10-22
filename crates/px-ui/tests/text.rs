//! Tests del texto dentro de la ventana (specs/hito-2-texto.md: filas 1 a 9, 14 y 15 de «Casos
//! límite» y propiedades 1 a 4): escala sin desenfoque, fuente Departure Mono, glifo ausente,
//! texto que viene de fuera (`replace_missing`) y truncado propio (`fit_text`).
//!
//! Un `egui::Context` ejecuta pases con `RawInput` sintético, sin GPU ni pantalla. Los cambios
//! de `set_fonts`, de `set_pixels_per_point` y de la política de glifo ausente se aplican **al
//! empezar el pase siguiente**, así que el banco hace pases de calentamiento antes de medir.
//! `has_glyph`, `layout_no_wrap` y `tessellate` salen del propio contexto, con `install_fonts` y
//! `apply_scale` aplicados.
//!
//! El banco de nitidez (fila 7 y propiedad 2) reúne los cambios de textura de cada pase para
//! reconstruir el atlas de fuentes (solo el alfa), tesela el texto y mide, glifo a glifo, si los
//! vértices caen en píxeles físicos, si las uv caen en texels y si el alfa bajo cada glifo es
//! 0 o 255. El `sha256` de la fila 8 está escrito aquí mismo (sin dependencias nuevas) y pasa
//! antes sus vectores conocidos.

use std::collections::HashMap;
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::sync::OnceLock;

use egui::epaint::Primitive;
use egui::{
    Align2, Color32, Context, FontFamily, FontId, ImageData, LayerId, MissingGlyphPolicy, Pos2,
    RawInput, Rect, TextStyle, TextureId, Ui, pos2, vec2,
};
use proptest::prelude::*;
use px_ui::text::{
    FONT_BYTES, FONT_EM_PX, FONT_LICENSE, INTERFACE_CHARSET, MAX_TEXT_SCALE, apply_scale, fit_text,
    font_size_points, heading_size_points, install_fonts, printable, replace_missing, text_scale,
    ui_unit,
};
use px_ui::theme::NOTICE_PREFIX;
use px_ui::tool::{Tool, status_text};
use px_ui::view::Zoom;

/// Pantalla sintética: ancha, para que el texto de 115 caracteres a escala 3 no se corte.
const TEST_SCREEN: egui::Vec2 = vec2(3000.0, 400.0);

// ---------------------------------------------------------------------------------------------
// Banco
// ---------------------------------------------------------------------------------------------

/// Canal alfa del atlas de fuentes, reconstruido a partir de los cambios de textura.
struct Atlas {
    width_2: usize,
    height_2: usize,
    alpha: Vec<u8>,
}

struct Bench {
    ctx: Context,
    atlas: HashMap<TextureId, Atlas>,
}

impl Bench {
    /// Contexto con `ppp`, la fuente instalada, la escala aplicada y `MissingGlyphPolicy::Panic`
    /// (como en los tests de la spec), ya calentado.
    fn new_2(ppp: f32) -> Bench {
        let ctx = Context::default();
        ctx.set_pixels_per_point(ppp);
        ctx.set_missing_glyph_policy(MissingGlyphPolicy::Panic);
        install_fonts(&ctx);
        let mut bench = Bench {
            ctx,
            atlas: HashMap::new(),
        };
        // El primer pase (vacío) activa el ppp, la política y las fuentes; `apply_scale` lee el
        // ppp ya activo; los dos siguientes dejan todo aplicado.
        bench.pass(|_| {});
        apply_scale(&bench.ctx);
        bench.pass(|_| {});
        bench.pass(|_| {});
        bench
    }

    /// Ejecuta un pase, absorbe sus cambios de textura en el atlas (egui entra en pánico si se
    /// sueltan sin aplicar) y devuelve las formas que se pintaron.
    fn pass(&mut self, mut draw: impl FnMut(&mut Ui)) -> Vec<egui::epaint::ClippedShape> {
        let input = RawInput {
            screen_rect: Some(Rect::from_min_size(Pos2::ZERO, TEST_SCREEN)),
            ..RawInput::default()
        };
        let mut output_2 = self.ctx.run_ui(input, &mut draw);
        for (id, deltas) in &output_2.textures_delta.set {
            for delta in deltas.iter() {
                let ImageData::Color(image_2) = &delta.image;
                let [w, h] = image_2.size;
                let atlas = self.atlas.entry(*id).or_insert_with(|| Atlas {
                    width_2: w,
                    height_2: h,
                    alpha: vec![0; w * h],
                });
                if delta.pos.is_none() {
                    *atlas = Atlas {
                        width_2: w,
                        height_2: h,
                        alpha: vec![0; w * h],
                    };
                }
                let [ox, oy] = delta.pos.unwrap_or([0, 0]);
                for y in 0..h {
                    for x in 0..w {
                        atlas.alpha[(oy + y) * atlas.width_2 + ox + x] =
                            image_2.pixels[y * w + x].a();
                    }
                }
            }
        }
        output_2.textures_delta.clear();
        std::mem::take(&mut output_2.shapes)
    }

    fn ppp(&self) -> f32 {
        self.ctx.pixels_per_point()
    }

    /// Tamaño en píxeles físicos de `text_2` maquetado en una línea con el tamaño de la interfaz.
    fn measure_px(&self, text_2: &str) -> (f32, f32) {
        let ppp = self.ppp();
        let source = FontId::proportional(font_size_points(ppp));
        let size_2 = self.ctx.fonts_mut(|f| {
            f.layout_no_wrap(text_2.to_string(), source, Color32::WHITE)
                .size()
        });
        (size_2.x * ppp, size_2.y * ppp)
    }
}

/// Contexto compartido solo para **consultar** glifos (`has_glyph`), a ppp 1. Nunca se le piden
/// pases: varios tests lo usan a la vez desde hilos distintos.
fn ctx_shared() -> &'static Context {
    static CTX: OnceLock<Context> = OnceLock::new();
    CTX.get_or_init(|| Bench::new_2(1.0).ctx)
}

/// `has_glyph` de la fuente instalada en `ctx` (la de `font_size_points` del ppp activo).
fn has_glyph(ctx: &Context) -> impl Fn(char) -> bool + '_ {
    let source = FontId::proportional(font_size_points(ctx.pixels_per_point()));
    move |c| ctx.fonts_mut(|f| f.has_glyph(&source, c))
}

/// Maqueta `text_2` dentro de un pase de `ctx`; `true` si no entra en pánico. Tras un pánico el
/// contexto no se debe reutilizar.
fn mockup_without_panic(ctx: &Context, text_2: &str) -> bool {
    let source = FontId::proportional(font_size_points(ctx.pixels_per_point()));
    catch_unwind(AssertUnwindSafe(|| {
        let input = RawInput {
            screen_rect: Some(Rect::from_min_size(Pos2::ZERO, TEST_SCREEN)),
            ..RawInput::default()
        };
        let output_2 = ctx.run_ui(input, |ui| {
            ui.ctx().fonts_mut(|f| {
                let _ = f.layout_no_wrap(text_2.to_string(), source.clone(), Color32::WHITE);
            });
        });
        output_2.drop_without_applying_deltas();
    }))
    .is_ok()
}

/// Resultado del banco de nitidez para un texto.
#[derive(Debug, Default)]
struct Sharpness {
    glyphs: usize,
    /// Glifos con algún vértice fuera de píxel físico (±0,001).
    outside_of_pixel: usize,
    /// Glifos con alguna uv fuera de texel (±0,01).
    uv_outside_texel: usize,
    /// Texels de atlas bajo los glifos, y de ellos los de alfa distinto de 0 y 255.
    texels: usize,
    intermediate: usize,
}

/// Pinta `text_2` en `source_2` (puntos) a `ppp`, lo tesela y mide la nitidez glifo a glifo.
fn sharpness(ppp: f32, source_2: Pos2, text_2: &str) -> Sharpness {
    let mut bench = Bench::new_2(ppp);
    let size_2 = font_size_points(ppp);
    let shapes_2 = bench.pass(|ui| {
        let _ = ui.ctx().layer_painter(LayerId::background()).text(
            source_2,
            Align2::LEFT_TOP,
            text_2,
            FontId::proportional(size_2),
            Color32::WHITE,
        );
    });
    let mut r = Sharpness::default();
    for primitive in bench.ctx.tessellate(shapes_2, ppp) {
        let Primitive::Mesh(mesh) = primitive.primitive else {
            continue;
        };
        let atlas = bench
            .atlas
            .get(&mesh.texture_id)
            .expect("el atlas de la malla está reconstruido");
        let (aw, ah) = (atlas.width_2 as f32, atlas.height_2 as f32);
        for glyph in mesh.vertices.chunks(4) {
            if glyph.len() < 4 {
                continue;
            }
            r.glyphs += 1;
            let outside =
                |c: f32, scale: f32, tol: f32| (c * scale - (c * scale).round()).abs() > tol;
            if glyph
                .iter()
                .any(|v| outside(v.pos.x, ppp, 1e-3) || outside(v.pos.y, ppp, 1e-3))
            {
                r.outside_of_pixel += 1;
            }
            if glyph
                .iter()
                .any(|v| outside(v.uv.x, aw, 1e-2) || outside(v.uv.y, ah, 1e-2))
            {
                r.uv_outside_texel += 1;
            }
            let (mut x0, mut x1, mut y0, mut y1) = (f32::MAX, f32::MIN, f32::MAX, f32::MIN);
            for v in glyph {
                x0 = x0.min(v.uv.x * aw);
                x1 = x1.max(v.uv.x * aw);
                y0 = y0.min(v.uv.y * ah);
                y1 = y1.max(v.uv.y * ah);
            }
            for y in y0.round() as usize..y1.round() as usize {
                for x in x0.round() as usize..x1.round() as usize {
                    if let Some(&alpha) = atlas.alpha.get(y * atlas.width_2 + x) {
                        r.texels += 1;
                        if alpha != 0 && alpha != 255 {
                            r.intermediate += 1;
                        }
                    }
                }
            }
        }
    }
    r
}

fn near(a: f32, b: f32, tolerance: f32) -> bool {
    (a - b).abs() <= tolerance
}

/// Tolerancia (px físicos) de las medidas de texto maquetado. La spec dice ±0,05, pero egui
/// cuantiza el tamaño del texto maquetado (`Galley`) a 1/32 de punto, y esa cuantización vale
/// hasta `ppp/64` píxeles (0,061 a 3,9×): se acepta `max(0,05; ppp/64 + 0,005)`.
fn tolerance_of_measure(ppp: f32) -> f32 {
    0.05_f32.max(ppp / 64.0 + 0.005)
}

// ---------------------------------------------------------------------------------------------
// SHA-256 (sin dependencias)
// ---------------------------------------------------------------------------------------------

/// SHA-256 de `data` en hexadecimal minúscula (FIPS 180-4).
fn sha256(data: &[u8]) -> String {
    const K: [u32; 64] = [
        0x428a2f98, 0x71374491, 0xb5c0fbcf, 0xe9b5dba5, 0x3956c25b, 0x59f111f1, 0x923f82a4,
        0xab1c5ed5, 0xd807aa98, 0x12835b01, 0x243185be, 0x550c7dc3, 0x72be5d74, 0x80deb1fe,
        0x9bdc06a7, 0xc19bf174, 0xe49b69c1, 0xefbe4786, 0x0fc19dc6, 0x240ca1cc, 0x2de92c6f,
        0x4a7484aa, 0x5cb0a9dc, 0x76f988da, 0x983e5152, 0xa831c66d, 0xb00327c8, 0xbf597fc7,
        0xc6e00bf3, 0xd5a79147, 0x06ca6351, 0x14292967, 0x27b70a85, 0x2e1b2138, 0x4d2c6dfc,
        0x53380d13, 0x650a7354, 0x766a0abb, 0x81c2c92e, 0x92722c85, 0xa2bfe8a1, 0xa81a664b,
        0xc24b8b70, 0xc76c51a3, 0xd192e819, 0xd6990624, 0xf40e3585, 0x106aa070, 0x19a4c116,
        0x1e376c08, 0x2748774c, 0x34b0bcb5, 0x391c0cb3, 0x4ed8aa4a, 0x5b9cca4f, 0x682e6ff3,
        0x748f82ee, 0x78a5636f, 0x84c87814, 0x8cc70208, 0x90befffa, 0xa4506ceb, 0xbef9a3f7,
        0xc67178f2,
    ];
    let mut h: [u32; 8] = [
        0x6a09e667, 0xbb67ae85, 0x3c6ef372, 0xa54ff53a, 0x510e527f, 0x9b05688c, 0x1f83d9ab,
        0x5be0cd19,
    ];
    let mut message = data.to_vec();
    message.push(0x80);
    while message.len() % 64 != 56 {
        message.push(0);
    }
    message.extend_from_slice(&((data.len() as u64) * 8).to_be_bytes());
    for block in message.as_chunks::<64>().0 {
        let mut w = [0u32; 64];
        for (i, word) in block.as_chunks::<4>().0.iter().enumerate() {
            w[i] = u32::from_be_bytes(*word);
        }
        for i in 16..64 {
            let s0 = w[i - 15].rotate_right(7) ^ w[i - 15].rotate_right(18) ^ (w[i - 15] >> 3);
            let s1 = w[i - 2].rotate_right(17) ^ w[i - 2].rotate_right(19) ^ (w[i - 2] >> 10);
            w[i] = w[i - 16]
                .wrapping_add(s0)
                .wrapping_add(w[i - 7])
                .wrapping_add(s1);
        }
        let [mut a, mut b, mut c, mut d, mut e, mut f, mut g, mut hh] = h;
        for i in 0..64 {
            let s1 = e.rotate_right(6) ^ e.rotate_right(11) ^ e.rotate_right(25);
            let ch = (e & f) ^ (!e & g);
            let t1 = hh
                .wrapping_add(s1)
                .wrapping_add(ch)
                .wrapping_add(K[i])
                .wrapping_add(w[i]);
            let s0 = a.rotate_right(2) ^ a.rotate_right(13) ^ a.rotate_right(22);
            let maj = (a & b) ^ (a & c) ^ (b & c);
            let t2 = s0.wrapping_add(maj);
            hh = g;
            g = f;
            f = e;
            e = d.wrapping_add(t1);
            d = c;
            c = b;
            b = a;
            a = t1.wrapping_add(t2);
        }
        for (state_2, new_2) in h.iter_mut().zip([a, b, c, d, e, f, g, hh]) {
            *state_2 = state_2.wrapping_add(new_2);
        }
    }
    h.iter().map(|word| format!("{word:08x}")).collect()
}

/// Lee un fichero de la raíz del repositorio (desde `crates/px-ui`).
fn reads_from_the_root(path: &str) -> Vec<u8> {
    let complete = format!("{}/../../{path}", env!("CARGO_MANIFEST_DIR"));
    std::fs::read(&complete).unwrap_or_else(|e| panic!("no se pudo leer {complete}: {e}"))
}

/// Texto de la raíz con los finales de línea normalizados a `\n` (un checkout de Windows con
/// `autocrlf` puede haberlos cambiado en los ficheros de texto).
fn reads_text_from_the_root(path: &str) -> String {
    String::from_utf8(reads_from_the_root(path))
        .expect("UTF-8")
        .replace("\r\n", "\n")
}

// ---------------------------------------------------------------------------------------------
// Fila 1: text_scale
// ---------------------------------------------------------------------------------------------

#[test]
fn row_01_text_scale_rounds_with_the_half_upward() {
    let cases: [(f32, u32); 11] = [
        (0.5, 1),
        (1.0, 1),
        (1.25, 1),
        (1.49, 1),
        (1.5, 2),
        (1.75, 2),
        (2.0, 2),
        (2.5, 3),
        (3.0, 3),
        (8.0, 8),
        (20.0, 8),
    ];
    for (ppp, expected_2) in cases {
        assert_eq!(text_scale(ppp), expected_2, "ppp {ppp}");
    }
}

#[test]
fn row_01_text_scale_of_an_invalid_ppp_is_1() {
    for ppp in [f32::NAN, 0.0, -1.0, f32::INFINITY] {
        assert_eq!(text_scale(ppp), 1, "ppp {ppp}");
    }
}

#[test]
fn row_01_the_constants_of_the_spec() {
    assert_eq!(FONT_EM_PX, 11.0);
    assert_eq!(MAX_TEXT_SCALE, 8);
}

// ---------------------------------------------------------------------------------------------
// Fila 2: tamaños en puntos
// ---------------------------------------------------------------------------------------------

#[test]
fn row_02_the_font_size_in_physical_pixels_is_a_multiple_of_11() {
    let cases: [(f32, f32); 6] = [
        (1.0, 11.0),
        (1.25, 11.0),
        (1.5, 22.0),
        (2.0, 22.0),
        (2.5, 33.0),
        (3.0, 33.0),
    ];
    for (ppp, physical) in cases {
        let body = font_size_points(ppp) * ppp;
        let title = heading_size_points(ppp) * ppp;
        assert!(near(body, physical, 1e-3), "ppp {ppp}: cuerpo {body}");
        assert!(
            near(title, 2.0 * physical, 1e-3),
            "ppp {ppp}: título {title}"
        );
    }
}

#[test]
fn row_02_the_interface_unit_is_n_physical_pixels() {
    // `u = n/ppp` puntos: con 1,5 (n = 2) son 2 píxeles físicos; con 2,5 (n = 3), 3.
    for (ppp, n) in [
        (1.0_f32, 1.0_f32),
        (1.25, 1.0),
        (1.5, 2.0),
        (2.5, 3.0),
        (3.0, 3.0),
    ] {
        assert!(near(ui_unit(ppp) * ppp, n, 1e-3), "ppp {ppp}");
    }
}

// ---------------------------------------------------------------------------------------------
// Fila 3: cobertura de INTERFACE_CHARSET
// ---------------------------------------------------------------------------------------------

#[test]
fn row_03_the_font_has_a_glyph_for_the_115_characters_of_the_interface() {
    let characters: Vec<char> = INTERFACE_CHARSET.chars().collect();
    assert_eq!(characters.len(), 115, "ASCII 95 + 14 + 2 + 4");
    let mut unique = characters.clone();
    unique.sort_unstable();
    unique.dedup();
    assert_eq!(unique.len(), 115, "sin repetidos");
    for c in ' '..='~' {
        assert!(characters.contains(&c), "falta el ASCII {c:?}");
    }
    for c in "ñÑáéíóúÁÉÍÓÚüÜ¿¡×·—…".chars() {
        assert!(characters.contains(&c), "falta {c:?}");
    }

    let ctx = ctx_shared();
    let has = has_glyph(ctx);
    for c in characters {
        assert!(has(c), "sin glifo para {c:?} (U+{:04X})", c as u32);
    }
}

// ---------------------------------------------------------------------------------------------
// Fila 4: glifo ausente y replace_missing
// ---------------------------------------------------------------------------------------------

#[test]
fn row_04_japanese_check_and_emoji_have_no_glyph_and_the_ellipsis_does() {
    // La spec ponía también `→`, pero la cmap de Departure Mono 1.500 sí tiene U+2192 (se
    // comprobó leyendo la tabla del fichero fijado por SHA-256): se usa `✓` (U+2713), que no.
    let ctx = ctx_shared();
    let has = has_glyph(ctx);
    assert!(!has('日'));
    assert!(!has('✓'));
    assert!(!has('😀'));
    assert!(has('…'));
}

#[test]
fn row_04_replace_missing_changes_only_what_cannot_be_drawn() {
    let ctx = ctx_shared();
    let has = has_glyph(ctx);
    assert_eq!(replace_missing("a日b✓c", &has), "a?b?c");
    assert_eq!(replace_missing("a\0b\x1bc\r\n\u{2028}", &has), "a b c   ");
    assert_eq!(replace_missing("", &has), "");
    assert_eq!(replace_missing("straße €", &has), "straße €");
}

#[test]
fn row_04_u2029_is_also_a_space() {
    let ctx = ctx_shared();
    let has = has_glyph(ctx);
    assert_eq!(replace_missing("a\u{2029}b", &has), "a b");
}

#[test]
fn row_04_printable_is_replace_missing_with_the_fonts_of_the_context() {
    let ctx = ctx_shared();
    let has = has_glyph(ctx);
    for text_2 in [
        "a日b✓c",
        "a\0b\x1bc\r\n\u{2028}",
        "",
        "straße €",
        "e\u{301}",
    ] {
        assert_eq!(printable(ctx, text_2), replace_missing(text_2, &has));
    }
}

// ---------------------------------------------------------------------------------------------
// Fila 5: la guardia de glifo ausente
// ---------------------------------------------------------------------------------------------

/// Cadenas de texto fijo de la interfaz: lo que `px-app` pone en la barra, reconstruido con los
/// mismos trozos que `status_line` (la función en sí se prueba en `px-app`, con la propiedad 5,
/// que exige que solo use `INTERFACE_CHARSET`).
fn texts_fixed() -> Vec<String> {
    let mut texts = vec![
        "Pixoko".to_string(),
        "Pixoko: ".to_string(),
        NOTICE_PREFIX.to_string(),
        INTERFACE_CHARSET.to_string(),
    ];
    for tool_2 in [Tool::Pencil, Tool::Line, Tool::Fill] {
        texts.push(tool_2.label().to_string());
        let zoom = Zoom::from_int(8).expect("el zoom existe");
        for cursor in [None, Some((3, 4))] {
            for color in [None, Some("color 3 #ff0000"), Some("color 9 transparente")] {
                for (undo, redo) in [(false, false), (true, false), (false, true), (true, true)] {
                    let mut line = format!("64×48 · {zoom}");
                    if let Some((x, y)) = cursor {
                        line.push_str(&format!(" · ({x}, {y})"));
                    }
                    line.push_str(&format!(" · {}", status_text(tool_2, color)));
                    if undo {
                        line.push_str(" · deshacer");
                    }
                    if redo {
                        line.push_str(" · rehacer");
                    }
                    texts.push(line);
                }
            }
        }
    }
    texts
}

#[test]
fn row_05_the_fixed_text_is_laid_out_without_panic_with_the_panic_policy() {
    for ppp in [1.0_f32, 1.5, 2.0] {
        let bench = Bench::new_2(ppp);
        for text_2 in texts_fixed() {
            assert!(
                mockup_without_panic(&bench.ctx, &text_2),
                "ppp {ppp}: el texto fijo {text_2:?} entra en pánico"
            );
        }
    }
}

#[test]
fn row_05_a_character_without_a_glyph_panics_with_the_panic_policy() {
    let bench = Bench::new_2(1.0);
    assert!(
        !mockup_without_panic(&bench.ctx, "a日b"),
        "la guardia no funciona: «日» se maquetó sin pánico"
    );
}

// ---------------------------------------------------------------------------------------------
// Fila 6: medidas
// ---------------------------------------------------------------------------------------------

#[test]
fn row_06_pixoko_measures_7n_per_character_and_14n_in_height() {
    let cases: [(f32, (f32, f32)); 4] = [
        (1.0, (42.0, 14.0)),
        (1.5, (84.0, 28.0)),
        (2.0, (84.0, 28.0)),
        (3.0, (126.0, 42.0)),
    ];
    for (ppp, (width_2, height_2)) in cases {
        let bench = Bench::new_2(ppp);
        let (w, h) = bench.measure_px("Pixoko");
        assert!(
            near(w, width_2, 0.05) && near(h, height_2, 0.05),
            "ppp {ppp}: {w} × {h}, se esperaba {width_2} × {height_2}"
        );
    }
}

#[test]
fn row_06_the_115_characters_in_a_row_at_1_5_measure_1610_px_wide() {
    let bench = Bench::new_2(1.5);
    let (w, h) = bench.measure_px(INTERFACE_CHARSET);
    assert!(near(w, 805.0 * 2.0, 0.05), "ancho {w}");
    assert!(near(h, 28.0, 0.05), "alto {h}");
}

// ---------------------------------------------------------------------------------------------
// Fila 7: nitidez geométrica
// ---------------------------------------------------------------------------------------------

#[test]
fn row_07_no_glyph_falls_outside_the_pixel_or_has_intermediate_texels() {
    let text_2 = "Pixoko: 64×48 · ×8 · (12, 7) · lápiz ñÑ ¿Qué?";
    let with_ink = text_2.chars().filter(|c| !c.is_whitespace()).count();
    for ppp in [1.0_f32, 1.25, 1.5, 1.75, 2.0, 2.5, 3.0] {
        let r = sharpness(ppp, pos2(3.37, 5.11), text_2);
        assert!(
            r.glyphs >= with_ink,
            "ppp {ppp}: {} glifos para {with_ink} caracteres con tinta",
            r.glyphs
        );
        assert!(r.texels > 0, "ppp {ppp}: no se midió ningún texel");
        assert_eq!(
            r.outside_of_pixel, 0,
            "ppp {ppp}: glifos fuera de píxel físico"
        );
        assert_eq!(r.uv_outside_texel, 0, "ppp {ppp}: uv fuera de texel");
        assert_eq!(r.intermediate, 0, "ppp {ppp}: texels de alfa intermedio");
    }
}

// ---------------------------------------------------------------------------------------------
// Fila 8: fuente, licencia, THIRD-PARTY.md y .gitattributes
// ---------------------------------------------------------------------------------------------

const SHA_SOURCE: &str = "4d53f663155cf8bf7ffc8e688776e719625f7bbb80a8d90073438b249261a2e0";
const SHA_LICENSE: &str = "b65e42750f3cb65437a97f4dfd781c58f199259949db2341012be60e32500e83";
const COPYRIGHT: &str = "Copyright 2022–2024 Helena Zhang (helenazhang.com).";

#[test]
fn row_08_the_test_sha256_passes_its_vectors() {
    assert_eq!(
        sha256(b""),
        "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
    );
    assert_eq!(
        sha256(b"abc"),
        "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
    );
    // Dos bloques (el relleno cruza el límite de 64 bytes).
    assert_eq!(
        sha256(b"abcdbcdecdefdefgefghfghighijhijkijkljklmklmnlmnomnopnopq"),
        "248d6a61d20638b8e5c026930c3e6039a33ce45964ff2167f6ecedd419db06c1"
    );
    // Un millón de «a».
    assert_eq!(
        sha256(&vec![b'a'; 1_000_000]),
        "cdc76e5c9914fb9281a1c7e284d73e67f1809a48a497200e046d39ccc7112cd0"
    );
}

#[test]
fn row_08_install_fonts_installs_a_single_font_departure_mono() {
    let bench = Bench::new_2(1.0);
    let definitions = bench.ctx.fonts(|f| f.definitions().clone());
    assert_eq!(definitions.font_data.len(), 1, "una sola fuente");
    let data = definitions
        .font_data
        .get("DepartureMono")
        .expect("la fuente se llama DepartureMono");
    assert_eq!(data.tweak.hinting, Some(false));
    assert_eq!(data.tweak.subpixel_binning, Some(false));
    let bytes: &[u8] = (*data.font).as_ref();
    assert_eq!(bytes, FONT_BYTES, "es la fuente sin modificar");
    for family in [FontFamily::Proportional, FontFamily::Monospace] {
        assert_eq!(
            definitions.families.get(&family),
            Some(&vec!["DepartureMono".to_string()]),
            "{family:?}: solo DepartureMono, sin fuentes de reserva"
        );
    }
}

#[test]
fn row_08_the_embedded_font_is_the_official_file() {
    let file = reads_from_the_root("assets/fonts/DepartureMono-Regular.otf");
    assert_eq!(file.as_slice(), FONT_BYTES, "FONT_BYTES es el fichero");
    assert_eq!(FONT_BYTES.len(), 84_480);
    assert_eq!(&FONT_BYTES[..4], b"OTTO");
    assert_eq!(sha256(&file), SHA_SOURCE);
    assert_eq!(sha256(FONT_BYTES), SHA_SOURCE);
}

#[test]
fn row_08_the_embedded_license_is_the_ofl_of_the_font() {
    let file = reads_from_the_root("assets/fonts/LICENSE");
    assert_eq!(
        file.as_slice(),
        FONT_LICENSE.as_bytes(),
        "FONT_LICENSE es el fichero"
    );
    assert_eq!(sha256(&file), SHA_LICENSE);
    assert_eq!(sha256(FONT_LICENSE.as_bytes()), SHA_LICENSE);
    assert!(FONT_LICENSE.starts_with(COPYRIGHT));
}

#[test]
fn row_08_third_party_documents_the_font_and_its_license() {
    let doc = reads_text_from_the_root("docs/THIRD-PARTY.md");
    for part in [
        "Departure Mono",
        "1.500",
        "https://github.com/rektdeckard/departure-mono",
        "public/assets/LICENSE",
        "OFL-1.1",
        COPYRIGHT,
        SHA_SOURCE,
        SHA_LICENSE,
    ] {
        assert!(doc.contains(part), "THIRD-PARTY.md no contiene {part:?}");
    }
    assert!(
        doc.lines()
            .any(|l| l.contains("Reserved Font Name") && l.to_lowercase().contains("ninguno")),
        "THIRD-PARTY.md debe declarar que no hay Reserved Font Name (una línea con \
         «Reserved Font Name» y «Ninguno»)"
    );
    let license = FONT_LICENSE.replace("\r\n", "\n");
    assert!(
        doc.contains(license.trim_end()),
        "THIRD-PARTY.md debe contener el texto completo de FONT_LICENSE"
    );
    // El MIT de la raíz del repositorio de la fuente es el del sitio web: no se atribuye.
    assert!(!doc.contains("MIT License"));
    assert!(!doc.contains("Tobias Fried"));
}

#[test]
fn row_08_gitattributes_marks_the_font_as_binary() {
    let attributes = reads_text_from_the_root(".gitattributes");
    assert!(
        attributes
            .lines()
            .any(|l| l.trim() == "assets/fonts/** binary"),
        ".gitattributes debe contener la línea `assets/fonts/** binary`"
    );
}

// ---------------------------------------------------------------------------------------------
// Fila 9: apply_scale
// ---------------------------------------------------------------------------------------------

fn size_of_style(ctx: &Context, style: TextStyle) -> f32 {
    ctx.global_style().text_styles[&style].size
}

fn pass_empty(ctx: &Context) {
    let input = RawInput {
        screen_rect: Some(Rect::from_min_size(Pos2::ZERO, vec2(400.0, 300.0))),
        ..RawInput::default()
    };
    ctx.run_ui(input, |_| {}).drop_without_applying_deltas();
}

#[test]
fn row_09_apply_scale_follows_the_ppp_and_does_not_touch_the_style_if_it_does_not_change() {
    let ctx = Context::default();
    install_fonts(&ctx);

    // `set_pixels_per_point` se activa al empezar el pase siguiente.
    ctx.set_pixels_per_point(1.5);
    pass_empty(&ctx);
    apply_scale(&ctx);
    for style in [
        TextStyle::Small,
        TextStyle::Body,
        TextStyle::Button,
        TextStyle::Monospace,
    ] {
        assert!(
            near(size_of_style(&ctx, style.clone()), 22.0 / 1.5, 1e-3),
            "ppp 1,5: {style:?}"
        );
    }
    assert!(near(size_of_style(&ctx, TextStyle::Body), 14.667, 1e-3));
    assert!(near(size_of_style(&ctx, TextStyle::Heading), 29.333, 1e-3));

    ctx.set_pixels_per_point(2.0);
    pass_empty(&ctx);
    apply_scale(&ctx);
    for style in [
        TextStyle::Small,
        TextStyle::Body,
        TextStyle::Button,
        TextStyle::Monospace,
    ] {
        assert!(
            near(size_of_style(&ctx, style.clone()), 11.0, 1e-3),
            "ppp 2: {style:?}"
        );
    }
    assert!(near(size_of_style(&ctx, TextStyle::Heading), 22.0, 1e-3));

    // Segunda llamada seguida: el estilo ni cambia de valor ni se reescribe (con una copia del
    // `Arc` en la mano, cualquier escritura con `Arc::make_mut` lo habría clonado).
    let antes = ctx.global_style();
    apply_scale(&ctx);
    let after_2 = ctx.global_style();
    assert_eq!(*antes, *after_2);
    assert!(
        std::sync::Arc::ptr_eq(&antes, &after_2),
        "la segunda llamada reescribió el estilo"
    );
}

// ---------------------------------------------------------------------------------------------
// Fila 14: texto externo
// ---------------------------------------------------------------------------------------------

#[test]
fn row_14_the_external_text_goes_through_the_substitution() {
    let ctx = ctx_shared();
    let has = has_glyph(ctx);
    let cases: [(&str, &str); 8] = [
        ("日本語.png", "???.png"),
        ("😀 ok", "? ok"),
        ("👨\u{200D}👩\u{200D}👧", "?????"),
        ("a\u{202E}b\u{202C}c", "a?b?c"),
        ("\u{200B}x\u{FEFF}", "?x?"),
        ("e\u{301}", "e?"),
        ("n\u{303}", "n?"),
        // `ñ` precompuesta (U+00F1): igual.
        ("\u{f1}", "\u{f1}"),
    ];
    for (input_2, expected_2) in cases {
        let output_2 = replace_missing(input_2, &has);
        assert_eq!(output_2, expected_2, "entrada {input_2:?}");
        assert_eq!(
            output_2.chars().count(),
            input_2.chars().count(),
            "mismo número de caracteres: {input_2:?}"
        );
        assert!(output_2.chars().all(&has), "todos con glifo: {output_2:?}");
    }
}

#[test]
fn row_14_a_conversion_report_stays_the_same() {
    let ctx = ctx_shared();
    let has = has_glyph(ctx);
    let report = "Convertido a 16 colores: año ×2 · se perdieron 3 colores…";
    assert_eq!(replace_missing(report, &has), report);
}

#[test]
fn row_14_each_result_is_laid_out_without_panic() {
    let bench = Bench::new_2(1.0);
    let has = has_glyph(&bench.ctx);
    for input_2 in [
        "日本語.png",
        "😀 ok",
        "👨\u{200D}👩\u{200D}👧",
        "a\u{202E}b\u{202C}c",
        "\u{200B}x\u{FEFF}",
        "e\u{301}",
        "n\u{303}",
        "\u{f1}",
        "a\0b\x1bc\r\n\u{2028}",
        "Convertido a 16 colores: año ×2 · se perdieron 3 colores…",
    ] {
        let output_2 = replace_missing(input_2, &has);
        assert!(
            mockup_without_panic(&bench.ctx, &output_2),
            "{output_2:?} (de {input_2:?}) entra en pánico"
        );
    }
}

// ---------------------------------------------------------------------------------------------
// Fila 15: fit_text
// ---------------------------------------------------------------------------------------------

#[test]
fn row_15_with_the_ellipsis_in_the_font_it_ends_in_one() {
    let with = |_: char| true;
    let cases: [(usize, &str); 6] = [
        (6, "abcdef"),
        (5, "abcd…"),
        (3, "ab…"),
        (2, "a…"),
        (1, "…"),
        (0, ""),
    ];
    for (n, expected_2) in cases {
        assert_eq!(fit_text("abcdef", n, with), expected_2, "n = {n}");
    }
}

#[test]
fn row_15_without_the_ellipsis_in_the_font_it_ends_in_three_dots() {
    let without = |c: char| c != '…';
    let cases: [(usize, &str); 6] = [
        (6, "abcdef"),
        (5, "ab..."),
        (3, "..."),
        (2, ".."),
        (1, "."),
        (0, ""),
    ];
    for (n, expected_2) in cases {
        assert_eq!(fit_text("abcdef", n, without), expected_2, "n = {n}");
    }
}

#[test]
fn row_15_empty_gives_empty_and_a_very_long_name_is_clipped() {
    let with = |_: char| true;
    let without = |c: char| c != '…';
    for n in [0, 1, 40] {
        assert_eq!(fit_text("", n, with), "");
        assert_eq!(fit_text("", n, without), "");
    }
    let largo: String = (0..100_000)
        .map(|i| char::from(b'a' + (i % 26) as u8))
        .collect();
    let clipped = fit_text(&largo, 40, with);
    assert_eq!(clipped.chars().count(), 40);
    assert!(clipped.ends_with('…'));
    assert_eq!(&clipped[..39], &largo[..39]);
    let without_points = fit_text(&largo, 40, without);
    assert_eq!(without_points.chars().count(), 40);
    assert!(without_points.ends_with("..."));
}

#[test]
fn row_15_counts_characters_and_not_bytes() {
    let with = |_: char| true;
    // 6 caracteres de 2 bytes cada uno.
    assert_eq!(fit_text("ñÑáéíó", 6, with), "ñÑáéíó");
    assert_eq!(fit_text("ñÑáéíóú", 4, with), "ñÑá…");
}

// ---------------------------------------------------------------------------------------------
// Propiedades
// ---------------------------------------------------------------------------------------------

/// Cualquier `f32` (incluidos NaN, infinitos, negativos y el rango útil de ppp).
fn ppp_any() -> impl Strategy<Value = f32> {
    prop_oneof![
        4 => any::<f32>(),
        4 => 0.0_f32..24.0,
        1 => Just(f32::NAN),
        1 => Just(f32::INFINITY),
        1 => Just(f32::NEG_INFINITY),
    ]
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(512))]

    /// Propiedad 1: escala en 1 a 8.
    #[test]
    fn property_1_the_scale_is_between_1_and_8(ppp in ppp_any()) {
        let n = text_scale(ppp);
        prop_assert!((1..=MAX_TEXT_SCALE).contains(&n), "ppp {ppp}: n = {n}");
    }

    /// Propiedad 1: la escala no decrece con ppp.
    #[test]
    fn property_1_the_scale_does_not_decrease(a in -10.0_f32..40.0, b in -10.0_f32..40.0) {
        let (smaller, mayor) = if a <= b { (a, b) } else { (b, a) };
        prop_assert!(text_scale(smaller) <= text_scale(mayor), "{smaller} y {mayor}");
    }

    /// Propiedad 1: en [0,5; 8) difiere de ppp en no más de 0,5.
    #[test]
    fn property_1_the_scale_is_half_a_point_from_ppp(ppp in 0.5_f32..8.0) {
        let n = text_scale(ppp) as f32;
        prop_assert!((n - ppp).abs() <= 0.5 + 1e-6, "ppp {ppp}: n = {n}");
    }

    /// Propiedad 1: en [0,25; 16] el tamaño en píxeles físicos es 11·n.
    #[test]
    fn property_1_the_physical_size_is_11_times_n(ppp in 0.25_f32..=16.0) {
        let n = text_scale(ppp) as f32;
        let physical = font_size_points(ppp) * ppp;
        prop_assert!((physical - 11.0 * n).abs() <= 1e-3, "ppp {ppp}: {physical} y n = {n}");
        let title = heading_size_points(ppp) * ppp;
        prop_assert!((title - 22.0 * n).abs() <= 1e-3, "ppp {ppp}: título {title}");
    }
}

/// ppp múltiplo de 0,05 entre 1 y 4 (`k / 20` es exacto en `f32` para los medios).
fn ppp_of_the_property() -> impl Strategy<Value = f32> {
    (20_u32..=80).prop_map(|k| k as f32 / 20.0)
}

/// De 1 a 60 caracteres de `INTERFACE_CHARSET`.
fn interface_string() -> impl Strategy<Value = String> {
    let characters: Vec<char> = INTERFACE_CHARSET.chars().collect();
    prop::collection::vec(prop::sample::select(characters), 1..=60)
        .prop_map(|v| v.into_iter().collect::<String>())
}

proptest! {
    // Cada caso crea un contexto, instala la fuente y hace varios pases: pocos casos.
    #![proptest_config(ProptestConfig::with_cases(48))]

    /// Propiedad 2: nitidez geométrica con origen y cadena cualesquiera.
    #[test]
    fn property_2_geometric_sharpness(
        ppp in ppp_of_the_property(),
        x in 0.0_f32..200.0,
        y in 0.0_f32..200.0,
        text_2 in interface_string(),
    ) {
        let r = sharpness(ppp, pos2(x, y), &text_2);
        prop_assert_eq!(r.outside_of_pixel, 0, "ppp {}, origen ({}, {}), {:?}", ppp, x, y, text_2);
        prop_assert_eq!(r.uv_outside_texel, 0, "ppp {}, origen ({}, {}), {:?}", ppp, x, y, text_2);
        prop_assert_eq!(r.intermediate, 0, "ppp {}, origen ({}, {}), {:?}", ppp, x, y, text_2);
    }

    /// Propiedad 3: ancho = 7·n·caracteres y alto = 14·n píxeles.
    #[test]
    fn property_3_measure(ppp in ppp_of_the_property(), text_2 in interface_string()) {
        let bench = Bench::new_2(ppp);
        let n = text_scale(ppp) as f32;
        let (w, h) = bench.measure_px(&text_2);
        let characters = text_2.chars().count() as f32;
        let tolerance = tolerance_of_measure(ppp);
        prop_assert!((w - 7.0 * n * characters).abs() <= tolerance, "ppp {}: ancho {} de {:?}", ppp, w, text_2);
        prop_assert!((h - 14.0 * n).abs() <= tolerance, "ppp {}: alto {} de {:?}", ppp, h, text_2);
    }
}

/// Barrido determinista de la propiedad 2 por **todos** los ppp múltiplos de 0,05 entre 1 y 4
/// (el proptest elige solo algunos): así un fallo sale siempre y con la lista completa.
#[test]
fn property_2_sweep_of_all_the_ppp_multiples_of_0_05() {
    let text_2 = "Pixoko: 64×48 · ×8 · (12, 7) · lápiz ñÑ ¿Qué? /@WMwm%&$#";
    let mut failures = Vec::new();
    for k in 20_u32..=80 {
        let ppp = k as f32 / 20.0;
        let r = sharpness(ppp, pos2(3.37, 5.11), text_2);
        if r.outside_of_pixel != 0 || r.uv_outside_texel != 0 || r.intermediate != 0 {
            failures.push(format!("ppp {ppp}: {r:?}"));
        }
    }
    assert!(
        failures.is_empty(),
        "ppp sin nitidez:\n{}",
        failures.join("\n")
    );
}

/// Rangos de caracteres invisibles o combinantes de la spec (sección «Glifo ausente»).
const INVISIBLE: [(u32, u32); 14] = [
    (0x00AD, 0x00AD),
    (0x061C, 0x061C),
    (0x200B, 0x200F),
    (0x202A, 0x202E),
    (0x2060, 0x206F),
    (0xFEFF, 0xFEFF),
    (0xFFF9, 0xFFFB),
    (0xFE00, 0xFE0F),
    (0xE0100, 0xE01EF),
    (0x0300, 0x036F),
    (0x1AB0, 0x1AFF),
    (0x1DC0, 0x1DFF),
    (0x20D0, 0x20FF),
    (0xFE20, 0xFE2F),
];

fn is_invisible_or_combining(c: char) -> bool {
    INVISIBLE
        .iter()
        .any(|&(from_2, until)| (from_2..=until).contains(&(c as u32)))
}

fn range(from_2: u32, until: u32) -> proptest::char::CharStrategy<'static> {
    proptest::char::range(
        char::from_u32(from_2).expect("escalar válido"),
        char::from_u32(until).expect("escalar válido"),
    )
}

/// Un carácter con sesgo hacia lo difícil: controles, marcas bidireccionales y de formato,
/// combinantes, emoji, japonés y el propio juego de caracteres.
fn character_external() -> impl Strategy<Value = char> {
    let own: Vec<char> = INTERFACE_CHARSET.chars().collect();
    prop_oneof![
        4 => any::<char>(),
        3 => prop::sample::select(own),
        3 => range(0x00, 0x9F),
        2 => Just('\u{2028}'),
        2 => Just('\u{2029}'),
        2 => range(0x200B, 0x200F),
        2 => range(0x202A, 0x202E),
        1 => range(0x2060, 0x206F),
        1 => Just('\u{FEFF}'),
        1 => Just('\u{00AD}'),
        1 => range(0xFE00, 0xFE0F),
        1 => range(0xE0100, 0xE01EF),
        3 => range(0x0300, 0x036F),
        1 => range(0x1AB0, 0x1AFF),
        1 => range(0x1DC0, 0x1DFF),
        1 => range(0x20D0, 0x20FF),
        1 => range(0xFE20, 0xFE2F),
        3 => range(0x1F300, 0x1F64F),
        1 => Just('\u{200D}'),
        3 => range(0x3040, 0x30FF),
        3 => range(0x4E00, 0x4FFF),
    ]
}

fn text_external() -> impl Strategy<Value = String> {
    prop::collection::vec(character_external(), 0..=100)
        .prop_map(|v| v.into_iter().collect::<String>())
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(512))]

    /// Propiedad 4: `replace_missing` conserva el número de caracteres, deja solo caracteres con
    /// glifo y no invisibles ni combinantes, y es idempotente.
    #[test]
    fn property_4_replace_missing_of_any_text(text_2 in text_external()) {
        let has = has_glyph(ctx_shared());
        let output_2 = replace_missing(&text_2, &has);
        prop_assert_eq!(output_2.chars().count(), text_2.chars().count());
        for c in output_2.chars() {
            prop_assert!(has(c), "sin glifo: {:?}", c);
            prop_assert!(!is_invisible_or_combining(c), "invisible o combinante: {:?}", c);
            prop_assert!(!c.is_control(), "control: {:?}", c);
        }
        prop_assert_eq!(replace_missing(&output_2, &has), output_2);
    }

    /// Propiedad 4: identidad sobre cualquier cadena de `INTERFACE_CHARSET`.
    #[test]
    fn property_4_replace_missing_is_the_identity_in_the_interface_set(
        text_2 in interface_string()
    ) {
        let has = has_glyph(ctx_shared());
        prop_assert_eq!(replace_missing(&text_2, &has), text_2);
    }

    /// Propiedad 4: `fit_text`, con y sin `…` en la fuente.
    #[test]
    fn property_4_fit_text_of_any_text(
        text_2 in text_external(),
        max_chars in 0_usize..=120,
        with_points in any::<bool>(),
    ) {
        let has = |c: char| with_points || c != '…';
        let output_2 = fit_text(&text_2, max_chars, has);
        let largo = text_2.chars().count();
        prop_assert!(output_2.chars().count() <= max_chars);
        if largo <= max_chars {
            prop_assert_eq!(&output_2, &text_2, "si cabe, queda igual");
        } else {
            let suffix = if with_points { "…" } else { "..." };
            let long_suffix = suffix.chars().count();
            if max_chars >= long_suffix {
                prop_assert!(output_2.ends_with(suffix), "termina en {:?}: {:?}", suffix, output_2);
                prop_assert_eq!(output_2.chars().count(), max_chars);
            } else {
                let expected_2: String = suffix.chars().take(max_chars).collect();
                prop_assert_eq!(output_2, expected_2);
            }
        }
    }
}
