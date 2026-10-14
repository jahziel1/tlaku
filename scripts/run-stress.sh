#!/usr/bin/env bash
# Prueba de estrés de punta a punta: descarga el corpus, genera los archivos sintéticos y mide todo.
#
#   scripts/run-stress.sh [DIRECTORIO_DE_TRABAJO]
#
# Deja en el directorio de trabajo (por defecto /tmp/tlaku-stress): corpus/ (descargado y sintético),
# lineas.csv, lineas.md (resumen) y relleno.csv. Compila en release. Ver docs/pruebas-estres.md.
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
WORK="${1:-/tmp/tlaku-stress}"
mkdir -p "$WORK"
cd "$ROOT"
cargo build --release -p tlaku-bench
BIN="${CARGO_TARGET_DIR:-$ROOT/target}/release/tlaku-bench"
"$ROOT/scripts/fetch-corpus.sh" "$WORK/corpus"
"$BIN" gen "$WORK/corpus/synthetic"
"$BIN" run "$WORK/corpus" --csv "$WORK/lineas.csv" --summary "$WORK/lineas.md" || true
"$BIN" run "$WORK/corpus" --modes fill --no-emu --csv "$WORK/relleno.csv" --summary "$WORK/relleno.md" || true
echo "Listo: $WORK/lineas.md y $WORK/relleno.md"
