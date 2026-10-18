# Hito 2 · Unidad 16: diálogos de archivo con `rfd`

Estado: **aprobada** (2026-10-18; decisiones 1, 2, 3, 4 y 6 como se recomendaban, con los cambios del visto bueno incorporados al texto, que **prevalecen sobre cualquier versión anterior**: (5) el fallo del diálogo no escribe nada: avisa y solo una segunda pulsación escribe, por confirmación por repetición; (7) repintado periódico mientras hay un diálogo; (8) cerrar la ventana con un diálogo abierto abandona el futuro; (9) directorio y nombre propuestos, y filtro de abrir sin distinguir mayúsculas; (3) la extensión no distingue mayúsculas y el error de «ya existe» tiene su texto; (4) la copia por conversión con el texto del aviso). Orden, decisiones D1 a D10 y datos de `rfd`: [`hito-2-reparto-cierre.md`](hito-2-reparto-cierre.md). Abrir, guardar, `Session`, `Confirm`, `write_atomic`, `copy_path` y `untitled_path`: [`hito-2-abrir-guardar.md`](hito-2-abrir-guardar.md). Avisos de 5 s y barra de estado: [`hito-2-linea-relleno-app.md`](hito-2-linea-relleno-app.md) y [`hito-2-texto.md`](hito-2-texto.md).

## Objetivo

`Ctrl`/`Cmd`+`O` abre un diálogo nativo de abrir; `Ctrl`+`S` sin ruta y `Ctrl`+`Mayús`+`S` abren uno de guardar. Los diálogos no bloquean el dibujo, van detrás de un trait y toda la lógica se prueba sin ventana. Sin portal ni `zenity`, la app avisa y arrastrar y soltar y `Ctrl`+`S` siguen funcionando. El menú de la unidad 17 llamará a las mismas piezas.

## Lo comprobado

- `rfd` 0.17.2 compila (`cargo check -p px-app --locked`, Linux) en una copia con la configuración de abajo; `cargo deny --locked check` da `advisories ok, bans ok, licenses ok, sources ok` sin tocar `deny.toml`. **No** lo he compilado en Windows ni macOS.
- `AsyncFileDialog` devuelve un futuro que se puede consultar a mano con `Waker::noop()`. En Linux lo ejecuta en un hilo propio (`async_thread`); su resultado es `Option<FileHandle>`, y **`None` vale tanto para «cancelado» como para «fallo»**: no se distinguen.
- Sin bus D-Bus, sin pantalla y sin `zenity` en el `PATH`, el futuro resuelve a `None` a los **10 ms** (probado con un programa suelto, sin ventana).
- `eframe::Frame` implementa `HasWindowHandle` y `HasDisplayHandle`, así que vale para `set_parent`.
- **No he probado:** un portal que responde pero no tiene backend, un portal que tarda en activarse, ni ningún diálogo real (eso es la prueba manual). Tampoco si los filtros con `png`, `PNG` y `Png` los respetan todos los backends de Linux.

## Dependencia (se anota al implementar)

En `crates/px-app/Cargo.toml`: `rfd = { version = "=0.17.2", default-features = false, features = ["xdg-portal", "wayland"] }` en `cfg(target_os = "linux")`, y `rfd = { version = "=0.17.2", default-features = false }` en `cfg(not(target_os = "linux"))`. Fijada con `=` como `eframe`.

`docs/DEPENDENCIES.md`: fila de `rfd` (MIT), sus directas y las 2 entradas nuevas de `Cargo.lock` (`rfd`, `pollster`), y esta nota: **el código de `rfd` usa `unsafe`** (carga `libdbus-1.so` con `dlopen` en Linux y llamadas a la API nativa en Windows y macOS). La regla de `CLAUDE.md` es sobre nuestro código, que no cambia (`unsafe_code = "forbid"` en `px-app`). Requisitos en Linux: `xdg-desktop-portal` con un backend de GTK, GNOME o KDE (el de wlroots solo no sirve), `libdbus-1.so.3` en ejecución, y `zenity` como respaldo. El CI no necesita paquetes nuevos.

## Cambios en otras specs

- `hito-2-abrir-guardar.md`, «Sin diálogos»: **`Ctrl`+`S` sin ruta ya no escribe `sin-titulo.png` en el directorio actual, sino que abre el diálogo** (solo lo escribe tras un fallo del diálogo y una segunda pulsación, ver 7). **`Ctrl`+`Mayús`+`S` ya no escribe `nombre-copia.png` solo: abre el diálogo.** `Session::save` y `Session::save_as` no cambian y quedan como respaldo; ninguna prueba de la unidad 15 cambia (se comprueba al implementar; si alguna necesitara algo más que ajustar el cableado, me paro y aviso). Los auto-repetidos de teclado (`repeat`) no disparan atajos de fichero.
- `px-ui`: `FileAction` gana `Open`.
- `px-app/confirm.rs`: `Request` gana `OpenDialog` y `SaveFallback { as_copy: bool }`, y `Confirm` gana `confirm_armed` (aditivo; `request` y sus pruebas no cambian).

## Piezas (sin egui salvo `main.rs` y `RfdPicker`)

```rust
// px-ui/src/shortcuts.rs: FileAction { Open, Save, SaveAs }; file_action_for_key: command+O (sin Mayús ni Alt) => Open

// px-app/src/files.rs: FileError gana `Exists(PathBuf)`; Display: «ya existe X.png; elige otro nombre» (solo el nombre)

// px-app/src/confirm.rs
pub enum Request { Open(PathBuf), Close, OpenDialog, SaveFallback { as_copy: bool } }
impl Confirm {
    /// `true` y la vacía si la pendiente es igual a `request` y `armada ≤ now < armada + 5`; si no, `false`
    /// y no cambia nada (no arma). Sirve para «segunda pulsación» sin armar la primera.
    pub fn confirm_armed(&mut self, request: &Request, now: f64) -> bool;
}
pub fn open_dialog_warning() -> String;

// px-app/src/dialogs.rs
pub const UNAVAILABLE_SECS: f64 = 0.5;            // un `None` antes de este plazo, en Linux, es un fallo
pub const REPAINT_EVERY: Duration = 100 ms;
pub const IGNORED_FILE_WHILE_DIALOG: &str;        // «se ignora el fichero: hay un diálogo abierto»
pub enum DialogKind { Open, Save, SaveAs }        // Save: Ctrl+S sin ruta; SaveAs: Ctrl+Mayús+S
pub struct DialogRequest { pub kind: DialogKind, pub dir: Option<PathBuf>, pub file_name: Option<String> }
pub enum Outcome { Chosen(PathBuf), Cancelled, Unavailable }
pub struct Finished { pub kind: DialogKind, pub discard: bool, pub outcome: Outcome }

pub trait FilePicker {
    fn start(&mut self, request: &DialogRequest, parent: Option<&eframe::Frame>);
    /// `None`: sigue abierto. `Some(None)`: se cerró sin ruta. `Some(Some(ruta))`: elegida.
    fn poll(&mut self) -> Option<Option<PathBuf>>;
    /// Abandona el diálogo pendiente: su resultado, si llega, se ignora.
    fn abandon(&mut self);
}
pub struct RfdPicker { /* futuro pendiente */ }   // filtros, título, directorio y nombre; `set_parent` con el Frame

pub struct Dialogs { /* picker, pendiente (tipo, descarte, instante), último directorio, directorio actual */ }
impl Dialogs {
    pub fn new(picker: Box<dyn FilePicker>, fast_failure: bool, cwd: Option<PathBuf>) -> Dialogs; // fast_failure = cfg!(target_os = "linux")
    pub fn pending(&self) -> bool;
    /// `false` y sin tocar nada si ya hay un diálogo. `discard`: abrir descarta los cambios (ya confirmado).
    pub fn begin(&mut self, kind: DialogKind, discard: bool, path: Option<&Path>, converted: bool, now: f64, parent: Option<&eframe::Frame>) -> bool;
    pub fn poll(&mut self, now: f64) -> Option<Finished>;
    pub fn abandon(&mut self);                     // cierra el estado pendiente y llama a `picker.abandon()`
    pub fn repaint_after(&self) -> Option<Duration>; // `Some(REPAINT_EVERY)` solo con un diálogo pendiente
    pub fn remember_dir(&mut self, file: &Path);   // el directorio de un fichero abierto o guardado con éxito
}
pub fn request_for(kind: DialogKind, path: Option<&Path>, converted: bool, last_dir: Option<&Path>, cwd: Option<&Path>) -> DialogRequest;
pub fn with_png_extension(path: PathBuf) -> PathBuf;
pub fn open_gate(modified: bool, confirm: &mut Confirm, now: f64) -> Option<bool>;   // Some(discard) = abrir el diálogo ya
pub fn fallback_request(kind: DialogKind) -> Option<Request>;                          // Open => None; Save => SaveFallback{false}; SaveAs => SaveFallback{true}
pub fn unavailable_notice(kind: DialogKind) -> String;

// px-app/src/session.rs
impl Session { pub fn save_to(&mut self, chosen: &Path) -> Result<Saved, FileError>; }
pub fn describe_saved_dialog(saved: &Saved) -> String;
```

## Comportamiento

1. **`Ctrl`+`O`.** Sin cambios sin guardar abre el diálogo de abrir. Con cambios, **la confirmación va antes del diálogo** (`open_gate`): la primera pulsación arma `Request::OpenDialog` y avisa (`open_dialog_warning`: «hay cambios sin guardar: pulsa Ctrl+O otra vez en 5 s para descartarlos, o guarda con Ctrl+S»); la segunda dentro de 5 s abre el diálogo con `discard = true`. La ruta elegida se abre con `Session::open(ruta, max_side, discard)`: el descarte ya está confirmado y no se vuelve a pedir. Con otra petición entre medias (un fichero soltado, el cierre), la confirmación se vuelve a armar, como en la unidad 15.
2. **Guardar.** `Ctrl`+`S` con ruta, igual que hoy (sobrescribe, o hace la copia si el fichero se convierte al abrir). `Ctrl`+`S` sin ruta abre el diálogo de guardar (`Save`). `Ctrl`+`Mayús`+`S` abre el de guardar como (`SaveAs`).
3. **Directorio y nombre propuestos (`request_for`).**
   - Directorio: el del fichero actual; sin ruta, el último directorio usado con éxito en esta sesión (`remember_dir`, solo en memoria); si no, el directorio actual del proceso (`cwd`); si tampoco hay, ninguno.
   - Nombre en `Save` y `SaveAs`: el del fichero actual; **`nombre-copia.png` (el de `copy_path`) si el PNG se convirtió al abrir** (`converted`); `sin-titulo.png` si no hay ruta. Abrir no propone nombre.
   - **Filtros.** Abrir: «PNG» sin distinguir mayúsculas (en Linux, las extensiones `png`, `PNG` y `Png`, porque los filtros del portal distinguen; en Windows y macOS, `png`) y un segundo filtro «Todos» (`*`) por si el backend no ve un `.pNg`. Guardar: «PNG».
4. **Ruta elegida al guardar (`Session::save_to`).** `with_png_extension` añade `.png` salvo que la extensión ya sea `png` **sin distinguir mayúsculas** (`casa` ⇒ `casa.png`; `casa.PNG` se queda; `casa.bmp` ⇒ `casa.bmp.png`; `.png` ⇒ `.png.png`). **La elección sobrescribe**: el sistema ya preguntó si se reemplaza, y se escribe con `write_atomic` (sin temporales, el destino anterior intacto si falla). Excepción: **si se añadió la extensión y el fichero resultante existe, `Err(Exists)` («ya existe casa.png; elige otro nombre») y no se escribe nada**, porque el sistema no preguntó por ese nombre. Tras guardar con éxito, la ruta del documento es la escrita, `modified` es falso y la conversión pendiente queda borrada.
5. **No se sobrescribe un PNG que se convierte al abrir.** Si la ruta elegida es la del documento (mismo fichero, comparado con `canonicalize` y, si falla, con `std::path::absolute`) y este tiene conversión pendiente, `save_to` hace lo mismo que `save`: escribe `copy_path` y devuelve `copy_reason`. El aviso (`describe_saved_dialog`) dice «el original se convirtió al abrir; guardado como casa-copia.png». Para sobrescribir el original hay que elegir otro nombre o hacerlo fuera de la app. Con otra ruta se escribe tal cual, sin motivo de copia. (`describe_saved` de la unidad 15 no cambia.)
6. **Mientras haya un diálogo abierto**, `busy()` es verdadero: se ignoran herramientas, atajos de fichero y de edición, y los ficheros soltados (aviso `IGNORED_FILE_WHILE_DIALOG`). **El repintado:** la app consulta el diálogo en cada fotograma (`Dialogs::poll`) y, mientras `repaint_after()` sea `Some`, pide un repintado con `request_repaint_after(REPAINT_EVERY)`, para procesar el resultado sin que el usuario mueva el ratón. **Cerrar la ventana con un diálogo abierto:** `close_requested` llama a `Dialogs::abandon` (el futuro se abandona y su resultado se ignora) y se aplica la regla de cierre de la unidad 15 (con cambios sin guardar, confirmación por repetición; sin ellos, cierra).
7. **Resultado.**
   - `Chosen(ruta)`: abre o guarda como en 1 a 5 y, con éxito, `remember_dir`.
   - `Cancelled`: no pasa nada (sin aviso) y se limpia la confirmación.
   - `Unavailable`: **no se escribe nada.** Aviso en la barra de estado (`unavailable_notice`). Para `Save` y `SaveAs` se arma `fallback_request(kind)` (`Confirm::request`, ya con el instante) y el aviso lo dice: con `Save`, «no hay diálogo de archivos: falta xdg-desktop-portal o zenity; pulsa Ctrl+S otra vez en 5 s para guardar en sin-titulo.png»; con `SaveAs`, «no hay diálogo de archivos: falta xdg-desktop-portal o zenity; pulsa Ctrl+Mayús+S otra vez en 5 s para guardar una copia»; con `Open`, «no hay diálogo de archivos: falta xdg-desktop-portal o zenity; arrastra un PNG sobre la ventana».
   - **Segunda pulsación.** Antes de abrir un diálogo de guardar, `Confirm::confirm_armed(SaveFallback { as_copy }, now)`: si es verdadero, **no se abre el diálogo**: se hace lo de la unidad 15 (`Session::save` o `Session::save_as` en el directorio actual, sin pisar nada) y se avisa con `describe_saved`. Si no, se abre el diálogo. Una cancelación rápida del usuario **no escribe nada**: da el mismo aviso, y solo una segunda pulsación dentro de 5 s escribe.
8. **Qué es `Unavailable`.** En Linux (`fast_failure`), un `Some(None)` antes de `UNAVAILABLE_SECS` desde `begin`. Después de ese plazo, o en Windows y macOS, es `Cancelled`. Una ruta elegida siempre es `Chosen`. Es una heurística: un cierre inmediato por el usuario también la dispara (por eso el aviso dice «falta…» sin hablar de error y no escribe sin una segunda pulsación). Se vuelve a intentar en cada pulsación; no se recuerda el fallo.
9. **Ventana padre.** `RfdPicker::start` hace `set_parent` con el `Frame` si se da, para que el diálogo sea modal respecto a la ventana (en Wayland, gracias a la feature `wayland`).

## Limitaciones

- Un solo fichero por diálogo; sin ficheros recientes ni directorio recordado entre sesiones.
- Un diálogo abierto de Linux (portal o `zenity`) no se puede cancelar desde la app: tras abandonarlo (cerrar la ventana), el diálogo se queda hasta que se cierre, y su resultado se ignora.
- Un portal presente sin backend, o muy lento en activarse, no está cubierto por la heurística de 8 (sin probar).
- Los filtros de Linux listan `png`, `PNG` y `Png`: un `.pNg` solo se ve con el filtro «Todos».
- Tampoco hay diálogos de mensaje, ni selección de carpeta, ni abrir varios ficheros.

## Fuera de alcance

El menú y los botones (unidad 17), el diálogo de nueva imagen y el de convertir (19 y 20), un diálogo propio de egui como respaldo, y conservar los chunks auxiliares al guardar (13b).

## Casos límite

`tmp` es un directorio temporal propio de cada test (como en la unidad 15). Los diálogos se prueban con un `FakePicker` que resuelve cuando el test lo dice y cuenta las llamadas a `start` y `abandon`; el instante `now` lo da el test.

| # | Entrada | Resultado |
| --- | --- | --- |
| 1 | `request_for`: `Open` con ruta `/a/b/casa.png`, último directorio `/x` y `cwd` `/c`; `Open` sin ruta con `/x`; `Open` sin ruta ni último y con `/c`; `Open` sin nada; `SaveAs` con ruta, sin y con `converted`; `Save` y `SaveAs` sin ruta | `dir` `/a/b`, `/x`, `/c`, `None`; `file_name` `None` en `Open`; `Some("casa.png")`, `Some("casa-copia.png")` (`copy_path`) y `Some("sin-titulo.png")` en los dos últimos |
| 2 | `with_png_extension`: `casa`, `casa.png`, `casa.PNG`, `casa.Png`, `casa.bmp`, `casa.tar.gz`, `.png`, `dir/casa` | `casa.png`, igual, igual, igual, `casa.bmp.png`, `casa.tar.gz.png`, `.png.png`, `dir/casa.png` |
| 3 | `Dialogs::begin` (Open, sin descarte) y un segundo `begin` con el primero abierto; `poll` y `repaint_after` mientras el `FakePicker` no resuelve; luego resuelve con una ruta | primer `begin` `true` y `pending()`; el segundo `false` y `start` se llamó **una** vez; `poll` da `None` y `repaint_after` `Some(100 ms)`; después `Finished { kind: Open, discard: false, outcome: Chosen(ruta) }`, `pending()` falso y `repaint_after` `None` |
| 4 | Resuelve `Some(None)` a los 0,2 s con `fast_failure`; a los 3,0 s con `fast_failure`; a los 0,2 s sin `fast_failure`; `Some(ruta)` a los 0,0 s con `fast_failure` | `Unavailable`; `Cancelled`; `Cancelled`; `Chosen(ruta)` |
| 5 | `Session::save_to` con `<tmp>/nuevo` (sin extensión); con un `<tmp>/viejo.png` existente de 1 KiB; con `<tmp>/otro` cuando ya existe `<tmp>/otro.png`; con `<tmp>/X.PNG` | escribe `nuevo.png`, ruta nueva y `modified` falso; el contenido nuevo completo y ningún `.pixoko-tmp`; `Err(Exists(<tmp>/otro.png))` con el texto «ya existe otro.png; elige otro nombre», nada escrito y `modified` sigue cierto; escribe `X.PNG` sin añadir nada |
| 6 | Sesión abierta de un PNG con informe de conversión no vacío: `save_to` con la ruta del documento (y también con una ruta equivalente: con `.`/`..` o un enlace simbólico); y con otra ruta | escribe `copy_path`, devuelve `copy_reason` y `describe_saved_dialog` da «el original se convirtió al abrir; guardado como X-copia.png», el original intacto byte a byte; con otra ruta, escribe la elegida sin motivo (`describe_saved_dialog` da `guardado …`) y la conversión pendiente queda borrada |
| 7 | `open_gate`: sin cambios; con cambios, dos veces dentro de 5 s; con cambios, dos veces separadas 5 s; con cambios, `OpenDialog`, luego `Request::Close` y de nuevo `OpenDialog` | `Some(false)`; `None` y luego `Some(true)`; `None` las dos veces; `None`, `None` (la otra petición rearma) |
| 8 | Resultado con sesión modificada y `discard = true`: `Chosen` de un PNG del corpus; `Cancelled`; `Chosen` de un fichero que no es PNG | abre y descarta (historia vacía, `modified` falso); sesión intacta y confirmación limpia; error de `read_png` y sesión intacta (filas 3 y 6 de la unidad 15) |
| 9 | Fallo y segunda pulsación: `fallback_request` de los tres tipos; `unavailable_notice` de los tres; `confirm_armed(SaveFallback{false})` sin armar, tras armar (dentro de 5 s), tras armar y esperar 5 s, tras armar `SaveFallback{true}`; con la armada confirmada, `Session::save(<tmp>)`; y `Cancelled` rápido | `None`, `Some(SaveFallback{false})`, `Some(SaveFallback{true})`; los textos de 7 (el de `Save` con `Ctrl+S` y `sin-titulo.png`, el de `SaveAs` con `Ctrl+Mayús+S`); `false` y sin armar; `true` (y vacía la pendiente); `false`; `false` (otra clase); la escritura es `<tmp>/sin-titulo.png` sin pisar nada; **sin fallo no hay armado, y `Cancelled` no escribe ni arma** |
| 10 | `file_action_for_key`: `O` con `command`; con `command`+`Mayús`; con `command`+`Alt`; `O` sin `command`; y las filas de `S` de la unidad 15 | `Some(Open)`, `None`, `None`, `None`; `S` y `Mayús`+`S` siguen como antes |
| 11 | Con un diálogo pendiente: `begin` de otro tipo; `Dialogs::abandon` y que el `FakePicker` resuelva después; tras resolver otro | el segundo `begin` es `false` sin `start`; tras `abandon`, `pending()` falso, `abandon` llamado **una** vez en el `FakePicker` y `poll` da `None` aunque resuelva; `pending()` cierto hasta que `poll` da `Finished` y falso después (`busy` cierto solo entonces); con cambios sin guardar, la regla de cierre de la unidad 15 (`Confirm`, filas de `Request::Close`) no cambia |
| 12 | Textos nuevos: `unavailable_notice` (los tres), `open_dialog_warning`, `IGNORED_FILE_WHILE_DIALOG`, `describe_saved_dialog` con copia, el `Display` de `Exists` | solo caracteres de `INTERFACE_CHARSET`; los dos primeros de `Save` y `SaveAs` contienen `xdg-desktop-portal`, `zenity` y el atajo con «otra vez en 5 s»; `open_dialog_warning`, `Ctrl+O` y `Ctrl+S` |

## Prueba manual (la haces tú, sin capturas)

En cada sistema, con `cargo run --release -p px-app`:

1. `Ctrl`+`O` (`Cmd`+`O` en macOS): aparece el diálogo nativo con el filtro PNG; elegir un PNG lo abre y el título dice `abierto …`. `Escape` en el diálogo: no cambia nada y no sale ningún aviso. El directorio inicial es el del fichero abierto.
2. Con el diálogo abierto, el diálogo queda por encima de la ventana de Pixoko. Redimensiona la ventana de Pixoko (si el sistema lo permite con un diálogo modal): se repinta sin quedarse congelada. Mientras tanto, `Ctrl`+`Z`, el ratón sobre el lienzo y soltar un fichero no hacen nada (soltar avisa «hay un diálogo abierto»). Elegir un fichero **sin tocar el ratón ni el teclado** de Pixoko: se abre solo, sin esperar un movimiento.
3. Pintar un píxel y `Ctrl`+`O`: el título avisa; `Ctrl`+`O` otra vez dentro de 5 s abre el diálogo, y al elegir un fichero se descarta el dibujo. Esperar más de 5 s entre las dos pulsaciones: vuelve a avisar. Mantener `Ctrl`+`O` pulsado: solo avisa la primera vez.
4. `Ctrl`+`Mayús`+`S`: diálogo de guardar con el nombre propuesto (el del fichero actual; con un PNG que Pixoko convierte al abrir, `nombre-copia.png`). Escribir un nombre sin extensión: se guarda `nombre.png`. Escribir `X.PNG`: se guarda como `X.PNG`. Elegir un fichero existente: el sistema pregunta y, al aceptar, se reemplaza. Escribir `x.bmp`: se guarda `x.bmp.png`. Con un `x.bmp.png` ya existente: aviso «ya existe x.bmp.png; elige otro nombre» y no se escribe. Con un PNG convertido al abrir, elegir su propio nombre: se guarda `…-copia.png` y el aviso dice «el original se convirtió al abrir; guardado como …».
5. Imagen sin ruta (arranque sin argumento): `Ctrl`+`S` abre el diálogo de guardar con `sin-titulo.png`.
6. **Solo Linux, sin portal ni `zenity`:** `env -u DBUS_SESSION_BUS_ADDRESS PATH=/nonexistent <ruta absoluta del binario>` (sin `zenity` en el `PATH`; no puede aparecer ningún diálogo). `Ctrl`+`O`: aviso `no hay diálogo de archivos…` en la barra de estado; arrastrar un PNG lo abre. Sin ruta, `Ctrl`+`S`: el aviso dice que pulses `Ctrl`+`S` otra vez en 5 s; **no se escribe nada** hasta la segunda pulsación, que guarda `sin-titulo.png` en el directorio actual (sin pisar uno existente). Pasados los 5 s, `Ctrl`+`S` vuelve a intentar el diálogo.
7. Linux X11 y Wayland (`env -u WAYLAND_DISPLAY` fuerza X11): el diálogo sale como modal de la ventana en los dos. Con el portal de KDE (`xdg-desktop-portal-kde`) el diálogo es el de Plasma.
8. Con un diálogo abierto, cerrar la ventana de Pixoko: sin cambios cierra; con cambios, el primer cierre avisa y el segundo cierra.

## Orden de implementación

Esqueleto con las firmas; tests de las filas 1 a 12 (subagente `test-writer`) con `FakePicker`; `FileAction::Open` y `file_action_for_key`; `FileError::Exists`, `Session::save_to`, `describe_saved_dialog`, `with_png_extension`; `Confirm::confirm_armed`; `Dialogs`, `open_gate`, `fallback_request`, `unavailable_notice`; `RfdPicker` y el cableado en `main.rs`; `Cargo.toml`, `Cargo.lock` y `docs/DEPENDENCIES.md`; `cargo deny` en el repo y `license-auditor`. Commits pequeños, sin tocar tests de unidades anteriores.
