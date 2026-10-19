//! Banco de egui de `px_ui::dialog_view::show_dialog` (specs/hito-2-dialogos-modales.md, «Casos»
//! filas 9 (solo la parte de banco), 10 y 11), sin ventana ni capturas: `Context::run_ui` con
//! `RawInput` sintético. En ROJO hasta la implementación: `dialog_view`, `dialog_layout` y el resto
//! de «Piezas» no existen todavía.
//!
//! - **Fila 9 (banco).** Cada texto y cada marca pintados, salvo el deshabilitado, frente al relleno
//!   que tiene debajo (el último relleno opaco, anterior en el orden de pintado, que contiene el
//!   texto): ≥ 4,5:1 el texto (también el de los errores), ≥ 3:1 las marcas (la opción elegida y la
//!   casilla, rellenos `ACCENT`) y el anillo de foco (trazo `ACCENT`). Estados: por defecto, con
//!   errores (vacío, no dígito, rango, 11 cifras, `check`), con el hexadecimal (válido, pendiente
//!   válido e inválido), con opciones y casilla marcadas, con el ratón sobre `Aceptar`, `Cancelar`
//!   y `−`, y con el foco en cada elemento de `focus_order`. Los colores pintados tienen que estar en
//!   `CONTRAST_PAIRS` (no una copia).
//! - **Fila 10.** Piezas y puntero a las cinco escalas en la ventana mínima real (`468n × 333n` px,
//!   el banco usa puntos: `px / ppp`): etiquetas, rango, `−`/`+`, opciones, casilla, teclear en el
//!   numérico, `Intro`, pérdida de foco, `Escape`, `Aceptar`, `Cancelar`, zona de solo lectura, y el
//!   velo (clic, arrastre y rueda no cierran ni cambian nada y no llegan a un botón, un campo ni un
//!   lienzo simulado dibujados detrás, en una capa inferior; con el diálogo oculto sí llegan).
//! - **Fila 11.** Rectángulos pintados frente a `dialog_layout` (ventana mínima y 960 × 720 pt),
//!   nada sale del diálogo salvo el velo, rueda de 42 u por muesca con contenido enorme, foco inicial,
//!   `Tab` y `Mayús`+`Tab` un ciclo entero (sin llegar nunca a lo de detrás), `Intro` y `Espacio` en
//!   los botones, `Intro` y `Escape` en un campo, y cambio de `ppp` y de tamaño de pantalla con el
//!   diálogo abierto.
//!
//! Suposiciones sobre las firmas de «Piezas» (derives no listados): `Dialog`, `Values`, `Reaction`
//! y `Event`: `Clone + Debug + PartialEq`; `FocusTarget`: `Clone + Copy + Debug + PartialEq + Eq`.
//! `dialog_layout` no tiene los tipos exactos en la spec: se llama **solo** desde `layout_de` (un único
//! punto que ajustar) como `dialog_layout((ancho_px, alto_px): (i64, i64), n: u32, content_h_u: i64,
//! fill: bool, has_cancel: bool)`, con `DialogLayout { dialog, title, body, footer, reason, accept:
//! ScreenRect, cancel: Option<ScreenRect>, max_scroll_u: i64 }`.
//!
//! Decisiones donde la spec no fija un detalle:
//! - Se identifica cada pieza por su texto pintado (`Shape::Text`): el rótulo del botón es
//!   `spec().accept`/`cancel`, las opciones son sus textos y el error en línea es exactamente
//!   `ERROR_PREFIX` + motivo, en una sola forma de texto (o varios tramos, todos `WARNING`). El
//!   motivo del pie es `ERROR_PREFIX` + `reason()`. Un texto cuenta como «el rótulo `−`» si es `−`
//!   (U+2212) o `-`, porque la spec no dice cuál de los dos glifos usa el botón.
//! - Texto deshabilitado = color `TEXT_DISABLED`, alfa < 255 u opacidad < 1; solo pueden estarlo
//!   `Aceptar` (con errores) y `−`/`+` (en su límite).
//! - El relleno bajo un texto se busca con el recorte aplicado y quitando un cuarto del alto por
//!   arriba y por abajo (el interlineado de la galería puede sobresalir un poco del campo).
//! - Una muesca de la rueda es un `MouseWheel` de una línea (`rueda_linea`): 42 u en el cuerpo, 3
//!   líneas en la zona. La rueda se mide con el puntero en el relleno izquierdo del cuerpo
//!   (`x = 3 u` desde el borde), que no es ninguna pieza ni la zona.
//! - Teclear: `Ctrl+A`, doce `Retroceso` y el texto con `Event::Text`, con el foco inicial (el primer
//!   campo) o el pedido con `Tab`.
//! - Las muescas de la rueda y los clics se aplican en `show_dialog` sobre el `Dialog` recibido (no
//!   hay `Modal`): «se cierra» es que la `Reaction` sea `Accepted` o `Cancelled`.
//! - `AvPág`, `RePág`, `Fin`, `Inicio` y las flechas con el foco en la zona: se exige la dirección y
//!   el recorte (`Fin` va al máximo, `Inicio` a 0), no cuántas líneas avanza `AvPág`.

use egui::{
    Area, Color32, Context, Event, Id, Key, Modifiers, Order, Pos2, Rect, Sense, Vec2, pos2, vec2,
};
use px_ui::dialog::{Dialog, Event as Evento, FieldKind, Reaction, Validity, Values};
use px_ui::dialog_keys::{FocusTarget, focus_order, initial_focus};
use px_ui::dialog_layout::dialog_layout;
use px_ui::dialog_view::{CONTRAST_PAIRS, ERROR_PREFIX, show_dialog};
use px_ui::licenses_view::{clamp_first_line, wrap_text};
use px_ui::text::apply_scale;
use px_ui::theme::{ACCENT, SURFACE_1, TEXT_DISABLED, WARNING, contrast_ratio, visuals};
use px_ui::view::ScreenRect;

mod common;
#[path = "common/dialog_forms.rs"]
mod dialog_forms;

use common::{
    PPPS, Relleno, Texto, boton, contexto, en_puntos, fotograma, modificadores_command, mover,
    n_de, rueda_linea, tecla, tolerancia_de_egui,
};
use dialog_forms::{all_kinds, huge, open};

const MIN_TEXTO: f64 = 4.5;
const MIN_MARCA: f64 = 3.0;

// ---------------------------------------------------------------------------------------------
// Lo pintado, en orden de pintado
// ---------------------------------------------------------------------------------------------

mod orden {
    //! Como `common::pintado`, pero conserva el orden de pintado entre textos, rellenos y trazos
    //! (hace falta para «el último relleno que hay debajo») y recoge los trazos de los rectángulos
    //! (el anillo de foco) y los círculos rellenos (una marca redonda).

    use egui::epaint::{ClippedShape, Shape};
    use egui::{Color32, Rect, Vec2};

    use crate::common::{Relleno, Texto, Tramo};

    #[derive(Clone, Copy, Debug)]
    pub struct Trazo {
        pub rect: Rect,
        pub color: Color32,
        pub ancho: f32,
        pub clip: Rect,
    }

    #[derive(Clone, Debug)]
    pub enum Forma {
        Texto(Texto),
        Relleno(Relleno),
        Trazo(Trazo),
    }

    #[derive(Clone, Debug, Default)]
    pub struct Pintura {
        pub formas: Vec<Forma>,
    }

    impl Pintura {
        pub fn textos(&self) -> impl Iterator<Item = (usize, &Texto)> + '_ {
            self.formas.iter().enumerate().filter_map(|(i, f)| match f {
                Forma::Texto(t) => Some((i, t)),
                _ => None,
            })
        }

        pub fn rellenos(&self) -> impl Iterator<Item = (usize, &Relleno)> + '_ {
            self.formas.iter().enumerate().filter_map(|(i, f)| match f {
                Forma::Relleno(r) => Some((i, r)),
                _ => None,
            })
        }

        pub fn trazos(&self) -> impl Iterator<Item = (usize, &Trazo)> + '_ {
            self.formas.iter().enumerate().filter_map(|(i, f)| match f {
                Forma::Trazo(t) => Some((i, t)),
                _ => None,
            })
        }

        /// La primera forma de texto que, recortada, es exactamente `buscado`.
        pub fn exacto(&self, buscado: &str) -> Option<&Texto> {
            self.textos()
                .map(|(_, t)| t)
                .find(|t| t.texto.trim() == buscado)
        }

        /// Todas las formas de texto que son exactamente `buscado`, de arriba abajo.
        pub fn todos_exactos(&self, buscado: &str) -> Vec<&Texto> {
            let mut v: Vec<&Texto> = self
                .textos()
                .map(|(_, t)| t)
                .filter(|t| t.texto.trim() == buscado)
                .collect();
            v.sort_by(|a, b| {
                a.pos
                    .y
                    .total_cmp(&b.pos.y)
                    .then(a.pos.x.total_cmp(&b.pos.x))
            });
            v
        }

        /// Todo el texto pintado, junto.
        pub fn todo_el_texto(&self) -> String {
            self.textos().map(|(_, t)| t.texto.as_str()).collect()
        }

        pub fn lista_de_textos(&self) -> Vec<&str> {
            self.textos().map(|(_, t)| t.texto.as_str()).collect()
        }
    }

    fn recoge(shape: &Shape, clip: Rect, fuera: &mut Vec<Forma>) {
        match shape {
            Shape::Text(t) => {
                let job = &t.galley.job;
                let tramos = job
                    .sections
                    .iter()
                    .map(|s| Tramo {
                        texto: job.text[s.byte_range.start.0..s.byte_range.end.0].to_owned(),
                        color: t.override_text_color.unwrap_or(
                            if s.format.color == Color32::PLACEHOLDER {
                                t.fallback_color
                            } else {
                                s.format.color
                            },
                        ),
                    })
                    .collect();
                fuera.push(Forma::Texto(Texto {
                    texto: t.galley.text().to_owned(),
                    pos: t.pos,
                    tamano: t.galley.size(),
                    clip,
                    tramos,
                    opacidad: t.opacity_factor,
                }));
            }
            Shape::Rect(r) => {
                if r.fill != Color32::TRANSPARENT {
                    fuera.push(Forma::Relleno(Relleno {
                        rect: r.rect,
                        fill: r.fill,
                        clip,
                    }));
                }
                if r.stroke.width > 0.0 && r.stroke.color != Color32::TRANSPARENT {
                    fuera.push(Forma::Trazo(Trazo {
                        rect: r.rect,
                        color: r.stroke.color,
                        ancho: r.stroke.width,
                        clip,
                    }));
                }
            }
            Shape::Circle(c) => {
                if c.fill != Color32::TRANSPARENT {
                    fuera.push(Forma::Relleno(Relleno {
                        rect: Rect::from_center_size(c.center, Vec2::splat(2.0 * c.radius)),
                        fill: c.fill,
                        clip,
                    }));
                }
            }
            Shape::Vec(formas) => {
                for forma in formas {
                    recoge(forma, clip, fuera);
                }
            }
            _ => {}
        }
    }

    pub fn pintura(formas: &[ClippedShape]) -> Pintura {
        let mut fuera = Vec::new();
        for clipped in formas {
            recoge(&clipped.shape, clipped.clip_rect, &mut fuera);
        }
        Pintura { formas: fuera }
    }
}

use orden::{Forma, Pintura, Trazo, pintura};

// ---------------------------------------------------------------------------------------------
// Ventanas y geometría
// ---------------------------------------------------------------------------------------------

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Ventana {
    /// `468n × 333n` px (la ventana mínima real de la escala).
    Minima,
    /// 960 × 720 pt.
    Grande,
}

/// Tamaño en píxeles y en puntos.
fn medidas(ppp: f32, ventana: Ventana) -> ((i64, i64), Vec2) {
    match ventana {
        Ventana::Minima => {
            let n = n_de(ppp);
            let (w, h) = (468 * n, 333 * n);
            ((w, h), vec2(w as f32 / ppp, h as f32 / ppp))
        }
        Ventana::Grande => (
            (
                (960.0 * f64::from(ppp)).round() as i64,
                (720.0 * f64::from(ppp)).round() as i64,
            ),
            vec2(960.0, 720.0),
        ),
    }
}

const VENTANAS: [Ventana; 2] = [Ventana::Minima, Ventana::Grande];

/// Lo que se usa de `DialogLayout`.
#[derive(Clone, Copy, Debug)]
struct Rects {
    dialog: ScreenRect,
    title: ScreenRect,
    body: ScreenRect,
    footer: ScreenRect,
    reason: ScreenRect,
    accept: ScreenRect,
    cancel: Option<ScreenRect>,
    max_scroll_u: i64,
}

/// El único sitio donde se llama a `dialog_layout` (ver la cabecera para los tipos supuestos).
fn layout_de(
    ventana_px: (i64, i64),
    n: i64,
    contenido_u: i64,
    fill: bool,
    con_cancelar: bool,
) -> Rects {
    let l = dialog_layout(
        ventana_px,
        u32::try_from(n).expect("n cabe en u32"),
        contenido_u,
        fill,
        con_cancelar,
    );
    Rects {
        dialog: l.dialog,
        title: l.title,
        body: l.body,
        footer: l.footer,
        reason: l.reason,
        accept: l.accept,
        cancel: l.cancel,
        max_scroll_u: l.max_scroll_u,
    }
}

fn rect_pt(r: ScreenRect, ppp: f32) -> Rect {
    Rect::from_min_max(
        en_puntos(ppp, r.x, r.y),
        en_puntos(ppp, r.x + r.width, r.y + r.height),
    )
}

/// El rectángulo pintado (puntos) es el de `esperado` (píxeles), con el redondeo de egui.
fn coincide(pintado: Rect, esperado: ScreenRect, ppp: f32) -> bool {
    let tol = tolerancia_de_egui(ppp);
    let cerca = |pt: f32, px: i64| (pt * ppp - px as f32).abs() <= tol;
    cerca(pintado.min.x, esperado.x)
        && cerca(pintado.min.y, esperado.y)
        && cerca(pintado.max.x, esperado.x + esperado.width)
        && cerca(pintado.max.y, esperado.y + esperado.height)
}

/// El rectángulo pintado (puntos) cae dentro de `zona` (píxeles), con el redondeo de egui.
fn dentro(pintado: Rect, zona: ScreenRect, ppp: f32) -> bool {
    let tol = tolerancia_de_egui(ppp);
    pintado.min.x * ppp >= zona.x as f32 - tol
        && pintado.min.y * ppp >= zona.y as f32 - tol
        && pintado.max.x * ppp <= (zona.x + zona.width) as f32 + tol
        && pintado.max.y * ppp <= (zona.y + zona.height) as f32 + tol
}

// ---------------------------------------------------------------------------------------------
// Lo que hay detrás del diálogo
// ---------------------------------------------------------------------------------------------

/// Un botón, un campo de texto y un lienzo simulado, cada uno en un `Area` de `Order::Middle`
/// (una capa **inferior** a la del diálogo), con lo que han recibido.
#[derive(Clone, Debug)]
struct Detras {
    boton: Rect,
    boton_id: Id,
    texto_id: Id,
    texto: String,
    clics_boton: usize,
    hovers_boton: usize,
    clics_lienzo: usize,
    hovers_lienzo: usize,
    ruedas_lienzo: usize,
}

impl Detras {
    fn nuevo() -> Detras {
        Detras {
            boton: Rect::NOTHING,
            boton_id: Id::NULL,
            texto_id: Id::NULL,
            texto: String::new(),
            clics_boton: 0,
            hovers_boton: 0,
            clics_lienzo: 0,
            hovers_lienzo: 0,
            ruedas_lienzo: 0,
        }
    }

    fn a_cero(&mut self) {
        self.clics_boton = 0;
        self.hovers_boton = 0;
        self.clics_lienzo = 0;
        self.hovers_lienzo = 0;
        self.ruedas_lienzo = 0;
    }

    fn nada_recibido(&self) -> bool {
        self.clics_boton == 0
            && self.hovers_boton == 0
            && self.clics_lienzo == 0
            && self.hovers_lienzo == 0
            && self.ruedas_lienzo == 0
    }
}

fn dibuja_detras(ctx: &Context, d: &mut Detras, pantalla: Vec2) {
    // El lienzo simulado: toda la pantalla, con clic, arrastre y rueda.
    let lienzo = Area::new(Id::unique("detras_lienzo"))
        .order(Order::Middle)
        .fixed_pos(Pos2::ZERO)
        .show(ctx, |ui| {
            ui.allocate_exact_size(pantalla, Sense::click_and_drag()).1
        });
    let r = lienzo.inner;
    if r.hovered() {
        d.hovers_lienzo += 1;
        d.ruedas_lienzo += ctx.input(|i| {
            i.events
                .iter()
                .filter(|e| matches!(e, Event::MouseWheel { .. }))
                .count()
        });
    }
    if r.clicked() || r.is_pointer_button_down_on() || r.dragged() {
        d.clics_lienzo += 1;
    }

    // Un campo de texto detrás (para la tabulación).
    let texto = Area::new(Id::unique("detras_texto"))
        .order(Order::Middle)
        .fixed_pos(pos2(2.0, 120.0))
        .show(ctx, |ui| ui.text_edit_singleline(&mut d.texto));
    d.texto_id = texto.inner.id;

    // El botón, encima del lienzo y en la esquina de la ventana (sobre el velo, fuera del diálogo).
    let boton = Area::new(Id::unique("detras_boton"))
        .order(Order::Middle)
        .fixed_pos(pos2(2.0, 2.0))
        .show(ctx, |ui| ui.button("Detras"));
    d.boton = boton.inner.rect;
    d.boton_id = boton.inner.id;
    if boton.inner.hovered() {
        d.hovers_boton += 1;
    }
    if boton.inner.clicked() || boton.inner.is_pointer_button_down_on() {
        d.clics_boton += 1;
    }
}

// ---------------------------------------------------------------------------------------------
// El banco
// ---------------------------------------------------------------------------------------------

struct Banco {
    ctx: Context,
    ppp: f32,
    ventana: Ventana,
    dialog: Dialog,
    focus: Option<FocusTarget>,
    detras: Option<Detras>,
    muestra_dialogo: bool,
    /// Toda `Reaction` que no es `None`, de todos los fotogramas desde el último `clear`.
    reacciones: Vec<Reaction>,
}

fn configura(ctx: &Context, ppp: f32) {
    ctx.set_visuals_of(egui::Theme::Dark, visuals(ppp));
}

impl Banco {
    fn nuevo(ppp: f32, ventana: Ventana, dialog: Dialog) -> Banco {
        let ctx = contexto(ppp);
        configura(&ctx, ppp);
        Banco {
            ctx,
            ppp,
            ventana,
            dialog,
            focus: None,
            detras: None,
            muestra_dialogo: true,
            reacciones: Vec::new(),
        }
    }

    /// Ya con unos fotogramas de reposo (el diálogo colocado y con su foco inicial).
    fn listo(ppp: f32, ventana: Ventana, dialog: Dialog) -> Banco {
        let mut banco = Banco::nuevo(ppp, ventana, dialog);
        banco.reposo();
        banco
    }

    fn con_detras(ppp: f32, ventana: Ventana, dialog: Dialog) -> Banco {
        let mut banco = Banco::nuevo(ppp, ventana, dialog);
        banco.detras = Some(Detras::nuevo());
        banco.reposo();
        banco
    }

    /// Solo lo de detrás, con el diálogo sin mostrar (el control de las pruebas del velo y de `Tab`).
    fn sin_dialogo(ppp: f32, ventana: Ventana, dialog: Dialog) -> Banco {
        let mut banco = Banco::nuevo(ppp, ventana, dialog);
        banco.detras = Some(Detras::nuevo());
        banco.muestra_dialogo = false;
        banco.reposo();
        banco
    }

    fn pantalla(&self) -> Vec2 {
        medidas(self.ppp, self.ventana).1
    }

    fn n(&self) -> i64 {
        n_de(self.ppp)
    }

    /// Una unidad `u` en puntos.
    fn u(&self) -> f32 {
        self.n() as f32 / self.ppp
    }

    fn rects(&self) -> Rects {
        layout_de(
            medidas(self.ppp, self.ventana).0,
            self.n(),
            self.dialog.content_h_u(),
            self.dialog.is_fill(),
            self.dialog.spec().cancel.is_some(),
        )
    }

    fn cambia(&mut self, ppp: f32, ventana: Ventana) {
        self.ppp = ppp;
        self.ventana = ventana;
        self.ctx.set_pixels_per_point(ppp);
        configura(&self.ctx, ppp);
    }

    fn fotograma(&mut self, events: Vec<Event>) -> Pintura {
        let pantalla = self.pantalla();
        let mut reaccion = Reaction::None;
        let formas = {
            let Banco {
                ctx,
                dialog,
                focus,
                detras,
                muestra_dialogo,
                ..
            } = &mut *self;
            let muestra = *muestra_dialogo;
            fotograma(ctx, pantalla, events, |ui| {
                let c = ui.ctx().clone();
                apply_scale(&c);
                if let Some(d) = detras.as_mut() {
                    dibuja_detras(&c, d, pantalla);
                }
                if muestra {
                    reaccion = show_dialog(&c, dialog, focus);
                }
            })
        };
        if !matches!(reaccion, Reaction::None) {
            self.reacciones.push(reaccion);
        }
        pintura(&formas)
    }

    /// Tres fotogramas sin eventos: lo bastante para que egui reparta el espacio y el diálogo
    /// asiente su posición, su foco y su desplazamiento.
    fn reposo(&mut self) -> Pintura {
        self.fotograma(vec![]);
        self.fotograma(vec![]);
        self.fotograma(vec![])
    }

    fn mover_a(&mut self, pos: Pos2) -> Pintura {
        self.fotograma(vec![mover(pos)]);
        self.reposo()
    }

    /// Mueve, pulsa y suelta en `pos`, y deja un fotograma de reposo.
    fn clic(&mut self, pos: Pos2) -> Pintura {
        self.fotograma(vec![mover(pos)]);
        self.fotograma(vec![boton(pos, true)]);
        self.fotograma(vec![boton(pos, false)]);
        self.reposo()
    }

    fn rueda(&mut self, abajo: bool, muescas: usize) -> Pintura {
        let events = (0..muescas).map(|_| rueda_linea(!abajo)).collect();
        self.fotograma(events)
    }

    fn pulsa(&mut self, key: Key, modifiers: Modifiers) -> Pintura {
        self.fotograma(vec![tecla(key, modifiers)]);
        self.reposo()
    }

    fn tab(&mut self, atras: bool) -> Pintura {
        let modifiers = Modifiers {
            shift: atras,
            ..Modifiers::NONE
        };
        self.pulsa(Key::Tab, modifiers)
    }

    /// `Tab` hasta que el foco es `objetivo` (como mucho una vuelta entera).
    fn foco_hasta(&mut self, objetivo: FocusTarget) {
        let vuelta = focus_order(&self.dialog).len() + 1;
        for _ in 0..vuelta {
            if self.focus == Some(objetivo) {
                return;
            }
            self.tab(false);
        }
        assert_eq!(self.focus, Some(objetivo), "el Tab no llega a {objetivo:?}");
    }

    /// Escribe `texto` en el campo con el foco: `Ctrl+A`, borrar y teclear.
    fn escribe(&mut self, texto: &str) -> Pintura {
        let mut borrar = vec![tecla(Key::A, modificadores_command(false))];
        for _ in 0..12 {
            borrar.push(tecla(Key::Backspace, Modifiers::NONE));
        }
        self.fotograma(borrar);
        self.fotograma(vec![Event::Text(texto.to_owned())]);
        self.reposo()
    }

    fn evento(&mut self, evento: Evento) {
        self.dialog.event(evento);
    }

    fn foco_de_egui(&self) -> Option<Id> {
        self.ctx.memory(|m| m.focused())
    }
}

// ---------------------------------------------------------------------------------------------
// Ayudantes de lectura
// ---------------------------------------------------------------------------------------------

fn aceptadas(reacciones: &[Reaction]) -> Vec<&Values> {
    reacciones
        .iter()
        .filter_map(|r| match r {
            Reaction::Accepted(v) => Some(v),
            _ => None,
        })
        .collect()
}

fn canceladas(reacciones: &[Reaction]) -> usize {
    reacciones
        .iter()
        .filter(|r| matches!(r, Reaction::Cancelled))
        .count()
}

fn cierra(reacciones: &[Reaction]) -> bool {
    !aceptadas(reacciones).is_empty() || canceladas(reacciones) > 0
}

/// El diálogo sin su desplazamiento (que se recorta según la ventana).
fn estado(dialog: &Dialog) -> Dialog {
    let mut d = dialog.clone();
    d.set_scroll_u(0);
    d
}

fn texto_de<'a>(p: &'a Pintura, buscado: &str) -> &'a Texto {
    p.exacto(buscado)
        .unwrap_or_else(|| panic!("no se pinta {buscado:?}; hay {:?}", p.lista_de_textos()))
}

/// La primera forma de texto que contiene `buscado` (una fila de la zona, o la zona entera si se
/// pinta de una vez).
fn texto_que_contiene<'a>(p: &'a Pintura, buscado: &str) -> &'a Texto {
    p.textos()
        .map(|(_, t)| t)
        .find(|t| t.texto.contains(buscado))
        .unwrap_or_else(|| panic!("no se pinta {buscado:?}; hay {:?}", p.lista_de_textos()))
}

fn centro_de(p: &Pintura, buscado: &str) -> Pos2 {
    texto_de(p, buscado).rect().center()
}

fn es_deshabilitado(t: &Texto) -> bool {
    t.opacidad < 1.0
        || t.tramos
            .iter()
            .any(|x| x.color == TEXT_DISABLED || x.color.a() < 255)
}

fn en_warning(t: &Texto) -> bool {
    !t.tramos.is_empty() && t.tramos.iter().all(|x| x.color == WARNING)
}

fn aceptar_etiqueta(dialog: &Dialog) -> &'static str {
    dialog.spec().accept
}

fn cancelar_etiqueta(dialog: &Dialog) -> &'static str {
    dialog.spec().cancel.expect("el formulario tiene Cancelar")
}

fn aceptar_deshabilitado(p: &Pintura, dialog: &Dialog) -> bool {
    es_deshabilitado(texto_de(p, aceptar_etiqueta(dialog)))
}

fn opciones_de(dialog: &Dialog, id: &str) -> &'static [&'static str] {
    for f in &dialog.spec().fields {
        if f.id == id {
            if let FieldKind::Choice { options, .. } = &f.kind {
                return options;
            }
        }
    }
    panic!("no hay un grupo de opciones {id:?}");
}

fn texto_del_informe(dialog: &Dialog) -> String {
    for f in &dialog.spec().fields {
        if let FieldKind::ReadOnly { text, .. } = &f.kind {
            return text.clone();
        }
    }
    panic!("no hay informe");
}

/// El formulario de la fila 3 con el texto del informe cambiado por `cambia(texto_original)`.
fn con_informe(cambia: impl Fn(&str) -> String) -> px_ui::dialog::FormSpec {
    let mut spec = all_kinds();
    for f in spec.fields.iter_mut() {
        if let FieldKind::ReadOnly { text, .. } = &mut f.kind {
            *text = cambia(text);
        }
    }
    spec
}

/// El formulario de la fila 3 con `日✓` (fuera del juego de caracteres) en el informe: en la
/// primera línea, para no cambiar el número de líneas.
fn formulario() -> px_ui::dialog::FormSpec {
    con_informe(|original| {
        let (primera, resto) = original
            .split_once('\n')
            .expect("el informe tiene varias líneas");
        format!("{primera} 日✓\n{resto}")
    })
}

/// Números de las líneas «linea N» que se pintan, ordenados (el número son los dígitos tras el
/// prefijo; lo que venga detrás, como el `日✓` sustituido, se ignora).
fn numeros_de_linea(p: &Pintura) -> Vec<usize> {
    let mut v: Vec<usize> = p
        .textos()
        .flat_map(|(_, t)| t.texto.lines())
        .filter_map(|l| {
            let resto = l.trim().strip_prefix("linea ")?;
            let digitos: String = resto.chars().take_while(char::is_ascii_digit).collect();
            digitos.parse().ok()
        })
        .collect();
    v.sort_unstable();
    v.dedup();
    v
}

/// Rectángulo de cada texto cuyo texto recortado es `-` o `−`, de arriba abajo.
fn menos(p: &Pintura) -> Vec<Rect> {
    let mut v: Vec<Rect> = p
        .textos()
        .map(|(_, t)| t)
        .filter(|t| matches!(t.texto.trim(), "−" | "-"))
        .map(|t| t.rect())
        .collect();
    v.sort_by(|a, b| a.min.y.total_cmp(&b.min.y));
    v
}

fn mas(p: &Pintura) -> Vec<Rect> {
    p.todos_exactos("+").iter().map(|t| t.rect()).collect()
}

/// Rellenos `ACCENT` de como mucho `10 u × 10 u` (marcas) con el centro entre `y0` y `y1` (puntos).
fn marcas_entre(p: &Pintura, u: f32, y0: f32, y1: f32) -> Vec<Rect> {
    p.rellenos()
        .map(|(_, r)| r)
        .filter(|r| r.fill == ACCENT)
        .map(|r| r.rect)
        .filter(|r| r.width() <= 10.0 * u + 0.05 && r.height() <= 10.0 * u + 0.05)
        .filter(|r| r.width() > 0.0 && (y0..=y1).contains(&r.center().y))
        .collect()
}

/// El anillo de foco: un trazo `ACCENT` de `2 u` con parte visible.
fn anillos(p: &Pintura, u: f32) -> Vec<&Trazo> {
    p.trazos()
        .map(|(_, t)| t)
        .filter(|t| t.color == ACCENT && (t.ancho - 2.0 * u).abs() <= 0.01 * u)
        .filter(|t| t.rect.intersect(t.clip).is_positive())
        .collect()
}

fn veladura(p: &Pintura) -> Vec<(usize, Relleno)> {
    p.rellenos()
        .filter(|(_, r)| r.fill == Color32::from_black_alpha(128))
        .map(|(i, r)| (i, *r))
        .collect()
}

/// El relleno del diálogo: el `SURFACE_1` de mayor área.
fn relleno_del_dialogo(p: &Pintura) -> Option<(usize, Relleno)> {
    p.rellenos()
        .filter(|(_, r)| r.fill == SURFACE_1)
        .max_by(|a, b| a.1.rect.area().total_cmp(&b.1.rect.area()))
        .map(|(i, r)| (i, *r))
}

// ---------------------------------------------------------------------------------------------
// Fila 9: contraste de lo pintado
// ---------------------------------------------------------------------------------------------

/// El último relleno opaco anterior a `hasta` que contiene `sonda` (con el recorte aplicado).
fn relleno_bajo(formas: &[Forma], hasta: usize, sonda: Rect) -> Option<Relleno> {
    formas[..hasta].iter().rev().find_map(|f| match f {
        Forma::Relleno(r)
            if r.fill.a() == 255 && r.rect.intersect(r.clip).expand(0.01).contains_rect(sonda) =>
        {
            Some(*r)
        }
        _ => None,
    })
}

/// Lo que se comprueba en un fotograma.
#[derive(Debug, Default)]
struct Informe {
    fallos: Vec<String>,
    /// Elementos comprobados (textos, marcas y anillos).
    comprobados: usize,
    /// Textos deshabilitados, exentos.
    exentos: Vec<String>,
    /// `(descripción, frente, fondo, mínimo exigido)` de cada comprobación.
    pares: Vec<(String, Color32, Color32, f64)>,
}

fn informe_de(p: &Pintura) -> Informe {
    let mut inf = Informe::default();
    for (i, f) in p.formas.iter().enumerate() {
        match f {
            Forma::Texto(t) => {
                let visible = t.rect().intersect(t.clip);
                if t.texto.trim().is_empty() || !visible.is_positive() {
                    continue;
                }
                if es_deshabilitado(t) {
                    inf.exentos.push(t.texto.trim().to_owned());
                    continue;
                }
                let sonda = visible.shrink2(vec2(0.0, visible.height() * 0.25));
                let Some(fondo) = relleno_bajo(&p.formas, i, sonda) else {
                    inf.fallos
                        .push(format!("el texto {:?} no tiene relleno debajo", t.texto));
                    continue;
                };
                for tramo in &t.tramos {
                    if tramo.texto.trim().is_empty() {
                        continue;
                    }
                    inf.comprobados += 1;
                    let ratio = contrast_ratio(tramo.color, fondo.fill);
                    inf.pares.push((
                        format!("texto {:?}", tramo.texto),
                        tramo.color,
                        fondo.fill,
                        MIN_TEXTO,
                    ));
                    if ratio < MIN_TEXTO {
                        inf.fallos.push(format!(
                            "el texto {:?} ({:?}) sobre {:?} da {ratio:.2} (< {MIN_TEXTO})",
                            tramo.texto, tramo.color, fondo.fill
                        ));
                    }
                }
            }
            Forma::Relleno(r) if r.fill == ACCENT => {
                let visible = r.rect.intersect(r.clip);
                if !visible.is_positive() {
                    continue;
                }
                let Some(fondo) = relleno_bajo(&p.formas, i, visible) else {
                    inf.fallos
                        .push(format!("la marca {:?} no tiene relleno debajo", r.rect));
                    continue;
                };
                inf.comprobados += 1;
                inf.pares
                    .push(("marca".into(), ACCENT, fondo.fill, MIN_MARCA));
                let ratio = contrast_ratio(ACCENT, fondo.fill);
                if ratio < MIN_MARCA {
                    inf.fallos.push(format!(
                        "la marca {:?} sobre {:?} da {ratio:.2} (< {MIN_MARCA})",
                        r.rect, fondo.fill
                    ));
                }
            }
            Forma::Trazo(t) if t.color == ACCENT => {
                if !t.rect.intersect(t.clip).is_positive() {
                    continue;
                }
                let sonda = Rect::from_center_size(t.rect.center(), Vec2::splat(0.1));
                let Some(fondo) = relleno_bajo(&p.formas, i, sonda) else {
                    inf.fallos
                        .push(format!("el anillo {:?} no tiene relleno debajo", t.rect));
                    continue;
                };
                inf.comprobados += 1;
                inf.pares
                    .push(("anillo".into(), ACCENT, fondo.fill, MIN_MARCA));
                let ratio = contrast_ratio(ACCENT, fondo.fill);
                if ratio < MIN_MARCA {
                    inf.fallos.push(format!(
                        "el anillo {:?} sobre {:?} da {ratio:.2} (< {MIN_MARCA})",
                        t.rect, fondo.fill
                    ));
                }
            }
            _ => {}
        }
    }
    inf
}

/// Los estados de la fila 9.
const ESCENARIOS: [&str; 15] = [
    "defecto",
    "vacio_y_no_digito",
    "rango",
    "once_cifras",
    "check_falla",
    "hex",
    "hex_invalido",
    "hex_pendiente_valido",
    "opciones_y_casilla",
    "hover_aceptar",
    "hover_cancelar",
    "hover_menos",
    "hover_aceptar_deshabilitado",
    "limite_inferior",
    "limite_superior",
];

fn numero(b: &mut Banco, campo: &'static str, texto: &str) {
    b.evento(Evento::Number {
        field: campo,
        text: texto.to_owned(),
    });
}

fn prepara(b: &mut Banco, escenario: &str) {
    match escenario {
        "defecto" => {}
        "vacio_y_no_digito" => {
            numero(b, "ancho", "abc");
            numero(b, "alto", "");
        }
        "rango" => {
            numero(b, "ancho", "0");
            numero(b, "alto", "8193");
        }
        "once_cifras" => numero(b, "ancho", "12345678901"),
        "check_falla" => {
            numero(b, "ancho", "8192");
            numero(b, "alto", "8192");
        }
        "hex" => b.evento(Evento::Choice {
            field: "modo",
            index: 2,
        }),
        "hex_invalido" => {
            b.evento(Evento::Choice {
                field: "modo",
                index: 2,
            });
            b.evento(Evento::Hex {
                field: "color",
                text: "zz".into(),
            });
        }
        "hex_pendiente_valido" => {
            b.evento(Evento::Choice {
                field: "modo",
                index: 2,
            });
            b.evento(Evento::Hex {
                field: "color",
                text: "#12f".into(),
            });
        }
        "opciones_y_casilla" => {
            b.evento(Evento::Choice {
                field: "modo",
                index: 1,
            });
            b.evento(Evento::Check {
                field: "opaco",
                on: true,
            });
        }
        "hover_aceptar" => {
            let p = b.reposo();
            let c = centro_de(&p, aceptar_etiqueta(&b.dialog));
            b.fotograma(vec![mover(c)]);
        }
        "hover_cancelar" => {
            let p = b.reposo();
            let c = centro_de(&p, cancelar_etiqueta(&b.dialog));
            b.fotograma(vec![mover(c)]);
        }
        "hover_menos" => {
            let p = b.reposo();
            let c = menos(&p).first().expect("hay un −").center();
            b.fotograma(vec![mover(c)]);
        }
        "hover_aceptar_deshabilitado" => {
            numero(b, "ancho", "abc");
            let p = b.reposo();
            let c = centro_de(&p, aceptar_etiqueta(&b.dialog));
            b.fotograma(vec![mover(c)]);
        }
        "limite_inferior" => numero(b, "ancho", "1"),
        "limite_superior" => numero(b, "ancho", "8192"),
        otro => panic!("escenario desconocido: {otro}"),
    }
}

fn comprueba_escenarios(ventana: Ventana) {
    for ppp in PPPS {
        for escenario in ESCENARIOS {
            let mut banco = Banco::nuevo(ppp, ventana, open(formulario()));
            banco.reposo();
            prepara(&mut banco, escenario);
            let p = banco.reposo();
            let inf = informe_de(&p);
            assert!(
                inf.fallos.is_empty(),
                "ppp {ppp} {ventana:?} {escenario}: {:#?}",
                inf.fallos
            );
            assert!(
                inf.comprobados >= 8,
                "ppp {ppp} {ventana:?} {escenario}: solo se comprueban {} elementos; hay {:?}",
                inf.comprobados,
                p.lista_de_textos()
            );
            // Solo lo deshabilitado queda exento: `Aceptar` y los botones `−` y `+`.
            for exento in &inf.exentos {
                assert!(
                    [aceptar_etiqueta(&banco.dialog), "−", "-", "+"].contains(&exento.as_str()),
                    "ppp {ppp} {ventana:?} {escenario}: texto exento que no es un botón: {exento:?}"
                );
            }
        }
    }
}

#[test]
fn fila_9_todo_texto_y_marca_pintados_supera_su_minimo_sobre_su_relleno_en_la_ventana_minima() {
    comprueba_escenarios(Ventana::Minima);
}

#[test]
fn fila_9_todo_texto_y_marca_pintados_supera_su_minimo_sobre_su_relleno_en_960x720() {
    comprueba_escenarios(Ventana::Grande);
}

#[test]
fn fila_9_con_errores_y_el_hexadecimal_se_pintan_las_marcas_y_los_errores_y_se_comprueban() {
    // Que la prueba de arriba no sea vacua en lo importante: en estos estados hay marcas de
    // opción y casilla, errores en `WARNING` y el hexadecimal.
    for ppp in PPPS {
        let u = n_de(ppp) as f32 / ppp;
        let mut banco = Banco::listo(ppp, Ventana::Minima, open(formulario()));
        prepara(&mut banco, "opciones_y_casilla");
        let p = banco.reposo();
        let opciones = opciones_de(&banco.dialog, "modo");
        let primera = centro_de(&p, opciones[0]).y;
        let casilla = centro_de(&p, "Opaco").y;
        let marcas = marcas_entre(&p, u, primera - 8.0 * u, casilla + 8.0 * u);
        assert!(
            marcas.len() >= 2,
            "ppp {ppp}: la opción elegida y la casilla se marcan en ACCENT: {marcas:?}"
        );

        let mut banco = Banco::listo(ppp, Ventana::Minima, open(formulario()));
        prepara(&mut banco, "hex_invalido");
        let p = banco.reposo();
        let reason = banco.dialog.reason().expect("hay un motivo");
        let pie = format!("{ERROR_PREFIX}{reason}");
        let t = texto_de(&p, &pie);
        assert!(en_warning(t), "ppp {ppp}: el motivo del pie va en WARNING");
        assert!(
            informe_de(&p)
                .pares
                .iter()
                .any(|(_, fg, bg, min)| *fg == WARNING
                    && *min >= MIN_TEXTO
                    && contrast_ratio(*fg, *bg) >= MIN_TEXTO),
            "ppp {ppp}: el error se compara con su fondo (≥ 4,5)"
        );
    }
}

#[test]
fn fila_9_el_anillo_de_foco_de_cada_elemento_se_pinta_y_supera_3_a_1() {
    for ppp in PPPS {
        for ventana in VENTANAS {
            for escenario in ["defecto", "hex_pendiente_valido", "vacio_y_no_digito"] {
                let u = n_de(ppp) as f32 / ppp;
                let mut banco = Banco::nuevo(ppp, ventana, open(formulario()));
                banco.reposo();
                prepara(&mut banco, escenario);
                let mut p = banco.reposo();
                let orden = focus_order(&banco.dialog);
                assert!(orden.len() >= 8, "{escenario}: {orden:?}");
                for (k, objetivo) in orden.iter().enumerate() {
                    if k > 0 {
                        p = banco.tab(false);
                    }
                    assert_eq!(
                        banco.focus,
                        Some(*objetivo),
                        "ppp {ppp} {ventana:?} {escenario}: paso {k}"
                    );
                    let inf = informe_de(&p);
                    assert!(
                        inf.fallos.is_empty(),
                        "ppp {ppp} {ventana:?} {escenario} foco {objetivo:?}: {:#?}",
                        inf.fallos
                    );
                    // El contraste del anillo frente a lo que tiene debajo lo comprueba `informe_de`.
                    assert!(
                        !anillos(&p, u).is_empty(),
                        "ppp {ppp} {ventana:?} {escenario}: el foco en {objetivo:?} no pinta un \
                         anillo ACCENT de 2 u a la vista"
                    );
                }
            }
        }
    }
}

#[test]
fn fila_9_los_colores_pintados_estan_en_la_tabla_de_pares_con_su_minimo() {
    // «Los colores de la tabla son los que se pintan (no una copia)»: cada par frente/fondo que se
    // pinta está en `CONTRAST_PAIRS`, con un mínimo de al menos 4,5 (texto) o 3,0 (marcas y anillo)
    // y el par lo cumple.
    for ppp in PPPS {
        for ventana in VENTANAS {
            for escenario in [
                "defecto",
                "vacio_y_no_digito",
                "check_falla",
                "hex_invalido",
                "opciones_y_casilla",
                "hover_aceptar",
                "hover_menos",
            ] {
                let mut banco = Banco::nuevo(ppp, ventana, open(formulario()));
                banco.reposo();
                prepara(&mut banco, escenario);
                let p = banco.reposo();
                for (desc, fg, bg, exigido) in informe_de(&p).pares {
                    let en_tabla = CONTRAST_PAIRS
                        .iter()
                        .find(|&&(_, f, b, _)| f == fg && b == bg);
                    let Some(&(nombre, _, _, minimo)) = en_tabla else {
                        panic!(
                            "ppp {ppp} {ventana:?} {escenario}: {desc} se pinta con {fg:?} sobre \
                             {bg:?}, que no está en CONTRAST_PAIRS"
                        );
                    };
                    assert!(
                        minimo >= exigido,
                        "{nombre}: la tabla pide {minimo} y se necesita {exigido}"
                    );
                    assert!(
                        contrast_ratio(fg, bg) >= minimo,
                        "{nombre}: {fg:?} sobre {bg:?} no llega a {minimo}"
                    );
                }
            }
        }
    }
}

#[test]
fn fila_9_solo_se_exime_el_texto_deshabilitado_aceptar_con_errores_y_los_limites() {
    for ppp in PPPS {
        // Con errores, `Aceptar` se pinta deshabilitado (y exento); con todo válido, no.
        let mut banco = Banco::listo(ppp, Ventana::Minima, open(formulario()));
        let valido = banco.reposo();
        assert!(!aceptar_deshabilitado(&valido, &banco.dialog), "ppp {ppp}");
        assert!(informe_de(&valido).exentos.is_empty(), "ppp {ppp}");

        prepara(&mut banco, "vacio_y_no_digito");
        let invalido = banco.reposo();
        assert!(
            aceptar_deshabilitado(&invalido, &banco.dialog),
            "ppp {ppp}: con errores, Aceptar se pinta deshabilitado"
        );
        let inf = informe_de(&invalido);
        assert_eq!(
            inf.exentos,
            [aceptar_etiqueta(&banco.dialog)],
            "ppp {ppp}: lo único exento es Aceptar"
        );

        // `−` en el mínimo y `+` en el máximo.
        let mut banco = Banco::listo(ppp, Ventana::Minima, open(formulario()));
        prepara(&mut banco, "limite_inferior");
        let p = banco.reposo();
        assert!(
            informe_de(&p).exentos.iter().any(|t| t == "−" || t == "-"),
            "ppp {ppp}: el `−` en el mínimo se pinta deshabilitado"
        );
        prepara(&mut banco, "limite_superior");
        let p = banco.reposo();
        assert!(
            informe_de(&p).exentos.iter().any(|t| t == "+"),
            "ppp {ppp}: el `+` en el máximo se pinta deshabilitado"
        );
    }
}

// ---------------------------------------------------------------------------------------------
// Fila 10: piezas y puntero
// ---------------------------------------------------------------------------------------------

#[test]
fn fila_10_se_pinta_titulo_etiquetas_rango_opciones_casilla_y_botones() {
    for ppp in PPPS {
        let banco_dialog = open(formulario());
        let opciones = opciones_de(&banco_dialog, "modo");
        let mut banco = Banco::listo(ppp, Ventana::Minima, banco_dialog);
        let p = banco.reposo();
        let titulo = banco.dialog.spec().title;
        for esperado in [titulo, "Ancho", "Alto", "Modo", "Opaco"] {
            assert!(
                p.textos().any(|(_, t)| t.texto.contains(esperado)),
                "ppp {ppp}: falta {esperado:?}; hay {:?}",
                p.lista_de_textos()
            );
        }
        // El rango visible, una vez por numérico.
        let rangos = p
            .textos()
            .filter(|(_, t)| t.texto.contains("1-8192"))
            .count();
        assert_eq!(rangos, 2, "ppp {ppp}: `1-8192` en cada numérico");
        for opcion in opciones {
            assert!(p.exacto(opcion).is_some(), "ppp {ppp}: falta {opcion:?}");
        }
        assert!(p.exacto(aceptar_etiqueta(&banco.dialog)).is_some());
        assert!(p.exacto(cancelar_etiqueta(&banco.dialog)).is_some());
        // Con todo válido no hay ningún error.
        assert!(
            p.textos()
                .all(|(_, t)| !t.texto.trim_start().starts_with(ERROR_PREFIX)),
            "ppp {ppp}: sin errores no se pinta `{ERROR_PREFIX}`"
        );
        // El hexadecimal está oculto con la opción 0.
        assert!(!banco.dialog.is_shown("color"));
        assert!(p.exacto("Color").is_none(), "ppp {ppp}");
    }
}

#[test]
fn fila_10_el_informe_con_texto_fuera_del_juego_no_pinta_glifos_ausentes() {
    // El contexto del banco entra en pánico ante un glifo ausente: todo texto externo pasa por
    // `printable`.
    for ppp in PPPS {
        let mut banco = Banco::listo(ppp, Ventana::Minima, open(formulario()));
        let p = banco.reposo();
        let todo = p.todo_el_texto();
        assert!(!todo.contains('日'), "ppp {ppp}: {todo:?}");
        assert!(!todo.contains('✓'), "ppp {ppp}: {todo:?}");
        assert!(todo.contains("linea 1"), "ppp {ppp}");
        // También al recorrer el informe con la rueda (entran filas nuevas).
        let centro = texto_que_contiene(&p, "linea 2").rect().center();
        banco.mover_a(centro);
        banco.rueda(true, 3);
        let p = banco.reposo();
        let todo = p.todo_el_texto();
        assert!(
            !todo.contains('日') && !todo.contains('✓'),
            "ppp {ppp}: {todo:?}"
        );
        assert!(
            todo.contains("linea 10"),
            "ppp {ppp}: se ven las filas de más abajo"
        );
    }
}

#[test]
fn fila_10_clic_en_menos_y_mas_cambia_el_texto_y_no_pasa_del_limite() {
    for ppp in PPPS {
        let mut banco = Banco::listo(ppp, Ventana::Minima, open(formulario()));
        let p = banco.reposo();
        assert_eq!(banco.dialog.text("ancho"), Some("64".into()));

        // `+` y `−` del primer numérico (el de arriba).
        let mas0 = mas(&p)[0].center();
        let p = banco.clic(mas0);
        assert_eq!(banco.dialog.text("ancho"), Some("65".into()), "ppp {ppp}");
        assert_eq!(banco.dialog.text("alto"), Some("64".into()), "ppp {ppp}");
        assert!(p.exacto("65").is_some(), "ppp {ppp}: se pinta el 65");
        let menos0 = menos(&p)[0].center();
        banco.clic(menos0);
        banco.clic(menos0);
        assert_eq!(banco.dialog.text("ancho"), Some("63".into()), "ppp {ppp}");

        // El segundo numérico tiene los suyos.
        let p = banco.reposo();
        let mas1 = mas(&p)[1].center();
        banco.clic(mas1);
        assert_eq!(banco.dialog.text("alto"), Some("65".into()), "ppp {ppp}");
        assert_eq!(banco.dialog.text("ancho"), Some("63".into()), "ppp {ppp}");

        // Los límites: no pasan.
        numero(&mut banco, "ancho", "1");
        let p = banco.reposo();
        banco.clic(menos(&p)[0].center());
        assert_eq!(
            banco.dialog.text("ancho"),
            Some("1".into()),
            "ppp {ppp}: min"
        );
        numero(&mut banco, "ancho", "8192");
        let p = banco.reposo();
        banco.clic(mas(&p)[0].center());
        assert_eq!(
            banco.dialog.text("ancho"),
            Some("8192".into()),
            "ppp {ppp}: max"
        );
        // Con el texto inválido, `+` pone el valor por defecto.
        numero(&mut banco, "ancho", "abc");
        let p = banco.reposo();
        banco.clic(mas(&p)[0].center());
        assert_eq!(
            banco.dialog.text("ancho"),
            Some("64".into()),
            "ppp {ppp}: inválido → por defecto"
        );
    }
}

#[test]
fn fila_10_clic_en_una_opcion_la_marca_y_deja_una_sola_marcada() {
    for ppp in PPPS {
        let u = n_de(ppp) as f32 / ppp;
        let dialog = open(formulario());
        let opciones = opciones_de(&dialog, "modo");
        let mut banco = Banco::listo(ppp, Ventana::Minima, dialog);
        for indice in [2usize, 1, 0, 2] {
            let p = banco.reposo();
            let centro = centro_de(&p, opciones[indice]);
            banco.clic(centro);
            let values = banco.dialog.values().expect("sigue válido");
            assert_eq!(values.choice("modo"), Some(indice), "ppp {ppp}");
            assert_eq!(
                banco.dialog.is_shown("color"),
                indice == 2,
                "ppp {ppp}: el hexadecimal solo se muestra con la opción 2"
            );

            let p = banco.reposo();
            let ys: Vec<f32> = opciones.iter().map(|o| centro_de(&p, o).y).collect();
            let marcas = marcas_entre(&p, u, ys[0] - 7.0 * u, ys[ys.len() - 1] + 7.0 * u);
            assert_eq!(
                marcas.len(),
                1,
                "ppp {ppp}: una sola opción marcada, elegida {indice}: {marcas:?}"
            );
            assert!(
                (marcas[0].center().y - ys[indice]).abs() <= 7.0 * u,
                "ppp {ppp}: la marca está en la opción {indice}"
            );
        }
    }
}

#[test]
fn fila_10_clic_en_la_casilla_la_alterna_y_pinta_su_marca() {
    for ppp in PPPS {
        let u = n_de(ppp) as f32 / ppp;
        let mut banco = Banco::listo(ppp, Ventana::Minima, open(formulario()));
        for esperado in [true, false, true] {
            let p = banco.reposo();
            let t = texto_de(&p, "Opaco");
            let centro = t.rect().center();
            banco.clic(centro);
            let values = banco.dialog.values().expect("sigue válido");
            assert_eq!(values.check("opaco"), Some(esperado), "ppp {ppp}");

            let p = banco.reposo();
            let y = texto_de(&p, "Opaco").rect().center().y;
            let marcas = marcas_entre(&p, u, y - 8.0 * u, y + 8.0 * u);
            assert_eq!(
                !marcas.is_empty(),
                esperado,
                "ppp {ppp}: la marca de la casilla solo se pinta con la casilla marcada: {marcas:?}"
            );
        }
    }
}

/// Lo tecleado, y el error en línea y el del pie que se esperan (sin prefijo).
const TECLEADO: [(&str, Option<&str>); 9] = [
    ("abc", Some("solo dígitos 0-9")),
    ("-5", Some("solo dígitos 0-9")),
    ("12345678901", Some("máximo 10 cifras")),
    ("8193", Some("máximo 8192")),
    ("0", Some("mínimo 1")),
    ("", Some("escribe un número")),
    ("500", None),
    ("007", None),
    ("0000000064", None),
];

#[test]
fn fila_10_teclear_en_el_numerico_muestra_el_error_en_linea_con_prefijo_y_deshabilita_aceptar() {
    for ppp in PPPS {
        let mut banco = Banco::listo(ppp, Ventana::Minima, open(formulario()));
        // El foco inicial es el primer campo (`ancho`).
        assert_eq!(banco.focus, Some(FocusTarget::Field("ancho")), "ppp {ppp}");
        for (tecleado, error) in TECLEADO {
            let p = banco.escribe(tecleado);
            assert_eq!(
                banco.dialog.text("ancho"),
                Some(tecleado.to_owned()),
                "ppp {ppp}: {tecleado:?}"
            );
            match error {
                Some(motivo) => {
                    let en_linea = format!("{ERROR_PREFIX}{motivo}");
                    let t = texto_de(&p, &en_linea);
                    assert!(en_warning(t), "ppp {ppp}: {en_linea:?} va en WARNING");
                    // El motivo del pie: `error: Ancho: motivo`.
                    let pie = format!("{ERROR_PREFIX}Ancho: {motivo}");
                    assert_eq!(
                        banco.dialog.reason().as_deref(),
                        Some(format!("Ancho: {motivo}").as_str()),
                        "ppp {ppp}"
                    );
                    let t = texto_de(&p, &pie);
                    assert!(en_warning(t), "ppp {ppp}: {pie:?} va en WARNING");
                    assert!(
                        aceptar_deshabilitado(&p, &banco.dialog),
                        "ppp {ppp}: {tecleado:?}: Aceptar deshabilitado"
                    );
                    assert!(banco.dialog.values().is_none());
                }
                None => {
                    assert!(
                        p.textos()
                            .all(|(_, t)| !t.texto.trim_start().starts_with(ERROR_PREFIX)),
                        "ppp {ppp}: {tecleado:?} es válido: sin errores"
                    );
                    assert!(!aceptar_deshabilitado(&p, &banco.dialog));
                    assert!(banco.dialog.values().is_some());
                }
            }
        }
        // Una cadena larguísima se acota a 11 caracteres (y es `TooLong`).
        banco.escribe(&"9".repeat(40));
        let texto = banco.dialog.text("ancho").expect("el campo existe");
        assert!(texto.chars().count() <= 11, "ppp {ppp}: {texto:?}");
        assert!(banco.dialog.reason().is_some());
    }
}

#[test]
fn fila_10_teclear_en_el_segundo_numerico_cambia_solo_ese_campo() {
    for ppp in PPPS {
        let mut banco = Banco::listo(ppp, Ventana::Minima, open(formulario()));
        let p = banco.reposo();
        // El segundo «64» es el del alto: clic en él le da el foco.
        let valores = p.todos_exactos("64");
        assert_eq!(valores.len(), 2, "ppp {ppp}: {:?}", p.lista_de_textos());
        let centro = valores[1].rect().center();
        banco.clic(centro);
        assert_eq!(banco.focus, Some(FocusTarget::Field("alto")), "ppp {ppp}");
        banco.escribe("12");
        assert_eq!(banco.dialog.text("alto"), Some("12".into()), "ppp {ppp}");
        assert_eq!(banco.dialog.text("ancho"), Some("64".into()), "ppp {ppp}");
    }
}

#[test]
fn fila_10_intro_acepta_con_todo_valido_y_con_errores_no_y_la_repeticion_no_acepta() {
    for ppp in PPPS {
        // Todo válido: `Intro` en el campo acepta con los valores.
        let mut banco = Banco::listo(ppp, Ventana::Minima, open(formulario()));
        banco.escribe("500");
        banco.reacciones.clear();
        banco.pulsa(Key::Enter, Modifiers::NONE);
        let aceptadas = aceptadas(&banco.reacciones);
        assert_eq!(aceptadas.len(), 1, "ppp {ppp}: {:?}", banco.reacciones);
        assert_eq!(aceptadas[0].number("ancho"), Some(500), "ppp {ppp}");
        assert_eq!(aceptadas[0].number("alto"), Some(64), "ppp {ppp}");
        assert_eq!(canceladas(&banco.reacciones), 0);

        // Con errores no hace nada: ni acepta, ni cancela, ni cambia el texto.
        let mut banco = Banco::listo(ppp, Ventana::Minima, open(formulario()));
        banco.escribe("abc");
        banco.reacciones.clear();
        banco.pulsa(Key::Enter, Modifiers::NONE);
        assert!(
            !cierra(&banco.reacciones),
            "ppp {ppp}: {:?}",
            banco.reacciones
        );
        assert_eq!(banco.dialog.text("ancho"), Some("abc".into()), "ppp {ppp}");

        // Una repetición de teclado (`Intro` mantenida) no acepta.
        let mut banco = Banco::listo(ppp, Ventana::Minima, open(formulario()));
        banco.reacciones.clear();
        let repeticion = Event::Key {
            key: Key::Enter,
            physical_key: Some(Key::Enter),
            pressed: true,
            repeat: true,
            modifiers: Modifiers::NONE,
        };
        banco.fotograma(vec![repeticion]);
        banco.reposo();
        assert!(
            !cierra(&banco.reacciones),
            "ppp {ppp}: {:?}",
            banco.reacciones
        );
    }
}

#[test]
fn fila_10_la_perdida_de_foco_conserva_el_texto_del_numerico_y_su_error() {
    for ppp in PPPS {
        let mut banco = Banco::listo(ppp, Ventana::Minima, open(formulario()));
        banco.escribe("abc");
        banco.tab(false);
        assert_ne!(banco.focus, Some(FocusTarget::Field("ancho")), "ppp {ppp}");
        assert_eq!(banco.dialog.text("ancho"), Some("abc".into()), "ppp {ppp}");
        let p = banco.reposo();
        assert!(
            p.exacto(&format!("{ERROR_PREFIX}solo dígitos 0-9"))
                .is_some(),
            "ppp {ppp}: el error sigue en línea"
        );
        assert!(aceptar_deshabilitado(&p, &banco.dialog), "ppp {ppp}");
        assert!(!cierra(&banco.reacciones), "ppp {ppp}");
    }
}

#[test]
fn fila_10_escape_cancela_una_sola_vez() {
    for ppp in PPPS {
        let mut banco = Banco::listo(ppp, Ventana::Minima, open(formulario()));
        banco.reacciones.clear();
        banco.pulsa(Key::Escape, Modifiers::NONE);
        assert_eq!(canceladas(&banco.reacciones), 1, "ppp {ppp}");
        assert!(aceptadas(&banco.reacciones).is_empty(), "ppp {ppp}");
        // También con errores (no hace falta que sea válido para cancelar).
        let mut banco = Banco::listo(ppp, Ventana::Minima, open(formulario()));
        banco.escribe("abc");
        banco.reacciones.clear();
        banco.pulsa(Key::Escape, Modifiers::NONE);
        assert_eq!(canceladas(&banco.reacciones), 1, "ppp {ppp}: con errores");
    }
}

#[test]
fn fila_10_clic_en_aceptar_con_errores_no_acepta_y_el_motivo_es_visible() {
    for ppp in PPPS {
        let mut banco = Banco::listo(ppp, Ventana::Minima, open(formulario()));
        prepara(&mut banco, "vacio_y_no_digito");
        let p = banco.reposo();
        let motivo = banco.dialog.reason().expect("hay un motivo");
        assert_eq!(
            motivo, "Ancho: solo dígitos 0-9",
            "el primero en orden de campos"
        );
        assert!(
            p.exacto(&format!("{ERROR_PREFIX}{motivo}")).is_some(),
            "ppp {ppp}: el motivo es visible en el pie; hay {:?}",
            p.lista_de_textos()
        );
        assert!(aceptar_deshabilitado(&p, &banco.dialog), "ppp {ppp}");
        banco.reacciones.clear();
        banco.clic(centro_de(&p, aceptar_etiqueta(&banco.dialog)));
        assert!(
            !cierra(&banco.reacciones),
            "ppp {ppp}: {:?}",
            banco.reacciones
        );
        let p = banco.reposo();
        assert!(
            p.exacto(&format!("{ERROR_PREFIX}{motivo}")).is_some(),
            "ppp {ppp}: el motivo sigue visible tras el clic"
        );
    }
}

#[test]
fn fila_10_el_motivo_de_la_comprobacion_cruzada_tambien_es_visible() {
    for ppp in PPPS {
        let mut banco = Banco::listo(ppp, Ventana::Minima, open(formulario()));
        prepara(&mut banco, "check_falla");
        let p = banco.reposo();
        let motivo = banco.dialog.reason().expect("el check falla");
        assert_eq!(motivo, "demasiado grande");
        let t = texto_de(&p, &format!("{ERROR_PREFIX}{motivo}"));
        assert!(en_warning(t), "ppp {ppp}");
        assert!(aceptar_deshabilitado(&p, &banco.dialog), "ppp {ppp}");
    }
}

#[test]
fn fila_10_clic_en_aceptar_con_todo_valido_da_accepted_con_los_valores() {
    for ppp in PPPS {
        let mut banco = Banco::listo(ppp, Ventana::Minima, open(formulario()));
        let p = banco.reposo();
        banco.reacciones.clear();
        banco.clic(centro_de(&p, aceptar_etiqueta(&banco.dialog)));
        let aceptadas = aceptadas(&banco.reacciones);
        assert_eq!(aceptadas.len(), 1, "ppp {ppp}: {:?}", banco.reacciones);
        assert_eq!(aceptadas[0].number("ancho"), Some(64), "ppp {ppp}");
        assert_eq!(aceptadas[0].number("alto"), Some(64), "ppp {ppp}");
        assert_eq!(aceptadas[0].choice("modo"), Some(0), "ppp {ppp}");
        assert_eq!(aceptadas[0].check("opaco"), Some(false), "ppp {ppp}");
        assert!(aceptadas[0].color("color").is_none(), "ppp {ppp}: oculto");
        assert_eq!(canceladas(&banco.reacciones), 0);
    }
}

#[test]
fn fila_10_clic_en_cancelar_da_cancelled_y_nunca_entrega_valores() {
    for ppp in PPPS {
        let mut banco = Banco::listo(ppp, Ventana::Minima, open(formulario()));
        let p = banco.reposo();
        banco.reacciones.clear();
        banco.clic(centro_de(&p, cancelar_etiqueta(&banco.dialog)));
        assert_eq!(canceladas(&banco.reacciones), 1, "ppp {ppp}");
        assert!(aceptadas(&banco.reacciones).is_empty(), "ppp {ppp}");
        // `Cancelar` también con errores.
        let mut banco = Banco::listo(ppp, Ventana::Minima, open(formulario()));
        prepara(&mut banco, "vacio_y_no_digito");
        let p = banco.reposo();
        banco.reacciones.clear();
        banco.clic(centro_de(&p, cancelar_etiqueta(&banco.dialog)));
        assert_eq!(canceladas(&banco.reacciones), 1, "ppp {ppp}: con errores");
    }
}

#[test]
fn fila_10_el_hexadecimal_enseña_su_error_en_linea_y_en_directo() {
    for ppp in PPPS {
        let mut banco = Banco::listo(ppp, Ventana::Minima, open(formulario()));
        prepara(&mut banco, "hex");
        banco.reposo();
        banco.foco_hasta(FocusTarget::Field("color"));
        let p = banco.escribe("zz");
        assert!(banco.dialog.hex_pending("color"), "ppp {ppp}");
        let Some(Validity::Invalid(motivo)) = banco.dialog.validity("color") else {
            panic!("ppp {ppp}: `zz` no es un color");
        };
        let t = texto_de(&p, &format!("{ERROR_PREFIX}{motivo}"));
        assert!(en_warning(t), "ppp {ppp}: error en línea del hexadecimal");
        assert!(aceptar_deshabilitado(&p, &banco.dialog), "ppp {ppp}");
        // Uno válido, aún sin confirmar: sin error y `Aceptar` habilitado.
        let p = banco.escribe("#12f");
        assert!(banco.dialog.hex_pending("color"), "ppp {ppp}");
        assert!(
            p.textos()
                .all(|(_, t)| !t.texto.trim_start().starts_with(ERROR_PREFIX)),
            "ppp {ppp}: #12f es válido"
        );
    }
}

#[test]
fn fila_10_la_zona_se_ajusta_a_42_columnas_avanza_3_lineas_por_muesca_y_se_recorta() {
    for ppp in PPPS {
        let total = wrap_text(&texto_del_informe(&open(formulario())), 42).len();
        assert_eq!(total, 30);
        let mut banco = Banco::listo(ppp, Ventana::Minima, open(formulario()));
        let p = banco.reposo();
        // 4 filas, la primera línea al principio.
        let numeros = numeros_de_linea(&p);
        assert_eq!(
            numeros,
            [1, 2, 3, 4],
            "ppp {ppp}: 4 filas desde el principio"
        );
        assert_eq!(banco.dialog.first_line("informe"), 0);

        // El puntero sobre la zona.
        let centro = texto_que_contiene(&p, "linea 2").rect().center();
        banco.mover_a(centro);
        banco.rueda(true, 1);
        assert_eq!(
            banco.dialog.first_line("informe"),
            3,
            "ppp {ppp}: 3 por muesca"
        );
        banco.rueda(true, 2);
        assert_eq!(banco.dialog.first_line("informe"), 9, "ppp {ppp}");
        banco.rueda(false, 1);
        assert_eq!(banco.dialog.first_line("informe"), 6, "ppp {ppp}");
        let p = banco.reposo();
        assert_eq!(numeros_de_linea(&p), [7, 8, 9, 10], "ppp {ppp}");

        // Se recorta a `total − filas` y la última línea queda a la vista.
        banco.rueda(true, 1000);
        let maximo = clamp_first_line(usize::MAX / 2, total, 4);
        assert_eq!(maximo, 26);
        assert_eq!(banco.dialog.first_line("informe"), maximo, "ppp {ppp}");
        let p = banco.reposo();
        assert_eq!(numeros_de_linea(&p), [27, 28, 29, 30], "ppp {ppp}");
        // Sigue sobre la zona aunque el cuerpo se hubiera movido: se vuelve a poner el puntero.
        let centro = texto_que_contiene(&p, "linea 28").rect().center();
        banco.mover_a(centro);
        banco.rueda(false, 1000);
        assert_eq!(banco.dialog.first_line("informe"), 0, "ppp {ppp}");
    }
}

#[test]
fn fila_10_una_linea_larga_del_informe_se_corta_a_42_columnas() {
    for ppp in PPPS {
        let spec = con_informe(|_| format!("{}\nlinea 1\nlinea 2", "a".repeat(100)));
        let mut banco = Banco::listo(ppp, Ventana::Minima, open(spec));
        let p = banco.reposo();
        // Las filas de solo `a`, en el orden en que se pintan (de arriba abajo).
        let largos: Vec<usize> = p
            .textos()
            .flat_map(|(_, t)| t.texto.lines())
            .map(str::trim)
            .filter(|l| !l.is_empty() && l.chars().all(|c| c == 'a'))
            .map(|l| l.chars().count())
            .collect();
        assert_eq!(largos, [42, 42, 16], "ppp {ppp}: 100 letras en 42 columnas");
    }
}

#[test]
fn fila_10_las_teclas_con_el_foco_en_la_zona_la_desplazan() {
    for ppp in PPPS {
        let mut banco = Banco::listo(ppp, Ventana::Minima, open(formulario()));
        banco.foco_hasta(FocusTarget::Zone("informe"));
        let maximo = 26;
        assert_eq!(banco.dialog.first_line("informe"), 0);

        banco.pulsa(Key::ArrowDown, Modifiers::NONE);
        let tras_flecha = banco.dialog.first_line("informe");
        assert!(tras_flecha >= 1, "ppp {ppp}: la flecha abajo avanza");
        banco.pulsa(Key::ArrowUp, Modifiers::NONE);
        assert!(
            banco.dialog.first_line("informe") < tras_flecha,
            "ppp {ppp}"
        );

        banco.pulsa(Key::PageDown, Modifiers::NONE);
        let tras_pagina = banco.dialog.first_line("informe");
        assert!(tras_pagina > 0 && tras_pagina <= maximo, "ppp {ppp}: AvPág");
        banco.pulsa(Key::End, Modifiers::NONE);
        assert_eq!(banco.dialog.first_line("informe"), maximo, "ppp {ppp}: Fin");
        banco.pulsa(Key::PageUp, Modifiers::NONE);
        let tras_re = banco.dialog.first_line("informe");
        assert!(tras_re < maximo, "ppp {ppp}: RePág");
        banco.pulsa(Key::Home, Modifiers::NONE);
        assert_eq!(banco.dialog.first_line("informe"), 0, "ppp {ppp}: Inicio");
        // `Intro` en la zona no hace nada.
        banco.reacciones.clear();
        banco.pulsa(Key::Enter, Modifiers::NONE);
        assert!(!cierra(&banco.reacciones), "ppp {ppp}: Intro en la zona");
    }
}

/// Un punto del velo (fuera del diálogo): dentro del botón de detrás, cerca de su esquina.
fn sobre_el_boton(banco: &Banco) -> Pos2 {
    let boton = banco.detras.as_ref().expect("hay lo de detrás").boton;
    assert!(boton.is_positive(), "el botón de detrás no se ha pintado");
    boton.min + vec2(4.0, boton.height() / 2.0)
}

/// Un punto del velo que solo toca el lienzo simulado (la esquina inferior derecha).
fn sobre_el_lienzo(banco: &Banco) -> Pos2 {
    let p = banco.pantalla();
    pos2(p.x - 3.0, p.y - 3.0)
}

#[test]
fn fila_10_el_velo_consume_clic_arrastre_y_rueda_sin_cerrar_ni_llegar_a_lo_de_detras() {
    for ppp in PPPS {
        let mut banco = Banco::con_detras(ppp, Ventana::Minima, open(formulario()));
        let rects = banco.rects();
        let dialogo = rect_pt(rects.dialog, ppp);
        let puntos = [sobre_el_boton(&banco), sobre_el_lienzo(&banco)];
        for p in puntos {
            assert!(!dialogo.contains(p), "ppp {ppp}: {p:?} cae en el diálogo");
        }
        let antes = banco.dialog.clone();
        banco.reacciones.clear();
        banco.detras.as_mut().expect("detrás").a_cero();

        for p in puntos {
            // Clic.
            banco.fotograma(vec![mover(p)]);
            banco.reposo();
            banco.fotograma(vec![boton(p, true)]);
            banco.fotograma(vec![boton(p, false)]);
            banco.reposo();
            // Arrastre.
            let q = p + vec2(3.0, 2.0);
            banco.fotograma(vec![boton(p, true)]);
            banco.fotograma(vec![mover(q)]);
            banco.fotograma(vec![boton(q, false)]);
            banco.reposo();
            // Rueda, hacia los dos lados.
            banco.rueda(true, 2);
            banco.rueda(false, 3);
            banco.reposo();
        }

        assert!(
            banco.reacciones.is_empty(),
            "ppp {ppp}: el velo no cierra ni cambia nada: {:?}",
            banco.reacciones
        );
        assert_eq!(banco.dialog, antes, "ppp {ppp}: el diálogo no cambia");
        let detras = banco.detras.as_ref().expect("detrás");
        assert!(
            detras.nada_recibido(),
            "ppp {ppp}: lo de detrás recibió entrada: {detras:?}"
        );
        // Y el diálogo sigue ahí.
        let p = banco.reposo();
        assert!(
            p.exacto(aceptar_etiqueta(&banco.dialog)).is_some(),
            "ppp {ppp}"
        );
    }
}

#[test]
fn fila_10_sin_el_dialogo_lo_de_detras_si_recibe_el_clic_y_la_rueda() {
    // El control de la prueba anterior: el mismo banco con el diálogo sin mostrar.
    for ppp in PPPS {
        let mut banco = Banco::sin_dialogo(ppp, Ventana::Minima, open(formulario()));
        let sobre_boton = sobre_el_boton(&banco);
        let sobre_lienzo = sobre_el_lienzo(&banco);
        banco.detras.as_mut().expect("detrás").a_cero();

        banco.clic(sobre_boton);
        let d = banco.detras.as_ref().expect("detrás");
        assert!(
            d.clics_boton >= 1,
            "ppp {ppp}: el botón recibe el clic: {d:?}"
        );
        assert!(d.hovers_boton >= 1, "ppp {ppp}");

        banco.mover_a(sobre_lienzo);
        banco.rueda(true, 2);
        banco.clic(sobre_lienzo);
        let d = banco.detras.as_ref().expect("detrás");
        assert!(
            d.ruedas_lienzo >= 1,
            "ppp {ppp}: el lienzo recibe la rueda: {d:?}"
        );
        assert!(
            d.clics_lienzo >= 1,
            "ppp {ppp}: el lienzo recibe el clic: {d:?}"
        );
    }
}

#[test]
fn fila_10_el_velo_no_cierra_con_un_clic_fuera_y_el_clic_dentro_en_blanco_tampoco() {
    for ppp in PPPS {
        let mut banco = Banco::listo(ppp, Ventana::Minima, open(formulario()));
        let rects = banco.rects();
        let antes = banco.dialog.clone();
        banco.reacciones.clear();
        // Fuera del diálogo, en las cuatro esquinas de la ventana.
        let pantalla = banco.pantalla();
        for p in [
            pos2(1.0, 1.0),
            pos2(pantalla.x - 1.0, 1.0),
            pos2(1.0, pantalla.y - 1.0),
            pos2(pantalla.x - 1.0, pantalla.y - 1.0),
        ] {
            banco.clic(p);
        }
        // Dentro, en el relleno izquierdo del cuerpo (ninguna pieza).
        let cuerpo = rect_pt(rects.body, ppp);
        banco.clic(pos2(cuerpo.min.x + 3.0 * banco.u(), cuerpo.center().y));
        assert!(
            banco.reacciones.is_empty(),
            "ppp {ppp}: {:?}",
            banco.reacciones
        );
        assert_eq!(estado(&banco.dialog), estado(&antes), "ppp {ppp}");
    }
}

// ---------------------------------------------------------------------------------------------
// Fila 11: geometría
// ---------------------------------------------------------------------------------------------

fn comprueba_geometria(banco: &mut Banco, etiqueta: &str) {
    let ppp = banco.ppp;
    let ventana = banco.ventana;
    let p = banco.reposo();
    let rects = banco.rects();
    let donde = format!("ppp {ppp} {ventana:?} {etiqueta}");
    let (_, fondo) =
        relleno_del_dialogo(&p).unwrap_or_else(|| panic!("{donde}: sin fondo SURFACE_1"));
    assert!(
        coincide(fondo.rect, rects.dialog, ppp),
        "{donde}: el diálogo pintado es {:?} y el de dialog_layout {:?}",
        fondo.rect,
        rects.dialog
    );

    // Título, botones y motivo, en sus rectángulos.
    let titulo = texto_de(&p, banco.dialog.spec().title).rect();
    assert!(
        dentro(titulo, rects.title, ppp),
        "{donde}: título {titulo:?} fuera de {:?}",
        rects.title
    );
    let aceptar = texto_de(&p, aceptar_etiqueta(&banco.dialog)).rect();
    assert!(
        dentro(aceptar, rects.accept, ppp),
        "{donde}: Aceptar {aceptar:?}"
    );
    let cancelar = texto_de(&p, cancelar_etiqueta(&banco.dialog)).rect();
    let rect_cancelar = rects.cancel.expect("hay Cancelar");
    assert!(
        dentro(cancelar, rect_cancelar, ppp),
        "{donde}: Cancelar {cancelar:?}"
    );
    for (esperado, nombre) in [(rects.accept, "Aceptar"), (rect_cancelar, "Cancelar")] {
        assert!(
            p.rellenos().any(|(_, r)| coincide(r.rect, esperado, ppp)),
            "{donde}: ningún relleno es el botón {nombre} ({esperado:?})"
        );
    }
    if let Some(reason) = banco.dialog.reason() {
        let motivo = texto_de(&p, &format!("{ERROR_PREFIX}{reason}")).rect();
        assert!(
            dentro(motivo, rects.reason, ppp),
            "{donde}: motivo {motivo:?} fuera de {:?}",
            rects.reason
        );
    }

    // Los botones de 68n × 16n con 4n de hueco, a la derecha del pie y dentro de él.
    let n = banco.n();
    assert_eq!(
        (rects.accept.width, rects.accept.height),
        (68 * n, 16 * n),
        "{donde}"
    );
    assert_eq!(
        (rect_cancelar.width, rect_cancelar.height),
        (68 * n, 16 * n),
        "{donde}"
    );
    assert_eq!(
        rect_cancelar.x - (rects.accept.x + rects.accept.width),
        4 * n,
        "{donde}"
    );

    // Ningún texto ni forma sale del diálogo salvo el velo; el cuerpo no invade título ni pie.
    for (_, t) in p.textos() {
        let visible = t.rect().intersect(t.clip);
        if t.texto.trim().is_empty() || !visible.is_positive() {
            continue;
        }
        assert!(
            dentro(visible, rects.title, ppp)
                || dentro(visible, rects.body, ppp)
                || dentro(visible, rects.footer, ppp),
            "{donde}: el texto {:?} ({visible:?}) no cabe en el título, el cuerpo ni el pie",
            t.texto
        );
    }
    for (_, r) in p.rellenos() {
        if r.fill == Color32::from_black_alpha(128) {
            continue;
        }
        let visible = r.rect.intersect(r.clip);
        if !visible.is_positive() {
            continue;
        }
        assert!(
            dentro(visible, rects.dialog, ppp),
            "{donde}: un relleno {:?} sale del diálogo {:?}",
            visible,
            rects.dialog
        );
    }
    for (_, t) in p.trazos() {
        let visible = t.rect.expand(t.ancho / 2.0).intersect(t.clip);
        if !visible.is_positive() {
            continue;
        }
        assert!(
            dentro(visible, rects.dialog, ppp),
            "{donde}: un trazo {visible:?} sale del diálogo"
        );
    }
}

#[test]
fn fila_11_los_rectangulos_pintados_son_los_de_dialog_layout() {
    for ppp in PPPS {
        for ventana in VENTANAS {
            for escenario in ["defecto", "vacio_y_no_digito", "hex", "check_falla"] {
                let mut banco = Banco::nuevo(ppp, ventana, open(formulario()));
                banco.reposo();
                prepara(&mut banco, escenario);
                comprueba_geometria(&mut banco, escenario);
            }
        }
    }
}

#[test]
fn fila_11_con_contenido_enorme_tambien_cuadra_la_geometria() {
    for ppp in PPPS {
        for ventana in VENTANAS {
            let mut banco = Banco::nuevo(ppp, ventana, open(huge(60)));
            banco.reposo();
            comprueba_geometria(&mut banco, "huge");
            banco.dialog.set_scroll_u(100);
            comprueba_geometria(&mut banco, "huge con scroll 100");
        }
    }
}

#[test]
fn fila_11_el_velo_cubre_la_ventana_entera_con_alfa_128_por_debajo_del_dialogo() {
    for ppp in PPPS {
        for ventana in VENTANAS {
            let mut banco = Banco::listo(ppp, ventana, open(formulario()));
            let p = banco.reposo();
            let pantalla = Rect::from_min_size(Pos2::ZERO, banco.pantalla());
            let velos = veladura(&p);
            assert!(
                !velos.is_empty(),
                "ppp {ppp} {ventana:?}: no hay velo (negro de alfa 128)"
            );
            assert!(
                velos
                    .iter()
                    .any(|(_, r)| r.rect.expand(0.01).contains_rect(pantalla)),
                "ppp {ppp} {ventana:?}: el velo no cubre la ventana: {velos:?}"
            );
            let (i_dialogo, _) = relleno_del_dialogo(&p).expect("el diálogo se pinta");
            assert!(
                velos.iter().all(|(i, _)| *i < i_dialogo),
                "ppp {ppp} {ventana:?}: el velo va debajo del diálogo"
            );
        }
    }
}

#[test]
fn fila_11_el_dialogo_queda_centrado_con_margen_de_8_u_en_la_ventana_minima() {
    for ppp in PPPS {
        let mut banco = Banco::listo(ppp, Ventana::Minima, open(formulario()));
        let p = banco.reposo();
        let n = banco.n();
        let (_, fondo) = relleno_del_dialogo(&p).expect("el diálogo se pinta");
        let (ancho, alto) = medidas(ppp, Ventana::Minima).0;
        let rect = fondo.rect;
        let (izq, der) = (rect.min.x * ppp, ancho as f32 - rect.max.x * ppp);
        let (arr, aba) = (rect.min.y * ppp, alto as f32 - rect.max.y * ppp);
        let tol = tolerancia_de_egui(ppp) + 1.0;
        assert!((izq - der).abs() <= tol, "ppp {ppp}: {izq} y {der}");
        assert!((arr - aba).abs() <= tol, "ppp {ppp}: {arr} y {aba}");
        assert!(
            izq >= 8.0 * n as f32 - tol && arr >= 8.0 * n as f32 - tol,
            "ppp {ppp}: margen"
        );
        assert!(
            (rect.width() * ppp - 330.0 * n as f32).abs() <= tol,
            "ppp {ppp}: ancho 330 u"
        );
    }
}

// ---------------------------------------------------------------------------------------------
// Fila 11: contenido enorme y rueda
// ---------------------------------------------------------------------------------------------

/// El puntero en el relleno izquierdo del cuerpo (sin pieza debajo).
fn puntero_en_el_cuerpo(banco: &mut Banco) {
    let cuerpo = rect_pt(banco.rects().body, banco.ppp);
    let pos = pos2(cuerpo.min.x + 3.0 * banco.u(), cuerpo.center().y);
    banco.mover_a(pos);
}

/// Posiciones (puntos) de los textos de título y pie.
fn fijos(p: &Pintura, dialog: &Dialog) -> Vec<(String, Pos2)> {
    let mut v = Vec::new();
    for etiqueta in [
        dialog.spec().title,
        aceptar_etiqueta(dialog),
        cancelar_etiqueta(dialog),
    ] {
        let t = texto_de(p, etiqueta);
        v.push((etiqueta.to_owned(), t.pos));
    }
    v
}

/// Cada texto de `despues` (visible, en el cuerpo) estaba `delta` puntos más abajo antes, salvo los
/// que acaban de entrar por abajo. Devuelve cuántos se han podido contrastar.
fn desplazados(antes: &Pintura, despues: &Pintura, cuerpo: Rect, delta: f32, tol: f32) -> usize {
    let visibles = |p: &Pintura| -> Vec<(String, Pos2)> {
        p.textos()
            .map(|(_, t)| t)
            .filter(|t| {
                let v = t.rect().intersect(t.clip);
                v.is_positive() && cuerpo.expand(tol).contains_rect(v)
            })
            .map(|t| (t.texto.clone(), t.pos))
            .collect()
    };
    let (antes, despues) = (visibles(antes), visibles(despues));
    let mut contrastados = 0;
    for (texto, pos) in &despues {
        let estaba = antes.iter().any(|(t, q)| {
            t == texto && (q.x - pos.x).abs() <= tol && (q.y - (pos.y + delta)).abs() <= tol
        });
        if estaba {
            contrastados += 1;
        } else {
            // Solo puede ser algo que entra por abajo.
            assert!(
                pos.y + delta > cuerpo.max.y - tol,
                "el texto {texto:?} en {pos:?} no estaba {delta} pt más abajo antes ni entra por abajo"
            );
        }
    }
    contrastados
}

#[test]
fn fila_11_la_rueda_desplaza_el_cuerpo_42_u_por_muesca_sin_pasar_de_max_scroll() {
    for ppp in PPPS {
        for ventana in VENTANAS {
            let mut banco = Banco::listo(ppp, ventana, open(huge(60)));
            let rects = banco.rects();
            let maximo = rects.max_scroll_u;
            assert!(
                maximo > 5 * 42,
                "ppp {ppp} {ventana:?}: contenido enorme: {maximo}"
            );
            assert_eq!(banco.dialog.scroll_u(), 0);
            puntero_en_el_cuerpo(&mut banco);

            let mut esperado = 0;
            for (abajo, muescas) in [(true, 1), (true, 3), (false, 1), (true, 2)] {
                banco.rueda(abajo, muescas);
                let delta = 42 * muescas as i64;
                esperado = if abajo {
                    (esperado + delta).min(maximo)
                } else {
                    (esperado - delta).max(0)
                };
                assert_eq!(
                    banco.dialog.scroll_u(),
                    esperado,
                    "ppp {ppp} {ventana:?}: abajo={abajo} muescas={muescas}"
                );
                banco.reposo();
                assert_eq!(
                    banco.dialog.scroll_u(),
                    esperado,
                    "ppp {ppp} {ventana:?}: se mantiene"
                );
            }

            // Recorte: ni por abajo ni por arriba.
            banco.rueda(true, 10_000);
            assert_eq!(
                banco.dialog.scroll_u(),
                maximo,
                "ppp {ppp} {ventana:?}: tope"
            );
            banco.rueda(true, 1);
            assert_eq!(
                banco.dialog.scroll_u(),
                maximo,
                "ppp {ppp} {ventana:?}: no pasa"
            );
            banco.rueda(false, 10_000);
            assert_eq!(banco.dialog.scroll_u(), 0, "ppp {ppp} {ventana:?}: suelo");
        }
    }
}

#[test]
fn fila_11_al_desplazar_el_cuerpo_lo_visible_se_mueve_lo_mismo_y_titulo_y_pie_no_se_mueven() {
    for ppp in PPPS {
        for ventana in VENTANAS {
            let mut banco = Banco::listo(ppp, ventana, open(huge(60)));
            puntero_en_el_cuerpo(&mut banco);
            let cuerpo = rect_pt(banco.rects().body, ppp);
            let u = banco.u();
            let tol = tolerancia_de_egui(ppp) / ppp + 0.01;
            for muescas in [1usize, 2] {
                let antes = banco.reposo();
                let fijos_antes = fijos(&antes, &banco.dialog);
                let scroll_antes = banco.dialog.scroll_u();
                banco.rueda(true, muescas);
                let despues = banco.reposo();
                let movido = banco.dialog.scroll_u() - scroll_antes;
                assert_eq!(movido, 42 * muescas as i64, "ppp {ppp} {ventana:?}");
                assert_eq!(
                    fijos(&despues, &banco.dialog),
                    fijos_antes,
                    "ppp {ppp} {ventana:?}: título y pie fijos"
                );
                let contrastados = desplazados(&antes, &despues, cuerpo, movido as f32 * u, tol);
                assert!(
                    contrastados >= 2,
                    "ppp {ppp} {ventana:?}: solo se contrastan {contrastados} textos del cuerpo"
                );
            }
        }
    }
}

// ---------------------------------------------------------------------------------------------
// Fila 11: foco y teclado
// ---------------------------------------------------------------------------------------------

#[test]
fn fila_11_el_foco_inicial_es_initial_focus() {
    for ppp in PPPS {
        for ventana in VENTANAS {
            for spec in [formulario(), huge(20)] {
                let dialog = open(spec);
                let esperado = initial_focus(&dialog);
                let mut banco = Banco::nuevo(ppp, ventana, dialog);
                assert_eq!(banco.focus, None);
                banco.reposo();
                assert_eq!(
                    banco.focus,
                    Some(esperado),
                    "ppp {ppp} {ventana:?}: el foco inicial"
                );
                // El foco se pide una sola vez: otro fotograma no lo cambia.
                banco.reposo();
                assert_eq!(banco.focus, Some(esperado), "ppp {ppp} {ventana:?}");
            }
        }
    }
}

/// Un ciclo entero de `Tab` (o `Mayús`+`Tab`) más una vuelta extra: cada paso es el siguiente (o el
/// anterior) de `focus_order`.
fn recorre(banco: &mut Banco, atras: bool, con_anillo: bool) {
    let ppp = banco.ppp;
    let ventana = banco.ventana;
    let orden = focus_order(&banco.dialog);
    assert!(orden.len() >= 4, "{orden:?}");
    let inicio = orden
        .iter()
        .position(|t| Some(*t) == banco.focus)
        .expect("el foco inicial está en el orden");
    let u = banco.u();
    let longitud = orden.len();
    for paso in 1..=longitud + 2 {
        let p = banco.tab(atras);
        let esperado = if atras {
            orden[(inicio + longitud * 3 - paso) % longitud]
        } else {
            orden[(inicio + paso) % longitud]
        };
        assert_eq!(
            banco.focus,
            Some(esperado),
            "ppp {ppp} {ventana:?} atras={atras}: paso {paso}"
        );
        if con_anillo {
            assert!(
                !anillos(&p, u).is_empty(),
                "ppp {ppp} {ventana:?}: el anillo de {esperado:?} no está a la vista"
            );
        }
        if let Some(d) = banco.detras.as_ref() {
            let foco = banco.foco_de_egui();
            assert!(
                foco != Some(d.boton_id) && foco != Some(d.texto_id),
                "ppp {ppp} {ventana:?}: el Tab llegó a lo de detrás en el paso {paso}"
            );
        }
        let rects = banco.rects();
        assert!(
            (0..=rects.max_scroll_u).contains(&banco.dialog.scroll_u()),
            "ppp {ppp} {ventana:?}: scroll {} fuera de 0..={}",
            banco.dialog.scroll_u(),
            rects.max_scroll_u
        );
    }
}

#[test]
fn fila_11_tab_recorre_focus_order_en_orden_da_la_vuelta_y_el_anillo_se_ve() {
    for ppp in PPPS {
        for ventana in VENTANAS {
            let mut banco = Banco::listo(ppp, ventana, open(formulario()));
            recorre(&mut banco, false, true);
        }
    }
}

#[test]
fn fila_11_mayus_tab_recorre_el_orden_al_reves_y_da_la_vuelta() {
    for ppp in PPPS {
        for ventana in VENTANAS {
            let mut banco = Banco::listo(ppp, ventana, open(formulario()));
            recorre(&mut banco, true, true);
        }
    }
}

#[test]
fn fila_11_tab_con_el_hexadecimal_visible_y_con_errores_sigue_el_orden_sin_aceptar() {
    for ppp in PPPS {
        let mut banco = Banco::nuevo(ppp, Ventana::Minima, open(formulario()));
        banco.reposo();
        prepara(&mut banco, "hex_invalido");
        banco.reposo();
        let orden = focus_order(&banco.dialog);
        assert!(orden.contains(&FocusTarget::Field("color")), "ppp {ppp}");
        assert!(
            !orden.contains(&FocusTarget::Accept),
            "ppp {ppp}: con errores, Aceptar no se enfoca"
        );
        recorre(&mut banco, false, true);
        recorre(&mut banco, true, true);
    }
}

#[test]
fn fila_11_tab_con_contenido_enorme_mantiene_el_foco_a_la_vista_y_el_scroll_en_rango() {
    for ppp in PPPS {
        for ventana in VENTANAS {
            let mut banco = Banco::listo(ppp, ventana, open(huge(60)));
            let inicio = banco.focus;
            let mut maximo_visto = 0;
            let orden = focus_order(&banco.dialog);
            for _ in 0..orden.len() {
                banco.tab(false);
                maximo_visto = maximo_visto.max(banco.dialog.scroll_u());
            }
            assert_eq!(
                banco.focus, inicio,
                "ppp {ppp} {ventana:?}: un ciclo entero vuelve al inicio"
            );
            assert!(
                maximo_visto > 0 || banco.rects().max_scroll_u == 0,
                "ppp {ppp} {ventana:?}: con 60 casillas el Tab tiene que desplazar el cuerpo"
            );
            recorre(&mut banco, false, true);
        }
    }
}

#[test]
fn fila_11_tab_y_mayus_tab_nunca_llegan_a_lo_de_detras_y_sin_dialogo_si() {
    for ppp in PPPS {
        let mut banco = Banco::con_detras(ppp, Ventana::Minima, open(formulario()));
        recorre(&mut banco, false, false);
        recorre(&mut banco, true, false);

        // Control: sin el diálogo, la tabulación sí llega al botón de detrás.
        let mut banco = Banco::sin_dialogo(ppp, Ventana::Minima, open(formulario()));
        let boton = banco.detras.as_ref().expect("detrás").boton_id;
        let mut llego = false;
        for _ in 0..6 {
            banco.tab(false);
            llego |= banco.foco_de_egui() == Some(boton);
        }
        assert!(llego, "ppp {ppp}: el control no llega al botón con Tab");
    }
}

#[test]
fn fila_11_intro_y_espacio_en_cancelar_cancelan_y_en_aceptar_aceptan() {
    for ppp in PPPS {
        for tecla_boton in [Key::Enter, Key::Space] {
            let mut banco = Banco::listo(ppp, Ventana::Minima, open(formulario()));
            banco.foco_hasta(FocusTarget::Cancel);
            banco.reacciones.clear();
            banco.pulsa(tecla_boton, Modifiers::NONE);
            assert_eq!(
                canceladas(&banco.reacciones),
                1,
                "ppp {ppp} {tecla_boton:?}: con el foco en Cancelar cancela una vez: {:?}",
                banco.reacciones
            );
            assert!(
                aceptadas(&banco.reacciones).is_empty(),
                "ppp {ppp} {tecla_boton:?}"
            );

            let mut banco = Banco::listo(ppp, Ventana::Minima, open(formulario()));
            banco.foco_hasta(FocusTarget::Accept);
            banco.reacciones.clear();
            banco.pulsa(tecla_boton, Modifiers::NONE);
            let aceptadas = aceptadas(&banco.reacciones);
            assert_eq!(
                aceptadas.len(),
                1,
                "ppp {ppp} {tecla_boton:?}: con el foco en Aceptar acepta una vez: {:?}",
                banco.reacciones
            );
            assert_eq!(aceptadas[0].number("ancho"), Some(64), "ppp {ppp}");
            assert_eq!(
                canceladas(&banco.reacciones),
                0,
                "ppp {ppp} {tecla_boton:?}"
            );
        }
    }
}

#[test]
fn fila_11_escape_cancela_con_el_foco_en_cualquier_elemento() {
    for ppp in PPPS {
        let orden = focus_order(&open(formulario()));
        for objetivo in orden {
            let mut banco = Banco::listo(ppp, Ventana::Minima, open(formulario()));
            banco.foco_hasta(objetivo);
            banco.reacciones.clear();
            banco.pulsa(Key::Escape, Modifiers::NONE);
            assert_eq!(
                canceladas(&banco.reacciones),
                1,
                "ppp {ppp}: Escape con el foco en {objetivo:?}"
            );
            assert!(
                aceptadas(&banco.reacciones).is_empty(),
                "ppp {ppp} {objetivo:?}"
            );
        }
    }
}

#[test]
fn fila_11_escape_con_un_hexadecimal_pendiente_lo_revierte_y_el_siguiente_cancela() {
    for ppp in PPPS {
        let mut banco = Banco::nuevo(ppp, Ventana::Minima, open(formulario()));
        banco.reposo();
        prepara(&mut banco, "hex");
        banco.reposo();
        banco.foco_hasta(FocusTarget::Field("color"));
        let confirmado = banco.dialog.text("color").expect("el campo se muestra");
        banco.escribe("#12");
        assert!(banco.dialog.hex_pending("color"), "ppp {ppp}");
        assert_eq!(banco.dialog.text("color"), Some("#12".into()), "ppp {ppp}");

        banco.reacciones.clear();
        banco.pulsa(Key::Escape, Modifiers::NONE);
        assert!(
            !cierra(&banco.reacciones),
            "ppp {ppp}: el primer Escape solo revierte"
        );
        assert!(
            !banco.dialog.hex_pending("color"),
            "ppp {ppp}: ya no hay pendiente"
        );
        assert_eq!(
            banco.dialog.text("color"),
            Some(confirmado),
            "ppp {ppp}: revertido"
        );

        banco.pulsa(Key::Escape, Modifiers::NONE);
        assert_eq!(
            canceladas(&banco.reacciones),
            1,
            "ppp {ppp}: el segundo cancela"
        );
    }
}

// ---------------------------------------------------------------------------------------------
// Fila 11: cambio de ppp y de tamaño con el diálogo abierto
// ---------------------------------------------------------------------------------------------

/// Valores escritos, un hexadecimal pendiente y el foco puesto.
fn con_estado(ppp: f32, ventana: Ventana) -> Banco {
    let mut banco = Banco::nuevo(ppp, ventana, open(formulario()));
    banco.reposo();
    numero(&mut banco, "ancho", "300");
    banco.evento(Evento::Choice {
        field: "modo",
        index: 2,
    });
    banco.evento(Evento::Hex {
        field: "color",
        text: "#12".into(),
    });
    banco.evento(Evento::Check {
        field: "opaco",
        on: true,
    });
    banco.reposo();
    banco
}

#[test]
fn fila_11_un_cambio_de_ppp_y_de_ventana_recoloca_sin_perder_valores_ni_foco() {
    let cambios = [
        (1.0, 2.0),
        (2.0, 1.25),
        (1.5, 3.0),
        (3.0, 1.0),
        (1.25, 1.5),
        (2.0, 3.0),
    ];
    let ventanas = [
        (Ventana::Minima, Ventana::Minima),
        (Ventana::Minima, Ventana::Grande),
        (Ventana::Grande, Ventana::Minima),
    ];
    for (antes, despues) in cambios {
        for (va, vd) in ventanas {
            let mut banco = con_estado(antes, va);
            let guardado = estado(&banco.dialog);
            let foco = banco.focus;
            assert!(foco.is_some());
            banco.reacciones.clear();

            banco.cambia(despues, vd);
            banco.reposo();
            let donde = format!("{antes} → {despues}, {va:?} → {vd:?}");
            assert_eq!(
                estado(&banco.dialog),
                guardado,
                "{donde}: los valores no cambian"
            );
            assert_eq!(banco.focus, foco, "{donde}: el foco no cambia");
            assert!(
                banco.dialog.hex_pending("color"),
                "{donde}: el pendiente sigue"
            );
            assert!(
                banco.reacciones.is_empty(),
                "{donde}: {:?}",
                banco.reacciones
            );
            // Sigue centrado y con la geometría de la escala nueva.
            comprueba_geometria(&mut banco, &donde);
        }
    }
}

#[test]
fn fila_11_el_desplazamiento_se_conserva_en_u_al_cambiar_de_escala_o_de_tamano() {
    let recorridos = [
        (1.0, Ventana::Minima),
        (2.0, Ventana::Minima),
        (3.0, Ventana::Minima),
        (1.0, Ventana::Grande),
        (1.5, Ventana::Minima),
        (1.25, Ventana::Minima),
    ];
    let mut banco = Banco::nuevo(recorridos[0].0, recorridos[0].1, open(huge(60)));
    banco.reposo();
    banco.dialog.set_scroll_u(100);
    banco.reposo();
    assert_eq!(banco.dialog.scroll_u(), 100);
    for (ppp, ventana) in &recorridos[1..] {
        banco.cambia(*ppp, *ventana);
        banco.reposo();
        let maximo = banco.rects().max_scroll_u;
        assert_eq!(
            banco.dialog.scroll_u(),
            100.min(maximo),
            "ppp {ppp} {ventana:?}: el desplazamiento se guarda en u (máximo {maximo})"
        );
    }
}

#[test]
fn fila_11_un_cambio_de_ppp_no_mueve_el_foco_ni_el_texto_de_un_campo_con_foco() {
    // Un campo con el foco y texto escrito: el cambio de escala no lo suelta ni lo toca.
    let mut banco = Banco::listo(1.0, Ventana::Minima, open(formulario()));
    banco.escribe("512");
    assert_eq!(banco.focus, Some(FocusTarget::Field("ancho")));
    for ppp in [2.0, 1.25, 3.0, 1.5, 1.0] {
        banco.cambia(ppp, Ventana::Minima);
        banco.reposo();
        assert_eq!(banco.focus, Some(FocusTarget::Field("ancho")), "ppp {ppp}");
        assert_eq!(banco.dialog.text("ancho"), Some("512".into()), "ppp {ppp}");
        // Y se puede seguir tecleando.
        banco.escribe("77");
        assert_eq!(banco.dialog.text("ancho"), Some("77".into()), "ppp {ppp}");
        banco.escribe("512");
    }
}
