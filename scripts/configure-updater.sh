#!/usr/bin/env bash
# Configura el actualizador automático en apps/desktop/src-tauri/tauri.conf.json.
#
#   scripts/configure-updater.sh <url-del-repositorio> <archivo-con-la-llave-pública>
#   ej.: scripts/configure-updater.sh https://github.com/mi-org/tlaku ~/.tauri/tlaku.key.pub
#
# Hace tres cosas, y solo estas:
#   1. escribe `plugins.updater.pubkey` con el contenido del archivo .pub (la llave PÚBLICA);
#   2. escribe `plugins.updater.endpoints` con <repo>/releases/latest/download/latest.json;
#   3. activa `bundle.createUpdaterArtifacts`.
#
# Desde ese momento la compilación firma los paquetes de actualización y EXIGE la llave privada
# (variables TAURI_SIGNING_PRIVATE_KEY y TAURI_SIGNING_PRIVATE_KEY_PASSWORD). Por eso el
# repositorio las trae apagadas: la compilación local y la CI no las necesitan.
#
# La llave privada nunca se lee ni se guarda aquí; el script se niega a usarla. Ver docs/publicar.md.
set -euo pipefail

uso() {
  echo "Uso: $0 <url-del-repositorio> <archivo-llave-pública>" >&2
  echo "     $0 --desactivar   (vuelve a la plantilla sin actualizador)" >&2
  exit 2
}

raiz="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
conf="${TAURI_CONF:-$raiz/apps/desktop/src-tauri/tauri.conf.json}"
[[ -f "$conf" ]] || { echo "configure-updater: no existe $conf" >&2; exit 1; }
command -v node >/dev/null || { echo "configure-updater: hace falta node" >&2; exit 1; }

if [[ "${1:-}" == "--desactivar" ]]; then
  node - "$conf" <<'JS'
const fs = require('node:fs');
const path = process.argv[2];
const conf = JSON.parse(fs.readFileSync(path, 'utf8'));
conf.bundle.createUpdaterArtifacts = false;
conf.plugins = { ...conf.plugins, updater: { ...conf.plugins?.updater, pubkey: '', endpoints: [] } };
fs.writeFileSync(path, JSON.stringify(conf, null, 2) + '\n');
JS
  echo "Actualizador desactivado en $conf"
  exit 0
fi

[[ $# -eq 2 ]] || uso
repo="${1%/}"
repo="${repo%.git}"
llave="$2"

if [[ ! "$repo" =~ ^https://[A-Za-z0-9.-]+/[A-Za-z0-9._-]+/[A-Za-z0-9._-]+$ ]]; then
  echo "configure-updater: la URL debe ser https://<servidor>/<organización>/<repositorio>, recibí: $repo" >&2
  exit 1
fi
[[ -f "$llave" ]] || { echo "configure-updater: no existe el archivo $llave" >&2; exit 1; }

node - "$conf" "$repo" "$llave" <<'JS'
const fs = require('node:fs');
const [path, repo, keyFile] = process.argv.slice(2);
const fail = (msg) => {
  console.error(`configure-updater: ${msg}`);
  process.exit(1);
};

const pubkey = fs.readFileSync(keyFile, 'utf8').trim();
if (!pubkey) fail('el archivo de la llave está vacío');
if (/\s/.test(pubkey) && !pubkey.startsWith('untrusted comment')) fail('la llave pública debe ir en una sola línea (el .pub que crea `tauri signer generate`)');

// `tauri signer generate` guarda las llaves en base64 de un texto minisign. Nunca se acepta la privada.
let texto = pubkey;
try {
  texto = Buffer.from(pubkey, 'base64').toString('utf8');
} catch {
  // no era base64: se revisa tal cual
}
if (/secret key/i.test(texto) || /secret key/i.test(pubkey)) {
  fail('ese archivo parece la llave PRIVADA. Usa el .pub; la privada va solo en los secretos de GitHub.');
}
if (!/public key/i.test(texto)) fail('el archivo no parece una llave pública de `tauri signer generate` (se esperaba el contenido de tlaku.key.pub)');

const conf = JSON.parse(fs.readFileSync(path, 'utf8'));
conf.bundle.createUpdaterArtifacts = true;
conf.plugins = conf.plugins ?? {};
conf.plugins.updater = {
  ...conf.plugins.updater,
  pubkey,
  endpoints: [`${repo}/releases/latest/download/latest.json`],
};
fs.writeFileSync(path, JSON.stringify(conf, null, 2) + '\n');
console.log(`Actualizador configurado en ${path}`);
console.log(`  endpoint: ${repo}/releases/latest/download/latest.json`);
console.log('  createUpdaterArtifacts: true  (la compilación ahora exige TAURI_SIGNING_PRIVATE_KEY)');
JS
