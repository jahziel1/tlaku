import { renderToString } from 'react-dom/server';
import { describe, expect, it } from 'vitest';
import { es } from '@/i18n';
import { SettingsDialog } from './SettingsDialog';
import { SETTINGS_SECTIONS } from './uiStore';

describe('ventana de Preferencias', () => {
  // Con renderizado en servidor zustand lee siempre el estado inicial: se comprueba la estructura de la sección General.
  it('pinta un diálogo con las cinco secciones como pestañas accesibles', () => {
    const html = renderToString(<SettingsDialog />);
    expect(html).toContain('role="dialog"');
    expect(html).toContain('role="tablist"');
    expect((html.match(/role="tab"/g) ?? []).length).toBe(SETTINGS_SECTIONS.length);
    expect(html).toContain('aria-selected="true"');
    expect(html).toContain('role="tabpanel"');
    expect(html).toContain('Idioma');
    expect(html).not.toMatch(/prefs\.[a-z]/); // ninguna clave sin traducir
  });

  it('cada sección tiene su nombre en el diccionario', () => {
    for (const id of SETTINGS_SECTIONS) expect(es).toHaveProperty(`prefs.section.${id}`);
  });
});
