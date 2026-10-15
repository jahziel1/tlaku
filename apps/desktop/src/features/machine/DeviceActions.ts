import { getApi } from '@/api';
import type { DeviceProfile, Document } from '@/api/types';
import { t } from '@/i18n';
import { errorMessage } from '@/lib/errors';
import { fmt } from '@/lib/platform';
import { rotaryCircumference } from '@/lib/device';
import { useStore } from '@/store';

/** Valores con los que se propone la prueba de una vuelta: lenta y a poca potencia. */
export const ROTARY_TEST_SPEED = 1500;
export const ROTARY_TEST_POWER = 10;

/** `true` si la velocidad o la potencia de la prueba no sirven (velocidad > 0, potencia entre 0 y 100 %). */
export function isBadRotaryTest(speed: number, power: number): boolean {
  return !(Number.isFinite(speed) && speed > 0 && Number.isFinite(power) && power > 0 && power <= 100);
}

/**
 * Genera la prueba de una vuelta con el motor y la abre como documento nuevo, con el aviso de
 * cambios sin guardar de "Nuevo" (igual que la prueba de materiales). Se genera antes de
 * preguntar, así un error del motor no cuesta una pregunta ni el documento actual. Devuelve si se abrió.
 */
export async function openRotaryTestAsNewDocument(device: DeviceProfile, speed: number, power: number): Promise<boolean> {
  const app = () => useStore.getState();
  let doc: Document;
  try {
    doc = await getApi().rotaryTest({ device, speed, power });
  } catch (e) {
    app().notify('error', t('devices.rotary.test.err', { error: errorMessage(e) }));
    return false;
  }
  const s = app();
  s.openInTab({ ...doc, device_id: s.doc.device_id }, null);
  s.fitContent();
  void s.computePreview();
  app().notify('success', t('devices.rotary.test.done', { mm: fmt(rotaryCircumference(device.rotary) ?? 0, 1) }));
  return true;
}
