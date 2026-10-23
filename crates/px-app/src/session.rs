//! Sesión de edición: el documento, su fichero y si hay cambios sin guardar
//! (specs/hito-2-abrir-guardar.md). Sin egui: se prueba sin ventana.

use std::borrow::Cow;
use std::path::{Path, PathBuf};

use std::fmt;

use px_core::{
    Allowed, Command, Document, DocumentError, Image, ImageId, LayerId, Mode, Pixel, Span,
};
use px_formats::{write_aseprite, write_png};
use px_raster::ComposeError;
use px_ui::color::{
    ColorChoice, Role, SlotBank, SlotColors, cell_count, cell_of_choice, choice_for_cell,
    clamp_choice, default_background, initial_choice, pixel_for_choice_with,
};
use px_ui::color_edit::{ColorEditor, Commit, EditTarget, StaleReason, stale_notice};
use px_ui::dirty::{BlockSet, Change, ChangeLog, dirty_of};
use px_ui::edit_layout::PaletteButton;
use px_ui::palette_grid::{PanelView, Scroll};
use px_ui::palette_ops::{
    Fate, NOTHING_TO_ROTATE_NOTICE, PaletteOp, PaletteOps, blocked_reason, choice_after,
    default_destination, entry_after, entry_fate, summary, transparent_summary, uses_entry,
};
use px_ui::palette_resize::{PendingResize, reassigned_pixels};
use px_ui::pick::{Picked, pick_notice};
use px_ui::source::flatten;
use px_ui::tool::Tool;
use px_ui::view::Size;

use self::merge_ops::MergeNote;
use crate::dialogs::{SaveFormat, SaveRoute, is_aseprite_path, with_png_extension};
use crate::files::{
    FileError, MAX_FILE_BYTES, Opened, aseprite_copy_reason, aux_cause, copy_path,
    describe_open_aseprite, describe_open_aux, describe_report, file_label, is_aux_cause,
    load_image, untitled_path, write_atomic,
};
use crate::new_image::BLACK_INDEX;

/// Resultado de guardar: dónde se escribió y, si no se sobrescribió el original porque Pixoko
/// lo convierte al abrir, qué conversiones eran.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Saved {
    pub path: PathBuf,
    pub copy_reason: Option<String>,
}

/// Resultado de una edición de la paleta: si el documento cambió y el aviso de 5 s (resumen de lo
/// ocurrido, o por qué no pasó nada; con texto pendiente descartado, lo lleva antepuesto).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PaletteOutcome {
    pub changed: bool,
    pub notice: Option<String>,
}

/// Lo que pasa al soltar el asa o aceptar el tamaño de la paleta (specs/ui-palette-handle.md).
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ResizeOutcome {
    /// La misma longitud: no hay nada que hacer.
    Unchanged,
    /// Se aplicó ya, como un paso.
    Applied(PaletteOutcome),
    /// Queda pendiente de destino y de `Confirmar` (el panel lo pinta).
    Pending,
    /// Se pierde la transparente con varias capas: `App` abre el diálogo y, si se acepta, llama a
    /// [`Session::apply_resize`] con estos valores. Nada cambió.
    ConfirmLoss { len: u16, into: Option<u8> },
}

/// Resultado de aplicar lo que eligió el cuentagotas: el aviso de 5 s, si lo hay (un color con
/// alfa 0 se copia tal cual).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PickOutcome {
    pub notice: Option<String>,
}

/// Por qué no se pudo borrar la página.
#[derive(Debug, PartialEq, Eq)]
pub enum ClearPageError {
    /// No hay memoria para el paso de deshacer: la imagen queda intacta.
    OutOfMemory,
    /// La capa activa está oculta: no se pinta.
    HiddenLayer,
    /// La capa activa está bloqueada: no se pinta.
    LockedLayer,
    /// La capa activa está oculta y bloqueada.
    HiddenAndLockedLayer,
    Document(DocumentError),
}

impl fmt::Display for ClearPageError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ClearPageError::OutOfMemory => f.write_str("no hay memoria para borrar la página"),
            ClearPageError::HiddenLayer => f.write_str(HIDDEN_LAYER_NOTICE),
            ClearPageError::LockedLayer => f.write_str(LOCKED_LAYER_NOTICE),
            ClearPageError::HiddenAndLockedLayer => f.write_str(HIDDEN_AND_LOCKED_NOTICE),
            ClearPageError::Document(error) => write!(f, "{error}"),
        }
    }
}

/// Un fichero leído y listo para instalar como documento (`Session::load` + `Session::install`).
pub struct Loaded {
    pub document: Document,
    /// El aviso de apertura.
    pub text: String,
    choice: ColorChoice,
    path: PathBuf,
    conversion: Option<String>,
}

pub struct Session {
    pub document: Document,
    /// Color activo del panel de paletas. Abrir un fichero lo reinicia (specs/hito-2-panel-paletas.md).
    pub choice: ColorChoice,
    /// Color de fondo (botón derecho): por defecto el transparente del modo (specs/interfaz-dos-
    /// colores.md). Abrir o crear un documento lo reinicia.
    pub background: ColorChoice,
    /// Qué color edita el editor de color y sobre cuál actúan los botones de la paleta.
    pub editing: Role,
    /// The color editor is open (specs/ui-composition.md). It starts closed; `p`, `Esc`, a second
    /// right click on the edited rectangle of the indicator and changing document toggle it.
    pub editor_open: bool,
    /// La capa activa: donde actúan las herramientas. Es estado de la sesión, no del documento
    /// (specs/hito-3-capas-vista.md, decisión 1): cambiarla no es un paso de la historia.
    active: LayerId,
    /// Posición de la capa activa en la última comprobación: si la capa desaparece, la activa
    /// pasa a la que ocupa esa posición (o a la última).
    active_pos: usize,
    /// Bytes de celdas con píxeles en la última comprobación del aviso de presupuesto.
    mem_seen: usize,
    /// Desplazamiento del panel de paletas; abrir un fichero lo reinicia.
    pub scroll: Scroll,
    /// Estado del panel entre fotogramas (desplazamiento de todo el panel y «mostrar la elegida»);
    /// abrir un fichero lo reinicia (specs/hito-2-panel-ventana-baja.md).
    pub view: PanelView,
    /// Colores de las ranuras por modo; duran toda la sesión y no son del documento
    /// (specs/hito-2-editar-color.md).
    pub slots: SlotBank,
    /// Estado del editor de color; abrir un fichero lo reinicia.
    pub editor: ColorEditor,
    /// Arrastre de muestras, quita pendiente y rango de rotación (specs/hito-2-paleta-operaciones.md);
    /// abrir un fichero los reinicia.
    pub ops: PaletteOps,
    /// Una quita pendiente se canceló porque el documento cambió y aún no se avisó.
    removal_cancelled: bool,
    path: Option<PathBuf>,
    /// Un `open` descartó el texto pendiente del campo hexadecimal y aún no se avisó.
    discarded_text: bool,
    /// `state_id` del documento al abrir o guardar por última vez.
    saved_id: u64,
    /// Conversiones del fichero abierto, si las hubo: el primer `save` hace una copia.
    conversion: Option<String>,
    /// Qué bloques tocó cada paso de la historia, por `state_id` (specs/hito-2-zoom-baldosas.md,
    /// decisión 6): `undo` y `redo` invalidan lo que anotó el paso.
    log: ChangeLog,
    /// Bloques tocados desde la última vez que el lienzo los tomó; `None`: nada anotado.
    dirty: Option<BlockSet>,
    /// Qué capa estaba activa antes y después de cada combinación reciente: deshacer y rehacer
    /// la devuelven (specs/hito-3-combinar.md, D2).
    merge_notes: Vec<MergeNote>,
}

impl Session {
    pub fn sample(document: Document) -> Session {
        let choice = initial_choice(document.canvas_image());
        let background = default_background(document.canvas_image());
        let active = document.layers()[0].id();
        let mem_seen = document.materialized_bytes();
        Session {
            active,
            active_pos: 0,
            mem_seen,
            saved_id: document.state_id(),
            document,
            choice,
            background,
            editing: Role::Foreground,
            editor_open: false,
            scroll: Scroll::default(),
            view: PanelView::default(),
            slots: SlotBank::new(),
            editor: ColorEditor::default(),
            ops: PaletteOps::default(),
            removal_cancelled: false,
            path: None,
            discarded_text: false,
            conversion: None,
            log: ChangeLog::new(),
            dirty: None,
            merge_notes: Vec::new(),
        }
    }

    fn image_size(&self) -> Size {
        Size::new(self.document.width(), self.document.height())
    }

    /// La capa activa (la app usa la celda: [`Session::active_cel`]).
    #[cfg_attr(not(test), allow(dead_code))]
    pub fn active_layer(&self) -> LayerId {
        self.active
    }

    /// La celda de la capa activa: donde escriben las herramientas.
    pub fn active_cel(&self) -> ImageId {
        self.document
            .layer(self.active)
            .map_or(self.document.layers()[0].cel(), |layer| layer.cel())
    }

    /// Elige la capa activa. No toca el documento, la historia ni lo modificado.
    pub fn set_active_layer(&mut self, id: LayerId) -> Result<(), DocumentError> {
        let position = self
            .document
            .layers()
            .iter()
            .position(|layer| layer.id() == id)
            .ok_or(DocumentError::UnknownLayer(id))?;
        self.active = id;
        self.active_pos = position;
        Ok(())
    }

    /// Posición (desde 1) de la capa activa y número de capas, solo con más de una.
    pub fn layers_info(&self) -> Option<(usize, usize)> {
        let total = self.document.layers().len();
        let position = self
            .document
            .layers()
            .iter()
            .position(|layer| layer.id() == self.active)?;
        (total > 1).then_some((position + 1, total))
    }

    /// Aplica un comando de capa (o cualquier otro) anotando su región sucia, y deja válida la
    /// capa activa. `Ok(false)`: sin cambios.
    pub fn apply_layer(&mut self, command: Command) -> Result<bool, DocumentError> {
        let blocks = dirty_of(&command, self.image_size());
        let changed = self.document.apply(command)?;
        if changed {
            self.note_applied(blocks);
            self.normalize();
        }
        Ok(changed)
    }

    /// Si la capa activa ya no existe, pasa a la que ocupa su posición anterior (o a la última).
    fn keep_active_layer(&mut self) {
        let layers = self.document.layers();
        match layers.iter().position(|layer| layer.id() == self.active) {
            Some(position) => self.active_pos = position,
            None => {
                let position = self.active_pos.min(layers.len() - 1);
                self.active = layers[position].id();
                self.active_pos = position;
            }
        }
    }

    /// Por qué la herramienta no puede pintar en la capa activa (oculta, bloqueada o ambas); el
    /// cuentagotas lee igualmente.
    pub fn paint_refusal(&self, tool: Tool) -> Option<&'static str> {
        let paints = matches!(tool, Tool::Pencil | Tool::Line | Tool::Fill | Tool::Eraser);
        let layer = self.document.layer(self.active);
        let hidden = layer.is_some_and(|layer| !layer.visible());
        let locked = layer.is_some_and(|layer| layer.locked());
        if !paints {
            return None;
        }
        match (hidden, locked) {
            (true, true) => Some(HIDDEN_AND_LOCKED_NOTICE),
            (true, false) => Some(HIDDEN_LAYER_NOTICE),
            (false, true) => Some(LOCKED_LAYER_NOTICE),
            (false, false) => None,
        }
    }

    /// Alguna celda (no solo la activa) usa la entrada `index`: quitarla pide el destino.
    pub fn removal_needs_destination(&self, index: u8) -> bool {
        self.document.layers().iter().any(|layer| {
            self.document
                .image(layer.cel())
                .is_some_and(|image| uses_entry(image, index))
        })
    }

    /// Empieza la quita pendiente de `index` (la pregunta del destino). `false` si la imagen no
    /// tiene paleta.
    pub fn begin_removal_for(&mut self, index: u8) -> bool {
        let Some(palette) = self.document.palette() else {
            return false;
        };
        let default = default_destination(palette, index);
        self.ops.begin_removal(index, default);
        true
    }

    /// Da el estado actual por guardado (atajo de las pruebas).
    #[cfg_attr(not(test), allow(dead_code))]
    pub fn mark_unmodified(&mut self) {
        self.saved_id = self.document.state_id();
    }

    /// Toda la imagen.
    fn full(&self) -> BlockSet {
        let size = self.image_size();
        BlockSet::full(size.width, size.height)
    }

    fn note_dirty(&mut self, blocks: &BlockSet) {
        match &mut self.dirty {
            Some(dirty) => dirty.union(blocks),
            None => self.dirty = Some(blocks.clone()),
        }
    }

    /// Anota un paso que acaba de aplicarse: su región, por el `state_id` que dejó.
    fn note_applied(&mut self, blocks: BlockSet) {
        self.log.record(self.document.state_id(), blocks.clone());
        self.note_dirty(&blocks);
    }

    /// Anota los cambios que aplicaron las herramientas (`ToolState::take_changes`).
    pub fn absorb(&mut self, changes: Vec<Change>) {
        for change in changes {
            self.log.record(change.state, change.blocks.clone());
            self.note_dirty(&change.blocks);
        }
    }

    /// La unión de los bloques anotados desde la última vez, que el lienzo debe regenerar. Si no
    /// se anotó nada (un cambio que no se conoce), toda la imagen.
    pub fn take_dirty(&mut self) -> BlockSet {
        self.dirty.take().unwrap_or_else(|| self.full())
    }

    #[cfg(test)]
    pub fn change_log(&self) -> &ChangeLog {
        &self.log
    }

    /// Cambia la elección por teclado; si cambió, el panel muestra su fila en el fotograma
    /// siguiente.
    pub fn set_choice_revealing(&mut self, choice: ColorChoice) {
        if choice != self.choice {
            self.choice = choice;
            self.view.reveal = true;
            self.ops.clear_range();
        }
    }

    /// Recorre los colores `delta` pasos (`+1` el siguiente, `-1` el anterior): la entrada en
    /// indexado y la ranura en los demás modos, dando la vuelta en los extremos. Muestra la fila
    /// elegida. Con una elección que ya no vale para la imagen, vuelve a la inicial.
    pub fn cycle_choice(&mut self, delta: i8) {
        let next = Some(self.document.canvas_image()).and_then(|image| {
            let count = cell_count(image) as i64;
            if count == 0 {
                return None;
            }
            match cell_of_choice(image, self.choice) {
                Some(cell) => {
                    let cell = (cell as i64 + i64::from(delta)).rem_euclid(count);
                    choice_for_cell(image, cell as usize)
                }
                None => Some(initial_choice(image)),
            }
        });
        if let Some(choice) = next {
            self.set_choice_revealing(choice);
        }
    }

    /// Un deslizador del editor o un arrastre de muestra en curso.
    pub fn busy(&self) -> bool {
        self.editor.dragging() || self.ops.busy()
    }

    /// La elección de color del rol.
    pub fn choice_of(&self, role: Role) -> ColorChoice {
        match role {
            Role::Foreground => self.choice,
            Role::Background => self.background,
        }
    }

    /// `Pixel` del color del rol con los colores de ranura de la sesión.
    pub fn pixel_of(&self, role: Role) -> Option<Pixel> {
        let image = self.document.canvas_image();
        pixel_for_choice_with(image, self.choice_of(role), self.slots.get(image.mode()))
    }

    /// Recorre el color de fondo `delta` pasos (`+1` el siguiente, `-1` el anterior), dando la
    /// vuelta en los extremos, como [`Session::cycle_choice`] con el primer plano. No toca el
    /// rango de rotación.
    pub fn cycle_background(&mut self, delta: i8) {
        let next = Some(self.document.canvas_image()).and_then(|image| {
            let count = cell_count(image) as i64;
            if count == 0 {
                return None;
            }
            match cell_of_choice(image, self.background) {
                Some(cell) => {
                    let cell = (cell as i64 + i64::from(delta)).rem_euclid(count);
                    choice_for_cell(image, cell as usize)
                }
                None => Some(default_background(image)),
            }
        });
        if let Some(choice) = next {
            self.background = choice;
        }
    }

    /// Aplica lo que eligió el cuentagotas con el botón izquierdo: el primer plano. La app usa
    /// [`Session::apply_pick_for`]; esta es la vía de las pruebas anteriores a I.2a.
    #[cfg_attr(not(test), allow(dead_code))]
    pub fn apply_pick(&mut self, picked: Picked) -> PickOutcome {
        self.apply_pick_for(picked, Role::Foreground)
    }

    /// Aplica lo que eligió el cuentagotas (specs/hito-2-cuentagotas-borrador.md, decisión 4) al
    /// color de `role`. Indexada: elige la entrada (el primer plano además la muestra en el
    /// panel); con una quita pendiente se ignora. RGBA, Gray y GrayAlpha: escribe el color en la
    /// ranura del rol en ese modo. Nunca modifica el documento: ni paso de deshacer ni marca de
    /// modificado.
    pub fn apply_pick_for(&mut self, picked: Picked, role: Role) -> PickOutcome {
        let image = self.document.canvas_image();
        let mode = image.mode();
        let palette_len = image.palette().map_or(0, |palette| palette.len());
        let applied = match (picked, mode, self.choice_of(role)) {
            (Picked::Entry(index), Mode::Indexed, _)
                if usize::from(index) < palette_len && self.ops.removal().is_none() =>
            {
                match role {
                    Role::Foreground => self.set_choice_revealing(ColorChoice::Entry(index)),
                    Role::Background => self.background = ColorChoice::Entry(index),
                }
                true
            }
            (
                Picked::Color(color),
                Mode::Rgba | Mode::Gray | Mode::GrayAlpha,
                ColorChoice::Slot(slot),
            ) => self.slots.get_mut(mode).set(slot, color),
            _ => false,
        };
        PickOutcome {
            notice: applied.then(|| pick_notice(&picked)).flatten(),
        }
    }

    /// Borra la página: la rellena con el color de fondo como **un** paso (`FillSpans` con un
    /// tramo por fila). `Ok(false)` si ya era de ese color. Reserva antes de modificar; si no hay
    /// memoria, la imagen y la historia quedan intactas.
    pub fn clear_page(&mut self) -> Result<bool, ClearPageError> {
        self.clear_page_with(&mut |bytes| Vec::<u8>::new().try_reserve_exact(bytes).is_ok())
    }

    /// Como [`Session::clear_page`] con la reserva dada: `reserve(bytes)` dice si caben los bytes
    /// de la imagen entera (lo máximo que guardará el paso de deshacer).
    pub fn clear_page_with(
        &mut self,
        reserve: &mut dyn FnMut(usize) -> bool,
    ) -> Result<bool, ClearPageError> {
        if let Some(reason) = self.paint_refusal(Tool::Fill) {
            return Err(match reason {
                LOCKED_LAYER_NOTICE => ClearPageError::LockedLayer,
                HIDDEN_AND_LOCKED_NOTICE => ClearPageError::HiddenAndLockedLayer,
                _ => ClearPageError::HiddenLayer,
            });
        }
        let id = self.active_cel();
        let Some(pixel) = self.pixel_of(Role::Background) else {
            return Ok(false);
        };
        let image = self.document.canvas_image();
        let (width, height) = (self.document.width(), self.document.height());
        let bytes = (width as usize)
            .checked_mul(height as usize)
            .and_then(|pixels| pixels.checked_mul(image.mode().bytes_per_pixel()))
            .ok_or(ClearPageError::OutOfMemory)?;
        if !reserve(bytes) {
            return Err(ClearPageError::OutOfMemory);
        }
        let mut spans = Vec::new();
        spans
            .try_reserve_exact(height as usize)
            .map_err(|_| ClearPageError::OutOfMemory)?;
        spans.extend((0..height).map(|y| Span {
            y,
            x_start: 0,
            x_end: width,
        }));
        let changed = self
            .document
            .apply(Command::FillSpans {
                image: id,
                spans,
                pixel,
            })
            .map_err(|error| match error {
                DocumentError::OutOfMemory => ClearPageError::OutOfMemory,
                other => ClearPageError::Document(other),
            })?;
        if changed {
            self.note_applied(self.full());
        }
        Ok(changed)
    }

    /// Convierte la imagen principal a `target` como **un** paso de la historia (specs/hito-2-convertir-modo.md,
    /// D5). `Ok(true)` si cambió algo; con un error el documento queda intacto. Tras convertir,
    /// la elección de color se ajusta al modo nuevo como tras deshacer o rehacer.
    pub fn convert_mode(&mut self, target: Mode, allowed: Allowed) -> Result<bool, DocumentError> {
        let image = self.active_cel();
        let before = self.choice;
        let changed = self.document.apply(Command::ConvertMode {
            image,
            target,
            allowed,
        })?;
        if changed {
            self.note_applied(self.full());
            let layers = self.layer_ids();
            self.after_history_move(before, &layers);
            // El fondo por defecto es del modo: tras convertir se vuelve a elegir.
            self.background = default_background(self.document.canvas_image());
        }
        Ok(changed)
    }

    /// Deshace un paso y deja la selección válida para la paleta resultante.
    pub fn undo(&mut self) -> Result<(), DocumentError> {
        let before = self.choice;
        let layers_before = self.layer_ids();
        let from = self.document.state_id();
        let result = self.document.undo();
        if result.is_ok() {
            // Deshacer desde `from` toca lo que tocó el paso que dejó ese estado.
            let blocks = self.log.get(from).cloned().unwrap_or_else(|| self.full());
            self.note_dirty(&blocks);
        }
        self.after_history_move(before, &layers_before);
        result
    }

    /// Rehace un paso y deja la selección válida para la paleta resultante.
    pub fn redo(&mut self) -> Result<(), DocumentError> {
        let before = self.choice;
        let layers_before = self.layer_ids();
        let result = self.document.redo();
        if result.is_ok() {
            // Rehacer hacia `to` toca lo mismo que tocó el paso al aplicarse.
            let to = self.document.state_id();
            let blocks = self.log.get(to).cloned().unwrap_or_else(|| self.full());
            self.note_dirty(&blocks);
        }
        self.after_history_move(before, &layers_before);
        result
    }

    fn after_history_move(&mut self, before: ColorChoice, layers_before: &[LayerId]) {
        // El rango no sobrevive a deshacer ni a rehacer.
        self.ops.clear_range();
        self.follow_entering_layer(layers_before);
        self.follow_merge_notes();
        self.normalize();
        self.reveal_if_choice_changed(before);
    }

    /// Deja la selección válida para la imagen y pone al día el estado de las operaciones de
    /// paleta (rango, quita pendiente). Cada fotograma, antes del panel, y tras deshacer o
    /// rehacer.
    pub fn normalize(&mut self) {
        self.keep_active_layer();
        let image = self.document.canvas_image();
        self.choice = clamp_choice(image, self.choice);
        self.background = clamp_choice(image, self.background);
        let len = image.palette().map_or(0, |palette| palette.len());
        if self.ops.sync(self.document.state_id(), len) {
            self.removal_cancelled = true;
        }
    }

    /// Una quita pendiente se canceló porque el documento cambió: se dice una sola vez.
    pub fn take_removal_cancelled(&mut self) -> bool {
        std::mem::take(&mut self.removal_cancelled)
    }

    /// Un recorte pendiente se canceló porque el documento cambió: se dice una sola vez.
    pub fn take_resize_cancelled(&mut self) -> bool {
        self.ops.take_resize_cancelled()
    }

    /// Soltar el asa (o aceptar el tamaño de la paleta) en `len` entradas: estira ya, recorta ya si
    /// ningún píxel de ninguna capa usa lo que sobra, deja el recorte pendiente de destino y de
    /// `Confirmar` si alguno lo usa (o si cae la transparente con una sola capa), o pide confirmar
    /// la pérdida de la transparente con varias capas.
    pub fn request_resize(&mut self, len: u16) -> Result<ResizeOutcome, DocumentError> {
        let Some(palette) = self.document.palette() else {
            return Ok(ResizeOutcome::Unchanged);
        };
        let (old, new) = (palette.len(), usize::from(len));
        let transparent = palette.transparent_index();
        if new == 0 || new == old {
            return Ok(ResizeOutcome::Unchanged);
        }
        if new > old {
            return self.apply_resize(len, None).map(ResizeOutcome::Applied);
        }
        let pixels = self.pixels_cut_by(new);
        let loses = transparent.is_some_and(|t| usize::from(t) >= new);
        let several = self.document.layers().len() > 1;
        match (pixels, loses) {
            (0, false) => self.apply_resize(len, None).map(ResizeOutcome::Applied),
            (0, true) if several => Ok(ResizeOutcome::ConfirmLoss { len, into: None }),
            _ => {
                self.ops.begin_resize(PendingResize {
                    len,
                    into: None,
                    pixels,
                    transparent,
                });
                Ok(ResizeOutcome::Pending)
            }
        }
    }

    /// `Confirmar` del recorte pendiente. Sin destino, sigue pendiente; con varias capas y la
    /// transparente fuera, pide confirmar la pérdida (y el recorte pendiente se cancela: si se
    /// rechaza, el documento queda como estaba).
    pub fn confirm_resize(&mut self) -> Result<ResizeOutcome, DocumentError> {
        let Some(pending) = self.ops.resize_pending() else {
            return Ok(ResizeOutcome::Unchanged);
        };
        if pending.needs_destination() {
            return Ok(ResizeOutcome::Pending);
        }
        if pending.loses_transparent() && self.document.layers().len() > 1 {
            self.ops.cancel_resize();
            return Ok(ResizeOutcome::ConfirmLoss {
                len: pending.len,
                into: pending.into,
            });
        }
        self.apply_resize(pending.len, pending.into)
            .map(ResizeOutcome::Applied)
    }

    /// Fija la longitud de la paleta ya decidida, como **un** paso.
    pub fn apply_resize(
        &mut self,
        len: u16,
        into: Option<u8>,
    ) -> Result<PaletteOutcome, DocumentError> {
        self.apply_palette_op(PaletteOp::Resize { len, into }, false)
    }

    /// Píxeles de todas las celdas de todas las capas que usan una entrada de `len` en adelante.
    fn pixels_cut_by(&self, len: usize) -> u64 {
        reassigned_pixels(
            self.document
                .layers()
                .iter()
                .filter_map(|layer| self.document.image(layer.cel())),
            len,
        )
    }

    /// `Imagen → Añadir entrada`: lo mismo que hacía el icono (copia el color elegido al final).
    /// Fuera de las indexadas no hace nada; con 256 entradas, avisa.
    pub fn menu_add_entry(&mut self) -> Result<PaletteOutcome, DocumentError> {
        let Some(len) = self.document.palette().map(|palette| palette.len()) else {
            return Ok(PaletteOutcome {
                changed: false,
                notice: None,
            });
        };
        if let Some(reason) = blocked_reason(PaletteButton::Add, len, false) {
            return Ok(PaletteOutcome {
                changed: false,
                notice: Some(reason.to_string()),
            });
        }
        self.apply_palette_op(PaletteOp::Add, false)
    }

    /// `Imagen → Quitar entrada`: lo mismo que hacía el icono. Si algún píxel de alguna capa usa la
    /// entrada elegida, la quita queda pendiente de destino.
    pub fn menu_remove_entry(&mut self) -> Result<PaletteOutcome, DocumentError> {
        let nothing = PaletteOutcome {
            changed: false,
            notice: None,
        };
        let (Some(palette), Some(active)) = (self.document.palette(), self.active_entry()) else {
            return Ok(nothing);
        };
        if let Some(reason) = blocked_reason(PaletteButton::Remove, palette.len(), false) {
            return Ok(PaletteOutcome {
                changed: false,
                notice: Some(reason.to_string()),
            });
        }
        let default = default_destination(palette, active);
        if self.removal_needs_destination(active) {
            self.ops.begin_removal(active, default);
            return Ok(nothing);
        }
        let into = default.unwrap_or(u8::from(active == 0));
        self.apply_palette_op(
            PaletteOp::Remove {
                index: active,
                into,
            },
            true,
        )
    }

    /// Aplica una edición de la paleta como **un** paso: construye el comando, fija la
    /// selección, resuelve el texto pendiente del campo hexadecimal y cierra la quita pendiente.
    /// `unused`: ningún píxel usaba la entrada quitada (solo para el resumen).
    pub fn apply_palette_op(
        &mut self,
        op: PaletteOp,
        unused: bool,
    ) -> Result<PaletteOutcome, DocumentError> {
        let id = self.active_cel();
        let active = self.active_entry().unwrap_or(0);
        let (old_len, was_transparent, previous, command) = {
            let image = self.document.canvas_image();
            let palette = image.palette();
            let old_len = palette.map_or(0, |palette| palette.len());
            let previous = palette.and_then(|palette| palette.transparent_index());
            let was_transparent = match (op, palette) {
                (PaletteOp::Remove { index, .. }, Some(palette)) => {
                    palette.transparent_index() == Some(index)
                }
                (PaletteOp::Resize { len, .. } | PaletteOp::ResizeTo { len }, Some(palette)) => {
                    palette
                        .transparent_index()
                        .is_some_and(|t| u16::from(t) >= len)
                }
                _ => false,
            };
            let command = match op {
                PaletteOp::Add => {
                    let color = palette
                        .and_then(|palette| palette.resolved(active).ok())
                        .unwrap_or(px_core::Rgba::TRANSPARENT);
                    Command::AddPaletteEntry { image: id, color }
                }
                PaletteOp::Remove { index, into } => Command::RemovePaletteEntry {
                    image: id,
                    index,
                    into,
                },
                PaletteOp::Move { from, to } => Command::MovePaletteEntry {
                    image: id,
                    from,
                    to,
                },
                PaletteOp::Rotate { start, len, steps } => Command::RotateColors {
                    image: id,
                    start,
                    len,
                    steps,
                },
                PaletteOp::SetTransparent { index } => {
                    Command::SetTransparentIndex { image: id, index }
                }
                PaletteOp::Resize { len, into } => Command::ResizePalette {
                    image: id,
                    len,
                    into,
                },
                PaletteOp::ResizeTo { len } => Command::ResizePalette {
                    image: id,
                    len,
                    into: None,
                },
            };
            (old_len, was_transparent, previous, command)
        };
        let own_removal = matches!(
            (op, self.ops.removal()),
            (PaletteOp::Remove { index, .. }, Some(removal)) if removal.index == index
        );
        // Un recorte reasigna píxeles si alguna celda usa lo que se quita (se cuenta antes).
        let moved = match op {
            PaletteOp::Resize { len, .. } | PaletteOp::ResizeTo { len } => {
                self.pixels_cut_by(usize::from(len)) > 0
            }
            _ => false,
        };
        let size = self.image_size();
        let blocks = dirty_of(&command, size);
        if !self.document.apply(command)? {
            let notice = match op {
                PaletteOp::Rotate { len, .. } if len >= 2 => {
                    Some("nada que rotar: colores iguales")
                }
                PaletteOp::Rotate { .. } => Some(NOTHING_TO_ROTATE_NOTICE),
                _ => None,
            };
            return Ok(PaletteOutcome {
                changed: false,
                notice: notice.map(str::to_string),
            });
        }

        self.note_applied(blocks);

        // El texto pendiente del campo, si la operación movió, quitó o cambió su entrada.
        let stale = match self.editor.pending_target() {
            Some(EditTarget::Entry(entry)) => match entry_fate(&op, entry) {
                Fate::Same => None,
                Fate::Moved => Some(StaleReason::Moved),
                Fate::Changed => Some(StaleReason::Changed),
                Fate::Gone => Some(StaleReason::Gone),
            },
            _ => None,
        };
        let stale = stale
            .filter(|_| self.editor.discard_pending())
            .map(stale_notice);

        // Un recorte propio ya no está pendiente (sin avisar).
        if matches!(op, PaletteOp::Resize { .. } | PaletteOp::ResizeTo { .. }) {
            self.ops.cancel_resize();
        }

        // Selección y estado de las operaciones. Rotar, fijar o quitar la transparente y estirar
        // la paleta no mueven la selección ni borran el rango (specs/hito-2-fijar-transparente.md,
        // decisión 4; specs/ui-palette-handle.md, decisión 5).
        let stretching = matches!(
            op,
            PaletteOp::Resize { len, .. } | PaletteOp::ResizeTo { len } if usize::from(len) > old_len
        );
        if !stretching
            && !matches!(
                op,
                PaletteOp::Rotate { .. } | PaletteOp::SetTransparent { .. }
            )
        {
            let after = ColorChoice::Entry(choice_after(&op, active, old_len));
            // El otro color sigue a su entrada si se mueve o se desplaza (`entry_after`).
            let other = match self.editing {
                Role::Foreground => self.background,
                Role::Background => self.choice,
            };
            let other = match other {
                ColorChoice::Entry(entry) => ColorChoice::Entry(entry_after(&op, entry, old_len)),
                slot => slot,
            };
            match self.editing {
                Role::Foreground => {
                    self.set_choice_revealing(after);
                    self.background = other;
                }
                Role::Background => {
                    self.background = after;
                    self.choice = other;
                    self.view.reveal = true;
                }
            }
            self.ops.clear_range();
        }
        if self.ops.removal().is_some() {
            self.ops.cancel_removal();
            if !own_removal {
                self.removal_cancelled = true;
            }
        }
        self.normalize();
        // Para un recorte, `unused` es «ningún píxel cambió de índice».
        let unused = match op {
            PaletteOp::Resize { .. } | PaletteOp::ResizeTo { .. } => !moved,
            _ => unused,
        };
        let text = match op {
            PaletteOp::SetTransparent { index } => transparent_summary(index, active, previous),
            _ => summary(&op, active, old_len, was_transparent, unused),
        };
        Ok(PaletteOutcome {
            changed: true,
            notice: Some(match stale {
                Some(stale) => format!("{stale} · {text}"),
                None => text,
            }),
        })
    }

    fn active_entry(&self) -> Option<u8> {
        match self.choice_of(self.editing) {
            ColorChoice::Entry(index) => Some(index),
            ColorChoice::Slot(_) => None,
        }
    }

    /// Tras deshacer o rehacer: si la elección ya no es `before`, el panel muestra su fila.
    pub fn reveal_if_choice_changed(&mut self, before: ColorChoice) {
        if self.choice != before {
            self.view.reveal = true;
        }
    }

    /// Si un `open` descartó texto pendiente del campo, lo dice una sola vez.
    pub fn take_discarded_text(&mut self) -> bool {
        std::mem::take(&mut self.discarded_text)
    }

    /// `Pixel` del color de primer plano con los colores de ranura de la sesión.
    pub fn active_pixel(&self) -> Option<Pixel> {
        self.pixel_of(Role::Foreground)
    }

    /// Pone al día el editor con el destino y el color guardado de la selección actual. Si no hay
    /// nada que editar (otro modo, otra paleta), el editor se cierra.
    pub fn sync_editor(&mut self) {
        let current = Some(self.document.canvas_image()).and_then(|image| {
            let target = EditTarget::of(image, self.choice_of(self.editing))?;
            let color = target.stored(image, self.slots.get(image.mode()))?;
            Some((target, color))
        });
        if current.is_none() && self.editor_open {
            self.close_editor();
        }
        self.editor.sync(current);
    }

    /// Opens the color editor on the color of `role`.
    pub fn open_editor(&mut self, role: Role) {
        self.editing = role;
        self.editor_open = true;
    }

    /// Closes the color editor; the palette operations act on the foreground again.
    pub fn close_editor(&mut self) {
        self.editor_open = false;
        self.editing = Role::Foreground;
    }

    /// `p`: opens the editor on the role being edited or closes it.
    pub fn toggle_editor(&mut self) {
        if self.editor_open {
            self.close_editor();
        } else {
            self.open_editor(self.editing);
        }
    }

    /// Right click on the rectangle of `role` in the indicator: opens the editor on it, switches to
    /// it if the editor is open on the other role, closes the editor if it is open on this one.
    pub fn indicator_right_click(&mut self, role: Role) {
        if self.editor_open && self.editing == role {
            self.close_editor();
        } else {
            self.open_editor(role);
        }
    }

    /// Aplica un color del editor: `SetPaletteColor` en indexadas (`Ok(true)` si el documento
    /// cambió, `Ok(false)` si el color era el mismo) o escribe la ranura de ese modo
    /// (`Ok(false)`: no es del documento, ni deshace ni marca cambios).
    pub fn commit_edit(&mut self, commit: Commit) -> Result<bool, DocumentError> {
        let image = self.active_cel();
        match commit.target {
            EditTarget::Entry(index) => {
                let changed = self.document.apply(Command::SetPaletteColor {
                    image,
                    index,
                    color: commit.color,
                })?;
                if changed {
                    self.note_applied(self.full());
                }
                Ok(changed)
            }
            EditTarget::Slot(slot) => {
                if let Some(mode) = Some(self.document.canvas_image().mode()) {
                    self.slots.get_mut(mode).set(slot, commit.color);
                }
                Ok(false)
            }
        }
    }

    pub fn path(&self) -> Option<&Path> {
        self.path.as_deref()
    }

    /// Hay cambios sin guardar: sin falsos negativos (con cambios nunca da falso), pero puede
    /// haber falsos positivos.
    pub fn modified(&self) -> bool {
        self.document.state_id() != self.saved_id
    }

    /// Lee `path` **sin cambiar nada** (ni documento, ni ruta, ni registro): el resultado se
    /// instala con [`Session::install`], después de cargar el lienzo (specs/hito-2-zoom-baldosas.md,
    /// decisión 11). Con cambios sin guardar y sin `discard`, `Err(Unsaved)` sin leer el fichero.
    pub fn load(&self, path: &Path, max_side: usize, discard: bool) -> Result<Loaded, FileError> {
        if self.modified() && !discard {
            return Err(FileError::Unsaved);
        }
        let opened = load_image(path, MAX_FILE_BYTES, max_side)?;
        let absolute = std::path::absolute(path).unwrap_or_else(|_| path.to_path_buf());
        let (document, text, conversion) = match opened {
            Opened::Png(read) => {
                let text =
                    describe_open_aux(&file_label(&absolute), &read.image, &read.report, &read.aux);
                // La razón de la copia: las conversiones y, al final, la causa de los auxiliares.
                let mut reason = describe_report(&read.report);
                if let Some(cause) = read.aux.appearance {
                    if !reason.is_empty() {
                        reason.push_str(", ");
                    }
                    reason.push_str(aux_cause(cause));
                }
                let conversion = (!reason.is_empty()).then_some(reason);
                (Document::new(read.image), text, conversion)
            }
            Opened::Aseprite(read) => {
                let text =
                    describe_open_aseprite(&file_label(&absolute), &read.document, &read.report);
                let conversion = aseprite_copy_reason(&read.report);
                (read.document, text, conversion)
            }
        };
        let choice = initial_choice(document.canvas_image());
        Ok(Loaded {
            document,
            text,
            choice,
            path: absolute,
            conversion,
        })
    }

    /// Reemplaza el documento por lo leído (historia y registro vacíos).
    pub fn install(&mut self, loaded: Loaded) {
        self.replace_document(
            loaded.document,
            loaded.choice,
            Some(loaded.path),
            loaded.conversion,
        );
    }

    /// Abre `path` y reemplaza el documento (historia vacía): [`Session::load`] más
    /// [`Session::install`]. Solo lo usan las pruebas: la app carga el lienzo entre las dos. Con cualquier error la sesión no cambia. Devuelve el aviso de
    /// apertura.
    #[cfg(test)]
    pub fn open(
        &mut self,
        path: &Path,
        max_side: usize,
        discard: bool,
    ) -> Result<String, FileError> {
        let loaded = self.load(path, max_side, discard)?;
        let text = loaded.text.clone();
        self.install(loaded);
        Ok(text)
    }

    /// Sustituye el documento por una imagen nueva ya construida (specs/hito-2-nueva-imagen.md,
    /// D3): historia vacía, sin ruta y sin cambios sin guardar. Con cambios sin guardar y sin
    /// `discard`, `Err(Unsaved)` y no cambia nada. El color activo es el negro en indexado.
    pub fn new_image(&mut self, image: Image, discard: bool) -> Result<(), FileError> {
        if self.modified() && !discard {
            return Err(FileError::Unsaved);
        }
        let choice = match image.mode() {
            Mode::Indexed => ColorChoice::Entry(BLACK_INDEX),
            _ => initial_choice(&image),
        };
        self.replace_document(Document::new(image), choice, None, None);
        Ok(())
    }

    /// Lo que se reinicia al cambiar de documento (`open` y `new_image`): color activo,
    /// desplazamiento y estado del panel, editor y operaciones de paleta. Las ranuras de color
    /// son de la sesión y no se tocan.
    fn replace_document(
        &mut self,
        document: Document,
        choice: ColorChoice,
        path: Option<PathBuf>,
        conversion: Option<String>,
    ) {
        self.conversion = conversion;
        self.choice = choice;
        self.background = default_background(document.canvas_image());
        self.editing = Role::Foreground;
        self.editor_open = false;
        // La ranura 9 es el fondo por defecto (transparente, o blanca en gris): abrir o crear la
        // restaura aunque se haya editado. Es la única excepción a «las ranuras duran la sesión».
        for mode in [Mode::Rgba, Mode::Gray, Mode::GrayAlpha] {
            if let Some(color) = SlotColors::defaults(mode).get(9) {
                self.slots.get_mut(mode).set(9, color);
            }
        }
        self.scroll = Scroll::default();
        self.view = PanelView::default();
        self.discarded_text = self.editor.has_pending();
        self.removal_cancelled |= self.ops.removal().is_some();
        let resize_lost = self.ops.resize_pending().is_some() || self.ops.take_resize_cancelled();
        self.ops.reset();
        if resize_lost {
            self.ops.flag_resize_cancelled();
        }
        self.editor.reset();
        self.document = document;
        // Un `.aseprite` con varias capas se abre con la de arriba activa; un documento de una
        // capa, con la única.
        self.active_pos = self.document.layers().len() - 1;
        self.active = self.document.layers()[self.active_pos].id();
        self.mem_seen = self.document.materialized_bytes();
        self.log.clear();
        self.dirty = None;
        self.merge_notes.clear();
        self.saved_id = self.document.state_id();
        self.path = path;
    }

    /// El fichero abierto se convierte al abrir: el primer `save` hace una copia.
    pub fn converted(&self) -> bool {
        self.conversion.is_some()
    }

    /// Guarda en la ruta elegida en un diálogo (specs/hito-2-dialogos-archivo.md, 4 y 5).
    pub fn save_to(&mut self, chosen: &Path) -> Result<Saved, FileError> {
        // El formato lo decide la extensión: `.aseprite` se respeta tal cual y todo lo demás es PNG.
        let dest = if is_aseprite_path(chosen) {
            chosen.to_path_buf()
        } else {
            with_png_extension(chosen.to_path_buf())
        };
        // El sistema no preguntó por el nombre al que se añadió `.png`: no se pisa nada.
        if dest != chosen && std::fs::symlink_metadata(&dest).is_ok() {
            return Err(FileError::Exists(dest));
        }
        // Un PNG que se convierte al abrir no se sobrescribe: se guarda la copia.
        if let (Some(path), Some(reason)) = (self.path.clone(), self.conversion.clone())
            && same_file(&path, &dest)
        {
            return self.write_to(copy_path(&path), Some(reason));
        }
        self.write_to(dest, None)
    }

    /// `Ctrl+S`: guardar ya, o abrir el diálogo de guardar. Directo solo con ruta y si el formato
    /// de la ruta conserva lo que hay: con 2 o más capas, solo `.aseprite` (un PNG con capas pide
    /// elegir antes de aplanar).
    pub fn save_route(&self) -> SaveRoute {
        match &self.path {
            Some(path) if self.document.layers().len() < 2 || is_aseprite_path(path) => {
                SaveRoute::Direct
            }
            _ => SaveRoute::Dialog,
        }
    }

    /// Guarda en la ruta del documento; sin ruta, en `untitled_path(new_dir)`. Si el fichero
    /// abierto se convierte al abrir, escribe una copia en vez de sobrescribirlo.
    pub fn save(&mut self, new_dir: &Path) -> Result<Saved, FileError> {
        let (dest, copy_reason) = match (&self.path, &self.conversion) {
            (Some(path), Some(reason)) => (copy_path(path), Some(reason.clone())),
            (Some(path), None) => (path.clone(), None),
            (None, _) => (untitled_path(new_dir), None),
        };
        self.write_to(dest, copy_reason)
    }

    /// Guarda una copia junto al original (`copy_path`), que pasa a ser el fichero del
    /// documento; sin ruta, en `untitled_path(new_dir)`. Nunca pisa un fichero existente.
    pub fn save_as(&mut self, new_dir: &Path) -> Result<Saved, FileError> {
        let dest = match &self.path {
            Some(path) => copy_path(path),
            None => untitled_path(new_dir),
        };
        self.write_to(dest, None)
    }

    fn write_to(&mut self, dest: PathBuf, copy_reason: Option<String>) -> Result<Saved, FileError> {
        let bytes = match SaveFormat::of_path(&dest) {
            SaveFormat::Aseprite => write_aseprite(&self.document).map_err(FileError::Aseprite)?,
            SaveFormat::Png => {
                let image = flatten_to_save(&self.document)?;
                write_png(&image).map_err(FileError::Png)?
            }
        };
        write_atomic(&dest, &bytes)?;
        self.path = Some(dest.clone());
        self.saved_id = self.document.state_id();
        self.conversion = None;
        Ok(Saved {
            path: dest,
            copy_reason,
        })
    }
}

/// La imagen que se guarda como PNG. Sin nada visible en un gris o un indexado sin índice
/// transparente no hay valor con el que rellenar: el error dice qué hacer, no el de la composición.
pub(crate) fn flatten_to_save(document: &Document) -> Result<Cow<'_, Image>, FileError> {
    flatten(document).map_err(|error| {
        match error {
        ComposeError::NoTransparency { mode } => FileError::Write(match mode {
            Mode::Indexed => {
                "no hay nada visible que guardar: muestra una capa o fija una entrada transparente"
            }
            _ => "no hay nada visible que guardar: muestra una capa",
        }
        .to_string()),
        error => FileError::Write(error.to_string()),
    }
    })
}

/// Por qué no se pinta: la capa activa está oculta.
pub const HIDDEN_LAYER_NOTICE: &str = "la capa activa está oculta";

/// Por qué no se pinta: la capa activa está bloqueada, u oculta y bloqueada.
pub const LOCKED_LAYER_NOTICE: &str = "la capa activa está bloqueada";
pub const HIDDEN_AND_LOCKED_NOTICE: &str = "la capa activa está oculta y bloqueada";

/// Añade al aviso de guardado que con varias capas se guarda aplanado (y cuántas ocultas no
/// entran). Con una capa, el texto tal cual.
pub fn with_flatten_notice(text: String, document: &Document) -> String {
    if document.layers().len() < 2 {
        return text;
    }
    let mut text =
        format!("{text} · se guarda aplanado; con Guardar como… .aseprite conservas las capas");
    let hidden = document
        .layers()
        .iter()
        .filter(|layer| !layer.visible())
        .count();
    match hidden {
        0 => {}
        1 => text.push_str(" · 1 capa oculta no se guarda"),
        n => text.push_str(&format!(" · {n} capas ocultas no se guardan")),
    }
    text
}

/// El aviso de un guardado ya hecho: un `.aseprite` dice cuántas capas guarda; un PNG con varias
/// capas dice que se guardó aplanado (`with_flatten_notice`).
pub fn saved_notice(text: String, saved: &Saved, document: &Document) -> String {
    if is_aseprite_path(&saved.path) {
        let layers = document.layers().len();
        format!(
            "{text} · {layers} {}",
            if layers == 1 { "capa" } else { "capas" }
        )
    } else {
        with_flatten_notice(text, document)
    }
}

/// Aviso al guardar. Con copia por conversión, explica por qué no se sobrescribió el original.
pub fn describe_saved(saved: &Saved) -> String {
    let name = file_label(&saved.path);
    match &saved.copy_reason {
        None => format!("guardado {name}"),
        Some(reason) if is_aux_cause(reason) => format!("{reason}; guardado como {name}"),
        Some(reason) => format!(
            "guardado como {name}: el original se convierte al abrir ({reason}) y no se sobrescribe"
        ),
    }
}

/// Mismo fichero: `canonicalize` y, si falla (no existe), `std::path::absolute`.
fn same_file(a: &Path, b: &Path) -> bool {
    let resolve = |path: &Path| std::fs::canonicalize(path).or_else(|_| std::path::absolute(path));
    matches!((resolve(a), resolve(b)), (Ok(a), Ok(b)) if a == b)
}

/// Aviso al guardar con un diálogo: con copia por conversión, dice que el original no se tocó.
pub fn describe_saved_dialog(saved: &Saved) -> String {
    match &saved.copy_reason {
        None => describe_saved(saved),
        Some(reason) if is_aux_cause(reason) => describe_saved(saved),
        Some(_) => format!(
            "el original se convirtió al abrir; guardado como {}",
            file_label(&saved.path)
        ),
    }
}

#[cfg(test)]
mod aseprite_tests;
#[cfg(test)]
mod aux_tests;
#[cfg(test)]
mod c1_tests;
#[cfg(test)]
mod c2_tests;
#[cfg(test)]
mod c3_tests;
#[cfg(test)]
mod color_cycle_tests;
#[cfg(test)]
mod convert_tests;
#[cfg(test)]
mod dialog_tests;
#[cfg(test)]
mod edit_tests;
mod layer_ops;
pub use layer_ops::*;
mod merge_ops;

#[cfg(test)]
mod layer_ops_tests;
#[cfg(test)]
mod layers_tests;
#[cfg(test)]
mod merge_tests;
#[cfg(test)]
mod new_image_tests;
#[cfg(test)]
mod ops_tests;
#[cfg(test)]
mod panel_tests;
#[cfg(test)]
mod pick_tests;
#[cfg(test)]
mod tests;
#[cfg(test)]
mod tiles_tests;
#[cfg(test)]
mod transparent_tests;
#[cfg(test)]
mod two_colors_tests;
