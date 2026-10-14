import { ChevronLeft, ChevronRight, Search, X } from 'lucide-react';
import { useEffect, useMemo, useRef, useState, type ReactNode } from 'react';
import { getApi } from '@/api';
import { t } from '@/i18n';
import { getPref, useStore } from '@/store';
import { isJobBusy } from '@/store/selectors';
import { Modal } from '@/ui/Modal';
import { JobLogDialog } from './JobLogDialog';
import { CopyButton } from './CopyButton';
import { systemInfo } from './clipboard';
import { useHelpUi } from './helpStore';
import { donationLinks, PROJECT_LINKS } from './links';
import { MarkdownView, type MarkdownActions } from './MarkdownView';
import { findPage, indexPages, INDEX_FILE, manualPages, NEWS_FILE, searchPages, thirdPartyBlocks } from './pages';
import { isMainWindow, scheduleStartupCheck, useUpdateUi, type UpdateInfo, type UpdatePhase } from './updater';
import './help.css';

/** Marco común: fondo, tarjeta, título y botón de cerrar. */
function Frame({ id, title, wide, onClose, children, head }: {
  id: string;
  title: string;
  wide?: boolean;
  onClose: () => void;
  children: ReactNode;
  head?: ReactNode;
}) {
  return (
    <Modal
      labelId={id}
      onClose={onClose}
      backdropClassName="help"
      cardClassName={`help__card${wide ? ' help__card--wide' : ''}`}
      initialFocus="[data-autofocus], input"
      closeOnBackdrop
      guardOutside
    >
      <div className="help__head">
        <h2 id={id} className="help__title">{title}</h2>
        {head}
        <button type="button" className="icon-btn" aria-label={t('help.close')} title={t('help.close')} onClick={onClose}>
          <X size={16} aria-hidden="true" />
        </button>
      </div>
      {children}
    </Modal>
  );
}

/** Una dirección copiable, o el aviso de que aún no se publica. */
function LinkRow({ url }: { url: string }) {
  if (!url) return <p className="help__pending">{t('help.linkPending')}</p>;
  return (
    <p className="help__linkrow">
      <code className="md__url">{url}</code>
      <CopyButton text={url} label={t('help.copyLink')} />
    </p>
  );
}

// ---- manual -------------------------------------------------------------------------------

export function ManualReader({ title, titleId, file, single }: { title: string; titleId: string; file?: string; single?: boolean }) {
  const close = useHelpUi((s) => s.close);
  const page = useHelpUi((s) => s.page);
  const anchor = useHelpUi((s) => s.anchor);
  const goTo = useHelpUi((s) => s.goTo);
  const [query, setQuery] = useState('');
  const scroller = useRef<HTMLDivElement>(null);

  const shown = single ? [] : searchPages(query);
  const current = file ? findPage(file) : (findPage(page) ?? findPage(INDEX_FILE) ?? manualPages()[0]);
  const order = indexPages();
  const at = current ? order.findIndex((p) => p.file === current.file) : -1;
  const prev = !single && at > 0 ? order[at - 1] : null;
  const next = !single && at >= 0 && at < order.length - 1 ? order[at + 1] : null;

  const scrollTo = (id: string) => {
    const el = scroller.current?.querySelector<HTMLElement>(`#md-${CSS.escape(id)}`);
    if (el) el.scrollIntoView?.({ block: 'start' });
  };
  useEffect(() => {
    if (anchor && !single) scrollTo(anchor);
    else if (scroller.current) scroller.current.scrollTop = 0;
    // `scrollTo` solo lee el contenedor.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [current?.file, anchor]);

  const actions = useMemo<MarkdownActions>(
    () => ({
      hasPage: (f) => findPage(f) !== undefined,
      onPage: (f, a) => {
        if (!single) goTo(f, a);
      },
      onAnchor: scrollTo,
    }),
    [goTo, single],
  );

  return (
    <Frame id={titleId} title={title} wide onClose={close}
      head={single ? null : (
        <div className="help__search">
          <Search size={14} aria-hidden="true" />
          <input
            type="search"
            className="input"
            value={query}
            placeholder={t('help.manual.search')}
            aria-label={t('help.manual.search')}
            onChange={(e) => setQuery(e.target.value)}
          />
        </div>
      )}
    >
      <div className={`help__body${single ? ' help__body--single' : ''}`}>
        {!single && (
          <nav className="help__nav" aria-label={t('help.manual.toc')}>
            {shown.length === 0 && <p className="panel-empty" role="status">{t('help.manual.none', { q: query })}</p>}
            {shown.map((p) => (
              <button
                key={p.file}
                type="button"
                className="help__navitem"
                aria-current={current?.file === p.file ? 'page' : undefined}
                onClick={() => goTo(p.file)}
              >
                {p.title}
              </button>
            ))}
          </nav>
        )}
        <div className="help__content" ref={scroller} tabIndex={0} role="region" aria-label={current?.title ?? t('help.manual.page')}>
          {current ? <MarkdownView blocks={current.blocks} actions={actions} /> : <p className="panel-empty">{t('help.manual.missing')}</p>}
          <p className="hint help__note">{t('help.linkNote')}</p>
        </div>
      </div>
      {!single && (
        <div className="help__foot">
          <button type="button" className="btn" disabled={!prev} aria-label={prev ? undefined : t('a11y.manual.prev')} onClick={() => prev && goTo(prev.file)}
            title={prev ? t('help.manual.prev', { title: prev.title }) : undefined}>
            <ChevronLeft size={14} aria-hidden="true" />
            <span>{prev ? t('help.manual.prev', { title: prev.title }) : ''}</span>
          </button>
          <button type="button" className="btn" disabled={!next} aria-label={next ? undefined : t('a11y.manual.next')} onClick={() => next && goTo(next.file)}
            title={next ? t('help.manual.next', { title: next.title }) : undefined}>
            <span>{next ? t('help.manual.next', { title: next.title }) : ''}</span>
            <ChevronRight size={14} aria-hidden="true" />
          </button>
        </div>
      )}
    </Frame>
  );
}

// ---- acerca de, reporte y donaciones ---------------------------------------------------------

/** Versión de la app (comando `app_version`); `null` mientras carga o si falla. */
function useAppVersion(): string | null | undefined {
  const [version, setVersion] = useState<string | null | undefined>(undefined);
  useEffect(() => {
    let alive = true;
    getApi()
      .appVersion()
      .then((v) => alive && setVersion(v))
      .catch(() => alive && setVersion(null));
    return () => {
      alive = false;
    };
  }, []);
  return version;
}

export function AboutDialog() {
  const close = useHelpUi((s) => s.close);
  const version = useAppVersion();
  const credits = useMemo(() => thirdPartyBlocks(), []);
  return (
    <Frame id="help-about-title" title={t('help.about.title')} onClose={close}>
      <div className="help__scroll help__prose">
        <p className="help__tagline">{t('help.about.tagline')}</p>
        <dl className="help__facts">
          <dt>{t('help.about.version')}</dt>
          <dd data-autofocus tabIndex={-1}>{version === undefined ? '…' : (version ?? t('help.about.versionUnknown'))}</dd>
          <dt>{t('help.about.license')}</dt>
          <dd>{t('help.about.licenseText')}</dd>
        </dl>
        <h3 className="help__sub">{t('help.about.name')}</h3>
        <p>{t('help.about.nameText')}</p>
        <p>{t('help.about.donate')}</p>
        <p className="help__warnnote">{t('help.about.honest')}</p>
        <details className="help__credits">
          <summary>{t('help.about.credits')}</summary>
          <p className="hint">{t('help.about.creditsHint')}</p>
          <MarkdownView blocks={credits} />
        </details>
      </div>
    </Frame>
  );
}

export function ReportDialog() {
  const close = useHelpUi((s) => s.close);
  const version = useAppVersion();
  const info = systemInfo(version ?? null, typeof navigator === 'undefined' ? {} : navigator);
  return (
    <Frame id="help-report-title" title={t('help.report.title')} onClose={close}>
      <div className="help__scroll help__prose">
        <p>{t('help.report.intro')}</p>
        <h3 className="help__sub">{t('help.report.include')}</h3>
        <ul>
          <li>{t('help.report.item1')}</li>
          <li>{t('help.report.item2')}</li>
          <li>{t('help.report.item3')}</li>
          <li>{t('help.report.item4')}</li>
        </ul>
        <p className="help__warnnote">{t('help.report.warning')}</p>
        <h3 className="help__sub">{t('help.report.info')}</h3>
        <pre className="md__pre" data-autofocus tabIndex={0} role="region" aria-label={t('help.report.info')}><code>{info}</code></pre>
        <p><CopyButton text={info} label={t('help.report.copyInfo')} /></p>
        <h3 className="help__sub">{t('help.report.where')}</h3>
        <LinkRow url={PROJECT_LINKS.issues} />
        <p className="hint">{t('help.linkNote')}</p>
      </div>
    </Frame>
  );
}

export function DonateDialog() {
  const close = useHelpUi((s) => s.close);
  const links = donationLinks();
  return (
    <Frame id="help-donate-title" title={t('help.donate.title')} onClose={close}>
      <div className="help__scroll help__prose">
        {links.length === 0 ? (
          <p data-autofocus tabIndex={-1}>{t('help.donate.none')}</p>
        ) : (
          <>
            <p>{t('help.donate.text')}</p>
            <h3 className="help__sub">{t('help.donate.where')}</h3>
            {links.map((l) => (
              <div key={l.id} data-donation={l.id}>
                <p className="help__donor">{l.id === 'sponsors' ? 'GitHub Sponsors' : 'Ko-fi'}</p>
                <LinkRow url={l.url} />
              </div>
            ))}
            <p className="hint">{t('help.linkNote')}</p>
          </>
        )}
      </div>
    </Frame>
  );
}

// ---- actualizaciones -------------------------------------------------------------------------

export interface UpdateViewProps {
  phase: UpdatePhase;
  info: UpdateInfo | null;
  progress: { done: number; total: number | null };
  error: string;
  /** Hay un trabajo en la máquina: no se instala hasta que termine. */
  jobBusy: boolean;
  onClose: () => void;
  onInstall: () => void;
  onRestart: () => void;
  onRetry: () => void;
}

/** Vista pura del diálogo de actualizaciones (un solo cuadro que pasa por buscar, avisar, bajar e instalar). */
export function UpdateView({ phase, info, progress, error, jobBusy, onClose, onInstall, onRestart, onRetry }: UpdateViewProps) {
  const percent = progress.total ? Math.min(100, Math.floor((progress.done / progress.total) * 100)) : null;
  const closeOnly = phase === 'idle' || phase === 'unconfigured' || phase === 'current';
  return (
    <Frame id="help-update-title" title={t('help.update.title')} onClose={onClose}>
      <div className="help__scroll help__prose">
        {phase === 'checking' && <p role="status" data-autofocus tabIndex={-1}>{t('help.update.checking')}</p>}
        {phase === 'unconfigured' && (
          <>
            <p data-autofocus tabIndex={-1}>{t('help.update.unconfigured')}</p>
            <p className="hint">{t('help.update.unconfiguredHint')}</p>
          </>
        )}
        {phase === 'current' && <p data-autofocus tabIndex={-1}>{t('help.update.current')}</p>}
        {(phase === 'available' || phase === 'downloading') && info && (
          <>
            <p className="help__tagline" data-autofocus tabIndex={-1}>{t('help.update.available', { version: info.version })}</p>
            <p>{t('help.update.yours', { current: info.current })}</p>
            <h3 className="help__sub">{t('help.update.notes')}</h3>
            <div className="help__notes" role="region" aria-label={t('help.update.notes')} tabIndex={0}>{info.notes || t('help.update.noNotes')}</div>
            <p className="hint">{t('help.update.verified')}</p>
            {phase === 'available' && jobBusy && <p className="help__warnnote">{t('help.update.busy')}</p>}
            {phase === 'downloading' && (
              <p role="status">{percent === null ? t('help.update.downloading') : t('help.update.downloadingPct', { percent })}</p>
            )}
          </>
        )}
        {phase === 'installed' && (
          <>
            <p data-autofocus tabIndex={-1}>{t('help.update.installed')}</p>
            <p className="help__warnnote">{t('help.update.installedHint')}</p>
          </>
        )}
        {phase === 'error' && <p role="alert" className="help__pending" data-autofocus tabIndex={-1}>{t('help.update.error', { error })}</p>}
      </div>
      <div className="help__foot help__foot--end">
        {phase === 'available' && (
          <>
            <button type="button" className="btn" onClick={onClose}>{t('help.update.later')}</button>
            <button type="button" className="btn btn--accent" disabled={jobBusy} onClick={onInstall}>{t('help.update.install')}</button>
          </>
        )}
        {phase === 'installed' && (
          <>
            <button type="button" className="btn" onClick={onClose}>{t('help.update.restartLater')}</button>
            <button type="button" className="btn btn--accent" onClick={onRestart}>{t('help.update.restart')}</button>
          </>
        )}
        {phase === 'error' && (
          <>
            <button type="button" className="btn" onClick={onClose}>{t('help.close')}</button>
            <button type="button" className="btn btn--accent" onClick={onRetry}>{t('help.update.retry')}</button>
          </>
        )}
        {closeOnly && <button type="button" className="btn" onClick={onClose}>{t('help.close')}</button>}
      </div>
    </Frame>
  );
}

export function UpdateDialog() {
  const close = useHelpUi((s) => s.close);
  const phase = useUpdateUi((s) => s.phase);
  const info = useUpdateUi((s) => s.info);
  const progress = useUpdateUi((s) => s.progress);
  const error = useUpdateUi((s) => s.error);
  const jobBusy = useStore(isJobBusy);
  return (
    <UpdateView
      phase={phase}
      info={info}
      progress={progress}
      error={error}
      jobBusy={jobBusy}
      onClose={close}
      onInstall={() => void useUpdateUi.getState().install()}
      onRestart={() => void useUpdateUi.getState().restart()}
      onRetry={() => void useUpdateUi.getState().check(false)}
    />
  );
}

/** Monta el diálogo de ayuda que esté abierto. Va una sola vez en `AppShell`. */
export function HelpDialogHost() {
  const dialog = useHelpUi((s) => s.dialog);
  // Chequeo de actualizaciones al iniciar, si la preferencia lo pide (apagada de fábrica).
  useEffect(() => scheduleStartupCheck(() => getPref('updateCheckOnStart'), isMainWindow), []);
  switch (dialog) {
    case 'manual':
      return <ManualReader title={t('help.manual.title')} titleId="help-manual-title" />;
    case 'news':
      return <ManualReader title={t('help.news.title')} titleId="help-news-title" file={NEWS_FILE} single />;
    case 'about':
      return <AboutDialog />;
    case 'report':
      return <ReportDialog />;
    case 'donate':
      return <DonateDialog />;
    case 'jobLog':
      return <JobLogDialog />;
    case 'update':
      return <UpdateDialog />;
    default:
      return null;
  }
}
