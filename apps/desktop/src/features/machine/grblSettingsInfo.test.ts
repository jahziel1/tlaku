import { describe, expect, it } from 'vitest';
import { en, es } from '@/i18n';
import { GRBL_SETTINGS_INFO, isCriticalSetting, settingDescKey, settingInfo, settingNameKey } from './grblSettingsInfo';

describe('metadatos de los ajustes de GRBL', () => {
  it('cubre todos los ajustes de GRBL 1.1', () => {
    const base = [0, 1, 2, 3, 4, 5, 6, 10, 11, 12, 13, 20, 21, 22, 23, 24, 25, 26, 27, 30, 31, 32, 100, 101, 102, 110, 111, 112, 120, 121, 122, 130, 131, 132];
    for (const id of base) {
      expect(settingInfo(id), `$${id}`).toBeDefined();
      expect(settingInfo(id)?.halOnly, `$${id}`).toBeUndefined();
    }
  });

  it('los de grblHAL están marcados como tales', () => {
    for (const id of [14, 15, 16, 17, 18, 19, 28, 29, 33, 34, 35, 36, 37, 38, 39, 40, 43, 44, 45, 46, 60, 61, 62, 63, 64]) {
      expect(settingInfo(id)?.halOnly, `$${id}`).toBe(true);
    }
  });

  it('los ajustes críticos son exactamente los peligrosos', () => {
    const critical = GRBL_SETTINGS_INFO.filter((i) => i.critical).map((i) => i.id);
    expect(critical).toEqual([3, 5, 20, 21, 22, 23, 30, 32, 100, 101, 102, 110, 111, 112, 120, 121, 122, 130, 131, 132]);
    expect(isCriticalSetting(30)).toBe(true);
    expect(isCriticalSetting(0)).toBe(false);
    expect(isCriticalSetting(999)).toBe(false);
  });

  it('cada ajuste tiene nombre y descripción en español y en inglés, distintos entre sí', () => {
    for (const { id } of GRBL_SETTINGS_INFO) {
      for (const key of [settingNameKey(id), settingDescKey(id)]) {
        expect(es[key], key).toBeTruthy();
        expect(en[key], key).toBeTruthy();
        expect(en[key], key).not.toBe(es[key]);
      }
    }
  });

  it('un ajuste desconocido usa el texto «Ajuste sin descripción»', () => {
    expect(settingNameKey(999)).toBe('mset.unknown.name');
    expect(es[settingNameKey(999)]).toBe('Ajuste sin descripción');
    expect(settingDescKey(999)).toBe('mset.unknown.desc');
  });

  it('no hay números repetidos y salen ordenados', () => {
    const ids = GRBL_SETTINGS_INFO.map((i) => i.id);
    expect(new Set(ids).size).toBe(ids.length);
    expect([...ids].sort((a, b) => a - b)).toEqual(ids);
  });
});
