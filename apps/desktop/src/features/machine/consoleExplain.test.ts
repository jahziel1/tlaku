import { describe, expect, it } from 'vitest';
import { t } from '@/i18n';
import { classifyLine } from './consoleExplain';
import { CREALITY_CODES } from './grblCodes';

describe('consola: qué se explica de cada línea', () => {
  it('ERROR:NN de Creality se explica con su significado y un consejo', () => {
    const flame = classifyLine('in', 'ERROR:05', 'grbl');
    expect(flame.kind).toBe('alarm');
    expect(flame.explain).toEqual(['machine.creality.5', 'machine.creality.recovery']);
    expect(t(flame.explain[0]!)).toMatch(/llama/i);
    expect(t(classifyLine('in', 'ERROR:01', 'grbl').explain[0]!)).toMatch(/inclinación/i);
    expect(t(classifyLine('in', 'ERROR:24', 'grbl').explain[0]!)).toMatch(/tapa/i);
    expect(t('machine.creality.recovery')).toMatch(/Reset/);
  });

  it('con el punto final y los códigos de sensores o desconocidos', () => {
    expect(classifyLine('in', 'ERROR:06.', 'grbl').explain[0]).toBe('machine.creality.6');
    expect(classifyLine('in', 'ERROR:09', 'grbl').explain[0]).toBe('machine.creality.sensors');
    expect(classifyLine('in', 'ERROR:99', 'grbl').explain[0]).toBe('machine.creality.unknown');
    for (const c of CREALITY_CODES) expect(classifyLine('in', `ERROR:${String(c).padStart(2, '0')}`, 'unknown').explain).toHaveLength(2);
  });

  it('si el motor ya puso el significado, solo agrega el consejo', () => {
    const l = classifyLine('in', 'ERROR:05: Llama detectada: la máquina se detuvo.', 'grbl');
    expect(l.kind).toBe('alarm');
    expect(l.explain).toEqual(['machine.creality.recovery']);
  });

  it('el ERROR:05 de Creality no se lee con la tabla de error:5 de GRBL', () => {
    expect(classifyLine('in', 'ERROR:05', 'grbl').explain).not.toContain('machine.error.5');
    expect(classifyLine('in', 'error:5', 'grbl')).toEqual({ kind: 'error', explain: ['machine.error.5'] });
  });

  it('ALARM:10 y los error:N extendidos se leen según la familia del firmware', () => {
    expect(classifyLine('in', 'ALARM:10', 'grbl').explain).toEqual(['machine.alarm.10.grbl']);
    expect(classifyLine('in', 'ALARM:10', 'grblhal').explain).toEqual(['machine.alarm.10.grblhal']);
    expect(classifyLine('in', 'ALARM:10', 'fluidnc').explain).toEqual(['machine.alarm.10.fluidnc']);
    expect(classifyLine('in', 'ALARM:10', 'unknown').explain).toEqual(['machine.alarm.10']);
    expect(classifyLine('in', 'error:60', 'grblhal').explain).toEqual(['machine.error.hal.sd']);
    expect(classifyLine('in', 'error:60', 'fluidnc').explain).toEqual(['machine.error.fnc.sd']);
    expect(classifyLine('in', 'error:60', 'grbl').explain).toEqual(['machine.error.unknown']);
    expect(classifyLine('in', 'error:60', 'unknown').explain).toEqual(['machine.error.extended']);
  });

  it('lo que el motor ya explicó no se explica dos veces', () => {
    expect(classifyLine('in', 'ALARM:3: reinicio durante un movimiento', 'grbl')).toEqual({ kind: 'alarm', explain: [] });
    expect(classifyLine('in', 'error:20: comando no soportado', 'grbl')).toEqual({ kind: 'error', explain: [] });
  });

  it('lo enviado y lo demás no se explica', () => {
    expect(classifyLine('out', 'ERROR:05', 'grbl')).toEqual({ kind: 'plain', explain: [] });
    expect(classifyLine('out', 'ALARM:3', 'grbl')).toEqual({ kind: 'plain', explain: [] });
    expect(classifyLine('in', 'ok', 'grbl')).toEqual({ kind: 'ok', explain: [] });
    expect(classifyLine('in', '<Idle|MPos:0,0,0>', 'grbl')).toEqual({ kind: 'status', explain: [] });
    expect(classifyLine('in', '[MSG:Caution: Unlocked]', 'grbl')).toEqual({ kind: 'plain', explain: [] });
    expect(classifyLine('in', 'ERRORS:05', 'grbl').explain).toEqual([]);
  });
});
