# Hito 4 · 4.1: textos de las operaciones de imagen (para revisión)

Copia de revisión de los textos de [`hito-4-imagen.md`](hito-4-imagen.md), tal como están en el código (`crates/px-ui/src/image_ops_es.rs`). La fuente de píxeles no tiene el signo de grado, así que los giros se nombran por dirección. Si cambias algo, se pasa al código en un commit aparte. Límites: nombre de ayuda ≤ 20 caracteres, descripción ≤ 8 líneas de 42, todo dentro de `INTERFACE_CHARSET`.

## Menú `Imagen` (en este orden, tras `Convertir modo…`)

Voltear horizontal · Voltear vertical · Girar a la derecha · Girar a la izquierda · Girar media vuelta · Tamaño del lienzo…

## Ventana `Tamaño del lienzo…`

- Título: Tamaño del lienzo. Botones: Aceptar, Cancelar.
- Campos: Ancho, Alto (de 1 a 16384) y Ancla, con nueve opciones: Arriba a la izquierda, Arriba, Arriba a la derecha, Izquierda, Centro (por defecto), Derecha, Abajo a la izquierda, Abajo, Abajo a la derecha.
- Aviso cuando la operación no cambia nada: «Sin cambios: la imagen queda igual».

## Ayuda (`F1` sobre la entrada del menú)

| Clave | Nombre | Descripción |
| --- | --- | --- |
| `image.flip-h` | Voltear horizontal | Da la vuelta a toda la imagen de izquierda a derecha, con todas las capas. Es un paso de deshacer. No tiene atajo por defecto: puedes asignarle uno. |
| `image.flip-v` | Voltear vertical | Da la vuelta a toda la imagen de arriba abajo, con todas las capas. Es un paso de deshacer. No tiene atajo por defecto: puedes asignarle uno. |
| `image.turn-right` | Girar a la derecha | Gira toda la imagen un cuarto de vuelta a la derecha, con todas las capas; el ancho y el alto se intercambian. Es un paso de deshacer. No tiene atajo por defecto: puedes asignarle uno. |
| `image.turn-left` | Girar a la izquierda | Gira toda la imagen un cuarto de vuelta a la izquierda, con todas las capas; el ancho y el alto se intercambian. Es un paso de deshacer. No tiene atajo por defecto: puedes asignarle uno. |
| `image.turn-half` | Girar media vuelta | Gira toda la imagen media vuelta, con todas las capas. Es un paso de deshacer. No tiene atajo por defecto: puedes asignarle uno. |
| `image.canvas-size` | Tamaño del lienzo | Cambia el ancho y el alto sin escalar la imagen. El ancla dice dónde queda lo que ya hay; lo que sobra se pierde y lo nuevo se rellena (transparente si el modo lo permite; si no, con el color de fondo). Un paso de deshacer. |
