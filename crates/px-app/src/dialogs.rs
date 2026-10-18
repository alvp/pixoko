//! Diálogos de archivo no bloqueantes detrás de un trait (specs/hito-2-dialogos-archivo.md).
//! Sin egui salvo `RfdPicker` y el `Frame` padre.

use std::future::Future;
use std::path::{Path, PathBuf};
use std::pin::Pin;
use std::task::{Context, Poll, Waker};
use std::time::Duration;

use crate::confirm::{CONFIRM_SECS, Confirm, Confirmation, Request};
use crate::files::{copy_path, file_label};

/// Un `None` antes de este plazo (segundos), en Linux, es un fallo del diálogo y no una cancelación.
pub const UNAVAILABLE_SECS: f64 = 0.5;
/// Cada cuánto se pide un repintado mientras hay un diálogo abierto.
pub const REPAINT_EVERY: Duration = Duration::from_millis(100);
/// Aviso al soltar un fichero con un diálogo abierto.
pub const IGNORED_FILE_WHILE_DIALOG: &str = "se ignora el fichero: hay un diálogo abierto";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DialogKind {
    Open,
    /// `Ctrl`+`S` sin ruta.
    Save,
    /// `Ctrl`+`Mayús`+`S`.
    SaveAs,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DialogRequest {
    pub kind: DialogKind,
    pub dir: Option<PathBuf>,
    pub file_name: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Outcome {
    Chosen(PathBuf),
    Cancelled,
    Unavailable,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Finished {
    pub kind: DialogKind,
    pub discard: bool,
    pub outcome: Outcome,
}

pub trait FilePicker {
    fn start(&mut self, request: &DialogRequest, parent: Option<&eframe::Frame>);
    /// `None`: sigue abierto. `Some(None)`: se cerró sin ruta. `Some(Some(ruta))`: elegida.
    fn poll(&mut self) -> Option<Option<PathBuf>>;
    /// Abandona el diálogo pendiente: su resultado, si llega, se ignora.
    fn abandon(&mut self);
}

/// Extensiones del filtro de abrir. Los filtros del portal de Linux distinguen mayúsculas; en
/// Windows y macOS no hace falta.
#[cfg(target_os = "linux")]
const PNG_EXTENSIONS: &[&str] = &["png", "PNG", "Png"];
#[cfg(not(target_os = "linux"))]
const PNG_EXTENSIONS: &[&str] = &["png"];

/// Nombre propuesto cuando el documento no tiene ruta.
const UNTITLED_NAME: &str = "sin-titulo.png";

type PendingPick = Pin<Box<dyn Future<Output = Option<PathBuf>>>>;

/// Selector real, sobre `rfd`: el futuro se consulta a mano con un `Waker` vacío, en cada
/// fotograma.
#[derive(Default)]
pub struct RfdPicker {
    pending: Option<PendingPick>,
}

impl RfdPicker {
    pub fn new() -> RfdPicker {
        RfdPicker::default()
    }
}

impl FilePicker for RfdPicker {
    fn start(&mut self, request: &DialogRequest, parent: Option<&eframe::Frame>) {
        let mut dialog = rfd::AsyncFileDialog::new();
        dialog = match request.kind {
            DialogKind::Open => dialog
                .set_title("Abrir PNG")
                .add_filter("PNG", PNG_EXTENSIONS)
                .add_filter("Todos", &["*"]),
            DialogKind::Save | DialogKind::SaveAs => {
                dialog.set_title("Guardar PNG").add_filter("PNG", &["png"])
            }
        };
        if let Some(dir) = &request.dir {
            dialog = dialog.set_directory(dir);
        }
        if let Some(name) = &request.file_name {
            dialog = dialog.set_file_name(name);
        }
        if let Some(frame) = parent {
            dialog = dialog.set_parent(frame);
        }
        self.pending = Some(match request.kind {
            DialogKind::Open => Box::pin(async move {
                dialog
                    .pick_file()
                    .await
                    .map(|handle| handle.path().to_path_buf())
            }),
            DialogKind::Save | DialogKind::SaveAs => Box::pin(async move {
                dialog
                    .save_file()
                    .await
                    .map(|handle| handle.path().to_path_buf())
            }),
        });
    }

    fn poll(&mut self) -> Option<Option<PathBuf>> {
        let pending = self.pending.as_mut()?;
        match pending
            .as_mut()
            .poll(&mut Context::from_waker(Waker::noop()))
        {
            Poll::Ready(result) => {
                self.pending = None;
                Some(result)
            }
            Poll::Pending => None,
        }
    }

    fn abandon(&mut self) {
        self.pending = None;
    }
}

struct Pending {
    kind: DialogKind,
    discard: bool,
    started: f64,
}

pub struct Dialogs {
    picker: Box<dyn FilePicker>,
    fast_failure: bool,
    cwd: Option<PathBuf>,
    last_dir: Option<PathBuf>,
    pending: Option<Pending>,
}

impl Dialogs {
    pub fn new(picker: Box<dyn FilePicker>, fast_failure: bool, cwd: Option<PathBuf>) -> Dialogs {
        Dialogs {
            picker,
            fast_failure,
            cwd,
            last_dir: None,
            pending: None,
        }
    }

    pub fn pending(&self) -> bool {
        self.pending.is_some()
    }

    /// `false` y sin tocar nada si ya hay un diálogo. `discard`: abrir descarta los cambios (ya
    /// confirmado).
    pub fn begin(
        &mut self,
        kind: DialogKind,
        discard: bool,
        path: Option<&Path>,
        converted: bool,
        now: f64,
        parent: Option<&eframe::Frame>,
    ) -> bool {
        if self.pending.is_some() {
            return false;
        }
        let request = request_for(
            kind,
            path,
            converted,
            self.last_dir.as_deref(),
            self.cwd.as_deref(),
        );
        self.picker.start(&request, parent);
        self.pending = Some(Pending {
            kind,
            discard,
            started: now,
        });
        true
    }

    pub fn poll(&mut self, now: f64) -> Option<Finished> {
        let pending = self.pending.as_ref()?;
        let result = self.picker.poll()?;
        let outcome = match result {
            Some(path) => Outcome::Chosen(path),
            None if self.fast_failure && now - pending.started < UNAVAILABLE_SECS => {
                Outcome::Unavailable
            }
            None => Outcome::Cancelled,
        };
        let finished = Finished {
            kind: pending.kind,
            discard: pending.discard,
            outcome,
        };
        self.pending = None;
        Some(finished)
    }

    /// Cierra el estado pendiente y abandona el diálogo: su resultado, si llega, se ignora.
    pub fn abandon(&mut self) {
        if self.pending.take().is_some() {
            self.picker.abandon();
        }
    }

    pub fn repaint_after(&self) -> Option<Duration> {
        self.pending.is_some().then_some(REPAINT_EVERY)
    }

    /// El directorio de un fichero abierto o guardado con éxito.
    pub fn remember_dir(&mut self, file: &Path) {
        if let Some(dir) = file.parent().filter(|dir| !dir.as_os_str().is_empty()) {
            self.last_dir = Some(dir.to_path_buf());
        }
    }
}

/// Directorio y nombre propuestos: el directorio del fichero actual; sin ruta, el último usado y,
/// si no, el directorio actual del proceso. Abrir no propone nombre; guardar propone el del
/// fichero (`nombre-copia.png` si se convirtió al abrir) o `sin-titulo.png` sin ruta.
pub fn request_for(
    kind: DialogKind,
    path: Option<&Path>,
    converted: bool,
    last_dir: Option<&Path>,
    cwd: Option<&Path>,
) -> DialogRequest {
    let dir = path
        .and_then(Path::parent)
        .filter(|dir| !dir.as_os_str().is_empty())
        .or(last_dir)
        .or(cwd)
        .map(Path::to_path_buf);
    let file_name = match kind {
        DialogKind::Open => None,
        DialogKind::Save | DialogKind::SaveAs => Some(match path {
            Some(path) if converted => file_label(&copy_path(path)),
            Some(path) => file_label(path),
            None => UNTITLED_NAME.to_string(),
        }),
    };
    DialogRequest {
        kind,
        dir,
        file_name,
    }
}

/// Añade `.png` salvo que la extensión ya sea `png` (sin distinguir mayúsculas).
pub fn with_png_extension(path: PathBuf) -> PathBuf {
    if path
        .extension()
        .is_some_and(|extension| extension.eq_ignore_ascii_case("png"))
    {
        return path;
    }
    let mut name = path.into_os_string();
    name.push(".png");
    PathBuf::from(name)
}

/// `Some(discard)`: abrir el diálogo ya. `None`: confirmación armada, hay que avisar.
pub fn open_gate(modified: bool, confirm: &mut Confirm, now: f64) -> Option<bool> {
    if !modified {
        return Some(false);
    }
    (confirm.request(Request::OpenDialog, now) == Confirmation::Confirmed).then_some(true)
}

/// Lo que se arma tras un fallo del diálogo: `Open` nada; `Save` y `SaveAs`, su segunda pulsación.
pub fn fallback_request(kind: DialogKind) -> Option<Request> {
    match kind {
        DialogKind::Open => None,
        DialogKind::Save => Some(Request::SaveFallback { as_copy: false }),
        DialogKind::SaveAs => Some(Request::SaveFallback { as_copy: true }),
    }
}

pub fn unavailable_notice(kind: DialogKind) -> String {
    let what = match kind {
        DialogKind::Open => "arrastra un PNG sobre la ventana".to_string(),
        DialogKind::Save => {
            format!("pulsa Ctrl+S otra vez en {CONFIRM_SECS:.0} s para guardar en sin-titulo.png")
        }
        DialogKind::SaveAs => {
            format!("pulsa Ctrl+Mayús+S otra vez en {CONFIRM_SECS:.0} s para guardar una copia")
        }
    };
    format!("no hay diálogo de archivos: falta xdg-desktop-portal o zenity; {what}")
}

#[cfg(test)]
mod tests;
