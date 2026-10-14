import { useId } from 'react';
import type { LaserSource } from '@/api/types';
import { GalvoExperimentalBadge } from '@/features/machine/GalvoNotice';
import { COMMON_FIELDS_MM, GALVO_SOURCES } from '@/features/machine/galvoProfile';
import { t } from '@/i18n';

/** Lo que el asistente pregunta de un galvo: el campo de la lente y la fuente. */
export interface GalvoSetup {
  field_mm: number;
  source: LaserSource;
}

export const DEFAULT_GALVO_SETUP: GalvoSetup = { field_mm: 110, source: 'fiber_q' };

/**
 * Asistente "Agregar máquina" con un galvo: lente y fuente, y el aviso de que es experimental y de que
 * lo demás (frecuencias, anchos de pulso, retardos) se completa en el perfil con el manual de la fuente.
 */
export function GalvoSetupField({ setup, onSetup }: { setup: GalvoSetup; onSetup: (s: GalvoSetup) => void }) {
  const id = useId();
  const field = COMMON_FIELDS_MM.includes(setup.field_mm as (typeof COMMON_FIELDS_MM)[number]) ? String(setup.field_mm) : 'custom';
  return (
    <section className="field" aria-label={t('galvo.wizard.setup')}>
      <p className="field__label">
        {t('galvo.wizard.setup')} <GalvoExperimentalBadge />
      </p>
      <label htmlFor={`${id}-field`} className="field__label">{t('galvo.dev.field')}</label>
      <select id={`${id}-field`} className="select" value={field} aria-describedby={`${id}-field-help`} onChange={(e) => e.target.value !== 'custom' && onSetup({ ...setup, field_mm: Number(e.target.value) })}>
        {COMMON_FIELDS_MM.map((mm) => (
          <option key={mm} value={mm}>{`${mm} × ${mm} mm`}</option>
        ))}
        {field === 'custom' && <option value="custom">{`${setup.field_mm} × ${setup.field_mm} mm`}</option>}
      </select>
      <p id={`${id}-field-help`} className="hint">{t('galvo.wizard.field.help')}</p>
      <label htmlFor={`${id}-source`} className="field__label">{t('galvo.dev.source')}</label>
      <select id={`${id}-source`} className="select" value={setup.source} aria-describedby={`${id}-source-help`} onChange={(e) => onSetup({ ...setup, source: e.target.value as LaserSource })}>
        {GALVO_SOURCES.map((s) => (
          <option key={s} value={s}>{t(`galvo.source.${s}`)}</option>
        ))}
      </select>
      <p id={`${id}-source-help`} className="hint">{t(setup.source === 'fiber_mopa' ? 'galvo.wizard.source.mopa' : 'galvo.wizard.source.help')}</p>
      <p className="hint">{t('galvo.wizard.defaults')}</p>
      <p className="hint">{t('galvo.experimental.note')}</p>
    </section>
  );
}
