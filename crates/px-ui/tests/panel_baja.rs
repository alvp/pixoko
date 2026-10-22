//! Tests del panel de paletas con la ventana baja, dentro de la ventana
//! (specs/hito-2-panel-ventana-baja.md, tabla «Casos», filas 3, 4, 5, 6, 7, 8, 9 y 11 en su parte
//! de banco). Lo puro (`panel_layout`, `PanelView::scroll_by`, `panel_thumb`, `Scroll::reveal`,
//! `ColorEditor::confirm_hex_checked`) se prueba en `edit_layout.rs`, `panel_view.rs` y
//! `color_edit.rs`; aquí se prueba que `show_editing_view` lo une bien a `egui`.
//!
//! El banco es el de `tests/color_editor.rs`, copiado y adaptado (ese fichero no se toca): un
//! `egui::Context` ejecuta fotogramas con `RawInput` sintético, sin GPU ni pantalla. En cada
//! fotograma, como hará la app, se llama a `ColorEditor::sync`, se monta `status::show`, luego
//! `palette_panel::show_editing_view` con un `PanelView` que **persiste entre fotogramas** y por
//! fin el lienzo. `enabled` es `!editor.dragging()`.
//!
//! La imagen del banco sale de un `Document` real: `aplica`, `deshace` y `rehace` mutan la imagen
//! con `Command::SetPaletteColor`, `AddPaletteEntry` o `RemovePaletteEntry`, como lo haría la
//! app. El banco **nunca aplica** lo que devuelve el panel (la confirmación es de `Session`, en
//! `px-app`): cada `Commit` se guarda en `banco.historial`.
//!
//! Se mide a `ppp` 1,0 y 2,0 (`n` = 1 y 2). Los altos del panel son múltiplos de `n`, así que
//! `top_px = alto − H·n` cuando se desplaza al máximo. La rueda se manda en puntos
//! (`MouseWheelUnit::Point`): `delta.y = −px / ppp` desplaza `px` píxeles físicos hacia abajo.
//!
//! Lo pintado se lee de las formas del fotograma. «Todo lo pintado queda dentro del panel» se
//! comprueba con el `clip_rect` de cada `ClippedShape`. La barra de desplazamiento del panel son
//! los rectángulos de `x = TRACK_X_U·u` (el de `palette_grid`) y ancho `TRACK_W_U·u`: la pista
//! (alto de todo el panel, desde su borde superior) y el indicador `TEXT_DIM` de `panel_thumb`.
//! No hay capturas de pantalla.

use egui::epaint::{ClippedShape, Shape};
use egui::{
    Color32, Context, Event, Key, MissingGlyphPolicy, Modifiers, MouseWheelSource, MouseWheelUnit,
    PointerButton, Pos2, RawInput, Rect, TouchPhase, Vec2, pos2, vec2,
};
use px_core::{Command, Document, Image, Mode, Palette, Rgba};
use px_ui::canvas::Canvas;
use px_ui::color::{ColorChoice, SlotColors, cell_of_choice};
use px_ui::color_edit::{
    ColorEditor, Commit, Control, EditTarget, StaleReason, hex_notice, hex_text, slider_value_at,
    slider_x, stale_notice,
};
use px_ui::edit_layout::{
    FIELD_H_U, FIELD_W_U, FIELD_X_U, HEADER_Y_U, PanelLayout, ROW_H_U, TRACK_W_U, TRACK_X_U,
    block_height_px, control_row_u, editor_height_u, field_y_u, panel_layout,
};
use px_ui::palette_grid::{
    self, CELL_U, COLUMNS, GRID_X_U, GRID_Y_U, PANEL_WIDTH_U, PITCH_U, PanelView, panel_thumb,
};
use px_ui::palette_grid::{TRACK_W_U as BARRA_W_U, TRACK_X_U as BARRA_X_U};
use px_ui::palette_panel::{self, PanelOutput};
use px_ui::status::bar_height_px;
use px_ui::text::{apply_scale, install_fonts, text_scale};
use px_ui::theme::TEXT_DIM;

#[path = "common/zonas.rs"]
mod zonas;
use zonas::panel_a_la_izquierda;

// ---------------------------------------------------------------------------------------------
// Imágenes de prueba
// ---------------------------------------------------------------------------------------------

const ROJO: Rgba = Rgba::opaque(255, 0, 0);
const VERDE: Rgba = Rgba::opaque(0, 255, 0);
const AZUL: Rgba = Rgba::opaque(0, 0, 255);
const NEGRO: Rgba = Rgba::opaque(0, 0, 0);
const AMARILLO: Rgba = Rgba::opaque(255, 255, 0);

const PPPS: [f32; 2] = [1.0, 2.0];

/// Indexada de 64×48 con la paleta `[R, G, B, K]`.
fn imagen_f() -> Image {
    let paleta = Palette::from_colors([ROJO, VERDE, AZUL, NEGRO]).expect("paleta");
    Image::new_indexed(64, 48, paleta).expect("imagen indexada")
}

/// Indexada de 64×48 con 256 colores (16 filas de la rejilla).
fn imagen_indexada_256() -> Image {
    let colores: Vec<Rgba> = (0..256usize)
        .map(|i| Rgba::opaque(i as u8, 255 - i as u8, (i * 7) as u8))
        .collect();
    let paleta = Palette::from_colors(colores).expect("paleta de 256 colores");
    Image::new_indexed(64, 48, paleta).expect("imagen indexada")
}

fn imagen_rgba() -> Image {
    Image::new(64, 48, Mode::Rgba).expect("imagen RGBA")
}

// ---------------------------------------------------------------------------------------------
// Eventos sintéticos
// ---------------------------------------------------------------------------------------------

fn boton(pos: Pos2, pressed: bool) -> Event {
    Event::PointerButton {
        pos,
        button: PointerButton::Primary,
        pressed,
        modifiers: Modifiers::NONE,
    }
}

fn texto(s: &str) -> Event {
    Event::Text(s.to_owned())
}

fn tecla_ctrl_a() -> Event {
    Event::Key {
        key: Key::A,
        physical_key: Some(Key::A),
        pressed: true,
        repeat: false,
        modifiers: Modifiers {
            ctrl: true,
            command: true,
            ..Modifiers::NONE
        },
    }
}

fn en_puntos(ppp: f32, x_px: i64, y_px: i64) -> Pos2 {
    pos2(x_px as f32 / ppp, y_px as f32 / ppp)
}

// ---------------------------------------------------------------------------------------------
// Lo pintado
// ---------------------------------------------------------------------------------------------

#[derive(Clone, Debug)]
struct Pintado {
    texto: String,
    pos: Pos2,
}

fn recoge(shape: &Shape, textos: &mut Vec<Pintado>, rects: &mut Vec<(Rect, Color32)>) {
    match shape {
        Shape::Text(t) => textos.push(Pintado {
            texto: t.galley.text().to_owned(),
            pos: t.pos,
        }),
        Shape::Rect(r) => rects.push((r.rect, r.fill)),
        Shape::Vec(formas) => {
            for forma in formas {
                recoge(forma, textos, rects);
            }
        }
        _ => {}
    }
}

// ---------------------------------------------------------------------------------------------
// Banco
// ---------------------------------------------------------------------------------------------

const PANTALLA_ALTA: Vec2 = vec2(800.0, 600.0);

struct Banco {
    ctx: Context,
    canvas: Canvas,
    scroll: palette_grid::Scroll,
    view: PanelView,
    pantalla: Vec2,
    documento: Document,
    image: Image,
    choice: ColorChoice,
    slots: SlotColors,
    editor: ColorEditor,
    /// Lo que devolvió `show_editing_view` en cada fotograma, en orden.
    historial: Vec<PanelOutput>,
}

struct Fotograma {
    rect_lienzo: Rect,
    salida: PanelOutput,
    textos: Vec<Pintado>,
    rects: Vec<(Rect, Color32)>,
    formas: Vec<ClippedShape>,
}

impl Fotograma {
    fn pinta(&self, buscado: &str) -> bool {
        self.textos.iter().any(|t| t.texto == buscado)
    }

    fn pinta_algo_con(&self, buscado: &str) -> bool {
        self.textos.iter().any(|t| t.texto.contains(buscado))
    }

    fn texto_pintado(&self, buscado: &str) -> Option<&Pintado> {
        self.textos.iter().find(|t| t.texto == buscado)
    }
}

/// Medidas del editor a la escala del banco, en píxeles físicos desde el borde superior del panel.
#[derive(Clone, Copy, Debug)]
struct Geo {
    n: i64,
    mode: Mode,
    top_px: i64,
}

impl Geo {
    /// `y` del centro de la fila del deslizador `control`.
    fn y_fila(&self, control: Control) -> i64 {
        let fila = control_row_u(self.mode, control).expect("el modo tiene ese deslizador");
        self.top_px + fila * self.n + ROW_H_U * self.n / 2
    }

    fn x_pista(&self) -> i64 {
        TRACK_X_U * self.n
    }

    fn ancho_pista(&self) -> i64 {
        TRACK_W_U * self.n
    }

    fn x_de_valor(&self, valor: u32, max: u32) -> i64 {
        self.x_pista() + slider_x(valor, self.ancho_pista(), max)
    }

    fn valor_en(&self, dx: i64, control: Control) -> u32 {
        slider_value_at(dx, self.ancho_pista(), control.max())
    }

    /// Centro del campo hexadecimal.
    fn campo(&self) -> (i64, i64) {
        (
            FIELD_X_U * self.n + FIELD_W_U * self.n / 2,
            self.top_px + field_y_u(self.mode) * self.n + FIELD_H_U * self.n / 2,
        )
    }
}

/// Centro de la muestra `indice` en píxeles físicos con el bloque sin desplazar.
fn centro_de_muestra(n: i64, indice: usize, primera_fila: usize) -> (i64, i64) {
    let columna = (indice % COLUMNS) as i64;
    let fila = (indice / COLUMNS) as i64 - primera_fila as i64;
    (
        (GRID_X_U + columna * PITCH_U) * n + CELL_U * n / 2,
        (GRID_Y_U + fila * PITCH_U) * n + CELL_U * n / 2,
    )
}

fn pantalla_para_panel(ppp: f32, panel_h_px: i64) -> Vec2 {
    let n = text_scale(ppp);
    vec2(
        800.0,
        (panel_h_px + i64::from(bar_height_px(n))) as f32 / ppp,
    )
}

impl Banco {
    /// Contexto de `pantalla` puntos a `ppp`, con la fuente instalada, `image` en un `Document`
    /// y, si `calentar`, dos fotogramas de reposo del panel (sin ellos el primer fotograma del
    /// panel es el primero que se llama a `show_editing_view`).
    fn nuevo_con(
        ppp: f32,
        pantalla: Vec2,
        image: Image,
        choice: ColorChoice,
        calentar: bool,
    ) -> Banco {
        let ctx = Context::default();
        ctx.set_pixels_per_point(ppp);
        ctx.set_missing_glyph_policy(MissingGlyphPolicy::Panic);
        install_fonts(&ctx);
        let mut banco = Banco {
            ctx,
            canvas: Canvas::new(),
            scroll: palette_grid::Scroll::default(),
            view: PanelView::default(),
            pantalla,
            documento: Document::new(image.clone()),
            slots: SlotColors::defaults(image.mode()),
            image,
            choice,
            editor: ColorEditor::default(),
            historial: Vec::new(),
        };
        for _ in 0..2 {
            let salida = banco.ctx.run_ui(banco.entrada(vec![]), |ui| {
                apply_scale(ui.ctx());
            });
            salida.drop_without_applying_deltas();
        }
        banco.canvas.set_image(&banco.image);
        if calentar {
            banco.reposo();
            banco.reposo();
            banco.historial.clear();
        }
        banco
    }

    fn nuevo(ppp: f32, pantalla: Vec2, image: Image, choice: ColorChoice) -> Banco {
        Banco::nuevo_con(ppp, pantalla, image, choice, true)
    }

    fn con_alto_de_panel(ppp: f32, panel_h_px: i64, image: Image, choice: ColorChoice) -> Banco {
        Banco::nuevo(ppp, pantalla_para_panel(ppp, panel_h_px), image, choice)
    }

    /// Cambia el alto del panel (la ventana) para el siguiente fotograma.
    fn alto_de_panel(&mut self, panel_h_px: i64) {
        self.pantalla = pantalla_para_panel(self.ppp(), panel_h_px);
    }

    fn entrada(&self, events: Vec<Event>) -> RawInput {
        RawInput {
            screen_rect: Some(Rect::from_min_size(Pos2::ZERO, self.pantalla)),
            events,
            ..RawInput::default()
        }
    }

    fn ppp(&self) -> f32 {
        self.ctx.pixels_per_point()
    }

    fn n(&self) -> i64 {
        i64::from(text_scale(self.ppp()))
    }

    fn panel_h_px(&self) -> i64 {
        (self.pantalla.y * self.ppp()).round() as i64 - i64::from(bar_height_px(self.n() as u32))
    }

    /// El panel, en puntos.
    fn rect_panel(&self) -> Rect {
        let u = self.n() as f32 / self.ppp();
        Rect::from_min_size(
            Pos2::ZERO,
            vec2(
                PANEL_WIDTH_U as f32 * u,
                self.panel_h_px() as f32 / self.ppp(),
            ),
        )
    }

    /// El diseño del panel con el desplazamiento que tiene la vista ahora.
    fn disposicion(&self) -> PanelLayout {
        panel_layout(
            self.image.mode(),
            self.panel_h_px(),
            self.n() as u32,
            self.view.offset_px,
        )
    }

    /// El máximo desplazamiento posible con el alto actual.
    fn max_offset(&self) -> i64 {
        panel_layout(self.image.mode(), self.panel_h_px(), self.n() as u32, 0).max_offset_px
    }

    fn geo(&self) -> Geo {
        Geo {
            n: self.n(),
            mode: self.image.mode(),
            top_px: self.disposicion().top_px,
        }
    }

    fn actual(&self) -> Option<(EditTarget, Rgba)> {
        let destino = EditTarget::of(&self.image, self.choice)?;
        let color = destino.stored(&self.image, &self.slots)?;
        Some((destino, color))
    }

    fn fotograma(&mut self, events: Vec<Event>) -> Fotograma {
        let actual = self.actual();
        self.editor.sync(actual);
        let enabled = !self.editor.dragging();
        let input = self.entrada(events);
        let mut resultado = None;
        let Banco {
            ctx,
            canvas,
            scroll,
            view,
            image,
            choice,
            slots,
            editor,
            ..
        } = self;
        let mut salida = ctx.run_ui(input, |ui| {
            apply_scale(ui.ctx());
            px_ui::status::show(ui, "lápiz · color 1 #000000", false);
            let (panel, rect_lienzo) = panel_a_la_izquierda(
                ui,
                |ui| {
                    palette_panel::show_editing_view(
                        ui, image, *choice, scroll, view, enabled, slots, editor,
                    )
                },
                |ui| {
                    let rect_lienzo = ui.available_rect_before_wrap();
                    let _ = canvas
                        .interact_roles(ui)
                        .into_iter()
                        .map(|(input, _)| input)
                        .collect::<Vec<_>>();
                    let _ = canvas.paint(ui);
                    rect_lienzo
                },
            );
            resultado = Some((rect_lienzo, panel));
        });
        salida.textures_delta.clear();
        let formas: Vec<ClippedShape> = std::mem::take(&mut salida.shapes);
        let (rect_lienzo, panel) = resultado.expect("el lienzo se dibuja");
        let (mut textos, mut rects) = (Vec::new(), Vec::new());
        for clipped in &formas {
            recoge(&clipped.shape, &mut textos, &mut rects);
        }
        self.historial.push(panel.clone());
        Fotograma {
            rect_lienzo,
            salida: panel,
            textos,
            rects,
            formas,
        }
    }

    fn reposo(&mut self) -> Fotograma {
        self.fotograma(vec![])
    }

    fn quiere_teclado(&self) -> bool {
        self.ctx.text_edit_focused()
    }

    // --- Ratón ---

    fn mueve(&mut self, x_px: i64, y_px: i64) -> Fotograma {
        let pos = en_puntos(self.ppp(), x_px, y_px);
        self.fotograma(vec![Event::PointerMoved(pos)])
    }

    fn pulsa(&mut self, x_px: i64, y_px: i64) -> Fotograma {
        let pos = en_puntos(self.ppp(), x_px, y_px);
        self.fotograma(vec![boton(pos, true)])
    }

    fn suelta(&mut self, x_px: i64, y_px: i64) -> Fotograma {
        let pos = en_puntos(self.ppp(), x_px, y_px);
        self.fotograma(vec![boton(pos, false)])
    }

    fn clic(&mut self, x_px: i64, y_px: i64) {
        self.mueve(x_px, y_px);
        self.pulsa(x_px, y_px);
        self.suelta(x_px, y_px);
    }

    fn clic_en_el_lienzo(&mut self) {
        let centro = self.reposo().rect_lienzo.center();
        let (x, y) = (
            (centro.x * self.ppp()).round() as i64,
            (centro.y * self.ppp()).round() as i64,
        );
        self.clic(x, y);
    }

    /// La rueda, `px_abajo` píxeles físicos hacia abajo, en el punto donde esté el puntero.
    fn rueda(&mut self, px_abajo: i64) -> Fotograma {
        let delta = vec2(0.0, -(px_abajo as f32) / self.ppp());
        self.fotograma(vec![Event::MouseWheel {
            unit: MouseWheelUnit::Point,
            delta,
            phase: TouchPhase::Move,
            source: MouseWheelSource::Unknown,
            modifiers: Modifiers::NONE,
        }])
    }

    /// Mueve el puntero a `(x, y)` y gira la rueda `px_abajo` píxeles físicos.
    fn rueda_en(&mut self, x_px: i64, y_px: i64, px_abajo: i64) -> Fotograma {
        self.mueve(x_px, y_px);
        self.rueda(px_abajo)
    }

    /// Un punto del panel dentro del editor (su última fila de píxeles) para poner el puntero.
    fn sobre_el_editor(&self) -> (i64, i64) {
        (100 * self.n(), self.panel_h_px() - self.n())
    }

    /// Un punto del panel sobre el texto de arriba.
    fn sobre_el_texto(&self) -> (i64, i64) {
        (10 * self.n(), 10 * self.n())
    }

    // --- Campo hexadecimal ---

    fn enfoca_campo(&mut self) {
        let (x, y) = self.geo().campo();
        self.clic(x, y);
        self.reposo();
        assert!(
            self.quiere_teclado(),
            "un clic en el campo le da el foco de texto"
        );
    }

    fn escribe(&mut self, cadena: &str) -> Fotograma {
        self.fotograma(vec![tecla_ctrl_a()]);
        self.fotograma(vec![texto(cadena)])
    }

    // --- Documento ---

    fn refresca(&mut self) {
        let id = self.documento.main_image();
        self.image = self.documento.image(id).expect("la imagen").clone();
    }

    fn aplica(&mut self, command: Command) {
        self.documento.apply(command).expect("comando válido");
        self.refresca();
    }

    fn deshace(&mut self) {
        self.documento.undo().expect("hay algo que deshacer");
        self.refresca();
    }

    fn rehace(&mut self) {
        self.documento.redo().expect("hay algo que rehacer");
        self.refresca();
    }

    fn pone_color(&mut self, index: u8, color: Rgba) {
        let image = self.documento.main_image();
        self.aplica(Command::SetPaletteColor {
            image,
            index,
            color,
        });
    }

    // --- Lo recogido ---

    fn commits(&self) -> Vec<Commit> {
        self.historial.iter().filter_map(|o| o.commit).collect()
    }

    fn avisos(&self) -> Vec<String> {
        self.historial
            .iter()
            .filter_map(|o| o.notice.clone())
            .collect()
    }

    fn elecciones(&self) -> Vec<ColorChoice> {
        self.historial.iter().filter_map(|o| o.choice).collect()
    }
}

/// Banco con la imagen `F` y un panel de 700·n (cabe todo), elegida la entrada `entrada`.
fn banco_f(ppp: f32, entrada: u8) -> Banco {
    Banco::nuevo(ppp, PANTALLA_ALTA, imagen_f(), ColorChoice::Entry(entrada))
}

// ---------------------------------------------------------------------------------------------
// Comprobaciones de lo pintado
// ---------------------------------------------------------------------------------------------

const TOLERANCIA: f32 = 0.05;

fn cerca(a: f32, b: f32) -> bool {
    (a - b).abs() <= TOLERANCIA
}

/// Todo lo pintado que cae sobre el panel se recorta al panel: el `clip_rect` de cada forma que
/// asoma en él está dentro del panel. Lo de la barra de estado y el lienzo no se solapa con él.
fn comprueba_dentro_del_panel(f: &Fotograma, panel: Rect, contexto: &str) {
    let panel = panel.expand(TOLERANCIA);
    for forma in &f.formas {
        let visible = forma
            .shape
            .visual_bounding_rect()
            .intersect(forma.clip_rect);
        let solape = visible.intersect(panel);
        if !(visible.is_positive()
            && solape.width() > 2.0 * TOLERANCIA
            && solape.height() > 2.0 * TOLERANCIA)
        {
            continue;
        }
        assert!(
            panel.contains_rect(forma.clip_rect),
            "{contexto}: una forma que cae en el panel se recorta a {:?}, fuera del panel {panel:?}",
            forma.clip_rect
        );
        assert!(
            panel.contains_rect(visible),
            "{contexto}: se ve {visible:?} y el panel es {panel:?}"
        );
    }
}

/// Rectángulos con la `x` y el ancho de la barra de desplazamiento del panel.
fn rects_de_la_barra(f: &Fotograma, ppp: f32, n: i64) -> Vec<(Rect, Color32)> {
    let x = (BARRA_X_U * n) as f32 / ppp;
    let ancho = (BARRA_W_U * n) as f32 / ppp;
    f.rects
        .iter()
        .filter(|(r, _)| cerca(r.min.x, x) && cerca(r.width(), ancho))
        .copied()
        .collect()
}

/// La pista de la barra: desde el borde superior del panel y con todo su alto.
fn pistas_de_la_barra(f: &Fotograma, banco: &Banco) -> Vec<Rect> {
    let alto = banco.panel_h_px() as f32 / banco.ppp();
    rects_de_la_barra(f, banco.ppp(), banco.n())
        .into_iter()
        .map(|(r, _)| r)
        .filter(|r| cerca(r.min.y, 0.0) && cerca(r.height(), alto))
        .collect()
}

/// La barra solo se pinta con desplazamiento, con el indicador de `panel_thumb`; sin él no hay
/// pista del panel. Con desplazamiento, la rejilla no pinta el suyo: solo hay pista e indicador.
fn comprueba_barra(f: &Fotograma, banco: &Banco, contexto: &str) {
    let disposicion = banco.disposicion();
    let pistas = pistas_de_la_barra(f, banco);
    if !disposicion.scrolling {
        assert!(
            pistas.is_empty(),
            "{contexto}: sin desplazamiento no hay pista"
        );
        return;
    }
    let (ppp, n) = (banco.ppp(), banco.n());
    let bloque = block_height_px(banco.image.mode(), n as u32);
    let (y, alto) = panel_thumb(disposicion.offset_px, banco.panel_h_px(), bloque, n as u32)
        .expect("con desplazamiento hay indicador");
    assert_eq!(pistas.len(), 1, "{contexto}: una pista del alto del panel");
    let barra = rects_de_la_barra(f, ppp, n);
    assert_eq!(
        barra.len(),
        2,
        "{contexto}: solo la pista y el indicador (la rejilla no pinta el suyo): {barra:?}"
    );
    let indicadores: Vec<&Rect> = barra
        .iter()
        .filter(|(_, fill)| *fill == TEXT_DIM)
        .map(|(r, _)| r)
        .collect();
    assert_eq!(indicadores.len(), 1, "{contexto}: un indicador TEXT_DIM");
    let indicador = indicadores[0];
    assert!(
        cerca(indicador.min.y, y as f32 / ppp) && cerca(indicador.height(), alto as f32 / ppp),
        "{contexto}: indicador en {indicador:?} y `panel_thumb` da ({y}, {alto}) px"
    );
}

// ---------------------------------------------------------------------------------------------
// Fila 3: el editor se ve siempre y la rejilla tiene una fila
// ---------------------------------------------------------------------------------------------

#[test]
fn fila_3_el_editor_se_ve_siempre_y_la_rejilla_tiene_una_fila() {
    for ppp in PPPS {
        let n = i64::from(text_scale(ppp));
        for alto in [277 * n, 276 * n, 200 * n, 100 * n] {
            let contexto = format!("ppp {ppp}, alto {alto}");
            let mut banco =
                Banco::con_alto_de_panel(ppp, alto, imagen_indexada_256(), ColorChoice::Entry(5));
            let f = banco.reposo();
            let geo = banco.geo();
            assert_eq!(geo.top_px, 91 * n, "{contexto}: top_px con offset 0");
            // El editor se pinta, en su sitio.
            let cabecera = f
                .texto_pintado("editar índice 5")
                .unwrap_or_else(|| panic!("{contexto}: sin cabecera"));
            assert!(
                cerca(cabecera.pos.y * ppp, (geo.top_px + HEADER_Y_U * n) as f32),
                "{contexto}: cabecera en y = {}",
                cabecera.pos.y * ppp
            );
            // Una fila de rejilla.
            assert_eq!(banco.view.last_visible, Some(1), "{contexto}");
            let (x, y) = centro_de_muestra(n, 0, 0);
            banco.clic(x, y);
            let (x, y) = centro_de_muestra(n, COLUMNS, 0);
            banco.clic(x, y);
            assert_eq!(
                banco.elecciones(),
                vec![ColorChoice::Entry(0)],
                "{contexto}: la fila 1 se elige y la 2 no se ve"
            );
        }
    }
}

#[test]
fn fila_3_de_305n_a_304n_el_editor_y_la_rejilla_no_saltan() {
    for ppp in PPPS {
        let n = i64::from(text_scale(ppp));
        let mut banco =
            Banco::con_alto_de_panel(ppp, 305 * n, imagen_indexada_256(), ColorChoice::Entry(5));
        let antes = banco.reposo();
        let cabecera_antes = antes
            .texto_pintado("editar índice 5")
            .expect("cabecera")
            .pos;
        assert!(!banco.disposicion().scrolling);

        banco.alto_de_panel(304 * n);
        let despues = banco.reposo();
        assert!(banco.disposicion().scrolling, "ppp {ppp}: 304n desplaza");
        assert_eq!(banco.view.offset_px, 0);
        let cabecera_despues = despues
            .texto_pintado("editar índice 5")
            .expect("cabecera")
            .pos;
        assert!(
            cerca(cabecera_antes.y, cabecera_despues.y)
                && cerca(cabecera_antes.x, cabecera_despues.x),
            "ppp {ppp}: la cabecera salta de {cabecera_antes:?} a {cabecera_despues:?}"
        );
        assert_eq!(banco.view.last_visible, Some(1));
    }
}

#[test]
fn fila_3_con_offset_maximo_el_editor_queda_anclado_al_fondo() {
    for ppp in PPPS {
        let n = i64::from(text_scale(ppp));
        for alto in [276 * n, 200 * n, 100 * n] {
            let mut banco =
                Banco::con_alto_de_panel(ppp, alto, imagen_indexada_256(), ColorChoice::Entry(5));
            let (x, y) = banco.sobre_el_editor();
            banco.rueda_en(x, y, 100_000 * n);
            let bloque = block_height_px(Mode::Indexed, n as u32);
            let max = ((bloque - alto) / n) * n;
            assert_eq!(banco.view.offset_px, max, "ppp {ppp}, alto {alto}");
            assert_eq!(banco.max_offset(), max);
            assert_eq!(
                banco.geo().top_px,
                alto - editor_height_u(Mode::Indexed) * n,
                "ppp {ppp}, alto {alto}"
            );
        }
    }
}

// ---------------------------------------------------------------------------------------------
// Fila 4: la rueda mueve el panel, no las filas
// ---------------------------------------------------------------------------------------------

#[test]
fn fila_4_la_rueda_sobre_el_texto_y_sobre_el_editor_mueve_offset_px_y_no_las_filas() {
    for ppp in PPPS {
        let n = i64::from(text_scale(ppp));
        let mut banco =
            Banco::con_alto_de_panel(ppp, 200 * n, imagen_indexada_256(), ColorChoice::Entry(5));
        assert!(banco.disposicion().scrolling);
        assert_eq!(banco.view.offset_px, 0);

        // Sobre el texto y la rejilla.
        let (x, y) = banco.sobre_el_texto();
        banco.rueda_en(x, y, 10 * n);
        assert_eq!(banco.view.offset_px, 10 * n, "ppp {ppp}: sobre el texto");
        // Sobre el editor.
        let (x, y) = banco.sobre_el_editor();
        banco.rueda_en(x, y, 5 * n);
        assert_eq!(banco.view.offset_px, 15 * n, "ppp {ppp}: sobre el editor");
        // Hacia arriba.
        banco.rueda(-(6 * n));
        assert_eq!(banco.view.offset_px, 9 * n, "ppp {ppp}: hacia arriba");

        // Las filas de la rejilla no se mueven.
        assert_eq!(banco.scroll.first_row, 0, "ppp {ppp}");
        assert_eq!(banco.scroll.rest, 0, "ppp {ppp}");
        // Sin eventos el desplazamiento se queda donde está.
        for _ in 0..3 {
            banco.reposo();
        }
        assert_eq!(banco.view.offset_px, 9 * n, "ppp {ppp}");
        assert_eq!(banco.scroll.first_row, 0);
    }
}

#[test]
fn fila_4_offset_px_siempre_es_multiplo_de_n_y_se_recorta_a_cero_y_al_maximo() {
    for ppp in PPPS {
        let n = i64::from(text_scale(ppp));
        let mut banco =
            Banco::con_alto_de_panel(ppp, 200 * n, imagen_indexada_256(), ColorChoice::Entry(5));
        let max = banco.max_offset();
        assert_eq!(max, 105 * n);
        let (x, y) = banco.sobre_el_texto();
        banco.mueve(x, y);
        // Subir con el desplazamiento a 0: se queda en 0.
        banco.rueda(-(50 * n));
        assert_eq!(banco.view.offset_px, 0);
        // Pasos que no son múltiplo de `n` (en `ppp` 2 un píxel suelto), desde el interior para
        // no mezclarlos con el resto del extremo superior: se acumulan en el resto.
        banco.rueda(10 * n);
        assert_eq!(banco.view.offset_px, 10 * n);
        for px in [1, 1, 1, 3, 5, 7] {
            banco.rueda(px);
            assert_eq!(banco.view.offset_px % n, 0, "ppp {ppp}: tras bajar {px} px");
            assert!(banco.view.rest_px >= 0 && banco.view.rest_px < n);
        }
        // 1 + 1 + 1 + 3 + 5 + 7 = 18 px.
        assert_eq!(
            banco.view.offset_px + banco.view.rest_px,
            10 * n + 18,
            "ppp {ppp}: no se pierde nada"
        );
        // Bajar de más: el máximo, sin resto.
        banco.rueda(100_000 * n);
        assert_eq!(banco.view.offset_px, max);
        assert_eq!(banco.view.rest_px, 0);
        banco.rueda(n);
        assert_eq!(banco.view.offset_px, max);
        // Y subir de más: el cero.
        banco.rueda(-(100_000 * n));
        assert_eq!(banco.view.offset_px, 0);
    }
}

#[test]
fn fila_4_la_rueda_fuera_del_panel_no_mueve_el_panel() {
    for ppp in PPPS {
        let n = i64::from(text_scale(ppp));
        let mut banco =
            Banco::con_alto_de_panel(ppp, 200 * n, imagen_indexada_256(), ColorChoice::Entry(5));
        let centro = banco.reposo().rect_lienzo.center();
        let (x, y) = (
            (centro.x * ppp).round() as i64,
            (centro.y * ppp).round() as i64,
        );
        banco.rueda_en(x, y, 10 * n);
        assert_eq!(banco.view.offset_px, 0, "ppp {ppp}: sobre el lienzo");
        // Ni sobre la barra de estado.
        banco.rueda_en(50 * n, 210 * n, 10 * n);
        assert_eq!(
            banco.view.offset_px, 0,
            "ppp {ppp}: sobre la barra de estado"
        );
    }
}

#[test]
fn fila_4_con_un_arrastre_en_curso_la_rueda_no_mueve_el_panel() {
    for ppp in PPPS {
        let n = i64::from(text_scale(ppp));
        let mut banco = Banco::con_alto_de_panel(ppp, 200 * n, imagen_f(), ColorChoice::Entry(1));
        let geo = banco.geo();
        let y = geo.y_fila(Control::Red);
        let x = geo.x_de_valor(128, Control::Red.max());
        banco.mueve(x, y);
        banco.pulsa(x, y);
        assert!(banco.editor.dragging(), "ppp {ppp}: el arrastre empieza");
        banco.rueda(10 * n);
        assert_eq!(
            banco.view.offset_px, 0,
            "ppp {ppp}: la rueda no mueve el panel"
        );
        assert!(
            banco.editor.dragging(),
            "ppp {ppp}: y no cancela el arrastre"
        );
        // Al soltar, la rueda vuelve a valer.
        banco.suelta(x, y);
        banco.rueda(10 * n);
        assert_eq!(banco.view.offset_px, 10 * n, "ppp {ppp}");
    }
}

#[test]
fn fila_4_al_agrandar_hasta_el_bloque_offset_px_es_0_en_el_mismo_fotograma() {
    for ppp in PPPS {
        let n = i64::from(text_scale(ppp));
        let mut banco =
            Banco::con_alto_de_panel(ppp, 200 * n, imagen_indexada_256(), ColorChoice::Entry(5));
        let (x, y) = banco.sobre_el_texto();
        banco.rueda_en(x, y, 100_000 * n);
        assert_eq!(banco.view.offset_px, 105 * n);

        // Agrandar un poco: el máximo baja y el desplazamiento se recorta en el mismo fotograma.
        banco.alto_de_panel(220 * n);
        banco.reposo();
        assert_eq!(banco.view.offset_px, 85 * n, "ppp {ppp}: 220n");
        assert_eq!(banco.max_offset(), 85 * n);

        // Hasta el bloque: 0 en el mismo fotograma, sin pista y con el editor en su sitio.
        banco.alto_de_panel(305 * n);
        let f = banco.reposo();
        assert_eq!(banco.view.offset_px, 0, "ppp {ppp}: 305n");
        assert!(!banco.disposicion().scrolling);
        comprueba_barra(&f, &banco, &format!("ppp {ppp}, 305n"));
        let cabecera = f.texto_pintado("editar índice 5").expect("cabecera");
        assert!(cerca(
            cabecera.pos.y * ppp,
            (91 * n + HEADER_Y_U * n) as f32
        ));

        // Y mucho más alto: el diseño normal, sin desplazamiento posible.
        banco.alto_de_panel(700 * n);
        banco.reposo();
        assert_eq!(banco.view.offset_px, 0);
        assert_eq!(banco.view.rest_px, 0);
    }
}

#[test]
fn fila_4_la_barra_solo_se_pinta_con_desplazamiento_y_la_rejilla_no_pinta_la_suya() {
    for ppp in PPPS {
        let n = i64::from(text_scale(ppp));
        for (alto, desplaza) in [
            (700 * n, false),
            (308 * n, false),
            (305 * n, false),
            (304 * n, true),
            (200 * n, true),
            (100 * n, true),
        ] {
            let contexto = format!("ppp {ppp}, alto {alto}");
            let mut banco =
                Banco::con_alto_de_panel(ppp, alto, imagen_indexada_256(), ColorChoice::Entry(5));
            let f = banco.reposo();
            assert_eq!(banco.disposicion().scrolling, desplaza, "{contexto}");
            comprueba_barra(&f, &banco, &contexto);
            if !desplaza {
                continue;
            }
            // En reposo, a media altura y abajo del todo.
            let max = banco.max_offset();
            let (x, y) = banco.sobre_el_texto();
            banco.mueve(x, y);
            for objetivo in [max / 2 / n * n, max] {
                banco.rueda(-100_000 * n);
                banco.rueda(objetivo);
                assert_eq!(banco.view.offset_px, objetivo, "{contexto}");
                let f = banco.reposo();
                comprueba_barra(&f, &banco, &format!("{contexto}, offset {objetivo}"));
            }
        }
    }
}

// ---------------------------------------------------------------------------------------------
// Fila 5: clics, recorte y arrastre con el panel desplazado
// ---------------------------------------------------------------------------------------------

#[test]
fn fila_5_un_clic_en_un_deslizador_halla_el_control_con_y_menos_offset() {
    for ppp in PPPS {
        let n = i64::from(text_scale(ppp));
        let mut banco = Banco::con_alto_de_panel(ppp, 200 * n, imagen_f(), ColorChoice::Entry(1));
        let (x, y) = banco.sobre_el_texto();
        banco.rueda_en(x, y, 46 * n);
        assert_eq!(banco.view.offset_px, 46 * n);
        let geo = banco.geo();
        assert_eq!(geo.top_px, 45 * n);

        // El centro de la fila de R, ya desplazada, es R.
        let y_rojo = geo.y_fila(Control::Red);
        let x_a = geo.x_de_valor(128, Control::Red.max());
        banco.mueve(x_a, y_rojo);
        banco.pulsa(x_a, y_rojo);
        assert_eq!(
            banco.editor.dragging_control(),
            Some(Control::Red),
            "ppp {ppp}: el control se halla con y − offset"
        );
        let valor = geo.valor_en(x_a - geo.x_pista(), Control::Red);
        assert_eq!(
            banco.editor.preview(),
            Some((EditTarget::Entry(1), Rgba::opaque(valor as u8, 255, 0)))
        );
        // Soltar aplica a la entrada 1.
        let f = banco.suelta(x_a, y_rojo);
        assert_eq!(
            f.salida.commit,
            Some(Commit {
                target: EditTarget::Entry(1),
                color: Rgba::opaque(valor as u8, 255, 0)
            })
        );

        // El sitio donde estaría R con 36·n menos de desplazamiento (y = 84·n) es ahora la fila de G.
        let y_sin_desplazar = (55 + 22 + ROW_H_U / 2) * n;
        banco.mueve(x_a, y_sin_desplazar);
        banco.pulsa(x_a, y_sin_desplazar);
        assert_eq!(
            banco.editor.dragging_control(),
            Some(Control::Green),
            "ppp {ppp}: y = {y_sin_desplazar} cae ya en G"
        );
        banco.suelta(x_a, y_sin_desplazar);
    }
}

#[test]
fn fila_5_un_clic_en_el_campo_con_offset_le_da_el_foco() {
    for ppp in PPPS {
        let n = i64::from(text_scale(ppp));
        let mut banco = Banco::con_alto_de_panel(ppp, 200 * n, imagen_f(), ColorChoice::Entry(1));
        let (x, y) = banco.sobre_el_texto();
        banco.rueda_en(x, y, 46 * n);
        assert_eq!(banco.view.offset_px, 46 * n);
        // El campo está en la parte visible (y = 91·n − 46·n + 132·n + 9·n = 186·n < 200·n).
        let (x, y) = banco.geo().campo();
        assert!(y < banco.panel_h_px());
        banco.clic(x, y);
        banco.reposo();
        assert!(banco.quiere_teclado(), "ppp {ppp}: el clic enfoca el campo");
        // Tras teclear algo y salir, el texto va a la entrada 1.
        banco.escribe("#ff00ff");
        banco.clic_en_el_lienzo();
        banco.reposo();
        assert_eq!(
            banco.commits(),
            vec![Commit {
                target: EditTarget::Entry(1),
                color: Rgba::opaque(255, 0, 255)
            }]
        );
    }
}

#[test]
fn fila_5_fuera_del_rectangulo_del_panel_no_se_golpea_nada() {
    for ppp in PPPS {
        let n = i64::from(text_scale(ppp));
        // Panel de 136·n y sin desplazar: la fila de B (141..155·n) y el campo (223..241·n)
        // quedan por debajo del panel, bajo la barra de estado (136..156·n).
        let mut banco = Banco::con_alto_de_panel(ppp, 136 * n, imagen_f(), ColorChoice::Entry(1));
        let geo = banco.geo();
        assert_eq!(geo.top_px, 91 * n);
        let x = geo.x_de_valor(128, 255);
        let y_azul = geo.y_fila(Control::Blue);
        assert!(y_azul >= 136 * n, "B está fuera del panel");
        banco.mueve(x, y_azul);
        banco.pulsa(x, y_azul);
        assert!(
            !banco.editor.dragging(),
            "ppp {ppp}: un clic fuera del panel no empieza un arrastre"
        );
        banco.suelta(x, y_azul);
        assert!(banco.commits().is_empty());

        // Contraste: la fila de G, dentro del panel (127..141·n, y = 134·n), sí.
        let y_verde = 134 * n;
        banco.mueve(x, y_verde);
        banco.pulsa(x, y_verde);
        assert_eq!(banco.editor.dragging_control(), Some(Control::Green));
        banco.suelta(x, y_verde);
    }
}

#[test]
fn fila_5_un_clic_en_la_parte_del_campo_que_cae_fuera_del_panel_no_lo_enfoca() {
    for ppp in PPPS {
        let n = i64::from(text_scale(ppp));
        // 226·n sin desplazar: el campo va de 223 a 241·n y el panel acaba en 226·n. Egui
        // acepta clics hasta 5 puntos fuera de un widget (`interact_radius`): se pulsa a más de
        // eso (en 236·n), sobre la barra de estado.
        let mut banco = Banco::con_alto_de_panel(ppp, 226 * n, imagen_f(), ColorChoice::Entry(1));
        let geo = banco.geo();
        assert_eq!(geo.top_px, 91 * n);
        let (x, _) = geo.campo();
        banco.clic(x, 236 * n);
        banco.reposo();
        assert!(
            !banco.quiere_teclado(),
            "ppp {ppp}: la parte del campo bajo el panel no recibe el clic"
        );
        // La parte de dentro (y = 224·n, de 223 a 226·n) sí.
        banco.clic(x, 224 * n);
        banco.reposo();
        assert!(banco.quiere_teclado(), "ppp {ppp}: la de dentro sí");
    }
}

#[test]
fn fila_5_todo_lo_pintado_queda_dentro_del_rectangulo_del_panel() {
    for ppp in PPPS {
        let n = i64::from(text_scale(ppp));
        for alto in [300 * n, 277 * n, 276 * n, 200 * n, 100 * n] {
            let mut banco =
                Banco::con_alto_de_panel(ppp, alto, imagen_indexada_256(), ColorChoice::Entry(5));
            let panel = banco.rect_panel();
            let contexto = format!("ppp {ppp}, alto {alto}, offset 0");
            let f = banco.reposo();
            comprueba_dentro_del_panel(&f, panel, &contexto);
            if !banco.disposicion().scrolling {
                continue;
            }
            let (x, y) = banco.sobre_el_texto();
            banco.mueve(x, y);
            let max = banco.max_offset();
            for objetivo in [n, max / 2 / n * n, max] {
                banco.rueda(-100_000 * n);
                banco.rueda(objetivo);
                let contexto = format!("ppp {ppp}, alto {alto}, offset {}", banco.view.offset_px);
                let f = banco.reposo();
                comprueba_dentro_del_panel(&f, panel, &contexto);
            }
        }
    }
}

#[test]
fn fila_5_el_campo_enfocado_y_el_arrastre_tambien_se_recortan_al_panel() {
    for ppp in PPPS {
        let n = i64::from(text_scale(ppp));
        let mut banco = Banco::con_alto_de_panel(ppp, 236 * n, imagen_f(), ColorChoice::Entry(1));
        let panel = banco.rect_panel();
        banco.enfoca_campo();
        banco.escribe("#ff00ff");
        let f = banco.reposo();
        comprueba_dentro_del_panel(&f, panel, &format!("ppp {ppp}: campo enfocado"));
        banco.clic_en_el_lienzo();

        let geo = banco.geo();
        let (x, y) = (geo.x_de_valor(100, 255), geo.y_fila(Control::Red));
        banco.mueve(x, y);
        banco.pulsa(x, y);
        let f = banco.reposo();
        comprueba_dentro_del_panel(&f, panel, &format!("ppp {ppp}: arrastrando"));
    }
}

#[test]
fn fila_5_bajar_la_ventana_no_cancela_un_arrastre() {
    for ppp in PPPS {
        let n = i64::from(text_scale(ppp));
        let mut banco = Banco::con_alto_de_panel(ppp, 300 * n, imagen_f(), ColorChoice::Entry(1));
        let geo = banco.geo();
        let y = geo.y_fila(Control::Red);
        let x_a = geo.x_de_valor(128, Control::Red.max());
        banco.mueve(x_a, y);
        banco.pulsa(x_a, y);
        assert!(banco.editor.dragging(), "ppp {ppp}: el arrastre empieza");
        let provisional = banco.editor.preview();
        assert!(provisional.is_some());

        // La ventana baja hasta el bloque, por debajo (se desplaza) y hasta el mínimo.
        for alto in [277 * n, 276 * n, 200 * n, 100 * n] {
            banco.alto_de_panel(alto);
            let f = banco.reposo();
            assert!(
                banco.editor.dragging(),
                "ppp {ppp}: el arrastre sigue con el panel de {alto} px"
            );
            assert_eq!(banco.editor.preview(), provisional, "ppp {ppp}, {alto}");
            assert_eq!(f.salida.commit, None);
        }

        // Seguir moviendo y soltar da el paso, con la geometría nueva.
        let geo = banco.geo();
        let x_b = geo.x_de_valor(200, Control::Red.max());
        let y_b = geo.y_fila(Control::Green);
        banco.mueve(x_b, y_b);
        banco.suelta(x_b, y_b);
        let valor = geo.valor_en(x_b - geo.x_pista(), Control::Red);
        assert_eq!(
            banco.commits(),
            vec![Commit {
                target: EditTarget::Entry(1),
                color: Rgba::opaque(valor as u8, 255, 0)
            }],
            "ppp {ppp}: soltar aplica el valor de la `x` final"
        );
    }
}

// ---------------------------------------------------------------------------------------------
// Fila 6: sin aviso de ventana baja
// ---------------------------------------------------------------------------------------------

#[test]
fn fila_6_con_alto_200n_no_hay_aviso_de_ventana_baja_y_el_editor_pinta_su_cabecera() {
    for ppp in PPPS {
        let n = i64::from(text_scale(ppp));
        for alto in [200 * n, 100 * n, 276 * n] {
            let mut banco = Banco::con_alto_de_panel(ppp, alto, imagen_f(), ColorChoice::Entry(1));
            for _ in 0..3 {
                let f = banco.reposo();
                // `PanelOutput` solo tiene `choice`, `commit`, `notice`, desde 11d `op` y
                // `op_unused` y, desde I.2a, `background`, `indicator` y `editing`: si recupera
                // `editor_hidden` (u otro campo) esta desestructuración deja de compilar.
                let PanelOutput {
                    choice,
                    commit,
                    notice,
                    op,
                    op_unused,
                    background,
                    indicator,
                    editing,
                } = f.salida.clone();
                assert_eq!((choice, commit, notice), (None, None, None));
                assert_eq!((op, op_unused), (None, false));
                assert_eq!((background, indicator, editing), (None, None, None));
                assert!(
                    f.pinta("editar índice 1"),
                    "ppp {ppp}, alto {alto}: el editor pinta su cabecera"
                );
                for pintado in &f.textos {
                    assert!(
                        !pintado.texto.contains("ventana") && !pintado.texto.contains("oculto"),
                        "ppp {ppp}, alto {alto}: {:?}",
                        pintado.texto
                    );
                }
            }
            assert!(banco.avisos().is_empty());
        }
    }
}

#[test]
fn fila_6_ni_al_bajar_ni_al_subir_la_ventana_hay_avisos() {
    let ppp = 1.0;
    let mut banco = banco_f(ppp, 1);
    for alto in [300, 277, 276, 100, 277, 700] {
        banco.alto_de_panel(alto);
        let f = banco.reposo();
        assert_eq!(f.salida.notice, None, "alto {alto}");
        assert!(f.pinta("editar índice 1"), "alto {alto}");
    }
    assert!(banco.avisos().is_empty());
    assert!(banco.commits().is_empty());
}

// ---------------------------------------------------------------------------------------------
// Filas 7, 8 y 9: texto pendiente del campo hexadecimal
// ---------------------------------------------------------------------------------------------

/// Banco a 700·n con el campo enfocado y `escrito` pendiente en la entrada `entrada` de `F`
/// (o en la ranura de la imagen RGBA si `image` lo es).
fn banco_con_pendiente(image: Image, choice: ColorChoice, escrito: &str) -> Banco {
    let mut banco = Banco::nuevo(1.0, PANTALLA_ALTA, image, choice);
    banco.enfoca_campo();
    banco.escribe(escrito);
    banco.reposo();
    assert!(banco.editor.has_pending(), "se tecleó {escrito:?}");
    banco.historial.clear();
    banco
}

fn solo_commit(banco: &Banco, target: EditTarget, color: Rgba) {
    assert_eq!(banco.commits(), vec![Commit { target, color }]);
}

fn sin_commit_y_con_aviso(banco: &Banco, motivo: StaleReason) {
    assert!(banco.commits().is_empty(), "no hay commit");
    assert_eq!(banco.avisos(), vec![stale_notice(motivo).to_owned()]);
    assert!(!banco.editor.has_pending());
    assert_eq!(banco.editor.hex_display(), None);
}

// Fila 7.

/// **[DEPENDE DE EGUI]** Un clic fuera quita el foco al soltar.
#[test]
fn fila_7_perder_el_foco_aplica_el_texto_a_la_entrada_original() {
    let mut banco = banco_con_pendiente(imagen_f(), ColorChoice::Entry(2), "#00ff00");
    banco.clic_en_el_lienzo();
    banco.reposo();
    solo_commit(&banco, EditTarget::Entry(2), VERDE);
    assert!(banco.avisos().is_empty());
    assert!(!banco.editor.has_pending());
    assert!(!banco.quiere_teclado());
    // No se repite.
    for _ in 0..3 {
        banco.reposo();
    }
    assert_eq!(banco.commits().len(), 1);
}

#[test]
fn fila_7_quedarse_sin_destino_aplica_el_texto_a_la_entrada_original() {
    let mut banco = banco_con_pendiente(imagen_f(), ColorChoice::Entry(2), "#00ff00");
    // La elección pasa a una entrada que no existe: no hay editor.
    banco.choice = ColorChoice::Entry(200);
    let f = banco.reposo();
    assert_eq!(
        f.salida.commit,
        Some(Commit {
            target: EditTarget::Entry(2),
            color: VERDE
        }),
        "el panel resuelve el texto en el mismo fotograma en que se queda sin destino"
    );
    assert_eq!(f.salida.notice, None);
    assert!(!f.pinta_algo_con("editar"), "ya no hay editor");
    assert!(!banco.editor.has_pending());
    assert_eq!(banco.editor.hex_display(), None);
    for _ in 0..3 {
        banco.reposo();
    }
    solo_commit(&banco, EditTarget::Entry(2), VERDE);
    assert!(banco.avisos().is_empty());
}

#[test]
fn fila_7_el_texto_no_reaparece_al_volver_el_editor_ni_con_otra_entrada() {
    let mut banco = banco_con_pendiente(imagen_f(), ColorChoice::Entry(2), "#00ff00");
    banco.choice = ColorChoice::Entry(200);
    banco.reposo();
    banco.reposo();
    assert_eq!(banco.commits().len(), 1);

    // Vuelve el editor con la misma entrada: muestra el color guardado, no el texto viejo.
    banco.choice = ColorChoice::Entry(2);
    let f = banco.reposo();
    assert!(f.pinta(&hex_text(AZUL)), "muestra el guardado");
    assert!(!f.pinta("#00ff00"));
    // Y con otra entrada.
    banco.choice = ColorChoice::Entry(0);
    let f = banco.reposo();
    assert!(f.pinta(&hex_text(ROJO)));
    assert!(!f.pinta("#00ff00"));
    assert_eq!(banco.editor.hex_display(), None);

    // Un clic dentro y fuera del campo no aplica nada a la entrada original.
    banco.enfoca_campo();
    banco.clic_en_el_lienzo();
    banco.reposo();
    assert_eq!(banco.commits().len(), 1, "sin commits nuevos");
    assert!(banco.avisos().is_empty());
}

#[test]
fn fila_7_sin_destino_con_texto_invalido_avisa_del_hexadecimal() {
    let mut banco = banco_con_pendiente(imagen_f(), ColorChoice::Entry(2), "zzz");
    banco.choice = ColorChoice::Entry(200);
    let f = banco.reposo();
    assert_eq!(f.salida.commit, None);
    assert_eq!(
        f.salida.notice.as_deref(),
        Some(hex_notice(&px_ui::color_edit::HexError::Digit)),
        "aviso del hexadecimal inválido"
    );
    assert!(!banco.editor.has_pending());
    for _ in 0..3 {
        banco.reposo();
    }
    assert_eq!(banco.avisos().len(), 1);
    assert!(banco.commits().is_empty());
}

#[test]
fn fila_7_con_una_ranura_y_el_texto_valido_se_aplica_a_la_ranura_original() {
    // La ranura 3 de RGBA es roja por defecto: teclear verde cambia el color.
    let mut banco = banco_con_pendiente(imagen_rgba(), ColorChoice::Slot(3), "#00ff00");
    banco.choice = ColorChoice::Slot(10);
    let f = banco.reposo();
    assert_eq!(
        f.salida.commit,
        Some(Commit {
            target: EditTarget::Slot(3),
            color: VERDE
        })
    );
    assert!(!banco.editor.has_pending());
}

// Fila 8: la entrada ya no existe.

/// **[DEPENDE DE EGUI]** La imagen tiene una paleta más corta y la elección se movió a otra
/// entrada: el campo sigue pintado y pierde el foco con un clic fuera.
#[test]
fn fila_8_la_entrada_ya_no_existe_y_se_pierde_el_foco() {
    let mut banco = banco_con_pendiente(imagen_f(), ColorChoice::Entry(3), "#00ff00");
    let image = banco.documento.main_image();
    banco.aplica(Command::RemovePaletteEntry {
        image,
        index: 3,
        into: 0,
    });
    assert_eq!(banco.image.palette().expect("paleta").len(), 3);
    banco.choice = ColorChoice::Entry(0);
    banco.reposo();
    assert!(banco.editor.has_pending(), "el texto sigue pendiente");
    banco.clic_en_el_lienzo();
    banco.reposo();
    sin_commit_y_con_aviso(&banco, StaleReason::Gone);
    // Una vez: no se repite.
    for _ in 0..3 {
        banco.reposo();
    }
    assert_eq!(banco.avisos().len(), 1);
}

#[test]
fn fila_8_la_entrada_ya_no_existe_y_se_queda_sin_destino() {
    // Una imagen con la paleta más corta, con la misma elección: no hay destino.
    let mut banco = banco_con_pendiente(imagen_f(), ColorChoice::Entry(3), "#00ff00");
    let corta = Image::new_indexed(64, 48, Palette::from_colors([ROJO, VERDE]).expect("paleta"))
        .expect("imagen");
    banco.image = corta;
    assert!(banco.actual().is_none());
    let f = banco.reposo();
    assert_eq!(f.salida.commit, None, "sin Commit");
    assert_eq!(
        f.salida.notice.as_deref(),
        Some(stale_notice(StaleReason::Gone)),
        "texto descartado: la entrada ya no existe"
    );
    sin_commit_y_con_aviso(&banco, StaleReason::Gone);
    for _ in 0..3 {
        banco.reposo();
    }
    assert_eq!(banco.avisos().len(), 1);
}

#[test]
fn fila_8_deshacer_un_add_deja_la_entrada_sin_existir() {
    let mut banco = banco_f(1.0, 0);
    let image = banco.documento.main_image();
    banco.aplica(Command::AddPaletteEntry {
        image,
        color: AMARILLO,
    });
    banco.choice = ColorChoice::Entry(4);
    banco.reposo();
    banco.enfoca_campo();
    banco.escribe("#00ff00");
    banco.reposo();
    assert!(banco.editor.has_pending());
    banco.historial.clear();

    banco.deshace();
    assert_eq!(banco.image.palette().expect("paleta").len(), 4);
    let f = banco.reposo();
    assert_eq!(f.salida.commit, None);
    sin_commit_y_con_aviso(&banco, StaleReason::Gone);
    assert!(!f.pinta_algo_con("editar"), "sin destino no hay editor");
}

// Fila 9: el color guardado cambió.

#[test]
fn fila_9_el_color_cambio_y_se_pierde_el_foco() {
    let mut banco = banco_con_pendiente(imagen_f(), ColorChoice::Entry(2), "#00ff00");
    banco.pone_color(2, ROJO);
    banco.reposo();
    assert!(banco.editor.has_pending());
    banco.clic_en_el_lienzo();
    banco.reposo();
    sin_commit_y_con_aviso(&banco, StaleReason::Changed);
    for _ in 0..3 {
        banco.reposo();
    }
    assert_eq!(banco.avisos().len(), 1, "el aviso no se repite");
}

#[test]
fn fila_9_el_color_cambio_y_se_queda_sin_destino() {
    let mut banco = banco_con_pendiente(imagen_f(), ColorChoice::Entry(2), "#00ff00");
    banco.pone_color(2, ROJO);
    banco.choice = ColorChoice::Entry(200);
    let f = banco.reposo();
    assert_eq!(f.salida.commit, None);
    assert_eq!(
        f.salida.notice.as_deref(),
        Some(stale_notice(StaleReason::Changed)),
        "texto descartado: el color cambió"
    );
    sin_commit_y_con_aviso(&banco, StaleReason::Changed);
}

#[test]
fn fila_9_deshacer_un_set_palette_color_con_texto_pendiente_descarta() {
    let mut banco = banco_f(1.0, 2);
    banco.pone_color(2, ROJO);
    banco.reposo();
    banco.enfoca_campo();
    banco.escribe("#00ff00");
    banco.reposo();
    banco.historial.clear();

    banco.deshace();
    banco.reposo();
    banco.clic_en_el_lienzo();
    banco.reposo();
    sin_commit_y_con_aviso(&banco, StaleReason::Changed);
}

#[test]
fn fila_9_rehacer_un_set_palette_color_con_texto_pendiente_descarta() {
    let mut banco = banco_f(1.0, 2);
    banco.pone_color(2, ROJO);
    banco.deshace();
    banco.reposo();
    banco.enfoca_campo();
    banco.escribe("#00ff00");
    banco.reposo();
    banco.historial.clear();

    banco.rehace();
    banco.reposo();
    banco.choice = ColorChoice::Entry(200);
    banco.reposo();
    sin_commit_y_con_aviso(&banco, StaleReason::Changed);
}

#[test]
fn fila_9_si_el_color_vuelve_a_ser_el_base_se_aplica() {
    let mut banco = banco_con_pendiente(imagen_f(), ColorChoice::Entry(2), "#00ff00");
    banco.pone_color(2, ROJO);
    banco.reposo();
    banco.deshace();
    assert_eq!(banco.actual().map(|(_, c)| c), Some(AZUL));
    banco.reposo();
    assert!(banco.editor.has_pending());
    banco.clic_en_el_lienzo();
    banco.reposo();
    solo_commit(&banco, EditTarget::Entry(2), VERDE);
    assert!(banco.avisos().is_empty());
    assert!(!banco.editor.has_pending());

    // Y con rehacer: cambia, vuelve a lo que era al rehacer el cambio y deshacerlo otra vez.
    let mut banco = banco_con_pendiente(imagen_f(), ColorChoice::Entry(2), "#00ff00");
    banco.pone_color(2, ROJO);
    banco.deshace();
    banco.rehace();
    banco.deshace();
    banco.choice = ColorChoice::Entry(200);
    banco.reposo();
    solo_commit(&banco, EditTarget::Entry(2), VERDE);
}

/// **[DEPENDE DE EGUI]**
#[test]
fn fila_9_cambia_la_ranura() {
    // La ranura 3 de RGBA es roja por defecto.
    let mut banco = banco_con_pendiente(imagen_rgba(), ColorChoice::Slot(3), "#00ff00");
    assert!(banco.slots.set(3, AZUL), "la ranura 3 existe");
    banco.reposo();
    banco.clic_en_el_lienzo();
    banco.reposo();
    sin_commit_y_con_aviso(&banco, StaleReason::Changed);

    let mut banco = banco_con_pendiente(imagen_rgba(), ColorChoice::Slot(3), "#00ff00");
    assert!(banco.slots.set(3, AZUL));
    banco.choice = ColorChoice::Slot(10);
    banco.reposo();
    sin_commit_y_con_aviso(&banco, StaleReason::Changed);
}

// Texto pendiente y ventana baja: el campo no desaparece.

#[test]
fn reducir_y_agrandar_la_ventana_no_pierde_el_texto_pendiente() {
    for ppp in PPPS {
        let n = i64::from(text_scale(ppp));
        let mut banco = Banco::nuevo(ppp, PANTALLA_ALTA, imagen_f(), ColorChoice::Entry(2));
        banco.enfoca_campo();
        banco.escribe("#00ff00");
        banco.reposo();
        banco.historial.clear();
        for alto in [
            300 * n,
            277 * n,
            200 * n,
            100 * n,
            200 * n,
            277 * n,
            500 * n,
        ] {
            banco.alto_de_panel(alto);
            let f = banco.reposo();
            assert!(banco.editor.has_pending(), "ppp {ppp}, alto {alto}");
            assert_eq!(
                banco.editor.hex_display().as_deref(),
                Some("#00ff00"),
                "ppp {ppp}, alto {alto}"
            );
            assert_eq!(f.salida.commit, None, "ppp {ppp}, alto {alto}");
            assert_eq!(f.salida.notice, None, "ppp {ppp}, alto {alto}");
        }
        assert!(banco.commits().is_empty());
        assert!(banco.avisos().is_empty());
        let f = banco.reposo();
        assert!(
            f.pinta("#00ff00"),
            "ppp {ppp}: el campo muestra lo tecleado"
        );

        // Al salir del campo, se aplica a la entrada original.
        banco.clic_en_el_lienzo();
        banco.reposo();
        solo_commit(&banco, EditTarget::Entry(2), VERDE);
    }
}

// ---------------------------------------------------------------------------------------------
// Fila 11: mostrar la elegida
// ---------------------------------------------------------------------------------------------

/// Alto de la fila de iconos de la unidad I.4 con su margen (24 + 4 u), que la rejilla de
/// indexadas ya no usa: para el mismo número de filas visibles hay que dar tanto panel de más
/// (antes, con los botones de texto de 11d, 70 + 4 u).
const BOTONES_U: i64 = 28;

fn fila_de(entrada: usize) -> usize {
    entrada / COLUMNS
}

/// La fila de la elegida está entre las `visibles` que empiezan en la primera del banco.
fn elegida_visible(banco: &Banco, visibles: usize) -> bool {
    let fila =
        fila_de(cell_of_choice(&banco.image, banco.choice).expect("la elegida tiene muestra"));
    banco.scroll.first_row <= fila && fila < banco.scroll.first_row + visibles
}

fn banco_256(ppp: f32, panel_h_px: i64, entrada: u8, calentar: bool) -> Banco {
    Banco::nuevo_con(
        ppp,
        pantalla_para_panel(ppp, panel_h_px),
        imagen_indexada_256(),
        ColorChoice::Entry(entrada),
        calentar,
    )
}

#[test]
fn fila_11_el_primer_fotograma_no_muestra_la_elegida() {
    for ppp in PPPS {
        let n = i64::from(text_scale(ppp));
        // (436 + 74)·n: 12 filas visibles de 16 y la elegida (200 → fila 12) fuera de la vista.
        let mut banco = banco_256(ppp, (436 + BOTONES_U) * n, 200, false);
        assert_eq!(banco.view, PanelView::default());
        banco.reposo();
        assert_eq!(banco.view.last_visible, Some(12), "ppp {ppp}");
        assert_eq!(banco.scroll.first_row, 0, "ppp {ppp}: primer fotograma");
        assert!(!banco.view.reveal);
        // Ni en los siguientes mientras nada cambie.
        for _ in 0..3 {
            banco.reposo();
        }
        assert_eq!(
            banco.scroll.first_row, 0,
            "ppp {ppp}: sin cambios no hay reveal"
        );
        assert!(!elegida_visible(&banco, 12));
    }
}

#[test]
fn fila_11_cambiar_el_numero_de_filas_visibles_muestra_la_elegida_con_el_minimo_movimiento() {
    for ppp in PPPS {
        let n = i64::from(text_scale(ppp));
        let mut banco = banco_256(ppp, (436 + BOTONES_U) * n, 200, true);
        assert_eq!(banco.scroll.first_row, 0);
        assert_eq!(banco.view.last_visible, Some(12));

        // (336 + 74)·n: 5 filas visibles. La fila 12 pasa a ser la última visible (primera = 8).
        banco.alto_de_panel((336 + BOTONES_U) * n);
        banco.reposo();
        assert_eq!(banco.view.last_visible, Some(5), "ppp {ppp}");
        assert_eq!(banco.scroll.first_row, 8, "ppp {ppp}: el mínimo movimiento");
        assert_eq!(banco.scroll.rest, 0);
        assert!(elegida_visible(&banco, 5));
        // Sin más cambios no se vuelve a llamar: lo que se desplace a mano se queda.
        banco.reposo();
        assert_eq!(banco.scroll.first_row, 8);

        // Agrandar: 12 filas visibles; la elegida sigue a la vista.
        banco.alto_de_panel((436 + BOTONES_U) * n);
        banco.reposo();
        assert_eq!(banco.view.last_visible, Some(12));
        assert!(elegida_visible(&banco, 12), "ppp {ppp}");
        assert_eq!(
            banco.scroll.first_row, 4,
            "ppp {ppp}: recortado al máximo (16 − 12)"
        );
    }
}

#[test]
fn fila_11_al_bajar_hasta_el_respaldo_la_elegida_sigue_a_la_vista() {
    for ppp in PPPS {
        let n = i64::from(text_scale(ppp));
        // 700·n: caben las 16 filas.
        let mut banco = banco_256(ppp, 700 * n, 200, true);
        assert_eq!(banco.scroll.first_row, 0);
        assert!(banco.view.last_visible.is_some_and(|v| v >= 16));

        // 200·n: una sola fila visible (respaldo): tiene que ser la de la elegida.
        banco.alto_de_panel(200 * n);
        banco.reposo();
        assert_eq!(banco.view.last_visible, Some(1), "ppp {ppp}");
        assert_eq!(banco.scroll.first_row, 12, "ppp {ppp}");
        assert!(elegida_visible(&banco, 1));
    }
}

#[test]
fn fila_11_view_reveal_muestra_la_elegida_y_el_panel_lo_apaga() {
    for ppp in PPPS {
        let n = i64::from(text_scale(ppp));
        let mut banco = banco_256(ppp, (436 + BOTONES_U) * n, 200, true);
        assert_eq!(banco.scroll.first_row, 0);
        // Como las teclas 1 a 9, deshacer o rehacer: la sesión lo pone y el panel lo apaga.
        banco.view.reveal = true;
        banco.reposo();
        assert!(!banco.view.reveal, "ppp {ppp}: el panel lo apaga");
        assert_eq!(
            banco.scroll.first_row, 1,
            "ppp {ppp}: la fila 12 pasa a ser la última visible"
        );
        assert!(elegida_visible(&banco, 12));

        // Ya visible: no se mueve, pero el aviso se apaga igual.
        banco.view.reveal = true;
        banco.reposo();
        assert!(!banco.view.reveal);
        assert_eq!(banco.scroll.first_row, 1, "ppp {ppp}");

        // Por encima: sube lo justo.
        banco.choice = ColorChoice::Entry(0);
        banco.reposo();
        assert_eq!(
            banco.scroll.first_row, 1,
            "cambiar la elección sin `reveal` no mueve"
        );
        banco.view.reveal = true;
        banco.reposo();
        assert_eq!(banco.scroll.first_row, 0, "ppp {ppp}");
    }
}

#[test]
fn fila_11_view_reveal_con_una_eleccion_sin_muestra_no_mueve_nada_y_se_apaga() {
    let mut banco = banco_f(1.0, 1);
    banco.choice = ColorChoice::Entry(200);
    banco.scroll.first_row = 0;
    banco.view.reveal = true;
    banco.reposo();
    assert!(!banco.view.reveal);
    assert_eq!(banco.scroll.first_row, 0);
}

#[test]
fn fila_11_view_reveal_en_el_primer_fotograma_si_muestra_la_elegida() {
    let n = 1;
    let mut banco = banco_256(1.0, (436 + BOTONES_U) * n, 200, false);
    banco.view.reveal = true;
    banco.reposo();
    assert!(!banco.view.reveal);
    assert_eq!(banco.scroll.first_row, 1);
}

#[test]
fn fila_11_la_rueda_no_muestra_la_elegida() {
    for ppp in PPPS {
        let n = i64::from(text_scale(ppp));
        let mut banco = banco_256(ppp, (336 + BOTONES_U) * n, 0, true);
        assert_eq!(banco.view.last_visible, Some(5));
        let (x, y) = centro_de_muestra(n, 0, 0);
        banco.mueve(x, y);
        // Tres filas hacia abajo: la elegida (fila 0) sale de la vista.
        banco.rueda(3 * PITCH_U * n);
        assert_eq!(banco.scroll.first_row, 3, "ppp {ppp}");
        for _ in 0..3 {
            banco.reposo();
        }
        assert_eq!(banco.scroll.first_row, 3, "ppp {ppp}: la rueda manda");
        banco.rueda(PITCH_U * n);
        assert_eq!(banco.scroll.first_row, 4, "ppp {ppp}");
        assert!(!elegida_visible(&banco, 5));
    }
}

#[test]
fn fila_11_un_clic_en_una_muestra_no_muestra_la_elegida() {
    for ppp in PPPS {
        let n = i64::from(text_scale(ppp));
        let mut banco = banco_256(ppp, 354 * n, 0, true);
        let (x, y) = centro_de_muestra(n, 0, 0);
        banco.mueve(x, y);
        banco.rueda(3 * PITCH_U * n);
        assert_eq!(banco.scroll.first_row, 3);
        banco.reposo();

        // Un clic en una muestra de la fila 3 (la primera visible): la elección es de la
        // sesión y el panel no recoloca la rejilla, ni con la elección vieja (fila 0) ni con la
        // nueva.
        let (x, y) = centro_de_muestra(n, 3 * COLUMNS + 2, 3);
        banco.mueve(x, y);
        banco.pulsa(x, y);
        banco.suelta(x, y);
        assert_eq!(
            banco.elecciones(),
            vec![ColorChoice::Entry((3 * COLUMNS + 2) as u8)],
            "ppp {ppp}"
        );
        assert_eq!(banco.scroll.first_row, 3, "ppp {ppp}: tras el clic");
        banco.choice = ColorChoice::Entry((3 * COLUMNS + 2) as u8);
        for _ in 0..3 {
            banco.reposo();
        }
        assert_eq!(
            banco.scroll.first_row, 3,
            "ppp {ppp}: tras aplicar la elección"
        );
        assert!(!banco.view.reveal);
    }
}

#[test]
fn fila_11_arrastrar_un_deslizador_no_muestra_la_elegida() {
    let ppp = 1.0;
    let n = 1;
    let mut banco = banco_256(ppp, 400 * n, 0, true);
    let (x, y) = centro_de_muestra(n, 0, 0);
    banco.mueve(x, y);
    banco.rueda(3 * PITCH_U * n);
    assert_eq!(banco.scroll.first_row, 3);
    let geo = banco.geo();
    let (x, y) = (geo.x_de_valor(100, 255), geo.y_fila(Control::Green));
    banco.mueve(x, y);
    banco.pulsa(x, y);
    banco.mueve(x + 10, y);
    banco.suelta(x + 10, y);
    banco.reposo();
    assert_eq!(banco.scroll.first_row, 3);
}
