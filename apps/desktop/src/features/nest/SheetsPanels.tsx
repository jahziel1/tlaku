import { useId } from 'react';
import { Check } from '@/features/arrange/Dialog';
import { t, tn } from '@/i18n';
import { fmt } from '@/lib/platform';
import { NumberField } from '@/ui/NumberField';
import { setRemnantsPanel } from './remnantStore';
import { MAX_NEST_SHEETS, NEST_REMNANT_MIN_RANGE, type NestOptions } from './request';
import type { Remnant } from './remnants';
import type { NestSheet, NestStatus } from './types';

interface SectionProps {
  options: NestOptions;
  patch: (patch: Partial<NestOptions>) => void;
  /** Retazos del inventario que sirven para el material y grosor elegidos. */
  matching: readonly Remnant[];
  /** Materiales que ya hay en el inventario (para sugerirlos). */
  materials: readonly string[];
}

/** Sección «Hojas y retazos» del diálogo: más hojas, retazos guardados, material y grosor. */
export function SheetsSection({ options, patch, matching, materials }: SectionProps) {
  const id = useId();
  const hint =
    options.material.trim() === '' ? t('nest.sheets.remnantsNeedMaterial')
    : matching.length === 0 ? t('nest.sheets.remnantsNone')
    : tn('nest.sheets.remnantsMatch', matching.length, { n: matching.length });
  return (
    <section className="ne-section" aria-labelledby={`${id}-h`}>
      <h3 id={`${id}-h`} className="ne-heading">{t('nest.sheets.title')}</h3>
      <Check label={t('nest.sheets.multi')} checked={options.multiSheet} onChange={(multiSheet) => patch({ multiSheet })} />
      {options.multiSheet && (
        <NumberField label={t('nest.sheets.max')} value={options.maxSheets} min={1} max={MAX_NEST_SHEETS} step={1} onCommit={(maxSheets) => patch({ maxSheets: Math.round(maxSheets) })} />
      )}
      <div className="field-grid">
        <div className="field">
          <label htmlFor={`${id}-material`} className="field__label">{t('nest.sheets.material')}</label>
          <input
            id={`${id}-material`}
            className="input"
            list={`${id}-materials`}
            value={options.material}
            placeholder={t('nest.sheets.material.placeholder')}
            onChange={(e) => patch({ material: e.target.value })}
          />
          <datalist id={`${id}-materials`}>{materials.map((m) => <option key={m} value={m} />)}</datalist>
        </div>
        <NumberField label={t('nest.sheets.thickness')} value={options.thicknessMm} min={0} max={200} step={0.5} unit={t('nest.unit.mm')} onCommit={(thicknessMm) => patch({ thicknessMm })} />
      </div>
      <Check label={t('nest.sheets.useRemnants')} checked={options.useRemnants} onChange={(useRemnants) => patch({ useRemnants })} />
      <p className="hint num" role="status">{hint}</p>
      <NumberField
        label={t('nest.sheets.minRemnant')}
        value={options.remnantMinMm}
        min={NEST_REMNANT_MIN_RANGE[0]}
        max={NEST_REMNANT_MIN_RANGE[1]}
        step={5}
        unit={t('nest.unit.mm')}
        onCommit={(remnantMinMm) => patch({ remnantMinMm })}
      />
      <button type="button" className="btn ne-link" onClick={() => setRemnantsPanel(true)}>{t('nest.remnants.open')}</button>
    </section>
  );
}

/** Medidas de un sobrante («420×180») por la caja de su contorno exterior. */
export function leftoverSize(rings: readonly (readonly [number, number][])[]): string {
  const outer = rings[0] ?? [];
  const xs = outer.map((p) => p[0]);
  const ys = outer.map((p) => p[1]);
  return `${Math.round(Math.max(...xs) - Math.min(...xs))}×${Math.round(Math.max(...ys) - Math.min(...ys))}`;
}

interface ResultProps {
  status: NestStatus;
  /** Nombre de un retazo usado como hoja. */
  remnantName: (id: string) => string;
  /** Hojas cuyo sobrante ya se guardó. */
  saved: ReadonlySet<number>;
  onSave: (sheet: NestSheet) => void;
  /** Nada se guarda mientras corre el cálculo. */
  disabled: boolean;
}

/** Resumen por hoja: aprovechamiento, piezas y el sobrante que se puede guardar como retazo. */
export function SheetsResult({ status, remnantName, saved, onSave, disabled }: ResultProps) {
  if (status.sheets.length === 0) return null;
  const multi = status.sheets.length > 1;
  return (
    <ul className="ne-sheets" aria-label={t('nest.sheets.list')}>
      {status.sheets.map((s) => {
        const left = s.leftovers;
        const what = s.source === 'remnant' ? t('nest.sheets.fromRemnant', { name: s.remnant_id ? remnantName(s.remnant_id) : '' }) : t('nest.sheets.fromSheet');
        return (
          <li key={s.index} className="ne-sheetrow">
            <p className="ne-sheetrow__head num">
              <strong>{t('nest.sheets.row', { n: s.index + 1 })}</strong>
              <span>{what}</span>
              <span>{t('nest.progress.pct', { pct: fmt(Math.round(s.utilization * 10) / 10) })}</span>
              <span>{tn('nest.sheets.pieces', s.placed, { n: s.placed })}</span>
            </p>
            {!status.running && left.length > 0 && (
              <p className="ne-sheetrow__left">
                <span className="hint num">
                  {tn('nest.sheets.leftover', left.length, { n: left.length, list: left.slice(0, 3).map(leftoverSize).join(', ') + (left.length > 3 ? '…' : '') })}
                </span>
                <button type="button" className="btn" aria-disabled={disabled || saved.has(s.index)} onClick={() => !disabled && !saved.has(s.index) && onSave(s)}>
                  {saved.has(s.index) ? t('nest.sheets.saved') : tn('nest.sheets.save', left.length, { n: left.length })}
                </button>
              </p>
            )}
          </li>
        );
      })}
      {!multi && status.sheets[0]!.leftovers.length === 0 && !status.running && <li className="hint">{t('nest.sheets.noLeftover')}</li>}
    </ul>
  );
}
