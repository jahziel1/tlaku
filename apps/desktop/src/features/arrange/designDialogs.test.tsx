import { renderToString } from 'react-dom/server';
import { describe, expect, it } from 'vitest';
import { AlongPathDialog } from './AlongPathDialog';
import { BooleanAssistantDialog } from './BooleanAssistantDialog';
import { DesignDialogHost } from './DesignDialogHost';
import { DockDialog } from './DockDialog';
import { HullDialog } from './HullDialog';
import { TangentDialog } from './TangentDialog';
import { VirtualArrayDialog } from './VirtualArrayDialog';
import { WarpDialog } from './WarpDialog';

/** Humo: los diálogos se dibujan sin errores (el render en servidor ve el estado inicial, sin selección). */
describe('diálogos de diseño avanzado', () => {
  it('sin diálogo abierto no pinta nada', () => {
    expect(renderToString(<DesignDialogHost />)).toBe('');
  });

  it('cada diálogo muestra su título y el motivo de "nada seleccionado"', () => {
    const cases: [string, string][] = [
      [renderToString(<WarpDialog />), 'Deformar'],
      [renderToString(<HullDialog />), 'Contorno de banda elástica'],
      [renderToString(<TangentDialog />), 'Círculo tangente'],
      [renderToString(<BooleanAssistantDialog />), 'Asistente booleano'],
      [renderToString(<AlongPathDialog />), 'Copiar a lo largo de un trazo'],
      [renderToString(<DockDialog />), 'Acoplar y mover juntos'],
      [renderToString(<VirtualArrayDialog />), 'Arreglo virtual'],
    ];
    for (const [html, title] of cases) {
      expect(html).toContain(title);
      expect(html).toContain('Selecciona uno o más objetos.');
    }
  });

  it('el asistente booleano muestra las cinco operaciones', () => {
    const html = renderToString(<BooleanAssistantDialog />);
    for (const name of ['Unión', 'A menos B', 'Intersección', 'Diferencia simétrica', 'Soldar']) expect(html).toContain(name);
  });

  it('los presets de deformar son botones', () => {
    const html = renderToString(<WarpDialog />);
    for (const name of ['Arco hacia arriba', 'Abombar', 'Pellizcar', 'Perspectiva']) expect(html).toContain(name);
  });
});
