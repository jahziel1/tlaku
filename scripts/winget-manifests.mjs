#!/usr/bin/env node
// Genera y valida los manifiestos de winget (microsoft/winget-pkgs) para una versión de Tlaku.
// Guía completa: docs/publicar.md#winget-windows
//
//   node scripts/winget-manifests.mjs generate v0.2.0 <carpeta-de-salida> [--date AAAA-MM-DD] [--installer ruta.exe]
//   node scripts/winget-manifests.mjs validate <carpeta-con-yaml> <carpeta-con-esquemas>
//
// `generate` toma como base los archivos ya revisados de packaging/winget/manifests/j/jahziel1/Tlaku/<base>/,
// cambia la versión, la URL, la fecha y el SHA256 (baja el instalador de la Release y lo calcula, salvo
// que se pase --installer) y quita las notas de versión escritas a mano (queda ReleaseNotesUrl).
// `validate` comprueba cada YAML contra los esquemas JSON públicos de winget-cli; necesita los paquetes
// npm `ajv` y `js-yaml` (en la CI se instalan en una carpeta temporal y se apunta a ella con NODE_PATH).
// No envía nada a ningún repositorio.
import { createHash } from 'node:crypto';
import { createReadStream } from 'node:fs';
import { mkdir, readFile, readdir, writeFile } from 'node:fs/promises';
import { createRequire } from 'node:module';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';

const RAIZ = join(dirname(fileURLToPath(import.meta.url)), '..');
const BASE_DIR = join(RAIZ, 'packaging/winget/manifests/j/jahziel1/Tlaku');
const BASE_VERSION = '0.1.0';
const REPO = 'jahziel1/tlaku';

function arg(args, nombre) {
  const i = args.indexOf(nombre);
  return i === -1 ? undefined : args[i + 1];
}

async function sha256Archivo(ruta) {
  const h = createHash('sha256');
  for await (const trozo of createReadStream(ruta)) h.update(trozo);
  return h.digest('hex').toUpperCase();
}

async function sha256Url(url) {
  const r = await fetch(url, { redirect: 'follow' });
  if (!r.ok) throw new Error(`No pude bajar ${url}: HTTP ${r.status}`);
  const h = createHash('sha256');
  for await (const trozo of r.body) h.update(trozo);
  return h.digest('hex').toUpperCase();
}

async function generate(args) {
  const [tag, salida] = args;
  if (!tag || !salida || !/^v\d+\.\d+\.\d+$/.test(tag)) {
    throw new Error('uso: generate vX.Y.Z <carpeta-de-salida> [--date AAAA-MM-DD] [--installer ruta]');
  }
  const version = tag.slice(1);
  const fecha = arg(args, '--date') ?? new Date().toISOString().slice(0, 10);
  const nombre = `Tlaku_${version}_x64-setup.exe`;
  const url = `https://github.com/${REPO}/releases/download/${tag}/${nombre}`;
  const instalador = arg(args, '--installer');
  const sha = instalador ? await sha256Archivo(instalador) : await sha256Url(url);

  const origen = join(BASE_DIR, BASE_VERSION);
  const destino = join(salida, 'manifests/j/jahziel1/Tlaku', version);
  await mkdir(destino, { recursive: true });
  for (const f of (await readdir(origen)).filter((x) => x.endsWith('.yaml'))) {
    let t = await readFile(join(origen, f), 'utf8');
    if (version !== BASE_VERSION) {
      // Las notas escritas a mano son de la 0.1.0: se sustituyen por el enlace a la Release.
      t = t.replace(/^ReleaseNotes: \|-\n(?: {2}.*\n)+/m, '');
      t = t.replaceAll(BASE_VERSION, version);
    }
    t = t.replace(/^ReleaseDate: .*$/m, `ReleaseDate: ${fecha}`);
    t = t.replace(/^( {2}InstallerSha256: ).*$/m, `$1${sha}`);
    await writeFile(join(destino, f), t);
  }
  console.log(`Manifiestos de ${tag} en ${destino}\n  ${url}\n  SHA256 ${sha}`);
}

async function validate(args) {
  const [dir, esquemas] = args;
  if (!dir || !esquemas) throw new Error('uso: validate <carpeta-con-yaml> <carpeta-con-esquemas>');
  const require = createRequire(import.meta.url);
  const Ajv = require('ajv');
  const yaml = require('js-yaml');
  const ajv = new Ajv({ allErrors: true, strict: false, validateFormats: false });
  const archivos = (await readdir(dir)).filter((x) => x.endsWith('.yaml'));
  if (archivos.length === 0) throw new Error(`no hay .yaml en ${dir}`);
  let malos = 0;
  for (const f of archivos) {
    const doc = yaml.load(await readFile(join(dir, f), 'utf8'), { schema: yaml.JSON_SCHEMA });
    const esquema = JSON.parse(
      await readFile(join(esquemas, `manifest.${doc.ManifestType}.${doc.ManifestVersion}.json`), 'utf8'),
    );
    const ok = ajv.validate(esquema, doc);
    console.log(`${ok ? 'OK  ' : 'FALLA'} ${f} (${doc.ManifestType} ${doc.ManifestVersion})`);
    if (!ok) {
      malos++;
      console.log(JSON.stringify(ajv.errors, null, 1));
    }
  }
  if (malos) process.exit(1);
}

const [cmd, ...resto] = process.argv.slice(2);
try {
  if (cmd === 'generate') await generate(resto);
  else if (cmd === 'validate') await validate(resto);
  else throw new Error('uso: winget-manifests.mjs generate|validate ...');
} catch (e) {
  console.error(`error: ${e.message}`);
  process.exit(1);
}
