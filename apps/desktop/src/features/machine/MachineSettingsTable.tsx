import { Pencil, TriangleAlert } from 'lucide-react';
import { t, type I18nKey } from '@/i18n';
import type { FirmwareFamily } from './grblCodes';
import { settingDescKey, settingInfo, settingNameKey, type SettingUnit } from './grblSettingsInfo';
import { validateSettingValue, validationParams, type RawSetting } from './MachineSettingsLogic';
import './MachineSettings.css';

/** Texto de la unidad: los símbolos (mm, µs...) van tal cual; los demás se traducen. */
export function unitLabel(unit: SettingUnit | undefined): string {
  if (unit === undefined) return '';
  if (unit === 'steps/mm') return t('mset.unit.stepsmm');
  if (unit === 'mask') return t('mset.unit.mask');
  if (unit === 'bool') return t('mset.unit.bool');
  return unit;
}

/** Etiquetas visibles de un ajuste: «Crítico» y «Solo grblHAL». Nunca solo con color. */
export function SettingTags({ id }: { id: number }) {
  const info = settingInfo(id);
  return (
    <>
      {info?.critical && (
        <span className="mset-tag" data-tone="critical" title={t('mset.critical.help')}>
          <TriangleAlert size={11} aria-hidden="true" />
          {t('mset.critical')}
        </span>
      )}
      {info?.halOnly && <span className="mset-tag" data-tone="hal">{t('mset.halOnly')}</span>}
    </>
  );
}

/** ¿Coincide el ajuste con lo buscado? Por número (`30`, `$30`) o por nombre. */
export function matchesFilter(id: number, filter: string): boolean {
  const q = filter.trim().toLowerCase().replace(/^\$/, '');
  if (q === '') return true;
  return String(id).startsWith(q) || t(settingNameKey(id)).toLowerCase().includes(q);
}

export interface EditingState {
  id: number;
  text: string;
}

interface Props {
  settings: readonly RawSetting[];
  family: FirmwareFamily;
  filter: string;
  /** Por qué no se puede editar ahora (sin conexión o con un trabajo); `null` si se puede. */
  blocked: I18nKey | null;
  editing: EditingState | null;
  onEditStart: (id: number) => void;
  onEditChange: (text: string) => void;
  onEditCancel: () => void;
  onReview: () => void;
}

/** Tabla de ajustes: número, nombre y descripción, valor actual, unidad y editor. */
export function MachineSettingsTable({ settings, family, filter, blocked, editing, onEditStart, onEditChange, onEditCancel, onReview }: Props) {
  const rows = settings.filter((s) => matchesFilter(s.id, filter));
  if (rows.length === 0) return <p className="hint" role="status">{t('mset.table.none')}</p>;
  return (
    <table className="mc-table mset-table" aria-label={t('mset.table')}>
      <thead>
        <tr>
          <th scope="col">{t('mset.col.id')}</th>
          <th scope="col">{t('mset.col.name')}</th>
          <th scope="col">{t('mset.col.value')}</th>
          <th scope="col">{t('mset.col.unit')}</th>
          <th scope="col"><span className="sr-only">{t('mset.col.action')}</span></th>
        </tr>
      </thead>
      <tbody>
        {rows.map((s) => {
          const info = settingInfo(s.id);
          const isEditing = editing?.id === s.id;
          const verdict = isEditing ? validateSettingValue(s.id, editing.text, family) : null;
          const errorId = `mset-err-${s.id}`;
          const error = verdict && !verdict.ok ? t(verdict.error.key, validationParams(verdict.error)) : null;
          const readOnly = s.value === null;
          const why = blocked ? t(blocked) : readOnly ? t('mset.row.noEdit') : undefined;
          return (
            <tr key={s.id} data-critical={info?.critical ? '' : undefined} data-editing={isEditing ? '' : undefined}>
              <th scope="row" className="num mset-id">{`$${s.id}`}</th>
              <td>
                <div className="mset-name">
                  <span>{t(settingNameKey(s.id))}</span>
                  <SettingTags id={s.id} />
                </div>
                <div className="hint mset-desc">{t(settingDescKey(s.id))}</div>
              </td>
              <td className="num mset-value">{s.raw}</td>
              <td className="mset-unit">{unitLabel(info?.unit)}</td>
              <td className="mset-act">
                {isEditing ? (
                  <div className="mset-edit">
                    <input
                      className="input num"
                      type="text"
                      inputMode="decimal"
                      autoComplete="off"
                      spellCheck={false}
                      value={editing.text}
                      data-autofocus
                      aria-label={t('mset.edit.input', { n: s.id })}
                      aria-invalid={error ? true : undefined}
                      aria-describedby={error ? errorId : undefined}
                      onChange={(e) => onEditChange(e.target.value)}
                      onKeyDown={(e) => {
                        if (e.key === 'Enter') {
                          e.preventDefault();
                          if (verdict?.ok) onReview();
                        } else if (e.key === 'Escape') {
                          // Escape cancela solo la edición, no cierra la ventana.
                          e.preventDefault();
                          e.stopPropagation();
                          onEditCancel();
                        }
                      }}
                    />
                    <button type="button" className="btn" disabled={!verdict?.ok} aria-label={t('mset.edit.review', { n: s.id })} onClick={onReview}>
                      {t('mset.edit.reviewShort')}
                    </button>
                    <button type="button" className="btn" aria-label={t('mset.edit.cancel', { n: s.id })} onClick={onEditCancel}>
                      {t('machine.dlg.cancel')}
                    </button>
                    {error && <p id={errorId} className="hint hint--danger" role="alert">{error}</p>}
                  </div>
                ) : (
                  <button type="button" className="btn" disabled={blocked !== null || readOnly} title={why} aria-label={t('mset.row.edit', { n: s.id })} onClick={() => onEditStart(s.id)}>
                    <Pencil size={12} aria-hidden="true" />
                    {t('mset.row.editShort')}
                  </button>
                )}
              </td>
            </tr>
          );
        })}
      </tbody>
    </table>
  );
}
