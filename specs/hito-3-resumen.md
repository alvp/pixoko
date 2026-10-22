# Hito 3 · Resumen del cierre

Solo notas: no es una spec de comportamiento ni tiene tests. Estado del hito: **cerrado** (22 de octubre): la prueba manual de la unidad 3.9 ([`hito-3-puerta.md`](hito-3-puerta.md)) salió bien en Linux, Windows y macOS y los dos fuzzers de 10 minutos terminaron sin crash; los resultados están en esa spec. Reparto y decisiones: [`mapa-funciones.md`](mapa-funciones.md) (sección 3 y decisiones D02 y D04 a D10). Cada punto remite a la spec que lo decidió, que sigue siendo la fuente; nada de lo que sigue es nuevo salvo las cifras de la fila 10 de la 3.9 y los dos hallazgos de «Defectos conocidos».

## Unidades

| Estado | Unidades (spec) |
| --- | --- |
| **Cerradas** | 3.0 [`px-app` en lib y binario](hito-3-px-app-lib.md) · 3.1 [modelo de capas](hito-3-capas-modelo.md) · 3.2 [mezcla](hito-3-mezcla.md) · 3.3 [vista y guardado con capas](hito-3-capas-vista.md) · 3.4 [interfaz de capas](hito-3-capas-interfaz.md) · 3.5 [combinar, aplanar y `ConvertAll`](hito-3-combinar.md) · 3.8 [`.aseprite`, etapa 1](hito-3-aseprite.md) · 3.9 [puerta](hito-3-puerta.md) · corpus de 29 casos de Aseprite ([`corpus-hito-3.md`](corpus-hito-3.md), no es una unidad) |
| **Aplazadas fuera del hito** | **3.6 página de reserva** (D09, spec sin escribir) y **3.7 imágenes de referencia** (D11, spec sin escribir): deseables, no estaban en la puerta del hito y el usuario las dejó fuera el 22 de octubre ([`hito-3-puerta.md`](hito-3-puerta.md)); origen: filas 3.6 y 3.7 de [`mapa-funciones.md`](mapa-funciones.md). La 3.6 reserva la tecla `j` ([`hito-3-capas-interfaz.md`](hito-3-capas-interfaz.md), decisión 2) |
| **Aplazadas a una unidad corta entre hitos** | La lista única de limpiezas pendientes: [`mapa-funciones.md`](mapa-funciones.md), sección 11 (`window_title`, `interact` e `interact_ordered`, `info_flags*`, variantes de `Analysis` y `DialogKind`, el nombre `CelMismatch`, `main_image` y las vistas antiguas de la tabla de atajos) |

## Aplazado a los hitos siguientes

Cada fila cita su spec de origen y su sección. Salen de leer `specs/hito-3-capas-*.md` y los «Fuera de alcance» de 3.2, 3.5 y 3.8, y de un `grep` de «pulido», «más adelante» y «TODO» sobre `specs/` y `docs/` (el 22 de octubre); no se ha inventado ninguna.

| Qué | Hito | Origen |
| --- | --- | --- |
| Fotogramas, duraciones, etiquetas y celdas enlazadas en el `.aseprite` (hoy solo se lee el primero y se avisa; `Ctrl+S` guarda una copia) | 5 (5.5) | [`hito-3-aseprite.md`](hito-3-aseprite.md), «Fuera de alcance» |
| Gris con alfa del `.aseprite` (la profundidad de 16 bits es valor + alfa = `GrayAlpha`; caso de corpus `gris-alfa-capas`), grupos, mapas de tiles, capa de referencia, opacidad de celda, z-index, los diez modos de mezcla restantes, paleta de 6 bits, perfil ICC, propiedades de usuario y conservar los chunks no entendidos | 6 (6.1) | [`hito-3-aseprite.md`](hito-3-aseprite.md), «Fuera de alcance» y decisión 8; [`hito-3-mezcla.md`](hito-3-mezcla.md), «Fuera de alcance» |
| Fotogramas y mesa de luz sobre la composición | 5 | [`hito-3-mezcla.md`](hito-3-mezcla.md), «Fuera de alcance»; [`hito-3-capas-modelo.md`](hito-3-capas-modelo.md), «Fuera de alcance» |
| Serialización de comandos | 8 | [`hito-3-capas-modelo.md`](hito-3-capas-modelo.md), «Fuera de alcance» |
| Preferencias del panel de capas en disco | 8 | [`hito-3-capas-interfaz.md`](hito-3-capas-interfaz.md), «Fuera de alcance» |
| Barra de progreso (hoy «Guardando…», «Combinando…», «Aplanando…» y «Exportando…» con dos fotogramas) | 8 | [`hito-3-combinar.md`](hito-3-combinar.md), «Fuera de alcance»; [`hito-3-capas-vista.md`](hito-3-capas-vista.md), decisión 4; **riesgo medido en la 3.9** (abajo) |
| Teclas por defecto de combinar, combinar visibles y aplanar (hoy sin tecla, editables) | pulido de interfaz | [`hito-3-combinar.md`](hito-3-combinar.md), «Fuera de alcance» (D10) |
| Combinar varias capas elegidas a mano | por decidir | [`hito-3-combinar.md`](hito-3-combinar.md), «Fuera de alcance» |
| Miniaturas en el panel de capas | por decidir | [`hito-3-capas-interfaz.md`](hito-3-capas-interfaz.md), decisión 3 y «Fuera de alcance»; [`hito-3-combinar.md`](hito-3-combinar.md), «Fuera de alcance» |
| Arrastrar capas en la lista para reordenarlas, selección de varias capas, capa siguiente y anterior por teclado, opción persistente del cuentagotas | por decidir | [`hito-3-capas-interfaz.md`](hito-3-capas-interfaz.md), «Fuera de alcance» |
| Vista previa en vivo de la opacidad (hace falta fusionar pasos en `px-core`) | por decidir | [`hito-3-capas-interfaz.md`](hito-3-capas-interfaz.md), «Fuera de alcance» |
| El selector de modo de mezcla (`Popup::menu`) no tiene el aspecto de píxeles del resto | pulido de interfaz | [`hito-3-capas-interfaz.md`](hito-3-capas-interfaz.md), riesgo 8 |
| Composición por bandas con `std::thread::scope` y caché de baldosas compuestas, solo si la medida lo exige; repartir la composición de los bloques nuevos entre dos fotogramas | por decidir (rendimiento) | [`hito-3-capas-vista.md`](hito-3-capas-vista.md), «Fuera de alcance» y «Rendimiento» (puntos 4 y 5); [`hito-3-mezcla.md`](hito-3-mezcla.md), «Fuera de alcance» y corrección 4 |
| Pulidos de interfaz de antes que tocan capas: seguir a la entrada al deshacer (la capa activa ya lo hace; queda `Move` de la paleta) | 8 (8.8) | [`mapa-funciones.md`](mapa-funciones.md), sección 9 |

## Defectos conocidos (hallazgos de la 3.9; ninguno se arregla en el hito)

| Qué | Dónde | Origen |
| --- | --- | --- |
| Un indexado **sin índice transparente** con una sola capa (un PNG indexado sin `tRNS`, o un RGBA sin alfa 0 convertido) al que se **oculta la única capa** da `ComposeError::NoTransparency`. Es lo que especifica 3.2, pero el lienzo lo traduce a «sin memoria para mostrar la imagen» y guardar o exportar da el error de composición. Es una decisión de diseño por tomar (impedir ocultar la última capa visible, o mostrar vacío): por leer del código, no probado con ventana | `px_ui::source` (comentario de la línea 146), `Session::set_layer_visible` | [`hito-3-mezcla.md`](hito-3-mezcla.md), «Indexado y grises»; [`hito-3-puerta.md`](hito-3-puerta.md), corrección 4 |
| El RGB bajo alfa 0 fuera del rectángulo mínimo no se guarda en el `.aseprite`; no afecta a nada visible (límite conocido, [`docs/DISENO-INTERFAZ.md`](../docs/DISENO-INTERFAZ.md)): 4 casos del corpus y 133 píxeles, todos con alfa 0 | `write_aseprite` | [`hito-3-aseprite.md`](hito-3-aseprite.md), decisión 9 |
| El techo de error 2 de la fórmula empírica de la mezcla (fondo translúcido y modo distinto de Normal) es un máximo observado, no una garantía: si un `.aseprite` con otros pares de alfa lo supera, se re-evalúa con datos y nunca se ensancha | `px_raster::composite_pixel` | [`hito-3-mezcla.md`](hito-3-mezcla.md), «Datos conocidos» |

## Cifras medidas

Solo las que ya constan en una spec, con su enlace; las de la unidad 3.9 son nuevas.

| Qué | Cifra | Fuente |
| --- | --- | --- |
| Composición completa de 8192² con 8 capas (solo exportar y aplanar) | hasta 7,2 s; baldosa de 2048² con 8 capas, 11 a 453 ms; trazo de 64×64, ≤ 0,43 ms | [`hito-3-mezcla.md`](hito-3-mezcla.md), corrección 4 |
| Vista con 8 capas de 8192² | fotograma ≤ 9,7 ms; recomponer al soltar o deshacer ≤ 127 ms; peor caso de una baldosa 583 ms | [`hito-3-capas-vista.md`](hito-3-capas-vista.md) |
| Combinar abajo, combinar visibles y aplanar, 8 × 8192² | 0,24 s, 3,9 s y 9,2 s; `ConvertAll` de 8 × 4096² a gris con alfa, 0,73 s | [`hito-3-combinar.md`](hito-3-combinar.md) |
| Fuzz de `read_aseprite` | 520 652 ejecuciones en 60 s y 578 371 en 10 minutos, sin fallos | [`hito-3-aseprite.md`](hito-3-aseprite.md), «Resultados» |
| **Unidad 3.9: fuzzers de cierre** | `read_png` y `read_aseprite`, 601 s cada uno, sin crash; ejecuciones: no anotado | [`hito-3-puerta.md`](hito-3-puerta.md), «Resultado de los fuzzers» |
| **Unidad 3.9: RGBA 8192² con 4 capas** (release, un hilo, Linux) | pintar 1,63 s; combinar visibles 0,78 s; `undo` y `redo` en microsegundos; exportar el PNG aplanado 1,14 s; **guardar el `.aseprite` 1,44 s** (912 639 bytes); reabrir 0,59 s; pico de RSS de todo el recorrido **1571 MiB** | [`hito-3-puerta.md`](hito-3-puerta.md), corrección 5 |
| **Unidad 3.9: indexado 6000² con 4 capas** | pintar 0,66 s; combinar visibles 0,06 s; exportar 0,11 s; guardar 0,25 s (122 944 bytes); reabrir 0,20 s; pico **231 MiB** | ídem |
| **Unidad 3.9: peor caso de guardar** (4 capas de 8192² RGBA de ruido, que no comprimen) | escribir **29,5 s**, 879 MiB de salida y pico de 2421 MiB; abrir 2,8 s | ídem |

**Riesgo para el hito 8 (barra de progreso):** guardar un `.aseprite` de ruido de 8192² con 4 capas son 29,5 s (menos de un minuto); con el tope de 2 GiB de celdas (8 capas) el ruido llegaría, por **extrapolación lineal sin medir**, a ~59 s. Hoy el guardado dice «Guardando…» antes de empezar (la 3.9 lo añadió para el `.aseprite`) pero no hay progreso ni se puede cancelar.

## `cargo deny` al cerrar

Medido el 22 de octubre con `cargo deny --locked check`: `advisories ok, bans ok, licenses ok, sources ok` y **los mismos 17 avisos `duplicate`** de la línea base de [`docs/DEPENDENCIES.md`](../docs/DEPENDENCIES.md); el hito 3 no añadió ninguno (`miniz_oxide` ya estaba). **Decisión (22 de octubre): se aceptan sin `skip`** (un `skip` taparía un duplicado nuevo) y se vuelven a mirar al actualizar `eframe`, `rfd` o `png`. El fuzzer se comprueba con `cargo deny --manifest-path fuzz/Cargo.toml --locked --config fuzz/deny.toml check`.

## Cierre del hito

Sin nada pendiente (22 de octubre). Resultados completos, tal como los comunicó el usuario, en «Resultado de la prueba manual» y «Resultado de los fuzzers» de [`hito-3-puerta.md`](hito-3-puerta.md):

1. **Prueba manual:** Aseprite abre lo que escribe Pixoko en Linux (1.3.18.6-dev), Windows (1.3) y macOS (1.3); `Cmd+Q` con cambios sin guardar en macOS: avisa como la x.
2. **Fuzzers de 10 minutos:** `read_png` y `read_aseprite`, 601 s cada uno, sin crash; número de ejecuciones: no anotado.
