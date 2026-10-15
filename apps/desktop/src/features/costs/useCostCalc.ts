import { useMemo } from 'react';
import { useShallow } from 'zustand/react/shallow';
import { useJobLog } from '@/features/help/jobLog';
import { getLanguage } from '@/i18n';
import { selectActiveDevice, useStore } from '@/store';
import { baseName } from '@/features/history/paths';
import { calibrationFactor, CALIBRATION_MIN_JOBS, jobMetrics, resolveTime, toFigures, type JobMetrics } from './costMetrics';
import { computeCost, type CostInputs, type CostResult, type TimeSource } from './costModel';
import { machineFor } from './costProfiles';
import type { ReportContext } from './costReport';
import { useCostStore } from './costStore';
import { currencyDigits, formatTag } from './currency';

export interface CostCalc {
  metrics: JobMetrics;
  inputs: CostInputs;
  result: CostResult;
  report: ReportContext;
  device: { id: string; name: string };
  digits: number;
  tag: string;
  /** Fuente de tiempo pedida y la que se usó de verdad. */
  timeUsed: TimeSource;
  /** Factor real ÷ estimado de la máquina y trabajos con que se calculó (o `null`). */
  calibration: { factor: number; jobs: number } | null;
  /** Hay un tiempo real del último trabajo en el registro. */
  hasLastReal: boolean;
}

/** Une el diseño, la vista previa, el registro de trabajos y los datos guardados en el resultado de la calculadora. */
export function useCostCalc(): CostCalc {
  const doc = useStore((s) => s.doc);
  const preview = useStore((s) => s.preview);
  const selection = useStore(useShallow((s) => (s.cutSelectionOnly && s.selectedIds.length > 0 ? [...s.selectedIds] : null)));
  const filePath = useStore((s) => s.filePath);
  const device = useStore(selectActiveDevice);
  const stored = useCostStore();
  const log = useJobLog((s) => s.entries);

  const metrics = useMemo(() => jobMetrics(doc, preview, selection), [doc, preview, selection]);
  return useMemo(() => {
    const language = getLanguage();
    const tag = formatTag(language, typeof navigator === 'undefined' ? null : navigator.language);
    const digits = currencyDigits(stored.sales.currency);
    const time = resolveTime(stored.timeSource, metrics, log, device.name);
    const inputs: CostInputs = {
      job: toFigures(metrics, time),
      machine: machineFor(stored, device.id),
      material: stored.material,
      materialMode: stored.materialMode,
      sales: stored.sales,
      piecesInDesign: stored.piecesInDesign,
      quantity: stored.quantity,
      timeUsed: time.used,
      approximateTime: metrics.approximate,
    };
    const result = computeCost(inputs, digits);
    const calibration = calibrationFactor(log, device.name);
    return {
      metrics,
      inputs,
      result,
      device: { id: device.id, name: device.name },
      digits,
      tag,
      timeUsed: time.used,
      calibration,
      hasLastReal: log.some((e) => e.machine === device.name && e.result === 'finished' && e.actualS > 0),
      report: {
        inputs,
        result,
        project: filePath ? baseName(filePath) : null,
        machine: device.name,
        timeUsed: time.used,
        cutMm: metrics.cutMm,
        digits,
        tag,
      },
    };
  }, [metrics, log, device, stored, filePath]);
}

export { CALIBRATION_MIN_JOBS };
