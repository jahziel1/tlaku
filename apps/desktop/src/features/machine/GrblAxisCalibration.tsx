import { useState } from 'react';
import { t } from '@/i18n';
import { fmt } from '@/lib/platform';
import { calibrateGrblSteps, GRBL_AXES } from './grblCalibrate';
import type { RawSetting } from './MachineSettingsLogic';

/**
 * Calibrar ejes de GRBL (M25): de la distancia pedida y la medida con una regla propone el valor
 * nuevo de `$100`, `$101` o `$102`. Solo propone: el valor se carga en la edición de siempre, con su
 * confirmación de la línea exacta, respaldo automático y verificación.
 */
export function GrblAxisCalibration({ settings, disabled, onUse }: { settings: readonly RawSetting[]; disabled: boolean; onUse: (id: number, text: string) => void }) {
  const axes = GRBL_AXES.filter((a) => settings.some((s) => s.id === a.id));
  const [id, setId] = useState<number>(axes[0]?.id ?? 100);
  const [commanded, setCommanded] = useState('100');
  const [measured, setMeasured] = useState('');
  const [shown, setShown] = useState<{ ok: true; proposed: number; current: number; ratio: number } | { ok: false; text: string } | null>(null);
  if (axes.length === 0) return null;
  const num = (s: string) => Number(s.trim().replace(',', '.'));

  const compute = () => {
    const current = Number(settings.find((s) => s.id === id)?.raw);
    const r = calibrateGrblSteps(current, num(commanded), num(measured));
    if (r.ok) setShown(r);
    else setShown({ ok: false, text: t(`mset.cal.err.${r.reason}`, { pct: r.ratio === undefined ? '' : fmt((r.ratio - 1) * 100, 1) }) });
  };

  return (
    <details className="mset-cal">
      <summary>{t('mset.cal.title')}</summary>
      <p className="hint">{t('mset.cal.intro')}</p>
      <div className="mset-edit">
        <label>
          {t('mset.cal.axis')}{' '}
          <select className="select" value={id} onChange={(e) => { setId(Number(e.target.value)); setShown(null); }}>
            {axes.map((a) => <option key={a.id} value={a.id}>{a.axis} (${a.id})</option>)}
          </select>
        </label>
        <label>
          {t('mset.cal.commanded')}{' '}
          <input className="input num" inputMode="decimal" value={commanded} onChange={(e) => { setCommanded(e.target.value); setShown(null); }} />
        </label>
        <label>
          {t('mset.cal.measured')}{' '}
          <input className="input num" inputMode="decimal" value={measured} onChange={(e) => { setMeasured(e.target.value); setShown(null); }} />
        </label>
        <button type="button" className="btn" disabled={measured.trim() === ''} onClick={compute}>{t('mset.cal.compute')}</button>
      </div>
      {shown && !shown.ok && <p className="hint hint--danger" role="alert">{shown.text}</p>}
      {shown?.ok && (
        <>
          <p role="status">{t('mset.cal.result', { id, current: shown.current, proposed: shown.proposed, pct: fmt((shown.ratio - 1) * 100, 2) })}</p>
          <button type="button" className="btn" disabled={disabled} onClick={() => onUse(id, String(shown.proposed))}>{t('mset.cal.use')}</button>
        </>
      )}
    </details>
  );
}
