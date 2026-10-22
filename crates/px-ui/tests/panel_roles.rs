//! Unidad I.2a, filas 5 y 6 en el panel (specs/interfaz-dos-colores.md): `palette_panel::show_roles`
//! con el banco de egui sin ventana (el de `tests/palette_panel.rs`, reducido). Clic y clic
//! derecho en las muestras, anillos de los dos colores, texto de la muestra, indicador de color
//! (clic izquierdo: cuentagotas; clic derecho: foco al editor y rol) y los dos selectores.
//!
//! Las posiciones salen de las constantes y funciones de `palette_grid`; el panel empieza en
//! `(0, 0)`. Las pruebas de anillos usan `ppp` 1,0 (un punto es un píxel físico).

use egui::epaint::{ClippedShape, Shape};
use egui::{
    Color32, Context, Event, MissingGlyphPolicy, Modifiers, PointerButton, Pos2, RawInput, Rect,
    Vec2, pos2, vec2,
};
use px_core::{Image, Palette, Rgba};
use px_ui::color::{ColorChoice, Role, SlotColors};
use px_ui::color_edit::{ColorEditor, EditTarget};
use px_ui::color_editor::hex_field_id;
use px_ui::palette_grid::{
    CELL_U, COLUMNS, GRID_X_U, GRID_Y_U, PITCH_U, PanelView, Scroll, corner_rects, indicator_rects,
    ring_rects, selector_rect,
};
use px_ui::palette_ops::PaletteOps;
use px_ui::palette_panel::{Indicator, PanelOutput, Roles, show_roles};
use px_ui::text::{apply_scale, install_fonts, text_scale};
use px_ui::theme::{RING_ACTIVE, RING_BACKGROUND};
use px_ui::view::ScreenRect;

#[path = "common/zones.rs"]
mod zones;
use zones::panel_a_la_izquierda;

const PANTALLA: Vec2 = vec2(1200.0, 1000.0);

fn indexada(count: usize) -> Image {
    let colores: Vec<Rgba> = (0..count)
        .map(|i| Rgba::opaque(i as u8, 255 - i as u8, (i * 5) as u8))
        .collect();
    let paleta = Palette::from_colors(colores).expect("paleta");
    Image::new_indexed(8, 8, paleta).expect("imagen")
}

struct Banco {
    ctx: Context,
    scroll: Scroll,
    view: PanelView,
    editor: ColorEditor,
    ops: PaletteOps,
}

struct Fotograma {
    salida: PanelOutput,
    formas: Vec<ClippedShape>,
}

impl Banco {
    fn nuevo(ppp: f32) -> Banco {
        let ctx = Context::default();
        ctx.set_pixels_per_point(ppp);
        ctx.set_missing_glyph_policy(MissingGlyphPolicy::Panic);
        install_fonts(&ctx);
        let banco = Banco {
            ctx,
            scroll: Scroll::default(),
            view: PanelView::default(),
            editor: ColorEditor::default(),
            ops: PaletteOps::default(),
        };
        for _ in 0..2 {
            let salida = banco
                .ctx
                .run_ui(Banco::entrada(vec![]), |ui| apply_scale(ui.ctx()));
            salida.drop_without_applying_deltas();
        }
        banco
    }

    fn entrada(events: Vec<Event>) -> RawInput {
        RawInput {
            screen_rect: Some(Rect::from_min_size(Pos2::ZERO, PANTALLA)),
            events,
            ..RawInput::default()
        }
    }

    fn n(&self) -> i64 {
        i64::from(text_scale(self.ctx.pixels_per_point()))
    }

    fn fotograma(
        &mut self,
        image: &Image,
        roles: Roles,
        enabled: bool,
        events: Vec<Event>,
    ) -> Fotograma {
        let slots = SlotColors::defaults(image.mode());
        let editing = match roles.editing {
            Role::Foreground => roles.foreground,
            Role::Background => roles.background,
        };
        if let Some(target) = EditTarget::of(image, editing) {
            let color = target.stored(image, &slots).expect("color");
            self.editor.sync(Some((target, color)));
        }
        let scroll = &mut self.scroll;
        let view = &mut self.view;
        let editor = &mut self.editor;
        let ops = &mut self.ops;
        let mut resultado = None;
        let mut salida = self.ctx.run_ui(Banco::entrada(events), |ui| {
            apply_scale(ui.ctx());
            let (panel, ()) = panel_a_la_izquierda(
                ui,
                |ui| show_roles(ui, image, roles, scroll, view, enabled, &slots, editor, ops),
                |_| (),
            );
            resultado = Some(panel);
        });
        salida.textures_delta.clear();
        let formas = std::mem::take(&mut salida.shapes);
        Fotograma {
            salida: resultado.expect("el panel se pinta"),
            formas,
        }
    }

    /// Mueve el puntero a `pos` y luego ejecuta `events` en otro fotograma.
    fn con_puntero(
        &mut self,
        image: &Image,
        roles: Roles,
        enabled: bool,
        pos: Pos2,
        events: Vec<Event>,
    ) -> Fotograma {
        self.fotograma(image, roles, enabled, vec![Event::PointerMoved(pos)]);
        self.fotograma(image, roles, enabled, events)
    }

    /// Pulsa `button` en `(x, y)` píxeles físicos y devuelve lo que el panel dijo al pulsar.
    fn clic(
        &mut self,
        image: &Image,
        roles: Roles,
        enabled: bool,
        button: PointerButton,
        (x, y): (i64, i64),
    ) -> PanelOutput {
        let ppp = self.ctx.pixels_per_point();
        let pos = pos2(x as f32 / ppp, y as f32 / ppp);
        let pulsado = self
            .con_puntero(image, roles, enabled, pos, vec![boton(pos, button, true)])
            .salida;
        self.fotograma(image, roles, enabled, vec![boton(pos, button, false)]);
        pulsado
    }
}

fn boton(pos: Pos2, button: PointerButton, pressed: bool) -> Event {
    Event::PointerButton {
        pos,
        button,
        pressed,
        modifiers: Modifiers::NONE,
    }
}

fn roles(foreground: ColorChoice, background: ColorChoice) -> Roles {
    Roles {
        foreground,
        background,
        editing: Role::Foreground,
    }
}

fn centro_de_muestra(n: i64, indice: usize) -> (i64, i64) {
    let columna = (indice % COLUMNS) as i64;
    let fila = (indice / COLUMNS) as i64;
    (
        (GRID_X_U + columna * PITCH_U) * n + CELL_U * n / 2,
        (GRID_Y_U + fila * PITCH_U) * n + CELL_U * n / 2,
    )
}

fn recoge(shape: &Shape, salida: &mut Vec<(Rect, Color32)>) {
    match shape {
        Shape::Rect(rect) => salida.push((rect.rect, rect.fill)),
        Shape::Vec(formas) => formas.iter().for_each(|f| recoge(f, salida)),
        _ => {}
    }
}

fn rectangulos(formas: &[ClippedShape]) -> Vec<(Rect, Color32)> {
    let mut salida = Vec::new();
    for forma in formas {
        recoge(&forma.shape, &mut salida);
    }
    salida
}

fn textos(formas: &[ClippedShape]) -> Vec<String> {
    fn rec(shape: &Shape, salida: &mut Vec<String>) {
        match shape {
            Shape::Text(texto) => salida.push(texto.galley.text().to_string()),
            Shape::Vec(formas) => formas.iter().for_each(|f| rec(f, salida)),
            _ => {}
        }
    }
    let mut salida = Vec::new();
    for forma in formas {
        rec(&forma.shape, &mut salida);
    }
    salida
}

/// Rectángulos pintados con `color` (a `ppp` 1,0, en píxeles físicos de la ventana).
fn con_color(formas: &[ClippedShape], color: Color32) -> Vec<Rect> {
    rectangulos(formas)
        .into_iter()
        .filter(|(_, fill)| *fill == color)
        .map(|(rect, _)| rect)
        .collect()
}

/// La esquina superior izquierda de la muestra `indice` a `ppp` 1,0.
fn celda(indice: usize) -> ScreenRect {
    let columna = (indice % COLUMNS) as i64;
    let fila = (indice / COLUMNS) as i64;
    ScreenRect {
        x: GRID_X_U + columna * PITCH_U,
        y: GRID_Y_U + fila * PITCH_U,
        width: CELL_U,
        height: CELL_U,
    }
}

fn a_rect(r: ScreenRect) -> Rect {
    Rect::from_min_size(
        pos2(r.x as f32, r.y as f32),
        vec2(r.width as f32, r.height as f32),
    )
}

// ---------------------------------------------------------------------------------------------
// Fila 5: clic, clic derecho y anillos
// ---------------------------------------------------------------------------------------------

#[test]
fn fila_5_clic_elige_el_primer_plano_y_clic_derecho_el_fondo() {
    let image = indexada(256);
    let mut banco = Banco::nuevo(1.0);
    let r = roles(ColorChoice::Entry(5), ColorChoice::Entry(9));
    let izquierdo = banco.clic(
        &image,
        r,
        true,
        PointerButton::Primary,
        centro_de_muestra(1, 200),
    );
    assert_eq!(izquierdo.choice, Some(ColorChoice::Entry(200)));
    assert_eq!(izquierdo.background, None);
    let derecho = banco.clic(
        &image,
        r,
        true,
        PointerButton::Secondary,
        centro_de_muestra(1, 200),
    );
    assert_eq!(derecho.background, Some(ColorChoice::Entry(200)));
    assert_eq!(derecho.choice, None);
}

#[test]
fn fila_5_en_rgba_el_clic_derecho_elige_la_ranura_como_fondo() {
    let image = Image::new(64, 48, px_core::Mode::Rgba).expect("imagen");
    let mut banco = Banco::nuevo(1.0);
    let r = roles(ColorChoice::Slot(1), ColorChoice::Slot(9));
    let derecho = banco.clic(
        &image,
        r,
        true,
        PointerButton::Secondary,
        centro_de_muestra(1, 2),
    );
    assert_eq!(derecho.background, Some(ColorChoice::Slot(3)));
}

#[test]
fn fila_5_el_central_el_hueco_lo_de_fuera_y_enabled_falso_no_eligen_ninguno() {
    let image = indexada(256);
    let mut banco = Banco::nuevo(1.0);
    let r = roles(ColorChoice::Entry(5), ColorChoice::Entry(9));
    let hueco = (GRID_X_U + CELL_U + 1, GRID_Y_U + 1);
    for (button, lugar, enabled) in [
        (PointerButton::Middle, centro_de_muestra(1, 20), true),
        (PointerButton::Primary, hueco, true),
        (PointerButton::Secondary, hueco, true),
        (PointerButton::Secondary, (1100, 900), true),
        (PointerButton::Primary, centro_de_muestra(1, 20), false),
        (PointerButton::Secondary, centro_de_muestra(1, 20), false),
    ] {
        let salida = banco.clic(&image, r, enabled, button, lugar);
        assert_eq!(salida.choice, None, "{button:?} {lugar:?} {enabled}");
        assert_eq!(salida.background, None, "{button:?} {lugar:?} {enabled}");
    }
}

#[test]
fn fila_5_los_dos_anillos_rodean_cada_uno_su_muestra() {
    let image = indexada(256);
    let mut banco = Banco::nuevo(1.0);
    let r = roles(ColorChoice::Entry(3), ColorChoice::Entry(40));
    let f = banco.fotograma(&image, r, true, vec![]);
    let verdes = con_color(&f.formas, RING_ACTIVE);
    for esperado in ring_rects(celda(3), 2) {
        assert!(verdes.contains(&a_rect(esperado)), "falta {esperado:?}");
    }
    let amarillos = con_color(&f.formas, RING_BACKGROUND);
    let esperados = corner_rects(celda(40), 0, 2, 5);
    assert_eq!(amarillos.len(), 8, "{amarillos:?}");
    for esperado in esperados {
        assert!(amarillos.contains(&a_rect(esperado)), "falta {esperado:?}");
    }
}

#[test]
fn fila_5_con_la_misma_muestra_el_primer_plano_va_dentro_y_el_fondo_fuera() {
    let image = indexada(256);
    let mut banco = Banco::nuevo(1.0);
    let r = roles(ColorChoice::Entry(7), ColorChoice::Entry(7));
    let f = banco.fotograma(&image, r, true, vec![]);
    let verdes = con_color(&f.formas, RING_ACTIVE);
    let amarillos = con_color(&f.formas, RING_BACKGROUND);
    for esperado in ring_rects(celda(7), 2) {
        assert!(verdes.contains(&a_rect(esperado)));
    }
    for esperado in corner_rects(celda(7), 2, 2, 5) {
        assert!(amarillos.contains(&a_rect(esperado)));
    }
    for v in &verdes {
        for a in &amarillos {
            assert!(!v.intersect(*a).is_positive(), "{v:?} y {a:?}");
        }
    }
}

#[test]
fn fila_5_la_linea_2_dice_frente_y_fondo_para_la_muestra_de_los_dos() {
    let image = indexada(256);
    let mut banco = Banco::nuevo(1.0);
    let r = roles(ColorChoice::Entry(7), ColorChoice::Entry(7));
    let f = banco.fotograma(&image, r, true, vec![]);
    let textos = textos(&f.formas);
    assert!(
        textos.iter().any(|t| t == "frente · fondo"),
        "textos: {textos:?}"
    );
    // Con el cursor en otra muestra, la línea 2 habla de esa muestra y no del fondo.
    let pos = pos2(
        centro_de_muestra(1, 20).0 as f32,
        centro_de_muestra(1, 20).1 as f32,
    );
    let f = banco.con_puntero(&image, r, true, pos, vec![]);
    let textos = textos_de(&f);
    assert!(textos.iter().any(|t| t == "cursor"), "textos: {textos:?}");
}

fn textos_de(f: &Fotograma) -> Vec<String> {
    textos(&f.formas)
}

// ---------------------------------------------------------------------------------------------
// Fila 6: indicador y selector
// ---------------------------------------------------------------------------------------------

fn centro(r: &ScreenRect) -> (i64, i64) {
    (r.x + r.width / 2, r.y + r.height / 2)
}

#[test]
fn fila_6_clic_izquierdo_en_el_indicador_pide_el_cuentagotas_en_los_dos_rectangulos() {
    let image = indexada(16);
    for ppp in [1.0, 2.0, 3.0] {
        let mut banco = Banco::nuevo(ppp);
        let n = banco.n();
        let (fg, bg) = indicator_rects(n as u32);
        let r = roles(ColorChoice::Entry(2), ColorChoice::Entry(3));
        for punto in [centro(&fg), (bg.x + bg.width - 1, bg.y + bg.height - 1)] {
            let salida = banco.clic(&image, r, true, PointerButton::Primary, punto);
            assert_eq!(salida.indicator, Some(Indicator::Dropper), "ppp {ppp}");
            assert_eq!(salida.choice, None);
        }
    }
}

#[test]
fn fila_6_clic_derecho_lleva_el_foco_al_editor_con_el_rol_del_rectangulo_pulsado() {
    let image = indexada(16);
    let mut banco = Banco::nuevo(1.0);
    let (fg, bg) = indicator_rects(1);
    let r = roles(ColorChoice::Entry(2), ColorChoice::Entry(3));
    let salida = banco.clic(&image, r, true, PointerButton::Secondary, centro(&fg));
    assert_eq!(
        salida.indicator,
        Some(Indicator::FocusEditor(Role::Foreground))
    );
    let salida = banco.clic(
        &image,
        r,
        true,
        PointerButton::Secondary,
        (bg.x + bg.width - 1, bg.y + bg.height - 1),
    );
    assert_eq!(
        salida.indicator,
        Some(Indicator::FocusEditor(Role::Background))
    );
    // El foco está en el campo hexadecimal del editor.
    banco.fotograma(&image, r, true, vec![]);
    assert_eq!(banco.ctx.memory(|m| m.focused()), Some(hex_field_id()));
}

#[test]
fn fila_6_el_clic_en_el_hueco_de_la_banda_o_fuera_no_hace_nada_y_enabled_falso_tampoco() {
    let image = indexada(16);
    let mut banco = Banco::nuevo(1.0);
    let (fg, bg) = indicator_rects(1);
    let r = roles(ColorChoice::Entry(2), ColorChoice::Entry(3));
    let esquina_vacia = (bg.x + bg.width - 1, fg.y);
    for button in [PointerButton::Primary, PointerButton::Secondary] {
        let salida = banco.clic(&image, r, true, button, esquina_vacia);
        assert_eq!(salida.indicator, None, "{button:?}");
        assert_eq!(salida.editing, None);
        let salida = banco.clic(&image, r, false, button, centro(&fg));
        assert_eq!(salida.indicator, None, "{button:?} deshabilitado");
    }
}

#[test]
fn fila_6_los_selectores_cambian_el_destino_del_editor_y_se_excluyen() {
    let image = indexada(16);
    let mut banco = Banco::nuevo(1.0);
    let r = roles(ColorChoice::Entry(2), ColorChoice::Entry(3));
    for rol in [Role::Background, Role::Foreground] {
        let fila = selector_rect(rol, 1);
        let salida = banco.clic(&image, r, true, PointerButton::Primary, centro(&fila));
        assert_eq!(salida.editing, Some(rol));
        assert_eq!(salida.indicator, None);
    }
}

#[test]
fn fila_6_el_pequeno_se_pinta_despues_que_el_grande_y_con_sus_colores() {
    let image = indexada(16);
    let mut banco = Banco::nuevo(1.0);
    let r = roles(ColorChoice::Entry(2), ColorChoice::Entry(3));
    let f = banco.fotograma(&image, r, true, vec![]);
    let (fg, bg) = indicator_rects(1);
    let paleta = image.palette().expect("paleta");
    let color = |i: u8| {
        let c = paleta.resolved(i).expect("entrada");
        Color32::from_rgba_unmultiplied(c.r, c.g, c.b, c.a)
    };
    let todos = rectangulos(&f.formas);
    let pos_de = |rect: Rect, fill: Color32| {
        todos
            .iter()
            .position(|(r, c)| *r == rect && *c == fill)
            .unwrap_or_else(|| panic!("no se pinta {rect:?} {fill:?}"))
    };
    let p_bg = pos_de(a_rect(bg), color(3));
    let p_fg = pos_de(a_rect(fg), color(2));
    assert!(p_fg > p_bg, "el pequeño va encima");
}
