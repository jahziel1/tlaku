import { useEffect, useId, useMemo, useState } from 'react';
import { t, tn } from '@/i18n';
import { Modal } from '@/ui/Modal';
import { baseName } from '@/features/history/paths';
import { formatBytes, formatCopyDate } from '@/features/recovery/format';
import { openHistoryOfFile, openSaveVersion, versionTitle } from './actions';
import { Thumb } from './Thumb';
import type { VersionInfo } from './types';
import { useVersions } from './useVersions';
import { VersionDetail } from './VersionDetail';
import { VersionsSettings } from './VersionsSettings';
import { useVersionsUi } from './versionsUi';
import './versions.css';

function VersionRow({ path, v, selected, onPick }: { path: string; v: VersionInfo; selected: boolean; onPick: () => void }) {
  return (
    <li>
      <button type="button" className="ver-row" data-kind={v.kind} aria-pressed={selected} onClick={onPick}>
        <Thumb path={path} id={v.id} has={v.has_thumb} />
        <span className="ver-row__text">
          <span className="ver-row__title">{versionTitle(v)}</span>
          <span className="ver-row__meta num">
            <span className="ver-row__kind" data-kind={v.kind}>
              {t(`versions.kind.${v.kind}`)}
            </span>
            {v.label !== null && <span> · {formatCopyDate(v.created_ms)}</span>}
          </span>
          <span className="ver-row__meta num">
            {tn('versions.row.objects', v.objects)} · {tn('versions.row.layers', v.layers)} · {formatBytes(v.size_bytes)}
          </span>
        </span>
      </button>
    </li>
  );
}

/** Diálogo «Historial de versiones»: lista, vista previa, comparación y restauración. */
export function VersionsDialog() {
  const path = useVersionsUi((s) => s.path);
  const changes = useVersionsUi((s) => s.changes);
  const close = useVersionsUi((s) => s.close);
  const openFor = useVersionsUi((s) => s.openFor);
  const titleId = useId();
  const projectsId = useId();
  const { listing, projects, loading, error, reload } = useVersions(path);
  const [selectedId, setSelectedId] = useState<string | null>(null);

  useEffect(() => {
    if (changes > 0) void reload();
    // eslint-disable-next-line react-hooks/exhaustive-deps -- solo cuando el historial cambia desde fuera
  }, [changes]);

  const versions = useMemo(() => listing?.versions ?? [], [listing]);
  const selected = versions.find((v) => v.id === selectedId) ?? null;
  useEffect(() => {
    // Si la versión elegida ya no existe (se borró), se vuelve a la primera.
    if (selectedId !== null && !versions.some((v) => v.id === selectedId)) setSelectedId(null);
  }, [versions, selectedId]);

  const others = projects.filter((p) => p.path !== path);

  return (
    <Modal labelId={titleId} onClose={close} backdropClassName="modal" cardClassName="modal__card ver-card" initialFocus="[data-autofocus]" guardOutside>
      <div className="modal__head">
        <div className="ver-head">
          <h2 id={titleId} className="modal__title">
            {t('versions.title')}
          </h2>
          {path && <span className="hint ver-head__path" title={path}>{t('versions.subtitle', { name: baseName(path) })}</span>}
        </div>
      </div>

      <div className="ver-toolbar">
        <button type="button" className="btn" data-autofocus="" disabled={!path} onClick={() => openSaveVersion()}>
          {t('versions.toolbar.save')}
        </button>
        <button type="button" className="btn" onClick={() => void openHistoryOfFile()}>
          {t('versions.toolbar.openOther')}
        </button>
        {others.length > 0 && (
          <div className="ver-toolbar__projects">
            <label htmlFor={projectsId} className="hint">
              {t('versions.projects.label')}
            </label>
            <select id={projectsId} className="select" value="" onChange={(e) => e.target.value && openFor(e.target.value)}>
              <option value="">{t('versions.projects.choose')}</option>
              {others.map((p) => (
                <option key={p.path} value={p.path} title={p.path}>
                  {t('versions.projects.option', { name: p.name, n: p.versions })}
                  {p.exists ? '' : ` ${t('versions.projects.missing')}`}
                </option>
              ))}
            </select>
          </div>
        )}
      </div>

      <div className="ver-body">
        {!path ? (
          <div className="ver-empty">
            <h3>{t('versions.noFile.title')}</h3>
            <p className="hint">{t('versions.noFile.body')}</p>
          </div>
        ) : error ? (
          <p className="hint hint--danger ver-empty">{t('versions.loadFailed', { error })}</p>
        ) : (
          <>
            <div className="ver-list-wrap">
              {loading && !listing && <p className="hint ver-empty">{t('versions.loading')}</p>}
              {listing && versions.length === 0 && <p className="hint ver-empty">{t('versions.list.empty')}</p>}
              {versions.length > 0 && (
                <ul className="ver-list" aria-label={t('versions.list.label')}>
                  {versions.map((v) => (
                    <VersionRow key={v.id} path={path} v={v} selected={v.id === selectedId} onPick={() => setSelectedId(v.id)} />
                  ))}
                </ul>
              )}
            </div>
            <div className="ver-detail-wrap">
              {selected ? (
                <VersionDetail key={selected.id} path={path} version={selected} onChanged={() => void reload()} onClose={close} />
              ) : (
                <p className="hint ver-empty">{t('versions.detail.empty')}</p>
              )}
            </div>
          </>
        )}
      </div>

      <VersionsSettings path={path} count={versions.length} onCleared={() => void reload()} />

      <div className="modal__foot ver-foot">
        <span className="hint ver-foot__info" title={listing?.dir}>
          {listing ? `${tn('versions.footer.summary', versions.length, { size: formatBytes(listing.total_bytes) })} · ${t('versions.footer.dir', { dir: listing.dir })}` : ''}
        </span>
        <button type="button" className="btn" onClick={close}>
          {t('versions.close')}
        </button>
      </div>
    </Modal>
  );
}
