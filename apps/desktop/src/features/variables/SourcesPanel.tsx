import { ArrowUpToLine, RefreshCw, Trash2, TriangleAlert } from 'lucide-react';
import { useId } from 'react';
import type { CsvTable, DataSource } from '@/api/types';
import { t } from '@/i18n';
import { pinTab, useStore } from '@/store';
import { useVariablesUi, type CsvState } from './context';
import { loadCsv, loadExtraCsv, pickCsvFile } from './csvSync';
import { variableTextOf } from './defaults';
import { rowFits, type RowFit } from './sources';
import { addDataSource, makeMainSource, removeDataSource, renameDataSource, replaceDataSource, setSourceOnShort } from './settings';

function baseName(path: string): string {
  return path.split(/[\\/]/).pop() ?? path;
}

/** Estado de lectura de cada fuente, en el orden de `sources` (la principal primero). */
export function useSourceStates(sources: readonly DataSource[]): (CsvState | null)[] {
  const main = useVariablesUi((s) => s.csv);
  const extra = useVariablesUi((s) => s.extra);
  return sources.map((s, i) => {
    const st = i === 0 ? main : extra[s.name.toLowerCase()];
    return st && st.path === s.path ? st : null;
  });
}

/** Tablas ya leídas de cada fuente (o `null`). */
export function tablesOf(states: readonly (CsvState | null)[]): (CsvTable | null)[] {
  return states.map((s) => s?.table ?? null);
}

function fitMessage(name: string, fit: RowFit | null): string | null {
  if (!fit || fit.kind === 'same') return null;
  switch (fit.kind) {
    case 'repeat':
      return t('variables.sources.fitRepeat', { name, rows: fit.rows, main: fit.main });
    case 'block':
      return t('variables.sources.fitBlock', { name, rows: fit.rows, main: fit.main });
    case 'extra':
      return t('variables.sources.fitExtra', { name, rows: fit.rows, main: fit.main, extra: fit.rows - fit.main });
    case 'empty':
      return t('variables.sources.fitEmpty', { name });
  }
}

/** Lista de fuentes de datos (CSV) del proyecto: agregar, quitar, nombrar y elegir la principal. */
export function SourcesPanel() {
  const doc = useStore((s) => s.doc);
  const vt = variableTextOf(doc);
  const states = useSourceStates(vt.sources);
  const fits = rowFits(vt.sources, tablesOf(states));
  const listId = useId();

  const pick = async (replaceIndex: number | null) => {
    // El archivo elegido es de la pestaña donde se abrió el selector, aunque el usuario cambie de pestaña mientras tanto.
    const inTab = pinTab(useStore.getState());
    try {
      const path = await pickCsvFile();
      if (!path) return;
      const applied = inTab(() => {
        if (replaceIndex === null) addDataSource(path);
        else replaceDataSource(replaceIndex, path);
        return true;
      });
      if (applied === undefined) useStore.getState().notify('warning', t('tabs.targetClosedGeneric'));
    } catch (e) {
      useStore.getState().notify('error', t('variables.csv.pickFailed', { message: String(e instanceof Error ? e.message : e) }));
    }
  };
  const reload = (i: number) => (i === 0 ? void loadCsv(vt.sources[0]?.path ?? null) : void loadExtraCsv(vt.sources[i]!.name.toLowerCase(), vt.sources[i]!.path));

  return (
    <fieldset className="vt-group">
      <legend className="vt-legend">{t('variables.sources.title')}</legend>
      <p className="hint">{t('variables.sources.hint')}</p>
      {vt.sources.length === 0 ? (
        <p className="vt-path">{t('variables.csv.none')}</p>
      ) : (
        <ul className="vt-sources" id={listId} aria-label={t('variables.sources.title')}>
          {vt.sources.map((s, i) => (
            <SourceItem key={`${i}`} source={s} index={i} state={states[i] ?? null} fit={fits[i] ?? null} onReplace={() => void pick(i)} onReload={() => reload(i)} />
          ))}
        </ul>
      )}
      <div className="vt-row">
        <button type="button" className="btn" onClick={() => void pick(null)}>
          {vt.sources.length === 0 ? t('variables.csv.choose') : t('variables.sources.add')}
        </button>
      </div>
    </fieldset>
  );
}

function SourceItem({ source, index, state, fit, onReplace, onReload }: { source: DataSource; index: number; state: CsvState | null; fit: RowFit | null; onReplace: () => void; onReload: () => void }) {
  const nameId = useId();
  const shortId = useId();
  const note = fitMessage(source.name, fit);
  const isMain = index === 0;
  return (
    <li className="vt-source">
      <div className="vt-source__head">
        <label className="vt-source__name" htmlFor={nameId}>
          <span className="vt-source__label">{isMain ? t('variables.sources.main') : t('variables.sources.name')}</span>
          <input
            id={nameId}
            className="input"
            defaultValue={source.name}
            key={source.name}
            spellCheck={false}
            onBlur={(e) => {
              if (e.target.value.trim() !== source.name) renameDataSource(index, e.target.value);
            }}
            onKeyDown={(e) => {
              if (e.key === 'Enter') e.currentTarget.blur();
            }}
          />
        </label>
        <span className="vt-source__tag" title={t('variables.sources.tagHint', { name: source.name })}>{`{csv${isMain ? '' : `.${source.name}`}:…}`}</span>
      </div>
      <p className="vt-path" title={source.path}>{baseName(source.path)}</p>
      {state?.loading && <p className="hint" role="status">{t('variables.csv.loading')}</p>}
      {state?.error && (
        <p className="warn-line" role="alert">
          <TriangleAlert size={14} aria-hidden="true" />
          <span>{t('variables.csv.error', { message: state.error })}</span>
        </p>
      )}
      {state?.table && (
        <p className="hint num">
          {t('variables.csv.summary', { cols: state.table.headers.length, rows: state.table.rows.length, sep: state.table.delimiter === '\t' ? t('variables.csv.sepTab') : state.table.delimiter })}
        </p>
      )}
      {note && (
        <p className={fit?.kind === 'extra' ? 'hint' : 'warn-line'} role={fit?.kind === 'extra' ? undefined : 'status'}>
          {fit?.kind !== 'extra' && <TriangleAlert size={14} aria-hidden="true" />}
          <span>{note}</span>
        </p>
      )}
      {!isMain && (
        <div className="vt-source__short">
          <label htmlFor={shortId} className="vt-source__label">{t('variables.sources.onShort')}</label>
          <select id={shortId} className="select" value={source.on_short} onChange={(e) => setSourceOnShort(index, e.target.value as DataSource['on_short'])}>
            <option value="repeat">{t('variables.sources.onShort.repeat')}</option>
            <option value="block">{t('variables.sources.onShort.block')}</option>
          </select>
        </div>
      )}
      <div className="vt-row">
        <button type="button" className="btn" onClick={onReplace}>{t('variables.sources.replace')}</button>
        <button type="button" className="icon-btn" title={t('variables.csv.reload')} aria-label={t('variables.csv.reload')} onClick={onReload}>
          <RefreshCw size={14} aria-hidden="true" />
        </button>
        {!isMain && (
          <button type="button" className="icon-btn" title={t('variables.sources.makeMain')} aria-label={t('variables.sources.makeMain')} onClick={() => makeMainSource(index)}>
            <ArrowUpToLine size={14} aria-hidden="true" />
          </button>
        )}
        <button type="button" className="icon-btn" title={t('variables.sources.remove')} aria-label={t('variables.sources.remove')} onClick={() => removeDataSource(index)}>
          <Trash2 size={14} aria-hidden="true" />
        </button>
      </div>
    </li>
  );
}
