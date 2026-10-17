#!/bin/bash
INPUT=$(cat)
TARGET=$(echo "$INPUT" | jq -r '[.tool_input.file_path, .tool_input.path, .tool_input.command, .tool_input.url] | map(select(. != null)) | join(" ")')
# La especificación pública del formato .aseprite sí se puede leer
echo "$TARGET" | grep -qi "ase-file-specs" && exit 0
for p in "GrafX2/grafX2" "LibreSprite/LibreSprite" "aseprite/aseprite" "dpaint"; do
  if echo "$TARGET" | grep -qi -- "$p"; then
    echo "Bloqueado: no se lee ni se clona código de programas de referencia ($p)" >&2
    exit 2
  fi
done
exit 0
