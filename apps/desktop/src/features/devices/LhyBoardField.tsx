import { useId } from 'react';
import { LhyExperimentalBadge } from '@/features/machine/LhyNotice';
import { boardOptions } from '@/features/machine/LhyFields';
import { LHY_DEFAULT_BOARD } from '@/features/machine/lhyPorts';
import { t } from '@/i18n';

/**
 * Asistente «Agregar máquina» con un K40: el modelo de la placa (la M2 es la de serie) y el aviso
 * de que es experimental y de que la cama, el home y la velocidad son valores de partida.
 */
export function LhyBoardField({ board, onBoard }: { board: string; onBoard: (board: string) => void }) {
  const id = useId();
  return (
    <section className="field" aria-label={t('lhymicro.wizard.board')}>
      <label htmlFor={`${id}-board`} className="field__label">
        {t('lhymicro.wizard.board')} <LhyExperimentalBadge />
      </label>
      <select id={`${id}-board`} className="select" value={board} aria-describedby={`${id}-help`} onChange={(e) => onBoard(e.target.value)}>
        {boardOptions(board).map((b) => (
          <option key={b} value={b}>{b === LHY_DEFAULT_BOARD ? t('lhymicro.dev.board.default', { board: b }) : b}</option>
        ))}
      </select>
      <p id={`${id}-help`} className="hint">{t('lhymicro.wizard.board.help')}</p>
      <p className="hint">{t('lhymicro.wizard.defaults')}</p>
      <p className="hint">{t('lhymicro.experimental.note')}</p>
    </section>
  );
}
