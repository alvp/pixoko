# Iconos de Pixoko

Los 22 iconos de la interfaz son **originales de Pixoko**, **dibujados nuevos** para este proyecto y
se distribuyen bajo la **licencia MIT** del proyecto, como el resto del repositorio. No proceden de
ningún otro programa, tema ni biblioteca de iconos y no se han copiado, traducido ni calcado de
ninguno.

## Formato

- PNG de 16 × 16 en `assets/icons/<nombre>.png`.
- De 1 bit en lo que dibujan: cada píxel es opaco y blanco (`255, 255, 255, 255`) o transparente
  (alfa 0). La interfaz los tiñe con el color del tema (`TEXT`, `ACCENT`, `TEXT_DISABLED`) y los
  pinta a escala entera con muestreo de vecino más cercano.
- Estilo: trazo grueso de 2 px, margen de 1 px, diagonales a 45 grados y siluetas claras, sobre un
  botón cuadrado con relieve.
- Se incrustan con `include_bytes!` en `px-app` y se decodifican al arrancar con `px-formats`. Se
  pueden retocar con Pixoko: vale cualquier PNG que `px-formats` lea (RGBA, indexado con
  transparencia o gris con alfa); solo cuenta el alfa y el color del retoque da igual.

## Los 22 iconos

| Fichero | Motivo | Dónde |
| --- | --- | --- |
| `lapiz.png` | lápiz en diagonal con la punta abajo a la izquierda | caja de herramientas |
| `linea.png` | diagonal con un punto grueso en cada extremo | caja de herramientas |
| `relleno.png` | bote inclinado derramando una gota | caja de herramientas |
| `cuentagotas.png` | pipeta en diagonal con la punta abajo | caja de herramientas |
| `borrador.png` | bloque con la cara superior inclinada | caja de herramientas |
| `acercar.png`, `alejar.png` | lupa con un más o con un menos | botones de zoom |
| `ajustar.png` | cuatro esquinas | botones de zoom |
| `uno-a-uno.png` | los dígitos «1:1» en píxeles | botones de zoom |
| `anadir.png`, `quitar.png` | un más; una cruz | editor de la paleta |
| `rotar-izquierda.png`, `rotar-derecha.png` | flecha circular de tres cuartos de vuelta en cada sentido | editor de la paleta |
| `transparente.png` | cuadrado a cuadros pequeños con una esquina marcada | editor de la paleta |
| `ojo.png`, `ojo-cerrado.png` | ojo con pupila; párpado cerrado con tres pestañas | ojo de cada capa del panel de capas |
| `candado.png`, `candado-abierto.png` | candado cerrado; candado con el arco levantado | candado de cada capa del panel de capas |
| `duplicar.png` | dos cuadrados desplazados, el de delante tapa al de detrás | botón Duplicar del panel de capas |
| `subir.png`, `bajar.png` | flecha hacia arriba; flecha hacia abajo | botones Subir y Bajar del panel de capas |
| `intercambiar.png` | dos flechas horizontales opuestas, la de arriba hacia la derecha y la de abajo hacia la izquierda | botón de intercambiar primer plano y fondo, junto a las dos barras de color |

## Reservado

`pincel.png` (el icono único de pincel del hito 4, que mostrará el pincel activo) está **reservado y
sin dibujar**: no hay fichero ni `Icon::Brush` todavía. La prueba de `px-app` exige que en esta
carpeta no haya más ficheros que los 22 de arriba y este `README.md`, así que al dibujarlo hay que
darlo de alta en `px-ui::icons`.
