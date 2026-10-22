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

use super::model::{Cel4, Order, Stack};
use crate::session::Session;

struct Snap {
    id: u64,
    stack: Stack,
    /// La capa activa justo antes de aplicar el paso (a lo que vuelve el modelo al deshacerlo).
    active_before: usize,
    structural: bool,
    /// La orden que dio el paso (para los mensajes).
    order: String,
    /// Fue un combinar o aplanar: la sesión recuerda la capa activa de antes y de después.
    fusion: bool,
    /// Fue `New` o `Duplicate`: al deshacerlo la activa es la que ocupa su posición (3.4).
    creates: bool,
}

pub struct World3 {
    pub s: Session,
    pub tools: ToolState,
    pub m: Stack,
    /// `snaps[n]`: el estado tras `n` pasos de la rama actual.
    snaps: Vec<Snap>,
    pub pos: usize,
    used: HashSet<u64>,
    /// `state_id` del último estado guardado (el de apertura, al principio).
    pub saved: u64,
    pub label_2: String,
}

fn position_active(s: &Session) -> usize {
    s.document
        .layers()
        .iter()
        .position(|c| c.id() == s.active_layer())
        .expect("la capa activa existe")
}

impl World3 {
    pub fn of_session(s: Session, label_2: &str) -> World3 {
        let active_2 = position_active(&s);
        let m = Stack::of_document(&s.document, active_2);
        let id = s.document.state_id();
        World3 {
            s,
            tools: ToolState::new(),
            snaps: vec![Snap {
                id,
                stack: m.clone(),
                active_before: active_2,
                structural: false,
                order: "estado inicial".to_string(),
                fusion: false,
                creates: false,
            }],
            pos: 0,
            used: HashSet::from([id]),
            saved: id,
            m,
            label_2: label_2.to_string(),
        }
    }

    /// La pila que hay ahora en la sesión.
    pub fn read_2_2(&self) -> Stack {
        Stack::of_document(&self.s.document, position_active(&self.s))
    }

    pub fn steps_2(&self) -> usize {
        self.snaps.len() - 1
    }

    /// Pila y composición iguales al modelo y «modificado» coherente con el estado guardado.
    pub fn check(&self) {
        let ctx = &self.label_2;
        if let Err(e) = self.m.equal(&self.read_2_2()) {
            panic!("{ctx}: la sesión no es el modelo: {e}");
        }
        let of_the_session = compose(&self.s.document).map_err(|e| format!("{e:?}"));
        match (of_the_session, self.m.compose_2()) {
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
            self.s.document.state_id() != self.saved,
            "{ctx}: modified no corresponde al state_id guardado"
        );
        if !self.s.modified() {
            let (id, stack) = (&self.snaps[self.pos].id, &self.snaps[self.pos].stack);
            assert_eq!(*id, self.saved, "{ctx}: sin modificar y otro estado");
            assert!(
                stack.equal_content(&self.m).is_ok(),
                "{ctx}: modified es falso con otro contenido"
            );
        }
    }

    /// Anota el estado actual como el guardado (tras un guardado que salió bien).
    pub fn mark_saved(&mut self) {
        self.saved = self.s.document.state_id();
        assert!(
            !self.s.modified(),
            "{}: modified tras guardar",
            self.label_2
        );
        self.check();
    }

    /// Lanza los eventos de un gesto y devuelve si el documento cambió y si la herramienta
    /// rechazó el gesto (el borrador sin valor de borrado).
    fn gesture(&mut self, tool_2: Tool, color: Cel4, events: Vec<CanvasEvent>) -> (bool, bool) {
        self.tools.apply_action(ToolAction::Select(tool_2));
        let pixel = self.m.pixel(color);
        let id = self.s.active_cel();
        let mut change = false;
        for event_2 in events {
            change |= self
                .tools
                .canvas_event_with(&mut self.s.document, id, Some(pixel), event_2)
                .unwrap_or_else(|e| panic!("{}: el gesto dio error: {e}", self.label_2));
        }
        let changes_2 = self.tools.take_changes();
        self.s.absorb(changes_2);
        let rejected = self
            .tools
            .take_events()
            .iter()
            .any(|e| matches!(e, ToolEvent::Refused(_)));
        (change, rejected)
    }

    fn paint(&mut self, tool_2: Tool, color: Cel4, path_2: &[(i32, i32)]) -> bool {
        // Como la aplicación: con la activa oculta o bloqueada no se empieza ningún gesto.
        let rejection = self.s.paint_refusal(tool_2);
        assert_eq!(
            rejection.is_some(),
            !self.m.can_paint(),
            "{}: la sesión y el modelo no coinciden en si se puede pintar",
            self.label_2
        );
        if rejection.is_some() {
            return false;
        }
        let mut events = vec![CanvasEvent::StrokeBegin(path_2[0])];
        events.extend(path_2[1..].iter().map(|&p| CanvasEvent::StrokeMove(p)));
        events.push(CanvasEvent::StrokeEnd);
        let (change, rejected) = self.gesture(tool_2, color, events);
        let eraser_without_value = tool_2 == Tool::Eraser && !self.m.can_delete();
        assert_eq!(
            rejected, eraser_without_value,
            "{}: ¿rechazó la herramienta el gesto?",
            self.label_2
        );
        change
    }

    fn in_session(&mut self, order: &Order) -> bool {
        let s = &mut self.s;
        match order {
            Order::New => s.new_layer().changed,
            Order::Duplicate => s.duplicate_layer().changed,
            Order::Remove => s.remove_layer().changed,
            Order::Move(delta) => s.move_layer(*delta).changed,
            Order::Activate(i) => {
                let id = s.document.layers()[*i].id();
                s.set_active_layer(id).expect("activar");
                false
            }
            Order::Visible(i, v) => {
                let id = s.document.layers()[*i].id();
                s.set_layer_visible(id, *v).changed
            }
            Order::Lock(i, v) => {
                let id = s.document.layers()[*i].id();
                s.set_layer_locked(id, *v).changed
            }
            Order::Rename(i, name_2) => {
                let id = s.document.layers()[*i].id();
                s.rename_layer(id, name_2.clone()).changed
            }
            Order::Opacity(i, o) => {
                let id = s.document.layers()[*i].id();
                if s.document.mode() == Mode::Indexed && *o != 255 {
                    let r = s.apply_layer(Command::SetLayerOpacity {
                        layer: id,
                        opacity: *o,
                    });
                    assert_eq!(r, Err(DocumentError::IndexedOpacity), "{}", self.label_2);
                    return false;
                }
                s.set_layer_opacity(id, *o).changed
            }
            Order::Blend(i, m) => {
                let id = s.document.layers()[*i].id();
                if s.document.mode() == Mode::Indexed && *m != px_core::BlendMode::Normal {
                    let r = s.apply_layer(Command::SetLayerBlend {
                        layer: id,
                        blend: *m,
                    });
                    assert_eq!(r, Err(DocumentError::IndexedBlend), "{}", self.label_2);
                    return false;
                }
                s.set_layer_blend(id, *m).changed
            }
            Order::Pencil(path_2, color) => self.paint(Tool::Pencil, *color, path_2),
            Order::Line(a, b, color) => self.paint(Tool::Line, *color, &[*a, *b]),
            Order::Fill(seed, color) => self.paint(Tool::Fill, *color, &[*seed]),
            Order::Eraser(path_2) => self.paint(Tool::Eraser, [0, 0, 0, 0], path_2),
            Order::Merge(kind) => s.merge(*kind).changed,
            Order::Convert(target_2) => s
                .convert_all(*target_2, Allowed::NOTHING)
                .unwrap_or_else(|e| panic!("{}: convertir a {target_2:?} dio {e:?}", self.label_2)),
            Order::Vector((x, y), expected_2) => {
                let composite = compose(&s.document).expect("compose");
                let c = match composite.get_pixel(*x, *y).expect("píxel") {
                    Pixel::Rgba(c) => [c.r, c.g, c.b, c.a],
                    other_2 => panic!("{}: se esperaba RGBA, hay {other_2:?}", self.label_2),
                };
                assert_eq!(
                    c, *expected_2,
                    "{}: vector a mano en ({x}, {y}) de la composición",
                    self.label_2
                );
                false
            }
        }
    }

    /// Aplica la orden a la sesión y al modelo y comprueba que coinciden. Devuelve si hubo cambio.
    pub fn apply(&mut self, order: &Order) -> bool {
        let ctx = format!("{}: {order:?}", self.label_2);
        let before_id = self.s.document.state_id();
        let active_before = self.m.active_2;
        let mut new_2 = self.m.clone();
        if let Order::Convert(Mode::Indexed) = order {
            return self.convert_to_indexed(order, &ctx, before_id, active_before);
        }
        let effect = new_2.apply(order);
        let change = self.in_session(order);
        assert_eq!(change, effect.changes, "{ctx}: ¿hubo paso de historia?");
        let id = self.s.document.state_id();
        if change {
            assert_ne!(id, before_id, "{ctx}: state_id sin cambiar");
            assert!(self.used.insert(id), "{ctx}: state_id repetido");
            self.snaps.truncate(self.pos + 1);
            self.pos += 1;
            self.snaps.push(Snap {
                id,
                stack: new_2.clone(),
                active_before,
                structural: effect.structural,
                order: format!("{order:?}"),
                fusion: matches!(order, Order::Merge(_)),
                creates: matches!(order, Order::New | Order::Duplicate),
            });
        } else {
            assert_eq!(id, before_id, "{ctx}: state_id cambió sin paso");
        }
        self.m = new_2;
        self.check();
        change
    }

    /// RGBA → indexado: la paleta nueva la decide la sesión; el modelo la adopta **comprobando que
    /// la conversión es exacta** (cada píxel conserva su color resuelto). Si la sesión la rechaza
    /// (`Err`) no cambia nada. Devuelve si hubo paso.
    fn convert_to_indexed(
        &mut self,
        order: &Order,
        ctx: &str,
        before_id: u64,
        active_before: usize,
    ) -> bool {
        match self.s.convert_all(Mode::Indexed, Allowed::NOTHING) {
            Ok(true) => {
                let mut new_2 = self.m.clone();
                let palette = self.s.document.palette().cloned().expect("paleta nueva");
                new_2
                    .adopts_palette(palette)
                    .unwrap_or_else(|e| panic!("{ctx}: la conversión no es exacta: {e}"));
                let id = self.s.document.state_id();
                assert!(self.used.insert(id), "{ctx}: state_id repetido");
                self.snaps.truncate(self.pos + 1);
                self.pos += 1;
                self.snaps.push(Snap {
                    id,
                    stack: new_2.clone(),
                    active_before,
                    structural: false,
                    order: format!("{order:?}"),
                    fusion: false,
                    creates: false,
                });
                self.m = new_2;
                self.check();
                true
            }
            other_2 => {
                assert!(
                    !matches!(other_2, Ok(true)) && self.s.document.state_id() == before_id,
                    "{ctx}: convertir {other_2:?} cambió el estado"
                );
                self.check();
                false
            }
        }
    }

    /// La capa activa que la sesión recuerda si el estado `pos` es el de antes o el de después de
    /// un combinar (3.5, decisión D2).
    fn active_in_boundary(&self, pos: usize) -> Option<usize> {
        let id = self.snaps[pos].id;
        (1..self.snaps.len()).rev().find_map(|j| {
            let step = &self.snaps[j];
            if !step.fusion {
                None
            } else if self.snaps[j - 1].id == id {
                Some(step.active_before)
            } else if step.id == id {
                Some(step.stack.active_2)
            } else {
                None
            }
        })
    }

    /// Tras `undo` o `redo`: contenido igual al de la instantánea, `state_id` y la capa activa
    /// (la de antes del paso al deshacer una orden de estructura, la de después al rehacerla; en
    /// las demás no cambia).
    fn after_history(&mut self, pos: usize, active_expected: Option<usize>, before: LayerId) {
        let ctx = format!(
            "{} (instantánea {pos}, paso «{}»)",
            self.label_2, self.snaps[pos].order
        );
        let snap = &self.snaps[pos];
        assert_eq!(self.s.document.state_id(), snap.id, "{ctx}: state_id");
        let read = self.read_2_2();
        if let Err(e) = snap.stack.equal_content(&read) {
            panic!("{ctx}: la sesión no es la instantánea: {e}");
        }
        // Llegar al estado de antes o de después de un combinar devuelve la capa activa de entonces
        // (las notas de 3.5), también si el paso que se cruza no era el combinar, y **gana** a la
        // regla de la capa que entra (las notas se aplican después).
        let active_expected = self.active_in_boundary(pos).or(active_expected);
        match active_expected {
            Some(expected) => assert_eq!(read.active_2, expected, "{ctx}: capa activa"),
            None => assert_eq!(self.s.active_layer(), before, "{ctx}: la activa cambió"),
        }
        self.m = read;
        self.check();
    }

    pub fn undo(&mut self) {
        let before = self.s.active_layer();
        // Lo que depende del estado de antes de deshacer: la capa que creó el paso.
        let created = (self.pos > 0 && self.snaps[self.pos].creates)
            .then(|| self.s.document.layers()[self.snaps[self.pos].stack.active_2].id());
        let result = self.s.undo();
        if self.pos == 0 {
            assert_eq!(
                result,
                Err(DocumentError::NothingToUndo),
                "{}",
                self.label_2
            );
            self.check();
            return;
        }
        result.unwrap_or_else(|e| panic!("{}: undo dio {e:?}", self.label_2));
        let step = &self.snaps[self.pos];
        // Combinar y Quitar: vuelve la capa de antes (notas de 3.5; la capa que entra, 3.4).
        // Nueva y Duplicar: si la activa era la creada deja de existir y pasa a la que ocupa su
        // posición, o a la última (`keep_active_layer`); si era otra, no cambia.
        let expected = if step.creates {
            let stay = self.snaps[self.pos - 1].stack.layers_2.len();
            (Some(before) == created).then(|| step.stack.active_2.min(stay - 1))
        } else {
            step.structural.then_some(step.active_before)
        };
        self.pos -= 1;
        self.after_history(self.pos, expected, before);
    }

    pub fn redo(&mut self) {
        let before = self.s.active_layer();
        // Lo que depende del estado de antes de rehacer: la capa que quitará el paso.
        let removed = self.snaps.get(self.pos + 1).and_then(|step| {
            (step.structural && !step.creates && !step.fusion)
                .then(|| self.s.document.layers()[step.active_before].id())
        });
        let result = self.s.redo();
        if self.pos + 1 == self.snaps.len() {
            assert_eq!(
                result,
                Err(DocumentError::NothingToRedo),
                "{}",
                self.label_2
            );
            self.check();
            return;
        }
        result.unwrap_or_else(|e| panic!("{}: redo dio {e:?}", self.label_2));
        self.pos += 1;
        let step = &self.snaps[self.pos];
        // Nueva, Duplicar y combinar: la capa que entra o la de las notas. Quitar: si la activa
        // era la quitada pasa a la que ocupa su posición; si era otra, no cambia.
        let expected = if step.creates || step.fusion {
            Some(step.stack.active_2)
        } else if step.structural {
            (Some(before) == removed).then_some(step.stack.active_2)
        } else {
            None
        };
        self.after_history(self.pos, expected, before);
    }

    /// `undo` hasta el principio (y uno más, que debe fallar); devuelve cuántos hizo.
    pub fn undo_all(&mut self) -> usize {
        let mut facts = 0;
        while self.pos > 0 {
            self.undo();
            facts += 1;
        }
        self.undo();
        facts
    }

    /// `redo` hasta el final de la rama (y uno más, que debe fallar); devuelve cuántos hizo.
    pub fn redo_all(&mut self) -> usize {
        let mut facts = 0;
        while self.pos + 1 < self.snaps.len() {
            self.redo();
            facts += 1;
        }
        self.redo();
        facts
    }

    /// Con varias capas, `convert_mode` da error y una conversión que pierde datos sin permiso
    /// también: ni el documento ni la historia cambian.
    pub fn rejects_convert(&mut self, target_2: Mode) {
        assert!(self.s.document.layers().len() > 1);
        let before = self.s.document.state_id();
        assert!(
            self.s.convert_mode(target_2, Allowed::NOTHING).is_err(),
            "{}: convert_mode con varias capas debía dar error",
            self.label_2
        );
        assert!(
            self.s
                .convert_all(Mode::GrayAlpha, Allowed::NOTHING)
                .is_err(),
            "{}: convertir a gris con alfa sin permiso debía dar error",
            self.label_2
        );
        assert_eq!(self.s.document.state_id(), before, "{}", self.label_2);
        self.check();
    }
}
