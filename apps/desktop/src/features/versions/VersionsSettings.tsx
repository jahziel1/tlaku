import { useEffect, useId, useState } from 'react';
import { getApi } from '@/api';
import { t } from '@/i18n';
import { errorMessage } from '@/lib/errors';
import { askChoice } from '@/features/history/prompt';
import { baseName } from '@/features/history/paths';
import { NumberField } from '@/ui/NumberField';
import { useStore } from '@/store';
import type { VersionsConfig, VersionsLocation } from './types';
import { forgetThumbs } from './useVersions';
import { useVersionsUi } from './versionsUi';

interface Props {
  /** Proyecto abierto en el diálogo (para «Borrar todo el historial»), si lo hay. */
  path: string | null;
  count: number;
  onCleared: () => void;
}

/** Ajustes del historial: dónde se guarda, cada cuánto y qué se conserva. Se aplican con el botón «Aplicar». */
export function VersionsSettings({ path, count, onCleared }: Props) {
  const saved = useVersionsUi((s) => s.config);
  const setConfig = useVersionsUi((s) => s.setConfig);
  const [draft, setDraft] = useState<VersionsConfig>(saved);
  const [busy, setBusy] = useState(false);
  const locId = useId();
  const enabledId = useId();

  useEffect(() => setDraft(saved), [saved]);

  const patch = (change: Partial<VersionsConfig>) => setDraft((d) => ({ ...d, ...change }));
  const dirty = JSON.stringify(draft) !== JSON.stringify(saved);

  const apply = async () => {
    setBusy(true);
    try {
      const kept = await getApi().versionsConfigSet({ config: draft });
      setConfig(kept);
      useStore.getState().notifyKey('success', 'versions.settings.applied');
    } catch (e) {
      useStore.getState().notify('error', t('versions.settings.failed', { error: errorMessage(e) }));
    } finally {
      setBusy(false);
    }
  };

  const clearAll = async () => {
    if (!path) return;
    const choice = await askChoice({
      title: t('versions.clear.title'),
      message: t('versions.clear.message', { n: count, name: baseName(path) }),
      choices: [
        { id: 'clear', label: t('versions.clear.confirm'), tone: 'danger' },
        { id: 'keep', label: t('versions.delete.cancel') },
      ],
      cancelId: 'keep',
    });
    if (choice !== 'clear') return;
    try {
      await getApi().versionsClear({ path });
      forgetThumbs(path);
      onCleared();
    } catch (e) {
      useStore.getState().notify('error', t('versions.notice.deleteFailed', { error: errorMessage(e) }));
    }
  };

  const off = !draft.enabled;
  return (
    <details className="ver-settings">
      <summary>{t('versions.settings.title')}</summary>
      <div className="ver-settings__body">
        <label className="ver-check" htmlFor={enabledId}>
          <input id={enabledId} type="checkbox" checked={draft.enabled} onChange={(e) => patch({ enabled: e.target.checked })} />
          <span>{t('versions.settings.enabled')}</span>
        </label>

        <div className="field">
          <label htmlFor={locId} className="field__label">
            {t('versions.settings.location')}
          </label>
          <select
            id={locId}
            className="select"
            value={draft.location}
            onChange={(e) => patch({ location: e.target.value as VersionsLocation })}
          >
            <option value="app">{t('versions.settings.location.app')}</option>
            <option value="beside">{t('versions.settings.location.beside')}</option>
          </select>
          <p className="hint">{t('versions.settings.locationHint')}</p>
        </div>

        <div className="field-grid">
          <NumberField label={t('versions.settings.autoMinutes')} value={draft.auto_minutes} min={0} max={240} disabled={off} onCommit={(v) => patch({ auto_minutes: v })} />
          <NumberField label={t('versions.settings.recentHours')} value={draft.recent_hours} min={1} max={720} disabled={off} onCommit={(v) => patch({ recent_hours: v })} />
          <NumberField label={t('versions.settings.dailyDays')} value={draft.daily_days} min={0} max={365} disabled={off} onCommit={(v) => patch({ daily_days: v })} />
          <NumberField label={t('versions.settings.weeklyWeeks')} value={draft.weekly_weeks} min={0} max={520} disabled={off} onCommit={(v) => patch({ weekly_weeks: v })} />
          <NumberField label={t('versions.settings.maxMb')} value={draft.max_mb} min={1} max={10240} disabled={off} onCommit={(v) => patch({ max_mb: v })} />
          <NumberField label={t('versions.settings.maxVersions')} value={draft.max_versions} min={10} max={5000} disabled={off} onCommit={(v) => patch({ max_versions: v })} />
        </div>
        <p className="hint">{t('versions.settings.help')}</p>

        <div className="ver-settings__btns">
          <button type="button" className="btn btn--accent" disabled={busy || !dirty} onClick={() => void apply()}>
            {t('versions.settings.apply')}
          </button>
          <button type="button" className="btn btn--danger" disabled={!path || count === 0} onClick={() => void clearAll()}>
            {t('versions.settings.clearAll')}
          </button>
        </div>
      </div>
    </details>
  );
}
