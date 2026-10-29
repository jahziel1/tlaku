import { useEffect, useMemo, useState } from 'react';
import { useRepeat } from '@/features/feeder/repeat';
import { useJobPlacement } from '@/features/machine/hooks';
import { fetchLinkWarning } from '@/features/machine/useLinkWarning';
import { useMaterials } from '@/features/materials/state';
import { useReview } from '@/features/review/useReview';
import { selectActiveDevice, usePref, useStore } from '@/store';
import { selectJobOptions } from '@/store/selectors';
import { fullReason, makeSnapshot, type FullReason, type StartChecks } from './model';
import { verifiedBaseline } from './session';

export type StartMode = 'full' | 'checking' | 'compact';

interface LinkState {
  settled: boolean;
  warning: string | null;
  failed: boolean;
}
const LINK_PENDING: LinkState = { settled: false, warning: null, failed: false };

/**
 * ¿El diálogo de inicio es el completo, el paso compacto del modo producción o todavía se está
 * comprobando? `fromUser` = lo abrió una persona con Iniciar; la cola guiada, la automatización y el
 * modo sencillo piden el inicio sin eso y siempre ven el diálogo completo.
 *
 * Mientras se espera la vista previa, el aviso de enlace y la revisión, el modo es `checking`; si
 * cualquier cosa falla o da un aviso, pasa a `full`. Nunca se llega a `compact` por omisión.
 */
export function useProductionStart(fromUser: boolean): { mode: StartMode; reason: FullReason | null } {
  const enabled = usePref('productionMode');
  const connection = useStore((s) => s.connection);
  const doc = useStore((s) => s.doc);
  const jobOptions = useStore((s) => s.jobOptions);
  const cutSelectionOnly = useStore((s) => s.cutSelectionOnly);
  const selectedIds = useStore((s) => s.selectedIds);
  const device = useStore(selectActiveDevice);
  const library = useMaterials((s) => s.library);
  const factory = useMaterials((s) => s.factory);
  const positionDoubtful = useStore((s) => s.positionLost || s.status?.position_unconfirmed === true);
  const previewStatus = useStore((s) => s.previewStatus);
  const preview = useStore((s) => s.preview);
  const repeating = useRepeat((s) => s.config.enabled);
  const placement = useJobPlacement();

  const now = useMemo(
    () =>
      makeSnapshot({
        connection,
        deviceId: device.id,
        doc,
        options: selectJobOptions({ jobOptions, cutSelectionOnly, selectedIds }),
        presets: [...library.presets, ...factory.presets],
      }),
    [connection, device.id, doc, jobOptions, cutSelectionOnly, selectedIds, library.presets, factory.presets],
  );
  const baseline = verifiedBaseline();
  const base = { enabled, fromUser, deviceKind: device.kind, connected: connection.connected, baseline, now, positionDoubtful, repeating };

  // Lo barato primero: si ya hay una causa, no se pide nada al motor. `pending` = solo faltan las comprobaciones.
  const cheap = fullReason({ ...base, checks: null });
  const wantChecks = cheap === 'pending';
  const ready = previewStatus === 'ready' && preview !== null;

  const review = useReview(wantChecks);
  const [link, setLink] = useState<LinkState>(LINK_PENDING);
  useEffect(() => {
    if (!wantChecks || !ready) {
      setLink(LINK_PENDING);
      return;
    }
    let live = true;
    setLink(LINK_PENDING);
    fetchLinkWarning().then(
      (warning) => live && setLink({ settled: true, warning, failed: false }),
      () => live && setLink({ settled: true, warning: null, failed: true }),
    );
    return () => {
      live = false;
    };
  }, [wantChecks, ready, doc, jobOptions, cutSelectionOnly, selectedIds, device]);

  let checks: StartChecks | null = null;
  if (previewStatus === 'error') {
    checks = { previewWarnings: [], linkWarning: null, seriousFindings: 0, placementOk: false, failed: true };
  } else if (ready && link.settled && review.status !== 'running') {
    checks = {
      previewWarnings: preview.warnings,
      linkWarning: link.warning,
      seriousFindings: review.findings.filter((f) => f.severity === 'serious').length,
      placementOk: placement.status === 'ok' && placement.fits,
      failed: link.failed || review.status === 'error',
    };
  }

  if (!wantChecks) return { mode: 'full', reason: cheap };
  const reason = fullReason({ ...base, checks });
  if (reason === null) return { mode: 'compact', reason: null };
  return { mode: reason === 'pending' ? 'checking' : 'full', reason };
}
