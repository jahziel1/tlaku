import { useEffect } from 'react';
import { create } from 'zustand';
import { getApiKind } from '@/api';
import { t } from '@/i18n';
import { useStore } from '@/store';
import { importPaths, unsavedIo } from './files';
import { baseName } from './paths';
import { guardUnsaved } from './unsaved';

// ---- título de la ventana ----------------------------------------------------

/** Título de la ventana: marca de cambios sin guardar, nombre del proyecto y de la app. */
export function documentTitle(path: string | null, dirty: boolean): string {
  const name = path ? baseName(path) : t('doc.untitled');
  return `${dirty ? '● ' : ''}${name} — Tlaku`;
}

// ---- permisos de ventana ----------------------------------------------------------

/**
 * Permisos declarados en `src-tauri/capabilities/*.json`, leídos al compilar. Sirven para activar
 * solo lo que la ventana puede hacer: registrar `onCloseRequested` sin `core:window:allow-destroy`
 * dejaría la ventana sin poder cerrarse.
 */
const CAPABILITY_FILES = import.meta.glob('/src-tauri/capabilities/*.json', { eager: true, import: 'default' });

export function declaredPermissions(files: Record<string, unknown> = CAPABILITY_FILES): Set<string> {
  const out = new Set<string>();
  for (const file of Object.values(files)) {
    const perms = (file as { permissions?: unknown })?.permissions;
    if (!Array.isArray(perms)) continue;
    for (const p of perms) {
      if (typeof p === 'string') out.add(p);
      else if (p && typeof (p as { identifier?: unknown }).identifier === 'string') out.add((p as { identifier: string }).identifier);
    }
  }
  return out;
}

export const PERMISSION_CLOSE = 'core:window:allow-destroy';
export const PERMISSION_TITLE = 'core:window:allow-set-title';

export function canGuardClose(perms: Set<string> = declaredPermissions()): boolean {
  return perms.has(PERMISSION_CLOSE);
}

export function canSetNativeTitle(perms: Set<string> = declaredPermissions()): boolean {
  return perms.has(PERMISSION_TITLE);
}

/** Mantiene el título del documento web y, si hay permiso, el de la ventana nativa. */
export function useWindowTitle(): void {
  const path = useStore((s) => s.filePath);
  const dirty = useStore((s) => s.dirty);
  useEffect(() => {
    const title = documentTitle(path, dirty);
    document.title = title;
    if (getApiKind() === 'tauri' && canSetNativeTitle()) {
      void import('@tauri-apps/api/window')
        .then(({ getCurrentWindow }) => getCurrentWindow().setTitle(title))
        .catch((e) => console.warn('No se pudo poner el título de la ventana', e));
    }
  }, [path, dirty]);
}

// ---- cerrar la ventana con cambios --------------------------------------------------------

/**
 * Pregunta guardar, descartar o cancelar al cerrar la ventana con cambios. Solo se activa en Tauri
 * y si la capacidad `core:window:allow-destroy` está declarada (ver `canGuardClose`).
 */
export function useCloseGuard(): void {
  useEffect(() => {
    if (getApiKind() !== 'tauri' || !canGuardClose()) return;
    let off: (() => void) | undefined;
    let cancelled = false;
    void import('@tauri-apps/api/window').then(async ({ getCurrentWindow }) => {
      const win = getCurrentWindow();
      const unlisten = await win.onCloseRequested(async (event) => {
        if (!useStore.getState().dirty) return; // se cierra sin preguntar
        event.preventDefault();
        if (await guardUnsaved('close', unsavedIo)) await win.destroy();
      });
      if (cancelled) unlisten();
      else off = unlisten;
    });
    return () => {
      cancelled = true;
      off?.();
    };
  }, []);
}

// ---- arrastrar y soltar archivos ---------------------------------------------------------------

/** `true` mientras se arrastran archivos sobre la ventana (muestra el aviso "Suelta para importar"). */
export const useDropOver = create<{ over: boolean }>()(() => ({ over: false }));
const setOver = (over: boolean) => useDropOver.setState({ over });

/**
 * Importa lo que se suelte sobre la ventana. En Tauri usa el evento nativo (da rutas reales); en
 * el navegador (modo mock) usa el arrastre HTML y toma el nombre del archivo como ruta.
 */
export function useFileDrop(): void {
  useEffect(() => {
    let off: (() => void) | undefined;
    let cancelled = false;

    if (getApiKind() === 'tauri') {
      void import('@tauri-apps/api/webview').then(async ({ getCurrentWebview }) => {
        const unlisten = await getCurrentWebview().onDragDropEvent((e) => {
          const p = e.payload;
          if (p.type === 'enter' || p.type === 'over') setOver(true);
          else if (p.type === 'drop') {
            setOver(false);
            void importPaths(p.paths);
          } else setOver(false);
        });
        if (cancelled) unlisten();
        else off = unlisten;
      });
    } else {
      const hasFiles = (e: DragEvent) => !!e.dataTransfer && [...e.dataTransfer.types].includes('Files');
      const onOver = (e: DragEvent) => {
        if (!hasFiles(e)) return;
        e.preventDefault();
        setOver(true);
      };
      const onLeave = (e: DragEvent) => {
        // relatedTarget nulo = el arrastre salió de la ventana
        if (e.relatedTarget === null) setOver(false);
      };
      const onDrop = (e: DragEvent) => {
        if (!hasFiles(e)) return;
        e.preventDefault();
        setOver(false);
        const files = [...(e.dataTransfer?.files ?? [])];
        void importPaths(files.map((f) => (f as File & { path?: string }).path ?? f.name));
      };
      window.addEventListener('dragenter', onOver);
      window.addEventListener('dragover', onOver);
      window.addEventListener('dragleave', onLeave);
      window.addEventListener('drop', onDrop);
      off = () => {
        window.removeEventListener('dragenter', onOver);
        window.removeEventListener('dragover', onOver);
        window.removeEventListener('dragleave', onLeave);
        window.removeEventListener('drop', onDrop);
      };
    }
    return () => {
      cancelled = true;
      off?.();
      setOver(false);
    };
  }, []);
}
