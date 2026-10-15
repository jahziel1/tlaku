import { useEffect, useState } from 'react';
import { getApi } from '@/api';
import { t } from '@/i18n';
import { errorMessage } from '@/lib/errors';
import { normalizeLayers } from '@/lib/layers';
import { askChoice } from '@/features/history/prompt';
import { formatBytes, formatCopyDate } from '@/features/recovery/format';
import { findTabByPath, tabDoc, useStore } from '@/store';
import { highlightInCanvas, restoreAsNewTab, restoreInPlace, versionTitle } from './actions';
import { diffDocuments, highlightIds, type DocDiff } from './diff';
import { describeDiff } from './diffText';
import { Thumb } from './Thumb';
import type { VersionInfo } from './types';
import { forgetThumbs } from './useVersions';
import { tn } from '@/i18n';

interface Props {
  path: string;
  version: VersionInfo;
  /** Tras borrar o restaurar: recargar la lista. */
  onChanged: () => void;
  /** Cerrar el diálogo (al resaltar o restaurar, para ver el lienzo). */
  onClose: () => void;
}

type Compare =
  | { status: 'loading' }
  | { status: 'needOpen' }
  | { status: 'error'; message: string }
  | { status: 'ready'; diff: DocDiff };

/** Detalle de la versión elegida: vista previa, qué cambiaría al restaurarla y las acciones. */
export function VersionDetail({ path, version, onChanged, onClose }: Props) {
  const tabId = useStore((s) => findTabByPath(s, path));
  const current = useStore((s) => (tabId ? tabDoc(s, tabId) : null));
  const [compare, setCompare] = useState<Compare>({ status: 'loading' });
  const [busy, setBusy] = useState(false);

  useEffect(() => {
    let alive = true;
    if (!current) {
      setCompare({ status: 'needOpen' });
      return;
    }
    setCompare({ status: 'loading' });
    getApi()
      .versionsRead({ path, id: version.id })
      .then((doc) => alive && setCompare({ status: 'ready', diff: diffDocuments(normalizeLayers(doc), current) }))
      .catch((e: unknown) => alive && setCompare({ status: 'error', message: errorMessage(e) }));
    return () => {
      alive = false;
    };
  }, [path, version.id, current]);

  const diff = compare.status === 'ready' ? compare.diff : null;
  const highlight = diff ? highlightIds(diff) : [];

  const run = async (job: () => Promise<boolean>) => {
    setBusy(true);
    try {
      if (await job()) {
        onChanged();
        onClose();
      }
    } finally {
      setBusy(false);
    }
  };

  const remove = async () => {
    const choice = await askChoice({
      title: t('versions.delete.title'),
      message: t('versions.delete.message', { name: versionTitle(version) }),
      choices: [
        { id: 'delete', label: t('versions.delete.confirm'), tone: 'danger' },
        { id: 'keep', label: t('versions.delete.cancel') },
      ],
      cancelId: 'keep',
    });
    if (choice !== 'delete') return;
    setBusy(true);
    try {
      const n = await getApi().versionsDelete({ path, ids: [version.id] });
      forgetThumbs(path);
      useStore.getState().notify('success', tn('versions.notice.deleted', n));
      onChanged();
    } catch (e) {
      useStore.getState().notify('error', t('versions.notice.deleteFailed', { error: errorMessage(e) }));
    } finally {
      setBusy(false);
    }
  };

  const lines = diff ? describeDiff(diff) : [];

  return (
    <section className="ver-detail" aria-label={versionTitle(version)}>
      <div className="ver-detail__head">
        <h3 className="ver-detail__title">{versionTitle(version)}</h3>
        <span className="hint num">
          {t(`versions.kind.${version.kind}`)} · {formatCopyDate(version.created_ms)}
        </span>
      </div>

      <div className="ver-detail__preview">
        <Thumb path={path} id={version.id} has={version.has_thumb} large alt={t('versions.detail.preview')} />
        {!version.has_thumb && <p className="hint ver-detail__nothumb">{t('versions.detail.noThumb')}</p>}
      </div>
      <p className="hint num">
        {tn('versions.row.objects', version.objects)} · {tn('versions.row.layers', version.layers)} ·{' '}
        {t('versions.detail.size', { size: formatBytes(version.size_bytes), stored: formatBytes(version.stored_bytes) })}
      </p>

      <div className="ver-compare" aria-live="polite">
        <h4 className="ver-compare__title">{t('versions.compare.title')}</h4>
        {compare.status === 'loading' && <p className="hint">{t('versions.compare.loading')}</p>}
        {compare.status === 'needOpen' && <p className="hint">{t('versions.compare.needOpen')}</p>}
        {compare.status === 'error' && <p className="hint hint--danger">{t('versions.compare.failed', { error: compare.message })}</p>}
        {lines.length > 0 && (
          <ul className="ver-compare__list">
            {lines.map((line) => (
              <li key={line}>{line}</li>
            ))}
          </ul>
        )}
      </div>

      <div className="ver-detail__actions">
        <button type="button" className="btn btn--accent" disabled={busy} onClick={() => void run(() => restoreAsNewTab(path, version))}>
          {t('versions.detail.restoreNew')}
        </button>
        <button
          type="button"
          className="btn"
          disabled={busy || !tabId}
          title={tabId ? undefined : t('versions.reason.notOpen')}
          onClick={() => void run(() => restoreInPlace(path, version))}
        >
          {t('versions.detail.restoreHere')}
        </button>
        <button
          type="button"
          className="btn"
          disabled={busy || highlight.length === 0}
          title={highlight.length === 0 && diff ? t('versions.reason.nothingToHighlight') : undefined}
          onClick={() => {
            if (highlightInCanvas(path, highlight)) onClose();
          }}
        >
          {t('versions.detail.highlight')}
        </button>
        <button type="button" className="btn btn--danger" disabled={busy} onClick={() => void remove()}>
          {t('versions.detail.delete')}
        </button>
      </div>
    </section>
  );
}
