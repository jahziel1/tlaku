import type { DeviceProfile } from '@/api/types';
import { t } from '@/i18n';
import { Field, NumInput } from './DeviceFields';
import { feedLines, validateFeeder, type FeederIssue } from './DeviceFeederModel';

interface Props {
  draft: DeviceProfile;
  edit: (patch: Partial<DeviceProfile>) => void;
}

const issueText = (issue: FeederIssue | undefined) => (issue ? t(`tiling.feeder.err.${issue}`) : null);

/** Sección «Alimentador automático» del perfil: cinta o rodillos que avanzan el material entre tramos. */
export function DeviceFeederFields({ draft, edit }: Props) {
  const feeder = draft.feeder;
  const patch = (p: Partial<typeof feeder>) => edit({ feeder: { ...feeder, ...p } });
  const issues = validateFeeder(feeder);
  const gcodeError = issueText(issues.feed_gcode);
  const mmError = issueText(issues.feed_mm);
  return (
    <div className="til-feeder">
      <Field label={t('tiling.feeder.enabled')} help="tiling.feeder.enabled.help">
        {({ id, describedBy }) => (
          <label className="mc-check">
            <input id={id} type="checkbox" checked={feeder.enabled} aria-describedby={describedBy} onChange={(e) => patch({ enabled: e.target.checked })} />
            <span>{t('tiling.feeder.enabled.label')}</span>
          </label>
        )}
      </Field>
      {feeder.enabled && (
        <>
          <Field label={t('tiling.feeder.gcode')} help="tiling.feeder.gcode.help">
            {({ id, describedBy }) => (
              <>
                <textarea
                  id={id}
                  className="input num mc-textarea"
                  rows={3}
                  spellCheck={false}
                  aria-describedby={describedBy}
                  aria-invalid={gcodeError ? true : undefined}
                  value={feeder.feed_gcode}
                  onChange={(e) => patch({ feed_gcode: e.target.value })}
                />
                {gcodeError && <p className="hint hint--danger" role="alert">{gcodeError}</p>}
              </>
            )}
          </Field>
          <Field label={t('tiling.feeder.mm')} help="tiling.feeder.mm.help">
            {({ id, describedBy }) => (
              <>
                <NumInput id={id} unit="mm" value={feeder.feed_mm} describedBy={describedBy} invalid={mmError !== null} onCommit={(v) => patch({ feed_mm: v })} />
                {mmError && <p className="hint hint--danger" role="alert">{mmError}</p>}
              </>
            )}
          </Field>
          <p className="hint">{t('tiling.feeder.preview', { gcode: feedLines(feeder, feeder.feed_mm).join(' / ') || '—' })}</p>
        </>
      )}
    </div>
  );
}
