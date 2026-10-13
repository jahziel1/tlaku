import { describe, expect, it } from 'vitest';
import { arrange } from './es/arrange';
import { camera } from './es/camera';
import { canvas } from './es/canvas';
import { catalog } from './es/catalog';
import { codes } from './es/codes';
import { devices } from './es/devices';
import { fileio } from './es/fileio';
import { help } from './es/help';
import { history } from './es/history';
import { layers } from './es/layers';
import { machine } from './es/machine';
import { nodes } from './es/nodes';
import { preview } from './es/preview';
import { prefs } from './es/prefs';
import { text } from './es/text';
import { trace } from './es/trace';
import { materials } from './es/materials';

describe('diccionarios por área', () => {
  it('cada clave lleva el prefijo de su área, así ningún archivo pisa a otro', () => {
    const areas = { canvas, help, history, layers, machine, preview, arrange, text, nodes, trace, camera, materials, devices, catalog, prefs, codes, fileio };
    for (const [area, dict] of Object.entries(areas)) {
      for (const key of Object.keys(dict)) expect(key.startsWith(`${area}.`), key).toBe(true);
    }
  });
});
