import { renderToString } from 'react-dom/server';
import { describe, expect, it } from 'vitest';
import { t } from '@/i18n';
import { commandWarning } from './CommandConfirm';
import { fixedLine } from './commandSafety';
import { stateLabel, stateTone } from './StatusSection';
import { VirtualBadge } from './VirtualBadge';

describe('distintivo de máquina virtual', () => {
  it('dice "Virtual" y explica que no se graba nada', () => {
    const h = renderToString(<VirtualBadge />);
    expect(h).toContain('Virtual');
    expect(h).toContain('no hay láser ni hardware reales');
    expect(h).toContain('role="img"');
  });
});

describe('estado desconocido (other)', () => {
  it('se muestra con el nombre original que informó la controladora', () => {
    expect(stateLabel('other', 'Tool')).toBe('Tool');
    expect(stateLabel('other', null)).toBe(t('machine.state.other'));
    expect(stateLabel('other')).toBe('Ocupada');
    expect(stateLabel('idle', 'Tool')).toBe('Lista');
    expect(stateLabel(null)).toBe('Sin conexión');
  });

  it('cuenta como ocupada, no como lista ni como alarma', () => {
    expect(stateTone('other')).toBe('accent');
  });
});

describe('confirmación de comandos destructivos', () => {
  it('$RST y $SLP muestran la advertencia destructiva; $X conserva la suya', () => {
    for (const line of ['$RST=$', '$RST=#', '$RST=*', '$SLP']) {
      expect(commandWarning(fixedLine(line)), line).toBe('machine.risk.destructive');
    }
    expect(t('machine.risk.destructive')).toMatch(/no se puede deshacer/i);
    expect(commandWarning(fixedLine('$X'))).toBe('machine.risk.unlock');
    expect(commandWarning(fixedLine('$32=1'))).toBe('machine.risk.setting');
    expect(commandWarning(fixedLine('$$'))).toBeNull();
  });
});
