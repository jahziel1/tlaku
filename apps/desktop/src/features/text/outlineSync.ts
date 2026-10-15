import { useEffect } from 'react';
import type { StoreApi } from 'zustand';
import { getApi, type LaserApi } from '@/api';
import type { DesignObject, Document, FontFamily } from '@/api/types';
import { useVariablesUi } from '@/features/variables/context';
import { issueMessage } from '@/features/variables/messages';
import { t } from '@/i18n';
import type { BezPath } from '@/lib/bezpath';
import { useStore, type AppState } from '@/store';
import { effectiveFont, loadFonts } from './fonts';
import { isOutlineCurrent, rememberOutline } from './outlineSigs';
import { isTextObject, resolveText, type TextDesignObject } from './signature';
import { useTextStatus } from './status';

/**
 * Pone `outline` en los textos `id` de `doc` cuya firma es `sig`. Devuelve el mismo documento si
 * no hay nada que cambiar. `memo` reutiliza el objeto nuevo entre documentos que comparten el
 * mismo objeto (los pasos del historial lo comparten), para no multiplicar memoria.
 */
export function patchDocOutline(
  doc: Document,
  id: string,
  sig: string,
  outline: BezPath[],
  memo: Map<DesignObject, DesignObject>,
): Document {
  let changed = false;
  const objects = doc.objects.map((o) => {
    if (o.id !== id || !isTextObject(o)) return o;
    if (o.kind.outline === outline || (o.kind.outline.length === 0 && outline.length === 0)) return o;
    const r = resolveText(o, doc);
    if (!r.ok || r.sig !== sig) return o;
    changed = true;
    let next = memo.get(o);
    if (!next) {
      next = { ...o, kind: { ...o.kind, outline } };
      memo.set(o, next);
    }
    return next;
  });
  return changed ? { ...doc, objects } : doc;
}

/** Espera tras el último cambio antes de pedir el contorno (escribir no debe saturar al motor). */
export const OUTLINE_DEBOUNCE_MS = 250;

export { isOutlineCurrent, outlineMatches, rememberOutline } from './outlineSigs';

export interface OutlineSyncOptions {
  /** Store del que se lee el documento (la app usa `useStore`). */
  store?: Pick<StoreApi<AppState>, 'getState' | 'subscribe'>;
  api?: () => Pick<LaserApi, 'textOutline'>;
  fonts?: () => Promise<FontFamily[]>;
  debounceMs?: number;
}

export interface OutlineSync {
  /** Revisa el documento actual (se llama sola al cambiar). */
  scan: () => void;
  stop: () => void;
}

interface Job {
  sig: string;
  timer: ReturnType<typeof setTimeout> | undefined;
}

/**
 * Mantiene al día el `outline` de cada `TextObject` del documento: cuando está vacío o cambió
 * algo que lo determina (texto, fuente, tamaño, espaciado, trayecto guía...), pide el contorno
 * al motor con debounce y lo guarda sin ensuciar el historial (`patchSilently` del store).
 */
export function startOutlineSync(options: OutlineSyncOptions = {}): OutlineSync {
  const store = options.store ?? useStore;
  const api = options.api ?? getApi;
  const fonts = options.fonts ?? loadFonts;
  const debounceMs = options.debounceMs ?? OUTLINE_DEBOUNCE_MS;
  const status = useTextStatus.getState();

  const jobs = new Map<string, Job>();
  /** Última firma resuelta por id (también las de contorno vacío) y las que fallaron. */
  const done = new Map<string, string>();
  const failed = new Map<string, string>();
  let stopped = false;

  const cancel = (id: string) => {
    const job = jobs.get(id);
    if (job?.timer) clearTimeout(job.timer);
    jobs.delete(id);
  };

  const find = (doc: Document, id: string): TextDesignObject | undefined => {
    const o = doc.objects.find((x) => x.id === id);
    return o && isTextObject(o) ? o : undefined;
  };

  const run = async (id: string, sig: string) => {
    const job = jobs.get(id);
    if (stopped || !job || job.sig !== sig) return;
    job.timer = undefined;
    const obj = find(store.getState().doc, id);
    const resolved = obj && resolveText(obj, store.getState().doc);
    if (!obj || !resolved?.ok || resolved.sig !== sig) return;

    let families: FontFamily[] = [];
    try {
      families = await fonts();
    } catch {
      // Sin lista de fuentes se respeta la familia pedida y el motor dirá si no existe.
    }
    const font = effectiveFont(families, obj.kind.font_family);
    let result;
    try {
      result = await api().textOutline({
        text: { ...obj.kind, text: resolved.text ?? obj.kind.text, font_family: font.family, outline: [] },
        path: resolved.path,
      });
    } catch (e) {
      if (stopped || jobs.get(id) !== job) return;
      jobs.delete(id);
      failed.set(id, sig);
      status.set(id, { busy: false, warnings: [], error: String(e instanceof Error ? e.message : e) });
      return;
    }
    // Si mientras tanto cambió algo, esta respuesta ya no sirve: hay otro trabajo en camino.
    if (stopped || jobs.get(id) !== job) return;
    const now = find(store.getState().doc, id);
    const again = now && resolveText(now, store.getState().doc);
    if (!now || !again?.ok || again.sig !== sig) {
      jobs.delete(id);
      scan();
      return;
    }
    jobs.delete(id);
    done.set(id, sig);
    rememberOutline(result.paths, sig);
    const warnings = [
      ...(resolved.issues ?? []).map(issueMessage),
      ...(font.fellBack ? [t('text.warn.fontFallback', { family: obj.kind.font_family.trim(), used: font.family })] : []),
      ...result.warnings,
    ];
    status.set(id, { busy: false, warnings, error: null });
    // Sin paso de deshacer: deshacer un contorno dejaría un texto sin contorno que se regeneraría y
    // borraría el rehacer. El mismo contorno se escribe en el documento, en lo guardado y en el historial.
    const memo = new Map<DesignObject, DesignObject>();
    store.getState().patchSilently((d) => patchDocOutline(d, id, sig, result.paths, memo));
  };

  const schedule = (id: string, sig: string) => {
    cancel(id);
    const job: Job = { sig, timer: undefined };
    job.timer = setTimeout(() => void run(id, sig), debounceMs);
    jobs.set(id, job);
    status.set(id, { busy: true, error: null });
  };

  function scan() {
    if (stopped) return;
    const doc = store.getState().doc;
    const seen = new Set<string>();
    for (const o of doc.objects) {
      if (!isTextObject(o)) continue;
      seen.add(o.id);
      const r = resolveText(o, doc);
      if (!r.ok) {
        cancel(o.id);
        status.set(o.id, { busy: false, warnings: [], error: t('text.onPath.missing') });
        continue;
      }
      if (isOutlineCurrent(o.kind.outline, r.sig, done.get(o.id), r.text === undefined)) {
        if (jobs.has(o.id)) {
          cancel(o.id);
          status.set(o.id, { busy: false });
        }
        continue;
      }
      if (failed.get(o.id) === r.sig || jobs.get(o.id)?.sig === r.sig) continue;
      failed.delete(o.id);
      schedule(o.id, r.sig);
    }
    for (const id of [...jobs.keys(), ...done.keys(), ...failed.keys(), ...Object.keys(useTextStatus.getState().byId)]) {
      if (seen.has(id)) continue;
      cancel(id);
      done.delete(id);
      failed.delete(id);
      status.clear(id);
    }
  }

  let prevDoc = store.getState().doc;
  const unsubscribe = store.subscribe((s) => {
    if (s.doc === prevDoc) return;
    prevDoc = s.doc;
    scan();
  });
  // Los valores del texto variable también cambian con el CSV cargado, el reloj y "ver plantilla".
  let prevUi = useVariablesUi.getState();
  const unsubscribeUi = useVariablesUi.subscribe((ui) => {
    const changed = ui.csv !== prevUi.csv || ui.extra !== prevUi.extra || ui.showTemplate !== prevUi.showTemplate || ui.clock !== prevUi.clock;
    prevUi = ui;
    if (changed) scan();
  });
  scan();

  return {
    scan,
    stop: () => {
      stopped = true;
      unsubscribe();
      unsubscribeUi();
      for (const id of [...jobs.keys()]) cancel(id);
    },
  };
}

/** Monta la sincronización de contornos mientras la interfaz esté abierta. */
export function useOutlineSync(): void {
  useEffect(() => {
    const sync = startOutlineSync();
    return sync.stop;
  }, []);
}
