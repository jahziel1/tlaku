import type { CommandDef } from '../types';

export const previewCommands: CommandDef[] = [
  {
    id: 'view.preview',
    label: 'cmd.view.preview',
    category: 'view',
    shortcuts: ['P'],
    checked: (s) => s.previewVisible,
    run: (s) => {
      const show = !s.previewVisible;
      s.setPreviewVisible(show);
      if (show) void s.computePreview();
    },
  },
];
