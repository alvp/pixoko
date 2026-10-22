//! El banco de la puerta del hito 3: una `Session` real, sus herramientas y el modelo de la pila
//! (specs/hito-3-puerta.md). Cada orden se aplica a la sesión por el camino de la aplicación
//! (`new_layer`, `set_layer_*`, `merge`, `convert_all`, `ToolState::canvas_event_with`) y al
//! modelo, y se comprueba que coinciden: un paso de historia por orden con cambio y ninguno si se
//! rechaza, `state_id` nuevo, pila, composición, capa activa y «modificado».

use std::collections::HashSet;

use px_core::{Allowed, Command, DocumentError, LayerId, Mode, Pixel};
use px_raster::compose;
use px_ui::canvas::CanvasEvent;
use px_ui::tool::{Tool, ToolAction, ToolEvent, ToolState};

use super::modelo::{Celda4, Orden, Pila};
use crate::session::Session;

struct Snap {
    id: u64,
    pila: Pila,
    /// La capa activa justo antes de aplicar el paso (a lo que vuelve el modelo al deshacerlo).
    activa_antes: usize,
    estructural: bool,
    /// La orden que dio el paso (para los mensajes).
    orden: String,
    /// Fue un combinar o aplanar: la sesión recuerda la capa activa de antes y de después.
    fusion: bool,
    /// Fue `Nueva` o `Duplicar`: al deshacerlo la activa es la que ocupa su posición (3.4).
    crea: bool,
}

pub struct Mundo3 {
    pub s: Session,
    pub tools: ToolState,
    pub m: Pila,
    /// `snaps[n]`: el estado tras `n` pasos de la rama actual.
    snaps: Vec<Snap>,
    pub pos: usize,
    usados: HashSet<u64>,
    /// `state_id` del último estado guardado (el de apertura, al principio).
    pub guardado: u64,
    pub etiqueta: String,
}

fn posicion_activa(s: &Session) -> usize {
    s.document
        .layers()
        .iter()
        .position(|c| c.id() == s.active_layer())
        .expect("la capa activa existe")
}

impl Mundo3 {
    pub fn de_sesion(s: Session, etiqueta: &str) -> Mundo3 {
        let activa = posicion_activa(&s);
        let m = Pila::de_documento(&s.document, activa);
        let id = s.document.state_id();
        Mundo3 {
            s,
            tools: ToolState::new(),
            snaps: vec![Snap {
                id,
                pila: m.clone(),
                activa_antes: activa,
                estructural: false,
                orden: "estado inicial".to_string(),
                fusion: false,
                crea: false,
            }],
            pos: 0,
            usados: HashSet::from([id]),
            guardado: id,
            m,
            etiqueta: etiqueta.to_string(),
        }
    }

    /// La pila que hay ahora en la sesión.
    pub fn leer(&self) -> Pila {
        Pila::de_documento(&self.s.document, posicion_activa(&self.s))
    }

    pub fn pasos(&self) -> usize {
        self.snaps.len() - 1
    }

    /// Pila y composición iguales al modelo y «modificado» coherente con el estado guardado.
    pub fn comprobar(&self) {
        let ctx = &self.etiqueta;
        if let Err(e) = self.m.igual(&self.leer()) {
            panic!("{ctx}: la sesión no es el modelo: {e}");
        }
        let de_la_sesion = compose(&self.s.document).map_err(|e| format!("{e:?}"));
        match (de_la_sesion, self.m.componer()) {
            (Ok(a), Ok(b)) => assert!(
                a == b,
                "{ctx}: la composición de la sesión no es la del modelo"
            ),
            (Err(a), Err(b)) => assert_eq!(a, b, "{ctx}: error de composición distinto"),
            (a, b) => panic!(
                "{ctx}: la sesión compone {:?} y el modelo {:?}",
                a.map(|_| ()),
                b.map(|_| ())
            ),
        }
        assert_eq!(
            self.s.modified(),
            self.s.document.state_id() != self.guardado,
            "{ctx}: modified no corresponde al state_id guardado"
        );
        if !self.s.modified() {
            let (id, pila) = (&self.snaps[self.pos].id, &self.snaps[self.pos].pila);
            assert_eq!(*id, self.guardado, "{ctx}: sin modificar y otro estado");
            assert!(
                pila.igual_contenido(&self.m).is_ok(),
                "{ctx}: modified es falso con otro contenido"
            );
        }
    }

    /// Anota el estado actual como el guardado (tras un guardado que salió bien).
    pub fn marcar_guardado(&mut self) {
        self.guardado = self.s.document.state_id();
        assert!(
            !self.s.modified(),
            "{}: modified tras guardar",
            self.etiqueta
        );
        self.comprobar();
    }

    /// Lanza los eventos de un gesto y devuelve si el documento cambió y si la herramienta
    /// rechazó el gesto (el borrador sin valor de borrado).
    fn gesto(
        &mut self,
        herramienta: Tool,
        color: Celda4,
        eventos: Vec<CanvasEvent>,
    ) -> (bool, bool) {
        self.tools.apply_action(ToolAction::Select(herramienta));
        let pixel = self.m.pixel(color);
        let id = self.s.active_cel();
        let mut cambio = false;
        for evento in eventos {
            cambio |= self
                .tools
                .canvas_event_with(&mut self.s.document, id, Some(pixel), evento)
                .unwrap_or_else(|e| panic!("{}: el gesto dio error: {e}", self.etiqueta));
        }
        let cambios = self.tools.take_changes();
        self.s.absorb(cambios);
        let rechazado = self
            .tools
            .take_events()
            .iter()
            .any(|e| matches!(e, ToolEvent::Refused(_)));
        (cambio, rechazado)
    }

    fn pintar(&mut self, herramienta: Tool, color: Celda4, ruta: &[(i32, i32)]) -> bool {
        // Como la aplicación: con la activa oculta o bloqueada no se empieza ningún gesto.
        let rechazo = self.s.paint_refusal(herramienta);
        assert_eq!(
            rechazo.is_some(),
            !self.m.puede_pintar(),
            "{}: la sesión y el modelo no coinciden en si se puede pintar",
            self.etiqueta
        );
        if rechazo.is_some() {
            return false;
        }
        let mut eventos = vec![CanvasEvent::StrokeBegin(ruta[0])];
        eventos.extend(ruta[1..].iter().map(|&p| CanvasEvent::StrokeMove(p)));
        eventos.push(CanvasEvent::StrokeEnd);
        let (cambio, rechazado) = self.gesto(herramienta, color, eventos);
        let borrador_sin_valor = herramienta == Tool::Eraser && !self.m.puede_borrar();
        assert_eq!(
            rechazado, borrador_sin_valor,
            "{}: ¿rechazó la herramienta el gesto?",
            self.etiqueta
        );
        cambio
    }

    fn en_sesion(&mut self, orden: &Orden) -> bool {
        let s = &mut self.s;
        match orden {
            Orden::Nueva => s.new_layer().changed,
            Orden::Duplicar => s.duplicate_layer().changed,
            Orden::Quitar => s.remove_layer().changed,
            Orden::Mover(delta) => s.move_layer(*delta).changed,
            Orden::Activar(i) => {
                let id = s.document.layers()[*i].id();
                s.set_active_layer(id).expect("activar");
                false
            }
            Orden::Visible(i, v) => {
                let id = s.document.layers()[*i].id();
                s.set_layer_visible(id, *v).changed
            }
            Orden::Bloqueo(i, v) => {
                let id = s.document.layers()[*i].id();
                s.set_layer_locked(id, *v).changed
            }
            Orden::Renombrar(i, nombre) => {
                let id = s.document.layers()[*i].id();
                s.rename_layer(id, nombre.clone()).changed
            }
            Orden::Opacidad(i, o) => {
                let id = s.document.layers()[*i].id();
                if s.document.mode() == Mode::Indexed && *o != 255 {
                    let r = s.apply_layer(Command::SetLayerOpacity {
                        layer: id,
                        opacity: *o,
                    });
                    assert_eq!(r, Err(DocumentError::IndexedOpacity), "{}", self.etiqueta);
                    return false;
                }
                s.set_layer_opacity(id, *o).changed
            }
            Orden::Mezcla(i, m) => {
                let id = s.document.layers()[*i].id();
                if s.document.mode() == Mode::Indexed && *m != px_core::BlendMode::Normal {
                    let r = s.apply_layer(Command::SetLayerBlend {
                        layer: id,
                        blend: *m,
                    });
                    assert_eq!(r, Err(DocumentError::IndexedBlend), "{}", self.etiqueta);
                    return false;
                }
                s.set_layer_blend(id, *m).changed
            }
            Orden::Lapiz(ruta, color) => self.pintar(Tool::Pencil, *color, ruta),
            Orden::Linea(a, b, color) => self.pintar(Tool::Line, *color, &[*a, *b]),
            Orden::Relleno(semilla, color) => self.pintar(Tool::Fill, *color, &[*semilla]),
            Orden::Borrador(ruta) => self.pintar(Tool::Eraser, [0, 0, 0, 0], ruta),
            Orden::Combinar(tipo) => s.merge(*tipo).changed,
            Orden::Convertir(destino) => s
                .convert_all(*destino, Allowed::NOTHING)
                .unwrap_or_else(|e| panic!("{}: convertir a {destino:?} dio {e:?}", self.etiqueta)),
            Orden::Vector((x, y), esperado) => {
                let compuesta = compose(&s.document).expect("compose");
                let c = match compuesta.get_pixel(*x, *y).expect("píxel") {
                    Pixel::Rgba(c) => [c.r, c.g, c.b, c.a],
                    otro => panic!("{}: se esperaba RGBA, hay {otro:?}", self.etiqueta),
                };
                assert_eq!(
                    c, *esperado,
                    "{}: vector a mano en ({x}, {y}) de la composición",
                    self.etiqueta
                );
                false
            }
        }
    }

    /// Aplica la orden a la sesión y al modelo y comprueba que coinciden. Devuelve si hubo cambio.
    pub fn aplicar(&mut self, orden: &Orden) -> bool {
        let ctx = format!("{}: {orden:?}", self.etiqueta);
        let antes_id = self.s.document.state_id();
        let activa_antes = self.m.activa;
        let mut nuevo = self.m.clone();
        if let Orden::Convertir(Mode::Indexed) = orden {
            return self.convertir_a_indexado(orden, &ctx, antes_id, activa_antes);
        }
        let efecto = nuevo.aplicar(orden);
        let cambio = self.en_sesion(orden);
        assert_eq!(cambio, efecto.cambia, "{ctx}: ¿hubo paso de historia?");
        let id = self.s.document.state_id();
        if cambio {
            assert_ne!(id, antes_id, "{ctx}: state_id sin cambiar");
            assert!(self.usados.insert(id), "{ctx}: state_id repetido");
            self.snaps.truncate(self.pos + 1);
            self.pos += 1;
            self.snaps.push(Snap {
                id,
                pila: nuevo.clone(),
                activa_antes,
                estructural: efecto.estructural,
                orden: format!("{orden:?}"),
                fusion: matches!(orden, Orden::Combinar(_)),
                crea: matches!(orden, Orden::Nueva | Orden::Duplicar),
            });
        } else {
            assert_eq!(id, antes_id, "{ctx}: state_id cambió sin paso");
        }
        self.m = nuevo;
        self.comprobar();
        cambio
    }

    /// RGBA → indexado: la paleta nueva la decide la sesión; el modelo la adopta **comprobando que
    /// la conversión es exacta** (cada píxel conserva su color resuelto). Si la sesión la rechaza
    /// (`Err`) no cambia nada. Devuelve si hubo paso.
    fn convertir_a_indexado(
        &mut self,
        orden: &Orden,
        ctx: &str,
        antes_id: u64,
        activa_antes: usize,
    ) -> bool {
        match self.s.convert_all(Mode::Indexed, Allowed::NOTHING) {
            Ok(true) => {
                let mut nuevo = self.m.clone();
                let paleta = self.s.document.palette().cloned().expect("paleta nueva");
                nuevo
                    .adopta_paleta(paleta)
                    .unwrap_or_else(|e| panic!("{ctx}: la conversión no es exacta: {e}"));
                let id = self.s.document.state_id();
                assert!(self.usados.insert(id), "{ctx}: state_id repetido");
                self.snaps.truncate(self.pos + 1);
                self.pos += 1;
                self.snaps.push(Snap {
                    id,
                    pila: nuevo.clone(),
                    activa_antes,
                    estructural: false,
                    orden: format!("{orden:?}"),
                    fusion: false,
                    crea: false,
                });
                self.m = nuevo;
                self.comprobar();
                true
            }
            otro => {
                assert!(
                    !matches!(otro, Ok(true)) && self.s.document.state_id() == antes_id,
                    "{ctx}: convertir {otro:?} cambió el estado"
                );
                self.comprobar();
                false
            }
        }
    }

    /// La capa activa que la sesión recuerda si el estado `pos` es el de antes o el de después de
    /// un combinar (3.5, decisión D2).
    fn activa_en_frontera(&self, pos: usize) -> Option<usize> {
        let id = self.snaps[pos].id;
        (1..self.snaps.len()).rev().find_map(|j| {
            let paso = &self.snaps[j];
            if !paso.fusion {
                None
            } else if self.snaps[j - 1].id == id {
                Some(paso.activa_antes)
            } else if paso.id == id {
                Some(paso.pila.activa)
            } else {
                None
            }
        })
    }

    /// Tras `undo` o `redo`: contenido igual al de la instantánea, `state_id` y la capa activa
    /// (la de antes del paso al deshacer una orden de estructura, la de después al rehacerla; en
    /// las demás no cambia).
    fn tras_historia(&mut self, pos: usize, activa_esperada: Option<usize>, antes: LayerId) {
        let ctx = format!(
            "{} (instantánea {pos}, paso «{}»)",
            self.etiqueta, self.snaps[pos].orden
        );
        let snap = &self.snaps[pos];
        assert_eq!(self.s.document.state_id(), snap.id, "{ctx}: state_id");
        let leida = self.leer();
        if let Err(e) = snap.pila.igual_contenido(&leida) {
            panic!("{ctx}: la sesión no es la instantánea: {e}");
        }
        // Llegar al estado de antes o de después de un combinar devuelve la capa activa de entonces
        // (las notas de 3.5), también si el paso que se cruza no era el combinar, y **gana** a la
        // regla de la capa que entra (las notas se aplican después).
        let activa_esperada = self.activa_en_frontera(pos).or(activa_esperada);
        match activa_esperada {
            Some(esperada) => assert_eq!(leida.activa, esperada, "{ctx}: capa activa"),
            None => assert_eq!(self.s.active_layer(), antes, "{ctx}: la activa cambió"),
        }
        self.m = leida;
        self.comprobar();
    }

    pub fn deshacer(&mut self) {
        let antes = self.s.active_layer();
        // Lo que depende del estado de antes de deshacer: la capa que creó el paso.
        let creada = (self.pos > 0 && self.snaps[self.pos].crea)
            .then(|| self.s.document.layers()[self.snaps[self.pos].pila.activa].id());
        let resultado = self.s.undo();
        if self.pos == 0 {
            assert_eq!(
                resultado,
                Err(DocumentError::NothingToUndo),
                "{}",
                self.etiqueta
            );
            self.comprobar();
            return;
        }
        resultado.unwrap_or_else(|e| panic!("{}: undo dio {e:?}", self.etiqueta));
        let paso = &self.snaps[self.pos];
        // Combinar y Quitar: vuelve la capa de antes (notas de 3.5; la capa que entra, 3.4).
        // Nueva y Duplicar: si la activa era la creada deja de existir y pasa a la que ocupa su
        // posición, o a la última (`keep_active_layer`); si era otra, no cambia.
        let esperada = if paso.crea {
            let quedan = self.snaps[self.pos - 1].pila.capas.len();
            (Some(antes) == creada).then(|| paso.pila.activa.min(quedan - 1))
        } else {
            paso.estructural.then_some(paso.activa_antes)
        };
        self.pos -= 1;
        self.tras_historia(self.pos, esperada, antes);
    }

    pub fn rehacer(&mut self) {
        let antes = self.s.active_layer();
        // Lo que depende del estado de antes de rehacer: la capa que quitará el paso.
        let quitada = self.snaps.get(self.pos + 1).and_then(|paso| {
            (paso.estructural && !paso.crea && !paso.fusion)
                .then(|| self.s.document.layers()[paso.activa_antes].id())
        });
        let resultado = self.s.redo();
        if self.pos + 1 == self.snaps.len() {
            assert_eq!(
                resultado,
                Err(DocumentError::NothingToRedo),
                "{}",
                self.etiqueta
            );
            self.comprobar();
            return;
        }
        resultado.unwrap_or_else(|e| panic!("{}: redo dio {e:?}", self.etiqueta));
        self.pos += 1;
        let paso = &self.snaps[self.pos];
        // Nueva, Duplicar y combinar: la capa que entra o la de las notas. Quitar: si la activa
        // era la quitada pasa a la que ocupa su posición; si era otra, no cambia.
        let esperada = if paso.crea || paso.fusion {
            Some(paso.pila.activa)
        } else if paso.estructural {
            (Some(antes) == quitada).then_some(paso.pila.activa)
        } else {
            None
        };
        self.tras_historia(self.pos, esperada, antes);
    }

    /// `undo` hasta el principio (y uno más, que debe fallar); devuelve cuántos hizo.
    pub fn deshacer_todo(&mut self) -> usize {
        let mut hechos = 0;
        while self.pos > 0 {
            self.deshacer();
            hechos += 1;
        }
        self.deshacer();
        hechos
    }

    /// `redo` hasta el final de la rama (y uno más, que debe fallar); devuelve cuántos hizo.
    pub fn rehacer_todo(&mut self) -> usize {
        let mut hechos = 0;
        while self.pos + 1 < self.snaps.len() {
            self.rehacer();
            hechos += 1;
        }
        self.rehacer();
        hechos
    }

    /// Con varias capas, `convert_mode` da error y una conversión que pierde datos sin permiso
    /// también: ni el documento ni la historia cambian.
    pub fn rechaza_convertir(&mut self, destino: Mode) {
        assert!(self.s.document.layers().len() > 1);
        let antes = self.s.document.state_id();
        assert!(
            self.s.convert_mode(destino, Allowed::NOTHING).is_err(),
            "{}: convert_mode con varias capas debía dar error",
            self.etiqueta
        );
        assert!(
            self.s
                .convert_all(Mode::GrayAlpha, Allowed::NOTHING)
                .is_err(),
            "{}: convertir a gris con alfa sin permiso debía dar error",
            self.etiqueta
        );
        assert_eq!(self.s.document.state_id(), antes, "{}", self.etiqueta);
        self.comprobar();
    }
}
