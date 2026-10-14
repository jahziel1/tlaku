#!/usr/bin/env bash
# Prueba de humo de la app Tauri real, sin ventana visible.
#
# Compila y corre `src-tauri/examples/smoke.rs` (código en `src-tauri/src/smoke.rs`): arranca la
# aplicación con el runtime real (Wry) y una ventana oculta, y manda por `invoke` los comandos de
# solo lectura (versión, puertos, dispositivos, preferencias, materiales de fábrica...) más dos
# llamadas a `fire_test` (con snake_case debe aceptar los argumentos; con camelCase, rechazarlos).
# Después prueba «Nueva ventana» (D19) con una segunda ventana real: `new_window`, que la ventana
# exista y responda, que las preferencias se vean entre ventanas, que la recuperación no mezcle
# documentos, que el `localStorage` de una ventana llegue a la otra (evento `storage`) y que al
# cerrarla sus copias queden huérfanas.
# Sale con 0 si todo respondió como se esperaba. No usa la interfaz web ni el modo simulado.
#
# Uso:    apps/desktop/scripts/smoke-tauri.sh
# macOS:  funciona tal cual (hace falta la sesión gráfica; en los runners de GitHub existe).
# Linux:  necesita libwebkit2gtk-4.1-dev, libgtk-3-dev y xvfb (`sudo apt-get install xvfb`);
#         si no hay $DISPLAY el script se relanza con `xvfb-run`.
# Windows: sin script (usar el equivalente con PowerShell: `cargo run -p tlaku-desktop --example smoke`).
#
# Aísla HOME y XDG_*: la app escribe su copia de los materiales de fábrica y no debe tocar los
# datos reales de quien lo corre.
set -euo pipefail

raiz="$(cd "$(dirname "${BASH_SOURCE[0]}")/../../.." && pwd)"
cd "$raiz"

if [[ "$(uname -s)" == "Linux" && -z "${DISPLAY:-}" && -z "${WAYLAND_DISPLAY:-}" ]]; then
  if ! command -v xvfb-run >/dev/null; then
    echo "smoke-tauri: sin \$DISPLAY y sin xvfb-run; instala xvfb" >&2
    exit 2
  fi
  exec xvfb-run -a "$0" "$@"
fi

cargo build -p tlaku-desktop --example smoke
binario="${CARGO_TARGET_DIR:-$raiz/target}/debug/examples/smoke"

casa="$(mktemp -d)"
trap 'rm -rf "$casa"' EXIT
export HOME="$casa" XDG_DATA_HOME="$casa/data" XDG_CONFIG_HOME="$casa/config" XDG_CACHE_HOME="$casa/cache"

# Límite de tiempo sin depender de `timeout` (macOS no lo trae).
"$binario" &
pid=$!
for _ in $(seq 1 90); do
  kill -0 "$pid" 2>/dev/null || break
  sleep 1
done
if kill -0 "$pid" 2>/dev/null; then
  kill "$pid" 2>/dev/null || true
  echo "smoke-tauri FALLO: la app no terminó en 90 s" >&2
  exit 1
fi
wait "$pid"
