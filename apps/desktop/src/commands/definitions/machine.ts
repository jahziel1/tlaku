import type { I18nKey } from '@/i18n';
import { hasOutput, isJobBusy, type AppState } from '@/store';
import type { CommandDef } from '../types';

const needMachine = (s: AppState): true | I18nKey => (s.connection.connected ? true : 'reason.needMachine');

/** Requisitos comunes de iniciar y encuadrar. */
const canSend = (s: AppState): true | I18nKey => {
  if (!s.connection.connected) return 'reason.needMachine';
  if (isJobBusy(s)) return 'reason.jobRunning';
  if (!hasOutput(s)) return 'reason.nothingToSend';
  return true;
};

export const machineCommands: CommandDef[] = [
  {
    id: 'machine.connect',
    label: 'cmd.machine.connect',
    labelFor: (s) => (s.connection.connected ? 'cmd.machine.disconnect' : 'cmd.machine.connect'),
    category: 'machine',
    enabled: (s) => (s.connecting ? 'reason.connecting' : true),
    run: (s) => (s.connection.connected ? s.disconnect() : s.connect()),
  },
  { id: 'machine.frame', label: 'cmd.machine.frame', category: 'machine', shortcuts: ['F'], enabled: canSend, run: (s) => s.frameJob() },
  { id: 'machine.start', label: 'cmd.machine.start', category: 'machine', shortcuts: ['Mod+Enter'], enabled: canSend, run: (s) => s.startJob() },
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
];
