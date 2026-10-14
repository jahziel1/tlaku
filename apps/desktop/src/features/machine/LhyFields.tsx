import type { DeviceProfile } from '@/api/types';
import { t } from '@/i18n';
import { Field } from './DeviceFields';
import { LhyExperimentalBadge } from './LhyNotice';
import { LHY_BOARDS, LHY_DEFAULT_BOARD } from './lhyPorts';

/** Modelos de placa para el selector: los conocidos y, si el perfil trae otro, ese también. */
export function boardOptions(current: string): string[] {
  return LHY_BOARDS.includes(current as (typeof LHY_BOARDS)[number]) ? [...LHY_BOARDS] : [...LHY_BOARDS, current];
}

/**
 * Campos de un perfil K40 con placa Lihuiyu (EXPERIMENTAL): el modelo de placa, que cambia las
 * ecuaciones de velocidad, y las notas de origen/home, USB y de lo que no aplica. El origen y la
 * cama se editan en sus grupos de siempre: el origen es la esquina donde la placa hace su home.
 */
export function LhyFields({ draft, edit }: { draft: DeviceProfile; edit: (patch: Partial<DeviceProfile>) => void }) {
  const board = draft.lhymicro.board;
  return (
    <>
      <p className="hint">
        <LhyExperimentalBadge /> {t('lhymicro.experimental.note')}
      </p>
      <Field label={t('lhymicro.dev.board')} help="lhymicro.dev.board.help">
        {({ id, describedBy }) => (
          <select id={id} className="select" value={board} aria-describedby={describedBy} onChange={(e) => edit({ lhymicro: { ...draft.lhymicro, board: e.target.value } })}>
            {boardOptions(board).map((b) => (
              <option key={b} value={b}>{b === LHY_DEFAULT_BOARD ? t('lhymicro.dev.board.default', { board: b }) : b}</option>
            ))}
          </select>
        )}
      </Field>
      <p className="hint"><strong>{t('lhymicro.dev.origin')}.</strong> {t('lhymicro.dev.origin.help')}</p>
      <p className="hint">{t('lhymicro.dev.maxRate')}</p>
      <p className="hint">{t('lhymicro.power.note')}</p>
      <p className="hint">{t('lhymicro.dev.connection')}</p>
      <p className="hint">{t('lhymicro.dev.hidden')}</p>
    </>
  );
}
