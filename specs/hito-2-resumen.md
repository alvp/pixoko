# Hito 2 · Resumen del cierre

Solo notas: no es una spec de comportamiento ni tiene tests. Estado del hito: **cerrado**; el usuario confirmó la prueba manual de la unidad 22 ([`hito-2-puerta.md`](hito-2-puerta.md), lista de 15 pasos) en Linux, Windows y macOS. Orden y decisiones del reparto: [`hito-2-reparto-cierre.md`](hito-2-reparto-cierre.md). Cada punto remite a la spec que lo decidió, que sigue siendo la fuente; nada de lo que sigue es nuevo salvo las cifras de la unidad 22.

## Unidades

| Estado | Unidades (spec) |
| --- | --- |
| **Cerradas** | 2.1 [HSL](hito-2-hsl.md) · 2.2 [ventana](hito-2-ventana.md) · 2.3 [línea](hito-2-linea.md) · 2.4 [historia por regiones](hito-2-historia-regiones.md) · 2.5 [relleno](hito-2-relleno.md) · 2.6 [lápiz y deshacer](hito-2-lapiz-deshacer.md) · 2.7 [línea y relleno en la app](hito-2-linea-relleno-app.md) · 2.8 [color cercano](hito-2-color-cercano.md) · 2.9 [conversión de modos](hito-2-conversion-modos.md) · 2.10 [editar la paleta](hito-2-paleta-editar.md) · 2.11a a 2.11d ([texto](hito-2-texto.md), [panel de paletas](hito-2-panel-paletas.md), [editor de color](hito-2-editar-color.md), [operaciones de paleta](hito-2-paleta-operaciones.md)) · 2.15 [abrir y guardar](hito-2-abrir-guardar.md) · 2.16 [diálogos de archivo](hito-2-dialogos-archivo.md) · 2.17a [cuentagotas y borrador](hito-2-cuentagotas-borrador.md) · 2.17 [menú y herramientas](hito-2-menu-herramientas.md) · 2.17b [damero](hito-2-damero-lienzo.md) · 2.18 [diálogos modales](hito-2-dialogos-modales.md) · 2.19 [nueva imagen](hito-2-nueva-imagen.md) · 2.19b [color del fondo](hito-2-fondo-color.md) · 2.19c [contraste del menú](hito-2-contraste-menu.md) · 2.20 y 2.20b [convertir modo](hito-2-convertir-modo.md), [motivo y marcadores](hito-2-motivo-y-marcadores.md) · 2.21 [fijar transparente](hito-2-fijar-transparente.md) · 2.13b [chunks auxiliares](hito-2-chunks-auxiliares.md) · 2.12 [filas y bytes de `Image`](hito-2-image-filas-bytes.md) · 2.13 [PNG: rechazo antes de reservar](hito-2-png-flujo.md) · 2.14a [zoom y baldosas](hito-2-zoom-baldosas.md) · 14-limpieza [API](hito-2-limpieza-api.md) · 2.14c [topes del relleno](hito-2-limites-relleno.md) · 2.22 [puerta](hito-2-puerta.md) |
| **Descartada** | **2.14b** (índices y paleta en la GPU): [`hito-2-reparto-cierre.md`](hito-2-reparto-cierre.md) D2 y [`hito-2-notas-unidad-14.md`](hito-2-notas-unidad-14.md), «Idea de diseño». La prueba manual del usuario, tras la 14a, salió fluida con indexadas de 6000² y 8192² a zoom de trabajo y con la imagen entera visible; sin `px-render` ni `glow`/`egui_glow` directas. Se retoma solo si el ciclo de color de los hitos 5 o 7 lo exige |
| **Aplazadas** | Lista de abajo |

## Aplazado a los hitos siguientes

Cada fila cita su spec de origen (y su fila o sección). Las que vienen de una decisión del reparto llevan su `D`.

| Qué | Hito | Origen |
| --- | --- | --- |
| Pincel, rectángulo y elipse | 4 | [`hito-2-reparto-cierre.md`](hito-2-reparto-cierre.md), D1 y «Notas para el hito 4» |
| Cuantizador de color (RGBA o gris con alfa con más de 256 colores a indexado; paleta ajena, tramado) | 4 | [`hito-2-reparto-cierre.md`](hito-2-reparto-cierre.md), D8; [`hito-2-convertir-modo.md`](hito-2-convertir-modo.md), «Fuera de alcance» |
| Convertir todas las capas a otro modo de una vez (`ConvertAll`); `ConvertMode` en un `Batch` | 3 | [`hito-2-conversion-modos.md`](hito-2-conversion-modos.md), «Notas para el hito 3» y comportamiento 9; [`hito-2-convertir-modo.md`](hito-2-convertir-modo.md), «Fuera de alcance» |
| Capas, mezcla y composición; fotogramas | 3 y 5 | [`hito-1-document.md`](hito-1-document.md), comportamiento 1; [`hito-1-image.md`](hito-1-image.md) |
| **Partir `px-app` en lib y binario** (la puerta podría pasar a `px-golden`) | 3 (hecho en 3.0; la puerta a `px-golden`, en 3.9) | [`hito-2-puerta.md`](hito-2-puerta.md), D1 |
| **Marca de transparencia al guardar**: `write_png` escribe el índice transparente como alfa 0 de la paleta y `read_png` no recupera la marca (el aspecto no cambia) | 8 | [`hito-2-fijar-transparente.md`](hito-2-fijar-transparente.md), «Decisiones (resumen)» 6 y «Fuera de alcance» |
| Guardar en disco las preferencias, incluido el fondo del lienzo y su color; ranuras de color | 8 | [`hito-2-reparto-cierre.md`](hito-2-reparto-cierre.md), D12; [`hito-2-damero-lienzo.md`](hito-2-damero-lienzo.md), «Fuera de alcance»; [`hito-2-editar-color.md`](hito-2-editar-color.md), «Fuera de alcance» |
| **Escritor de PNG por flujo** (`write_png_to`, `write_atomic_with`): pospuesto el 2026-10-19; se retoma si hay un caso real de falta de memoria al guardar | 8 o antes | [`hito-2-notas-unidad-14.md`](hito-2-notas-unidad-14.md), «PNG»; [`hito-2-png-flujo.md`](hito-2-png-flujo.md) |
| **Relleno por filas** (`execute_fill` y `extend` con acceso por filas en vez de `get_pixel`/`set_pixel`); relleno fuera del hilo de la interfaz | candidato a una unidad de rendimiento | [`hito-2-notas-unidad-14.md`](hito-2-notas-unidad-14.md), «Rellenos» |
| **Perfil de color y gestión de color** (`iCCP`, `gAMA`, `cHRM`, `sRGB`, `sBIT`): hoy se ignoran al leer; con `iCCP` o color no sRGB, `Ctrl+S` guarda una copia y el resto solo se informa | cuando el documento guarde metadatos | [`hito-1-png.md`](hito-1-png.md), «Limitaciones»; [`hito-2-chunks-auxiliares.md`](hito-2-chunks-auxiliares.md), D2 |
| Conservar los chunks auxiliares al guardar (`pHYs`, texto, `tIME`, `eXIf`…), 16 bits y gris de menos de 8 bits al guardar | cuando el documento guarde metadatos | [`hito-1-png.md`](hito-1-png.md), «Limitaciones»; [`hito-2-abrir-guardar.md`](hito-2-abrir-guardar.md), «Limitaciones» |
| APNG (hoy se edita el primer fotograma) | 5 | [`hito-1-png.md`](hito-1-png.md), «Limitaciones» |
| Datos tras `IEND` (`TrailingData`): reconsiderar si los ficheros reales lo exigen | 6 | [`hito-1-png.md`](hito-1-png.md), «Limitaciones» |
| **Realce de la fila marcada de un menú con el ratón encima** (su relleno ya es `ACCENT`; valorar un borde o un tono más claro) | pulido de interfaz | [`hito-2-contraste-menu.md`](hito-2-contraste-menu.md), «Mejora posterior (anotada)» y D3 |
| **«Alfa base opaco por campo»** en el motor de diálogos de la 18, una muestra del color junto al campo y recordar el último color personalizado | pulido de interfaz | [`hito-2-fondo-color.md`](hito-2-fondo-color.md), «Fuera de alcance» |
| Campos numéricos editables y pasos con el teclado en los deslizadores del editor de color | pulido de interfaz | [`hito-2-editar-color.md`](hito-2-editar-color.md), «Fuera de alcance» |
| Seguir a la entrada al deshacer un `Move` (historia de selecciones por `state_id`) | pulido de interfaz | [`hito-2-paleta-operaciones.md`](hito-2-paleta-operaciones.md), «Pulido posterior» |
| Orden de `Cancelar` y `Aceptar` en macOS | pulido de interfaz | [`hito-2-dialogos-modales.md`](hito-2-dialogos-modales.md), D7 («Pulido posterior») |
| «Suavizar al alejar» (promedio o mipmaps) como opción | unidad propia o 14b | [`hito-2-zoom-baldosas.md`](hito-2-zoom-baldosas.md), «Muestreo» |
| Otro cuentagotas temporal (con otra combinación que `Alt`+clic) | pulido de interfaz | [`hito-2-cuentagotas-borrador.md`](hito-2-cuentagotas-borrador.md), decisiones 6 y 8 |
| Paleta en el sombreador (índices en la GPU) | 5 o 7, solo si el ciclo de color lo exige | [`hito-2-notas-unidad-14.md`](hito-2-notas-unidad-14.md), «Idea de diseño» |
| `convert_report` (la pasada 1 de `convert_image` sin construir la imagen; el informe a 8192² cuesta hasta 0,86 s) | candidato en `px-core` | [`hito-2-notas-unidad-14.md`](hito-2-notas-unidad-14.md), «API»; [`hito-2-convertir-modo.md`](hito-2-convertir-modo.md), D3 |
| Ficheros recientes y directorio recordado entre sesiones; cancelar desde la app un diálogo abierto de Linux; un portal sin backend o muy lento (sin probar) | 8 | [`hito-2-dialogos-archivo.md`](hito-2-dialogos-archivo.md), «Limitaciones» |
| `fsync` del directorio tras el `rename`, limpiar temporales viejos, guardado automático, pestañas | 8 | [`hito-2-abrir-guardar.md`](hito-2-abrir-guardar.md), «Limitaciones» y «Fuera de alcance» |

Los dos primeros puntos de «Rellenos» de las notas (tope de tramos y pico del peine) **no** están aplazados: los cerró la 14c.

## Cifras medidas

Solo las que ya constan en una spec, con su enlace; las de la unidad 22 son nuevas.

| Qué | Antes → después | Fuente |
| --- | --- | --- |
| `write_png`, pico sobre la `Image` a 8192² | RGBA 257 → 1 MiB; indexado 63 → 0 MiB | [`hito-1-png.md`](hito-1-png.md), «Para el hito 2» (unidad 12) |
| `read_png` con 8 bits, pico a 8192² | RGBA 511 → 256 MiB; indexado 127 → 64 MiB | ídem |
| `read_png` de un fichero falso de 66 bytes que declara gris de 32768² | 328 ms y +1024 MiB → 0 ms y +0,3 MiB | [`hito-2-png-flujo.md`](hito-2-png-flujo.md) (unidad 13) |
| Relleno: peor caso que deja pasar el tope de 3 000 000 tramos (peine a 8192²) | +1,25 GiB y 2,5 s | [`hito-2-limites-relleno.md`](hito-2-limites-relleno.md) (14c) |
| Relleno de 8192² enteros | ≈ 1,7 s (RGBA; puede duplicarse en una máquina lenta) | [`hito-2-notas-unidad-14.md`](hito-2-notas-unidad-14.md), «Rellenos» |
| Vista previa del editor de color, 8192² indexado, CPU | hoy 128 ms → 2,6 a 14,6 ms con 1 a 4 baldosas visibles y 43,6 ms con las 16 | [`hito-2-zoom-baldosas.md`](hito-2-zoom-baldosas.md) (14a) |
| Crear una imagen nueva de 8192² RGBA | ≈ 0,9 s → ≈ 0,15 s con `Image::filled` y `try_clone` | [`hito-2-notas-unidad-14.md`](hito-2-notas-unidad-14.md), «API» (unidad 12) |
| Escritura por flujo (pospuesta) | RGBA 8192² con ruido: 7549 → 7361 ms y +439 → +0,5 MiB; con contenido que comprime, sin ganancia | [`hito-2-notas-unidad-14.md`](hito-2-notas-unidad-14.md), «PNG» |
| **Unidad 22: lienzos grandes** (release, un hilo, Linux) | RGBA 8192²: relleno 702 ms, `undo` del relleno 143 ms, `redo` 137 ms, `save` 327 ms, relectura 40 ms; pico de RSS 271 MiB. Indexada 6000²: relleno 313 ms, `undo` 52 ms, `save` 60 ms, relectura 20 ms; pico 46 MiB. Salvo el relleno, `undo` y `redo` de los demás gestos cuestan microsegundos. La comprobación contra la fórmula cerrada, en cambio, tarda unos 150 ms (RGBA) y 80 ms (indexada) por vez. Medido con `cargo test --release -p px-app -- --ignored --test-threads=1`, `ru_maxrss` del proceso | [`hito-2-puerta.md`](hito-2-puerta.md), fila 12 |
| **Unidad 22: fuzzer, 10 minutos** | 12 733 737 ejecuciones en 601 s, sin ningún fallo y con `golden/` intacto (2026-10-20, lo lanzó el usuario) | [`hito-2-puerta.md`](hito-2-puerta.md), «Resultado del fuzzer» |
| **Pruebas del workspace** | 3379 pasadas, 0 falladas y 12 ignoradas (los lienzos grandes y bancos de medida) | `cargo test --workspace --locked` |

## `cargo deny` al cerrar

`advisories ok, bans ok, licenses ok, sources ok`; 17 avisos `duplicate` aceptados (línea base en [`docs/DEPENDENCIES.md`](../docs/DEPENDENCIES.md)); la excepción NCSA de `libfuzzer-sys` vive en `fuzz/deny.toml` y el fuzzer se comprueba con `cargo deny --manifest-path fuzz/Cargo.toml --locked --config fuzz/deny.toml check` ([`hito-2-puerta.md`](hito-2-puerta.md), D9).
