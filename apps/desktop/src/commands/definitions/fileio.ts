import { exportAs, exportEnabled, importWithOptions, printDesign } from '@/features/fileio/actions';
import type { ExportFormat } from '@/features/fileio/types';
import type { I18nKey } from '@/i18n';
import type { CommandDef } from '../types';

const run = (action: () => Promise<unknown>) => async (): Promise<void> => {
  await action();
};

const exportCommand = (format: ExportFormat, label: I18nKey): CommandDef => ({
  id: `fileio.export${format[0]!.toUpperCase()}${format.slice(1)}`,
  label,
  category: 'file',
  enabled: exportEnabled,
  run: run(() => exportAs(format)),
});

/** Importar con opciones, exportar a SVG, DXF, PNG, PDF y AI, e imprimir (ola 1, bloque Formatos). */
export const fileioCommands: CommandDef[] = [
  { id: 'fileio.importOptions', label: 'fileio.cmd.importOptions', category: 'file', run: run(() => importWithOptions()) },
  exportCommand('svg', 'fileio.cmd.exportSvg'),
  exportCommand('dxf', 'fileio.cmd.exportDxf'),
  exportCommand('png', 'fileio.cmd.exportPng'),
  exportCommand('pdf', 'fileio.cmd.exportPdf'),
  exportCommand('ai', 'fileio.cmd.exportAi'),
  { id: 'fileio.print', label: 'fileio.cmd.print', category: 'file', shortcuts: ['Mod+P'], enabled: exportEnabled, run: run(() => printDesign()) },
];
