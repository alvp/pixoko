# Hito 4 · Unidad 4.2: textos para revisión

Todos los textos que ve el usuario de `Reducir colores…`, en `crates/px-ui/src/quantize_es.rs` (y dos frases de `Convertir modo…` en `crates/px-app/src/convert_mode.rs`). Palabras propias. Límites que comprueban las pruebas: etiqueta de campo numérico ≤ 12 caracteres, resto ≤ 42, nombre de ayuda ≤ 20, descripción de ayuda ≤ 8 líneas de 42, todo dentro de `INTERFACE_CHARSET`.

| Dónde | Texto |
| --- | --- |
| Menú `Imagen` | Reducir colores… |
| Título del diálogo | Reducir colores |
| Botones | Aceptar · Cancelar |
| Campo numérico (2 a 256, 256) | Colores |
| Grupo | Paleta: «Calculada de la imagen» · «Guardada de la sesión» (esta solo si la sesión ya vio una paleta indexada) |
| Grupo | Tramado: «Ninguno» · «Difusión de error» · «Ordenado» |
| Campo numérico (1 a 255, 128) | Alfa mínimo |
| Informe (solo lectura) | Imagen de 8×8, 3 capas. Paleta guardada: 8 colores. aviso: más de 16777216 px; puede tardar |
| Aviso (modo no válido) | Reducir colores: solo RGBA o gris con alfa |
| Aviso (capas con opacidad o mezcla) | el mismo motivo que `Convertir modo…` a Indexado |
| Barra de estado (imagen grande) | Reduciendo colores… |
| Aviso de 5 s al terminar | colores reducidos a 16 (error medio 3,4) |
| Botón en `Convertir modo…` (destino Indexado, RGBA o gris con alfa) | etiqueta «Colores», texto «Reducir colores…» |
| Motivo de `Aceptar` atenuado con más de 256 colores | más de 256 colores: reduce colores |
| Informe con más de 256 colores | Esta imagen tiene más de 256 colores y una paleta indexada no los admite. Pulsa Reducir colores… para elegir cuántos conservar. |
| Ayuda (`F1`), nombre | Reducir colores |
| Ayuda, descripción | Pasa toda la imagen (RGBA o gris con alfa, con todas sus capas) a indexado con los colores que elijas. La paleta se calcula o se toma la guardada de la sesión; el tramado reparte el error y el alfa mínimo decide qué píxeles quedan transparentes. Un paso de deshacer. Sin atajo por defecto: puedes asignarle uno. |

Notas: las etiquetas «Colores» y «Alfa mínimo» son más cortas que las de la spec («Número de colores», «Alfa mínimo para ser opaco») porque el formulario solo admite 12 caracteres en un campo numérico; el alfa mínimo se explica en la ayuda.
