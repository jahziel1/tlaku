import { useEffect, useState } from 'react';
import { getApi } from '@/api';
import type { DeviceProfile, RuidaFile } from '@/api/types';
import { errorMessage } from '@/lib/errors';
import { getPref } from '@/store';

/**
 * Nombre del documento al guardar en la memoria de la controladora Ruida (M23). Vacío = nombre
 * automático (el motor elige uno libre). Con nombre, 1 a 8 letras, cifras, `_` o `-` (lo mismo que
 * exige el motor). Si ya hay un documento así, hay que confirmar que se reemplaza.
 */
export const DOC_NAME_RE = /^[A-Za-z0-9_-]{1,8}$/;

export function isValidDocName(name: string): boolean {
  return DOC_NAME_RE.test(name);
}

/** Número del documento que ya se llama así (sin distinguir mayúsculas), o `null`. */
export function existingDoc(files: readonly RuidaFile[], name: string): RuidaFile | null {
  const n = name.trim().toLowerCase();
  return n === '' ? null : (files.find((f) => f.name?.toLowerCase() === n) ?? null);
}

export interface RuidaStartGate {
  /** ¿Es una máquina Ruida que guarda el trabajo como documento (no lo ejecuta)? */
  applies: boolean;
  name: string;
  setName: (v: string) => void;
  overwrite: boolean;
  setOverwrite: (v: boolean) => void;
  /** Documento con ese nombre ya en la memoria. */
  clash: RuidaFile | null;
  nameValid: boolean;
  checking: boolean;
  checkError: string | null;
  /** ¿Se puede iniciar? Falla con un nombre inválido o repetido sin confirmar el reemplazo. */
  ok: boolean;
  /**
   * Le dice al motor, justo antes de iniciar, el nombre y si puede reemplazar. Se llama desde el
   * botón de confirmar (no desde un efecto): así un diálogo cancelado nunca deja un permiso de
   * reemplazar preparado. Devuelve `false` (y deja el error en `checkError`) si el motor lo rechaza:
   * entonces no se inicia.
   */
  commit: () => Promise<boolean>;
}

/**
 * Estado del nombre en el diálogo de inicio. Para otras máquinas no hace nada. La lista de
 * documentos se lee al abrir (solo lectura) para avisar de un nombre repetido; el motor lo vuelve a
 * comprobar al guardar y no borra nada sin el permiso.
 */
export function useRuidaStartGate(device: Pick<DeviceProfile, 'kind' | 'ruida'>, connected: boolean): RuidaStartGate {
  const applies = device.kind === 'ruida' && !device.ruida.run_after_upload;
  const [name, setName] = useState('');
  const [overwrite, setOverwrite] = useState(false);
  const [files, setFiles] = useState<RuidaFile[] | null>(null);
  const [checkError, setCheckError] = useState<string | null>(null);

  useEffect(() => {
    if (!applies || !connected) return;
    let live = true;
    getApi()
      .ruidaFiles()
      .then((f) => {
        if (!live) return;
        setFiles(f);
        setCheckError(null);
      })
      .catch((e) => live && setCheckError(errorMessage(e)));
    return () => {
      live = false;
    };
  }, [applies, connected]);

  const trimmed = name.trim();
  const nameValid = trimmed === '' || isValidDocName(trimmed);
  const clash = files && nameValid ? existingDoc(files, trimmed) : null;
  const ok = !applies || (nameValid && (clash === null || overwrite));

  return {
    applies,
    name,
    setName: (v) => {
      setName(v);
      setOverwrite(false);
    },
    overwrite,
    setOverwrite,
    clash,
    nameValid,
    checking: applies && connected && files === null && checkError === null,
    checkError,
    ok,
    commit: async () => {
      if (!applies || !connected) return true;
      try {
        await getApi().ruidaPrepareStore({ name: trimmed === '' ? null : trimmed, overwrite: clash !== null && overwrite, experimental_writes: getPref('ruidaExperimentalWrites') });
        return true;
      } catch (e) {
        setCheckError(errorMessage(e));
        return false;
      }
    },
  };
}
