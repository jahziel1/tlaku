import { describe, expect, it } from 'vitest';
import { es } from '@/i18n';
import {
  ALARM_CODES,
  alarmAdvice,
  alarmKey,
  CREALITY_CODES,
  crealityKey,
  ERROR_CODES,
  errorKey,
  firmwareFamily,
  HAL_ERROR_CODES,
  isStatusReport,
  parseAlarmLine,
  parseCrealityError,
  parseErrorLine,
} from './grblCodes';

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
    // Por encima del 38 el significado depende del firmware.
    expect(errorKey(99)).toBe('machine.error.extended');
    expect(errorKey(0)).toBe('machine.error.unknown');
  });
});

describe('fallas de Creality (ERROR:NN)', () => {
  it('ERROR:05 no se lee como el error:5 de GRBL', () => {
    expect(parseErrorLine('ERROR:05')).toBeNull();
    expect(parseErrorLine('ERROR:04')).toBeNull();
    expect(parseErrorLine('error:5')).toBe(5);
    expect(parseCrealityError('ERROR:05')).toBe(5);
    expect(parseCrealityError('error:5')).toBeNull();
    expect(parseCrealityError('Error:5')).toBeNull();
  });

  it('tolera variantes: punto final, espacios, explicación del motor', () => {
    expect(parseCrealityError('ERROR:06.')).toBe(6);
    expect(parseCrealityError('  ERROR: 24 ')).toBe(24);
    expect(parseCrealityError('ERROR:01: ángulo anormal')).toBe(1);
    expect(parseCrealityError('ERROR:')).toBeNull();
    expect(parseCrealityError('ERROR:0x')).toBeNull();
    expect(parseCrealityError('ERROR:1234')).toBeNull();
    expect(parseCrealityError('ok')).toBeNull();
  });

  it('cada código conocido tiene texto, y los demás uno genérico', () => {
    for (const c of CREALITY_CODES) expect(es).toHaveProperty(crealityKey(c));
    for (const c of [8, 9, 10, 11, 12]) expect(crealityKey(c)).toBe('machine.creality.sensors');
    expect(es['machine.creality.5']).toMatch(/llama/i);
    expect(es['machine.creality.1']).toMatch(/inclinación/i);
    expect(es['machine.creality.6']).toMatch(/lente/i);
    expect(es['machine.creality.24']).toMatch(/tapa/i);
    expect(crealityKey(99)).toBe('machine.creality.unknown');
    expect(es).toHaveProperty('machine.creality.recovery');
  });
});

describe('códigos extendidos según el firmware', () => {
  it('reconoce la familia por el nombre de la conexión', () => {
    expect(firmwareFamily('Grbl 1.1h')).toBe('grbl');
    expect(firmwareFamily('FluidNC v3.9.7')).toBe('fluidnc');
    expect(firmwareFamily('grblHAL 1.1f')).toBe('grblhal');
    expect(firmwareFamily('GRBL')).toBe('grbl');
    expect(firmwareFamily(null)).toBe('unknown');
  });

  it('ALARM:10 es paro de emergencia en grblHAL y autoescuadre en GRBL 1.1', () => {
    expect(alarmKey(10, 'grblhal')).toBe('machine.alarm.10.grblhal');
    expect(alarmKey(10, 'grbl')).toBe('machine.alarm.10.grbl');
    expect(alarmKey(10, 'fluidnc')).toBe('machine.alarm.10.fluidnc');
    // Sin saber el firmware, el texto genérico dice que depende de él.
    expect(alarmKey(10)).toBe('machine.alarm.10');
    expect(es['machine.alarm.10']).toMatch(/depende del firmware/i);
    expect(es['machine.alarm.10.grblhal']).toMatch(/emergencia/);
    for (const f of ['grbl', 'grblhal', 'fluidnc'] as const) expect(es).toHaveProperty(alarmKey(10, f));
    for (const c of [11, 12, 22]) expect(es[alarmKey(c)]).toMatch(/grblHAL/);
  });

  it('los errores extendidos solo se interpretan si se conoce la familia', () => {
    for (const c of HAL_ERROR_CODES) expect(es).toHaveProperty(errorKey(c, 'grblhal'));
    expect(errorKey(50, 'grblhal')).toBe('machine.error.hal.50');
    expect(es['machine.error.hal.50']).toMatch(/emergencia/);
    expect(errorKey(63, 'grblhal')).toBe('machine.error.hal.sd');
    expect(errorKey(80, 'grblhal')).toBe('machine.error.hal.flow');
    // 63 y 80 significan otra cosa en FluidNC.
    expect(errorKey(63, 'fluidnc')).toBe('machine.error.fnc.sd');
    expect(errorKey(80, 'fluidnc')).toBe('machine.error.fnc.setting');
    expect(errorKey(50)).toBe('machine.error.extended');
    expect(errorKey(63)).toBe('machine.error.extended');
    // En GRBL 1.1 puro no existen.
    expect(errorKey(50, 'grbl')).toBe('machine.error.unknown');
    expect(errorKey(0)).toBe('machine.error.unknown');
    for (const code of [18, 19, 39, 60, 66, 70, 75, 77, 82, 89, 92, 100, 110, 120, 152]) {
      for (const f of ['grbl', 'grblhal', 'fluidnc', 'unknown'] as const) {
        expect(es, `${code} ${f}`).toHaveProperty(errorKey(code, f));
      }
    }
  });

  it('grblHAL: ALARM:11 pide homing y ALARM:10 no recomienda $X a ciegas', () => {
    expect(alarmAdvice(11, null)).toMatchObject({ recommended: 'home', actions: ['home', 'unlock'] });
    expect(alarmAdvice(10, true).recommended).toBeNull();
  });

  it('ALARM:3 menciona la alimentación, el USB y los sensores (Falcon2)', () => {
    expect(es['machine.alarm.3']).toMatch(/alimentación/);
    expect(es['machine.alarm.3']).toMatch(/USB/);
    expect(es['machine.alarm.3']).toMatch(/sensor/);
  });
});
