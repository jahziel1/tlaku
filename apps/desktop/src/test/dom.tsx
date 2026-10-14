/**
 * Utilidades de pruebas de accesibilidad con jsdom: montan un componente de verdad (con efectos y
 * foco), lo recorren con axe-core y simulan teclas. Cada archivo de prueba que las use declara
 * `// @vitest-environment jsdom` en su primera línea; el resto de las pruebas sigue en `node`.
 */
import axe, { type AxeResults } from 'axe-core';
import { act, type ReactNode } from 'react';
import { createRoot, type Root } from 'react-dom/client';

(globalThis as { IS_REACT_ACT_ENVIRONMENT?: boolean }).IS_REACT_ACT_ENVIRONMENT = true;

const live = new Set<Mounted>();

/** Desmonta todo lo montado con `mount` (úsalo en `afterEach` para que una prueba que falla no contamine a las demás). */
export async function cleanup(): Promise<void> {
  for (const m of [...live]) await m.unmount();
  document.body.innerHTML = '';
}

export interface Mounted {
  host: HTMLElement;
  root: Root;
  unmount: () => Promise<void>;
  rerender: (ui: ReactNode) => Promise<void>;
}

/** Monta `ui` en un contenedor dentro de `document.body`. */
export async function mount(ui: ReactNode): Promise<Mounted> {
  const host = document.createElement('div');
  document.body.append(host);
  const root = createRoot(host);
  await act(async () => root.render(ui));
  const mounted: Mounted = {
    host,
    root,
    rerender: async (next) => {
      await act(async () => root.render(next));
    },
    unmount: async () => {
      if (!live.delete(mounted)) return;
      await act(async () => root.unmount());
      host.remove();
    },
  };
  live.add(mounted);
  return mounted;
}

/** Pulsa una tecla sobre `el` (o el elemento enfocado) y espera a que React termine. */
export async function press(key: string, init: KeyboardEventInit = {}, el: Element | null = document.activeElement): Promise<void> {
  const target = el ?? document.body;
  await act(async () => {
    target.dispatchEvent(new KeyboardEvent('keydown', { key, bubbles: true, cancelable: true, ...init }));
  });
}

export async function click(el: Element): Promise<void> {
  await act(async () => {
    (el as HTMLElement).click();
  });
}

/**
 * Corre axe sobre `scope`. Se apagan las reglas que necesitan maquetación real (contraste de color,
 * regiones de página, enlaces en bloques de texto), que jsdom no calcula; el contraste se comprueba
 * aparte, con los tokens (`contrast.test.ts`).
 */
export async function audit(scope: Element = document.body): Promise<AxeResults> {
  return axe.run(scope, {
    runOnly: { type: 'tag', values: ['wcag2a', 'wcag2aa', 'wcag21a', 'wcag21aa', 'best-practice'] },
    rules: {
      'color-contrast': { enabled: false },
      region: { enabled: false },
      'scrollable-region-focusable': { enabled: false },
      'landmark-one-main': { enabled: false },
      // Falsos avisos con un diálogo abierto: su <header>/<footer> se cuenta como banner de la página.
      'landmark-no-duplicate-banner': { enabled: false },
      'landmark-no-duplicate-contentinfo': { enabled: false },
      'landmark-unique': { enabled: false },
      'page-has-heading-one': { enabled: false },
    },
  });
}

/** Resumen legible de las violaciones, para el mensaje de `expect`. */
export function describeViolations(r: AxeResults): string[] {
  return r.violations.map((v) => `${v.id}: ${v.help} -> ${v.nodes.map((n) => n.target.join(' ')).join(' | ')}`);
}
