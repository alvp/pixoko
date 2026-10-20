# Hito 2 · Unidad 14-limpieza: dos limpiezas de API (`screen_to_image_i32` y `TitleInfo.tool`)

Estado: **aprobada** (2026-10-20; decisiones 1 a 7 como están, con los matices de «Visto bueno» al final, que **prevalecen sobre cualquier otra frase de la página**).

Referencias: D13 de [`hito-2-reparto-cierre.md`](hito-2-reparto-cierre.md) (la lista de partida, que esta página corrige) y los dos puntos de «API» de [`hito-2-notas-unidad-14.md`](hito-2-notas-unidad-14.md). Origen de cada deuda: decisiones 6 y 7 de [`hito-2-linea-relleno-app.md`](hito-2-linea-relleno-app.md).

Cambios: `px-ui` (`view.rs`, `canvas.rs`) y `px-app` (`title.rs`, `main.rs`). **No se toca `px-core` ni `px-raster`.** Sin dependencias nuevas ni `unsafe`; `docs/DEPENDENCIES.md` no cambia. Es una refactorización: **ningún texto ni valor que ve el usuario cambia**.

## Objetivo

1. `View::screen_to_image_i32` deja de recibir un `Size` que ignora.
2. `TitleInfo` tiene un campo propio para la herramienta. Hoy `window_title` construye un `TitleInfo` nuevo y mete `status_text(tool, color)` dentro de `color` (un apaño anotado como «hasta la unidad 14»).

## Lo comprobado

Hecho en una copia de las fuentes en `~/.cache/pixoko-scratch` (fuera del repositorio y de `/tmp`, con su `CARGO_TARGET_DIR` dentro), **borrada al terminar**. Se aplicó el cambio de producción mínimo (abajo) y se compiló **sin tocar ninguna prueba**, con `cargo build --tests --keep-going -p px-ui -p px-app`:

| Fichero de pruebas | Errores | Causa |
| --- | --- | --- |
| `px-ui/tests/view_i32.rs` | 14 | llamada con el `Size` de más |
| `px-ui/tests/view_ops.rs` | 2 | ídem |
| **`px-ui/tests/zoom_ladder.rs`** | **12** | ídem (**no está en la lista de D13**: es de la 14a-1, posterior a ella) |
| `px-app/src/title.rs` (pruebas en línea) | 22 | 7 literales sin `tool` y 15 llamadas a `window_title`/`status_line` con el argumento `tool` de más |
| resto de `px-ui` y `px-app` | 0 | (`canvas.rs` y `main.rs` son código de producción, no pruebas) |

Después se adaptaron las pruebas **solo** en firma y literales y se comprobó en la copia: `cargo fmt --check` limpio, `cargo clippy --workspace --all-targets --locked -- -D warnings` sin avisos y todas las pruebas de `px-ui` y `px-app` en verde (px-app: 822 pasan, 3 ignoradas, como antes). **Ningún texto ni valor esperado cambió.** Dos hallazgos más:

- Al quitar el `Size`, **dos pruebas de `view_i32.rs`** (`fila_13_zoom_x4_con_origen_10_10` y `los_bloques_de_cuatro_pixeles_y_los_signos_de_cada_eje_son_independientes`) dejan `img` sin usar; sin cambiar `let (v, img)` por `let (v, _)` clippy `-D warnings` falla. Es un enlace sin uso, no una aserción.
- `cargo fmt` recoloca en una línea algunos `assert_eq!` de varias líneas que se acortan; es formato.

### Correcciones a D13

| D13 decía | Es |
| --- | --- |
| `view_i32.rs`: «8 pruebas y ~15 referencias» | 4 pruebas con 14 llamadas (3 + 8 + 1 + 2) |
| `view_ops.rs`: 2 llamadas, prueba del centro | 1 prueba, 2 llamadas (igual) |
| (no estaba) | **`zoom_ladder.rs`: 5 pruebas, 12 llamadas** |
| `title.rs`: «31 literales en 4 módulos» | 27 literales en **3** módulos (`file_title_tests` no tiene ninguno); solo **7** enumeran todos los campos y necesitan `tool: ..`; los otros 20 usan `..base()`/`..completo()` |
| (no estaba) | **15 llamadas a `window_title`/`status_line` pierden el argumento `tool`** (ver decisión 2) |

## Firmas nuevas

```rust
// px-ui/src/view.rs
pub fn screen_to_image_i32(&self, p: (i64, i64)) -> (i32, i32)     // antes: (&self, p, _image: Size)

// px-app/src/title.rs
pub struct TitleInfo {
    pub size: Option<(u32, u32)>,
    pub zoom: Zoom,
    pub hover: Option<(u32, u32)>,
    pub tool: Option<Tool>,        // NUEVO; None = no escribe herramienta
    pub color: Option<String>,     // ya NO lleva la herramienta: solo `color 3 #ff0000`
    pub can_undo: bool,
    pub can_redo: bool,
    pub error: Option<String>,
}
pub fn title_text(info: &TitleInfo) -> String          // igual de firma; ahora escribe la herramienta si `tool` es Some
pub fn window_title(info: &TitleInfo) -> String        // antes: (tool, info); alias de `title_text(info)` (ver abajo)
pub fn status_line(info: &TitleInfo) -> String         // antes: (tool, info)
// Construcción de producción de `TitleInfo` (7 argumentos, el máximo de clippy): `tool` siempre es `Some(tool)`.
pub fn app_title_info(tool: Tool, image: Option<&Image>, canvas: &CanvasInfo,
                      color: Option<String>, can_undo: bool, can_redo: bool,
                      error: Option<String>) -> TitleInfo
```

`window_title` **es un alias de una línea de `title_text`** y lleva un comentario que dice por qué existe: lo citan varias specs y pruebas (filas 22 y 10, propiedad 5) y se conserva para no reescribirlas. **Debe quitarse cuando las specs dejen de citarlo** (y sus llamadas pasan a `title_text`).

`main.rs` construye el `TitleInfo` **solo** con `app_title_info`; ya no hay literal `TitleInfo { .. }` en `main.rs`.

`title_text` escribe, entre el cursor y deshacer, ` · {status_text(tool, color)}` si hay herramienta y ` · {color}` si no (y nada si no hay ninguna de las dos). `status_text` **se queda** como única fórmula de «`línea · color 3 #ff0000`»; `px-ui/src/tool.rs` y sus pruebas (`tool.rs`, `text.rs`) no se tocan. En `main.rs`, el literal gana `tool: Some(self.tools.tool())` y las dos llamadas pierden el argumento. `canvas.rs` pierde el segundo argumento de su única llamada.

## Convivencia o todo de una vez

**Todo de una vez**, sin periodo de convivencia:

- `screen_to_image_i32` no puede convivir: Rust no tiene sobrecarga, así que la firma nueva exigiría otro nombre y luego renombrar (el doble de cambios en las mismas 29 llamadas, para un método con **una** llamada de producción).
- Un campo nuevo sin valor por defecto rompe todos los literales por construcción; solo se evitaría con `Default` y `..Default::default()`, que también toca cada literal.
- Mantener `window_title(tool, info)` y `status_line(tool, info)` junto a la forma nueva dejaría la herramienta con dos fuentes (el argumento y el campo) y conservaría el apaño.

Consecuencia con el orden de `CLAUDE.md`: el commit de adaptación **no compila** hasta la lógica (permitido: «los commits de pruebas en rojo no tienen que compilar»). Las pruebas de `title.rs` están dentro del mismo fichero que el código: el commit de adaptación toca **solo** los módulos `#[cfg(test)]` (con `git add -p`), y el de `px-app` el resto del fichero y `main.rs`.

## Casos (8 filas, módulo nuevo `tool_field_tests` en `px-app/src/title.rs`)

`base`: tamaño 64×48, zoom ×8, cursor (3, 4), color `color 3 #ff0000`, sin deshacer ni rehacer. «Línea» = `Tool::Line`.

| # | Entrada | Esperado |
| --- | --- | --- |
| 1 | `title_text` con `tool: None` | `Pixoko: 64×48 · ×8 · (3, 4) · color 3 #ff0000` (la herramienta no sale; como el título de la unidad 6) |
| 2 | `tool: Some(Line)` | `Pixoko: 64×48 · ×8 · (3, 4) · línea · color 3 #ff0000` |
| 3 | `tool: Some(Line)`, `color: None` | `Pixoko: 64×48 · ×8 · (3, 4) · línea` |
| 4 | Las cinco herramientas, sin cursor, con el color de `base` | `… · ×8 · lápiz · color…`, `línea`, `relleno`, `cuentagotas`, `borrador` (hoy solo se prueban tres) |
| 5 | `tool: Some(Eraser)`, `color: Some("color 0 transparente")`, con deshacer y rehacer | `Pixoko: 64×48 · ×8 · (3, 4) · borrador · color 0 transparente · deshacer · rehacer` (orden de todas las partes) |
| 6 | `error: Some("no se pudo guardar")` con cada una de las cinco herramientas y con `None` | `Pixoko: no se pudo guardar`, nada más |
| 7 | `window_title(&info)` y `status_line(&info)` con `tool: None` y con `Some(Line)` | el título es `title_text(&info)`; la línea es ese texto sin `Pixoko: `. Con `None` no sale herramienta (antes `window_title` la ponía siempre) |
| 8 | `app_title_info(tool, ..)` con cada una de las cinco herramientas (imagen con y sin, `CanvasInfo` cualquiera) | `.tool == Some(tool)`, nunca `None`, y `window_title` lleva su etiqueta. Además, `include_str!("main.rs")` **no contiene** `TitleInfo {` (guarda de que `main.rs` no construye el literal por otro camino; es una comprobación de texto, frágil a propósito: si falla, hay que mirar por qué) |

`screen_to_image_i32` **no tiene filas nuevas**: su comportamiento no cambia y lo cubren las 10 pruebas adaptadas (incluida la propiedad contra el oráculo de `i128` de `view_i32.rs` y las de `zoom_ladder.rs`); el cambio de firma lo comprueba el compilador.

## Propiedades (`proptest`, 256 casos cada una; 2 de las 3 permitidas)

- **P1. Misma salida que el apaño.** Para cualquier `tool` (`None` o una de las cinco), tamaño (o ninguno), zoom, cursor, color, deshacer, rehacer y error (o ninguno): `title_text(&info)` es igual a un oráculo **escrito en la prueba** con `format!` que reproduce la fórmula anterior (`Pixoko: {w}×{h} · {zoom}`, ` · ({x}, {y})`, ` · {etiqueta} · {color}` o solo la etiqueta o solo el color, ` · deshacer`, ` · rehacer`; con error, `Pixoko: {error}`), sin llamar a `status_text` ni a `title_text`.
- **P2. Caracteres de la barra con las cinco herramientas.** Para cualquier combinación, `status_line` no contiene saltos de línea y todos sus caracteres están en `INTERFACE_CHARSET` (la propiedad 5 de [`hito-2-texto.md`](hito-2-texto.md), que hoy solo usa lápiz, línea y relleno, **se adapta tal cual**; esta la amplía a cuentagotas y borrador y a `tool: None`).

## Pruebas anteriores que cambian (lista exacta, medida en la copia)

Cada una afirma **lo mismo**: solo cambia la firma o el literal. El commit lista cada prueba con su aserción antigua y la nueva. Si alguna necesitara más, me paro.

**`px-ui` (10 pruebas, 28 llamadas)**: se quita el último argumento (`img`, `tam`, `IMAGEN`, `IMAGEN_10` o `IMAGEN_1000`) de `screen_to_image_i32`; los valores esperados no cambian.

| Fichero | Prueba | Llamadas |
| --- | --- | --- |
| `view_i32.rs` | `fila_13_zoom_x4_con_origen_10_10` (+ `let (v, _)`) | 3 |
| `view_i32.rs` | `los_bloques_de_cuatro_pixeles_y_los_signos_de_cada_eje_son_independientes` (+ `let (v, _)`) | 8 |
| `view_i32.rs` | `dentro_de_la_imagen_coincide_con_screen_to_image` | 1 |
| `view_i32.rs` | `coincide_con_el_modelo_y_con_screen_to_image` (propiedad) | 2 |
| `view_ops.rs` | `fila_6_acercar_alejar_y_actual_no_mueven_el_pixel_bajo_el_centro` | 2 |
| `zoom_ladder.rs` | `fila_a3_screen_to_image_a_1_4_con_origen_10_20` | 3 |
| `zoom_ladder.rs` | `fila_a5_acercar_a_un_entero_conserva_el_pixel_exacto` | 2 |
| `zoom_ladder.rs` | `propiedad_p1_la_conversion_coincide_con_el_oraculo_en_los_70_niveles` | 1 |
| `zoom_ladder.rs` | `propiedad_p2_el_pixel_bajo_el_ancla_sube_a_la_rejilla_del_nivel_nuevo` | 5 |
| `zoom_ladder.rs` | `propiedad_p2_con_anclas_extremas_el_origen_no_desborda` | 1 |

(14 + 2 + 12 = 28 llamadas en pruebas; con la de `canvas.rs`, 29 en total.)

**`px-app/src/title.rs` (14 pruebas y 4 ayudas)**: el literal gana `tool`, y la herramienta que iba como argumento de `window_title`/`status_line` pasa a `tool: Some(..)` del literal.

| Módulo | Qué cambia |
| --- | --- |
| `tests` (unidad 6) | ayuda `base()` (`tool: None`, así las otras 5 pruebas del módulo no se tocan) y la prueba `sin_tamano_se_conserva_la_cadena_vacia_de_title_for` (literal completo, `tool: None`). Los textos esperados no cambian. |
| `window_title_tests` | ayuda `completo()` (`tool: Some(Line)`) y **las 6 pruebas**: `fila_22_titulo_completo_con_la_linea`, `fila_22_sin_color_solo_sale_la_herramienta` (cambia la herramienta del mismo `info` asignando `info.tool` entre aserciones, sin `Clone`), `fila_22_deshacer_y_rehacer_aparecen_solo_si_se_puede`, `fila_22_con_error_el_titulo_es_pixoko_y_el_error_y_nada_mas`, `el_relleno_con_la_ranura_transparente_y_la_herramienta_van_juntos` y `sin_tamano_se_conserva_la_cadena_vacia_y_se_anade_la_herramienta`. |
| `status_line_tests` | ayudas `completo()` (`tool: Some(Line)`) y `comprueba_titulo(tool, info, linea)` → `comprueba_titulo(info, linea)`, y **las 7 pruebas**: las seis `fila_10_*` y `propiedad_5_el_titulo_es_pixoko_mas_la_linea_de_estado` (la herramienta de la estrategia pasa al literal). |
| `file_title_tests` | **ninguna** (no usa `TitleInfo`). |

Además, los comentarios de cabecera de `window_title_tests` y `status_line_tests` dicen «`window_title(tool, info)`» y «no cambia `TitleInfo`»: dejan de ser ciertos y se reescriben (solo comentarios).

**Comprobación mecánica antes de cerrar la adaptación:** un script (fuera del repositorio, no se commitea) toma cada una de las 24 pruebas y las 4 ayudas antes (`HEAD`) y después, aplica un filtro fijo (quita el último argumento eliminado de `screen_to_image_i32`, el primer argumento `tool` de `window_title`/`status_line`/`comprueba_titulo`, los `tool: ..` de los literales y las asignaciones `x.tool = ..;`, y compara sin diferencias de espacios ni comas finales) y **falla si difieren**; además exige que **todas las demás funciones** de los cuatro ficheros, también las de producción de `title.rs`, queden idénticas. Su salida va en el mensaje del commit. Si algo no se automatiza, se dice y se revisa por diff. En las dos pruebas de `view_i32.rs` donde `img` pasa a `_`, el script comprueba que `img` no aparece en ningún otro sitio del cuerpo (ninguna aserción lo usaba) y el mensaje lo dice.

## Orden de commits

1. Spec (esta página y su fila en `INDEX.md`).
2. Adaptación de las 24 pruebas anteriores (lista de arriba; el mensaje da, de cada una, la aserción antigua y la nueva). **No compila** hasta el 4 y el 5.
3. Pruebas nuevas en rojo (8 filas y 2 propiedades), por `test-writer`.
4. Lógica en `px-ui`: `view.rs` y `canvas.rs`. Tras ella, `cargo test -p px-ui` en verde.
5. `px-app`: parte de producción de `title.rs` (incluida `app_title_info`) y `main.rs`.

En cada commit: `cargo fmt`, `clippy --all-targets` y `test` del crate tocado (en los commits en rojo, lo que compile).

Cierre: `cargo fmt`, `clippy --workspace --all-targets --locked -- -D warnings`, `cargo test --workspace --locked`, `cargo deny check` y el `license-auditor` una vez (aquí no hay dependencias ni formatos nuevos).

## Fuera de alcance

Quitar `status_text`, `set_image`, `set_owned_image` o cualquier otra limpieza; `convert_report`; renombrar `window_title`.

## Decisiones que necesito que confirmes

1. **`tool: Option<Tool>`, no `Tool`** (con la fila 8, que fija que en producción nunca es `None`). Con `Tool` obligatorio `title_text` escribiría siempre la herramienta y cambiarían los textos esperados de la unidad 6 (6 pruebas de `tests`), lo que prohibiste. El coste: en producción `tool` es siempre `Some`; el `None` solo representa «sin herramienta» (el título de la unidad 6).
2. **`window_title` y `status_line` pierden el argumento `tool`** (una sola fuente de la herramienta). D13 no lo preveía: añade 15 llamadas cambiadas en las pruebas de `title.rs`, todas solo de firma. Alternativa descartada: conservarlo y duplicar la fuente.
3. **Conservar `window_title` como `title_text(info)`** (nombres que citan varias specs) en vez de borrarlo; borrarlo cambia todavía más llamadas.
4. **Todo de una vez, sin convivencia**, con el commit de adaptación en rojo (razones arriba).
5. **Incluir `zoom_ladder.rs`** (5 pruebas, 12 llamadas) en la adaptación: D13 no la lista, pero el compilador la exige. **Y corregir D13** y los dos puntos de «API» de las notas de la 14 con las cifras de esta página, en el commit de la spec (dime si prefieres que no toque esos dos ficheros).
6. **`status_text` se queda** y la usa `title_text`, para no tocar `tool.rs` ni `text.rs`.
7. **Quién adapta:** `test-writer` hace la adaptación y las pruebas nuevas, y yo compruebo la lista y la comprobación mecánica de arriba antes de cada commit.

## Visto bueno (2026-10-20)

Decisiones 1 a 7 como están, con estos matices:

1. `tool: Option<Tool>`, con una prueba que fija que la construcción real en `px-app` nunca da `None` (fila 8, vía `app_title_info`) y que con `None` no se pinta herramienta (filas 1, 6 y 7).
3. `window_title` es alias de `title_text` con un comentario que dice por qué existe; se anota arriba que debe quitarse cuando las specs dejen de citarlo.
5. D13 y las notas de la 14 se corrigen con las cifras reales **en el commit de la spec**.
7. La comprobación mecánica es un script fuera del repositorio, no commiteado, con su salida en el mensaje del commit de adaptación.
8. En los dos casos de `view_i32.rs` donde `img` pasa a `_`, se confirma que la variable no se usaba en ninguna aserción y el mensaje del commit lo dice.

Commits: spec; adaptación de las 24 pruebas (en rojo); pruebas nuevas (en rojo, `test-writer`; el orden de `CLAUDE.md`, que el visto bueno no repetía); lógica de `px-ui`; `px-app`. Cierre completo y `license-auditor` una vez al final. No se toca `px-core` ni `px-raster`.
