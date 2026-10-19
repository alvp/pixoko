# Hito 2 · Unidad 20b: motivo con todos los requisitos y marcador de opciones

Estado: **aprobada** (2026-10-19; D1 a D6 con los matices del visto bueno, que **prevalecen sobre cualquier otra frase de la página**: (cuenta del pie) el pie mide 44 columnas en total y el prefijo `error: ` (7) está dentro: la lista de requisitos lleva `falta: ` (7) en su lugar y **no** `error: `, y los valores inválidos conservan `error: `; dos requisitos que no quepan con rótulos cortos pasan a la forma con cuenta; (D3) opción A, todo aditivo, y si alguna prueba de la 18, 19 o 20 necesitara más que un literal, me paro; (D4) el error del campo sigue junto al campo; (D5) el borde de las casillas también pasa a `TEXT_DIM`, medido: ninguna prueba lo fija). Cambios: `px-ui` y `px-app`. **No se toca `px-core` ni `px-raster`.** Sin dependencias nuevas ni `unsafe`. Nace de la prueba manual de la 20: con RGBA con transparencia a Gris, el pie solo decía «falta permitir perder el color»; mientras no se marcaba, elegir un fondo del alfa parecía no hacer nada. Base: [`hito-2-convertir-modo.md`](hito-2-convertir-modo.md) (D4, D6, filas 3 y 11) y [`hito-2-dialogos-modales.md`](hito-2-dialogos-modales.md) (pie, geometría, contraste).

## Objetivo

1. **El motivo del pie lista todo lo que falta**, en orden fijo (permisos, fondo del alfa, valores inválidos), en **una línea** de como mucho 44 columnas (el ancho del pie, con su prefijo).
2. **Los grupos de opciones excluyentes se dibujan redondos** (aro y punto) y las casillas siguen cuadradas, en todos los diálogos, con el mismo anillo de foco y contraste ≥ 3:1.

## Lo comprobado (y lo que no)

- **Anchura, cuenta exacta.** El pie mide `CONTENT_W_U` = 308 u = **44 columnas** de 7 u en total. `REASON_MAX_CHARS` = 37 es el motivo **sin** el prefijo `error: ` (7): `error: falta permitir perder el color` mide 37 en total (el motivo, 30). La lista lleva `falta: ` (también 7) en lugar de `error: `, así que cabe lo mismo: **44 columnas en total**, 37 tras el prefijo. «falta: permitir perder el color; elegir el fondo del alfa» (57) no cabe. El pie tiene 14 u de alto para el motivo: **dos líneas** exigirían `FOOTER_H_U` 45 → 59, que cambia el alto de todos los diálogos y la ventana mínima (ver D1).
- **Tres pendientes a la vez no pueden darse en este diálogo.** Un valor inválido (hexadecimal mal escrito, con alfa, o color activo con alfa) solo existe cuando ya se **ha elegido** un fondo; «falta el fondo» y «valor inválido» son excluyentes. Lo máximo real son **dos** (color + fondo, o color + valor). El caso de tres se prueba sobre la función pura, con una lista sintética (D4).
- **El hexadecimal inválido tapa hoy al motivo externo:** `Dialog::reason()` da primero el campo inválido, luego el `check` y por último `set_block`. Con la casilla sin marcar y un hexadecimal mal escrito, el pie solo dice «Color: …». Listar «valores inválidos» exige o una ampliación del motor (D3-A) o aceptar esa sombra (D3-B).
- **Marcador.** `draw_toggle` dibuja casillas y opciones iguales (cuadrado de 10 u; marca cuadrada de 6 u). **Medido** en una copia (fuera del repo) cambiando solo las opciones a aro + punto redondos: **0 pruebas existentes fallan** en `px-ui` y `px-app` (el banco de la 9 ya reconoce «círculos rellenos» y mide el relleno `ACCENT` de ≤ 10 u). Esa medición dejó el aro en `EDGE`; con el color de aro de D5 hay que repetirla al implementar.
- **Contraste, calculado:** `EDGE` (#424242) sobre `FIELD_FILL` (#1a1a1a) da **1,73:1**: un aro en `EDGE` no pasaría 3:1 (el borde de la casilla actual tampoco, y hoy ninguna prueba lo mide). `ACCENT` sobre `FIELD_FILL`, 8,98:1.
- **No comprobado:** nada visual; que un círculo de 10 u se vea limpio a ppp 1 (con `n = 1` son 10 px).

## Decisiones (a confirmar)

**D1. Una línea, no dos.** Pie de ≤ 44 columnas con rótulos cortos y, si no cabe, la forma con cuenta. Dos líneas cambiarían `FOOTER_H_U` y con él la geometría y las alturas de todos los diálogos y de la ventana mínima: un cambio de cifras en muchas pruebas de la 18, la 19 y la 20 para ganar 37 caracteres más. **Recomiendo una línea.**

**D2. Textos (orden fijo de los pendientes: color, fondo del alfa, valor inválido).** Prefijo: la lista de pendientes lleva `falta: ` y **no** `error: ` (no es un error del usuario; la pista es textual y el color no es lo único que la distingue); el valor inválido conserva `error: `. Pie total ≤ 44 columnas. Con **uno**, el rótulo largo; con **dos o más**, los rótulos cortos separados por `; `; si ni así caben, `faltan N: ` más las palabras `color`, `fondo`, `valor` separadas por `, `:
| Pendientes | Pie (todo lo pintado) |
| --- | --- |
| solo color | `falta: permitir perder el color` (31) |
| solo fondo | `falta: elegir el fondo del alfa` (31) |
| solo valor | con `error: `, el de hoy: `error: Color: …` (campo o `check`), `error: el color activo #rrggbbaa tiene alfa` o `error: no hay color activo`; el error del campo sigue **junto al campo** |
| color + fondo | `falta: perder el color; fondo del alfa` (38) |
| color + valor | `falta: perder el color; fondo válido` (36; el detalle del valor sigue junto al campo, o sale al marcar la casilla) |
| fondo + valor (solo función pura: en el diálogo no coexisten) | `falta: fondo del alfa; fondo válido` (35) |
| tres (función pura) | `faltan 3: color, fondo, valor` (29; la lista larga mide 52) |
Los motivos imposibles (`calculando el informe`, `más de 256 colores: sin cuantizador`, `la imagen convertida no cabe`) siguen solos y con `error: `, como hoy: no son requisitos que el usuario pueda cubrir. **Efecto en la 20:** los textos de uno (`falta permitir perder el color`, `elige el fondo para el alfa`) cambian a los de la tabla; es un literal, y por eso D6 lista más pruebas que en la propuesta.

**D3. Cómo entra «valor inválido» en la lista.** (A, aprobada) Tres ampliaciones aditivas del motor (**por sí solas no tocan ninguna prueba** de la 18, 19 ni 20; las 12 de D6 cambian por los textos, no por el motor): `Dialog::problems(&self) -> Vec<String>` (los motivos de campos y `check`, sin recortar, en el orden de hoy), `Dialog::set_summary(&mut self, Option<String>)` y `Dialog::summary(&self) -> Option<&str>`; con resumen, `reason()` lo devuelve **antes** que todo lo demás y el conjunto es inválido; el recorte de `set_block`, `set_summary` y `reason()` pasa a ser «37 caracteres, o 44 si el texto empieza por `falta: `» (el pie total, con su prefijo, nunca pasa de 44; los textos sin ese prefijo se recortan como hoy); `set_block` queda como está. La unidad 20 pone el resumen solo cuando hay **dos o más** pendientes y alguno es un valor inválido de campo (`problems()` no vacío); en los demás casos sigue usando `set_block` con el texto de D2 (un `falta: …` o un imposible). El `check` y los campos visibles son los de hoy. (B) No tocar el motor: el hexadecimal inválido sigue tapando la lista (el error del campo ya sale bajo él); solo se listan color y fondo. Cambiar el orden de `reason()` (el bloque primero) **no** es una opción: invierte aserciones de la 18 y me pararía.

**D4. Tres pendientes** se prueban con una función pura sobre una lista (`missing_text(&[Missing]) -> Option<String>`), porque en el diálogo real no son alcanzables (Lo comprobado). Las pruebas de diálogo cubren uno y dos.

**D5. Marcador y bordes.** Opciones: aro de 10 u (1 u de trazo) con el relleno del campo; la elegida lleva un punto relleno `FOCUS_RING` de 6 u de diámetro. Casillas: siguen cuadradas, con la marca cuadrada de 6 u. **El aro y el borde de la casilla usan el mismo color, `TOGGLE_EDGE = TEXT_DIM`** (≥ 3:1 sobre `FIELD_FILL`: 7,x) y entran en `CONTRAST_PAIRS` como un par nuevo (el array pasa de 10 a 11). **Medido** en una copia: el borde de la casilla en `TEXT_DIM` no rompe ninguna prueba (nadie lo fija), así que entra en esta unidad. El anillo de foco (rectángulo de 2 u, `ACCENT`, alrededor de la fila) es **el mismo** para opciones y casillas.

**D6. Pruebas anteriores que cambian: 12 de la 20, todas por un texto esperado** (el de uno o el de dos pendientes, o su prefijo `falta: ` en lugar de `error: `), en `px-app/src/convert_mode/` (medido con un parche mínimo en una copia; las ampliaciones del motor no cambian ninguna): `tests.rs`: `fila_3_lossy_de_solo_color_pide_la_casilla_y_nada_mas`, `fila_3_lossy_de_solo_alfa_pide_el_fondo_y_un_fondo_opaco_lo_cubre`, `fila_3_lossy_de_color_y_alfa_pide_los_dos_y_el_color_va_primero` (ahora el texto de «color + fondo»), `fila_3_matriz_completa_contra_el_oraculo_del_orden_de_d4` (el oráculo gana la fila de dos, como decía la propuesta), `fila_6_cambiar_de_destino_hace_una_llamada_con_el_destino_elegido`, `fila_6_cambiar_casillas_fondo_o_hexadecimal_no_llama_al_analizador`, `propiedad_1_el_informe_y_aceptar_dicen_la_verdad` y `propiedad_2_el_estado_del_formulario_no_se_corrompe` (constantes y oráculo, sin cambiar lo que afirman); `dialog_tests.rs`: `fila_11_con_un_destino_que_necesita_permiso_aceptar_esta_atenuado_con_el_motivo_visible`, `fila_11_gris_pide_la_casilla_y_el_fondo_y_cada_opcion_del_alfa_lo_cubre`, `fila_11_a_la_minima_el_motivo_y_los_botones_se_ven_con_el_cuerpo_al_final` y `fila_11_a_la_minima_la_rueda_desplaza_el_cuerpo_y_titulo_pie_y_botones_no_se_mueven`. Ninguna cambia cifras de geometría ni se debilita; el oráculo de dos pendientes **añade** una rama a una aserción, no la afloja. Si alguna necesitara más que un literal o esa rama, me paro. Van en un commit propio **antes** de las pruebas nuevas, con un mensaje que dice qué comprobaba cada una. *Marcador y bordes:* ninguna (medido con aro en `EDGE`, y con el borde de la casilla en `TEXT_DIM`; se repite con la lógica real), sin commit de adaptación.

## Piezas (firmas; las pruebas se escriben contra ellas)

```rust
// px-ui/src/dialog.rs (aditivas; D3-A)
impl Dialog { pub fn problems(&self) -> Vec<String>; pub fn set_summary(&mut self, s: Option<String>); pub fn summary(&self) -> Option<&str>; }
// px-ui/src/dialog_view.rs: aro y punto en las opciones; const TOGGLE_EDGE: Color32 = TEXT_DIM (aro y borde de casilla);
//   CONTRAST_PAIRS: [_; 11] (+ "borde de opción y casilla sobre un campo", TOGGLE_EDGE, FIELD_FILL, MIN_MARK);
//   pub const MISSING_PREFIX: &str = "falta: ";  pub fn footer_text(reason: &str) -> String  // MISSING_PREFIX → tal cual; si no, ERROR_PREFIX + motivo; ≤ 44 columnas
// px-ui/src/dialog.rs: el recorte de motivos es 37, o 44 con MISSING_PREFIX (REASON_MAX_CHARS sigue siendo 37).

// px-app/src/convert_mode.rs
pub enum Missing { Luma, AlphaBackground, Value }                      // en este orden; Ord
pub fn missing_text(items: &[Missing]) -> Option<String>;              // D2; None si está vacía; ≤ 37
pub fn pending(analysis: &Analysis, perms: &Permissions, active: Option<Rgba>, value_problem: bool) -> Vec<Missing>;
pub fn block_reason(..) -> Option<String>;   // misma firma: el texto de D2 (value_problem = false); imposibles y valores del color activo, como hoy
// sync_convert: set_block(block_reason(..)); y, si hay ≥ 2 pendientes y problems() no está vacío, set_summary(missing_text(pending(.., true))); si no, set_summary(None).
```

## Casos (12 filas; lógica pura y banco de egui, sin ventana)

| # | Entrada | Esperado |
| --- | --- | --- |
| 1 | **Un pendiente.** `missing_text` y `block_reason` con color, con fondo y con cada valor inválido (`Color: …`, activo con alfa, sin color activo) | `falta: permitir perder el color` y `falta: elegir el fondo del alfa` literales (con `falta: `, sin `error: `); el valor inválido, el texto de hoy (con `error: ` al pintarse) |
| 2 | **Dos pendientes.** color + fondo; color + valor (hexadecimal `zz`, `#ff000080`, activo con alfa); `Analysis` `Lossy(c/a)` sobre RGBA y sobre Indexado a Gris | `falta: perder el color; fondo del alfa` y `falta: perder el color; fondo válido` literales; permiso marcado o fondo elegido → vuelve al de uno |
| 3 | **Tres y recuento.** `missing_text` con las 7 listas no vacías posibles de `{Luma, AlphaBackground, Value}` (sin repetidos) y con una lista de 3 | 1 y 2 → D2; 3 → `faltan 3: color, fondo, valor`; vacía → `None`; el pie total (con su prefijo) siempre ≤ 44 columnas y en el juego |
| 4 | **Orden fijo.** `pending` con todas las combinaciones de `Analysis` × casilla × alfa × color activo × `value_problem` | Siempre en el orden color, fondo, valor; sin repetidos; vacío ⇔ la 20 diría `None`; imposibles → no entran (los motivos solos de D2) |
| 5 | **Oráculo de la 20.** `block_reason` con los permisos y análisis de la matriz de la 20, comparado con el oráculo nuevo escrito aparte | `None` ⇔ la 20; con un pendiente igual; con dos, el texto de dos; nunca dos motivos mezclados |
| 6 | **Motor (D3-A).** `problems` con campo inválido, con `check` y válido; `set_summary(Some(..))` con y sin campo inválido; `Accept`; `set_summary(None)`; `set_block` a la vez | `problems` en el orden de hoy y sin recorte; con resumen `reason()` es él (recortado a 37) y `Accept` da `Rejected`; `None` deja el diálogo `==` al de antes; `set_block` sigue como está; **las pruebas de la 18, 19 y 20 no se tocan** |
| 7 | **Diálogo real, el caso de la prueba manual.** RGBA con transparencia y color → Gris: abrir con un clic; sin marcar nada; marcar la casilla; elegir blanco; volver a «No perder el alfa»; hexadecimal `zz` con la casilla sin marcar | Pie `falta: perder el color; fondo del alfa`; tras la casilla `falta: elegir el fondo del alfa`; tras el fondo, sin motivo y `Aceptar` activo; con `zz` y sin casilla `falta: perder el color; fondo válido` **y** el error del campo (`Color: …`) sigue junto al campo; con la casilla marcada y `zz`, `error: Color: …`; el cambio se ve **en el fotograma siguiente** al evento |
| 8 | **Encaje del texto, medido.** Todos los pies de D2 y el **más largo posible** (un motivo recortado a 37 con `error: `, y la lista más larga de dos con `falta: `), con `footer_text`, a las cinco escalas, en la ventana mínima y en 960 × 720 | `footer_text` ≤ 44 columnas; el texto pintado mide como mucho `layout.reason.width` (44 × 7 u × n), sin recorte ni solapamiento con los botones; la pista `falta:` o `error:` está siempre en el texto (no depende solo del color); ≥ 4,5:1 (`ERROR_FG` sobre `DIALOG_FILL`); sin glifo ausente |
| 9 | **Marcador.** Nueva imagen, Convertir modo y el diálogo de prueba, a las cinco escalas: opción elegida, no elegida, casilla marcada y sin marcar | Opciones: un aro redondo (centro y ancho = 10 u) sin relleno de punto si no elegida y con **un** punto `ACCENT` de 6 u si elegida; exactamente un punto por grupo; casillas: cuadrado de 10 u con borde `TOGGLE_EDGE` y marca cuadrada de 6 u; opción y casilla **distinguibles** por forma, no solo por color |
| 10 | **Contraste.** El aro, el punto y el anillo de foco contra `FIELD_FILL` y `DIALOG_FILL`, a las cinco escalas | ≥ 3:1 los tres (`TEXT_DIM` 7,x; `ACCENT` 8,98), también el borde de la casilla; `TOGGLE_EDGE` está en `CONTRAST_PAIRS` con mínimo 3,0; el banco de la 9 sigue sin fallos |
| 11 | **Foco y entrada.** `Tab`, flechas y clic sobre opciones y casillas | El anillo de foco es el rectángulo de 2 u `ACCENT` de siempre, igual en opción y casilla; clic y teclado eligen como hoy; nada cambia en `Event` ni en el orden de foco |
| 12 | **Lo anterior no cambia.** Todo el espacio de trabajo | Verde salvo las pruebas de D6, que ya van adaptadas; la geometría y `FOOTER_H_U` idénticos |

## Propiedades (proptest; 256 casos en la 1 y la 2, las otras dos exhaustivas)

1. **El texto dice todo y cabe.** Para cualquier lista sin repetidos de `Missing` (0 a 3): `missing_text` es `None` ⇔ vacía; si no, empieza por `falta: ` o `faltan N: `, mide ≤ 44 columnas, está en el juego, y contiene el rótulo de **cada** pendiente en orden fijo o la forma con cuenta de N (nunca omite uno sin decirlo).
2. **`Aceptar` y la lista coinciden con `apply`.** Para imágenes aleatorias de hasta 6×6, cualquier destino, permisos y color activo: `block_reason` es `None` ⇔ `Command::ConvertMode` da `Ok` (la propiedad 1 de la 20 sigue valiendo), y el número de requisitos del texto es el de requisitos que realmente faltan.
3. **El resumen nunca contradice a la validez.** Para secuencias de 0 a 48 eventos (destino, casilla, alfa, hexadecimal válido e inválido) con `sync_convert` tras cada una: `summary` es `Some` ⇔ `!is_valid`, nunca hay un resumen con `Aceptar` activo ni `Aceptar` atenuado sin resumen, y `values()` es `Some` ⇔ no hay resumen.
4. **Un marcador por estado.** Para los cuatro estados (opción elegida o no × casilla marcada o no) y escalas 1 a 5 (exhaustivo): cada opción pinta un aro y como mucho un punto; cada casilla, un cuadrado; el anillo de foco es el mismo; contraste ≥ 3:1.

## Dónde van las pruebas y orden de trabajo

Lógica pura en `px-app/src/convert_mode/tests.rs` (filas 1 a 5, propiedades 1 a 3) y `dialog_tests.rs` (7 y 8); motor en `px-ui/tests/dialog_state.rs` (6); dibujo en `px-ui/tests/dialog_view.rs` (9 a 11, propiedad 4) y en los bancos de la 19 y la 20 para sus diálogos. **Commits, por separado y en este orden:** (a) esta spec aprobada, con su fila en `specs/INDEX.md`; **(1) motivo:** (b) adaptación de las 4 pruebas de la 20, con mensaje explicativo (rojas hasta la lógica); (c) pruebas nuevas del `test-writer`, en rojo, sin código mientras tanto; (d) lógica de `px-ui` (D3-A); (e) `px-app`: `missing_text`, `pending`, `block_reason` y su uso en `sync_convert`; **(2) marcador:** (f) pruebas nuevas del `test-writer`, en rojo; (g) lógica de `px-ui` (`draw_toggle`, `RADIO_EDGE`, `CONTRAST_PAIRS`); sin commit de adaptación si, repetida la medición, no hay pruebas anteriores que cambien. Durante la unidad, `cargo check -p` y `cargo test -p -q` de `px-ui` y `px-app`; al cerrar, `cargo fmt`, `cargo clippy --workspace -- -D warnings`, `cargo test --workspace --locked`, `cargo deny check` y el `license-auditor` una vez sobre el rango. No se toca `px-core` ni `px-raster`.

## Prueba manual (la haces tú, sin capturas; yo describo cómo)

`env -u WAYLAND_DISPLAY WINIT_X11_SCALE_FACTOR=1.5 cargo run --release -p px-app` (o `=2`).

1. **Lista.** RGBA con transparencia y color → `Imagen → Convertir modo…` → Gris: el pie dice `falta: perder el color; fondo del alfa` (sin `error:`). Marca la casilla: `falta: elegir el fondo del alfa`. Elige un fondo: sin motivo y `Aceptar` activo. Desmarca la casilla: `falta: permitir perder el color`.
2. **Valor inválido.** Con la casilla sin marcar, elige «un color hexadecimal» y escribe `zz`: el pie dice `falta: perder el color; fondo válido`, y bajo el campo sigue su error. `#ff000080`: lo mismo. Con la casilla marcada, solo `error:` y el motivo del valor.
3. **Ancho.** A 1,5 y 2,0 y con la ventana mínima, ningún motivo se corta ni toca los botones.
4. **Marcador.** En Nueva imagen (Modo y Fondo), Convertir modo (destino y alfa) y el diálogo de prueba: las opciones son círculos (la elegida con punto), las casillas cuadrados, y el aro y el borde de la casilla se ven con más claridad que antes; con `Tab` el anillo de foco rodea la fila igual en ambos; sin motivo para confundir una opción con una casilla.

## Fuera de alcance

Dos líneas de motivo y cambiar `FOOTER_H_U` (D1); cambiar el orden de `reason()` del motor; un tema claro; cambios en `px-core` y `px-raster`.
