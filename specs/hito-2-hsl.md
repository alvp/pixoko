# Hito 2 · Unidad 1: conversión RGB ↔ HSL

Estado: **aprobada** (2026-10-17; `f32` con paso a `f64` interno si el test exhaustivo falla en algún color; si el exhaustivo tarda más de ~10 s va `#[ignore]` con un subconjunto por defecto, se ejecuta una vez en release y el CI de Linux lo corre en un paso propio (no hizo falta: tarda 2,1 s); lista de unidades reordenada, ver el final)

## Objetivo

Definir en `px-core` el tipo `Hsl` (matiz, saturación, luminosidad) y la conversión de ida y vuelta con `Rgba`. El editor de paletas (RGB, HSL) y los rangos de color la necesitan, y es la unidad más pequeña y sin dependencias del hito: matemática pura sobre tipos que ya existen.

## Entradas y salidas

- Tipo: `px_core::Hsl`. Deriva `Clone`, `Copy`, `Debug`, `PartialEq`, `Default`. `Default` es negro: `Hsl::new(0.0, 0.0, 0.0)`.
- Campos privados, con los accesores `h() -> f32`, `s() -> f32` y `l() -> f32`. Rangos: `h` en `[0, 360)` (grados), `s` y `l` en `[0, 1]`. Invariante: nunca hay NaN ni infinitos.
- `Hsl::new(h: f32, s: f32, l: f32) -> Hsl`: normaliza la entrada (comportamiento 1) y no falla.
- `Hsl::from_rgba(color: Rgba) -> Hsl`: ignora el alfa.
- `Hsl::to_rgba(self, alpha: u8) -> Rgba`: el alfa se pasa aparte, porque `Hsl` no lo guarda.

No hay `Result`: toda entrada tiene salida (ver comportamiento 1). No se añade `Rgba::to_hsl`; hay un solo camino, para tener una sola función que probar.

## Comportamiento

1. **Normalización (`new`).**
   - `h`: si no es finito (NaN, ±∞) pasa a `0`; si no, `h.rem_euclid(360.0)`, y si el resultado es `>= 360.0` (puede ocurrir por redondeo con negativos diminutos) pasa a `0`. Así `360` equivale a `0`, `-60` a `300` y `720.5` a `0.5`.
   - `s` y `l`: NaN pasa a `0`; el resto se recorta a `[0, 1]` (así, `+∞` da `1` y `-∞` da `0`).
2. **Fórmulas.** Son las del hexágono HSL estándar, equivalentes a las de CSS Color Module Level 4 (§«HSL»); es matemática pública. Con `r, g, b` en `[0, 1]` (canal / 255), `max`, `min` y `d = max - min`:
   - `l = (max + min) / 2`.
   - Si `d == 0` (gris, incluidos negro y blanco): `h = 0`, `s = 0`.
   - Si no: `s = d / (1 - |2l - 1|)`, y `h = 60 · sector`, donde el sector es `((g - b) / d) mod 6` si `max` es `r`, `(b - r) / d + 2` si es `g` y `(r - g) / d + 4` si es `b` (con `mod 6` positivo). Si hay empate de máximos, el orden de prueba es `r`, `g`, `b`.
   - Inversa: `c = (1 - |2l - 1|) · s`, `x = c · (1 - |(h / 60) mod 2 - 1|)`, `m = l - c / 2`; `(r, g, b)` es `(c, x, 0)`, `(x, c, 0)`, `(0, c, x)`, `(0, x, c)`, `(x, 0, c)` o `(c, 0, x)` según el sector `floor(h / 60)` (0 a 5), más `m` en cada canal.
3. **Redondeo a 8 bits.** `to_rgba` calcula `canal · 255`, redondea con `f32::round` (la mitad se aleja de cero: `127.5` da `128`) y recorta a `[0, 255]`. Solo se usan sumas, restas, productos, divisiones, resto (`rem_euclid`, `%`), `abs`, `min`, `max`, `floor` y `round`, todas exactas según IEEE 754 y sin dependencia de la biblioteca matemática del sistema: el resultado es el mismo en Linux, Windows y macOS.
4. **Ida y vuelta exacta.** Para todo `c: Rgba` (los 16 777 216 RGB posibles), `Hsl::from_rgba(c).to_rgba(c.a) == c`. Es la propiedad central de la unidad. Si el cálculo en `f32` no la cumpliera para algún color, la unidad no se da por buena: se pasa la aritmética interna a `f64` (los accesores siguen devolviendo `f32`) y se anota en «Decisiones».
5. **Alfa.** `from_rgba` descarta el alfa; `to_rgba(alpha)` lo copia tal cual. Con `alpha = 0` conserva el RGB (alfa directo, como en `Rgba`).
6. **Grises.** `Hsl::from_rgba` de un gris da `h = 0`, `s = 0`. Al revés, con `s == 0` el resultado es un gris `round(l · 255)` para cualquier `h`; con `l == 0` es negro y con `l == 1` es blanco para cualquier `h` y `s`. La conversión pierde el matiz de los grises: no es biyectiva fuera de los colores cromáticos, y es parte del contrato (ver «Notas para el panel de paletas»).
7. **Sin pánico.** Ninguna operación pública hace pánico con ningún `f32` (incluidos NaN, ±∞ y subnormales) ni con ningún `Rgba`.

## Fuera de alcance

- HSV/HSB, y luminosidad perceptual (Lab, luma de Rec. 709).
- Alfa dentro de `Hsl`.
- Búsqueda del color más cercano en una paleta (unidad 2).
- Rangos de color y degradados en HSL (hito 4).
- Los controles del panel (deslizadores, campos numéricos).

## Notas para el panel de paletas

Como los grises (y el negro y el blanco) pierden el matiz, el panel no debe volver a calcular el `Hsl` desde el `Rgba` en cada fotograma: guarda el `Hsl` que está editando y lo convierte a `Rgba` al aplicar. Si no, arrastrar la saturación a 0 y volver a subirla perdería el matiz elegido. Esta unidad solo fija la conversión.

## Casos límite

Notación: `Hsl(h, s, l)` pasa por `new`; las comparaciones de `f32` en los tests usan una tolerancia de `1e-4`, salvo que se diga «exacto».

| Operación | Resultado |
| --- | --- |
| `Hsl::default()` y `Hsl::new(0.0, 0.0, 0.0)` | iguales; `h() == 0`, `s() == 0`, `l() == 0` |
| `Hsl::new(360.0, 1.0, 0.5).h()`; `new(-60.0, …).h()`; `new(720.5, …).h()` | `0.0` (exacto); `300.0`; `0.5` |
| `new(f32::NAN, …).h()`, `new(f32::INFINITY, …).h()`, `new(f32::NEG_INFINITY, …).h()` | `0.0` los tres |
| `new(-1e-9, …).h()` | `0.0` (nunca `360.0`) |
| `new(0.0, 2.0, -3.0)`; `new(0.0, f32::NAN, f32::NAN)`; `new(0.0, f32::INFINITY, f32::NEG_INFINITY)` | `s() == 1`, `l() == 0`; `s() == 0`, `l() == 0`; `s() == 1`, `l() == 0` |
| `from_rgba` de `(255,0,0)`, `(255,255,0)`, `(0,255,0)`, `(0,255,255)`, `(0,0,255)`, `(255,0,255)` (alfa 255) | `h` = 0, 60, 120, 180, 240, 300; `s = 1`; `l = 0.5` |
| `from_rgba(0,0,0)`; `from_rgba(255,255,255)`; `from_rgba(128,128,128)` | `(0, 0, 0)`; `(0, 0, 1)`; `h = 0`, `s = 0`, `l = 128/255` |
| `from_rgba(1,0,0)`; `from_rgba(255,254,254)` | `h = 0`, `s = 1`, `l = 1/510`; `h = 0`, `s = 1`, `l = 509/510` (la saturación es máxima aunque el color casi sea gris) |
| `from_rgba(255,128,0)` | `h ≈ 30.1176` (`60 · 128/255`), `s = 1`, `l = 0.5` |
| Empates de máximo: `from_rgba(255,255,0)`, `(0,255,255)` y `(255,0,255)` | `h = 60`, `180` y `300` (orden de prueba `r`, `g`, `b`) |
| `Hsl::new(0.0, 1.0, 0.5).to_rgba(255)` y los cinco matices restantes (60 … 300) | `Rgba(255,0,0,255)`, `(255,255,0,255)`, `(0,255,0,255)`, `(0,255,255,255)`, `(0,0,255,255)`, `(255,0,255,255)` |
| `new(30.0, 1.0, 0.5).to_rgba(255)` | `Rgba(255,128,0,255)` (`127.5` redondea a 128) |
| `new(h, 0.0, 0.5).to_rgba(255)` con `h` = 0, 90, 359.9 | `Rgba(128,128,128,255)` los tres |
| `new(h, s, 0.0).to_rgba(255)` y `new(h, s, 1.0).to_rgba(255)` con cualquier `h`, `s` | `Rgba(0,0,0,255)` y `Rgba(255,255,255,255)` |
| `from_rgba(Rgba(10,20,30,0))` frente a `from_rgba(Rgba(10,20,30,255))` | el mismo `Hsl` |
| `Hsl::new(210.0, 0.5, 0.4).to_rgba(0)` y `to_rgba(77)` | el RGB es el mismo; el alfa es `0` y `77` |
| `from_rgba(c).to_rgba(c.a)` para cada `c` de un conjunto fijo: las 8 esquinas del cubo RGB, `(1,0,0)`, `(0,1,0)`, `(0,0,1)`, `(254,255,255)`, `(127,128,129)` | igual a `c` |

## Criterios de aceptación

Desde la raíz, en Linux (el CI cubre los otros dos sistemas):

- `cargo fmt --check`
- `cargo clippy --workspace --all-targets -- -D warnings`
- `cargo test --workspace`
- `cargo deny check` (no se añade ninguna dependencia: `proptest` ya está como dev-dependency de `px-core`)

## Tests

En `crates/px-core/tests/hsl.rs` (integración sobre la API pública, como en `Rgba` y `Palette`):

**Unitarios**: una prueba por fila de la tabla de casos límite (cada fila puede ser una o varias aserciones).

**Exhaustivo** (comportamiento 4): recorre los 2²⁴ RGB y comprueba la ida y vuelta exacta. Medido en el perfil de test (sin optimizar, Linux): 2,1 s, por debajo del umbral de ~10 s fijado al aprobar la spec, así que corre siempre, en los tres sistemas, sin `#[ignore]` ni subconjunto y sin paso de CI propio. Si en algún sistema del CI superara el umbral, se vuelve al plan previsto: `#[ignore]`, subconjunto de paso fijo por defecto y un paso solo en Linux con `cargo test --release --locked -- --ignored`.

**Propiedades** (`proptest`):

1. **Rangos.** Para cualquier `Rgba`, `from_rgba` da `h` en `[0, 360)`, `s` y `l` en `[0, 1]`, sin NaN. Para cualquier trío de `f32` (incluidos NaN, ±∞ y subnormales), `new` cumple el mismo invariante y no hace pánico.
2. **Normalización.** `new` es idempotente: `Hsl::new(x.h(), x.s(), x.l()) == x` para todo `x` construido con `new`. Para `h` finito, `new(h + 360·k, s, l)` y `new(h, s, l)` dan el mismo `Hsl` cuando `h + 360·k` es exacto (se prueba con `k` en `-3..=3` y `h` entero).
3. **Ida y vuelta.** Para cualquier `Rgba`, `Hsl::from_rgba(c).to_rgba(c.a) == c`.
4. **Rotación de canales.** Para cualquier `Rgba` cromático `c`, sea `c' = Rgba(c.b, c.r, c.g, c.a)`: `s` y `l` de `c'` son iguales a las de `c` (exacto, salen de los mismos `max` y `min`) y `h` de `c'` es `h` de `c` más 120 módulo 360, con distancia circular menor que `1e-3`. Comprueba los tres sectores y sus empalmes.
5. **Grises.** `from_rgba(Rgba(v, v, v, a))` da `h = 0` y `s = 0` para todo `v`; y `l == v / 255` dentro de `1e-6`.
6. **Alfa.** Para cualquier `Hsl` y cualquier `alpha: u8`, `to_rgba(alpha).a == alpha`, y el RGB no depende de `alpha`.
7. **Cromáticos.** Para cualquier `Rgba` con `r`, `g`, `b` no todos iguales, `from_rgba(c).s() > 0`, y `to_rgba` de ese `Hsl` no es gris (`r == g == b` es falso).

No hay dorado: HSL es una definición matemática pública (CSS Color Level 4), no el comportamiento de un programa de referencia. Si más adelante el panel de paletas debe coincidir con la conversión de Grafx2 o LibreSprite (que pueden usar enteros), se hará con un dorado en la unidad del panel y se anotará aquí cualquier discrepancia.

## Decisiones

1. **`f32`, con aritmética interna en `f32`** (confirmado: el test exhaustivo pasa con los 2²⁴ colores, sin pasar a `f64`). `egui` trabaja en `f32`, y una conversión con solo operaciones IEEE exactas es reproducible entre sistemas.
2. **Campos privados con `new` que normaliza.** El invariante (sin NaN, rangos fijos) se mantiene sin que cada llamador lo repita, como el invariante del índice transparente en `Palette`.
3. **Alfa fuera de `Hsl`.** Sigue la regla de alfa directo del proyecto: la conversión toca el color, no la opacidad.
4. **Sin `Rgba::to_hsl`.** Un solo camino por función; se puede añadir como azúcar si el uso lo pide.
5. **Sin dependencias nuevas.**

## Unidades del hito 2

Hito 2: «Editor mínimo». Puerta de salida: abrir, editar y guardar un PNG real en tres sistemas. Cada unidad es una función o un panel. El orden pone primero el mayor riesgo técnico (la ventana en los tres sistemas) y luego las herramientas; la lista se puede ajustar al cerrar cada unidad. Regla común: toda herramienta se ejecuta como un `Command` de `Document`, nunca tocando la `Image` directamente.

1. **Conversión RGB ↔ HSL** (esta página, `px-core`): el tipo `Hsl` y su ida y vuelta exacta con `Rgba`.
2. **Ventana mínima con `eframe`** (`px-app`, `px-ui`, `px-render`): una ventana que muestra una `Image` con zoom entero y desplazamiento, sin herramientas, que compila y arranca en Linux, Windows y macOS; su spec lista antes `eframe` y su árbol (versión, features mínimas, licencias comprobadas con `cargo deny` sobre una copia, licencias nuevas que habría que permitir y paquetes del sistema del CI de Linux) y no toca `deny.toml` sin consultar.
3. **Línea determinista** (`px-raster`): una función que da los píxeles de un segmento y se ejecuta como `SetPixels`.
4. **Historia por regiones** (`px-core`): los pasos de deshacer guardan rectángulos o tramos en vez de un píxel por cambio, y un comando de relleno de región los usa.
5. **Relleno por inundación** (`px-raster`): calcula la región conectada y la aplica con el comando de la unidad 4.
6. **Lápiz, deshacer y rehacer en la app** (`px-ui`, `px-app`): el trazo de ratón se convierte en `Command` y se añaden Ctrl+Z y Ctrl+Y, que son la primera vía de edición desde la ventana.
7. **Línea y relleno en la app** (`px-ui`): la barra de herramientas conecta las unidades 3 y 5 con el ratón, ambas como `Command`.
8. **Color más cercano en una paleta** (`px-core`): `Palette` busca la entrada de menor distancia a un `Rgba`.
9. **Conversión entre modos de `Image`** (`px-core`): RGBA ↔ indexado ↔ grises, documentada y sin pérdidas silenciosas.
10. **Editar la paleta de una imagen y reordenarla sin alterar la imagen** (`px-core`): métodos de `Image` que mantienen el invariante y remapean los píxeles, como comando con deshacer.
11. **Panel de paletas** (`px-ui`): edición en RGB y HSL guardando el `Hsl` en edición, y selección del color activo.
12. **Acceso por filas y bytes a `Image`** (`px-core`): `from_bytes`, `as_bytes` y filas, con su propia API pública y sin saltarse el invariante del modo indexado.
13. **Escritor de PNG por flujo y lectura sin reserva anticipada** (`px-formats`): `write_png` sobre un `impl Write` fila a fila y `read_png` que no reserva 1 GiB antes de ver los datos.
14. **Lienzos grandes en la app** (`px-render`, `px-ui`): redibujo por regiones modificadas y prueba con un lienzo grande, usando las unidades 4, 12 y 13.
15. **Abrir y guardar PNG en la app y puerta del hito** (`px-app`, `px-cli`): diálogo de abrir y guardar, y la comprobación de la puerta, con una prueba sin ventana en `px-cli` en los tres sistemas más una lista de prueba manual con la app (se detalla en su spec, porque el CI no abre ventanas).

### Reparto de las notas «Para el hito 2»

| Nota | Dónde está | Unidad |
| --- | --- | --- |
| Relleno por regiones o tramos en la historia | `specs/hito-1-document.md`, «Notas para el hito 2» | 4 (la usa la 5) |
| `apply` devuelve si hubo cambio, para no marcar el documento ni refrescar el lienzo | ídem | 6 (la UI la usa) |
| Acceso por bytes (y filas) a `Image` | `specs/hito-1-png.md`, «Para el hito 2» y decisión 2; `specs/hito-1-image.md` | 12 |
| Escritor por flujo y pico de memoria al escribir (~2 GiB) | `specs/hito-1-png.md`, «Para el hito 2» | 13 |
| Reserva de hasta 1 GiB antes de detectar un fichero truncado | ídem | 13 |
| Lienzos grandes (regiones modificadas, límites) | `specs/hito-1-rgba.md` y `specs/hito-1-document.md` | 14 |
| Editar la paleta de una `Image` manteniendo el invariante, y quitar, insertar y reordenar entradas | `specs/hito-1-image.md` (comportamiento 8) y `specs/hito-1-palette.md` | 10 |
| Conversión entre modos | `specs/hito-1-image.md` | 9 |
| Búsqueda del color más cercano | `specs/hito-1-palette.md` | 8 |
