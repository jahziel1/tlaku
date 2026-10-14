import { Info, TriangleAlert } from 'lucide-react';
import { useId } from 'react';
import type { ProbeResult } from '@/api/api';
import type { Point } from '@/api/types';
import type { DeviceKind } from '@/bindings/DeviceKind';
import { t, type I18nKey } from '@/i18n';
import { BAUD_RATES, DEVICE_KINDS } from '@/lib/device';
import { fmt } from '@/lib/platform';
import { entrySources, type CatalogEntry, type Fact } from './catalog';
import type { CompareField, Comparison, CompareRow } from './compare';
import type { RankedEntry } from './filter';
import type { BuiltProfile, Provenance, ProfileField } from './profile';
import { LhyBoardField } from './LhyBoardField';
import { RuidaHostField } from './RuidaHostField';

// ---------------------------------------------------------------------------
// Utilidades de presentación
// ---------------------------------------------------------------------------

/** `400 × 415`, `1000` o `—`. */
export function showValue(v: number | Point | null): string {
  if (v === null) return t('catalog.review.empty');
  if (typeof v === 'number') return fmt(v, Number.isInteger(v) ? 0 : 2);
  return `${fmt(v[0], 0)} × ${fmt(v[1], 0)}`;
}

const FIELD_LABEL: Record<CompareField, I18nKey> = {
  s_max: 'catalog.field.s_max',
  bed: 'catalog.field.bed',
  max_rate: 'catalog.field.max_rate',
  accel: 'catalog.field.accel',
};

const kindLabel = (k: DeviceKind) => t(`catalog.kind.${k}`);

// ---------------------------------------------------------------------------
// 1. Detectar
// ---------------------------------------------------------------------------

export type ProbeState =
  | { status: 'idle' }
  | { status: 'probing' }
  | { status: 'error'; message: string }
  | { status: 'done'; result: ProbeResult };

export interface DetectStepProps {
  ports: string[];
  port: string;
  baud: number;
  probe: ProbeState;
  /** Hay una máquina conectada: su puerto está ocupado. */
  machineConnected: boolean;
  onPort: (port: string) => void;
  onBaud: (baud: number) => void;
  onRefresh: () => void;
  onDetect: () => void;
  /** Placas de K40 (M2 Nano) que el motor encontró por USB (`usb:k40:<n>`). */
  k40Boards?: string[];
  /** Elegir «K40 con M2 Nano» y pasar al modelo; sin esto no se muestra la parte del K40. */
  onPickK40?: () => void;
}

function portLabel(port: string): string {
  return port.startsWith('virtual:') ? t('catalog.detect.virtual', { port }) : port;
}

export function DetectStep(p: DetectStepProps) {
  const id = useId();
  const probing = p.probe.status === 'probing';
  const blocked = p.machineConnected || probing || !p.port;
  return (
    <div className="dev-step">
      <p className="hint">{t('catalog.detect.intro')}</p>
      <div className="dev-detect-row">
        <div className="field grow">
          <label htmlFor={`${id}-port`} className="field__label">{t('catalog.detect.port')}</label>
          <select id={`${id}-port`} className="select" value={p.port} disabled={probing} onChange={(e) => p.onPort(e.target.value)}>
            {p.ports.length === 0 && <option value="">{t('catalog.detect.noPorts')}</option>}
            {p.ports.map((port) => (
              <option key={port} value={port}>{portLabel(port)}</option>
            ))}
          </select>
        </div>
        <div className="field">
          <label htmlFor={`${id}-baud`} className="field__label">{t('catalog.detect.baud')}</label>
          <select id={`${id}-baud`} className="select" value={p.baud} disabled={probing} aria-describedby={`${id}-baud-help`} onChange={(e) => p.onBaud(Number(e.target.value))}>
            {BAUD_RATES.map((b) => (
              <option key={b} value={b}>{b}</option>
            ))}
          </select>
        </div>
        <button type="button" className="btn" disabled={probing} onClick={p.onRefresh}>{t('catalog.detect.refresh')}</button>
        <button type="button" className="btn btn--accent" disabled={blocked} onClick={p.onDetect}>
          {probing ? t('catalog.detect.running') : t('catalog.detect.run')}
        </button>
      </div>
      <p id={`${id}-baud-help`} className="hint">{t('catalog.detect.baudHelp')}</p>
      {p.ports.length === 0 && <p className="hint">{t('catalog.detect.noPortsHelp')}</p>}
      {p.machineConnected && (
        <p className="warn-line"><TriangleAlert size={14} aria-hidden="true" /><span>{t('catalog.detect.connected')}</span></p>
      )}
      <div aria-live="polite" className="dev-status">
        {probing && <p className="hint">{t('catalog.detect.wait')}</p>}
        {p.probe.status === 'error' && (
          <p className="hint hint--danger" role="alert">{t('catalog.detect.failed', { error: p.probe.message })}</p>
        )}
        {p.probe.status === 'done' && <ProbeSummary result={p.probe.result} />}
      </div>
      {p.onPickK40 && (
        <section className="dev-k40" aria-labelledby={`${id}-k40`}>
          <h3 id={`${id}-k40`} className="mc-sub"><Info size={13} aria-hidden="true" /> {t('lhymicro.wizard.title')}</h3>
          <p className="hint">{t('lhymicro.wizard.intro')}</p>
          {(p.k40Boards ?? []).length > 0 ? (
            <p className="hint" role="status">{t('lhymicro.wizard.found', { n: p.k40Boards?.length ?? 0 })}</p>
          ) : (
            <p className="hint" role="status">{t('lhymicro.wizard.none')}</p>
          )}
          <button type="button" className={(p.k40Boards ?? []).length > 0 ? 'btn btn--accent' : 'btn'} onClick={p.onPickK40}>{t('lhymicro.wizard.use')}</button>
        </section>
      )}
      <section className="dev-network" aria-labelledby={`${id}-net`}>
        <h3 id={`${id}-net`} className="mc-sub"><Info size={13} aria-hidden="true" /> {t('catalog.detect.network.title')}</h3>
        <p className="hint">{t('catalog.detect.network.text')}</p>
      </section>
    </div>
  );
}

export function ProbeSummary({ result }: { result: ProbeResult }) {
  const count = result.settings ? Object.keys(result.settings).length : 0;
  return (
    <section className="dev-summary" aria-label={t('catalog.detect.result')}>
      <dl className="dev-dl">
        <dt>{t('catalog.detect.family')}</dt>
        <dd>{t(`catalog.family.${result.family}`)}</dd>
        <dt>{t('catalog.detect.firmware')}</dt>
        <dd className="num">{result.firmware || t('catalog.detect.firmwareUnknown')}</dd>
        <dt>{t('catalog.detect.settings')}</dt>
        <dd>{count > 0 ? t('catalog.detect.settingsCount', { n: count }) : t('catalog.detect.settingsNone')}</dd>
      </dl>
      {result.family === 'unknown' && (
        <p className="warn-line"><TriangleAlert size={14} aria-hidden="true" /><span>{t('catalog.detect.unknownHelp')}</span></p>
      )}
      <details>
        <summary>{t('catalog.detect.banner')}</summary>
        {result.banner || result.info.length > 0 ? (
          <pre className="dev-pre num">{[result.banner, ...result.info].filter(Boolean).join('\n')}</pre>
        ) : (
          <p className="hint">{t('catalog.detect.bannerEmpty')}</p>
        )}
      </details>
    </section>
  );
}

// ---------------------------------------------------------------------------
// 2. Modelo
// ---------------------------------------------------------------------------

/** Identificador de la opción "Otro / genérico". */
export const OTHER_ID = 'other';

export interface ModelStepProps {
  ranked: RankedEntry[];
  query: string;
  onQuery: (q: string) => void;
  selectedId: string | null;
  onSelect: (id: string) => void;
  probe: ProbeResult | null;
  showAll: boolean;
  onShowAll: (v: boolean) => void;
  otherKind: DeviceKind;
  onOtherKind: (k: DeviceKind) => void;
}

export function ModelStep(p: ModelStepProps) {
  const id = useId();
  const selected = p.ranked.find((r) => r.entry.id === p.selectedId)?.entry ?? null;
  return (
    <div className="dev-step dev-model">
      <div className="dev-model__list">
        <div className="field">
          <label htmlFor={`${id}-q`} className="field__label">{t('catalog.model.search')}</label>
          <input id={`${id}-q`} type="search" className="input" value={p.query} placeholder={t('catalog.model.searchPlaceholder')} onChange={(e) => p.onQuery(e.target.value)} />
        </div>
        {p.probe && p.probe.family !== 'unknown' ? (
          <label className="mat-check">
            <input type="checkbox" checked={p.showAll} onChange={(e) => p.onShowAll(e.target.checked)} />
            <span>{t('catalog.model.showAll')}</span>
          </label>
        ) : (
          <p className="hint">{t('catalog.model.noDetection')}</p>
        )}
        <ul className="dev-list" role="radiogroup" aria-label={t('catalog.model.list')}>
          {p.ranked.length === 0 && <li className="hint">{t('catalog.model.empty')}</li>}
          {p.ranked.map((r) => {
            const e = r.entry;
            const checked = e.id === p.selectedId;
            return (
              <li key={e.id}>
                <label className="dev-item" data-checked={checked || undefined}>
                  <input type="radio" name={`${id}-model`} checked={checked} onChange={() => p.onSelect(e.id)} />
                  <span className="dev-item__name">
                    <strong>{e.generic ? e.model : `${e.brand} ${e.model}`}</strong>
                    <span className="hint">
                      {e.bed ? t('catalog.model.bed', { w: fmt(e.bed.value[0], 0), h: fmt(e.bed.value[1], 0) }) : e.generic ? kindLabel(e.kind) : t('catalog.model.bedUnknown')}
                    </span>
                  </span>
                  {r.byBanner && <span className="badge" data-tone="ok"><span className="badge__dot" />{t('catalog.model.recognized')}</span>}
                </label>
              </li>
            );
          })}
          <li>
            <label className="dev-item" data-checked={p.selectedId === OTHER_ID || undefined}>
              <input type="radio" name={`${id}-model`} checked={p.selectedId === OTHER_ID} onChange={() => p.onSelect(OTHER_ID)} />
              <span className="dev-item__name">
                <strong>{t('catalog.model.other')}</strong>
                <span className="hint">{t('catalog.model.otherHelp')}</span>
              </span>
            </label>
          </li>
        </ul>
      </div>
      <div className="dev-model__detail" aria-live="polite">
        {p.selectedId === OTHER_ID ? (
          <div className="field">
            <label htmlFor={`${id}-kind`} className="field__label">{t('catalog.model.otherKind')}</label>
            <select id={`${id}-kind`} className="select" value={p.otherKind} onChange={(e) => p.onOtherKind(e.target.value as DeviceKind)}>
              {DEVICE_KINDS.map((k) => (
                <option key={k} value={k}>{kindLabel(k)}</option>
              ))}
            </select>
          </div>
        ) : selected ? (
          <EntryDetail entry={selected} />
        ) : (
          <p className="hint">{t('catalog.model.pickOne')}</p>
        )}
      </div>
    </div>
  );
}

function factRow(label: I18nKey, f: Fact<unknown> | undefined, text: string) {
  if (!f) return null;
  return (
    <tr key={label}>
      <th scope="row">{t(label)}</th>
      <td className="num">{text}</td>
      <td>{t(`catalog.source.${f.kind}`)}</td>
      <td className="dev-url"><code>{f.source}</code>{f.note && <span className="hint">{t(f.note)}</span>}</td>
    </tr>
  );
}

const yesNo = (v: boolean) => t(v ? 'catalog.fact.yes' : 'catalog.fact.no');

/** Datos del catálogo de un modelo, cada uno con su fuente, y los que se desconocen. */
export function EntryDetail({ entry: e }: { entry: CatalogEntry }) {
  const rows = [
    factRow('catalog.fact.bed', e.bed, e.bed ? showValue(e.bed.value) + ' mm' : ''),
    factRow('catalog.fact.maxRate', e.maxRate, e.maxRate ? `${showValue(e.maxRate.value)} mm/min` : ''),
    factRow('catalog.fact.accel', e.accel, e.accel ? `${showValue(e.accel.value)} mm/s²` : ''),
    factRow('catalog.fact.sMax', e.sMax, e.sMax ? showValue(e.sMax.value) : ''),
    factRow('catalog.fact.m4', e.m4, e.m4 ? yesNo(e.m4.value) : ''),
    factRow('catalog.fact.air', e.air, e.air ? yesNo(e.air.value) : ''),
    factRow('catalog.fact.baud', e.baud, e.baud ? String(e.baud.value) : ''),
    factRow('catalog.fact.origin', e.origin, e.origin ? t(`catalog.origin.${e.origin.value}`) : ''),
    factRow('catalog.fact.homing', e.homing, e.homing ? yesNo(e.homing.value) : ''),
    factRow('catalog.fact.zAxis', e.zAxis, e.zAxis ? yesNo(e.zAxis.value) : ''),
    factRow('catalog.fact.detect', e.detect, e.detect ? e.detect.value.join(', ') : ''),
  ].filter(Boolean);
  const missing: I18nKey[] = [];
  if (!e.bed) missing.push('catalog.fact.bed');
  if (!e.maxRate) missing.push('catalog.fact.maxRate');
  if (!e.accel) missing.push('catalog.fact.accel');
  if (!e.sMax) missing.push('catalog.fact.sMax');
  return (
    <section aria-label={t('catalog.model.detail')}>
      <h3 className="mc-sub">{e.generic ? e.model : `${e.brand} ${e.model}`}</h3>
      <p className="hint">{t('catalog.fact.kind')}: {kindLabel(e.kind)}</p>
      {rows.length > 0 && (
        <table className="mc-table dev-facts">
          <tbody>{rows}</tbody>
        </table>
      )}
      {missing.length > 0 && (
        <p className="warn-line">
          <TriangleAlert size={14} aria-hidden="true" />
          <span>{t('catalog.model.unknownFields', { fields: missing.map((k) => t(k)).join(', ') })}</span>
        </p>
      )}
      {e.notes.length > 0 && (
        <ul className="dev-notes">
          {e.notes.map((n) => (
            <li key={n} className="hint">{t(n)}</li>
          ))}
        </ul>
      )}
      <p className="hint">{t('catalog.model.sourceNote')} ({t('catalog.model.sources')}: {entrySources(e).length})</p>
    </section>
  );
}

// ---------------------------------------------------------------------------
// 3. Revisar
// ---------------------------------------------------------------------------

export interface ReviewStepProps {
  comparison: Comparison | null;
  /** Campos en los que se usa lo que dice la máquina. */
  useMachine: ReadonlySet<CompareField>;
  onToggle: (field: CompareField, on: boolean) => void;
  /** Avisos de tipo de firmware (detectado frente al del modelo). */
  kindMismatch: { detected: DeviceKind; chosen: DeviceKind } | null;
  oldGrbl: boolean;
}

const STATUS_TONE: Record<CompareRow['status'], string> = { match: 'ok', differs: 'warn', catalogEmpty: 'accent', unread: 'off' };

export function ReviewStep(p: ReviewStepProps) {
  const c = p.comparison;
  return (
    <div className="dev-step">
      <p className="hint">{t('catalog.review.intro')}</p>
      {c && <LaserModeNotice mode={c.laserMode} />}
      {p.oldGrbl && <p className="warn-line"><TriangleAlert size={14} aria-hidden="true" /><span>{t('catalog.review.oldGrbl')}</span></p>}
      {p.kindMismatch && (
        <p className="warn-line">
          <TriangleAlert size={14} aria-hidden="true" />
          <span>{t('catalog.review.kindMismatch', { detected: kindLabel(p.kindMismatch.detected), chosen: kindLabel(p.kindMismatch.chosen) })}</span>
        </p>
      )}
      {!c || c.nothingRead ? (
        <p className="hint">{c ? t('catalog.review.nothingRead') : t('catalog.review.noSettings')}</p>
      ) : (
        <table className="mc-table dev-compare" aria-label={t('catalog.review.table')}>
          <thead>
            <tr>
              <th scope="col">{t('catalog.review.field')}</th>
              <th scope="col">{t('catalog.review.catalog')}</th>
              <th scope="col">{t('catalog.review.machine')}</th>
              <th scope="col">{t('catalog.review.status')}</th>
              <th scope="col">{t('catalog.review.useMachine')}</th>
            </tr>
          </thead>
          <tbody>
            {c.rows.map((r) => {
              const usable = r.machine !== null && r.status !== 'match';
              return (
                <tr key={r.field} data-status={r.status}>
                  <th scope="row">{t(FIELD_LABEL[r.field])} <span className="num mc-faint">{r.codes}</span></th>
                  <td className="num">{showValue(r.catalog)}</td>
                  <td className="num mc-new">{showValue(r.machine)}</td>
                  <td><span className="badge" data-tone={STATUS_TONE[r.status]}><span className="badge__dot" />{t(`catalog.review.status.${r.status}`)}</span></td>
                  <td>
                    <input
                      type="checkbox"
                      disabled={!usable}
                      checked={usable && p.useMachine.has(r.field)}
                      aria-label={t('catalog.review.useMachineFor', { field: t(FIELD_LABEL[r.field]) })}
                      onChange={(e) => p.onToggle(r.field, e.target.checked)}
                    />
                  </td>
                </tr>
              );
            })}
          </tbody>
        </table>
      )}
    </div>
  );
}

/** `$32`: en rojo si el modo láser no está activado. */
export function LaserModeNotice({ mode }: { mode: Comparison['laserMode'] }) {
  if (mode === 'off' || mode === 'other') {
    return (
      <p className="hint hint--danger dev-laser" role="alert">
        <TriangleAlert size={14} aria-hidden="true" /> {t(mode === 'off' ? 'catalog.review.laserOff' : 'catalog.review.laserOther')}
      </p>
    );
  }
  return <p className="hint">{t(mode === 'on' ? 'catalog.review.laserOn' : 'catalog.review.laserUnknown')}</p>;
}

// ---------------------------------------------------------------------------
// 4. Guardar
// ---------------------------------------------------------------------------

export interface SaveStepProps {
  name: string;
  onName: (name: string) => void;
  built: BuiltProfile;
  /** Solo con Ruida: la IP de la controladora, escrita a mano. */
  host?: string;
  onHost?: (host: string) => void;
  /** Solo con un K40: el modelo de placa. */
  board?: string;
  onBoard?: (board: string) => void;
}

const PROVENANCE_TEXT: Record<Provenance, I18nKey> = {
  machine: 'catalog.save.fromMachine',
  catalog: 'catalog.save.fromCatalog',
  default: 'catalog.save.fromDefault',
};

export function SaveStep({ name, onName, built, host = '', onHost, board = 'M2', onBoard }: SaveStepProps) {
  const id = useId();
  const { profile: d, provenance } = built;
  const empty = name.trim() === '';
  const rows: [ProfileField, I18nKey, string][] = [
    ['bed', 'catalog.field.bed', showValue(d.bed)],
    ['max_rate', 'catalog.field.max_rate', showValue(d.max_rate)],
    ['accel', 'catalog.field.accel', showValue(d.accel)],
    ['s_max', 'catalog.field.s_max', showValue(d.s_max)],
    ['baud', 'catalog.detect.baud', String(d.baud)],
    ['origin', 'catalog.fact.origin', t(`catalog.origin.${d.origin}`)],
  ].filter(([f]) => !((d.kind === 'ruida' || d.kind === 'lhy_micro') && (f === 'baud' || f === 's_max'))) as [ProfileField, I18nKey, string][];
  const hasDefaults = rows.some(([f]) => provenance[f] === 'default');
  return (
    <div className="dev-step">
      <div className="field">
        <label htmlFor={`${id}-name`} className="field__label">{t('catalog.save.name')}</label>
        <input id={`${id}-name`} className="input" value={name} aria-invalid={empty || undefined} aria-describedby={`${id}-help`} onChange={(e) => onName(e.target.value)} />
        <p id={`${id}-help`} className={empty ? 'hint hint--danger' : 'hint'} role={empty ? 'alert' : undefined}>
          {empty ? t('catalog.save.nameRequired') : t('catalog.save.nameHelp')}
        </p>
      </div>
      {d.kind === 'ruida' && onHost && <RuidaHostField host={host} onHost={onHost} />}
      {d.kind === 'lhy_micro' && onBoard && <LhyBoardField board={board} onBoard={onBoard} />}
      <section aria-label={t('catalog.save.summary')}>
        <h3 className="mc-sub">{t('catalog.save.summary')}</h3>
        <p className="hint">{t('catalog.save.kind')}: {kindLabel(d.kind)}</p>
        <table className="mc-table">
          <thead>
            <tr>
              <th scope="col">{t('catalog.review.field')}</th>
              <th scope="col">{t('catalog.save.value')}</th>
              <th scope="col">{t('catalog.save.from')}</th>
            </tr>
          </thead>
          <tbody>
            {rows.map(([field, label, value]) => (
              <tr key={field} data-from={provenance[field]}>
                <th scope="row">{t(label)}</th>
                <td className="num">{value}</td>
                <td className={provenance[field] === 'default' ? 'dev-default' : undefined}>{t(PROVENANCE_TEXT[provenance[field]])}</td>
              </tr>
            ))}
          </tbody>
        </table>
      </section>
      {hasDefaults && <p className="warn-line"><TriangleAlert size={14} aria-hidden="true" /><span>{t('catalog.save.defaultsWarn')}</span></p>}
      <p className="hint">{t('catalog.save.activeNote')}</p>
    </div>
  );
}
