#!/usr/bin/env node
// Pasos de versión que antes se hacían a mano. Guía: docs/publicar.md#sacar-una-versión
//
//   node scripts/release-version.mjs bump X.Y.Z        antes de la etiqueta
//   node scripts/release-version.mjs after vX.Y.Z      después de publicar la Release
//
// `bump` cambia la versión en los tres lugares que compara el paso «Verificar versión» de
// release.yml (Cargo.toml de la raíz, tauri.conf.json y package.json) más package-lock.json y
// Cargo.lock (con `cargo update --workspace --offline`).
// `after` lee los archivos de la Release publicada (API pública de GitHub, sin token) y pone al día
// los botones de descarga de README.md y README.es.md, el cask de Homebrew (versión y SHA256 del
// .dmg) y los manifiestos de winget (packaging/winget/.../X.Y.Z, con scripts/winget-manifests.mjs).
// No sube nada: el tap y el PR de winget los envía el autor (docs/publicar.md#dar-de-alta-en-tiendas).
import { execFileSync } from 'node:child_process';
import { readFileSync, writeFileSync } from 'node:fs';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';

const RAIZ = join(dirname(fileURLToPath(import.meta.url)), '..');
const REPO = 'jahziel1/tlaku';
const SEMVER = /^\d+\.\d+\.\d+$/;

const leer = (rel) => readFileSync(join(RAIZ, rel), 'utf8');
const escribir = (rel, texto) => writeFileSync(join(RAIZ, rel), texto);

/** Reemplaza con `re` y falla si no hubo exactamente `veces` coincidencias. */
export function sustituir(texto, re, nuevo, donde, veces = 1) {
  const n = [...texto.matchAll(new RegExp(re.source, re.flags.includes('g') ? re.flags : `${re.flags}g`))].length;
  if (n !== veces) throw new Error(`${donde}: esperaba ${veces} coincidencia(s) de ${re}, hay ${n}`);
  return texto.replace(new RegExp(re.source, re.flags.includes('g') ? re.flags : `${re.flags}g`), nuevo);
}

export function versionActual() {
  return JSON.parse(leer('apps/desktop/src-tauri/tauri.conf.json')).version;
}

function bump(version) {
  if (!SEMVER.test(version ?? '')) throw new Error('uso: bump X.Y.Z');
  const antes = versionActual();
  const cargo = leer('Cargo.toml');
  escribir(
    'Cargo.toml',
    sustituir(cargo, /(\[workspace\.package\][^[]*?\nversion = )"[^"]+"/, `$1"${version}"`, 'Cargo.toml'),
  );
  for (const rel of ['apps/desktop/src-tauri/tauri.conf.json', 'apps/desktop/package.json']) {
    escribir(rel, sustituir(leer(rel), /^( {2}"version": )"[^"]+"/m, `$1"${version}"`, rel));
  }
  // package-lock.json: la raíz y packages[""] (las dos primeras apariciones).
  const lock = 'apps/desktop/package-lock.json';
  const datos = JSON.parse(leer(lock));
  datos.version = version;
  datos.packages[''].version = version;
  escribir(lock, `${JSON.stringify(datos, null, 2)}\n`);
  execFileSync('cargo', ['update', '--workspace', '--offline'], { cwd: RAIZ, stdio: 'inherit' });
  console.log(`Versión ${antes} -> ${version}. Revisa con git diff y crea la etiqueta v${version} solo con aprobación.`);
}

/** Botones de descarga: cambia vA.B.C/Tlaku_A.B.C_ por la versión nueva. */
export function botonesReadme(texto, version, donde) {
  return sustituir(
    texto,
    /releases\/download\/v\d+\.\d+\.\d+\/Tlaku_\d+\.\d+\.\d+_/,
    `releases/download/v${version}/Tlaku_${version}_`,
    donde,
    4,
  );
}

export function cask(texto, version, sha256) {
  let t = sustituir(texto, /^( {2}version )"[^"]+"/m, `$1"${version}"`, 'cask');
  t = sustituir(t, /^( {2}sha256 )"[0-9a-f]{64}"/m, `$1"${sha256}"`, 'cask');
  return t;
}

async function after(tag) {
  if (!/^v\d+\.\d+\.\d+$/.test(tag ?? '')) throw new Error('uso: after vX.Y.Z');
  const version = tag.slice(1);
  if (version !== versionActual()) throw new Error(`tauri.conf.json dice ${versionActual()}, no ${version}`);
  const r = await fetch(`https://api.github.com/repos/${REPO}/releases/tags/${tag}`, {
    headers: { accept: 'application/vnd.github+json' },
  });
  if (!r.ok) throw new Error(`La Release ${tag} no está publicada (HTTP ${r.status})`);
  const rel = await r.json();
  if (rel.draft) throw new Error(`${tag} sigue en borrador`);
  const assets = new Map(rel.assets.map((a) => [a.name, a]));
  const necesarios = ['x64-setup.exe', 'universal.dmg', 'amd64.deb', 'amd64.AppImage'].map(
    (s) => `Tlaku_${version}_${s}`,
  );
  const faltan = necesarios.filter((n) => !assets.has(n));
  if (faltan.length) throw new Error(`faltan en la Release: ${faltan.join(', ')}`);
  const dmg = assets.get(`Tlaku_${version}_universal.dmg`);
  const sha = (dmg.digest ?? '').replace(/^sha256:/, '');
  if (!/^[0-9a-f]{64}$/.test(sha)) throw new Error('GitHub no dio el SHA256 del .dmg');

  for (const f of ['README.md', 'README.es.md']) escribir(f, botonesReadme(leer(f), version, f));
  const rutaCask = 'packaging/homebrew/Casks/tlaku.rb';
  escribir(rutaCask, cask(leer(rutaCask), version, sha));
  const fecha = rel.published_at.slice(0, 10);
  execFileSync(
    'node',
    [join(RAIZ, 'scripts/winget-manifests.mjs'), 'generate', tag, join(RAIZ, 'packaging/winget'), '--date', fecha],
    { stdio: 'inherit' },
  );
  console.log(`\nListo para ${tag}: README.md, README.es.md, ${rutaCask} y packaging/winget/.../${version}.
Siguiente: commit, copiar el cask al tap jahziel1/homebrew-tlaku y abrir el PR "Update" de winget.`);
}

const [cmd, ...args] = process.argv.slice(2);
if (process.argv[1] === fileURLToPath(import.meta.url)) {
  const accion = { bump: () => bump(args[0]), after: () => after(args[0]) }[cmd];
  if (!accion) {
    console.error('uso: release-version.mjs bump X.Y.Z | after vX.Y.Z');
    process.exit(2);
  }
  Promise.resolve(accion()).catch((e) => {
    console.error(e.message);
    process.exit(1);
  });
}
