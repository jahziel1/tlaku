import { ExternalLink, RefreshCw, TriangleAlert, X } from 'lucide-react';
import { useId, useMemo } from 'react';
import type { CatalogEntry, CatalogOperation, CatalogState } from '@/api/types';
import { openExternal, isAllowedUrl } from '@/features/help/openExternal';
import { SITE_URL } from '@/features/help/links';
import { t, tn, type I18nKey } from '@/i18n';
import { selectActiveDevice, useStore } from '@/store';
import { Modal } from '@/ui/Modal';
import { activeLayerOf } from './actions';
import { applyEntryToLayer, closeCommunity, copyEntryToLibrary, formatFetched, refreshCommunity, shareFromActiveLayer } from './communityActions';
import {
  entryMaterialName,
  entrySummary,
  filterEntries,
  isOnlyIndicative,
  materialOptions,
  mineIsDegraded,
  myMachine,
  thicknessOptions,
  type Compat,
  type CommunityFilter,
  type MachineScope,
  type MyMachine,
} from './communityModel';
import { useCommunity, type CommunityStatus } from './communityStore';
import { classLabel } from './labels';
import { thicknessLabel } from './library';
import { useMaterials } from './state';
import './community.css';

const OPERATIONS: readonly CatalogOperation[] = ['cut', 'score', 'engrave', 'fill', 'image'];
const OP_LABEL: Record<CatalogOperation, I18nKey> = {
  cut: 'materials.community.op.cut',
  score: 'materials.community.op.score',
  engrave: 'materials.community.op.engrave',
  fill: 'materials.community.op.fill',
  image: 'materials.community.op.image',
};
const COMPAT_LABEL: Record<Compat, I18nKey> = {
  model: 'materials.community.compat.model',
  class: 'materials.community.compat.class',
  kind: 'materials.community.compat.kind',
  other: 'materials.community.compat.other',
};
const COMPAT_WHY: Record<Compat, I18nKey> = {
  model: 'materials.community.compat.model.why',
  class: 'materials.community.compat.class.why',
  kind: 'materials.community.compat.kind.why',
  other: 'materials.community.compat.other.why',
};
const COMPAT_TONE: Record<Compat, 'ok' | 'class' | 'warn'> = { model: 'ok', class: 'class', kind: 'warn', other: 'warn' };

const rowTitle = (e: CatalogEntry) => [entryMaterialName(e), e.thickness === undefined ? '' : thicknessLabel(e.thickness), t(OP_LABEL[e.operation])].filter(Boolean).join(' · ');

// ---------------------------------------------------------------------------
// Estado de la descarga
// ---------------------------------------------------------------------------

/** Una línea que dice de dónde salen los ajustes que se ven, qué tan viejos son y por qué no se pudo actualizar. */
export function StatusLine({ catalog, status }: { catalog: CatalogState | null; status: CommunityStatus }) {
  const date = catalog?.fetched_at != null ? formatFetched(catalog.fetched_at) : '';
  const published = catalog?.index?.generated_at ?? '';
  const loading = status === 'loading';
  let text: string;
  let warn = false;
  if (!catalog) {
    text = t('materials.community.status.loading');
  } else if (catalog.error) {
    warn = true;
    text = catalog.index ? t('materials.community.status.offline', { date, published }) : t('materials.community.status.none');
  } else if (catalog.source === 'network') {
    text = t('materials.community.status.network', { published });
  } else if (catalog.source === 'cache') {
    text = loading ? t('materials.community.status.cacheChecking', { date }) : t('materials.community.status.cache', { date, published });
  } else {
    text = loading ? t('materials.community.status.loading') : t('materials.community.status.none');
    warn = !loading;
  }
  return (
    <>
      <p className="com__status" data-tone={warn ? 'warn' : undefined} role="status">
        {text}
      </p>
      {catalog?.error && (
        <p className="com__status" data-tone="warn">
          {catalog.error}
        </p>
      )}
    </>
  );
}

// ---------------------------------------------------------------------------
// Lista y detalle
// ---------------------------------------------------------------------------

function VerifiedTag({ entry }: { entry: CatalogEntry }) {
  return entry.verified_by > 0 ? (
    <span className="mat-tag" data-tone="ok">{tn('materials.community.verified', entry.verified_by)}</span>
  ) : (
    <span className="mat-tag" title={t('materials.community.unverified.why')}>{t('materials.community.unverified')}</span>
  );
}

function CompatTag({ compat, entry }: { compat: Compat; entry: CatalogEntry }) {
  return (
    <span className="mat-tag" data-tone={COMPAT_TONE[compat]} title={t(COMPAT_WHY[compat], { class: classLabel(entry.machine_class), watts: entry.laser_watts })}>
      {t(COMPAT_LABEL[compat])}
    </span>
  );
}

interface DetailProps {
  entry: CatalogEntry;
  compat: Compat;
  canApply: boolean;
  onUse: (e: CatalogEntry) => void;
  onApply: (e: CatalogEntry) => void;
}

function Detail({ entry, compat, canApply, onUse, onApply }: DetailProps) {
  const name = rowTitle(entry);
  const source = isAllowedUrl(entry.source) ? entry.source : null;
  return (
    <article aria-label={t('materials.community.detail')}>
      <h3 className="com-d__title">{name}</h3>
      <p className="com-d__compat" data-tone={isOnlyIndicative(compat) ? 'warn' : undefined}>
        {isOnlyIndicative(compat) && <TriangleAlert size={14} aria-hidden="true" />}
        <span>
          <strong>{t(COMPAT_LABEL[compat])}.</strong> {t(COMPAT_WHY[compat], { class: classLabel(entry.machine_class), watts: entry.laser_watts })}
        </span>
      </p>

      <div className="com-d__actions">
        <button type="button" className="btn btn--accent" title={t('materials.community.use.why')} onClick={() => onUse(entry)}>
          {t('materials.community.use')}
        </button>
        <button
          type="button"
          className="btn"
          aria-disabled={!canApply}
          title={canApply ? undefined : t('materials.reason.noLayer')}
          onClick={canApply ? () => onApply(entry) : undefined}
        >
          {t('materials.community.apply')}
        </button>
        {source && (
          <button type="button" className="btn" aria-label={t('materials.community.detail.sourceFor', { name })} onClick={() => void openExternal(source)}>
            <ExternalLink size={14} aria-hidden="true" />
            {t('materials.community.detail.source')}
          </button>
        )}
      </div>

      <h4 className="com-d__sub">{t('materials.community.detail.values')}</h4>
      <dl className="com-d__facts num">
        <dt>{t('materials.community.detail.speed')}</dt>
        <dd>{entry.speed} mm/min</dd>
        <dt>{t('materials.community.detail.power')}</dt>
        <dd>{entry.power} %</dd>
        {entry.passes !== undefined && (
          <>
            <dt>{t('materials.community.detail.passes')}</dt>
            <dd>{entry.passes}</dd>
          </>
        )}
        {entry.interval !== undefined && (
          <>
            <dt>{t('materials.community.detail.interval')}</dt>
            <dd>{entry.interval} mm</dd>
          </>
        )}
        {entry.frequency_khz !== undefined && (
          <>
            <dt>{t('materials.community.detail.frequency')}</dt>
            <dd>{entry.frequency_khz} kHz</dd>
          </>
        )}
        <dt>{t('materials.community.detail.air')}</dt>
        <dd>{entry.air ? t('materials.community.detail.yes') : t('materials.community.detail.no')}</dd>
        <dt>{t('materials.community.detail.mode')}</dt>
        <dd>{t(`materials.mode.${entry.mode}`)}</dd>
      </dl>

      <h4 className="com-d__sub">{t('materials.community.detail.machine')}</h4>
      <dl className="com-d__facts">
        <dt>{t('materials.community.filter.machine')}</dt>
        <dd>{entry.machine}</dd>
        <dt>{t('materials.community.detail.power')}</dt>
        <dd className="num">{t('materials.community.detail.laser', { watts: entry.laser_watts, class: classLabel(entry.machine_class) })}</dd>
      </dl>

      {entry.notes && (
        <>
          <h4 className="com-d__sub">{t('materials.community.detail.notes')}</h4>
          <p className="hint" style={{ margin: 0 }}>{entry.notes}</p>
        </>
      )}

      <h4 className="com-d__sub">{entry.verified_by > 0 ? tn('materials.community.verified', entry.verified_by) : t('materials.community.unverified')}</h4>
      {entry.verified_by === 0 && <p className="hint" style={{ margin: 0 }}>{t('materials.community.unverified.why')}</p>}
      {entry.confirmations && entry.confirmations.length > 0 && (
        <>
          <p className="hint" style={{ margin: '0 0 var(--s-2)' }}>{t('materials.community.confirmations')}</p>
          <ul className="com-d__confirms">
            {entry.confirmations.map((c) => (
              <li key={c.by}>
                @{c.by} · {c.machine}
                {c.machine_class ? ` · ${classLabel(c.machine_class)}` : ''}
                {c.date ? ` · ${c.date}` : ''}
              </li>
            ))}
          </ul>
        </>
      )}

      <dl className="com-d__facts" style={{ marginTop: 'var(--s-5)' }}>
        {entry.contributor && (
          <>
            <dt>{t('materials.community.detail.author')}</dt>
            <dd>@{entry.contributor}</dd>
          </>
        )}
        {entry.date && (
          <>
            <dt>{t('materials.community.detail.date')}</dt>
            <dd className="num">{entry.date}</dd>
          </>
        )}
        {entry.tlaku_version && (
          <>
            <dt>{t('materials.community.detail.version')}</dt>
            <dd className="num">{entry.tlaku_version}</dd>
          </>
        )}
      </dl>
      <p className="hint com-d__license">{t('materials.community.detail.license')}</p>

    </article>
  );
}

// ---------------------------------------------------------------------------
// Vista
// ---------------------------------------------------------------------------

export interface BrowserViewProps {
  catalog: CatalogState | null;
  status: CommunityStatus;
  filter: CommunityFilter;
  selectedId: string | null;
  me: MyMachine;
  /** Hay una capa seleccionada a la que aplicar. */
  canApply: boolean;
  onClose: () => void;
  onRefresh: () => void;
  onFilter: (patch: Partial<CommunityFilter>) => void;
  onSelect: (id: string) => void;
  onUse: (e: CatalogEntry) => void;
  onApply: (e: CatalogEntry) => void;
  onShare: () => void;
}

/** Contenido del diálogo a partir de datos: lo pinta `CommunityBrowser` y lo prueban los tests. */
export function CommunityBrowserView({ catalog, status, filter, selectedId, me, canApply, onClose, onRefresh, onFilter, onSelect, onUse, onApply, onShare }: BrowserViewProps) {
  const titleId = useId();
  const ids = { machine: useId(), material: useId(), thickness: useId(), operation: useId(), search: useId() };
  const entries = useMemo(() => catalog?.index?.entries ?? [], [catalog]);
  const rows = useMemo(() => filterEntries(entries, filter, me), [entries, filter, me]);
  const materials = useMemo(() => materialOptions(entries), [entries]);
  const thicknesses = useMemo(() => thicknessOptions(entries), [entries]);
  const selected = rows.find((r) => r.entry.id === selectedId) ?? rows[0] ?? null;
  const loading = status === 'loading';
  const hasCatalog = catalog?.index != null;
  const degraded = mineIsDegraded(filter.scope, me);
  const classText = me.machineClass ? ` · ${classLabel(me.machineClass)}` : '';

  return (
    <Modal labelId={titleId} onClose={onClose} portal closeOnBackdrop fallbackFocus="first" initialFocus="[data-autofocus]" backdropClassName="mat-backdrop" cardClassName="mat-modal mat-modal--wide">
      <div className="com">
        <header className="com__head">
          <div>
            <h2 id={titleId} className="com__title">{t('materials.community.title')}</h2>
            <StatusLine catalog={catalog} status={status} />
            {hasCatalog && <p className="com__status">{tn('materials.community.count', entries.length)}</p>}
          </div>
          <div className="com__head-actions">
            <button type="button" className="btn" aria-disabled={loading} onClick={loading ? undefined : onRefresh}>
              <RefreshCw size={14} aria-hidden="true" />
              {loading ? t('materials.community.refreshing') : t('materials.community.refresh')}
            </button>
            <button type="button" className="icon-btn" title={t('materials.community.close')} aria-label={t('materials.community.close')} onClick={onClose}>
              <X size={16} aria-hidden="true" />
            </button>
          </div>
        </header>

        <div className="com__notes">
          <p className="mat-notice" role="note">
            <TriangleAlert size={14} aria-hidden="true" />
            <span>{t('materials.community.safety')}</span>
          </p>
          <p className="hint" style={{ margin: 0 }}>{t('materials.community.privacy', { site: new URL(SITE_URL).host })}</p>
        </div>

        <section className="com__filters" aria-label={t('materials.community.filters')}>
          <div className="field">
            <label className="field__label" htmlFor={ids.machine}>{t('materials.community.filter.machine')}</label>
            <select id={ids.machine} className="select" value={filter.scope} onChange={(e) => onFilter({ scope: e.target.value as MachineScope })}>
              <option value="mine">{t('materials.community.filter.machine.mine', { name: me.name, class: classText })}</option>
              <option value="kind">{t('materials.community.filter.machine.kind')}</option>
              <option value="all">{t('materials.community.filter.machine.all')}</option>
            </select>
          </div>
          <div className="field">
            <label className="field__label" htmlFor={ids.material}>{t('materials.community.filter.material')}</label>
            <select id={ids.material} className="select" value={filter.material} onChange={(e) => onFilter({ material: e.target.value })}>
              <option value="">{t('materials.community.filter.all')}</option>
              {materials.map((m) => <option key={m.key} value={m.key}>{m.name}</option>)}
            </select>
          </div>
          <div className="field">
            <label className="field__label" htmlFor={ids.thickness}>{t('materials.community.filter.thickness')}</label>
            <select id={ids.thickness} className="select" value={filter.thickness} onChange={(e) => onFilter({ thickness: e.target.value })}>
              <option value="">{t('materials.community.filter.all')}</option>
              {thicknesses.values.map((v) => <option key={v} value={String(v)}>{thicknessLabel(v)}</option>)}
              {thicknesses.hasNone && <option value="none">{t('materials.community.filter.noThickness')}</option>}
            </select>
          </div>
          <div className="field">
            <label className="field__label" htmlFor={ids.operation}>{t('materials.community.filter.operation')}</label>
            <select id={ids.operation} className="select" value={filter.operation} onChange={(e) => onFilter({ operation: e.target.value as CatalogOperation | '' })}>
              <option value="">{t('materials.community.filter.all')}</option>
              {OPERATIONS.map((op) => <option key={op} value={op}>{t(OP_LABEL[op])}</option>)}
            </select>
          </div>
          <div className="field com__wide">
            <label className="field__label" htmlFor={ids.search}>{t('materials.community.filter.search')}</label>
            <input id={ids.search} type="search" className="input" value={filter.search} placeholder={t('materials.community.filter.search.placeholder')} data-autofocus onChange={(e) => onFilter({ search: e.target.value })} />
          </div>
          <label className="mat-check com__wide">
            <input type="checkbox" checked={filter.verifiedOnly} onChange={(e) => onFilter({ verifiedOnly: e.target.checked })} />
            {t('materials.community.filter.verified')}
          </label>
          {degraded && (
            <p className="hint com__wide" role="note" style={{ margin: 0 }}>
              {me.kind === null ? t('materials.community.filter.degradedUnknown') : t('materials.community.filter.degraded')}
            </p>
          )}
        </section>

        <div className="com__main">
          <div className="com__list-wrap">
            {hasCatalog && entries.length === 0 && (
              <div className="com__empty">
                <p>{t('materials.community.empty')}</p>
                <button type="button" className="btn btn--accent" title={t('materials.community.share.why')} onClick={onShare}>
                  {t('materials.community.share')}
                </button>
              </div>
            )}
            {entries.length > 0 && rows.length === 0 && (
              <div className="com__empty" role="status">
                <p>{t('materials.community.noMatches')}</p>
                <p className="hint">{t('materials.community.noMatches.hint')}</p>
              </div>
            )}
            {rows.length > 0 && (
              <>
                <p className="com__count" role="status">{tn('materials.community.results', rows.length)}</p>
                <ul className="com__list" aria-label={t('materials.community.list')}>
                  {rows.map(({ entry, compat }) => (
                    <li key={entry.id}>
                      <button type="button" className="com-row" aria-current={selected?.entry.id === entry.id ? 'true' : undefined} onClick={() => onSelect(entry.id)}>
                        <span className="com-row__title">{rowTitle(entry)}</span>
                        <span className="com-row__sum num">{entrySummary(entry)}</span>
                        <span className="com-row__tags">
                          <CompatTag compat={compat} entry={entry} />
                          <VerifiedTag entry={entry} />
                        </span>
                        <span className="com-row__machine">{entry.machine} · {entry.laser_watts} W</span>
                      </button>
                    </li>
                  ))}
                </ul>
              </>
            )}
          </div>
          <div className="com__detail">
            {selected ? (
              <Detail entry={selected.entry} compat={selected.compat} canApply={canApply} onUse={onUse} onApply={onApply} />
            ) : (
              <p className="hint">{t('materials.community.detail.none')}</p>
            )}
          </div>
        </div>

        <footer className="com__foot">
          <button type="button" className="btn" title={t('materials.community.share.why')} onClick={onShare}>
            {t('materials.community.share')}
          </button>
          <button type="button" className="btn" onClick={onClose}>{t('materials.community.close')}</button>
        </footer>
      </div>
    </Modal>
  );
}

/** Diálogo conectado al estado de la app: lo monta `MaterialsDialogHost` mientras `useCommunity().open`. */
export function CommunityBrowser() {
  const catalog = useCommunity((s) => s.catalog);
  const status = useCommunity((s) => s.status);
  const filter = useCommunity((s) => s.filter);
  const selectedId = useCommunity((s) => s.selectedId);
  const device = useStore(selectActiveDevice);
  const machineClass = useMaterials((s) => s.machineClass);
  const canApply = useStore((s) => activeLayerOf(s) !== null);
  const me = useMemo(() => myMachine(device, machineClass), [device, machineClass]);
  return (
    <CommunityBrowserView
      {...{ catalog, status, filter, selectedId, me, canApply }}
      onClose={closeCommunity}
      onRefresh={() => void refreshCommunity()}
      onFilter={(patch) => useCommunity.getState().setFilter(patch)}
      onSelect={(id) => useCommunity.getState().select(id)}
      onUse={(e) => void copyEntryToLibrary(e)}
      onApply={(e) => void applyEntryToLayer(e, me)}
      onShare={shareFromActiveLayer}
    />
  );
}

