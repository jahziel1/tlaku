import { addTab } from '@/features/queue/queue';
import { useQueue } from '@/features/queue/queueStore';
import type { CommandDef } from '../types';

/** Cola guiada de trabajos (ver `features/queue`). Abrirla o agregar a ella nunca envía nada a la máquina. */
export const queueCommands: CommandDef[] = [
  { id: 'queue.open', label: 'queue.cmd.open', category: 'machine', run: () => useQueue.getState().setDialogOpen(true) },
  {
    id: 'queue.addTab',
    label: 'queue.cmd.addTab',
    category: 'machine',
    run: (s) => {
      const added = addTab(s.activeTabId);
      s.notifyKey(added ? 'success' : 'info', added ? 'queue.notice.added' : 'queue.notice.alreadyIn');
    },
  },
];
