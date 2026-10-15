import { TriangleAlert, X } from 'lucide-react';
import { useEffect, useId, useMemo, useRef, useState } from 'react';
import type { CsvTable } from '@/api/types';
import { variableTextOf, editVariableText } from './settings';
import { getLanguage, t } from '@/i18n';
import { useStore } from '@/store';
import { Modal } from '@/ui/Modal';
import { NumberField } from '@/ui/NumberField';
import { atCsvEnd, variableTexts } from './advance';
import { displayContext, useVariablesUi } from './context';
import { makeCopiesPerRow, MAX_COPIES } from './copies';
import { DataCheck } from './DataCheck';
import { formatSerial } from './format';
import { issueMessage } from './messages';
import { resolveTemplate } from './resolve';
import { SourcesPanel, useSourceStates } from './SourcesPanel';
import { objectTemplates } from './varObjects';
import './variables.css';

/** Filas que se dibujan a la vez en la tabla (un CSV grande no debe congelar el diálogo). */
export const TABLE_WINDOW = 100;

/** Primera fila que se dibuja para que la actual quede a la vista. */
export function windowStart(current: number, total: number, size = TABLE_WINDOW): number {
  return Math.max(0, Math.min(current - 4, total - size));
}

/** Diálogo "Texto variable": serie, CSV, fila actual y avance. */
export function VariablesDialog() {
  const titleId = useId();
  const close = useVariablesUi((s) => s.close);
  return (
    <Modal labelId={titleId} onClose={close} portal closeOnBackdrop fallbackFocus="card" backdropClassName="vt-backdrop" cardClassName="vt-modal">
      <VariablesContent titleId={titleId} />
    </Modal>
  );
}

/** Contenido del diálogo (exportado para pruebas: el diálogo entero va en un portal). */
export function VariablesContent({ titleId }: { titleId: string }) {
  const close = useVariablesUi((s) => s.close);
  const csv = useVariablesUi((s) => s.csv);
  const showTemplate = useVariablesUi((s) => s.showTemplate);
  const setShowTemplate = useVariablesUi((s) => s.setShowTemplate);
  const doc = useStore((s) => s.doc);
  const vt = variableTextOf(doc);
  const states = useSourceStates(vt.sources);
  const rows = csv.table && csv.path === vt.csv_path ? csv.table.rows.length : null;
  const end = atCsvEnd(vt, rows);
  const serialPreview = formatSerial(Number(vt.serial), vt.serial_digits);
  // La tabla que se ve: la fuente elegida (por omisión, la principal).
  const [viewIndex, setViewIndex] = useState(0);
  const shown = Math.min(viewIndex, Math.max(0, vt.sources.length - 1));
  const shownTable = states[shown]?.table ?? null;
  // En una fuente secundaria más corta, la fila que se resalta es la que le toca (se repite).
  const highlight = shown > 0 && shownTable && shownTable.rows.length > 0 ? vt.csv_row % shownTable.rows.length : vt.csv_row;

  const goRow = (row: number) => editVariableText({ csv_row: Math.max(0, rows === null ? row : Math.min(row, rows)) }, 'row');

  return (
    <>
      <header className="vt-head">
        <h2 id={titleId} className="vt-title">{t('variables.title')}</h2>
        <button type="button" className="icon-btn" title={t('variables.close')} aria-label={t('variables.close')} onClick={close}>
          <X size={16} aria-hidden="true" />
        </button>
      </header>

      <div className="vt-body">
        <div className="vt-side">
          <fieldset className="vt-group">
            <legend className="vt-legend">{t('variables.serial.title')}</legend>
            <div className="vt-grid">
              <NumberField label={t('variables.serial.next')} value={Number(vt.serial)} step={1} onCommit={(v) => editVariableText({ serial: Math.trunc(v) }, 'serial')} />
              <NumberField label={t('variables.serial.step')} value={Number(vt.serial_step)} step={1} onCommit={(v) => editVariableText({ serial_step: Math.trunc(v) }, 'step')} />
              <NumberField label={t('variables.serial.digits')} value={vt.serial_digits} min={0} max={20} step={1} onCommit={(v) => editVariableText({ serial_digits: Math.trunc(v) }, 'digits')} />
            </div>
            <p className="hint">{t('variables.serial.preview', { value: serialPreview })}</p>
            <div className="vt-row">
              <button type="button" className="btn" onClick={() => editVariableText({ serial: Number(vt.serial) - Number(vt.serial_step) }, 'serial')}>
                {t('variables.serial.prev')}
              </button>
              <button type="button" className="btn" onClick={() => editVariableText({ serial: Number(vt.serial) + Number(vt.serial_step) }, 'serial')}>
                {t('variables.serial.nextBtn')}
              </button>
            </div>
          </fieldset>

          <SourcesPanel />

          <fieldset className="vt-group">
            <legend className="vt-legend">{t('variables.row.title')}</legend>
            <p className="hint">{t('variables.row.hint')}</p>
            {vt.csv_path && csv.table && (
              <>
                <p className="hint num" role="status">{t('variables.row.current', { row: Math.min(vt.csv_row + 1, csv.table.rows.length), total: csv.table.rows.length })}</p>
                <div className="vt-row">
                  <button type="button" className="btn" disabled={vt.csv_row <= 0} onClick={() => goRow(vt.csv_row - 1)}>{t('variables.row.prev')}</button>
                  <button type="button" className="btn" disabled={rows !== null && vt.csv_row >= rows} onClick={() => goRow(vt.csv_row + 1)}>{t('variables.row.next')}</button>
                  <button type="button" className="btn" disabled={vt.csv_row === 0} onClick={() => goRow(0)}>{t('variables.row.first')}</button>
                </div>
                <NumberField
                  label={t('variables.row.goto', { total: csv.table.rows.length })}
                  value={vt.csv_row + 1}
                  min={1}
                  max={csv.table.rows.length}
                  step={1}
                  onCommit={(v) => goRow(Math.trunc(v) - 1)}
                />
              </>
            )}
            {end && (
              <p className="warn-line" role="alert">
                <TriangleAlert size={14} aria-hidden="true" />
                <span>{t('variables.csv.end')}</span>
              </p>
            )}
          </fieldset>

          <fieldset className="vt-group">
            <legend className="vt-legend">{t('variables.advance.title')}</legend>
            <label className="vt-check">
              <input type="checkbox" checked={vt.advance_after_job} onChange={(e) => editVariableText({ advance_after_job: e.target.checked }, 'advance')} />
              {t('variables.advance.auto')}
            </label>
            <p className="hint">{t('variables.advance.hint')}</p>
            <label className="vt-check">
              <input type="checkbox" checked={showTemplate} onChange={(e) => setShowTemplate(e.target.checked)} />
              {t('variables.canvas.template')}
            </label>
            <p className="hint">{t('variables.canvas.templateHint')}</p>
          </fieldset>
        </div>

        <div className="vt-main">
          {vt.sources.length > 1 && (
            <label className="vt-viewsel">
              <span>{t('variables.table.source')}</span>
              <select className="select" value={shown} onChange={(e) => setViewIndex(Number(e.target.value))}>
                {vt.sources.map((s, i) => (
                  <option key={s.name} value={i}>{s.name}</option>
                ))}
              </select>
            </label>
          )}
          <CsvTableView table={shownTable} current={highlight} onPick={shown === 0 ? goRow : () => undefined} />
          <TextsPreview />
          <DataCheck onPickRow={goRow} />
          <CopiesPerRow rows={rows} />
        </div>
      </div>

      <footer className="vt-foot">
        <button type="button" className="btn" onClick={close}>{t('variables.done')}</button>
      </footer>
    </>
  );
}

/** Tabla con las filas del CSV y la actual resaltada; un clic en una fila la hace la actual. */
export function CsvTableView({ table, current, onPick }: { table: CsvTable | null; current: number; onPick: (row: number) => void }) {
  const currentRef = useRef<HTMLTableRowElement>(null);
  useEffect(() => {
    currentRef.current?.scrollIntoView?.({ block: 'nearest' });
  }, [current, table]);
  if (!table) return <p className="hint vt-empty">{t('variables.table.empty')}</p>;
  const start = windowStart(current, table.rows.length);
  const shown = table.rows.slice(start, start + TABLE_WINDOW);
  return (
    <div className="vt-tablebox">
      <div className="vt-scroll" role="region" aria-label={t('variables.table.label')} tabIndex={0}>
        <table className="vt-table" aria-label={t('a11y.table.variables')}>
          <thead>
            <tr>
              <th scope="col" className="num">#</th>
              {table.headers.map((h, i) => (
                <th key={i} scope="col">{h === '' ? i + 1 : h}</th>
              ))}
            </tr>
          </thead>
          <tbody>
            {shown.map((row, i) => {
              const index = start + i;
              const isCurrent = index === current;
              return (
                <tr
                  key={index}
                  ref={isCurrent ? currentRef : undefined}
                  className={isCurrent ? 'vt-current' : undefined}
                  aria-current={isCurrent ? 'true' : undefined}
                  onClick={() => onPick(index)}
                >
                  <td className="num">{index + 1}</td>
                  {table.headers.map((_, c) => (
                    <td key={c}>{row[c] ?? ''}</td>
                  ))}
                </tr>
              );
            })}
          </tbody>
        </table>
      </div>
      {table.rows.length > TABLE_WINDOW && (
        <p className="hint">{t('variables.table.window', { from: start + 1, to: start + shown.length, total: table.rows.length })}</p>
      )}
    </div>
  );
}

/** Los objetos con variables del diseño (textos, imágenes y códigos), cada uno con lo que se grabaría ahora y sus problemas. */
function TextsPreview() {
  const doc = useStore((s) => s.doc);
  // Se recalcula con los CSV y el reloj de la interfaz.
  const csvState = useVariablesUi((s) => s.csv);
  const extra = useVariablesUi((s) => s.extra);
  const clock = useVariablesUi((s) => s.clock);
  const items = useMemo(() => {
    const ctx = displayContext(doc);
    return variableTexts(doc).map((o) => {
      const template = objectTemplates(o)[0] ?? '';
      const kind: 'text' | 'image' | 'code' = o.kind.type === 'image' ? 'image' : o.kind.type === 'vector' ? 'code' : 'text';
      return { id: o.id, template, kind, ...resolveTemplate(template, ctx) };
    });
    // `csvState`, `extra` y `clock` no se leen aquí, pero `displayContext` los toma del estado de la interfaz.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [doc, csvState, extra, clock]);
  return (
    <section className="vt-texts" aria-label={t('variables.texts.title')}>
      <h3 className="vt-sub">{t('variables.texts.title')}</h3>
      {items.length === 0 ? (
        <p className="hint">{t('variables.texts.none', { example: `{${getLanguage() === 'es' ? 'serie' : 'serial'}}` })}</p>
      ) : (
        <ul className="vt-list">
          {items.map((it) => (
            <li key={it.id}>
              {it.kind !== 'text' && <span className="vt-kind">{t(it.kind === 'image' ? 'variables.texts.kindImage' : 'variables.texts.kindCode')}</span>}
              <code className="vt-code">{it.template.replace(/\n/g, ' ⏎ ')}</code>
              <span aria-hidden="true"> → </span>
              <strong className="vt-value">{it.text.replace(/\n/g, ' ⏎ ')}</strong>
              {it.issues.map((issue, i) => (
                <p key={i} className="warn-line">
                  <TriangleAlert size={14} aria-hidden="true" />
                  <span>{issueMessage(issue)}</span>
                </p>
              ))}
            </li>
          ))}
        </ul>
      )}
    </section>
  );
}

/** "Una copia por fila": copias de la selección en rejilla, cada una con una fila distinta del CSV. */
function CopiesPerRow({ rows }: { rows: number | null }) {
  const selected = useStore((s) => s.selectedIds);
  const row = useStore((s) => variableTextOf(s.doc).csv_row);
  const remaining = rows === null ? 1 : Math.max(1, rows - row);
  const [count, setCount] = useState<number | null>(null);
  const [cols, setCols] = useState<number | null>(null);
  const [gap, setGap] = useState(5);
  const n = Math.min(MAX_COPIES, count ?? remaining);
  const c = Math.min(n, cols ?? Math.max(1, Math.ceil(Math.sqrt(n))));
  const blocked = selected.length === 0 ? t('variables.copies.noSelection') : n < 2 ? t('variables.copies.few') : null;

  return (
    <section className="vt-copies" aria-label={t('variables.copies.title')}>
      <h3 className="vt-sub">{t('variables.copies.title')}</h3>
      <p className="hint">{t('variables.copies.hint')}</p>
      <div className="vt-grid">
        <NumberField label={t('variables.copies.count')} value={n} min={1} max={MAX_COPIES} step={1} onCommit={(v) => setCount(Math.trunc(v))} />
        <NumberField label={t('variables.copies.cols')} value={c} min={1} max={MAX_COPIES} step={1} onCommit={(v) => setCols(Math.trunc(v))} />
        <NumberField label={t('variables.copies.gap')} unit="mm" value={gap} min={0} max={1000} step={1} onCommit={setGap} />
      </div>
      <div className="vt-row">
        <button
          type="button"
          className="btn btn--accent"
          aria-disabled={blocked !== null}
          title={blocked ?? t('variables.copies.makeTitle')}
          onClick={() => {
            if (blocked) return;
            const made = makeCopiesPerRow({ count: n, cols: c, gap });
            useStore.getState().notify('success', t('variables.copies.done', { n: made }));
          }}
        >
          {t('variables.copies.make')}
        </button>
        {blocked && <span className="hint">{blocked}</span>}
      </div>
    </section>
  );
}
