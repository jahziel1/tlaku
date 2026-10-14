/**
 * Diccionario en inglés. Mismas claves que `es.ts`, un archivo por área en `en/`. Una clave que
 * falte aquí se muestra en español (ver `t()`); `i18n.test.ts` exige que no falte ninguna.
 */
import type { I18nKey } from './index';
import { base } from './en/base';
import { arrange } from './en/arrange';
import { design } from './en/design';
import { art } from './en/art';
import { camera } from './en/camera';
import { devices } from './en/devices';
import { fileio } from './en/fileio';
import { canvas } from './en/canvas';
import { catalog } from './en/catalog';
import { codes } from './en/codes';
import { help } from './en/help';
import { history } from './en/history';
import { layers } from './en/layers';
import { machine } from './en/machine';
import { mset } from './en/mset';
import { materials } from './en/materials';
import { nodes } from './en/nodes';
import { preview } from './en/preview';
import { prefs } from './en/prefs';
import { bundle } from './en/bundle';
import { text } from './en/text';
import { trace } from './en/trace';
import { variables } from './en/variables';
import { nest } from './en/nest';
import { lhymicro } from './en/lhymicro';
import { ruida } from './en/ruida';
import { shapes } from './en/shapes';

export const en: { [K in I18nKey]?: string } = {
  ...base,
  ...arrange,
  ...art,
  ...design,
  ...camera,
  ...devices,
  ...fileio,
  ...canvas,
  ...catalog,
  ...codes,
  ...help,
  ...history,
  ...layers,
  ...machine,
  ...mset,
  ...materials,
  ...nodes,
  ...preview,
  ...prefs,
  ...bundle,
  ...text,
  ...variables,
  ...trace,
  ...nest,
  ...ruida,
  ...lhymicro,
  ...shapes,
};
