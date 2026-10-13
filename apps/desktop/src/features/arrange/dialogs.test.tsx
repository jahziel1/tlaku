import { renderToString } from 'react-dom/server';
import { describe, expect, it } from 'vitest';
import { AlignDialog } from './AlignDialog';
import { ArrayDialog } from './ArrayDialog';
import { BooleanDialog } from './BooleanDialog';
import { ArrangeDialogHost } from './index';
import { OffsetDialog } from './OffsetDialog';

/**
 * Humo: los diálogos se dibujan sin errores. (El render en servidor ve el estado inicial del store,
 * es decir, sin selección; por eso se comprueban los títulos y el motivo de "nada seleccionado".)
 */
describe('diálogos de Organizar', () => {
  it('sin diálogo abierto no pinta nada', () => {
    expect(renderToString(<ArrangeDialogHost />)).toBe('');
  });

  it('diferencia, intersección y xor', () => {
    expect(renderToString(<BooleanDialog op="difference" />)).toContain('Diferencia: A menos B');
    expect(renderToString(<BooleanDialog op="intersection" />)).toContain('Intersección con una forma principal');
    expect(renderToString(<BooleanDialog op="xor" />)).toContain('Selecciona uno o más objetos.');
  });

  it('offset, alinear y arreglos', () => {
    expect(renderToString(<OffsetDialog />)).toContain('Conservar el original');
    const align = renderToString(<AlignDialog />);
    expect(align).toContain('Distribuir con espacio igual');
    expect(align).toContain('aria-disabled="true"');
    expect(renderToString(<ArrayDialog kind="grid" />)).toContain('Arreglo en rejilla');
    expect(renderToString(<ArrayDialog kind="circular" />)).toContain('Arreglo circular');
  });
});
