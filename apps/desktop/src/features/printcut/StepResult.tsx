import { CircleCheck, TriangleAlert } from 'lucide-react';
import type { Point } from '@/api/types';
import { t, type I18nKey } from '@/i18n';
import { fmt } from '@/lib/platform';
import { GOOD_RESIDUAL_MM, isGoodFit } from './flow';
import type { DesignMark } from './marks';
import { usePrintCut } from './printcutStore';
import { useMarksMoved } from './StepMarks';
import type { FitError, FitWarning } from './types';
import type { RegistrationState } from './useRegistration';

export function warningText(w: FitWarning): string {
  switch (w.kind) {
    case 'mirrored':
      return t('printcut.warn.mirrored');
    case 'scale':
      return t('printcut.warn.scale', { pct: fmt(w.pct, 2) });
    case 'anisotropy':
      return t('printcut.warn.anisotropy', { x: fmt(w.xPct, 2), y: fmt(w.yPct, 2) });
  }
}

export function errorText(e: FitError): string {
  const key: Record<FitError['kind'], I18nKey> = {
    few: 'printcut.error.few',
    nan: 'printcut.error.nan',
    close: 'printcut.error.close',
    collinear: 'printcut.error.collinear',
  };
  return t(key[e.kind], e.kind === 'few' ? { need: e.need, have: e.have } : e.kind === 'close' ? { a: e.a, b: e.b } : undefined);
}

interface ViewProps {
  marks: readonly DesignMark[];
  physical: readonly (Point | null)[];
  /** El ajuste que calculó el motor para esas marcas (ver `useRegistration`). */
  state: RegistrationState;
  /** Las marcas se movieron desde que se eligieron. */
  moved: boolean;
}

/** Error residual de cada marca y resumen de la transformación, antes de aceptarla. */
export function ResultView({ marks, physical, state, moved }: ViewProps) {
  if (state.status === 'incomplete') return <p className="warn-line" role="alert">{t('printcut.result.incomplete')}</p>;
  if (state.status === 'pending') return <p className="hint" role="status">{t('printcut.result.computing')}</p>;
  if (state.status === 'failed') return <p className="warn-line" role="alert">{state.message}</p>;
  const { result } = state;
  if (!result.ok) return <p className="warn-line" role="alert">{errorText(result.error)}</p>;
  const { fit } = result;
  const lowError = isGoodFit(fit.residualMax);
  const good = lowError && fit.warnings.length === 0;
  const shift = [fit.transform[4], fit.transform[5]];

  return (
    <>
      <p className={good ? 'pc-verdict pc-verdict--ok' : 'pc-verdict pc-verdict--bad'} role="status">
        {good ? <CircleCheck size={15} aria-hidden="true" /> : <TriangleAlert size={15} aria-hidden="true" />}
        <span>{t(good ? 'printcut.result.good' : lowError ? 'printcut.result.check' : 'printcut.result.bad', { mm: fmt(GOOD_RESIDUAL_MM, 1) })}</span>
      </p>
      <dl className="pc-stats num">
        <dt>{t('printcut.result.rms')}</dt>
        <dd>{fmt(fit.residualRms, 3)} mm</dd>
        <dt>{t('printcut.result.max')}</dt>
        <dd>{fmt(fit.residualMax, 3)} mm</dd>
        <dt>{t('printcut.result.rotation')}</dt>
        <dd>{fmt(fit.rotationDeg, 3)}°</dd>
        <dt>{t('printcut.result.scale')}</dt>
        <dd>{fmt(fit.scale * 100, 3)} %</dd>
        <dt>{t('printcut.result.shift')}</dt>
        <dd>{fmt(shift[0]!, 2)}; {fmt(shift[1]!, 2)} mm</dd>
        <dt>{t('printcut.result.fit')}</dt>
        <dd>{t(`printcut.mode.${fit.mode}` as I18nKey)}</dd>
      </dl>
      <table className="pc-table num">
        <thead>
          <tr>
            <th scope="col">{t('printcut.result.mark')}</th>
            <th scope="col">{t('printcut.result.design')}</th>
            <th scope="col">{t('printcut.result.measured')}</th>
            <th scope="col">{t('printcut.result.error')}</th>
          </tr>
        </thead>
        <tbody>
          {marks.map((m, i) => (
            <tr key={m.ids.join(',')}>
              <th scope="row">{i + 1}</th>
              <td>{fmt(m.point[0], 2)}; {fmt(m.point[1], 2)}</td>
              <td>{fmt(physical[i]![0], 2)}; {fmt(physical[i]![1], 2)}</td>
              <td>{fmt(fit.residuals[i]!, 3)}</td>
            </tr>
          ))}
        </tbody>
      </table>
      {fit.warnings.map((w) => (
        <p key={w.kind} className="warn-line" role="alert">{warningText(w)}</p>
      ))}
      {moved && <p className="warn-line" role="alert">{t('printcut.marks.moved')}</p>}
      <p className="hint">{t('printcut.result.hint')}</p>
    </>
  );
}

/** Paso 3 conectado al estado del asistente. */
export function StepResult({ state }: { state: RegistrationState }) {
  const marks = usePrintCut((s) => s.marks);
  const physical = usePrintCut((s) => s.physical);
  const moved = useMarksMoved();
  return <ResultView marks={marks} physical={physical} state={state} moved={moved} />;
}
