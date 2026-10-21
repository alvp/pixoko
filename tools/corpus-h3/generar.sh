#!/usr/bin/env bash
# Corpus del hito 3 (specs/corpus-hito-3.md): genera en golden/aseprite/ los 28 casos con Aseprite
# por lotes y deja, para cada uno, el .aseprite, el PNG aplanado y el .sha256 del PNG.
#
# Uso:
#   tools/corpus-h3/generar.sh                  todos los casos (los 25 primeros deben salir IGUAL)
#   tools/corpus-h3/generar.sh --nuevos         solo los tres casos añadidos (los de NUEVOS)
#   tools/corpus-h3/generar.sh --caso NOMBRE    solo ese (se puede repetir; también valen nombres sueltos)
#   tools/corpus-h3/generar.sh --sobrescribir   (con cualquiera de los anteriores) deja que un caso
#                                               ya existente cambie si sale distinto
#   tools/corpus-h3/generar.sh --prueba         paso 0: versión y modo por lotes, sin tocar golden/
#   tools/corpus-h3/generar.sh --verificar      vuelca capas y celdas de lo ya generado
#
# Los casos se generan primero en una carpeta de prueba (en disco, no en /tmp) y se comparan **byte a
# byte** con lo que ya hay en golden/aseprite/: un caso existente que sale igual se informa como IGUAL;
# uno que sale distinto se informa como DISTINTO (qué cambia: .aseprite, PNG o los píxeles) y **no se
# sobrescribe** salvo con --sobrescribir; uno que no existe se copia. El script sale con 3 si algún
# caso existente salió distinto y no se sobrescribió.
# Variables: ASEPRITE (orden del programa, por defecto `aseprite`) y SALIDA (por defecto
# golden/aseprite).
#
# Cada caso lo dibuja generar.lua desde cero (colores y formas propios, paleta propia). El PNG sale
# de abrir el .aseprite ya guardado y exportarlo con --save-as (aplana las capas visibles). El hash
# lo calcula Pillow, no Pixoko: SHA-256 de la imagen en RGBA de 8 bits, sin cabecera, como en
# golden/png/. Los resultados no se commitean solos: se revisan y se commitean a mano.
set -euo pipefail

RAIZ="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
ASEPRITE="${ASEPRITE:-aseprite}"
SALIDA="${SALIDA:-$RAIZ/golden/aseprite}"
LUA="$RAIZ/tools/corpus-h3/generar.lua"

# Misma lista que la tabla de specs/corpus-hito-3.md (un caso por fila, 28 como máximo).
CASOS=(
  rgba-una-capa capas-normal-tres capas-opacidad capas-alfa-origen capas-alfa-fondo
  mezcla-normal mezcla-multiplicar mezcla-trama mezcla-superponer
  mezcla-oscurecer mezcla-aclarar mezcla-sumar mezcla-diferencia
  mezcla-ocho-opacidad mezcla-ocho-alfa mezcla-ocho-fondo-translucido
  indexado-capas indexado-transparente-7 indexado-paleta-256 indexado-celdas
  celda-desplazada celda-recortada celda-vacia capas-ocultas mixto-rgba
  mezcla-alfa-rejilla-multiplicar mezcla-alfa-rejilla-superponer fondo-con-opacidad
)
# Los tres añadidos después de los 25 primeros (corpus-hito-3.md, «Casos añadidos»).
NUEVOS=(mezcla-alfa-rejilla-multiplicar mezcla-alfa-rejilla-superponer fondo-con-opacidad)

falta() { echo "falta $1" >&2; exit 1; }
command -v "$ASEPRITE" >/dev/null || falta "$ASEPRITE (pasa ASEPRITE=/ruta/al/programa)"
command -v python3 >/dev/null || falta python3
python3 -I -c 'import PIL' 2>/dev/null || falta "Pillow (python3 -I -c 'import PIL')"

# SHA-256 de la RGBA de 8 bits de un PNG (alfa directo, filas de arriba abajo, sin cabecera).
hash_rgba() {
  python3 -I -c '
import hashlib, sys
from PIL import Image
print(hashlib.sha256(Image.open(sys.argv[1]).convert("RGBA").tobytes()).hexdigest())
' "$1"
}

if [[ "${1:-}" == "--prueba" ]]; then
  # No es /tmp (tmpfs): carpeta en disco, borrada al terminar.
  PRUEBA="${XDG_CACHE_HOME:-$HOME/.cache}/pixoko-corpus-h3-prueba"
  rm -rf "$PRUEBA"; mkdir -p "$PRUEBA"
  echo "== versión"; "$ASEPRITE" --version
  echo "== script de Lua por lotes"
  # Los --script-param van ANTES de --script: si no, app.params llega vacío.
  "$ASEPRITE" -b --script-param "salida=$PRUEBA" --script "$RAIZ/tools/corpus-h3/comprobar.lua"
  [[ -s "$PRUEBA/humo.aseprite" ]] || falta "humo.aseprite (el script no lo escribió)"
  echo "== exportar PNG aplanado"
  "$ASEPRITE" -b "$PRUEBA/humo.aseprite" --save-as "$PRUEBA/humo.png"
  python3 -I - "$PRUEBA/humo.png" <<'PY'
import sys
from PIL import Image
im = Image.open(sys.argv[1]).convert("RGBA")
assert im.size == (4, 4), im.size
assert im.getpixel((0, 0)) == (196, 46, 52, 255), im.getpixel((0, 0))
assert im.getpixel((3, 3)) == (196, 46, 52, 255), im.getpixel((3, 3))
print("centro (1,1):", im.getpixel((1, 1)), "(esperado: azul con alfa 128 sobre rojo, alfa 255)")
assert im.getpixel((1, 1))[3] == 255
print("prueba mínima correcta")
PY
  rm -rf "$PRUEBA"
  exit 0
fi

if [[ "${1:-}" == "--verificar" ]]; then
  compgen -G "$SALIDA/*.aseprite" >/dev/null || falta "ningún .aseprite en $SALIDA (genera primero)"
  for f in "$SALIDA"/*.aseprite; do
    "$ASEPRITE" -b "$f" --script "$RAIZ/tools/corpus-h3/verificar.lua"
  done
  exit 0
fi

SOBRESCRIBIR=0
ELEGIDOS=()
while [[ $# -gt 0 ]]; do
  case "$1" in
    --caso) [[ $# -ge 2 ]] || falta "un nombre tras --caso"; ELEGIDOS+=("$2"); shift 2 ;;
    --nuevos) ELEGIDOS+=("${NUEVOS[@]}"); shift ;;
    --sobrescribir) SOBRESCRIBIR=1; shift ;;
    -*) echo "opción desconocida: $1" >&2; exit 1 ;;
    *) ELEGIDOS+=("$1"); shift ;;
  esac
done
if [[ ${#ELEGIDOS[@]} -gt 0 ]]; then CASOS=("${ELEGIDOS[@]}"); fi
mkdir -p "$SALIDA"

# Carpeta de prueba en disco (no /tmp, que es tmpfs), borrada al terminar.
PRUEBA="${XDG_CACHE_HOME:-$HOME/.cache}/pixoko-corpus-h3-generar"
rm -rf "$PRUEBA"; mkdir -p "$PRUEBA"
trap 'rm -rf "$PRUEBA"' EXIT

IGUALES=(); DISTINTOS=(); COPIADOS=()
copia() { # caso hash
  cp "$PRUEBA/$1.aseprite" "$SALIDA/$1.aseprite"
  cp "$PRUEBA/$1.png" "$SALIDA/$1.png"
  echo "$2" > "$SALIDA/$1.png.sha256"
}

for caso in "${CASOS[@]}"; do
  nase="$PRUEBA/$caso.aseprite"; npng="$PRUEBA/$caso.png"
  ase="$SALIDA/$caso.aseprite"; png="$SALIDA/$caso.png"
  "$ASEPRITE" -b --script-param "caso=$caso" --script-param "salida=$PRUEBA" --script "$LUA"
  [[ -s "$nase" ]] || falta "$caso.aseprite (el script no lo escribió; mira el error de arriba)"
  "$ASEPRITE" -b "$nase" --save-as "$npng"
  [[ -s "$npng" ]] || falta "$caso.png (no se exportó)"
  nuevo_hash="$(hash_rgba "$npng")"
  if [[ -e "$ase" || -e "$png" ]]; then
    cambia=()
    cmp -s "$ase" "$nase" || cambia+=(".aseprite")
    cmp -s "$png" "$npng" || cambia+=("PNG")
    viejo_hash=""; [[ -s "$png.sha256" ]] && viejo_hash="$(tr -d '[:space:]' < "$png.sha256")"
    if [[ ${#cambia[@]} -eq 0 && "$viejo_hash" == "$nuevo_hash" ]]; then
      echo "$caso: IGUAL (mismos bytes del .aseprite y del PNG; hash $nuevo_hash)"
      IGUALES+=("$caso")
    else
      pixeles="los píxeles son los mismos"
      [[ "$viejo_hash" == "$nuevo_hash" ]] || pixeles="los píxeles CAMBIAN (hash $viejo_hash -> $nuevo_hash)"
      echo "$caso: DISTINTO en ${cambia[*]:-solo el .sha256}; $pixeles"
      DISTINTOS+=("$caso")
      if [[ $SOBRESCRIBIR -eq 1 ]]; then copia "$caso" "$nuevo_hash"; echo "  sobrescrito (--sobrescribir)"; fi
    fi
  else
    copia "$caso" "$nuevo_hash"
    echo "$caso: NUEVO $nuevo_hash"
    COPIADOS+=("$caso")
  fi
done

echo "== resumen: ${#IGUALES[@]} iguales, ${#COPIADOS[@]} nuevos, ${#DISTINTOS[@]} distintos"
if [[ ${#DISTINTOS[@]} -gt 0 ]]; then
  echo "distintos: ${DISTINTOS[*]}"
  [[ $SOBRESCRIBIR -eq 1 ]] || echo "no se tocó golden/ para ellos (usa --sobrescribir si el cambio es querido)"
fi
if [[ ${#COPIADOS[@]} -eq 0 && $SOBRESCRIBIR -eq 0 && -s "$SALIDA/README.md" ]]; then
  [[ ${#DISTINTOS[@]} -eq 0 ]] || exit 3
  echo "listo: nada nuevo que escribir (README.md intacto)"
  exit 0
fi

# README.md: versión y fecha reales de esta ejecución.
VERSION="$("$ASEPRITE" --version | head -n 1)"
FECHA="$(date +%F)"
TODOS=("$SALIDA"/*.aseprite)
{
  cat <<EOF
# golden/aseprite

Corpus del hito 3 (\`specs/corpus-hito-3.md\`): capas, opacidad, modos de mezcla, celdas con
desplazamiento y con recorte, capas ocultas, paleta y canal alfa, en ${#TODOS[@]} casos.

- **Programa:** $VERSION
- **Fecha:** $FECHA
- **Cómo se generó:** \`tools/corpus-h3/generar.sh\` (MIT, en este repositorio): un script de Lua
  propio crea cada caso desde cero y guarda el \`.aseprite\`; el PNG aplanado sale de abrir ese
  fichero y exportarlo con el modo por lotes del programa.
- **Origen de las imágenes:** son dibujos propios de prueba (rectángulos, discos y diagonales con
  colores definidos en el script). No se copió ningún recurso del programa: ni la paleta por
  defecto, ni imágenes, ni ficheros de ejemplo. Se publican bajo la licencia MIT del repositorio.
- **Hash:** cada \`X.png\` lleva al lado \`X.png.sha256\`: SHA-256 de la imagen expandida a RGBA de
  8 bits (alfa directo, sin cabecera), calculado con Pillow, como en \`golden/png/\`.

| Fichero | PNG aplanado |
| --- | --- |
EOF
  for f in "${TODOS[@]}"; do n="$(basename "$f" .aseprite)"; echo "| $n.aseprite | $n.png |"; done
} > "$SALIDA/README.md"
echo "listo: $SALIDA (revisa y commitea a mano)"
[[ ${#DISTINTOS[@]} -eq 0 || $SOBRESCRIBIR -eq 1 ]] || exit 3
