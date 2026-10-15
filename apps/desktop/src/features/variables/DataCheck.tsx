import { CircleCheck, TriangleAlert } from 'lucide-react';
import { useState } from 'react';
import { getApi } from '@/api';
import { t, tn } from '@/i18n';
import { useStore } from '@/store';
import { checkData, MAX_LISTED_ROWS, type DataCheck as DataCheckResult } from './check';
import { extraTables, useVariablesUi } from './context';
import { variableTextOf } from './defaults';
import { variableTexts } from './advance';

/**
 * «Revisar datos»: recorre todas las filas del CSV con lo que usa el diseño y lista las que tienen
 * datos que faltan (celdas vacías, imágenes que no existen, códigos sin contenido...). Un clic en
 * una fila de la lista la deja como fila actual para ver qué se grabaría.
 */
export function DataCheck({ onPickRow }: { onPickRow: (row: number) => void }) {
  const [state, setState] = useState<{ status: 'idle' | 'running' | 'done' | 'error'; result?: DataCheckResult; error?: string }>({ status: 'idle' });
  const hasWork = useStore((s) => variableTexts(s.doc).length > 0);
  const main = useVariablesUi((s) => s.csv);

  const run = async () => {
    setState({ status: 'running' });
    try {
      const doc = useStore.getState().doc;
      const path = variableTextOf(doc).csv_path;
      const table = path !== null && main.path === path ? main.table : null;
      const result = await checkData(doc, { main: table, extra: extraTables(doc) }, getApi());
      setState({ status: 'done', result });
    } catch (e) {
      setState({ status: 'error', error: String(e instanceof Error ? e.message : e) });
    }
  };

  const r = state.result;
  const clean = r !== undefined && r.problemRows === 0 && r.general.length === 0;
  return (
    <section className="vt-check-data" aria-label={t('variables.check.title')}>
      <h3 className="vt-sub">{t('variables.check.title')}</h3>
      <p className="hint">{t('variables.check.hint')}</p>
      <div className="vt-row">
        <button type="button" className="btn" disabled={!hasWork || !main.table || state.status === 'running'} onClick={() => void run()}>
          {state.status === 'running' ? t('variables.check.running') : t('variables.check.run')}
        </button>
        {!hasWork && <span className="hint">{t('variables.check.nothing')}</span>}
      </div>
      {state.status === 'error' && (
        <p className="warn-line" role="alert">
          <TriangleAlert size={14} aria-hidden="true" />
          <span>{t('variables.check.failed', { message: state.error ?? '' })}</span>
        </p>
      )}
      {clean && (
        <p className="ok-line" role="status">
          <CircleCheck size={14} aria-hidden="true" />
          <span>{tn('variables.check.clean', r.rows, { count: r.rows })}</span>
        </p>
      )}
      {r && r.general.length > 0 && (
        <ul className="vt-list">
          {r.general.map((m, i) => (
            <li key={i} className="warn-line" role="alert">
              <TriangleAlert size={14} aria-hidden="true" />
              <span>{m}</span>
            </li>
          ))}
        </ul>
      )}
      {r && r.problemRows > 0 && (
        <>
          <p className="warn-line" role="status">
            <TriangleAlert size={14} aria-hidden="true" />
            <span>{tn('variables.check.summary', r.problemRows, { count: r.problemRows, rows: r.rows })}</span>
          </p>
          <ul className="vt-list vt-check-list" aria-label={t('variables.check.listLabel')}>
            {r.problems.map((p) => (
              <li key={p.row}>
                <button type="button" className="vt-linkbtn" onClick={() => onPickRow(p.row - 1)} title={t('variables.check.goto', { row: p.row })}>
                  {t('variables.check.row', { row: p.row })}
                </button>
                <span> {p.messages.join(' ')}</span>
              </li>
            ))}
          </ul>
          {r.problemRows > MAX_LISTED_ROWS && <p className="hint">{t('variables.check.more', { shown: MAX_LISTED_ROWS, total: r.problemRows })}</p>}
        </>
      )}
    </section>
  );
}
