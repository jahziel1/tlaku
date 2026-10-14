import { readFileSync } from 'node:fs';
import { describe, expect, it } from 'vitest';
import { contrastRatio } from '@/lib/palette';

/**
 * WCAG 2.1 AA sobre los tokens de `tokens.css`: 4,5:1 para texto (1.4.3) y 3:1 para el borde de los
 * controles y el anillo de foco (1.4.11). `--border` es solo separador decorativo y no se mide.
 */
const css = readFileSync(new URL('./tokens.css', import.meta.url), 'utf8');

function theme(selector: string): Record<string, string> {
  const start = css.indexOf(selector);
  const body = css.slice(start, css.indexOf('}', start));
  return Object.fromEntries([...body.matchAll(/--([\w-]+):\s*(#[0-9a-fA-F]{6})\s*;/g)].map((m) => [m[1]!, m[2]!]));
}

const dark = theme(":root,\n:root[data-theme='dark']");
const light = theme(":root[data-theme='light']");

const SURFACES = ['bg-app', 'bg-chrome', 'bg-raised', 'bg-sunken'];
const TEXT = ['text', 'text-muted', 'text-faint', 'accent-ink', 'danger', 'ok', 'warn'];
const UI = [
  ['border-strong', ['bg-chrome', 'bg-raised', 'bg-sunken', 'bg-hover']],
  ['focus', SURFACES],
] as const;

describe.each([['oscuro', dark], ['claro', light]] as const)('contraste, tema %s', (_name, v) => {
  it('el texto cumple 4,5:1 sobre las superficies', () => {
    for (const fg of TEXT) {
      // El color de aviso `warn` solo se usa sobre las barras y tarjetas, no sobre el fondo de la ventana.
      for (const bg of SURFACES.filter((s) => s !== 'bg-app' || !['ok', 'warn', 'danger'].includes(fg))) {
        expect(contrastRatio(v[fg]!, v[bg]!), `${fg} sobre ${bg}`).toBeGreaterThanOrEqual(4.5);
      }
    }
    expect(contrastRatio(v['text-faint']!, v['bg-hover']!), 'text-faint sobre bg-hover').toBeGreaterThanOrEqual(4.5);
    expect(contrastRatio(v['on-accent']!, v.accent!), 'texto del botón de acento').toBeGreaterThanOrEqual(4.5);
    expect(contrastRatio(v['on-danger']!, v.danger!), 'texto del botón de peligro').toBeGreaterThanOrEqual(4.5);
    expect(contrastRatio(v['ruler-ink']!, v['ruler-bg']!), 'regla').toBeGreaterThanOrEqual(4.5);
  });

  it('los bordes de controles y el foco cumplen 3:1', () => {
    for (const [fg, bgs] of UI) {
      for (const bg of bgs) expect(contrastRatio(v[fg]!, v[bg]!), `${fg} sobre ${bg}`).toBeGreaterThanOrEqual(3);
    }
    expect(contrastRatio(v['ruler-tick']!, v['ruler-bg']!), 'marcas de la regla').toBeGreaterThanOrEqual(3);
  });
});
