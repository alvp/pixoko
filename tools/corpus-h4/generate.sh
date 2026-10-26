#!/usr/bin/env bash
# Corpus del cuantizador (specs/corpus-hito-4-cuantizador.md): genera en golden/aseprite-quant/ las
# 6 imágenes y sus 36 versiones indexadas con Aseprite por lotes.
#
# Uso:
#   tools/corpus-h4/generate.sh --prueba       paso 0: versión y modo por lotes, sin tocar golden/
#   tools/corpus-h4/generate.sh                genera los 42 ficheros (no pisa los que ya existen)
#   tools/corpus-h4/generate.sh --sobrescribir lo mismo, pisando lo que haya
# Variables: ASEPRITE (orden del programa, por defecto `aseprite`) y SALIDA (por defecto
# golden/aseprite-quant). Los resultados no se commitean solos: se revisan y se commitean a mano.
set -euo pipefail

RAIZ="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
ASEPRITE="${ASEPRITE:-aseprite}"
SALIDA="${SALIDA:-$RAIZ/golden/aseprite-quant}"
LUA="$RAIZ/tools/corpus-h4/quantize.lua"

if [[ "${1:-}" == "--prueba" ]]; then
  "$ASEPRITE" --version
  "$ASEPRITE" -b --script-param salida=/dev/null --script <(echo 'print("modo por lotes: ok")')
  exit 0
fi

PRUEBA="$(mktemp -d "${HOME}/.cache/pixoko-corpus-h4.XXXXXX")"
trap 'rm -rf "$PRUEBA"' EXIT
mkdir -p "$SALIDA"
"$ASEPRITE" -b --script-param salida="$PRUEBA" --script "$LUA"

total=0
for f in "$PRUEBA"/*.png; do
  nombre="$(basename "$f")"
  total=$((total + 1))
  if [[ -e "$SALIDA/$nombre" && "${1:-}" != "--sobrescribir" ]]; then
    if cmp -s "$f" "$SALIDA/$nombre"; then echo "IGUAL    $nombre"; else echo "DISTINTO $nombre (no se pisa; usa --sobrescribir)"; fi
  else
    cp "$f" "$SALIDA/$nombre"
    echo "NUEVO    $nombre"
  fi
done
echo "$total ficheros (se esperan 42: 6 originales y 36 resultados)"
