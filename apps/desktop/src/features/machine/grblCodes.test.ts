import { describe, expect, it } from 'vitest';
import { es } from '@/i18n';
import { ALARM_CODES, alarmAdvice, alarmKey, ERROR_CODES, errorKey, isStatusReport, parseAlarmLine, parseErrorLine } from './grblCodes';

describe('consejos ante una alarma (mismas reglas que el CLI)', () => {
  it('fallo de sonda: solo tiene sentido desbloquear', () => {
    const a = alarmAdvice(5, true);
    expect(a.actions).toEqual(['unlock']);
    expect(a.recommended).toBe('unlock');
  });

  it('fallos de homing (6 a 9): revisar finales de carrera y repetir $H', () => {
    for (const code of [6, 7, 8, 9]) {
      const a = alarmAdvice(code, null);
      expect(a.tips).toContain('checkLimits');
      expect(a.actions).toEqual(['home', 'unlock']);
      expect(a.recommended).toBe('home');
    }
  });

  it('límite físico con homing activado: apartar el cabezal, $X y luego $H', () => {
    const a = alarmAdvice(1, true);
    expect(a.tips).toEqual(['moveAwayThenUnlock', 'homeAfter']);
    expect(a.actions).toEqual(['unlock', 'home']);
  });

  it('límite físico sin homing: solo apartar y desbloquear', () => {
    const a = alarmAdvice(2, false);
    expect(a.tips).toEqual(['moveAway']);
    expect(a.actions).toEqual(['unlock']);
  });

  it('sin código: manda el ajuste $22', () => {
    expect(alarmAdvice(null, false)).toMatchObject({ actions: ['unlock'], recommended: 'unlock', tips: ['homingOff'] });
    expect(alarmAdvice(null, true)).toMatchObject({ actions: ['home', 'unlock'], recommended: 'home', tips: ['homingOn'] });
  });

  it('sin código ni $22 no recomienda nada y explica las dos opciones', () => {
    const a = alarmAdvice(null, null);
    expect(a.recommended).toBeNull();
    expect(a.tips).toEqual(['ifLimitSwitches', 'ifNoLimitSwitches']);
    expect(a.actions).toEqual(['home', 'unlock']);
  });

  it('reinicio durante un movimiento (3): avisa de que se perdió la posición', () => {
    expect(alarmAdvice(3, null).tips[0]).toBe('positionLost');
  });
});

describe('líneas de alarma y error', () => {
  it('extrae el código con o sin explicación del motor', () => {
    expect(parseAlarmLine('ALARM:9')).toBe(9);
    expect(parseAlarmLine('ALARM:1: límite físico activado')).toBe(1);
    expect(parseAlarmLine('error:20')).toBeNull();
    expect(parseErrorLine('error:20: comando no soportado')).toBe(20);
    expect(parseErrorLine('ok')).toBeNull();
  });

  it('reconoce los reportes de estado', () => {
    expect(isStatusReport('<Idle|MPos:0.000,0.000,0.000|FS:0,0>')).toBe(true);
    expect(isStatusReport('ok')).toBe(false);
    expect(isStatusReport('[MSG:<algo>]')).toBe(false);
  });

  it('todos los códigos conocidos tienen texto y los desconocidos caen en uno genérico', () => {
    for (const c of ALARM_CODES) expect(es).toHaveProperty(alarmKey(c));
    for (const c of ERROR_CODES) expect(es).toHaveProperty(errorKey(c));
    expect(alarmKey(99)).toBe('machine.alarm.unknown');
    expect(alarmKey(null)).toBe('machine.alarm.unknown');
    expect(errorKey(99)).toBe('machine.error.unknown');
  });
});
