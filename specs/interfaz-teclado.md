# Interfaz · Unidad I.1: el teclado como tabla de datos

Estado: **aprobada** (2026-10-20; decisiones 1 a 9 como están, con los cambios del visto bueno del final de la página, que **prevalecen sobre cualquier otra frase de ella**: commit propio para la adaptación de las pruebas con la lista de cada aserción antigua y nueva; pruebas de tecla muerta, IME y `Option`+letra; `Alt`+letra no es la letra; `Re Pág` y `Av Pág` en la tabla; las tres funciones antiguas como vistas con prueba y `TODO`). Cambios: `px-ui` y `px-app`. **No se toca `px-core` ni `px-raster`.** Sin dependencias nuevas ni `unsafe`; `docs/DEPENDENCIES.md` no cambia. Sin carga ni guardado en disco (hito 8).

**Actualización de la unidad C1 (2026-10-22):** `p` ya no está sin asignar: alterna el editor de color (`AppAction::ToggleColorEditor`, fila 36 de la tabla, entrada `Ver → Editor de color`, editable en `Ayuda → Atajos de teclado…`). Siguen sin asignar `l i e` y `P L I E`. ([`ui-composition.md`](ui-composition.md))

Referencias: esquema de teclas y reglas en `docs/DISENO-INTERFAZ.md` («Atajos de teclado»; «Dos colores y convenciones del ratón» para `K`, `{`, `}` y `Espacio`, que son de I.2). Capa de acciones, bloqueos, menú y columna: [`hito-2-menu-herramientas.md`](hito-2-menu-herramientas.md). Teclas `I` y `E`, `select_tool`, `gate_keys`: [`hito-2-cuentagotas-borrador.md`](hito-2-cuentagotas-borrador.md). Atajos `Ctrl+Z` y cifras 1 a 9: [`hito-2-lapiz-deshacer.md`](hito-2-lapiz-deshacer.md). `ColorChoice`, `Session.choice` y el panel: [`hito-2-panel-paletas.md`](hito-2-panel-paletas.md).

## Objetivo

Una **tabla de datos** (`Keymap`) que asigna teclas a `AppAction`, probada sin ventana, y el esquema por defecto de la sección 4 del diseño **limitado a lo que hoy existe**. La tabla es la única fuente: la traducción de teclas, el atajo del menú y la letra de los botones salen de ella.

## Lo comprobado (y lo que no)

- **egui 0.37 no tiene `Key` para `<` ni `>`** (`egui-0.37.0/src/data/key.rs`). `egui-winit` 0.37 manda `Event::Key { key }` con la tecla **lógica** o, si esa no existe en egui, con la **física**; el carácter que produce la pulsación llega justo después como `Event::Text`, y **no llega con `Ctrl`/`Cmd`** (`lib.rs`, `on_keyboard_input`). Por eso `<` en un teclado español (tecla propia, física `IntlBackslash`) y `<` en uno inglés (`Mayús`+`,`, física `Comma`) solo son **la misma cosa** si se mira el texto.
- **AltGr.** En Windows, `winit` 0.30.13 quita el `Ctrl`+`Alt` de AltGr de los modificadores cuando la distribución tiene AltGr (`platform_impl/windows/keyboard_layout.rs`, `filter_out_altgr`): **leído, no probado**. En Linux AltGr no es `Alt` en `winit`: **no comprobado**. En macOS, con la distribución española `[` y `]` se escriben con `Option` (según el rótulo, no verificado con teclado), que egui entrega como `alt`.
- **Bloq Mayús.** Con él activo, la `d` llega como `Text("D")` **sin** `shift`. Por eso las letras se comparan en minúscula y con `Mayús` aparte.
- **Hoy:** las teclas viven repartidas en cuatro funciones (`tool_action_for_key`, `action_for_key`, `file_action_for_key`, `action_for_shortcut`), en `menu::shortcut_for` (el texto del menú) y en `toolbar::TOOL_BUTTONS` (la letra del botón, a mano): tres sitios que hay que mantener iguales. Las condiciones de bloqueo ya son de la acción (`is_blocked`), no de la tecla, y `gate_keys` ya quita **todos** los eventos (también `Text`) con un campo enfocado.
- **Medido (2026-10-20, copia en `~/.cache/pixoko-scratch`, fuera de `/tmp` y del repositorio, borrada al cerrar):** cambiar solo la asignación (`d v f ,` y `u`; sin `P L I E` ni cifras) en las funciones actuales hace fallar **24 pruebas en 10 ficheros**; añadir las acciones nuevas rompe **1 compilación** (un `match` exhaustivo). Lista en «Pruebas anteriores que cambian».
- **No probado:** ningún teclado físico (español, inglés, alemán, macOS), ni Wayland frente a X11. Lo cubre la prueba manual.

## Decisiones (⚠ = añade algo a lo que pediste)

1. **Forma de la tabla.** `Keymap` es un `Vec<Binding>` de datos puros, **sin egui** (módulo `keymap`):
   - `KeyId { Char(char), Home, PageUp, PageDown }` (`Escape` y `Espacio` no son asignables: son del lienzo; `Inicio`, `Re Pág` y `Av Pág` son teclas con nombre). Las letras van en **minúscula**.
   - `Binding { key: KeyId, command: bool, shift: bool, action: AppAction, active: Platforms, shown: Platforms }` con `Platforms { All, Only(Platform), Except(Platform) }`. `active` dice dónde funciona la fila y `shown` dónde es la que se muestra (`Rehacer`: `Ctrl+Y` se muestra salvo en macOS y `Ctrl+Mayús+Z` solo en macOS; las dos funcionan en todas).
   - **Qué se compara** (`Keymap::action_for(&Press, Platform)`, una sola función): `command` exacto; en letras, `shift` exacto y `Alt` apagado (**`Alt` más una letra no es esa letra y no hace nada**: en macOS `Option`+`d` escribe `∂`, que no casa con ninguna fila, y en Linux y Windows `Alt`+`d` queda sin acción); en cifras `Alt` apagado; en símbolos y en teclas con nombre (`Inicio`, `Re Pág`, `Av Pág`), `shift` y `Alt` **no cuentan** (sin `command`) (en un símbolo el carácter ya dice si hubo `Mayús`: `<` frente a `,`; y `[` en un teclado español sale con AltGr o `Option`). Con `command`, `shift` es siempre exacto y `Alt` anula la fila, sea la tecla que sea (`Ctrl+Alt+Z` no es `Ctrl+Z`). Un `Ctrl` que no es `command` (macOS) anula letras y cifras, como `Alt`, y no cuenta en símbolos ni en teclas con nombre. La primera fila que casa gana.
   - **Condiciones: no van en la tabla.** Un gesto en curso, un diálogo, el panel modal, una quita pendiente y el campo de texto enfocado ya los resuelven `is_blocked` (por acción y origen) y `gate_keys`; ponerlos en cada fila haría que el menú y las teclas se bloquearan distinto. Así «ignorada con un campo enfocado, un diálogo o un gesto» sigue siendo una sola regla. **Con un diálogo o el panel modal abierto la tabla entera está bloqueada y `Inicio`, `Fin`, `Re Pág` y `Av Pág` son del propio diálogo** (su desplazamiento por teclas no cambia).
   - **Menú, botones y rótulos leen la tabla:** `Keymap::shown(action, platform) -> Option<Chord>` (la primera fila `shown` en esa plataforma) y `Chord` es lo que antes era `menu::Shortcut` (`pub use`, mismo nombre, con `key: KeyId`). Desaparecen `Button.key` y el texto aparte de `shortcut_for`. `shortcut_for`, `shortcut_text`, `show_menu`, `show_menu_with_background` y `show_toolbar` **conservan su firma** y usan `Keymap::default_scheme()`; las variantes `*_in` y `show_*_with(…, &Keymap, Platform)` reciben la tabla y son las que usa la app.
   - **Entrada:** `Press { key: Option<KeyId>, command, shift, alt, other_ctrl, repeat }`, y en `key_events` (usa `egui::Event` como dato, sin `Context`) `presses(&[Event]) -> Vec<Press>` y `press_from_key(Key, Modifiers) -> Press`. Reglas: cada `Event::Key` pulsado da una `Press`; el `Event::Text` de **un solo carácter** que le sigue **inmediatamente** le da el carácter; un `Text` suelto **no da `Press`**, y tampoco los `Event::Ime` (preedición o confirmación de un IME). **Carácter de la `Press`:** con `command`, el de la tecla de egui (con `Ctrl`/`Cmd` no hay texto); sin `command`, **solo el texto**, si es ASCII imprimible. Como respaldo por posición (distribución sin letras latinas: ruso, griego, árabe…), si el texto es **una letra de otra escritura** (alfabética y fuera de Latin-1 y Latín extendido) vale el carácter de la tecla de egui, como hace `egui-winit` con el portapapeles. **No dan carácter** (ni, por tanto, atajo): una **tecla muerta** (sin texto: la del acento del teclado español es la física de `[` o de `'`, y no debe disparar `[`), una letra con acento compuesta tras una tecla muerta (`ú`), un texto de varios caracteres (composición), un símbolo que no es ASCII (`∂` de `Option`+`d` en macOS) y la tecla pulsada durante una composición de IME. Las teclas con nombre (`Inicio`, `Re Pág`, `Av Pág`) valen siempre por la tecla. Las letras se pasan a minúscula. `press_from_key` (sin texto, para pruebas y para `action_for_shortcut`): `Mayús`, `Alt` o un `Ctrl` que no es `command` con una cifra o un símbolo dejan la tecla **sin carácter** (cuál sale depende de la distribución), salvo `+`, `=` y `-`, que valen con cualquier modificador sin `command`, como hasta ahora; las letras llevan `Mayús` y `Alt` aparte, y las teclas con nombre valen con cualquier modificador sin `command`.
2. ⚠ **Teclado: se compara el carácter que produce la tecla, no la tecla física.** Con la tecla física, `<` y `>` necesitarían una fila por distribución (en inglés `Mayús`+`,` y `Mayús`+`.`, en español y alemán la tecla ISO propia) y egui no dice qué distribución hay; `[` y `]` serían la tecla que en el teclado español lleva otro rótulo; y `d`/`D`/`K` (la sección 4 distingue mayúsculas) pedirían además el estado de `Mayús`. Con el carácter hay **una fila por tecla** y vale lo que el usuario lee en el teclado, también en Dvorak y AZERTY. Coste: un emparejado de eventos (arriba) y el respaldo a la posición solo para distribuciones sin letras latinas.

   | Distribución | `,` | `[` `]` | `<` `>` |
   | --- | --- | --- | --- |
   | Español (Linux, Windows) | tecla coma | AltGr + la tecla rotulada | tecla ISO propia; `>` con `Mayús` |
   | Inglés (EE. UU.) | tecla coma | teclas propias | `Mayús`+`,` y `Mayús`+`.` |
   | Alemán | tecla coma | AltGr+8 y AltGr+9 | tecla ISO propia; `>` con `Mayús` |
   | macOS | tecla coma | español: `Option` + la tecla rotulada; inglés: teclas propias | español ISO: tecla propia; inglés: `Mayús`+`,` y `.` |

   Las posiciones del español y de macOS salen del rótulo de la tecla, que es justo el criterio: no las he verificado con un teclado.
   **Segundo atajo para `[` y `]`** (con AltGr no son cómodas en el teclado español): `Re Pág` (color anterior) y `Av Pág` (siguiente), que no dependen de la distribución ni de modificadores (en un portátil suelen pedir `Fn`). **Están en la tabla por defecto** como filas aparte; el menú y los rótulos siguen mostrando `[` y `]`.
3. **Conflictos.** `Keymap::conflicts() -> Vec<Conflict>` (el validador que usará el hito 8): dos filas chocan si tienen la misma tecla, el mismo `command`, `shift` igual donde cuenta, y `active` solapado; y una fila choca con la lista reservada (`Escape`, `Espacio`, `Ctrl`+`C`/`X`/`V`/`A` y `Ctrl`/`Cmd`+`Q` fuera de Linux). La propiedad 1 exige que la tabla por defecto no tenga ninguno y que **las filas con `command` sean exactamente las nueve modernas** (`Ctrl`+`Z`, `Y`, `Mayús+Z`, `O`, `N`, `S`, `Mayús+S`, `1`, y `Q` en Linux): ninguna letra suelta puede colarse con `Ctrl`.
4. **Cómo se muestra la letra.** Como se teclea, no en mayúscula: `d`, `v`, `f`, `,`, `<`, `>`, porque la sección 4 distingue `d` de `D` y `b` de `B`. Con `command` la letra va en mayúscula, como los menús de siempre (`Ctrl+Z`, `Ctrl+Mayús+S`, `Cmd+Mayús+Z`). `Re Pág` y `Av Pág` no se muestran en ningún sitio. Los botones de herramienta enseñan `d v f ,` y el Borrador **no enseña nada** (no tiene fila). El menú `Ver` enseña `>` y `<` (las filas `+`, `=` y `-` siguen funcionando pero no se muestran: desaparecen con los pinceles). `Editar` sigue enseñando `Ctrl+Z`; `u` no se muestra. El ancho de los desplegables no cambia (el rótulo más largo de `Ver` sigue siendo `Ajustar a la ventana` + `Inicio`).
5. **Hito 8.** La tabla se construye con `Keymap::default_scheme()` y se sustituye entera con `Keymap::new(Vec<Binding>)`; `App` guarda su `Keymap` y lo pasa a `key_actions`, al menú y a la columna. `Binding` es `Clone + PartialEq + Debug`, sin tipos de egui, para que el hito 8 le ponga `serde` sin tocarlo. **Hoy no hay carga ni guardado.**
6. **Pruebas anteriores:** lista medida abajo; van en un commit propio. **13 necesitan más que cambiar teclas o literales**; me paro ahí (ver «Decisiones que necesito que confirmes», 1).
7. ⚠ **Acciones nuevas** (hacen falta para lo que pides): `AppAction::ColorNext` y `ColorPrev` (`]` y `[`): con `ColorChoice` y `cell_count`/`cell_of_choice`/`choice_for_cell`, en indexado la entrada y en los demás modos la ranura; **dan la vuelta** (de la última a la primera y al revés) y muestran la fila elegida (`set_choice_revealing`, como el cuentagotas). Se bloquean como `SelectColor` (gesto, campo de texto, quita pendiente). `AppAction::CenterOnCursor` (`n`) y `ViewOp::CenterOnCursor`: el píxel de la imagen bajo el cursor pasa al centro del lienzo, mismo zoom, y `clamp`; sin cursor sobre el lienzo no hace nada; **no tiene entrada de menú** (necesita el cursor). Es una acción de vista: nueva `AppAction::is_view()` (zoom + centrar) y la regla 4 de `is_blocked` pasa de `is_zoom` a `is_view` (`is_zoom` no cambia). `>` es **acercar** y `<` **alejar**: el diseño no dice cuál es cuál. `shortcuts::Action` gana `ColorNext` y `ColorPrev`. **`AppAction::SelectColor(u8)` y `Action::SelectColor` se quedan** (sin tecla por defecto y asignables en el hito 8; `dispatch` no cambia).
8. **Fuera de esta unidad (aviso):** la decisión 4 del 20 de octubre dice que el cambio de color se permite con el botón pulsado; **no lo hago aquí**: `ColorNext`/`ColorPrev` se bloquean con un gesto como las cifras de hoy, y es de I.2 (con el color de fondo).

## Esquema por defecto (25 filas; 16 sin `command`, 9 con él)

| Acción | Tecla | `command` | `shift` | `active` | `shown` |
| --- | --- | --- | --- | --- | --- |
| `SelectTool(Pencil)` | `d` | no | no | todas | todas |
| `SelectTool(Line)` | `v` | no | no | todas | todas |
| `SelectTool(Fill)` | `f` | no | no | todas | todas |
| `SelectTool(Dropper)` | `,` | no | no | todas | todas |
| `Undo` | `Z` | sí | no | todas | todas |
| `Undo` | `u` | no | no | todas | ninguna |
| `Redo` | `Y` | sí | no | todas | salvo macOS |
| `Redo` | `Z` | sí | sí | todas | solo macOS |
| `ColorNext` / `ColorPrev` | `]` / `[` | no | no | todas | todas |
| `ColorNext` / `ColorPrev` | `Av Pág` / `Re Pág` | no | no | todas | ninguna |
| `ZoomIn` | `>`, `+`, `=` | no | no | todas | solo `>` |
| `ZoomOut` | `<`, `-` | no | no | todas | solo `<` |
| `ZoomFit` | `Inicio` | no | no | todas | todas |
| `ZoomActual` | `1` | sí | no | todas | todas |
| `CenterOnCursor` | `n` | no | no | todas | todas |
| `Open`, `Save`, `SaveAs`, `NewImage` | `O`, `S`, `S`, `N` | sí | `SaveAs` sí | todas | todas |
| `Quit` | `Q` | sí | no | solo Linux | solo Linux |

Sin fila (sin asignar): el Borrador; las cifras 1 a 9; `p l i e` y `P L I E`; `s q r c w b t g m a`, `K`, `D`, `F1` a `F8`, `Tab`; `.`, `/`, `{`, `}`, `Fin`, `Escape`, `Espacio`.

## Piezas (firmas)

```rust
// px-ui/src/keymap.rs (nuevo, sin egui)
pub enum KeyId { Char(char), Home, PageUp, PageDown }
pub enum Platforms { All, Only(Platform), Except(Platform) }
pub struct Binding { pub key: KeyId, pub command: bool, pub shift: bool,
                     pub action: AppAction, pub active: Platforms, pub shown: Platforms }
pub struct Chord { pub key: KeyId, pub command: bool, pub shift: bool }       // = menu::Shortcut
pub struct Press { pub key: Option<KeyId>, pub command: bool, pub shift: bool,
                   pub alt: bool, pub other_ctrl: bool, pub repeat: bool }
pub enum Conflict { Duplicate(usize, usize), Reserved(usize) }
pub struct Keymap { /* Vec<Binding> */ }
impl Keymap {
    pub fn default_scheme() -> Keymap;                    // las 25 filas
    pub fn new(rows: Vec<Binding>) -> Keymap;             // sustitución entera (hito 8)
    pub fn rows(&self) -> &[Binding];
    pub fn action_for(&self, press: &Press, platform: Platform) -> Option<AppAction>;
    pub fn shown(&self, action: AppAction, platform: Platform) -> Option<Chord>;
    pub fn shown_text(&self, action: AppAction, platform: Platform) -> Option<String>;
    pub fn conflicts(&self) -> Vec<Conflict>;
}
// px-ui/src/key_events.rs (nuevo; egui::Event como dato; presses ignora Text suelto y Event::Ime)
pub fn presses(events: &[egui::Event]) -> Vec<Press>;
pub fn press_from_key(key: egui::Key, modifiers: egui::Modifiers) -> Press;
// px-ui/src/actions.rs: AppAction::{ColorNext, ColorPrev, CenterOnCursor}, is_view(); action_for_shortcut
//   conserva su firma y pasa por la tabla; tool_action_for_key, action_for_key y file_action_for_key
//   conservan la firma como vistas de la tabla (misma entrada, los tres resultados de hoy), con un
//   `TODO(hito 8)` para eliminarlas cuando los llamadores migren a `Keymap`.
// px-ui/src/view.rs: ViewOp::CenterOnCursor; View::center_on(&mut self, point: (i64, i64), viewport: Size, image: Size)
// Derives: KeyId, Platforms, Chord, Press y Conflict `Clone, Copy (salvo Conflict), Debug, PartialEq, Eq`;
//   Binding y Keymap `Clone, Debug, PartialEq`. `Press` tiene todos sus campos públicos.
// px-ui/src/menu.rs: shortcut_in(&Keymap, AppAction, Platform) -> Option<Shortcut>;
//   show_menu_in(ui, &UiState, Platform, CanvasBackground, &Keymap) -> Option<AppAction>.
// px-ui/src/toolbar.rs: show_toolbar_with(ui, &UiState, zoom_label: &str, &Keymap, Platform) -> Option<AppAction>.
//   Button pierde `key`; la letra de TOOL_BUTTONS sale de Keymap::shown_text.
// Canvas::interact resuelve ViewOp::CenterOnCursor con `ctx.input(|i| i.pointer.latest_pos())` si cae
//   dentro del lienzo y no hay otra capa encima; View::apply_op no la conoce (la deja como está).
// px-app: Session::cycle_choice(&mut self, delta: i8); dispatch: ColorNext/ColorPrev y View(CenterOnCursor);
//   App { keymap }, key_actions con presses(); Canvas resuelve CenterOnCursor con el puntero en interact.
```

## Casos (sin ventana; la 6, la 8 y la 11 usan el banco de egui)

`P(..)` = una `Press`. «Las tres plataformas» = `Linux`, `Windows`, `Mac`.

| # | Entrada | Esperado |
| --- | --- | --- |
| 1 | `Keymap::default_scheme()` entera, por plataforma | Las 25 filas de arriba (acción, tecla, `command`, `shift`, `active`, `shown`) en ese orden; 25 activas en Linux y 24 en Windows y macOS; `conflicts()` vacío |
| 2 | **Asignadas y sin asignar.** `P` de `d v f , u n ] [ < > + = -`, `Inicio`, `Re Pág` y `Av Pág`; las mismas con `shift`, con `alt`, con `command` y con `other_ctrl`; `P` de `p l i e P L I E`, de las cifras 1 a 9, de `Fin`, de la lista reservada y de `K D` | `SelectTool(Pencil/Line/Fill/Dropper)`, `Undo`, `CenterOnCursor`, `ColorNext/Prev`, `ZoomOut/In`, `ZoomIn`, `ZoomIn`, `ZoomOut`, `ZoomFit`, `ColorPrev` (`Re Pág`) y `ColorNext` (`Av Pág`). En letras (`d v f u n`), `shift`, `alt`, `command` y `other_ctrl` dan `None` (salvo `Ctrl`+`n`, que es Nueva imagen); en `, ] [ < > + = -`, `Inicio`, `Re Pág` y `Av Pág`, `shift` y `alt` **no cambian** el resultado, `command` da `None` y `other_ctrl` tampoco lo cambia (solo anula letras y cifras, decisión 1). Todo lo sin asignar da `None` con cualquier modificador en las tres plataformas (**sin convivencia**: `P L I E` y las cifras ya no hacen nada) |
| 3 | **Modernos intactos.** `Ctrl+Z`, `Ctrl+Y`, `Ctrl+Mayús+Z`, `Ctrl+O`, `Ctrl+N`, `Ctrl+S`, `Ctrl+Mayús+S`, `Ctrl+1`, `Ctrl+Q`, en las tres plataformas, y sus variantes con `alt`, con `shift` de más y `Ctrl+d`, `Ctrl+,`, `Ctrl+]` | `Undo`, `Redo`, `Redo`, `Open`, `NewImage`, `Save`, `SaveAs`, `ZoomActual`, `Quit` **solo en Linux** (`None` en Windows y macOS con cualquier modificador); con `alt`, con un `shift` que no es de la fila y las tres últimas: `None`. Las respuestas son las que daban las funciones de antes de la unidad (oráculo escrito a mano en la prueba) |
| 4 | **Distribuciones.** Secuencias de eventos (`Key` + `Text`) a `presses` y `Keymap::action_for`: español (AltGr+`[`, tecla ISO `<`, `Mayús`+ISO `>`), inglés (`Mayús`+`,` con `Text("<")`, `Mayús`+`.`), alemán (AltGr+8 → `[`), macOS español (`Option`+`[`, con `alt`), Dvorak (la `d` en otra posición física), ruso (`Key::D` con `Text("в")`), Bloq Mayús (`Text("D")` sin `shift`), AZERTY (`Ctrl` + la tecla de la cifra 1, cuyo carácter es `&`) | `[`: `ColorPrev`; `<`: `ZoomOut`; `>`: `ZoomIn`; Dvorak, ruso y Bloq Mayús: `SelectTool(Pencil)`; AZERTY `Ctrl`+tecla de la cifra 1: `ZoomActual`; `Option`+letra en macOS (`Text("∂")` con `alt`): `None`. **Nada** activa un atajo: la tecla muerta del acento del teclado español (`Key::OpenBracket`, sin texto y sin modificadores), la vocal compuesta tras ella (`Key::U` con `Text("ú")`), la composición `Key::D` con `Text("´d")`, `Option`+`d` en macOS (`Key::D`, `alt`, `Text("∂")`), una composición de IME (`Event::Ime` de preedición y de confirmación con `d`, y un `Text("d")` suelto) y un `Text` suelto de `,`. Un carácter de otra escritura **sí** vale por posición (ruso) |
| 5 | **Rótulos desde la tabla.** `shown_text` de cada acción en las tres plataformas; `TOOL_BUTTONS`/`ZOOM_BUTTONS` y `MENUS` con `shortcut_for`/`shortcut_text`; `popup_width_u` de cada menú | `d`, `v`, `f`, `,` y **nada** para el Borrador; `Ctrl+Z` (`Cmd+Z`); `Ctrl+Y` y `Cmd+Mayús+Z`; `>`, `<`, `Inicio`, `Ctrl+1`; `Ctrl+Q` solo en Linux; `ColorNext`, `ColorPrev` y `CenterOnCursor` con `]`, `[` y `n`; ningún rótulo pierde un carácter por `replace_missing`; los anchos de los desplegables **no cambian** (`Ver` = 208 u) |
| 6 | **Zoom y centrar.** `ZoomIn`/`ZoomOut` por `>`/`<` y por `+ = -` frente a las de antes; `View::center_on` con imagen 64×48, lienzo 700×500, zooms ×1, ×5 y ×64, el punto en el centro, en una esquina y fuera de la imagen; `Canvas::request_view(CenterOnCursor)` en el banco de egui con y sin puntero sobre el lienzo | Mismo zoom que antes con cualquier fila; el píxel bajo el punto queda en el centro (±1 px) tras `clamp`; fuera de la imagen no rompe nada y `clamp` manda; sin puntero sobre el lienzo, la vista no cambia; con un trazo a medias `n` mueve la vista y el trazo sigue |
| 7 | **Color siguiente y anterior** (`Session::cycle_choice`, por `dispatch`): indexada de 256 con `Entry(255)` y `Entry(0)`; RGBA con `Slot(9)` y `Slot(1)`; Gray y GrayAlpha; sin imagen; `choice` que ya no vale | `]` en la última da la primera y `[` en la primera da la última (dan la vuelta); un paso por pulsación; la fila elegida se muestra (`view.reveal`); el documento, `state_id`, `can_undo` y `can_redo` no cambian; sin imagen, nada; con `choice` inválido, `initial_choice` |
| 8 | **Bloqueos con `Origin::Key`.** `ColorNext`, `ColorPrev` y `CenterOnCursor` con cada `Locks`; en el banco, `gate_keys` con un campo enfocado y las secuencias de la fila 4 | Color: `Blocked` con gesto, campo, diálogo y modal, y con quita pendiente (como `SelectColor`). `n`: no la bloquean gesto ni quita; sí diálogo, modal y, solo con tecla, campo enfocado. `gate_keys` deja sin `Key` **ni `Text`** con el campo enfocado: `d`, `]` y `<` no hacen nada ahí. Con un diálogo de archivo o el panel modal abierto, **todas** las teclas de la tabla (salvo `Ctrl+Q` en Linux con el modal, que pasa como `Quit`, regla 2 de `is_blocked`) (también `Inicio`, `Re Pág` y `Av Pág`) dan `Blocked`, y `Inicio`, `Fin`, `Re Pág` y `Av Pág` siguen desplazando el diálogo (banco con `dialog_view`) |
| 9 | **La misma acción, el mismo efecto.** Menú, botón y tecla para `Undo`, `SelectTool` de las cuatro herramientas con letra, `ZoomIn`, `ZoomOut`, `ColorNext` y `SelectTool(Eraser)` (solo botón) | Las acciones y los `Dispatched` son iguales; el estado final es igual; el aviso del Borrador sale igual desde el botón |
| 10 | **`SelectColor` sin tecla y vistas de la tabla.** `dispatch(SelectColor(3))` directo en indexada de 9 entradas y en RGBA; las cifras 1 a 9 sin modificadores por `action_for_shortcut`; `tool_action_for_key`, `action_for_key`, `file_action_for_key` y `action_for_shortcut` sobre `Key::ALL` × 32 combinaciones × tres plataformas frente a la tabla (`press_from_key`) | El `dispatch` hace lo de antes (`choice` cambia con la rejilla sincronizada); por teclado, **las cifras sin modificadores no hacen nada**; las cuatro funciones dan, para cada entrada, lo mismo que la tabla (donde su tipo de retorno lo permite; se omiten `mac_cmd` sin `command`, que egui no entrega, y `Re Pág`/`Av Pág` con `ctrl` sin `command` en `action_for_key`, que sigue dando `None` por su contrato) |
| 11 | **Tabla sustituible.** `Keymap::new` con letras cambiadas (`d` ↔ `x`), con una tabla vacía y con filas repetidas o reservadas; menú y columna pintados con `show_menu_in` y `show_toolbar_with` | Con la tabla nueva, `x` elige el lápiz y `d` no; el botón enseña `x`; con la vacía no funciona ninguna tecla (menú y botones siguen) y no se muestra atajo; `conflicts()` da `Duplicate` para filas iguales y `Reserved` para `Escape`, `Espacio` y `Ctrl+C`; `default_scheme()` no da ninguno |
| 12 | **Repetición.** `presses` con `repeat`; `ignores_repeat` de las acciones | `u`, `]`, `Av Pág` y `<` repetidos repiten (n pulsaciones, n pasos); `Open`, `Save`, `SaveAs`, `NewImage` y `Quit` mantenidos no repiten (como hoy) |

## Propiedades (proptest; los casos de la spec: 256)

1. **Sin choques y modernos intactos.** Para cada plataforma, la tabla por defecto tiene `conflicts()` vacío, las filas con `command` son **exactamente** las nueve modernas (las dos de `Re Pág` y `Av Pág` no llevan `command`) (y `Quit` solo en Linux), y para cualquier `Press` (carácter ASCII, `Inicio` o `None`, y los modificadores) el resultado es el mismo con las filas en orden inverso (no hay ambigüedad).
2. **Lo que se muestra funciona.** Para cada acción y plataforma con `shown`: su texto no está vacío ni sale de `INTERFACE_CHARSET`, y la `Press` construida desde el `Chord` da **esa acción**; y para toda fila activa, la `Press` de la fila da la acción de la fila (ninguna fila es inalcanzable).
3. **Independiente del orden.** Para cualquier tabla de 0 a 12 filas sacadas de un conjunto pequeño de teclas y acciones: si `conflicts()` está vacío, `action_for` da lo mismo con cualquier permutación de las filas y en las tres plataformas coincide con el oráculo (buscar la fila con las reglas de la decisión 1); si no, gana la primera.
4. **Emparejado de eventos.** Para cualquier secuencia de `Key` (pulsada o soltada) y `Text`: `presses` no entra en pánico; da **una `Press` por cada `Key` pulsada**, en orden (un `Text` suelto no da ninguna); ignora las soltadas; un `Text` suelto o un `Event::Ime` no dan `Press`; un `Text` de varios caracteres, de un símbolo no ASCII o de una letra latina con acento (y una tecla sin texto ni `command`) no dan carácter, y solo una letra de otra escritura cae al de la tecla; y `Text("D")` sin `shift` da la misma `Press` que `Text("d")` sin `shift`.

## Pruebas anteriores que cambian (medido)

Medidas en la copia desechable cambiando solo la asignación en las funciones de hoy. **A. Solo literales** (la tecla del ejemplo o el valor esperado; misma intención): 11.

| Fichero | Pruebas |
| --- | --- |
| `px-ui/tests/pick.rs` (17a) | `fila_1_p_l_f_y_escape_siguen_igual`, `fila_1_i_es_cuentagotas_y_e_es_borrador_sin_modificadores` (la `E` pasa a `None`), `propiedad_3_cada_herramienta_tiene_su_tecla_sin_modificadores`, `propiedad_3_teclas_exhaustiva_key_all_por_las_32_combinaciones` |
| `px-ui/tests/tool.rs` (7) | `fila_1_p_l_f_y_escape_sin_modificadores`, `propiedad_5_teclas_exhaustiva_key_all_por_las_32_combinaciones` |
| `px-ui/tests/shortcuts.rs` (6) | `propiedad_6_todas_las_combinaciones_y_todas_las_teclas` |
| `px-ui/tests/shortcuts_gate.rs` | `fila_17_sin_foco_los_atajos_vuelven_a_verse` (la tecla 3 pasa a `u`) |
| `px-ui/tests/actions.rs` (17) | `fila_3_action_for_shortcut_sobre_key_all_por_32_combinaciones_y_3_plataformas` |
| `px-ui/tests/dialog_lock.rs` y `px-app/src/dispatch/tests.rs` (18, 17) | `fila_6d_las_teclas_de_herramienta_color_zoom_y_fichero_estan_bloqueadas_con_modal` y `dispatch::tests::fila_6d_las_teclas_de_herramienta_color_zoom_y_fichero_dan_blocked_con_modal` (lista de teclas) |

**B. Más que un literal (13): me paro aquí.** Cada una con por qué y qué propongo; ninguna debilita lo que comprobaba:

| Prueba | Por qué no es solo un literal | Adaptación propuesta |
| --- | --- | --- |
| `dispatch::tests::fila_3b_las_nueve_ranuras_de_una_indexada_de_9_entradas_cambian_choice_como_antes` (11b) | comprobaba que 1 a 9 eligen entrada; la intención se invierte | las cifras no cambian `choice`; las mismas nueve entradas se recorren con `ColorNext`/`ColorPrev` |
| `actions::fila_3b_los_digitos_sin_modificadores_dan_select_color_en_las_tres_plataformas` (17) y `shortcuts::fila_12_las_teclas_numericas_eligen_ranura` (6) | idem | `None` en las tres plataformas; `Ctrl+1` sigue dando solo `ZoomActual` |
| `actions::fila_3_cada_tecla_de_herramienta_y_de_ranura_vale_sin_modificadores_en_las_tres_plataformas` (17) | mezcla herramientas (literal) y ranuras (invertida) | herramientas `d v f ,`; la parte de ranuras pasa a `]`/`[` |
| `dropper::fila_8_gate_keys_sin_foco_deja_pasar_i_y_e` y `fila_8_banco_de_egui_con_un_campo_enfocado` (17a) | usan `E` para elegir el borrador por tecla; ya no tiene | la `I` pasa a `,` y el borrador se elige con `ToolAction::Select(Eraser)` directo |
| `dispatch::tests::fila_4_*` (cuatro: aviso del borrador, cinco herramientas, tres `Dispatched`, menú-botón-atajo) (17) | cada una compara botón y tecla de **cinco** herramientas y de `SelectColor(3)`; el borrador y esa acción ya no tienen rama de tecla | el Borrador se compara solo con el botón; `SelectColor(3)` se sustituye por `ColorNext` |
| `toolbar::tool_buttons_son_los_cinco_botones_con_su_letra` y `las_letras_de_los_botones_no_activos_van_en_text_dim` (17) | leen `Button.key`, que desaparece (la letra sale de la tabla) | la letra se toma de `Keymap::shown_text`; el Borrador, sin letra |
| `menu::propiedad_1_el_atajo_de_cada_entrada_traducido_da_su_accion` (17) | usa `button.key` y un `tecla_de(letra)` de `P L F I E` | recorre las filas de la tabla en vez de las letras |

**C. Por lectura, no medidas** (cambian aunque hoy no fallen o dependan del tipo): `Shortcut.key` pasa a `KeyId`, así que los ayudantes `atajo` (`menu.rs`) y `pulsado` (`actions.rs`), el constructor de `fila_9_shortcut_for_de_nueva_imagen_va_y_vuelve…` y las llamadas `action_for_shortcut(shortcut.key, …)` (`menu.rs` y `actions.rs`, 3 sitios) cambian de tipo, no de comprobación; `zoom_buttons_son_los_cuatro_botones_sin_letra` (`b.key`); si el menú enseña `>` y `<`, `menu::fila_1_atajos_en_linux`, `…_en_windows_…`, `…_en_mac_…` y `fila_1_shortcut_for_da_la_tecla…` cambian `+`/`-` por `>`/`<` (literales); `actions::propiedad_3…` gana un brazo del `match` exhaustivo (medido: 1 línea). **No cambian** (pasan hoy y mañana): `puerta`, `color_editor.rs` fila 17 y `shortcuts_gate.rs` (usan `Num3` como evento cualquiera para `gate_keys`; solo quedan comentarios que dicen «la tecla 3»).

Specs que reciben una línea «Actualización de I.1, que prevalece» en el commit de adaptación: `hito-2-menu-herramientas.md`, `hito-2-cuentagotas-borrador.md`, `hito-2-lapiz-deshacer.md` y `hito-2-panel-paletas.md`.

## Dónde van las pruebas y orden de trabajo

`px-ui/tests/keymap.rs` (filas 1 a 3, 5, 10 y 11; propiedades 1 a 3), `px-ui/tests/key_events.rs` (fila 4, 12 en su parte de `presses`; propiedad 4), `px-ui/tests/view_center.rs` (fila 6), un módulo `px-app/src/dispatch/tests.rs` ampliado y `px-app/src/session/color_cycle_tests.rs` (filas 7 a 9). **Orden, un commit por paso:** (1) esta spec con su fila del INDEX; (2) adaptación de las pruebas anteriores (A, B y C; **el mensaje lista cada una con su aserción antigua y la nueva**, explica qué comprobaba, y para las 11 de solo literales pega la salida de un script mecánico guardado fuera del repositorio; las 13 de B se revisan por diff; no tiene que compilar el workspace, falla hasta la lógica); (3) las pruebas nuevas, en rojo, por `test-writer`; (4) lógica de `px-ui` (`keymap`, `key_events`, acciones, `View::center_on`, rótulos); (5) `px-app` (`cycle_choice`, `dispatch`, `key_actions`, `App.keymap`). Cierre: `cargo fmt`, `clippy -D warnings`, `cargo test --workspace --locked`, `cargo deny check` y el `license-auditor` una vez; al terminar se actualiza la columna «Hoy» de `docs/DISENO-INTERFAZ.md` y `PLAN-RESUMEN.md`.

## Prueba manual (la haces tú, sin capturas)

1. **Teclado español** (Linux, el tuyo): `d v f ,` eligen lápiz, línea, relleno y cuentagotas; el título y el botón lo dicen. `P L I E` y las cifras no hacen nada. `u` deshace como `Ctrl+Z`.
2. `]` y `[` (con AltGr) recorren la paleta y dan la vuelta; la fila elegida se ve aunque estuviera fuera. En RGBA recorren las nueve ranuras.
3. `<` y `>` (tecla ISO, `>` con `Mayús`) alejan y acercan; `+`, `=` y `-` siguen; `Inicio` ajusta; `n` centra el punto bajo el cursor y no hace nada con el cursor fuera del lienzo.
4. Con el campo hexadecimal enfocado, ninguna de esas teclas actúa. Con un trazo a medias, `]` no hace nada y `<`, `>` y `n` sí.
5. **Bloq Mayús activado:** `d` sigue eligiendo el lápiz. Menú `Ver`: `Acercar` muestra `>`, `Alejar` muestra `<`; el botón Borrador sin letra (desde el 21 de octubre, con tooltip `Borrador (k)`).
6. **Windows y macOS:** comprobar `[` y `]` (AltGr en Windows con teclado español o alemán; `Option` en macOS) y `<`/`>`. **Si en Windows no funcionan**, es que AltGr llega como `Ctrl`+`Alt`; se anota y se añade la regla en un commit aparte.
7. Con otra distribución (inglesa o Dvorak, si la tienes): `<` es `Mayús`+`,`.

## Fuera de alcance

Cambio de color con el botón pulsado, color de fondo (`{`, `}`), `K`, `Espacio` como mano y todo el ratón (I.2); caja a la derecha y panel de opciones (I.3); iconos (I.4); cargar o guardar la tabla y hacerla editable (hito 8); las teclas de las herramientas del hito 4 (`s q r c e w b t g m a`, `F1` a `F8`, `Tab`); que `+` y `-` pasen a ser el tamaño del pincel; ocultar paneles con `F9`/`F10`.

## Decisiones que necesito que confirmes

1. **Las 13 pruebas de la tabla B** cambian algo más que un literal (arriba, con el motivo y la adaptación de cada una): ¿las adapto como propongo? Si prefieres menos cambio, la alternativa es dejar `Button.key` y `Shortcut.key` como están y que los rótulos se dupliquen otra vez (pierde el punto 1 de tu encargo).
2. **Carácter, no tecla física** (decisión 2), con el respaldo a la posición solo para distribuciones sin letras latinas.
3. **`Alt` cuenta en letras y cifras pero no en símbolos** (AltGr y `Option` los necesitan); `Alt` más una letra no es esa letra y no hace nada.
4. **`[` y `]` por defecto**, con **`Re Pág`/`Av Pág`** como segundo atajo (incluido).
5. **`>` acerca y `<` aleja**; el menú `Ver` enseña `<`/`>` y no `+`/`-` (cuatro pruebas del menú cambian de literal).
6. **`]` y `[` dan la vuelta** en los extremos (alternativa: se paran).
7. **`n` no tiene entrada de menú** y no hace nada sin el cursor sobre el lienzo; **`AppAction::SelectColor` se queda** sin tecla.
8. **Las tres funciones antiguas** (`tool_action_for_key`, `action_for_key`, `file_action_for_key`) se quedan como vistas finas de la tabla para que sus pruebas cambien solo de literales; se podrían retirar en el hito 8.
9. Que el cambio de color con el botón pulsado (decisión 4 del 20 de octubre) **espere a I.2**.

## Visto bueno (2026-10-20)

1. **Adaptación en un commit propio**, antes de las pruebas nuevas, con un mensaje que lista cada prueba con su aserción antigua y la nueva. Cada una afirma lo mismo en su dominio; las que pierden la rama «elegir el Borrador por tecla» conservan su cobertura eligiéndolo por botón (no tiene entrada de menú); la inversión de las cifras 1 a 9 queda con nombre explícito (`…sin_modificadores_no_hacen_nada`). Las 11 de solo literales se comprueban con un script mecánico fuera del repositorio y su salida va en el mensaje; las 13 de B, por revisión de diff. Si alguna necesitara algo distinto de lo propuesto, se para y se avisa.
2. **Carácter, con respaldo por posición** para distribuciones sin letras latinas; pruebas de que **una tecla muerta, una composición de IME y `Option`+letra en macOS (símbolos como `∂`) no activan ningún atajo** (caso 4 y propiedad 4). Al escribirlo, la tecla muerta del teclado español resultó ser la física de `[`: por eso sin `command` solo vale el texto y un `Text` suelto no da `Press`.
3. **`Alt` más una letra no es esa letra y no hace nada** (decisión 1).
4. **`Re Pág` y `Av Pág` en la tabla por defecto** como segundo atajo de color anterior y siguiente; el menú y los rótulos muestran `[` y `]`; 25 filas, 16 sin `command`; las filas con `command` siguen siendo exactamente las nueve modernas. Con un diálogo abierto la tabla está bloqueada y `Inicio`, `Fin`, `Re Pág` y `Av Pág` son del diálogo.
8. **Las tres funciones antiguas** son vistas de la tabla, con una prueba de que dan lo mismo (caso 10) y un `TODO(hito 8)` para eliminarlas cuando los llamadores migren.

## Actualización del 21 de octubre de 2026

**El Borrador tiene la tecla `k`** (minúscula, sin modificadores; decisión del usuario, tarea corta con visto bueno previo). Esto prevalece sobre «sin fila» y «el Borrador sin letra» de esta página.

- **Tabla:** una fila nueva `SelectTool(Eraser)`, `k`, sin `command` ni `shift`, `active` y `shown` en todas las plataformas, justo tras la del cuentagotas: **31 filas, 22 sin `command` y 9 con él** (31 activas en Linux, 30 en Windows y macOS).
- **`K` con `Mayús` sigue siendo `ClearPage`** (I.2a): las letras se comparan en minúscula y `Mayús` va aparte (`shift_counts`), así que las dos filas no chocan y la propiedad de conflictos sigue sin ninguno. Con Bloq Mayús, la `K` que escribe el teclado no lleva `Mayús`: elige el Borrador.
- **Rótulos:** el botón del Borrador muestra `k` en la tabla; su tooltip, `Borrador (k)`, `clic derecho: opciones` y, nueva, `el clic derecho también borra con el fondo` (el clic derecho del lienzo pinta con el color de fondo); el recuadro de opciones dice `tecla k` (`sin tecla` queda solo para una tabla sin fila).
- **Bloqueos:** los de siempre de `SelectTool`: con un campo de texto enfocado, un diálogo abierto o un trazo en curso, la `k` no actúa.
- **Pruebas anteriores adaptadas** (solo literales y cifras): recuentos de filas (30 → 31, 21 → 22 sin `command`, 30/29/29 → 31/30/30 activas), los oráculos de teclas de `actions`, `tool` y `pick` (añaden `Key::K`), `shown_text` y tooltip del Borrador, `TOOL_BUTTONS`, la línea del recuadro y la fila de `k` sin `Mayús` de `two_colors` (de «no hace nada» a «elige el Borrador»). Prueba nueva: `k` elige el Borrador con y sin Bloq Mayús y `K` con `Mayús` borra la página; con campo, diálogo o trazo, no actúa.
- **Prueba manual:** con Bloq Mayús activado, `k` elige el Borrador; `Mayús`+`k` borra la página; el tooltip del Borrador dice `Borrador (k)`; el recuadro de opciones, `tecla k`.
