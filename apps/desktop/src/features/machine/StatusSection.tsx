import type { MachineState } from '@/api/types';
import { t, type I18nKey } from '@/i18n';
import { fmt } from '@/lib/platform';
import { useStore } from '@/store';
import { selectIsVirtual } from '@/store/machine';
import { selectIsGalvo } from './galvoPorts';
import { selectIsLhy } from './lhyPorts';
import { selectIsRuida } from './ruidaPorts';
import { VirtualBadge } from './VirtualBadge';

export type StateTone = 'ok' | 'accent' | 'warn' | 'danger' | 'off';

/** Color del estado: verde = lista, violeta = trabajando, ámbar = detenida a la espera, rojo = alarma. */
export function stateTone(state: MachineState | null): StateTone {
  switch (state) {
    case null:
    case 'sleep':
      return 'off';
    case 'idle':
      return 'ok';
    case 'run':
    case 'jog':
    case 'home':
    case 'other':
      return 'accent';
    case 'alarm':
      return 'danger';
    case 'hold':
    case 'door':
    case 'check':
      return 'warn';
  }
}

const HINTS: Partial<Record<MachineState, I18nKey>> = {
  hold: 'machine.stateHint.hold',
  door: 'machine.stateHint.door',
  check: 'machine.stateHint.check',
  sleep: 'machine.stateHint.sleep',
  home: 'machine.stateHint.home',
  alarm: 'machine.stateHint.alarm',
};

/**
 * Nombre del estado para mostrar. Un estado que Tlaku no conoce (`other`, p. ej. `Tool` de
 * grblHAL) se muestra con el nombre original que informó la controladora, si llegó.
 */
export function stateLabel(state: MachineState | null, otherState?: string | null): string {
  if (!state) return t('machine.state.off');
  if (state === 'other' && otherState) return otherState;
  return t(`machine.state.${state}`);
}

export function StateBadge({ state, otherState }: { state: MachineState | null; otherState?: string | null }) {
  return (
    <span className="badge" data-tone={stateTone(state)}>
      <span className="badge__dot" aria-hidden="true" />
      {stateLabel(state, otherState)}
    </span>
  );
}

function Pos({ label, pos }: { label: string; pos: [number, number] | undefined }) {
  return (
    <div className="mc-pos">
      <span className="mc-pos__label">{label}</span>
      <span className="num"><span className="mc-axis">X</span> {pos ? fmt(pos[0], 2) : '—'}</span>
      <span className="num"><span className="mc-axis">Y</span> {pos ? fmt(pos[1], 2) : '—'}</span>
    </div>
  );
}

/** Estado en vivo: texto y color, posición de trabajo y de máquina, avance F y potencia S. */
export function StatusSection() {
  const status = useStore((s) => s.status);
  const connected = useStore((s) => s.connection.connected);
  const virtual = useStore(selectIsVirtual);
  // Ruida y el K40 no informan avance F ni potencia S en vivo.
  const isRuida = useStore(selectIsRuida);
  const isLhy = useStore(selectIsLhy);
  const isGalvo = useStore(selectIsGalvo);
  const ruida = isRuida || isLhy || isGalvo;
  const state = connected ? (status?.state ?? 'idle') : null;
  const otherState = state === 'other' ? (status?.other_state ?? null) : null;
  const hint = state === 'other' ? t('machine.stateHint.other', { name: stateLabel(state, otherState) }) : state ? HINTS[state] && t(HINTS[state]) : undefined;
  return (
    <section className="section" aria-labelledby="sec-status">
      <h3 id="sec-status" className="panel-title">{t('machine.status')}</h3>
      <div className="readout">
        <div role="status" aria-live="polite" className="mc-state">
          <div className="mc-state__row">
            <StateBadge state={state} otherState={otherState} />
            {virtual && <VirtualBadge />}
          </div>
          {hint && <p className="hint">{hint}</p>}
        </div>
        <div className="mc-readout">
          <Pos label={t('machine.pos.work')} pos={connected ? status?.work_pos : undefined} />
          <Pos label={t('machine.pos.machine')} pos={connected ? status?.machine_pos : undefined} />
          {!ruida && (
          <dl className="kv mc-kv2">
            <div><dt title={t('machine.feed.help')}>F</dt><dd className="num">{connected && status ? Math.round(status.feed) : '—'}</dd></div>
            <div><dt title={t('machine.power.help')}>S</dt><dd className="num">{connected && status ? Math.round(status.spindle) : '—'}</dd></div>
          </dl>
          )}
        </div>
      </div>
    </section>
  );
}
