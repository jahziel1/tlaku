#!/usr/bin/env bash
# Crea o actualiza las etiquetas de .github/labels.yml en el repositorio con `gh label create --force`.
# Creates or updates the labels from .github/labels.yml with `gh label create --force`.
#
#   scripts/sync-labels.sh [propietario/repositorio]    (por omisión jahziel1/tlaku)
#   scripts/sync-labels.sh --dry-run                    (solo muestra los comandos)
#
# Requiere `gh` con sesión iniciada (gh auth login). No borra etiquetas existentes.
set -euo pipefail

repo="jahziel1/tlaku"
dry=0
for arg in "$@"; do
  case "$arg" in
    --dry-run) dry=1 ;;
    -h|--help) sed -n '2,8p' "$0"; exit 0 ;;
    *) repo="$arg" ;;
  esac
done

root="$(cd "$(dirname "$0")/.." && pwd)"
file="$root/.github/labels.yml"
[ -f "$file" ] || { echo "No existe $file" >&2; exit 1; }
if [ "$dry" -eq 0 ]; then command -v gh >/dev/null || { echo "Falta gh (https://cli.github.com)" >&2; exit 1; }; fi

# El archivo tiene un formato fijo (name, color, description_en, description_es), sin anidar.
name=""; color=""; en=""; es=""
flush() {
  [ -n "$name" ] || return 0
  desc="$en / $es"
  # GitHub limita la descripción a 100 caracteres (labels.yml las respeta; no se recorta aquí).
  if [ "$dry" -eq 1 ]; then
    printf 'gh label create %q --repo %q --color %q --description %q --force\n' "$name" "$repo" "$color" "$desc"
  else
    gh label create "$name" --repo "$repo" --color "$color" --description "$desc" --force
  fi
  name=""; color=""; en=""; es=""
}
while IFS= read -r line; do
  case "$line" in
    "- name: "*) flush; name="${line#- name: }" ;;
    "  color: "*) color="${line#  color: }" ;;
    "  description_en: "*) en="${line#  description_en: }" ;;
    "  description_es: "*) es="${line#  description_es: }" ;;
  esac
done < "$file"
flush
