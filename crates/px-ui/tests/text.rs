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
const PANTALLA: egui::Vec2 = vec2(3000.0, 400.0);

// ---------------------------------------------------------------------------------------------
// Banco
// ---------------------------------------------------------------------------------------------

/// Canal alfa del atlas de fuentes, reconstruido a partir de los cambios de textura.
struct Atlas {
    ancho: usize,
    alto: usize,
    alfa: Vec<u8>,
}

struct Banco {
    ctx: Context,
    atlas: HashMap<TextureId, Atlas>,
}

impl Banco {
    /// Contexto con `ppp`, la fuente instalada, la escala aplicada y `MissingGlyphPolicy::Panic`
    /// (como en los tests de la spec), ya calentado.
    fn nuevo(ppp: f32) -> Banco {
        let ctx = Context::default();
        ctx.set_pixels_per_point(ppp);
        ctx.set_missing_glyph_policy(MissingGlyphPolicy::Panic);
        install_fonts(&ctx);
        let mut banco = Banco {
            ctx,
            atlas: HashMap::new(),
        };
        // El primer pase (vacío) activa el ppp, la política y las fuentes; `apply_scale` lee el
        // ppp ya activo; los dos siguientes dejan todo aplicado.
        banco.pase(|_| {});
        apply_scale(&banco.ctx);
        banco.pase(|_| {});
        banco.pase(|_| {});
        banco
    }

    /// Ejecuta un pase, absorbe sus cambios de textura en el atlas (egui entra en pánico si se
    /// sueltan sin aplicar) y devuelve las formas que se pintaron.
    fn pase(&mut self, mut dibujar: impl FnMut(&mut Ui)) -> Vec<egui::epaint::ClippedShape> {
        let input = RawInput {
            screen_rect: Some(Rect::from_min_size(Pos2::ZERO, PANTALLA)),
            ..RawInput::default()
        };
        let mut salida = self.ctx.run_ui(input, &mut dibujar);
        for (id, deltas) in &salida.textures_delta.set {
            for delta in deltas.iter() {
                let ImageData::Color(imagen) = &delta.image;
                let [w, h] = imagen.size;
                let atlas = self.atlas.entry(*id).or_insert_with(|| Atlas {
                    ancho: w,
                    alto: h,
                    alfa: vec![0; w * h],
                });
                if delta.pos.is_none() {
                    *atlas = Atlas {
                        ancho: w,
                        alto: h,
                        alfa: vec![0; w * h],
                    };
                }
                let [ox, oy] = delta.pos.unwrap_or([0, 0]);
                for y in 0..h {
                    for x in 0..w {
                        atlas.alfa[(oy + y) * atlas.ancho + ox + x] = imagen.pixels[y * w + x].a();
                    }
                }
            }
        }
        salida.textures_delta.clear();
        std::mem::take(&mut salida.shapes)
    }

    fn ppp(&self) -> f32 {
        self.ctx.pixels_per_point()
    }

    /// Tamaño en píxeles físicos de `texto` maquetado en una línea con el tamaño de la interfaz.
    fn medida_px(&self, texto: &str) -> (f32, f32) {
        let ppp = self.ppp();
        let fuente = FontId::proportional(font_size_points(ppp));
        let tamano = self.ctx.fonts_mut(|f| {
            f.layout_no_wrap(texto.to_string(), fuente, Color32::WHITE)
                .size()
        });
        (tamano.x * ppp, tamano.y * ppp)
    }
}

/// Contexto compartido solo para **consultar** glifos (`has_glyph`), a ppp 1. Nunca se le piden
/// pases: varios tests lo usan a la vez desde hilos distintos.
fn ctx_compartido() -> &'static Context {
    static CTX: OnceLock<Context> = OnceLock::new();
    CTX.get_or_init(|| Banco::nuevo(1.0).ctx)
}

/// `has_glyph` de la fuente instalada en `ctx` (la de `font_size_points` del ppp activo).
fn tiene_glifo(ctx: &Context) -> impl Fn(char) -> bool + '_ {
    let fuente = FontId::proportional(font_size_points(ctx.pixels_per_point()));
    move |c| ctx.fonts_mut(|f| f.has_glyph(&fuente, c))
}

/// Maqueta `texto` dentro de un pase de `ctx`; `true` si no entra en pánico. Tras un pánico el
/// contexto no se debe reutilizar.
fn maqueta_sin_panico(ctx: &Context, texto: &str) -> bool {
    let fuente = FontId::proportional(font_size_points(ctx.pixels_per_point()));
    catch_unwind(AssertUnwindSafe(|| {
        let input = RawInput {
            screen_rect: Some(Rect::from_min_size(Pos2::ZERO, PANTALLA)),
            ..RawInput::default()
        };
        let salida = ctx.run_ui(input, |ui| {
            ui.ctx().fonts_mut(|f| {
                let _ = f.layout_no_wrap(texto.to_string(), fuente.clone(), Color32::WHITE);
            });
        });
        salida.drop_without_applying_deltas();
    }))
    .is_ok()
}

/// Resultado del banco de nitidez para un texto.
#[derive(Debug, Default)]
struct Nitidez {
    glifos: usize,
    /// Glifos con algún vértice fuera de píxel físico (±0,001).
    fuera_de_pixel: usize,
    /// Glifos con alguna uv fuera de texel (±0,01).
    uv_fuera_de_texel: usize,
    /// Texels de atlas bajo los glifos, y de ellos los de alfa distinto de 0 y 255.
    texels: usize,
    intermedios: usize,
}

/// Pinta `texto` en `origen` (puntos) a `ppp`, lo tesela y mide la nitidez glifo a glifo.
fn nitidez(ppp: f32, origen: Pos2, texto: &str) -> Nitidez {
    let mut banco = Banco::nuevo(ppp);
    let tamano = font_size_points(ppp);
    let formas = banco.pase(|ui| {
        let _ = ui.ctx().layer_painter(LayerId::background()).text(
            origen,
            Align2::LEFT_TOP,
            texto,
            FontId::proportional(tamano),
            Color32::WHITE,
        );
    });
    let mut r = Nitidez::default();
    for primitiva in banco.ctx.tessellate(formas, ppp) {
        let Primitive::Mesh(malla) = primitiva.primitive else {
            continue;
        };
        let atlas = banco
            .atlas
            .get(&malla.texture_id)
            .expect("el atlas de la malla está reconstruido");
        let (aw, ah) = (atlas.ancho as f32, atlas.alto as f32);
        for glifo in malla.vertices.chunks(4) {
            if glifo.len() < 4 {
                continue;
            }
            r.glifos += 1;
            let fuera =
                |c: f32, escala: f32, tol: f32| (c * escala - (c * escala).round()).abs() > tol;
            if glifo
                .iter()
                .any(|v| fuera(v.pos.x, ppp, 1e-3) || fuera(v.pos.y, ppp, 1e-3))
            {
                r.fuera_de_pixel += 1;
            }
            if glifo
                .iter()
                .any(|v| fuera(v.uv.x, aw, 1e-2) || fuera(v.uv.y, ah, 1e-2))
            {
                r.uv_fuera_de_texel += 1;
            }
            let (mut x0, mut x1, mut y0, mut y1) = (f32::MAX, f32::MIN, f32::MAX, f32::MIN);
            for v in glifo {
                x0 = x0.min(v.uv.x * aw);
                x1 = x1.max(v.uv.x * aw);
                y0 = y0.min(v.uv.y * ah);
                y1 = y1.max(v.uv.y * ah);
            }
            for y in y0.round() as usize..y1.round() as usize {
                for x in x0.round() as usize..x1.round() as usize {
                    if let Some(&alfa) = atlas.alfa.get(y * atlas.ancho + x) {
                        r.texels += 1;
                        if alfa != 0 && alfa != 255 {
                            r.intermedios += 1;
                        }
                    }
                }
            }
        }
    }
    r
}

fn cerca(a: f32, b: f32, tolerancia: f32) -> bool {
    (a - b).abs() <= tolerancia
}

/// Tolerancia (px físicos) de las medidas de texto maquetado. La spec dice ±0,05, pero egui
/// cuantiza el tamaño del texto maquetado (`Galley`) a 1/32 de punto, y esa cuantización vale
/// hasta `ppp/64` píxeles (0,061 a 3,9×): se acepta `max(0,05; ppp/64 + 0,005)`.
fn tolerancia_de_medida(ppp: f32) -> f32 {
    0.05_f32.max(ppp / 64.0 + 0.005)
}

// ---------------------------------------------------------------------------------------------
// SHA-256 (sin dependencias)
// ---------------------------------------------------------------------------------------------

/// SHA-256 de `datos` en hexadecimal minúscula (FIPS 180-4).
fn sha256(datos: &[u8]) -> String {
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
    let mut mensaje = datos.to_vec();
    mensaje.push(0x80);
    while mensaje.len() % 64 != 56 {
        mensaje.push(0);
    }
    mensaje.extend_from_slice(&((datos.len() as u64) * 8).to_be_bytes());
    for bloque in mensaje.as_chunks::<64>().0 {
        let mut w = [0u32; 64];
        for (i, palabra) in bloque.as_chunks::<4>().0.iter().enumerate() {
            w[i] = u32::from_be_bytes(*palabra);
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
        for (estado, nuevo) in h.iter_mut().zip([a, b, c, d, e, f, g, hh]) {
            *estado = estado.wrapping_add(nuevo);
        }
    }
    h.iter().map(|palabra| format!("{palabra:08x}")).collect()
}

/// Lee un fichero de la raíz del repositorio (desde `crates/px-ui`).
fn lee_de_la_raiz(ruta: &str) -> Vec<u8> {
    let completa = format!("{}/../../{ruta}", env!("CARGO_MANIFEST_DIR"));
    std::fs::read(&completa).unwrap_or_else(|e| panic!("no se pudo leer {completa}: {e}"))
}

/// Texto de la raíz con los finales de línea normalizados a `\n` (un checkout de Windows con
/// `autocrlf` puede haberlos cambiado en los ficheros de texto).
fn lee_texto_de_la_raiz(ruta: &str) -> String {
    String::from_utf8(lee_de_la_raiz(ruta))
        .expect("UTF-8")
        .replace("\r\n", "\n")
}

// ---------------------------------------------------------------------------------------------
// Fila 1: text_scale
// ---------------------------------------------------------------------------------------------

#[test]
fn fila_01_text_scale_redondea_con_el_medio_hacia_arriba() {
    let casos: [(f32, u32); 11] = [
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
    for (ppp, esperado) in casos {
        assert_eq!(text_scale(ppp), esperado, "ppp {ppp}");
    }
}

#[test]
fn fila_01_text_scale_de_un_ppp_no_valido_es_1() {
    for ppp in [f32::NAN, 0.0, -1.0, f32::INFINITY] {
        assert_eq!(text_scale(ppp), 1, "ppp {ppp}");
    }
}

#[test]
fn fila_01_las_constantes_de_la_spec() {
    assert_eq!(FONT_EM_PX, 11.0);
    assert_eq!(MAX_TEXT_SCALE, 8);
}

// ---------------------------------------------------------------------------------------------
// Fila 2: tamaños en puntos
// ---------------------------------------------------------------------------------------------

#[test]
fn fila_02_el_tamano_de_fuente_en_pixeles_fisicos_es_multiplo_de_11() {
    let casos: [(f32, f32); 6] = [
        (1.0, 11.0),
        (1.25, 11.0),
        (1.5, 22.0),
        (2.0, 22.0),
        (2.5, 33.0),
        (3.0, 33.0),
    ];
    for (ppp, fisico) in casos {
        let cuerpo = font_size_points(ppp) * ppp;
        let titulo = heading_size_points(ppp) * ppp;
        assert!(cerca(cuerpo, fisico, 1e-3), "ppp {ppp}: cuerpo {cuerpo}");
        assert!(
            cerca(titulo, 2.0 * fisico, 1e-3),
            "ppp {ppp}: título {titulo}"
        );
    }
}

#[test]
fn fila_02_la_unidad_de_interfaz_son_n_pixeles_fisicos() {
    // `u = n/ppp` puntos: con 1,5 (n = 2) son 2 píxeles físicos; con 2,5 (n = 3), 3.
    for (ppp, n) in [
        (1.0_f32, 1.0_f32),
        (1.25, 1.0),
        (1.5, 2.0),
        (2.5, 3.0),
        (3.0, 3.0),
    ] {
        assert!(cerca(ui_unit(ppp) * ppp, n, 1e-3), "ppp {ppp}");
    }
}

// ---------------------------------------------------------------------------------------------
// Fila 3: cobertura de INTERFACE_CHARSET
// ---------------------------------------------------------------------------------------------

#[test]
fn fila_03_la_fuente_tiene_glifo_para_los_115_caracteres_de_la_interfaz() {
    let caracteres: Vec<char> = INTERFACE_CHARSET.chars().collect();
    assert_eq!(caracteres.len(), 115, "ASCII 95 + 14 + 2 + 4");
    let mut unicos = caracteres.clone();
    unicos.sort_unstable();
    unicos.dedup();
    assert_eq!(unicos.len(), 115, "sin repetidos");
    for c in ' '..='~' {
        assert!(caracteres.contains(&c), "falta el ASCII {c:?}");
    }
    for c in "ñÑáéíóúÁÉÍÓÚüÜ¿¡×·—…".chars() {
        assert!(caracteres.contains(&c), "falta {c:?}");
    }

    let ctx = ctx_compartido();
    let tiene = tiene_glifo(ctx);
    for c in caracteres {
        assert!(tiene(c), "sin glifo para {c:?} (U+{:04X})", c as u32);
    }
}

// ---------------------------------------------------------------------------------------------
// Fila 4: glifo ausente y replace_missing
// ---------------------------------------------------------------------------------------------

#[test]
fn fila_04_japones_check_y_emoji_no_tienen_glifo_y_los_puntos_suspensivos_si() {
    // La spec ponía también `→`, pero la cmap de Departure Mono 1.500 sí tiene U+2192 (se
    // comprobó leyendo la tabla del fichero fijado por SHA-256): se usa `✓` (U+2713), que no.
    let ctx = ctx_compartido();
    let tiene = tiene_glifo(ctx);
    assert!(!tiene('日'));
    assert!(!tiene('✓'));
    assert!(!tiene('😀'));
    assert!(tiene('…'));
}

#[test]
fn fila_04_replace_missing_cambia_solo_lo_que_no_se_puede_dibujar() {
    let ctx = ctx_compartido();
    let tiene = tiene_glifo(ctx);
    assert_eq!(replace_missing("a日b✓c", &tiene), "a?b?c");
    assert_eq!(replace_missing("a\0b\x1bc\r\n\u{2028}", &tiene), "a b c   ");
    assert_eq!(replace_missing("", &tiene), "");
    assert_eq!(replace_missing("straße €", &tiene), "straße €");
}

#[test]
fn fila_04_u2029_tambien_es_un_espacio() {
    let ctx = ctx_compartido();
    let tiene = tiene_glifo(ctx);
    assert_eq!(replace_missing("a\u{2029}b", &tiene), "a b");
}

#[test]
fn fila_04_printable_es_replace_missing_con_las_fuentes_del_contexto() {
    let ctx = ctx_compartido();
    let tiene = tiene_glifo(ctx);
    for texto in [
        "a日b✓c",
        "a\0b\x1bc\r\n\u{2028}",
        "",
        "straße €",
        "e\u{301}",
    ] {
        assert_eq!(printable(ctx, texto), replace_missing(texto, &tiene));
    }
}

// ---------------------------------------------------------------------------------------------
// Fila 5: la guardia de glifo ausente
// ---------------------------------------------------------------------------------------------

/// Cadenas de texto fijo de la interfaz: lo que `px-app` pone en la barra, reconstruido con los
/// mismos trozos que `status_line` (la función en sí se prueba en `px-app`, con la propiedad 5,
/// que exige que solo use `INTERFACE_CHARSET`).
fn textos_fijos() -> Vec<String> {
    let mut textos = vec![
        "Pixoko".to_string(),
        "Pixoko: ".to_string(),
        NOTICE_PREFIX.to_string(),
        INTERFACE_CHARSET.to_string(),
    ];
    for herramienta in [Tool::Pencil, Tool::Line, Tool::Fill] {
        textos.push(herramienta.label().to_string());
        let zoom = Zoom::from_int(8).expect("el zoom existe");
        for cursor in [None, Some((3, 4))] {
            for color in [None, Some("color 3 #ff0000"), Some("color 9 transparente")] {
                for (deshacer, rehacer) in
                    [(false, false), (true, false), (false, true), (true, true)]
                {
                    let mut linea = format!("64×48 · {zoom}");
                    if let Some((x, y)) = cursor {
                        linea.push_str(&format!(" · ({x}, {y})"));
                    }
                    linea.push_str(&format!(" · {}", status_text(herramienta, color)));
                    if deshacer {
                        linea.push_str(" · deshacer");
                    }
                    if rehacer {
                        linea.push_str(" · rehacer");
                    }
                    textos.push(linea);
                }
            }
        }
    }
    textos
}

#[test]
fn fila_05_el_texto_fijo_se_maqueta_sin_panico_con_la_politica_panic() {
    for ppp in [1.0_f32, 1.5, 2.0] {
        let banco = Banco::nuevo(ppp);
        for texto in textos_fijos() {
            assert!(
                maqueta_sin_panico(&banco.ctx, &texto),
                "ppp {ppp}: el texto fijo {texto:?} entra en pánico"
            );
        }
    }
}

#[test]
fn fila_05_un_caracter_sin_glifo_entra_en_panico_con_la_politica_panic() {
    let banco = Banco::nuevo(1.0);
    assert!(
        !maqueta_sin_panico(&banco.ctx, "a日b"),
        "la guardia no funciona: «日» se maquetó sin pánico"
    );
}

// ---------------------------------------------------------------------------------------------
// Fila 6: medidas
// ---------------------------------------------------------------------------------------------

#[test]
fn fila_06_pixoko_mide_7n_por_caracter_y_14n_de_alto() {
    let casos: [(f32, (f32, f32)); 4] = [
        (1.0, (42.0, 14.0)),
        (1.5, (84.0, 28.0)),
        (2.0, (84.0, 28.0)),
        (3.0, (126.0, 42.0)),
    ];
    for (ppp, (ancho, alto)) in casos {
        let banco = Banco::nuevo(ppp);
        let (w, h) = banco.medida_px("Pixoko");
        assert!(
            cerca(w, ancho, 0.05) && cerca(h, alto, 0.05),
            "ppp {ppp}: {w} × {h}, se esperaba {ancho} × {alto}"
        );
    }
}

#[test]
fn fila_06_los_115_caracteres_seguidos_a_1_5_miden_1610_px_de_ancho() {
    let banco = Banco::nuevo(1.5);
    let (w, h) = banco.medida_px(INTERFACE_CHARSET);
    assert!(cerca(w, 805.0 * 2.0, 0.05), "ancho {w}");
    assert!(cerca(h, 28.0, 0.05), "alto {h}");
}

// ---------------------------------------------------------------------------------------------
// Fila 7: nitidez geométrica
// ---------------------------------------------------------------------------------------------

#[test]
fn fila_07_ningun_glifo_cae_fuera_de_pixel_ni_hay_texels_intermedios() {
    let texto = "Pixoko: 64×48 · ×8 · (12, 7) · lápiz ñÑ ¿Qué?";
    let con_tinta = texto.chars().filter(|c| !c.is_whitespace()).count();
    for ppp in [1.0_f32, 1.25, 1.5, 1.75, 2.0, 2.5, 3.0] {
        let r = nitidez(ppp, pos2(3.37, 5.11), texto);
        assert!(
            r.glifos >= con_tinta,
            "ppp {ppp}: {} glifos para {con_tinta} caracteres con tinta",
            r.glifos
        );
        assert!(r.texels > 0, "ppp {ppp}: no se midió ningún texel");
        assert_eq!(
            r.fuera_de_pixel, 0,
            "ppp {ppp}: glifos fuera de píxel físico"
        );
        assert_eq!(r.uv_fuera_de_texel, 0, "ppp {ppp}: uv fuera de texel");
        assert_eq!(r.intermedios, 0, "ppp {ppp}: texels de alfa intermedio");
    }
}

// ---------------------------------------------------------------------------------------------
// Fila 8: fuente, licencia, THIRD-PARTY.md y .gitattributes
// ---------------------------------------------------------------------------------------------

const SHA_FUENTE: &str = "4d53f663155cf8bf7ffc8e688776e719625f7bbb80a8d90073438b249261a2e0";
const SHA_LICENCIA: &str = "b65e42750f3cb65437a97f4dfd781c58f199259949db2341012be60e32500e83";
const COPYRIGHT: &str = "Copyright 2022–2024 Helena Zhang (helenazhang.com).";

#[test]
fn fila_08_el_sha256_del_test_pasa_sus_vectores() {
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
fn fila_08_install_fonts_instala_una_sola_fuente_departure_mono() {
    let banco = Banco::nuevo(1.0);
    let definiciones = banco.ctx.fonts(|f| f.definitions().clone());
    assert_eq!(definiciones.font_data.len(), 1, "una sola fuente");
    let datos = definiciones
        .font_data
        .get("DepartureMono")
        .expect("la fuente se llama DepartureMono");
    assert_eq!(datos.tweak.hinting, Some(false));
    assert_eq!(datos.tweak.subpixel_binning, Some(false));
    let bytes: &[u8] = (*datos.font).as_ref();
    assert_eq!(bytes, FONT_BYTES, "es la fuente sin modificar");
    for familia in [FontFamily::Proportional, FontFamily::Monospace] {
        assert_eq!(
            definiciones.families.get(&familia),
            Some(&vec!["DepartureMono".to_string()]),
            "{familia:?}: solo DepartureMono, sin fuentes de reserva"
        );
    }
}

#[test]
fn fila_08_la_fuente_embebida_es_el_fichero_oficial() {
    let fichero = lee_de_la_raiz("assets/fonts/DepartureMono-Regular.otf");
    assert_eq!(fichero.as_slice(), FONT_BYTES, "FONT_BYTES es el fichero");
    assert_eq!(FONT_BYTES.len(), 84_480);
    assert_eq!(&FONT_BYTES[..4], b"OTTO");
    assert_eq!(sha256(&fichero), SHA_FUENTE);
    assert_eq!(sha256(FONT_BYTES), SHA_FUENTE);
}

#[test]
fn fila_08_la_licencia_embebida_es_la_ofl_de_la_fuente() {
    let fichero = lee_de_la_raiz("assets/fonts/LICENSE");
    assert_eq!(
        fichero.as_slice(),
        FONT_LICENSE.as_bytes(),
        "FONT_LICENSE es el fichero"
    );
    assert_eq!(sha256(&fichero), SHA_LICENCIA);
    assert_eq!(sha256(FONT_LICENSE.as_bytes()), SHA_LICENCIA);
    assert!(FONT_LICENSE.starts_with(COPYRIGHT));
}

#[test]
fn fila_08_third_party_documenta_la_fuente_y_su_licencia() {
    let doc = lee_texto_de_la_raiz("docs/THIRD-PARTY.md");
    for parte in [
        "Departure Mono",
        "1.500",
        "https://github.com/rektdeckard/departure-mono",
        "public/assets/LICENSE",
        "OFL-1.1",
        COPYRIGHT,
        SHA_FUENTE,
        SHA_LICENCIA,
    ] {
        assert!(doc.contains(parte), "THIRD-PARTY.md no contiene {parte:?}");
    }
    assert!(
        doc.lines()
            .any(|l| l.contains("Reserved Font Name") && l.to_lowercase().contains("ninguno")),
        "THIRD-PARTY.md debe declarar que no hay Reserved Font Name (una línea con \
         «Reserved Font Name» y «Ninguno»)"
    );
    let licencia = FONT_LICENSE.replace("\r\n", "\n");
    assert!(
        doc.contains(licencia.trim_end()),
        "THIRD-PARTY.md debe contener el texto completo de FONT_LICENSE"
    );
    // El MIT de la raíz del repositorio de la fuente es el del sitio web: no se atribuye.
    assert!(!doc.contains("MIT License"));
    assert!(!doc.contains("Tobias Fried"));
}

#[test]
fn fila_08_gitattributes_marca_la_fuente_como_binaria() {
    let atributos = lee_texto_de_la_raiz(".gitattributes");
    assert!(
        atributos
            .lines()
            .any(|l| l.trim() == "assets/fonts/** binary"),
        ".gitattributes debe contener la línea `assets/fonts/** binary`"
    );
}

// ---------------------------------------------------------------------------------------------
// Fila 9: apply_scale
// ---------------------------------------------------------------------------------------------

fn tamano_de_estilo(ctx: &Context, estilo: TextStyle) -> f32 {
    ctx.global_style().text_styles[&estilo].size
}

fn pase_vacio(ctx: &Context) {
    let input = RawInput {
        screen_rect: Some(Rect::from_min_size(Pos2::ZERO, vec2(400.0, 300.0))),
        ..RawInput::default()
    };
    ctx.run_ui(input, |_| {}).drop_without_applying_deltas();
}

#[test]
fn fila_09_apply_scale_sigue_al_ppp_y_no_toca_el_estilo_si_no_cambia() {
    let ctx = Context::default();
    install_fonts(&ctx);

    // `set_pixels_per_point` se activa al empezar el pase siguiente.
    ctx.set_pixels_per_point(1.5);
    pase_vacio(&ctx);
    apply_scale(&ctx);
    for estilo in [
        TextStyle::Small,
        TextStyle::Body,
        TextStyle::Button,
        TextStyle::Monospace,
    ] {
        assert!(
            cerca(tamano_de_estilo(&ctx, estilo.clone()), 22.0 / 1.5, 1e-3),
            "ppp 1,5: {estilo:?}"
        );
    }
    assert!(cerca(tamano_de_estilo(&ctx, TextStyle::Body), 14.667, 1e-3));
    assert!(cerca(
        tamano_de_estilo(&ctx, TextStyle::Heading),
        29.333,
        1e-3
    ));

    ctx.set_pixels_per_point(2.0);
    pase_vacio(&ctx);
    apply_scale(&ctx);
    for estilo in [
        TextStyle::Small,
        TextStyle::Body,
        TextStyle::Button,
        TextStyle::Monospace,
    ] {
        assert!(
            cerca(tamano_de_estilo(&ctx, estilo.clone()), 11.0, 1e-3),
            "ppp 2: {estilo:?}"
        );
    }
    assert!(cerca(
        tamano_de_estilo(&ctx, TextStyle::Heading),
        22.0,
        1e-3
    ));

    // Segunda llamada seguida: el estilo ni cambia de valor ni se reescribe (con una copia del
    // `Arc` en la mano, cualquier escritura con `Arc::make_mut` lo habría clonado).
    let antes = ctx.global_style();
    apply_scale(&ctx);
    let despues = ctx.global_style();
    assert_eq!(*antes, *despues);
    assert!(
        std::sync::Arc::ptr_eq(&antes, &despues),
        "la segunda llamada reescribió el estilo"
    );
}

// ---------------------------------------------------------------------------------------------
// Fila 14: texto externo
// ---------------------------------------------------------------------------------------------

#[test]
fn fila_14_el_texto_externo_pasa_por_la_sustitucion() {
    let ctx = ctx_compartido();
    let tiene = tiene_glifo(ctx);
    let casos: [(&str, &str); 8] = [
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
    for (entrada, esperado) in casos {
        let salida = replace_missing(entrada, &tiene);
        assert_eq!(salida, esperado, "entrada {entrada:?}");
        assert_eq!(
            salida.chars().count(),
            entrada.chars().count(),
            "mismo número de caracteres: {entrada:?}"
        );
        assert!(salida.chars().all(&tiene), "todos con glifo: {salida:?}");
    }
}

#[test]
fn fila_14_un_informe_de_conversion_se_queda_igual() {
    let ctx = ctx_compartido();
    let tiene = tiene_glifo(ctx);
    let informe = "Convertido a 16 colores: año ×2 · se perdieron 3 colores…";
    assert_eq!(replace_missing(informe, &tiene), informe);
}

#[test]
fn fila_14_cada_resultado_se_maqueta_sin_panico() {
    let banco = Banco::nuevo(1.0);
    let tiene = tiene_glifo(&banco.ctx);
    for entrada in [
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
        let salida = replace_missing(entrada, &tiene);
        assert!(
            maqueta_sin_panico(&banco.ctx, &salida),
            "{salida:?} (de {entrada:?}) entra en pánico"
        );
    }
}

// ---------------------------------------------------------------------------------------------
// Fila 15: fit_text
// ---------------------------------------------------------------------------------------------

#[test]
fn fila_15_con_los_puntos_suspensivos_en_la_fuente_termina_en_uno() {
    let con = |_: char| true;
    let casos: [(usize, &str); 6] = [
        (6, "abcdef"),
        (5, "abcd…"),
        (3, "ab…"),
        (2, "a…"),
        (1, "…"),
        (0, ""),
    ];
    for (n, esperado) in casos {
        assert_eq!(fit_text("abcdef", n, con), esperado, "n = {n}");
    }
}

#[test]
fn fila_15_sin_los_puntos_suspensivos_en_la_fuente_termina_en_tres_puntos() {
    let sin = |c: char| c != '…';
    let casos: [(usize, &str); 6] = [
        (6, "abcdef"),
        (5, "ab..."),
        (3, "..."),
        (2, ".."),
        (1, "."),
        (0, ""),
    ];
    for (n, esperado) in casos {
        assert_eq!(fit_text("abcdef", n, sin), esperado, "n = {n}");
    }
}

#[test]
fn fila_15_vacio_da_vacio_y_un_nombre_larguisimo_se_recorta() {
    let con = |_: char| true;
    let sin = |c: char| c != '…';
    for n in [0, 1, 40] {
        assert_eq!(fit_text("", n, con), "");
        assert_eq!(fit_text("", n, sin), "");
    }
    let largo: String = (0..100_000)
        .map(|i| char::from(b'a' + (i % 26) as u8))
        .collect();
    let recortado = fit_text(&largo, 40, con);
    assert_eq!(recortado.chars().count(), 40);
    assert!(recortado.ends_with('…'));
    assert_eq!(&recortado[..39], &largo[..39]);
    let sin_puntos = fit_text(&largo, 40, sin);
    assert_eq!(sin_puntos.chars().count(), 40);
    assert!(sin_puntos.ends_with("..."));
}

#[test]
fn fila_15_cuenta_caracteres_y_no_bytes() {
    let con = |_: char| true;
    // 6 caracteres de 2 bytes cada uno.
    assert_eq!(fit_text("ñÑáéíó", 6, con), "ñÑáéíó");
    assert_eq!(fit_text("ñÑáéíóú", 4, con), "ñÑá…");
}

// ---------------------------------------------------------------------------------------------
// Propiedades
// ---------------------------------------------------------------------------------------------

/// Cualquier `f32` (incluidos NaN, infinitos, negativos y el rango útil de ppp).
fn ppp_cualquiera() -> impl Strategy<Value = f32> {
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
    fn propiedad_1_la_escala_esta_entre_1_y_8(ppp in ppp_cualquiera()) {
        let n = text_scale(ppp);
        prop_assert!((1..=MAX_TEXT_SCALE).contains(&n), "ppp {ppp}: n = {n}");
    }

    /// Propiedad 1: la escala no decrece con ppp.
    #[test]
    fn propiedad_1_la_escala_no_decrece(a in -10.0_f32..40.0, b in -10.0_f32..40.0) {
        let (menor, mayor) = if a <= b { (a, b) } else { (b, a) };
        prop_assert!(text_scale(menor) <= text_scale(mayor), "{menor} y {mayor}");
    }

    /// Propiedad 1: en [0,5; 8) difiere de ppp en no más de 0,5.
    #[test]
    fn propiedad_1_la_escala_esta_a_medio_punto_de_ppp(ppp in 0.5_f32..8.0) {
        let n = text_scale(ppp) as f32;
        prop_assert!((n - ppp).abs() <= 0.5 + 1e-6, "ppp {ppp}: n = {n}");
    }

    /// Propiedad 1: en [0,25; 16] el tamaño en píxeles físicos es 11·n.
    #[test]
    fn propiedad_1_el_tamano_fisico_es_11_por_n(ppp in 0.25_f32..=16.0) {
        let n = text_scale(ppp) as f32;
        let fisico = font_size_points(ppp) * ppp;
        prop_assert!((fisico - 11.0 * n).abs() <= 1e-3, "ppp {ppp}: {fisico} y n = {n}");
        let titulo = heading_size_points(ppp) * ppp;
        prop_assert!((titulo - 22.0 * n).abs() <= 1e-3, "ppp {ppp}: título {titulo}");
    }
}

/// ppp múltiplo de 0,05 entre 1 y 4 (`k / 20` es exacto en `f32` para los medios).
fn ppp_de_la_propiedad() -> impl Strategy<Value = f32> {
    (20_u32..=80).prop_map(|k| k as f32 / 20.0)
}

/// De 1 a 60 caracteres de `INTERFACE_CHARSET`.
fn cadena_de_la_interfaz() -> impl Strategy<Value = String> {
    let caracteres: Vec<char> = INTERFACE_CHARSET.chars().collect();
    prop::collection::vec(prop::sample::select(caracteres), 1..=60)
        .prop_map(|v| v.into_iter().collect::<String>())
}

proptest! {
    // Cada caso crea un contexto, instala la fuente y hace varios pases: pocos casos.
    #![proptest_config(ProptestConfig::with_cases(48))]

    /// Propiedad 2: nitidez geométrica con origen y cadena cualesquiera.
    #[test]
    fn propiedad_2_nitidez_geometrica(
        ppp in ppp_de_la_propiedad(),
        x in 0.0_f32..200.0,
        y in 0.0_f32..200.0,
        texto in cadena_de_la_interfaz(),
    ) {
        let r = nitidez(ppp, pos2(x, y), &texto);
        prop_assert_eq!(r.fuera_de_pixel, 0, "ppp {}, origen ({}, {}), {:?}", ppp, x, y, texto);
        prop_assert_eq!(r.uv_fuera_de_texel, 0, "ppp {}, origen ({}, {}), {:?}", ppp, x, y, texto);
        prop_assert_eq!(r.intermedios, 0, "ppp {}, origen ({}, {}), {:?}", ppp, x, y, texto);
    }

    /// Propiedad 3: ancho = 7·n·caracteres y alto = 14·n píxeles.
    #[test]
    fn propiedad_3_medida(ppp in ppp_de_la_propiedad(), texto in cadena_de_la_interfaz()) {
        let banco = Banco::nuevo(ppp);
        let n = text_scale(ppp) as f32;
        let (w, h) = banco.medida_px(&texto);
        let caracteres = texto.chars().count() as f32;
        let tolerancia = tolerancia_de_medida(ppp);
        prop_assert!((w - 7.0 * n * caracteres).abs() <= tolerancia, "ppp {}: ancho {} de {:?}", ppp, w, texto);
        prop_assert!((h - 14.0 * n).abs() <= tolerancia, "ppp {}: alto {} de {:?}", ppp, h, texto);
    }
}

/// Barrido determinista de la propiedad 2 por **todos** los ppp múltiplos de 0,05 entre 1 y 4
/// (el proptest elige solo algunos): así un fallo sale siempre y con la lista completa.
#[test]
fn propiedad_2_barrido_de_todos_los_ppp_multiplos_de_0_05() {
    let texto = "Pixoko: 64×48 · ×8 · (12, 7) · lápiz ñÑ ¿Qué? /@WMwm%&$#";
    let mut fallos = Vec::new();
    for k in 20_u32..=80 {
        let ppp = k as f32 / 20.0;
        let r = nitidez(ppp, pos2(3.37, 5.11), texto);
        if r.fuera_de_pixel != 0 || r.uv_fuera_de_texel != 0 || r.intermedios != 0 {
            fallos.push(format!("ppp {ppp}: {r:?}"));
        }
    }
    assert!(fallos.is_empty(), "ppp sin nitidez:\n{}", fallos.join("\n"));
}

/// Rangos de caracteres invisibles o combinantes de la spec (sección «Glifo ausente»).
const INVISIBLES: [(u32, u32); 14] = [
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

fn es_invisible_o_combinante(c: char) -> bool {
    INVISIBLES
        .iter()
        .any(|&(desde, hasta)| (desde..=hasta).contains(&(c as u32)))
}

fn rango(desde: u32, hasta: u32) -> proptest::char::CharStrategy<'static> {
    proptest::char::range(
        char::from_u32(desde).expect("escalar válido"),
        char::from_u32(hasta).expect("escalar válido"),
    )
}

/// Un carácter con sesgo hacia lo difícil: controles, marcas bidireccionales y de formato,
/// combinantes, emoji, japonés y el propio juego de caracteres.
fn caracter_externo() -> impl Strategy<Value = char> {
    let propios: Vec<char> = INTERFACE_CHARSET.chars().collect();
    prop_oneof![
        4 => any::<char>(),
        3 => prop::sample::select(propios),
        3 => rango(0x00, 0x9F),
        2 => Just('\u{2028}'),
        2 => Just('\u{2029}'),
        2 => rango(0x200B, 0x200F),
        2 => rango(0x202A, 0x202E),
        1 => rango(0x2060, 0x206F),
        1 => Just('\u{FEFF}'),
        1 => Just('\u{00AD}'),
        1 => rango(0xFE00, 0xFE0F),
        1 => rango(0xE0100, 0xE01EF),
        3 => rango(0x0300, 0x036F),
        1 => rango(0x1AB0, 0x1AFF),
        1 => rango(0x1DC0, 0x1DFF),
        1 => rango(0x20D0, 0x20FF),
        1 => rango(0xFE20, 0xFE2F),
        3 => rango(0x1F300, 0x1F64F),
        1 => Just('\u{200D}'),
        3 => rango(0x3040, 0x30FF),
        3 => rango(0x4E00, 0x4FFF),
    ]
}

fn texto_externo() -> impl Strategy<Value = String> {
    prop::collection::vec(caracter_externo(), 0..=100)
        .prop_map(|v| v.into_iter().collect::<String>())
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(512))]

    /// Propiedad 4: `replace_missing` conserva el número de caracteres, deja solo caracteres con
    /// glifo y no invisibles ni combinantes, y es idempotente.
    #[test]
    fn propiedad_4_replace_missing_de_cualquier_texto(texto in texto_externo()) {
        let tiene = tiene_glifo(ctx_compartido());
        let salida = replace_missing(&texto, &tiene);
        prop_assert_eq!(salida.chars().count(), texto.chars().count());
        for c in salida.chars() {
            prop_assert!(tiene(c), "sin glifo: {:?}", c);
            prop_assert!(!es_invisible_o_combinante(c), "invisible o combinante: {:?}", c);
            prop_assert!(!c.is_control(), "control: {:?}", c);
        }
        prop_assert_eq!(replace_missing(&salida, &tiene), salida);
    }

    /// Propiedad 4: identidad sobre cualquier cadena de `INTERFACE_CHARSET`.
    #[test]
    fn propiedad_4_replace_missing_es_la_identidad_en_el_juego_de_la_interfaz(
        texto in cadena_de_la_interfaz()
    ) {
        let tiene = tiene_glifo(ctx_compartido());
        prop_assert_eq!(replace_missing(&texto, &tiene), texto);
    }

    /// Propiedad 4: `fit_text`, con y sin `…` en la fuente.
    #[test]
    fn propiedad_4_fit_text_de_cualquier_texto(
        texto in texto_externo(),
        max_chars in 0_usize..=120,
        con_puntos in any::<bool>(),
    ) {
        let tiene = |c: char| con_puntos || c != '…';
        let salida = fit_text(&texto, max_chars, tiene);
        let largo = texto.chars().count();
        prop_assert!(salida.chars().count() <= max_chars);
        if largo <= max_chars {
            prop_assert_eq!(&salida, &texto, "si cabe, queda igual");
        } else {
            let sufijo = if con_puntos { "…" } else { "..." };
            let largo_sufijo = sufijo.chars().count();
            if max_chars >= largo_sufijo {
                prop_assert!(salida.ends_with(sufijo), "termina en {:?}: {:?}", sufijo, salida);
                prop_assert_eq!(salida.chars().count(), max_chars);
            } else {
                let esperado: String = sufijo.chars().take(max_chars).collect();
                prop_assert_eq!(salida, esperado);
            }
        }
    }
}
