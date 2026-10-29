import { useMaterials } from '@/features/materials/state';
import { selectActiveDevice, type AppState } from '@/store';
import { selectJobOptions } from '@/store/selectors';
import { makeSnapshot, type StartSnapshot } from './model';

/** La huella del estado de ahora (la misma que se compara con la del trabajo confirmado con la lista completa). */
export function currentSnapshot(s: AppState): StartSnapshot {
  const ui = useMaterials.getState();
  return makeSnapshot({
    connection: s.connection,
    deviceId: selectActiveDevice(s).id,
    doc: s.doc,
    options: selectJobOptions(s),
    presets: [...ui.library.presets, ...ui.factory.presets],
  });
}
