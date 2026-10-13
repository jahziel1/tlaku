import { getApi } from '@/api';
import { t } from '@/i18n';
import { errorMessage } from '@/lib/errors';

export type ConnectionTestResult = { ok: true; text: string } | { ok: false; text: string };

/**
 * «Probar conexión»: abre la dirección, lee el saludo de la máquina y cierra, sin mover nada ni
 * cambiar ningún ajuste (el motor solo envía las consultas de solo lectura de la detección).
 */
export async function testConnection(portId: string, baud: number): Promise<ConnectionTestResult> {
  try {
    const r = await getApi().probePort({ port: portId, baud });
    return { ok: true, text: r.firmware ? t('devices.conn.test.ok', { firmware: r.firmware }) : t('devices.conn.test.okUnknown') };
  } catch (e) {
    return { ok: false, text: t('devices.conn.test.fail', { error: errorMessage(e) }) };
  }
}
