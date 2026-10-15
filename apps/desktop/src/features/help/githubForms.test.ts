/// <reference types="node" />
import { readFileSync } from 'node:fs';
import { fileURLToPath } from 'node:url';
import { describe, expect, it } from 'vitest';
import type { DeviceProfile, MaterialPreset } from '@/api/types';
import { FALLBACK_DEVICE } from '@/lib/device';
import { makeLayer } from '@/lib/layers';
// @ts-expect-error -- módulo .mjs de scripts/, sin tipos
import { bodyToPreset, parseSpeed } from '../../../../../scripts/community/issue-to-preset.mjs';
import { DROPDOWN_VALUES, FORM_FIELDS, FORM_FILES, bugReportUrl, formUrl, lastInfoResponse, machineContext, machineReportUrl, materialFormValues, materialShareUrl, systemLine, contributionDraft } from './githubForms';
import { isAllowedUrl, openEnv, openExternal } from './openExternal';
import { PROJECT_LINKS, REPO_URL } from './links';

const ROOT = fileURLToPath(new URL('../../../../../', import.meta.url));
const read = (path: string) => readFileSync(`${ROOT}${path}`, 'utf8');

interface FormField {
  type: string;
  id?: string;
  label?: string;
  options: string[];
  required: boolean;
}

/** Lee los campos del formulario (formato fijo de nuestros YAML: sangría de 2/4/6/8 espacios). */
function fieldsOf(file: string): FormField[] {
  const text = read(`.github/ISSUE_TEMPLATE/${file}`);
  return text
    .split(/\n {2}- type: /)
    .slice(1)
    .map((block) => ({
      type: block.split('\n')[0]!.trim(),
      id: /^ {4}id: (\S+)$/m.exec(block)?.[1],
      label: /^ {6}label: "(.*)"$/m.exec(block)?.[1],
      options: [...block.matchAll(/^ {8}- (?:label: )?"(.*)"$/gm)].map((m) => m[1]!),
      required: /required: true/.test(block),
    }));
}

const device = (patch: Partial<DeviceProfile> = {}): DeviceProfile => ({ ...FALLBACK_DEVICE, id: 'd1', name: 'Falcon2 22 W', kind: 'grbl', ...patch });

describe('formularios de GitHub: ids y opciones', () => {
  for (const form of ['bug', 'machine', 'material'] as const) {
    it(`${FORM_FILES[form]}: cada campo que la app prellena existe en el formulario`, () => {
      const ids = new Set(fieldsOf(FORM_FILES[form]).flatMap((f) => (f.id ? [f.id] : [])));
      for (const id of FORM_FIELDS[form]) expect(ids.has(id), `${FORM_FILES[form]} no tiene el campo ${id}`).toBe(true);
    });
  }

  it('las opciones exactas de los campos desplegables existen en el formulario', () => {
    const options = (file: string, id: string) => fieldsOf(file).find((f) => f.id === id)!.options;
    for (const v of Object.values(DROPDOWN_VALUES.connection)) expect(options('machine_report.yml', 'connection')).toContain(v);
    for (const v of Object.values(DROPDOWN_VALUES.laser_type)) expect(options('material_settings.yml', 'laser_type')).toContain(v);
    for (const v of Object.values(DROPDOWN_VALUES.operation)) expect(options('material_settings.yml', 'operation')).toContain(v);
    for (const v of Object.values(DROPDOWN_VALUES.air)) expect(options('material_settings.yml', 'air')).toContain(v);
  });

  it('todos los formularios son bilingües: cada etiqueta lleva «inglés / español»', () => {
    for (const file of ['bug_report.yml', 'feature_request.yml', 'machine_report.yml', 'material_settings.yml', 'new_controller.yml']) {
      for (const f of fieldsOf(file)) {
        if (f.type === 'markdown') continue;
        expect(f.label, `${file}: campo ${f.id ?? '?'} sin etiqueta`).toBeTruthy();
        if (f.label !== 'Firmware' && f.label !== 'Material') expect(f.label, `${file}: ${f.label}`).toContain(' / ');
      }
    }
  });

  it('el formulario de material exige las dos casillas (probado y CC0)', () => {
    const confirm = fieldsOf('material_settings.yml').find((f) => f.id === 'confirm')!;
    expect(confirm.options).toHaveLength(2);
    expect(confirm.options.some((o) => /CC0/.test(o))).toBe(true);
    expect((read('.github/ISSUE_TEMPLATE/material_settings.yml').match(/required: true/g) ?? []).length).toBeGreaterThanOrEqual(2);
  });

  it('config.yml desactiva los issues en blanco y enlaza Discussions, manuales y seguridad', () => {
    const text = read('.github/ISSUE_TEMPLATE/config.yml');
    expect(text).toMatch(/^blank_issues_enabled: false$/m);
    for (const part of ['/discussions', 'docs/manual/en/00-indice.md', 'docs/manual/00-indice.md', '/security/advisories/new']) expect(text).toContain(part);
  });
});

describe('dirección del formulario prellenado', () => {
  it('lleva la plantilla y los campos, y omite los vacíos', () => {
    const url = new URL(formUrl('bug', { version: '0.1.0', system: '', machine: undefined, controller: 'GRBL' }));
    expect(url.origin + url.pathname).toBe(`${REPO_URL}/issues/new`);
    expect(url.searchParams.get('template')).toBe('bug_report.yml');
    expect(url.searchParams.get('version')).toBe('0.1.0');
    expect(url.searchParams.get('controller')).toBe('GRBL');
    expect(url.searchParams.has('system')).toBe(false);
    expect(url.searchParams.has('machine')).toBe(false);
  });

  it('recorta lo que no cabe', () => {
    const url = formUrl('machine', { grbl_i: 'x'.repeat(20000), version: '0.1.0' });
    expect(url.length).toBeLessThanOrEqual(6500);
    expect(new URL(url).searchParams.get('version')).toBe('0.1.0');
  });

  it('el reporte de error lleva versión, sistema y máquina', () => {
    const ctx = machineContext(device(), false, false, null, []);
    const url = new URL(bugReportUrl({ version: '0.1.0', nav: { platform: 'MacIntel', userAgent: 'UA' }, machine: ctx }));
    expect(url.searchParams.get('version')).toBe('0.1.0');
    expect(url.searchParams.get('system')).toBe('MacIntel (UA)');
    expect(url.searchParams.get('machine')).toBe('Falcon2 22 W');
    expect(url.searchParams.get('controller')).toBe('GRBL');
  });

  it('el reporte de máquina lleva controladora, firmware y la última respuesta de $I si hay un GRBL conectado', () => {
    const log = [
      { dir: 'out' as const, text: '$I' },
      { dir: 'in' as const, text: '[VER:1.1f.20220810:]' },
      { dir: 'in' as const, text: '[OPT:VHL,127,65536]' },
      { dir: 'in' as const, text: 'ok' },
      { dir: 'out' as const, text: '?' },
      { dir: 'in' as const, text: '<Idle|MPos:0,0,0>' },
    ];
    expect(lastInfoResponse(log)).toBe('[VER:1.1f.20220810:]\n[OPT:VHL,127,65536]\nok');
    const ctx = machineContext(device(), true, false, "Grbl 1.1f ['$' for help]", log);
    const url = new URL(machineReportUrl({ version: '0.1.0', nav: {}, machine: ctx }));
    expect(url.searchParams.get('controller')).toBe('GRBL');
    expect(url.searchParams.get('firmware')).toContain('Grbl 1.1f');
    expect(url.searchParams.get('grbl_i')).toContain('[VER:1.1f.20220810:]');
    expect(url.searchParams.get('model')).toBe('Falcon2 22 W');
    expect(url.searchParams.get('connection')).toBe(DROPDOWN_VALUES.connection.serial);
  });

  it('sin conexión, o con una máquina virtual o que no es GRBL, no se inventa nada de $I ni de firmware', () => {
    const log = [{ dir: 'out' as const, text: '$I' }, { dir: 'in' as const, text: '[VER:1.1f:]' }];
    expect(machineContext(device(), false, false, 'fw', log)).toMatchObject({ info: '', firmware: '' });
    const virtual = machineContext(device(), true, true, 'fw', log);
    expect(virtual).toMatchObject({ info: '', firmware: '', model: '', virtual: true });
    expect(new URL(machineReportUrl({ version: null, nav: {}, machine: virtual })).searchParams.has('connection')).toBe(false);
    expect(machineContext(device({ kind: 'ruida' }), true, false, 'fw', log).info).toBe('');
    expect(lastInfoResponse([])).toBe('');
  });

  it('el ajuste de material propio llena los campos del formulario', () => {
    const preset: MaterialPreset = {
      id: 'm1', material: 'Contrachapado de tilo', thickness: 3, title: 'Corte', notes: '', device_id: null,
      settings: makeLayer('#000000', 'Corte', { speed: 500, power: 100, passes: 2, air: true }),
      role: 'cut', machine_class: 'diode_20',
    };
    const v = materialFormValues(preset, device());
    expect(v).toMatchObject({ laser_type: DROPDOWN_VALUES.laser_type.diode, laser_watts: '20', machine: 'Falcon2 22 W', material: 'Contrachapado de tilo', thickness: '3', operation: DROPDOWN_VALUES.operation.cut, speed: '500', power: '100', passes: '2', air: DROPDOWN_VALUES.air.yes });
    expect(v.interval).toBeUndefined();
    const url = new URL(materialShareUrl(preset, device()));
    expect(url.searchParams.get('template')).toBe('material_settings.yml');
    expect(url.searchParams.get('speed')).toBe('500');
    expect(new URL(materialShareUrl(null, device())).searchParams.get('machine')).toBe('Falcon2 22 W');
  });

  it('la versión de Tlaku se prellena y el borrador JSON no lleva datos personales', () => {
    const preset: MaterialPreset = {
      id: 'mat-secreto-123', material: 'Contrachapado de tilo', thickness: 3, title: 'Corte', notes: 'Nota privada: la llave está en /Users/juan/doc.tlaku', device_id: 'dispositivo-privado',
      settings: makeLayer('#000000', 'Corte', { speed: 500, power: 100, passes: 2, air: true }),
      role: 'cut', machine_class: 'diode_20', source: 'https://github.com/jahziel1/tlaku/issues/9', confidence: 'forum',
    };
    expect(materialFormValues(preset, device(), null, '0.1.0').tlaku_version).toBe('0.1.0');
    expect(materialFormValues(preset, device()).tlaku_version).toBeUndefined();
    expect(new URL(materialShareUrl(preset, device(), null, '0.1.0')).searchParams.get('tlaku_version')).toBe('0.1.0');
    expect(new URL(materialShareUrl(null, device(), null, '0.1.0')).searchParams.get('tlaku_version')).toBe('0.1.0');

    const draft = contributionDraft(preset, device(), null, '0.1.0');
    expect(draft).toEqual({ material: 'Contrachapado de tilo', thickness: 3, role: 'cut', machine_class: 'diode_20', laser_watts: 20, machine: 'Falcon2 22 W', speed: 500, power: 100, passes: 2, air: true, tlaku_version: '0.1.0' });
    // Ni identificadores, ni notas, ni rutas, ni la dirección de origen: solo lo que el formulario ya muestra.
    const text = JSON.stringify(draft);
    for (const leak of ['mat-secreto', 'privada', '/Users', 'dispositivo-privado', 'issues/9']) expect(text).not.toContain(leak);
    // Un relleno agrega su modo e intervalo; sin material no se inventa uno.
    const fill: MaterialPreset = { ...preset, material: '', settings: makeLayer('#000000', 'G', { mode: 'fill', speed: 3000, power: 30, interval: 0.1 }), role: undefined, machine_class: undefined };
    expect(contributionDraft(fill, device(), 'diode_10')).toMatchObject({ mode: 'fill', interval: 0.1, role: 'engrave', machine_class: 'diode_10', laser_watts: 10 });
    expect(contributionDraft(fill, device())).not.toHaveProperty('material');
  });

  it('systemLine junta plataforma y webview', () => {
    expect(systemLine({})).toBe('');
    expect(systemLine({ platform: 'Win32' })).toBe('Win32');
  });
});

describe('abrir en el navegador', () => {
  it('solo se abren direcciones https del repositorio y de su sitio', () => {
    for (const ok of [REPO_URL, `${REPO_URL}/issues/new?template=bug_report.yml&version=1`, 'https://jahziel1.github.io/tlaku/en/']) expect(isAllowedUrl(ok), ok).toBe(true);
    for (const bad of [
      'http://github.com/jahziel1/tlaku',
      'https://github.com/jahziel1/tlaku-evil/issues',
      'https://github.com/jahziel1/otro',
      'https://github.com.evil.example/jahziel1/tlaku',
      'https://user:pw@github.com/jahziel1/tlaku',
      'https://example.com',
      'javascript:alert(1)',
      'file:///etc/passwd',
      'no es una dirección',
    ]) expect(isAllowedUrl(bad), bad).toBe(false);
    expect(isAllowedUrl(PROJECT_LINKS.issues)).toBe(true);
  });

  it('openExternal usa el entorno solo para direcciones permitidas', async () => {
    const opened: string[] = [];
    const original = openEnv.open;
    openEnv.open = async (u) => void opened.push(u);
    try {
      expect(await openExternal(`${REPO_URL}/issues`)).toBe(true);
      expect(await openExternal('https://example.com')).toBe(false);
      expect(opened).toEqual([`${REPO_URL}/issues`]);
      openEnv.open = async () => {
        throw new Error('sin navegador');
      };
      expect(await openExternal(`${REPO_URL}/issues`)).toBe(false);
    } finally {
      openEnv.open = original;
    }
  });

  it('el permiso de Tauri solo deja las mismas direcciones', () => {
    const caps = JSON.parse(read('apps/desktop/src-tauri/capabilities/default.json')) as { permissions: (string | { identifier: string; allow?: { url: string }[] })[] };
    const opener = caps.permissions.find((p) => typeof p !== 'string' && p.identifier === 'opener:allow-open-url');
    expect(opener).toBeTruthy();
    const urls = (opener as { allow: { url: string }[] }).allow.map((a) => a.url);
    for (const u of urls) expect(u.startsWith('https://github.com/jahziel1/tlaku') || u.startsWith('https://jahziel1.github.io/tlaku') || u === 'https://ko-fi.com/danielmartinez55371', u).toBe(true);
    expect(caps.permissions).not.toContain('opener:default');
  });
});

describe('script issue-to-preset', () => {
  const formLabels = (): string => read('.github/ISSUE_TEMPLATE/material_settings.yml');
  const body = (over: Record<string, string> = {}) => {
    const values: Record<string, string> = {
      laser_type: 'Diode / Diodo', laser_watts: '22', machine: 'Falcon2 22 W', material: 'Basswood plywood / Contrachapado de tilo', thickness: '3',
      operation: 'Cut / Corte', speed: '8 mm/s', power: '100', passes: '2', interval: '', air: 'Yes / Sí', focus: 'surface', result: 'Clean cut.', photo: '',
      confirm: '- [X] I tested these values on my machine. / Probé estos valores en mi máquina.\n- [X] I authorize publishing them under the CC0 1.0 license (public domain), so the library can be redistributed. / Autorizo publicarlos bajo la licencia CC0 1.0 (dominio público), para que la biblioteca pueda redistribuirse.',
      ...over,
    };
    return fieldsOf('material_settings.yml')
      .filter((f) => f.id)
      .map((f) => `### ${f.label}\n\n${values[f.id!] || '_No response_'}`)
      .join('\n\n');
  };

  it('genera el aporte con mm/s convertido a mm/min', () => {
    const e = bodyToPreset(body(), 12, { formText: formLabels(), author: 'alguien' });
    expect(e).toMatchObject({
      id: 'community.plywood_basswood.3.cut.diode_20.12', material: 'plywood_basswood', thickness: 3, role: 'cut', machine_class: 'diode_20', laser_watts: 22,
      speed: 480, power: 100, passes: 2, air: true, source: `${REPO_URL}/issues/12`, confidence: 'forum', license: 'CC0-1.0', contributor: 'alguien',
    });
    expect(e.material_name).toBeUndefined();
  });

  it('un material desconocido lleva su nombre y las casillas son obligatorias', () => {
    const e = bodyToPreset(body({ material: 'Felt', operation: 'Engrave / Grabado', interval: '0.1', thickness: '' }), 5, { formText: formLabels() });
    expect(e).toMatchObject({ material: 'felt', material_name: 'Felt', role: 'engrave', interval: 0.1 });
    expect(e.thickness).toBeUndefined();
    expect(() => bodyToPreset(body({ confirm: '- [X] a\n- [ ] b' }), 5, { formText: formLabels() })).toThrow(/casillas/);
    expect(() => bodyToPreset('hola', 5, { formText: formLabels() })).toThrow(/formulario/);
  });

  it('toma la versión de Tlaku, la fecha del issue y el modelo declarado por quien revisa', () => {
    const e = bodyToPreset(body({ tlaku_version: '0.1.0' }), 12, { formText: formLabels(), date: '2026-10-01', machineModel: 'creality-falcon2-22w' });
    expect(e).toMatchObject({ tlaku_version: '0.1.0', date: '2026-10-01', machine_model: 'creality-falcon2-22w' });
    // Valores mal formados no entran al archivo.
    const bad = bodyToPreset(body({ tlaku_version: 'última' }), 13, { formText: formLabels(), date: 'ayer', machineModel: 'Falcon 2' });
    expect(bad).not.toHaveProperty('tlaku_version');
    expect(bad).not.toHaveProperty('date');
    expect(bad).not.toHaveProperty('machine_model');
  });

  it('convierte unidades de velocidad', () => {
    expect(parseSpeed('20 mm/s')).toBe(1200);
    expect(parseSpeed('1200')).toBe(1200);
    expect(parseSpeed('1200 mm/min')).toBe(1200);
  });
});
