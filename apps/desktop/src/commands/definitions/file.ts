import { exportGcodeFile, importDesign, newProject, openProject, saveProject } from '@/features/history/files';
import { hasOutput } from '@/store';
import type { CommandDef } from '../types';

export const fileCommands: CommandDef[] = [
  { id: 'file.new', label: 'cmd.file.new', category: 'file', shortcuts: ['Mod+N'], run: () => newProject() },
  { id: 'file.open', label: 'cmd.file.open', category: 'file', shortcuts: ['Mod+O'], run: () => openProject() },
  { id: 'file.save', label: 'cmd.file.save', category: 'file', shortcuts: ['Mod+S'], run: () => saveProject(false) },
  { id: 'file.saveAs', label: 'cmd.file.saveAs', category: 'file', shortcuts: ['Mod+Shift+S'], run: () => saveProject(true) },
  { id: 'file.import', label: 'cmd.file.import', category: 'file', shortcuts: ['Mod+I'], run: () => importDesign() },
  {
    id: 'file.exportGcode',
    label: 'cmd.file.exportGcode',
    category: 'file',
    shortcuts: ['Mod+Shift+E'],
    enabled: (s) => (hasOutput(s) ? true : 'reason.nothingToSend'),
    run: () => exportGcodeFile(),
  },
];
