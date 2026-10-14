// @vitest-environment jsdom
import { act } from 'react';
import { afterEach, beforeEach, describe, expect, it } from 'vitest';
import { setLanguage } from '@/i18n';
import { useStore } from '@/store';
import { cleanup, mount } from '@/test/dom';
import { SettingsDialog } from './SettingsDialog';
import { useSettingsUi } from './uiStore';
import { applyBundle } from './bundle';

beforeEach(() => {
  useStore.getState().resetPrefs();
  setLanguage('es');
  useSettingsUi.getState().openSettings('general');
});
afterEach(async () => {
  await cleanup();
  useSettingsUi.getState().closeSettings();
  useStore.getState().resetPrefs();
});

describe('modo de la interfaz en Preferencias (D18)', () => {
  it('la sección General deja elegir entre sencillo (principiantes), completo y preguntar', async () => {
    await mount(<SettingsDialog />);
    const select = [...document.querySelectorAll('select')].find((s) => s.closest('.field')?.textContent?.includes('Modo de la interfaz'))!;
    expect([...select.options].map((o) => o.value)).toEqual(['simple', 'full', 'ask']);
    expect(select.value).toBe('ask');
    expect(document.body.textContent).toContain('Sencillo (para principiantes)');
  });

  it('elegir el modo sencillo en Preferencias hace lo mismo que el botón de la barra', async () => {
    await mount(<SettingsDialog />);
    const select = [...document.querySelectorAll('select')].find((s) => s.closest('.field')?.textContent?.includes('Modo de la interfaz'))!;
    useStore.getState().setTool('line');
    const setter = Object.getOwnPropertyDescriptor(HTMLSelectElement.prototype, 'value')!.set!;
    await act(async () => {
      setter.call(select, 'simple');
      select.dispatchEvent(new Event('change', { bubbles: true }));
    });
    expect(useStore.getState().prefs.uiMode).toBe('simple');
    // Entrar al modo sencillo deja la herramienta de seleccionar.
    expect(useStore.getState().activeTool).toBe('select');
  });
});

describe('paquete de preferencias con el modo principiante de antes', () => {
  const known = new Set<string>();

  it('«beginnerMode» se traduce a modo sencillo si no había elegido modo, sin contarse como ignorada', () => {
    const current = { prefs: useStore.getState().prefs, shortcuts: {} };
    const r = applyBundle({ prefs: { beginnerMode: true } as never }, current, known);
    expect(r.prefs.uiMode).toBe('simple');
    expect(r.ignoredPrefs).toBe(0);
    expect('beginnerMode' in r.prefs).toBe(false);
  });

  it('si el paquete trae su propio modo, manda ese; si ya había elegido, se respeta', () => {
    const base = useStore.getState().prefs;
    const a = applyBundle({ prefs: { beginnerMode: true, uiMode: 'full' } as never }, { prefs: base, shortcuts: {} }, known);
    expect(a.prefs.uiMode).toBe('full');
    const b = applyBundle({ prefs: { beginnerMode: true } as never }, { prefs: { ...base, uiMode: 'full' }, shortcuts: {} }, known);
    expect(b.prefs.uiMode).toBe('full');
  });
});
