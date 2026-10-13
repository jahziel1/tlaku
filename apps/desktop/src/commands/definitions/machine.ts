import { JOG_DIRS, type JogDirName } from '@/features/machine/jogMath';
import type { I18nKey } from '@/i18n';
import { isJobBusy } from '@/store';
import { jogBlockedReason, sendBlockedReason } from '@/store/machine';
import type { AppState } from '@/store';
import type { CommandDef } from '../types';

const needMachine = (s: AppState): true | I18nKey => (s.connection.connected ? true : 'reason.needMachine');

/** Requisitos de iniciar y encuadrar: conectada, en reposo, sin trabajo en curso y con algo que enviar. */
const canSend = (s: AppState): true | I18nKey => sendBlockedReason(s) ?? true;

/** Mover el cabezal a mano. */
const canJog = (s: AppState): true | I18nKey => jogBlockedReason(s) ?? true;

/** $H y $X: conectada y sin trabajo en curso (también valen en alarma, que es cuando se necesitan). */
const canRecover = (s: AppState): true | I18nKey => {
  if (!s.connection.connected) return 'reason.needMachine';
  if (isJobBusy(s)) return 'reason.jobRunning';
  return true;
};

/**
 * Jog con teclado desde cualquier panel: `Mod+Alt+flecha`. Las flechas solas (y con Mayús o Alt) son
 * del editor (mover objetos); con el foco en el pad de jog sirven para mover el cabezal sin modificador.
 */
const JOG_KEYS: [Exclude<JogDirName, 'upLeft' | 'upRight' | 'downLeft' | 'downRight'>, string, I18nKey][] = [
  ['left', 'Mod+Alt+ArrowLeft', 'machine.jog.left'],
  ['right', 'Mod+Alt+ArrowRight', 'machine.jog.right'],
  ['up', 'Mod+Alt+ArrowUp', 'machine.jog.up'],
  ['down', 'Mod+Alt+ArrowDown', 'machine.jog.down'],
];

const jogCommands: CommandDef[] = JOG_KEYS.map(([dir, shortcut, label]) => ({
  id: `machine.jog.${dir}`,
  label,
  category: 'machine',
  hidden: true,
  shortcuts: [shortcut],
  enabled: canJog,
  run: (s) => s.jogDir(JOG_DIRS[dir]),
}));

export const machineCommands: CommandDef[] = [
  {
    id: 'machine.connect',
    label: 'cmd.machine.connect',
    labelFor: (s) => (s.connection.connected ? 'cmd.machine.disconnect' : 'cmd.machine.connect'),
    category: 'machine',
    enabled: (s) => (s.connecting ? 'reason.connecting' : true),
    run: (s) => (s.connection.connected ? s.disconnect() : s.connect()),
  },
  { id: 'machine.devices', label: 'machine.cmd.devices', category: 'machine', run: (s) => s.openDialog({ kind: 'devices' }) },
  { id: 'machine.frame', label: 'cmd.machine.frame', category: 'machine', shortcuts: ['F'], enabled: canSend, run: (s) => s.frameJob() },
  // Iniciar nunca sale directo: abre "¿Láser listo y área despejada?" y solo el botón de ese diálogo envía.
  { id: 'machine.start', label: 'cmd.machine.start', category: 'machine', shortcuts: ['Mod+Enter'], enabled: canSend, run: (s) => s.requestStart() },
  {
    id: 'machine.pause',
    label: 'cmd.machine.pause',
    labelFor: (s) => (s.jobState === 'paused' ? 'cmd.machine.resume' : 'cmd.machine.pause'),
    category: 'machine',
    shortcuts: ['Mod+Shift+P'],
    enabled: (s) => (isJobBusy(s) ? true : 'reason.noJob'),
    run: (s) => (s.jobState === 'paused' ? s.resumeJob() : s.pauseJob()),
  },
  {
    id: 'machine.stop',
    label: 'cmd.machine.stop',
    category: 'machine',
    shortcuts: ['Mod+.'],
    enabled: (s) => needMachine(s),
    run: (s) => s.cancelJob(),
  },
  // $H y $X abren un diálogo que explica el riesgo; nada se envía hasta confirmar.
  { id: 'machine.home', label: 'machine.cmd.home', category: 'machine', enabled: canRecover, run: (s) => s.openDialog({ kind: 'alarm', action: 'home' }) },
  { id: 'machine.unlock', label: 'machine.cmd.unlock', category: 'machine', enabled: canRecover, run: (s) => s.openDialog({ kind: 'alarm', action: 'unlock' }) },
  { id: 'machine.setOrigin', label: 'machine.cmd.setOrigin', category: 'machine', enabled: needMachine, run: (s) => s.setUserOriginHere() },
  { id: 'machine.goOrigin', label: 'machine.cmd.goOrigin', category: 'machine', enabled: canJog, run: (s) => s.goToOrigin() },
  ...jogCommands,
];
