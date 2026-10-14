import type { DeviceProfile } from '@/api/types';
import type { CylinderAxis } from '@/bindings/CylinderAxis';
import { t } from '@/i18n';
import { fmt } from '@/lib/platform';
import { NumInput } from './DeviceFields';
import { Check, GField, Sub } from './GalvoFieldParts';
import type { GalvoIssues } from './galvoProfile';

const AXES: CylinderAxis[] = ['x', 'y'];

/** Mm de superficie de una vuelta de una pieza de ese diámetro (π × diámetro). */
export const turnMm = (diameterMm: number): number => Math.PI * diameterMm;

/** Editor propio del rotativo del galvo (M30): diámetro, pasos por vuelta, eje y sentido, con lo que significan. */
export function GalvoRotaryFields({ draft, edit, issues }: { draft: DeviceProfile; edit: (patch: Partial<DeviceProfile>) => void; issues: GalvoIssues }) {
  const g = draft.galvo;
  const r = g.rotary;
  const setRot = (patch: Partial<typeof r>) => edit({ galvo: { ...g, rotary: { ...r, ...patch } } });
  const legacy = draft.rotary.enabled && !r.enabled;
  const turn = turnMm(r.diameter_mm);
  const side = g.split.piece_mm > 0 ? Math.min(g.split.piece_mm, g.field_mm) : g.field_mm;
  const span = turn > 0 ? (side / turn) * 360 : 0;
  return (
    <Sub title={t('galvo.dev.rotary')} help="galvo.dev.rotary.help">
      {legacy && (
        <div className="warn-line" role="note">
          <span>{t('galvo.dev.rotary.legacy', { mm: fmt(draft.rotary.object_diameter, 1) })}</span>
          <button
            type="button"
            className="btn"
            onClick={() => edit({ rotary: { ...draft.rotary, enabled: false }, galvo: { ...g, rotary: { ...r, enabled: true, diameter_mm: draft.rotary.object_diameter, reverse: draft.rotary.mirror } } })}
          >
            {t('galvo.dev.rotary.migrate')}
          </button>
        </div>
      )}
      <Check label={t('galvo.dev.rotary.enable')} checked={r.enabled} onChange={(v) => setRot({ enabled: v })} />
      {r.enabled && (
        <>
          <div className="field-grid">
            <GField label={t('galvo.dev.rotary.diameter')} help="galvo.dev.rotary.diameter.help" error={issues.rotary_diameter}>
              {({ id, describedBy, invalid }) => <NumInput id={id} unit="mm" value={r.diameter_mm} describedBy={describedBy} invalid={invalid} onCommit={(v) => setRot({ diameter_mm: v })} />}
            </GField>
            <GField label={t('galvo.dev.rotary.steps')} help="galvo.dev.rotary.steps.help" error={issues.rotary_steps}>
              {({ id, describedBy, invalid }) => <NumInput id={id} value={r.steps_per_rev} describedBy={describedBy} invalid={invalid} onCommit={(v) => setRot({ steps_per_rev: v })} />}
            </GField>
          </div>
          <div className="field-grid">
            <GField label={t('galvo.dev.rotary.axis')} help="galvo.dev.rotary.axis.help">
              {({ id, describedBy }) => (
                <select id={id} className="select" value={r.axis} aria-describedby={describedBy} onChange={(e) => setRot({ axis: e.target.value as CylinderAxis })}>
                  {AXES.map((a) => (
                    <option key={a} value={a}>{t(`galvo.rotary.axis.${a}`)}</option>
                  ))}
                </select>
              )}
            </GField>
            <GField label={t('galvo.dev.rotary.reverse')} help="galvo.dev.rotary.reverse.help">
              {() => <Check label={t('galvo.dev.rotary.reverse')} checked={r.reverse} onChange={(v) => setRot({ reverse: v })} />}
            </GField>
          </div>
          {!issues.rotary_diameter && (
            <>
              <p className="hint num">{t('galvo.dev.rotary.turn', { mm: fmt(turn, 1) })}</p>
              <p className="hint num">
                {r.steps_per_rev > 0
                  ? t('galvo.dev.rotary.piece.steps', { side: fmt(side, 1), deg: fmt(span, 1), steps: fmt(Math.round((span / 360) * r.steps_per_rev), 0) })
                  : t('galvo.dev.rotary.piece', { side: fmt(side, 1), deg: fmt(span, 1) })}
              </p>
              {span > 120 && <p className="warn-line" role="note">{t('galvo.dev.rotary.wide')}</p>}
            </>
          )}
          <p className="hint">{t('galvo.dev.rotary.noAxis')}</p>
        </>
      )}
    </Sub>
  );
}
