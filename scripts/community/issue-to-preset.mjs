#!/usr/bin/env node
// Convierte un issue del formulario `material_settings` en un archivo de community/materials/.
// Converts an issue made with the `material_settings` form into a community/materials/ file.
//
//   node scripts/community/issue-to-preset.mjs <número> [--repo propietario/repo] [--dry-run]
//   node scripts/community/issue-to-preset.mjs <número> --body-file issue.md [--author usuario]
//
// Con `gh issue view --json` lee el issue (hace falta `gh auth login`) y escribe
// `community/materials/<número>.json`. Con `--dry-run` solo lo imprime. No toca git ni GitHub.
// Si el usuario escribió la velocidad con unidad («20 mm/s»), la convierte a mm/min.
// Después de generarlo, REVISA los valores contra la foto y el resultado, y corre
// `cargo test -p tlaku-doc community`.
//
// Las etiquetas del formulario se leen de .github/ISSUE_TEMPLATE/material_settings.yml, así que si
// se cambia un texto del formulario este script lo sigue.
//
// Solo Node, sin dependencias.
import { execFileSync } from 'node:child_process';
import { existsSync, mkdirSync, readFileSync, writeFileSync } from 'node:fs';
import { dirname, join, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';

const ROOT = resolve(dirname(fileURLToPath(import.meta.url)), '../..');
export const FORM_FILE = join(ROOT, '.github/ISSUE_TEMPLATE/material_settings.yml');
export const OUT_DIR = join(ROOT, 'community/materials');
export const DEFAULT_REPO = 'jahziel1/tlaku';

/** Clases de máquina (tipo y watts), igual que `MachineClass::ALL` en tlaku-doc. */
const CLASSES = {
  diode: [5, 10, 20, 40],
  co2: [40, 60, 80, 100],
  fiber: [20, 30, 50],
};

/** Materiales de la biblioteca de fábrica, con los nombres en inglés y español que se esperan en el formulario. */
const MATERIAL_ALIASES = {
  mdf: ['mdf', 'mdf board'],
  plywood_birch: ['birch plywood', 'plywood birch', 'contrachapado de abedul', 'triplay de abedul'],
  plywood_basswood: ['basswood plywood', 'plywood basswood', 'contrachapado de tilo', 'triplay de tilo'],
  basswood: ['basswood', 'madera de tilo', 'tilo'],
  acrylic_black: ['black acrylic', 'acrylic black', 'acrilico negro'],
  acrylic_clear: ['clear acrylic', 'acrylic clear', 'transparent acrylic', 'acrilico transparente'],
  leather: ['leather', 'cuero'],
  cardboard: ['cardboard', 'carton'],
  paper: ['paper', 'papel'],
  cork: ['cork', 'corcho'],
  slate: ['slate', 'pizarra'],
  anodized_aluminum: ['anodized aluminum', 'anodized aluminium', 'aluminio anodizado'],
  stainless_steel: ['stainless steel', 'acero inoxidable'],
  glass: ['glass', 'vidrio'],
};

const fold = (s) =>
  s
    .normalize('NFD')
    .replace(/[̀-ͯ]/g, '')
    .toLowerCase()
    .replace(/\s+/g, ' ')
    .trim();

/** Extrae `{ id: etiqueta }` del formulario YAML (formato fijo: `id:` seguido de `attributes: label:`). */
export function formLabels(formText) {
  const labels = {};
  const re = /^ {4}id: ([A-Za-z0-9_-]+)\n {4}attributes:\n(?: {6}(?!label:).*\n)* {6}label: "((?:[^"\\]|\\.)*)"/gm;
  for (const m of formText.matchAll(re)) labels[m[1]] = m[2].replace(/\\"/g, '"');
  return labels;
}

/** Divide el cuerpo de un issue (`### Etiqueta` + valor) en `{ id: texto }`. `_No response_` queda vacío. */
export function parseBody(body, labels) {
  const byLabel = new Map(Object.entries(labels).map(([id, label]) => [label.trim(), id]));
  const out = {};
  const parts = body.replace(/\r\n/g, '\n').split(/^### +/m).slice(1);
  for (const part of parts) {
    const nl = part.indexOf('\n');
    const label = (nl < 0 ? part : part.slice(0, nl)).trim();
    const id = byLabel.get(label);
    if (!id) continue;
    let value = nl < 0 ? '' : part.slice(nl + 1).trim();
    if (value === '_No response_') value = '';
    out[id] = value;
  }
  return out;
}

class FormError extends Error {}

const num = (text, field, { required = false } = {}) => {
  const s = (text ?? '').trim();
  if (s === '') {
    if (required) throw new FormError(`Falta el campo «${field}».`);
    return undefined;
  }
  const m = s.replace(',', '.').match(/^[^\d-]*(-?\d+(?:\.\d+)?)/);
  if (!m) throw new FormError(`«${field}» no es un número: ${s}`);
  return Number(m[1]);
};

const round = (n, digits = 2) => Math.round(n * 10 ** digits) / 10 ** digits;

/** Velocidad en mm/min; si trae «mm/s» se multiplica por 60. */
export function parseSpeed(text) {
  const s = (text ?? '').trim().toLowerCase();
  const value = num(s, 'speed', { required: true });
  if (/mm\s*\/\s*s(ec)?\b/.test(s) || /\bmm\s*s\^?-1\b/.test(s)) return round(value * 60);
  if (/m\s*\/\s*min\b/.test(s) && !/mm\s*\/\s*min/.test(s)) return round(value * 1000);
  return round(value);
}

/** La clase con la potencia más cercana; en un empate, la de menos potencia (igual que el motor). */
export function nearestClass(kind, watts) {
  let best = null;
  for (const w of CLASSES[kind]) if (best === null || Math.abs(w - watts) < Math.abs(best - watts)) best = w;
  return `${kind}_${best}`;
}

export function materialKey(name) {
  const f = fold(name);
  for (const [key, names] of Object.entries(MATERIAL_ALIASES)) if (names.includes(f)) return { key, known: true };
  // Si es bilingüe («Basswood plywood / Contrachapado de tilo»), se prueba cada mitad.
  for (const half of f.split('/').map((h) => h.trim())) {
    for (const [key, names] of Object.entries(MATERIAL_ALIASES)) if (names.includes(half)) return { key, known: true };
  }
  const slug = f.split('/')[0].replace(/[^a-z0-9]+/g, '_').replace(/^_+|_+$/g, '').slice(0, 40);
  if (!slug) throw new FormError('No se pudo derivar una clave del material.');
  return { key: slug, known: false };
}

const checked = (text, n) => [...text.matchAll(/^- \[([ xX])\]/gm)].slice(0, n).every((m) => m[1] !== ' ') && [...text.matchAll(/^- \[/gm)].length >= n;

/**
 * Del cuerpo del issue al JSON del aporte.
 * @param {string} body cuerpo del issue
 * @param {number} number número del issue
 * @param {{ formText?: string, repo?: string, author?: string }} [opts]
 */
export function bodyToPreset(body, number, opts = {}) {
  const formText = opts.formText ?? readFileSync(FORM_FILE, 'utf8');
  const repo = opts.repo ?? DEFAULT_REPO;
  const f = parseBody(body, formLabels(formText));
  if (!Number.isInteger(number) || number <= 0) throw new FormError('Número de issue inválido.');
  if (f.laser_type === undefined || f.speed === undefined) throw new FormError('El issue no parece venir del formulario material_settings.');
  if (!checked(f.confirm ?? '', 2)) throw new FormError('Faltan las casillas de confirmación (probado en mi máquina y licencia CC0): no se genera nada.');

  const kindText = fold(f.laser_type);
  const kind = kindText.startsWith('diod') ? 'diode' : kindText.startsWith('co2') ? 'co2' : kindText.startsWith('fib') ? 'fiber' : null;
  if (!kind) throw new FormError(`Tipo de láser desconocido: ${f.laser_type}`);
  const watts = num(f.laser_watts, 'laser_watts', { required: true });
  const op = fold(f.operation ?? '');
  const role = op.startsWith('cut') || op.startsWith('corte') ? 'cut' : op.startsWith('engrave') || op.startsWith('grabado') ? 'engrave' : op.startsWith('score') || op.startsWith('marcado') ? 'score' : null;
  if (!role) throw new FormError(`Operación desconocida: ${f.operation}`);
  const machineClass = nearestClass(kind, watts);
  const thickness = num(f.thickness, 'thickness');
  const material = (f.material ?? '').trim();
  if (!material) throw new FormError('Falta el material.');
  const { key, known } = materialKey(material);
  const passes = num(f.passes, 'passes');
  const interval = num(f.interval, 'interval');
  const air = fold(f.air ?? '').startsWith('yes') || fold(f.air ?? '').startsWith('si');

  const note = [(f.result ?? '').trim(), f.focus?.trim() ? `Focus: ${f.focus.trim()}.` : '']
    .filter(Boolean)
    .join(' ')
    .replace(/[\u0000-\u001f\u007f]+/g, ' ')
    .replace(/\s+/g, ' ')
    .trim()
    .slice(0, 400);

  const thick = thickness === undefined ? 'x' : String(thickness).replace('.', '_');
  const entry = {
    id: `community.${key}.${thick}.${role}.${machineClass}.${number}`,
    material: key,
    ...(known ? {} : { material_name: material.slice(0, 60) }),
    ...(thickness === undefined ? {} : { thickness }),
    role,
    machine_class: machineClass,
    laser_watts: watts,
    machine: (f.machine ?? '').replace(/\s+/g, ' ').trim(),
    speed: parseSpeed(f.speed),
    power: num(f.power, 'power', { required: true }),
    ...(passes === undefined ? {} : { passes }),
    ...(interval === undefined ? {} : { interval }),
    air,
    ...(note ? { notes: note } : {}),
    source: `https://github.com/${repo}/issues/${number}`,
    confidence: 'forum',
    license: 'CC0-1.0',
    ...(opts.author && /^[A-Za-z0-9-]{1,39}$/.test(opts.author) ? { contributor: opts.author } : {}),
  };
  return entry;
}

function parseArgs(argv) {
  const args = { repo: DEFAULT_REPO, dry: false, number: null, bodyFile: null, author: undefined };
  for (let i = 0; i < argv.length; i++) {
    const a = argv[i];
    if (a === '--dry-run') args.dry = true;
    else if (a === '--repo') args.repo = argv[++i];
    else if (a === '--body-file') args.bodyFile = argv[++i];
    else if (a === '--author') args.author = argv[++i];
    else if (/^\d+$/.test(a)) args.number = Number(a);
    else throw new FormError(`Argumento desconocido: ${a}`);
  }
  if (args.number === null) throw new FormError('Uso: issue-to-preset.mjs <número> [--repo propietario/repo] [--dry-run] [--body-file archivo] [--author usuario]');
  return args;
}

function main() {
  const args = parseArgs(process.argv.slice(2));
  let body;
  let author = args.author;
  if (args.bodyFile) {
    body = readFileSync(args.bodyFile, 'utf8');
  } else {
    const json = execFileSync('gh', ['issue', 'view', String(args.number), '--repo', args.repo, '--json', 'body,author,labels,state'], { encoding: 'utf8' });
    const issue = JSON.parse(json);
    body = issue.body ?? '';
    author ??= issue.author?.login;
    if (!(issue.labels ?? []).some((l) => l.name === 'material-settings')) console.error('Aviso: el issue no tiene la etiqueta material-settings.');
  }
  const entry = bodyToPreset(body, args.number, { repo: args.repo, author });
  const text = `${JSON.stringify(entry, null, 2)}\n`;
  if (args.dry) {
    process.stdout.write(text);
    return;
  }
  mkdirSync(OUT_DIR, { recursive: true });
  const file = join(OUT_DIR, `${args.number}.json`);
  if (existsSync(file)) throw new FormError(`Ya existe ${file}. Bórralo si quieres regenerarlo.`);
  writeFileSync(file, text);
  console.log(`Escrito ${file}\nRevisa los valores contra el resultado y la foto del issue, y corre: cargo test -p tlaku-doc community`);
}

if (process.argv[1] && resolve(process.argv[1]) === fileURLToPath(import.meta.url)) {
  try {
    main();
  } catch (e) {
    console.error(e instanceof FormError ? e.message : e);
    process.exit(1);
  }
}
