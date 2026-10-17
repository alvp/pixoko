# Hito 2 · Unidad 2: ventana mínima con `eframe`

Estado: **aprobada** (2026-10-17; decisiones 1 a 7 aceptadas como se recomendaban, con los matices de «Resolución» al final de la página, que prevalecen sobre el texto de las secciones: sin caché de CI, el nivel de zoom es un tipo ampliable, y la fuente queda pendiente). Lo medido sale de una prueba en una copia del repo fuera de él. Los textos de licencia completos están en `specs/hito-2-ventana-licencias.md`.

## Objetivo

Una ventana (`px-app`) que muestra una `Image` de un `Document` con zoom entero y desplazamiento, sin herramientas, y que compila en Linux, Windows y macOS. Es la primera unidad que mete dependencias externas grandes (`eframe`), así que esta página fija versión, features, licencias, CI y qué se prueba sin ventana antes de escribir código.

## 1. `eframe` y `egui`: versión, features y renderizador

Versión: `eframe` y `egui` **0.37.0** (la última de crates.io el 2026-10-17; `winit` 0.30.13), fijadas con `=`. Compila con el toolchain del CI (1.99.1).

Las features por defecto de `eframe` 0.37 incluyen `wgpu`, no `glow`. La configuración mínima propuesta desactiva las de por defecto:

```toml
eframe = { version = "=0.37.0", default-features = false, features = ["glow"] }

[target.'cfg(target_os = "linux")'.dependencies]
eframe = { version = "=0.37.0", default-features = false, features = ["glow", "x11", "wayland"] }
```

`x11` y `wayland` se activan solo en Linux (tabla específica del destino), para cubrir los dos servidores gráficos; sin ninguno de los dos no hay ventana en Linux. No se activan `accesskit` (sin lector de pantalla), `clipboard`, `links`, `system_fonts`, `persistence` ni `default_fonts` (ver §2).

### glow frente a wgpu

Medido en esta máquina (Linux, 12 hilos, perfil dev, `--locked`, `target` vacío) con el mismo `main` mínimo; «defecto» son las features por defecto de `eframe`, solo como referencia.

| | glow | wgpu | defecto |
| --- | --- | --- | --- |
| Crates en el árbol normal, Linux / Windows / macOS | 124 / 91 / 107 | 150 / 131 / 128 | 273 / – / – |
| Crates con licencia revisada (todos los destinos, con dependencias de build) | 235 | 278 | 405 |
| Entradas en `Cargo.lock` (hoy 67) | 318 | 346 | 450 |
| `cargo build -p px-app`, limpio | 11 s (134 crates) | 17 s (157) | 22 s (280) |
| `target/` tras ese build | 1,3 GB | 1,8 GB | 2,5 GB |
| CI de Linux en frío, limitado a 2 hilos: clippy + test + build | 55 s | 73 s | – |
| Release, binario sin símbolos | 7,9 MB | 13,2 MB | – |
| `target/` tras clippy + test + build, comprimido con zstd | 429 MB | 556 MB | – |

Línea base de hoy (sin `eframe`), mismos 2 hilos: clippy 2 s y test 11 s. Un runner de GitHub es más lento que esta máquina; el factor de 3 a 5 es una estimación mía, no medida: lo dará el primer CI real.

Diferencia en el árbol de Linux: `wgpu` añade 31 crates (`wgpu`, `wgpu-core`, `wgpu-hal`, `naga`, `ash`, `gpu-allocator`, `egui-wgpu`, `pollster`…) y `glow` añade 5 (`egui_glow`, `glutin`, `glutin-winit`, `glutin_egl_sys`, `glutin_glx_sys`): 26 de neto. Los identificadores de licencia fuera de `allow` son los mismos en los dos.

**Recomendación: `glow`.** Un editor 2D de píxeles pinta un cuadrilátero con una textura sin suavizar y la interfaz de egui; no usa cómputo ni nada que `wgpu` ofrezca y `glow` no. `glow` da un árbol un 17 % menor en Linux, compila antes y deja un binario y una caché más pequeños. Riesgos que no he podido probar aquí, y que la prueba manual debe mirar: **OpenGL está marcado como obsoleto en macOS** (sigue disponible hasta la 4.1) y en Windows con controlador básico, máquina virtual o escritorio remoto el OpenGL puede ser demasiado antiguo para `glow`, mientras que `wgpu` cae a DX12 o a un rasterizador por software. **Volver a `wgpu` es cambiar una línea** (`"glow"` por `"wgpu"`; `eframe::App` no depende del renderizador); el hito 14 (compositor y GPU) es el punto natural para reconsiderarlo. Esto se aparta del plan, que dice `winit` y `wgpu` (decisión 1 abajo).

### Decoración de ventana en Wayland

Quitar las features por defecto quita también `winit/default`, y con ello `wayland-csd-adwaita`: sin ella, en escritorios Wayland sin decoración del servidor (GNOME) la ventana lleva una decoración básica de reserva. Añadirla exige una dependencia directa de `winit` con esa feature: +9 crates (`sctk-adwaita`, `tiny-skia`, `ab_glyph`, `ttf-parser`…), todas con licencias de `allow`. No la incluyo; si la prueba manual en Wayland lo pide, se añade.

## 2. Licencias nuevas que no están hoy en `allow`

`cargo deny --locked check` sobre la copia da `advisories ok, bans ok, sources ok`; `licenses` **falla, en glow y en wgpu, por un solo crate**, que solo entra con `default_fonts`. Sin esa feature, glow da `licenses ok` con el `deny.toml` actual (ejecutado); en wgpu no lo he ejecutado sin ella, pero el análisis de licencias del árbol solo señala ese mismo crate. Hay avisos `duplicate` (`windows-sys` ×3, `objc2*`, `rustix`, `syn`, `thiserror`…), que no hacen fallar el check, y el aviso de excepción no usada de `libfuzzer-sys`, que ya existe hoy.

**No hay ninguna GPL, LGPL, AGPL ni MPL obligatoria en el árbol.** Sí hay **copyleft como rama alternativa de un `OR`**, que no se usa, y copyleft limitado a ficheros de fuente (tabla A):

### A. Obligatorias: `cargo deny` rechaza el crate con la lista actual

| Crate | Licencia declarada | Cuándo entra | Compatible con MIT |
| --- | --- | --- | --- |
| `epaint_default_fonts` 0.37.0 | `(MIT OR Apache-2.0) AND OFL-1.1 AND Ubuntu-font-1.0` | `default_fonts`, en glow y en wgpu | Sí para el programa, con matiz abajo |

Qué fuentes lleva y bajo qué licencia: Hack (MIT, con Bitstream Vera y DejaVu) y `egui-icons` (MIT) y **Ubuntu-Light (`Ubuntu-font-1.0`)** se compilan con `default_fonts`; **Noto Emoji (`OFL-1.1`)** y `emoji-icon-font` (MIT) solo con `monochrome_emoji_fonts`, que no se activa, aunque el campo `license` del crate declara siempre las tres. `OFL-1.1` y `Ubuntu-font-1.0` son licencias de fuentes con **copyleft limitado a la propia fuente**: una fuente derivada ha de seguir bajo la misma licencia y no se puede vender sola. Según el preámbulo de cada texto, no obligan a licenciar de nuevo el programa que las incrusta ni los documentos creados con ellas. No son GPL, LGPL ni AGPL. Piden conservar los avisos de copyright y el texto de la licencia junto a lo que se distribuya (el archivo de licencias de terceros del plan). Esto es lectura de los textos, no asesoramiento legal.

### B. Solo con la feature `clipboard` (Windows; no entra en la configuración propuesta)

| Crate | Licencia | Compatible con MIT |
| --- | --- | --- |
| `clipboard-win` 5.4.1, `error-code` 3.4.0 | `BSL-1.0` (Boost) | Sí, permisiva |

### C. Ramas alternativas de un `OR`: `cargo deny` ya las acepta por la rama de `allow`

| Crate (versión) | Licencia declarada | Rama que se usa | Rama que no se usa |
| --- | --- | --- | --- |
| `self_cell` 1.3.0 (vía `epaint`) | `Apache-2.0 OR GPL-2.0-only` | Apache-2.0 | **GPL-2.0-only (copyleft)** |
| `r-efi` 5.3.0 y 6.0.0 (vía `getrandom`; solo UEFI) | `MIT OR Apache-2.0 OR LGPL-2.1-or-later` | MIT o Apache-2.0 | **LGPL-2.1-or-later (copyleft)** |
| `byteorder-lite` 0.1.0 (vía `image`), `memchr` 2.8.3 | `Unlicense OR MIT` | MIT | `Unlicense` (dominio público; permisiva) |
| `same-file` 1.0.6, `walkdir` 2.5.0, `winapi-util` 0.1.11 (solo dependencias de build de Android) | `Unlicense OR MIT` o `Unlicense/MIT` | MIT | `Unlicense` |
| `adler2` 2.0.1 (ya está hoy) | `0BSD OR MIT OR Apache-2.0` | MIT o Apache-2.0 | `0BSD` (permisiva) |
| `rustix` 0.38.44 y 1.1.5, `linux-raw-sys` 0.4.15 y 0.12.1, `wasip2`, `wit-bindgen` | `Apache-2.0 WITH LLVM-exception OR Apache-2.0 OR MIT` | Apache-2.0 o MIT | la excepción LLVM (permisiva) |

Los identificadores que hoy no están en `allow` son, pues: `OFL-1.1`, `Ubuntu-font-1.0` y `BSL-1.0` (obligatorios, tablas A y B) y `Unlicense`, `0BSD`, `LGPL-2.1-or-later`, `GPL-2.0-only` y `LLVM-exception` (solo alternativas). Los textos están en el anexo, salvo los de la GPL y la LGPL, que no se usan.

**No se añade nada a `allow`.** Lo que hace falta para que `cargo deny` pase depende de la decisión 2:

- Sin `default_fonts` (recomendado): **ningún cambio en `deny.toml`**.
- Con `default_fonts`: una excepción por crate, como la de `libfuzzer-sys` para NCSA, con comentario de motivo y fecha:

```toml
[[licenses.exceptions]]
crate = "epaint_default_fonts"
allow = ["OFL-1.1", "Ubuntu-font-1.0"]
```

Sin `default_fonts`, egui no trae fuentes. En una prueba sin ventana, una etiqueta se maqueta sin pánico (4 glifos) con y sin la feature, pero no he comprobado que un glifo se *vea* sin fuente. La unidad no pinta texto: el estado (zoom, tamaño, píxel bajo el cursor) va en el título de la ventana (`ViewportCommand::Title`). **La decisión de la fuente queda pendiente antes de la primera unidad que dibuje texto**: preferible una fuente de píxeles con licencia OFL, incluyendo su texto de licencia en el archivo de licencias de terceros, o una propia. Entonces se comprobará su licencia con `cargo deny` y se anotará en `docs/DEPENDENCIES.md`.

## 3. Paquetes del sistema y cambios en el CI

**Para compilar en Linux no hace falta ningún paquete `-dev`.** Pruebas, con glow y con wgpu: compilan igual con `PKG_CONFIG_LIBDIR` apuntando a un directorio vacío (sin ficheros `.pc`), y `ldd` sobre el binario solo muestra `libc`, `libm` y `libgcc_s`: X11 (`x11-dl`), xkbcommon, Wayland, EGL/GL y Vulkan se cargan con `dlopen` al ejecutar. Con las features por defecto (`system_fonts`, fontconfig) también compiló. Límite de la prueba: está hecha en Arch ocultando los `.pc`, no en un Ubuntu; el primer CI la confirma. Hace falta un enlazador de C (`cc`), que `ubuntu-latest` trae.

Si el primer CI en Ubuntu fallara al enlazar, el plan B es un paso solo de Linux con `sudo apt-get install -y pkg-config libx11-dev libxkbcommon-dev libwayland-dev libgl1-mesa-dev`. No se añade de antemano. Para *abrir* una ventana en CI (no se hará) harían falta además `xvfb`, `libxkbcommon-x11-0` y Mesa (`libgl1`, `libgl1-mesa-dri`).

Windows y macOS no necesitan paquetes (MSVC y Xcode vienen en los runners). **No he podido compilar para esos dos destinos aquí**; lo confirma el primer CI.

### Cambios en el CI y en sus tests

Comprobado con una copia del repo en `HEAD`:

1. `.github/workflows/ci.yml`: un solo paso nuevo, `cargo build -p px-app --locked`, entre `test` y `deny`. Sin paso `apt` y sin caché (§6).
2. `crates/px-golden/tests/ci.rs`, en lo mínimo: `ALLOWED_ACTIONS` no cambia (no hay acciones nuevas) y los tests actuales pasan sin tocar nada. Para que el orden del paso nuevo quede vigilado, `EXPECTED_COMMANDS` pasa de cuatro a cinco comandos (fmt, clippy, test, build, deny) y se actualizan los textos que dicen «cuatro» (comentario, nombres de dos tests y mensajes). Lo mismo en el punto 6 y en los tests de `specs/hito-0-ci.md`.
3. `crates/px-golden/tests/structure.rs`: `every_external_dependency_is_documented` falla mientras `docs/DEPENDENCIES.md` no contenga `` `eframe` `` y `` `egui` ``. Se anotan en el mismo commit que la dependencia, como pide `CLAUDE.md`.

Si el primer CI falla en Windows o macOS, se corrige dentro de esta unidad.

## 4. Cómo se prueba

**Siempre, en los tres sistemas:** `cargo build -p px-app --locked` en el CI (comprueba que compila y enlaza). El `cargo clippy --workspace --all-targets` y el `cargo test --workspace` ya compilan `px-app`; el paso extra solo añade el enlazado del binario.

**Sin abrir ventana** (todo en `cargo test`, sin GPU ni pantalla):

1. **Modelo de vista** (`px-ui::view`, puro, sin `egui`): tests y propiedades con `proptest`, ver §5.
2. **Conversión `Image` → bytes RGBA** (`px-ui`): los cuatro modos y el índice transparente.
3. **Lienzo de egui sin ventana** (`px-ui::canvas`): un `egui::Context` ejecuta un fotograma con `RawInput` sintético. Comprobado en la copia: el fotograma con una textura cargada con `TextureOptions::NEAREST` produce un `TexturesDelta` con filtro `Nearest` en ampliación y reducción y exactamente una malla con esa textura. Se inyectan `PointerMoved` y `MouseWheel` y se comprueba que el zoom cambia y que el píxel bajo el cursor no se mueve. Detalle: el test debe llamar a `textures_delta.clear()` antes de soltar la salida; si no, egui entra en pánico.
4. `px-app`: sin tests propios; `main` no se ejecuta en tests.

**No se puede probar en CI:** que la ventana se abra, el contexto OpenGL, que el muestreo sea de verdad nítido en la GPU, la escala de pantalla (HiDPI), X11 frente a Wayland y la sensación del ratón. Es la prueba manual (la haces tú): `cargo run -p px-app` en cada sistema; ventana abierta; imagen nítida, sin desenfoque, con zoom 1, 2, 3, 7 y 64; rueda con zoom anclado al cursor; desplazamiento; redimensionar; en pantalla con escala 150 % o 200 % los bloques de píxel salen del mismo tamaño; en Linux, X11 y Wayland (`WINIT_UNIX_BACKEND=x11` o `wayland`) y decoración de la ventana en Wayland; cerrar sin errores.

## 5. Diseño mínimo

Reparto de crates:

- **`px-ui`** (depende de `px-core` y de `egui = "=0.37.0"` con `default-features = false`; nada de `eframe`, `winit` ni GPU): `view` (modelo de vista, no importa `egui`), `texture` (conversión) y `canvas` (el widget de egui). Se prueba entero con `cargo test -p px-ui`.
- **`px-app`** (`px-core`, `px-ui`, `eframe`): `main` abre la ventana con un documento de muestra generado en código (una `Image` RGBA de 64×48 con un patrón y algún píxel transparente; abrir PNG es la unidad 15) y una `App` con `Document`, `View` y la textura.
- **`px-render`**: no se toca; no hace falta `unsafe` (la GPU la maneja `eframe`).

La `Image` vive en un `Document` (`Document::new(image)`, `document.image(document.main_image())`). La unidad no edita nada.

### Modelo de vista (`px-ui::view`)

**No usa ningún tipo de `egui`** (ni de ninguna otra crate gráfica): solo `std` y enteros, para probarlo sin ventana y poder moverlo a `px-render` más adelante. Todo en píxeles físicos y con coordenadas de pantalla `i64`, para que cada píxel de imagen ocupe exactamente `zoom × zoom` píxeles de pantalla y no haya desbordamiento con imágenes grandes.

```rust
/// Nivel de zoom. La representación es privada: hoy solo hay enteros de 1 a 64; la unidad 14
/// añade 1/2, 1/4 y 1/8 sin cambiar esta API pública.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]   // Default = ONE; Display = «×4»
pub struct Zoom { /* privado */ }
Zoom::ONE;  Zoom::MIN /* = ONE */;  Zoom::MAX /* = 64 */
Zoom::from_int(n: u32) -> Option<Zoom>        // Some solo con 1 <= n <= 64
Zoom::zoom_in(self) -> Zoom                   // siguiente nivel (+1); satura en MAX
Zoom::zoom_out(self) -> Zoom                  // nivel anterior (-1); satura en MIN
Zoom::scale(self, n: i64) -> i64              // imagen → pantalla: floor(n · zoom)
Zoom::unscale(self, n: i64) -> i64            // pantalla → imagen: floor(n / zoom)
Zoom::rescale(self, n: i64, to: Zoom) -> i64  // floor(n · to / self)

pub struct Size { pub width: u32, pub height: u32 }                    // Size::new(w, h)
pub struct ScreenRect { pub x: i64, pub y: i64, pub width: i64, pub height: i64 }
pub const MIN_VISIBLE: u32 = 16;

pub struct View { /* zoom: Zoom, origin: (i64, i64) */ }               // origin: esquina sup-izq de la imagen en el lienzo
View::fit(viewport: Size, image: Size) -> View   // mayor zoom entero con la imagen entera visible (mínimo 1), centrada
View::zoom(&self) -> Zoom;   View::origin(&self) -> (i64, i64)
View::set_zoom(&mut self, zoom: Zoom, anchor: (i64, i64))   // el píxel bajo `anchor` no cambia; no hace clamp
View::zoom_in(&mut self, anchor: (i64, i64));  View::zoom_out(&mut self, anchor: (i64, i64))
View::pan(&mut self, dx: i64, dy: i64, viewport: Size, image: Size)   // desplaza y hace clamp
View::clamp(&mut self, viewport: Size, image: Size)
View::image_rect(&self, image: Size) -> ScreenRect
View::screen_to_image(&self, p: (i64, i64), image: Size) -> Option<(u32, u32)>   // None fuera de la imagen
View::image_to_screen(&self, x: u32, y: u32) -> (i64, i64)                       // esquina sup-izq del bloque
```

El zoom va **centrado en el cursor**: el lienzo llama a `zoom_in`/`zoom_out` con la posición del puntero como `anchor`, y luego a `clamp`. `set_zoom` calcula el origen nuevo como `anchor - zoom_viejo.rescale(anchor - origen, zoom_nuevo)` (por eje), de modo que la imagen se amplía alrededor del punto. Con `Zoom` entero, `scale(n) = n·z` y `unscale(n) = n.div_euclid(z)`.

Comportamiento y propiedades que prueban los tests:

1. **Niveles.** `from_int(n)` es `Some` exactamente para `1..=64` y `scale(1) == n`. `zoom_in` sube de uno en uno de 1 a 64 y satura; `zoom_out` baja y satura en 1. `Zoom` se ordena como su entero. `Display` da «×4».
2. **`scale`, `unscale` y `rescale`** para todo zoom entero `z` y todo `n` (también negativo): `scale(n) = n·z`; `unscale(n) = floor(n/z)`; `unscale(scale(n) + k) == n` para `0 <= k < z`; `rescale(n, to) = floor(n·to/z)`.
3. **Ida y vuelta exacta pantalla ↔ imagen, para cualquier zoom entero de 1 a 64**, cualquier origen y tamaño de imagen:
   - para todo píxel `(x, y)`: `screen_to_image(image_to_screen(x, y)) == Some((x, y))`, y lo mismo para las cuatro esquinas del bloque (`+ (z-1)` en cada eje);
   - para todo punto `p` dentro de `image_rect`: con `(x, y) = screen_to_image(p)` y `c = image_to_screen(x, y)`, se cumple `c <= p < c + z` y `screen_to_image(c) == Some((x, y))`;
   - para todo punto fuera de `image_rect`: `None`.
4. **Teselado sin huecos ni solapes:** `image_to_screen(x + 1, y).0 == image_to_screen(x, y).0 + z` y lo mismo en vertical; `image_rect` es `(origen, scale(ancho), scale(alto))`.
5. **Ancla:** tras `set_zoom(z2, anchor)` desde cualquier vista, el píxel que `screen_to_image` da para `anchor` es el mismo que antes (si estaba dentro de la imagen), y el índice del bloque (con `unscale`) se conserva aunque el ancla esté fuera de la imagen; `zoom_in`/`zoom_out` lo cumplen igual.
6. **`fit`:** el zoom es el mayor entero `z <= 64` con `ancho·z <= viewport.ancho` y `alto·z <= viewport.alto` (1 si no cabe ni a 1); la imagen queda centrada con diferencia de como mucho 1 px por eje; `fit` es determinista.
7. **Clamp:** tras `clamp`/`pan` quedan visibles al menos `min(MIN_VISIBLE, extensión·zoom, viewport)` píxeles de la imagen en cada eje; `clamp` es idempotente y no mueve una vista que ya cumple (con un mínimo de 0, es decir, lienzo o imagen de 0 píxeles, no mueve nada); `pan` equivale a desplazar el origen y hacer `clamp`.
8. **Sin pánico ni desbordamiento** para cualquier tamaño de imagen y de viewport (hasta 2³⁰ por eje), cualquier origen y ancla en un rango amplio de `i64` razonable (±2⁴⁰).

### De `Image` a textura

- `px-ui::texture::to_rgba8(image: &Image) -> Vec<u8>`, alfa directo, `w·h·4` bytes: RGBA tal cual; indexado con `palette.resolved(i)` (el índice transparente da `a = 0`); gris `(g, g, g, 255)`; gris con alfa `(g, g, g, a)`. Usa `get_pixel` hasta que la unidad 12 dé acceso por bytes y la sustituya.
- Luego `egui::ColorImage::from_rgba_unmultiplied([w, h], &bytes)` (egui premultiplica; un píxel semitransparente se cuantiza a 8 bits premultiplicado, y los de `a = 0` pierden el RGB; solo afecta a lo que se ve) y `ctx.load_texture("imagen", color_image, egui::TextureOptions::NEAREST)`: **sin suavizado** ni al ampliar ni al reducir, sin mipmaps. Se sube una vez; el refresco por cambios y por regiones son las unidades 6 y 14.
- Si `w` o `h` superan `max_texture_side` del contexto, el lienzo no pinta la imagen y devuelve un error (`TooLarge`), que `px-app` muestra en el título. Dividir en teselas es la unidad 14.

### Lienzo (`px-ui::canvas`)

Rellena el espacio disponible, con color de fondo liso (el tablero de ajedrez de la transparencia queda fuera). Convierte entre puntos y píxeles físicos con `pixels_per_point`; el rectángulo de la imagen sale de `View::image_rect` con el origen ya en píxeles físicos enteros. Entradas: rueda o pellizco = un paso de zoom por evento (el signo, no la cantidad) anclado al puntero; arrastre con botón central, o con espacio y botón izquierdo = `pan`; `Home` = `View::fit`; `+` y `-` = zoom. El estado se devuelve a `px-app` y este lo pone en el título («64×48 · ×4 · (12, 7)»).

## 6. Caché de compilación en el CI

**No se hace en esta unidad** (resolución 6). Queda como registro el análisis: `actions/cache` (MIT) con restore y save separados habría necesitado añadir `actions/cache/restore` y `actions/cache/save` a `ALLOWED_ACTIONS` de `ci.rs`, y unos 430 MB comprimidos por sistema (medido en Linux con glow). `Swatinem/rust-cache` queda descartada por su licencia, **LGPL-3.0** (comprobado con `gh api repos/Swatinem/rust-cache`), que `CLAUDE.md` excluye aunque sea solo CI. Se decide con los tiempos reales del primer CI: si el CI en frío es molesto en Windows o macOS, se abre una unidad aparte.

## Ficheros de la unidad

Tres commits pequeños, porque la unidad es mayor que una función:

1. `px-ui`: `view` (`Zoom`, `View`) y sus tests (sin dependencias nuevas; `proptest` como dev-dependency, que ya está anotada).
2. `px-ui`: `texture::to_rgba8` y sus tests (sin dependencias nuevas).
3. Dependencias y ventana: `px-ui/Cargo.toml` (`egui`), `px-app/Cargo.toml` y `src` (`eframe`), `canvas` con sus tests sin ventana, `docs/DEPENDENCIES.md` (filas de `eframe` y `egui`, las transitivas y las notas de `self_cell` y `r-efi`), `.github/workflows/ci.yml` (solo el paso de build), `crates/px-golden/tests/ci.rs` y `specs/hito-0-ci.md`. **`deny.toml` no cambia.** Antes: `cargo deny check` y el agente `license-auditor`.

## Fuera de alcance

Abrir y guardar ficheros (15), editar (3 a 7), tablero de la transparencia, texto en la interfaz y tema de píxeles, lienzos mayores que la textura máxima (14), `eframe` en web, accesibilidad, portapapeles y persistencia de la ventana.

## Resolución (visto bueno, 2026-10-17)

Decisiones 1 a 7 aceptadas como se recomendaban, con estos matices:

1. **glow**, dejando dicho que OpenGL está obsoleto en macOS y que volver a `wgpu` es una línea (§1). En `docs/DEPENDENCIES.md` se anota también `self_cell` (`GPL-2.0-only` solo como alternativa de un `OR`), igual que `r-efi`.
2. **Sin `default_fonts`**; la decisión de la fuente queda pendiente antes de la primera unidad que dibuje texto (§2).
3. **`egui` en `px-ui`**, pero el modelo de vista (zoom, desplazamiento y conversión pantalla ↔ imagen) no usa tipos de `egui`, para probarlo sin ventana y poder moverlo a `px-render` (§5).
4. **Zoom entero de 1 a 64, paso ±1, centrado en el cursor**; `Zoom` es un tipo opaco que admitirá 1/2, 1/4 y 1/8 en la unidad 14 sin cambiar la API pública, y la ida y vuelta pantalla ↔ imagen es exacta para todo zoom entero (propiedad 3 de §5).
5. **`px-render` sin tocar.**
6. **CI: solo `cargo build -p px-app --locked`, sin caché**; `ci.rs` en lo mínimo (§3).
7. **Sin `wayland-csd-adwaita`**; `x11` y `wayland` de `eframe` solo en Linux, sin `accesskit` ni `persistence` (§1).

Esta unidad **no abre ficheros**: la ventana muestra un documento de muestra generado en código; abrir PNG por argumento o por diálogo es la unidad 15.

## Cómo se midió

Copia del repo en el scratchpad (fuera del repo), con `cargo add` y `Cargo.lock` regenerado; builds limpios con `rm -rf target`; licencias con `cargo tree -e normal,build --target all --format '{p}|{l}'` y con `cargo deny --locked check`; CI simulado con `cargo clippy --workspace --all-targets --locked -j2 -- -D warnings`, `cargo test --workspace --locked -j2` y `cargo build -p px-app --locked -j2`. El repo no se ha modificado salvo por esta página y su anexo.
