import { useId } from 'react';
import { t } from '@/i18n';
import { RuidaExperimentalBadge } from '@/features/machine/RuidaSection';

/**
 * Asistente «Agregar máquina» con una Ruida: IP escrita a mano (Tlaku no busca Ruida en la red) y
 * el aviso de que es experimental y de que la cama y el home son valores de partida.
 */
export function RuidaHostField({ host, onHost }: { host: string; onHost: (host: string) => void }) {
  const id = useId();
  return (
    <section className="field" aria-label={t('ruida.wizard.host')}>
      <label htmlFor={`${id}-host`} className="field__label">
        {t('ruida.wizard.host')} <RuidaExperimentalBadge />
      </label>
      <input
        id={`${id}-host`}
        className="input num"
        placeholder="192.168.1.100"
        autoComplete="off"
        spellCheck={false}
        value={host}
        aria-describedby={`${id}-help`}
        onChange={(e) => onHost(e.target.value)}
      />
      <p id={`${id}-help`} className="hint">{t('ruida.wizard.help')}</p>
      <p className="hint">{t('ruida.wizard.defaults')}</p>
      <p className="hint">{t('ruida.experimental.note')}</p>
    </section>
  );
}
