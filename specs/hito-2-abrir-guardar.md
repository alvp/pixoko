# Hito 2 · Unidad 15 (mínima): abrir y guardar PNG en la app

Estado: **aprobada** (2026-10-17; decisiones 1 a 8 con los cambios del visto bueno incorporados al texto: `Document::state_id` en `px-core` en vez de la pila paralela, `png_dimensions` en `px-formats` para rechazar antes de decodificar, confirmación por repetición al abrir y al cerrar, error del argumento también a stderr, título con solo el nombre del fichero, conversión ⇒ copia con la razón en el título, y «Limitaciones»). **Sin dependencias externas nuevas** (`rfd` queda para la unidad 11): `px-app` gana `px-formats` (crate del workspace) y `proptest` como dev-dependency (la misma versión ya registrada).

## Objetivo

Abrir un PNG (argumento de línea de comandos o arrastrar y soltar) y guardarlo (`Ctrl`/`Cmd`+`S`, con `Mayús` «guardar como»), sin menús, sin fuente y sin diálogos. La lógica es pura sobre `std::fs` y se prueba sin ventana.

## Cambios en otras crates (aditivos)

- **`px-core`: `Document::state_id(&self) -> u64`.** Opaco. `0` en el estado inicial; cada paso aplicado (`apply` que devuelve `Ok(true)`) lleva a un identificador **nuevo y nunca reutilizado** (contador interno que solo crece, también si se descarta la rama de rehacer); `undo` y `redo` **recorren** esos ids (deshacer devuelve el id de antes del paso; rehacer, el de después); un `apply` que devuelve `Ok(false)` o `Err`, y un `undo`/`redo` que falla, **no lo cambian**; el descarte de pasos viejos por el límite de bytes (`trim`, `set_history_limit`) no cambia ningún id. `Clone` copia el id. `PartialEq` no cambia (compara imágenes). Implementación prevista: `Document` gana `current_id` y `next_id`, y `Step` gana `state` (el id del otro lado del paso, que se intercambia con `current_id` en `undo` y `redo`). Es aditivo y **no debe tocar ningún test de unidades anteriores**; si lo hiciera, me paro y aviso.
- **`px-formats`: `png_dimensions(bytes: &[u8]) -> Result<(u32, u32), PngError>`.** Lee solo la firma de 8 bytes y el primer chunk, que debe ser `IHDR` de longitud 13, y devuelve `(ancho, alto)`. No decodifica, no reserva nada proporcional y **no verifica el CRC ni nada posterior al IHDR** (de eso se encarga después `read_png`). `Err(Decode(..))` si faltan bytes, la firma es otra, el primer chunk no es `IHDR` de 13 bytes, o una dimensión es 0 o mayor que 2³¹−1 (máximo del formato). No cambia `read_png`.
- **`px-ui`: `FileAction` y `file_action_for_key`** en `shortcuts.rs` (aditivo; `action_for_key` y su propiedad 6 de la unidad 6 no cambian).

```rust
// px-ui/src/shortcuts.rs
pub enum FileAction { Save, SaveAs }
pub fn file_action_for_key(key: Key, modifiers: Modifiers) -> Option<FileAction>;
```

## Piezas de `px-app` (todas sin egui salvo `main.rs`)

```rust
// px-app/src/files.rs
/// Tope del fichero de entrada. TEMPORAL hasta la unidad 14.
pub const MAX_FILE_BYTES: u64 = 256 << 20;
/// Tope de píxeles de una imagen abierta (8192×8192). TEMPORAL hasta la unidad 14.
pub const MAX_OPEN_PIXELS: u64 = 1 << 26;
pub enum FileError {   // Clone, Debug, PartialEq, Eq; Display en español
    NotAFile, TooBigFile { bytes: u64, max: u64 }, Read(String), Png(PngError),
    TooLargeToShow { width: u32, height: u32, max_side: usize },
    TooManyPixels { width: u32, height: u32, max: u64 },
    Unsaved, ReadOnly(PathBuf), Write(String),
}
pub fn parse_args(args: &[OsString]) -> Result<Option<PathBuf>, String>;  // sin el nombre del programa
pub fn pick_dropped(files: &[Option<PathBuf>]) -> Option<(PathBuf, usize)>; // primero con ruta + cuántos se ignoran
pub fn load_png(path: &Path, max_bytes: u64, max_side: usize) -> Result<PngRead, FileError>;
pub fn check_fits(width: u32, height: u32, max_side: usize) -> Result<(), FileError>;
pub fn write_atomic(path: &Path, bytes: &[u8]) -> Result<(), FileError>;
pub fn copy_path(original: &Path) -> PathBuf;      // x.png ⇒ x-copia.png; si existe, x-copia-2.png, -3…
pub fn untitled_path(dir: &Path) -> PathBuf;       // dir/sin-titulo.png; si existe, -2, -3…
pub fn file_label(path: &Path) -> String;          // solo el nombre, sin directorios
pub fn describe_open(name: &str, image: &Image, report: &[Conversion]) -> String;

// px-app/src/confirm.rs
pub const CONFIRM_SECS: f64 = 5.0;
pub enum Request { Open(PathBuf), Close }          // Clone, Debug, PartialEq, Eq
pub enum Confirmation { Armed, Confirmed }         // Debug, PartialEq, Eq
pub struct Confirm { /* petición pendiente e instante */ }
impl Confirm {
    pub fn new() -> Confirm;
    /// El tiempo lo da quien llama (segundos).
    pub fn request(&mut self, request: Request, now: f64) -> Confirmation;
    pub fn clear(&mut self);
}
pub fn open_warning(name: &str) -> String;
pub fn close_warning() -> String;

// px-app/src/session.rs
pub struct Session { pub document: Document, /* ruta, id guardado, nota de conversión */ }
pub struct Saved { pub path: PathBuf, pub copy_reason: Option<String> }
impl Session {
    pub fn sample(document: Document) -> Session;           // sin ruta, sin cambios
    pub fn path(&self) -> Option<&Path>;
    pub fn modified(&self) -> bool;                          // document.state_id() != id guardado
    /// `discard`: reemplazar aunque haya cambios sin guardar (lo da la confirmación).
    pub fn open(&mut self, path: &Path, max_side: usize, discard: bool) -> Result<String, FileError>;
    pub fn save(&mut self, new_dir: &Path) -> Result<Saved, FileError>;
    pub fn save_as(&mut self, new_dir: &Path) -> Result<Saved, FileError>;
}
pub fn describe_saved(saved: &Saved) -> String;
pub fn adjust_slot(image: &Image, slot: u8) -> u8;          // la ranura si vale para la imagen; si no, 1

// px-app/src/title.rs: aditivo (window_title y TitleInfo no cambian)
pub fn title_with_file(file: Option<&str>, modified: bool, title: &str) -> String;
```

`main.rs` es pegamento: argumento y `dropped_files` ⇒ confirmación ⇒ `Session::open`; `FileAction` ⇒ `save`/`save_as`; `close_requested` ⇒ confirmación; avisos por el `Notice` de 5 s de la unidad 7. En egui 0.37 el fichero soltado es un `DroppedFile` con `path()` y `bytes()` (este último lee todo sin tope): se usa `path()` y se lee con `load_png`.

## Comportamiento

### 1. Cómo se abre y se guarda

- **`px-app [archivo.png]`**; sin argumento arranca la imagen de muestra. Más de un argumento: uso por stderr y salida 2, antes de la ventana. **Si el fichero del argumento falla, la ventana se abre con la muestra, el error en el título 5 s y también por stderr** (`px-app: {error}`). El argumento se procesa en el primer fotograma (el lado máximo de textura solo se conoce con un `Context`), por la misma vía que un fichero soltado, pero sin confirmación (la muestra no tiene cambios).
- **Arrastrar y soltar:** se abre el primero con ruta y el aviso dice cuántos se ignoran («se ignoran 2 más»). **Soltar durante un trazo se ignora** (aviso breve). No se mira la extensión: lo que no es PNG da el error de `read_png` en el título.
- **Atajos:** `command`+`S` guarda y `command`+`Mayús`+`S` guarda como, sin `Alt`. `command` es `Ctrl` en Linux y Windows y `Cmd` en macOS (el mismo criterio que deshacer: en macOS `Ctrl`+`S` no hace nada). Se ignoran con un trazo en curso.
- **Sin diálogos** (`rfd` queda para la unidad 11). **Guardar como** escribe junto al original `nombre-copia.png` (si existe, `-copia-2`, `-copia-3`…) y **nunca pisa un fichero existente**; ese fichero pasa a ser el del documento. Sin ruta, `Ctrl`+`S` y guardar como escriben `sin-titulo.png` (o `-2`, `-3`…) **en el directorio actual** sin pisar nunca nada, y esa pasa a ser su ruta.

### 2. Abrir

1. **Cambios sin guardar: confirmación por repetición.** Si `modified()` y `discard` es falso, `Session::open` da `Err(Unsaved)` sin leer el fichero. En la app, con cambios: la primera petición (`Request::Open(ruta)`) arma la confirmación y el título avisa (`open_warning`: «hay cambios sin guardar: suelta casa.png otra vez en 5 s para descartarlos, o guarda con Ctrl+S»); **la segunda petición con la misma ruta dentro de 5 s reemplaza el documento** (`discard = true`). Otra ruta, u otra petición (`Close`), vuelve a armar; pasados los 5 s hay que empezar de nuevo. Tras guardar o abrir con éxito, `Confirm::clear`. Sin cambios no hay confirmación.
2. Se lee con tope: fichero normal (`NotAFile` para directorios, FIFO, dispositivos), de hasta `MAX_FILE_BYTES` (`TooBigFile`, comprobado por `metadata` y por una lectura limitada que no reserva más de `max_bytes`).
3. **Se rechaza antes de decodificar.** `png_dimensions` lee la cabecera; `check_fits(ancho, alto, max_side)` da `TooLargeToShow` si un lado supera `max_side` y, si no, `TooManyPixels` si `ancho × alto` (en `u64`) supera `MAX_OPEN_PIXELS`. `max_side` es **`ctx.input(|i| i.max_texture_side)`**, no un número fijo. Solo si cabe se llama a `read_png`. Las dos constantes son temporales hasta la unidad 14 (texturas por teselas y tope de memoria); con 2^26 el pico entre imagen, copia pendiente y textura queda en torno a 1 GiB, porque `Canvas::set_image` y `to_rgba8` reservan sin comprobar. `Canvas` conserva su `TooLarge` como respaldo. El documento actual sigue intacto en cualquier error.
4. Éxito: `Document::new(imagen)` (historia vacía, `state_id` 0), ruta absoluta (`std::path::absolute`; si falla, la dada), id guardado = 0, **ranura de color `adjust_slot`** (se conserva si vale para el modo y la paleta de la imagen nueva; si no, 1) y **vista ajustada con `Home`** (`Canvas::set_image` ya reinicia la vista al ajuste). La herramienta activa se conserva.
5. Aviso 5 s (`describe_open`): `abierto casa.png · 16×16 · indexado`, más `· convertido: gris de 4 bits ampliado a 8, color clave convertido en alfa` y `· Ctrl+S guardará una copia` si el informe no está vacío (`DepthExpanded{n}` ⇒ «gris de n bits ampliado a 8»; `Depth16Reduced` ⇒ «16 bits reducido a 8»; `RgbToRgba` ⇒ «RGB convertido a RGBA»; `ColorKeyToAlpha` ⇒ «color clave convertido en alfa»). Modos: «indexado», «gris», «gris con alfa», «RGBA».
6. **Título normal:** `title_with_file`, **solo con el nombre** (sin directorios, `file_label`): `casa.png — Pixoko: 16×16 · …`; modificado, `casa.png* — …`; sin ruta y sin cambios, el título de hoy; sin ruta y modificado, `sin título* — …`.

### 3. Guardar

- `write_png(imagen)` y, con los bytes en memoria, `write_atomic`: **temporal en el mismo directorio** (`.nombre.pixoko-tmp-<pid>`, `create_new`), `write_all`, **`sync_all` del temporal antes del `rename`** y `rename` sobre el destino. Si algo falla, se borra el temporal y el destino anterior queda **intacto byte a byte**. Una caída a media escritura puede dejar el temporal, nunca un destino a medias; no se limpia al arrancar. No se hace `fsync` del directorio.
- **Enlace simbólico:** se resuelve con `canonicalize` y se escribe en el destino real; el enlace sigue siendo un enlace. **Permisos:** si el destino existe, se copian al temporal antes del `rename`.
- **Solo lectura:** un fichero con el atributo de solo lectura se rechaza antes de escribir (`ReadOnly`; si no, el `rename` lo pisaría, porque solo mira el permiso del directorio). Un directorio sin permiso de escritura da `Write(..)` al crear el temporal. En ambos casos: error 5 s, `modified` sigue a cierto, no queda ningún fichero nuevo. (Como `root` ignora los permisos, esas pruebas se saltan si la escritura tiene éxito.)
- **Destino:** `save` escribe en la ruta del documento; sin ruta, en `untitled_path(new_dir)` (`new_dir` es el directorio actual en la app y un directorio temporal en los tests). `save_as` escribe en `copy_path(ruta)`, o en `untitled_path(new_dir)` sin ruta. Tras guardar con éxito, la ruta del documento es la escrita, el id guardado es `document.state_id()` y `modified` es falso. Aviso 5 s: `describe_saved`.
- **Un PNG que Pixoko convierte no se sobrescribe.** Si `read_png` dio un informe no vacío, el primer `save` escribe la copia (`copy_path`) en vez del original, y el título dice por qué (`describe_saved`: «guardado como casa-copia.png: el original se convierte al abrir (gris de 4 bits ampliado a 8) y no se sobrescribe»). La ruta pasa a ser la copia (salida de Pixoko), y desde ahí `save` la sobrescribe con normalidad.
- **Modificado = `document.state_id() != id guardado`.** Sin falsos negativos: `modified` falso garantiza contenido igual al guardado; puede haber falsos positivos (dos historias que acaban en la misma imagen). Si la historia descarta el paso guardado por el límite de bytes, `modified` sigue a cierto, que es lo seguro.
- **Cerrar la ventana con cambios: confirmación por repetición.** `close_requested` con `modified()`: el primer cierre se cancela (`ViewportCommand::CancelClose`) y el título avisa (`close_warning`: «hay cambios sin guardar: cierra otra vez en 5 s para descartarlos, o guarda con Ctrl+S»); un segundo cierre dentro de 5 s cierra. Sin cambios cierra a la primera. Misma lógica `Confirm` que al abrir.

## Limitaciones

- **Los chunks auxiliares se pierden al guardar** (texto, gamma, perfil ICC, resolución, fecha…): `write_png` escribe un PNG de 8 bits sin ellos y `read_png` no los cuenta en el informe, así que no hay aviso. Sobrescribir un PNG que los tenga los elimina; la copia por conversión solo protege de lo que el informe detecta.
- Tampoco se conservan el entrelazado, los 16 bits ni las profundidades bajas (sí lo avisa el informe).
- Un corte de luz puede dejar un `.nombre.pixoko-tmp-*` suelto. El directorio no se sincroniza tras el `rename`.
- Los topes de fichero y de píxeles son provisionales (unidad 14).

## Fuera de alcance

Diálogos de archivo y `Ctrl`+`O` (unidad 11, con `rfd`), otras ventanas o pestañas, formatos distintos de PNG, conservar chunks auxiliares, limpiar temporales viejos, `fsync` del directorio, lienzos mayores que la GPU (unidad 14), guardado automático, ficheros recientes.

## Casos límite

`tmp` es un directorio temporal propio de cada test (`std::env::temp_dir()` con el pid y un contador, borrado al terminar; sin crates nuevas). Corpus: `golden/png/*.png` (tres indexados y un RGBA, de 16×16); gris y gris con alfa se construyen con `write_png`. `T` es un título cualquiera.

| # | Entrada | Resultado |
| --- | --- | --- |
| 1 | `parse_args`: `[]`, `["a.png"]`, `["a", "b"]`; `pick_dropped`: `[None, Some(a), Some(b)]` y `[None]` | `Ok(None)`, `Ok(Some("a.png"))`, `Err(uso)`; `Some((a, 1))` y `None` |
| 2 | `Session::open` de cada fichero del corpus (`max_side` 16384) | `Ok(texto)`; imagen = `read_png(bytes).image`; ruta absoluta; `modified` falso; `can_undo` y `can_redo` falsos; `state_id` 0; el texto lleva nombre, tamaño y modo |
| 3 | `load_png`: ruta inexistente; directorio; fichero vacío; bytes basura; cabecera válida de 8×8 sin datos; fichero mayor que `max_bytes` (límite de 100 bytes) | `Read`, `NotAFile`, `Png`, `Png`, `Png`, `TooBigFile` |
| 4 | **Antes de decodificar:** fichero con solo firma + `IHDR` que declara 20000×1 con `max_side` 16384; 8193×8192 con 16384; 2049×1 con 2048; 8192×8192 (justo el tope) con 16384 | `TooLargeToShow{max_side: 16384}`; `TooManyPixels`; `TooLargeToShow{max_side: 2048}`; **`Png(..)`** (no hay datos: pasó `check_fits` y llegó a `read_png`). Ninguno de los tres primeros es `Png`: no se decodificó |
| 5 | `check_fits(w, h, 16384)`: `(8192, 8192)`, `(16384, 4096)`, `(8193, 8192)`, `(16385, 1)`, `(1, 16385)`, `(u32::MAX, u32::MAX)` | `Ok`, `Ok`, `TooManyPixels`, `TooLargeToShow`, `TooLargeToShow`, `TooLargeToShow` (sin desbordar) |
| 6 | Tras un `apply` sin guardar, `open` de un fichero que no existe; y `open(.., discard = true)` del corpus | `Err(Unsaved)` (no `Read`: no se lee), sesión intacta; abre y descarta: historia vacía, ruta nueva, `modified` falso |
| 7 | Sesión con un fichero abierto, `apply`, `save`, y `open` de otro | tras `save`, `open` sin `discard` funciona; historia vacía, ruta y `modified` del nuevo; `can_undo` falso |
| 8 | `adjust_slot`: indexado de 3 colores con ranura 2, 5, 9; RGBA con 7; gris con 9 | `2`, `1`, `1`; `7`; `9` |
| 9 | `describe_open`: informe vacío; `[DepthExpanded{4}, ColorKeyToAlpha]`; `[Depth16Reduced]`; RGBA, gris, gris con alfa, indexado | `abierto casa.png · 16×16 · RGBA`; `… · convertido: gris de 4 bits ampliado a 8, color clave convertido en alfa · Ctrl+S guardará una copia`; `… 16 bits reducido a 8 …`; `RGBA`, `gris`, `gris con alfa`, `indexado` |
| 10 | Indicador con `state_id`: `apply` ⇒ `modified`; `save`; `apply, undo` (vuelve al guardado); `redo`; `save, undo, apply nuevo, undo, redo` | cierto; falso; falso; cierto; **cierto en cada paso del último** (el estado guardado ya no es alcanzable) |
| 11 | Ida y vuelta, cuatro modos: abrir (corpus indexado y RGBA; gris y gris con alfa sintéticos), `SetPixels` de 3 píxeles, `save`, abrir en otra sesión | imagen idéntica a la editada; `modified` falso tras guardar; ningún `.pixoko-tmp` en el directorio; `undo` + `save` deja la imagen original exacta |
| 12 | Escritura atómica, éxito sobre un destino nuevo; y destino que es un directorio no vacío (el `rename` falla) | contenido completo y el directorio solo contiene el destino; `Err(Write)`, directorio intacto y ningún temporal |
| 13 | `write_atomic` sobre un fichero existente de 1 KiB con permisos `0o640` (Unix) | contenido nuevo completo; permisos `0o640`; ningún temporal |
| 14 | Destino fichero de solo lectura; directorio de solo lectura sin destino; en los dos, después, un `save` correcto en otro sitio | `Err(ReadOnly)`, contenido intacto; `Err(Write)`, nada creado; `modified` sigue a cierto y no hay temporales (se salta como `root`) |
| 15 | Enlace simbólico `l.png → real.png` (solo Unix) y `save` | `real.png` con la imagen nueva; `l.png` sigue siendo un enlace |
| 16 | `save` en una sesión de muestra sin ruta, y otra sesión en el mismo `tmp`; después `save_as` de un `x.png` dos veces | `sin-titulo.png`, `sin-titulo-2.png` (nunca pisa); `x-copia.png`, `x-copia-2.png`; `x.png` queda byte a byte igual; la ruta es la última copia; `modified` falso; `copy_reason` es `None` |
| 17 | Abrir un RGB de 1×1 (incrustado en el test) y `save`; un segundo `save` | `Ok` con `x-copia.png` y `copy_reason` con el informe; el original no cambia; la ruta es la copia; el segundo `save` la sobrescribe y `copy_reason` es `None`; `describe_saved` explica el porqué la primera vez |
| 18 | `Confirm`: `request(Open(a), 0)`; `request(Open(a), 4.9)`; luego `request(Open(a), 5.0)` | `Armed`; `Confirmed`; `Armed` (se vació al confirmar) |
| 19 | `Confirm`: `Open(a)` a 0 y `Open(a)` a 5,0; `Open(a)` y `Open(b)`; `Open(a)`, `Close`, `Open(a)`; `Close` a 0 y `Close` a 4,9; `clear` entre dos; tiempo hacia atrás (`Open(a)` a 10 y `Open(a)` a 9) | `Armed`, `Armed`; `Armed`, `Armed`; `Armed`, `Armed`, `Armed`; `Armed`, `Confirmed`; `Armed`, `Armed`; `Armed`, `Armed` |
| 20 | `open_warning("casa.png")`, `close_warning()`, `describe_saved` con `Saved{path: "/d/casa.png", copy_reason: None}` y con `Saved{path: "/d/casa-copia.png", copy_reason: Some("gris de 4 bits ampliado a 8")}` | `hay cambios sin guardar: suelta casa.png otra vez en 5 s para descartarlos, o guarda con Ctrl+S`; `hay cambios sin guardar: cierra otra vez en 5 s para descartarlos, o guarda con Ctrl+S`; `guardado casa.png`; `guardado como casa-copia.png: el original se convierte al abrir (gris de 4 bits ampliado a 8) y no se sobrescribe` |
| 21 | `file_label`: `/a/b/casa.png`, `casa.png`, `/`, `a/b/` ; `title_with_file`: `(None, false, T)`, `(Some("a.png"), false, T)`, `(Some("a.png"), true, T)`, `(None, true, T)` | `casa.png`, `casa.png`, `/`, `b`; `T`, `a.png — T`, `a.png* — T`, `sin título* — T` |
| 22 | `file_action_for_key`: `S` con `COMMAND`; con `COMMAND`+`SHIFT`; con `command`+`ctrl` a la vez (Linux); con `mac_cmd`+`command` (macOS); `S` sola, con `SHIFT` solo, con `ALT`, con `COMMAND`+`ALT`, con `ctrl` solo (macOS: `command` falso); `P`, `Z`, `O` con `COMMAND`; y `action_for_key` con `S` | `Save`; `SaveAs`; `Save`; `Save`; `None` en el resto |
| 23 | `png_dimensions` de `write_png` en los cuatro modos y del corpus; solo firma + `IHDR` (sin CRC válido) de 20000×30000; `IHDR` con 2³¹−1 × 1 | `(w, h)`; `(20000, 30000)` y `(2147483647, 1)`, sin reservar memoria |
| 24 | `png_dimensions`: vacío; 7 bytes; firma incorrecta; firma sola; `IHDR` truncado a mitad; primer chunk `IDAT`; `IHDR` de longitud 12; ancho 0; alto 0; ancho 2³¹ | `Err(Decode)` en las diez |
| 25 | `state_id` en `Document`: nuevo; clon; tras `apply` con cambio, otro `apply`; `apply` sin cambios; `apply` con error (píxel fuera) | `0`; igual al original; ids distintos de 0 y entre sí; sin cambio; sin cambio |
| 26 | `state_id`: `apply` A y B, `undo`, `undo`, `redo`, `redo`; luego `undo`, `apply` C | ids 0 → a → b; deshacer devuelve b→a→0; rehacer a→b; con C: id nuevo distinto de a, b y 0 (nunca reutilizado); `redo` falla y no cambia el id |
| 27 | `state_id` con `set_history_limit(0)` tras 5 pasos, y `NothingToUndo` / `NothingToRedo` | el límite no cambia el id actual; los `undo` restantes dan los ids del historial retenido (no 0 al llegar al más antiguo conservado); los errores no cambian el id |

## Propiedades (proptest)

1. **Ida y vuelta por disco.** Para cualquier imagen de 1 a 16 de lado en los cuatro modos (paletas de 1 a 256 colores): `save` y abrir otra sesión da la imagen igual, sin informe de conversiones y sin temporales.
2. **Indicador contra un modelo.** Con secuencias aleatorias de edición, `undo`, `redo` y `save` sobre una `Session` y una copia de la imagen guardada: tras `save`, `modified` falso; tras una edición con cambio, cierto; y **si `modified` es falso, la imagen es igual a la guardada** (sin falsos negativos).
3. **`state_id` (en `px-core`).** Con secuencias aleatorias de `apply`, `undo`, `redo` y `set_history_limit`: un id nuevo solo lo da un `apply` con cambios y nunca coincide con uno ya dado; `undo` seguido de `redo` (y al revés) devuelve el mismo id; dos instantes con el mismo id tienen la misma imagen; ni los errores ni `trim` cambian el id.
4. **Un fallo no destruye.** Para cualquier contenido previo del destino y cualquier fallo inyectado (destino directorio, solo lectura): el contenido anterior queda byte a byte, `modified` no cambia y no queda ningún temporal.
5. **Abrir no entra en pánico.** `load_png` con bytes arbitrarios y con PNG válidos con bytes mutados: `Ok` o `Err`, sin pánico; con `Err`, `Session::open` deja la sesión igual (documento, ruta, `modified`).
6. **`png_dimensions` y `check_fits`.** `png_dimensions` nunca entra en pánico con bytes arbitrarios y, para cualquier `write_png(imagen)`, da las dimensiones de la imagen. `check_fits(w, h, m)` es `Ok` si y solo si ambos lados ≤ `m` y `w × h ≤ 2^26` (en `u64`), para cualquier `(u32, u32, usize)`.
7. **Nombres de copia.** Para cualquier nombre (espacios, Unicode, sin extensión, `.PNG`, empezando por punto) y con 0 a 5 copias ya existentes: `copy_path` y `untitled_path` están en el directorio esperado, terminan en `.png`, no existen todavía y no coinciden con el original.
8. **`Confirm`.** Para cualquier secuencia de peticiones con tiempos crecientes: `Confirmed` si y solo si la petición anterior fue `Armed`, es igual a esta y `armado ≤ ahora < armado + 5`; tras `Confirmed`, la siguiente es `Armed`. Con tiempos arbitrarios (incluso hacia atrás), sin pánico.

## Dónde van las pruebas

Sin tocar los tests existentes:

- `px-core/tests/state_id.rs` (filas 25 a 27, propiedad 3).
- `px-formats/tests/png_dimensions.rs` (filas 23 y 24, y la primera parte de la propiedad 6).
- `px-ui/tests/file_keys.rs` (fila 22).
- `px-app/src/files/tests.rs` (filas 1, 3, 4, 5, 12 a 15, 21 en `file_label`, propiedades 4 a 7), `px-app/src/session/tests.rs` (filas 2, 6 a 11, 16 y 17, propiedades 1, 2 y 5 con `Session`), `px-app/src/confirm/tests.rs` (filas 18 a 20, propiedad 8), y un módulo nuevo `file_title_tests` en `px-app/src/title.rs` (la parte `title_with_file` de la fila 21).
- Las filas 14 y 15 se saltan donde no apliquen (`root`, no Unix). El cableado de `main.rs` (argumento, soltar, atajos, cierre, título) no se prueba sin ventana: se comprueba a mano en `--release` (ver el cierre de la unidad).
- `Cargo.toml` de `px-app`: `px-formats = { path = "../px-formats" }` y `[dev-dependencies] proptest = "1.12.0"`. `docs/DEPENDENCIES.md` anota que `png` (vía `px-formats`) entra ahora también en `px-app` y que `proptest` es dev-dependency de `px-app`.
