import { getApi, type EventMap } from '@/api';
import type { AutomationConfig, AutomationStatus } from '@/api/api';
import { openProject, importPaths } from '@/features/history/files';
import { t } from '@/i18n';
import { errorMessage } from '@/lib/errors';
import { getPref, selectActiveDevice, useStore, type Prefs } from '@/store';
import { sendBlockedReason } from '@/store/machine';
import { create } from 'zustand';

/**
 * Automatización por UDP, lado de la interfaz (M54). El motor (`src-tauri/src/automation.rs`) atiende
 * los comandos de texto; lo que depende del documento o de las opciones del trabajo se pide aquí con
 * los eventos `automation://load` y `automation://start`. Protocolo en `docs/app-api/automatizacion.md`.
 */

/** Estado del servidor, para mostrarlo en Preferencias. `null` mientras no se ha configurado. */
export const useAutomationStatus = create<{ status: AutomationStatus | null }>()(() => ({ status: null }));

/** Las preferencias que configuran el servidor. */
export function automationConfig(p: Pick<Prefs, 'automationEnabled' | 'automationPort' | 'automationAllowStart'>): AutomationConfig {
  return { enabled: p.automationEnabled, port: p.automationPort, allow_start: p.automationAllowStart };
}

/** Aplica la configuración en el motor y guarda el estado real (puerto ocupado, etc.). */
export async function applyAutomation(config: AutomationConfig): Promise<AutomationStatus> {
  let status: AutomationStatus;
  try {
    status = await getApi().automationConfigure({ config });
  } catch (e) {
    status = { running: false, port: config.port, error: errorMessage(e) };
  }
  useAutomationStatus.setState({ status });
  return status;
}

/** `LOAD <ruta>`: abre un proyecto `.tlaku` o importa el diseño, como si se soltara sobre la ventana. */
export async function handleAutomationLoad(path: string): Promise<void> {
  if (/\.tlaku$/i.test(path)) await openProject(path);
  else await importPaths([path]);
}

/**
 * `START`. Con `confirmed` (el usuario activó «permitir iniciar desde automatización») se inicia
 * directamente, salvo que la lista de comprobación del diálogo de inicio sea obligatoria: esa se
 * contesta siempre en el diálogo. Sin `confirmed`, se abre el diálogo de inicio y decide el usuario.
 */
export function handleAutomationStart(confirmed: boolean): void {
  const s = useStore.getState();
  const why = sendBlockedReason(s);
  if (why) {
    s.notify('warning', t('prefs.automation.startBlocked', { reason: t(why) }));
    return;
  }
  // Un galvo (haz invisible, clase 4) nunca arranca sin el diálogo de inicio, aunque la automatización esté permitida.
  if (confirmed && !getPref('jobChecklistRequired') && selectActiveDevice(s).kind !== 'galvo') {
    s.notify('info', t('prefs.automation.started'));
    void s.startJob();
    return;
  }
  s.notify('info', t('prefs.automation.startAsked'));
  s.requestStart();
}

/** Suscribe los eventos del motor y mantiene el servidor al día con las preferencias. Devuelve cómo deshacerlo. */
export function startAutomation(): () => void {
  const api = getApi();
  let stopped = false;
  const offs: (() => void)[] = [];
  const listen = <K extends 'automation://load' | 'automation://start'>(event: K, handler: (p: EventMap[K]) => void) => {
    void api.on(event, handler).then((off) => (stopped ? off() : offs.push(off)));
  };
  listen('automation://load', (p) => void handleAutomationLoad(p.path));
  listen('automation://start', (p) => handleAutomationStart(p.confirmed));

  const current = () => automationConfig(useStore.getState().prefs);
  void applyAutomation(current());
  const unsubscribe = useStore.subscribe((s, prev) => {
    const a = automationConfig(s.prefs);
    const b = automationConfig(prev.prefs);
    if (a.enabled !== b.enabled || a.port !== b.port || a.allow_start !== b.allow_start) void applyAutomation(a);
  });
  return () => {
    stopped = true;
    unsubscribe();
    offs.forEach((off) => off());
  };
}
