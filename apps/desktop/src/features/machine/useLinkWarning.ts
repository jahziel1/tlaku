import { useEffect, useState } from 'react';
import { getApi } from '@/api';
import { resolveJobArgs } from '@/features/variables/jobResolve';
import { selectActiveDevice, useStore } from '@/store';
import { buildJobArgs } from '@/store/selectors';

/** Pide al motor el aviso de enlace saturado del trabajo actual (con los textos variables ya resueltos). */
export async function fetchLinkWarning(): Promise<string | null> {
  const args = await resolveJobArgs(buildJobArgs(useStore.getState()));
  return getApi().jobLinkWarning(args);
}

/**
 * Aviso (que no bloquea) de que el enlace serie del perfil no da abasto para el trabajo: lo calcula el
 * motor con el G-code real y los baudios del perfil (`jobLinkWarning`). `enabled` es false mientras la
 * vista previa no está lista; si el cálculo falla no se muestra nada (el diálogo ya enseña el error).
 */
export function useLinkWarning(enabled: boolean): string | null {
  const doc = useStore((s) => s.doc);
  const options = useStore((s) => s.jobOptions);
  const device = useStore(selectActiveDevice);
  const [warning, setWarning] = useState<string | null>(null);
  useEffect(() => {
    if (!enabled) {
      setWarning(null);
      return;
    }
    let live = true;
    void (async () => {
      try {
        const w = await fetchLinkWarning();
        if (live) setWarning(w);
      } catch {
        if (live) setWarning(null);
      }
    })();
    return () => {
      live = false;
    };
  }, [enabled, doc, options, device]);
  return warning;
}
