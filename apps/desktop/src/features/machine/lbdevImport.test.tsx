// @vitest-environment jsdom
import { afterEach, beforeEach, describe, expect, it } from 'vitest';
import { setApi } from '@/api';
import { createMockApi, type MockApi } from '@/api/mock';
import { mockReadLbdev } from '@/api/mock/devices';
import type { LbdevNotice, LbdevNoticeKind } from '@/api/types';
import { setLanguage } from '@/i18n';
import { useStore } from '@/store';
import { audit, cleanup, click, describeViolations, mount } from '@/test/dom';
import { LbdevImportDialog } from './LbdevImportDialog';
import { lbdevNoticeText, MOCK_LBDEV_PATH, readLbdevPreview } from './lbdevImport';

let api: MockApi;

beforeEach(() => {
  setLanguage('es');
  api = createMockApi({ latencyMs: 0 });
  setApi(api);
});

afterEach(async () => {
  await cleanup();
  api.dispose();
});

const KINDS: LbdevNoticeKind[] = [
  'not_imported',
  'invalid_value',
  'missing_value',
  'unsupported_controller',
  'skipped_entry',
  'simulator_values',
  'script_code',
  'z_axis_check',
  'origin_inferred',
  'connection_type',
  'ruida_host',
];

describe('importar perfil de LightBurn', () => {
  it('cada tipo de aviso tiene texto propio en español e inglés', () => {
    const spanish = new Set<string>();
    for (const kind of KINDS) {
      const n: LbdevNotice = { device: 'Taller', kind, detail: kind === 'script_code' ? 'start, end' : 'X' };
      setLanguage('es');
      const es = lbdevNoticeText(n);
      setLanguage('en');
      const en = lbdevNoticeText(n);
      expect(es, kind).not.toMatch(/devices\.lbdev|\{/);
      expect(en, kind).not.toMatch(/devices\.lbdev|\{/);
      expect(en, kind).not.toBe(es);
      spanish.add(es);
    }
    expect(spanish.size).toBe(KINDS.length);
    setLanguage('es');
    expect(lbdevNoticeText({ device: 'T', kind: 'script_code', detail: 'start, end' })).toContain('inicio / fin');
  });

  it('el aviso de simulación manda a contrastar con $$', () => {
    setLanguage('es');
    const text = lbdevNoticeText({ device: 'T', kind: 'simulator_values', detail: '' });
    expect(text).toContain('$$');
    expect(text).toContain('Leer de la máquina');
  });

  it('el mock lee el perfil de ejemplo y falla con una ruta ausente', async () => {
    const preview = await readLbdevPreview();
    expect(preview?.file).toBe(MOCK_LBDEV_PATH);
    expect(preview?.result.devices[0]!.max_rate).toEqual([30000, 24000]);
    expect(() => mockReadLbdev('no-existe.lbdev')).toThrow(/no existe/);
  });

  it('el diálogo resume perfiles y avisos, y solo guarda al confirmar', async () => {
    const preview = (await readLbdevPreview())!;
    const before = useStore.getState().devices.length;
    let saved: string[] | null = null;
    await mount(<LbdevImportDialog preview={preview} onDone={(s) => (saved = s.map((d) => d.id))} onCancel={() => {}} />);
    const dialog = document.querySelector('[role="dialog"]')!;
    expect(dialog.textContent).toContain('Perfil de ejemplo');
    expect(dialog.textContent).toContain('400 × 415 mm');
    expect(dialog.textContent).toContain('no importado: LaserFire_Enable, LaserFire_Power');
    expect(dialog.textContent).toContain('$$');
    expect(useStore.getState().devices.length).toBe(before);
    expect(saved).toBeNull();
    const confirm = [...dialog.querySelectorAll('button')].find((b) => /Crear 1 perfil/.test(b.textContent ?? ''))!;
    await click(confirm);
    await new Promise((r) => setTimeout(r, 20));
    expect(saved).toEqual(['perfil-de-ejemplo']);
    expect(useStore.getState().devices.some((d) => d.id === 'perfil-de-ejemplo')).toBe(true);
    const violations = describeViolations(await audit(dialog as HTMLElement));
    expect(violations).toEqual([]);
  });
});
