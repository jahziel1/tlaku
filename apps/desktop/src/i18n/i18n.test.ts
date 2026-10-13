import { describe, expect, it } from 'vitest';
import { canvas } from './es/canvas';
import { history } from './es/history';
import { layers } from './es/layers';
import { machine } from './es/machine';
import { preview } from './es/preview';

describe('diccionarios por área', () => {
  it('cada clave lleva el prefijo de su área, así ningún archivo pisa a otro', () => {
    const areas = { canvas, history, layers, machine, preview };
    for (const [area, dict] of Object.entries(areas)) {
      for (const key of Object.keys(dict)) expect(key.startsWith(`${area}.`), key).toBe(true);
    }
  });
});
