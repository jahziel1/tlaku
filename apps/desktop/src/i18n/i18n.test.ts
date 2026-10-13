import { describe, expect, it } from 'vitest';
import { arrange } from './es/arrange';
import { camera } from './es/camera';
import { canvas } from './es/canvas';
import { history } from './es/history';
import { layers } from './es/layers';
import { machine } from './es/machine';
import { nodes } from './es/nodes';
import { preview } from './es/preview';
import { text } from './es/text';
import { trace } from './es/trace';

describe('diccionarios por área', () => {
  it('cada clave lleva el prefijo de su área, así ningún archivo pisa a otro', () => {
    const areas = { canvas, history, layers, machine, preview, arrange, text, nodes, trace, camera };
    for (const [area, dict] of Object.entries(areas)) {
      for (const key of Object.keys(dict)) expect(key.startsWith(`${area}.`), key).toBe(true);
    }
  });
});
