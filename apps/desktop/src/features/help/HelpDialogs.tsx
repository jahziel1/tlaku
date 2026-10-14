import { ChevronLeft, ChevronRight, Search, X } from 'lucide-react';
import { useEffect, useMemo, useRef, useState, type ReactNode } from 'react';
import { getApi } from '@/api';
import { t } from '@/i18n';
import { Modal } from '@/ui/Modal';
import { JobLogDialog } from './JobLogDialog';
import { CopyButton } from './CopyButton';
import { systemInfo } from './clipboard';
import { useHelpUi } from './helpStore';
import { PROJECT_LINKS } from './links';
import { MarkdownView, type MarkdownActions } from './MarkdownView';
import { findPage, indexPages, INDEX_FILE, manualPages, NEWS_FILE, searchPages, thirdPartyBlocks } from './pages';
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
function LinkRow({ url }: { url: string | null }) {
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
          <button type="button" className="btn" disabled={!prev} onClick={() => prev && goTo(prev.file)}
            title={prev ? t('help.manual.prev', { title: prev.title }) : undefined}>
            <ChevronLeft size={14} aria-hidden="true" />
            <span>{prev ? t('help.manual.prev', { title: prev.title }) : ''}</span>
          </button>
          <button type="button" className="btn" disabled={!next} onClick={() => next && goTo(next.file)}
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
        <pre className="md__pre" data-autofocus tabIndex={0}><code>{info}</code></pre>
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
  return (
    <Frame id="help-donate-title" title={t('help.donate.title')} onClose={close}>
      <div className="help__scroll help__prose">
        <p>{t('help.donate.text')}</p>
        <h3 className="help__sub">{t('help.donate.where')}</h3>
        <LinkRow url={PROJECT_LINKS.donate} />
        <p className="hint">{t('help.linkNote')}</p>
      </div>
    </Frame>
  );
}

/** Monta el diálogo de ayuda que esté abierto. Va una sola vez en `AppShell`. */
export function HelpDialogHost() {
  const dialog = useHelpUi((s) => s.dialog);
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
    default:
      return null;
  }
}
