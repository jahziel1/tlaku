import type { I18nKey } from '@/i18n';
import type { AppState } from '@/store';
import { isJobBusy, selectActiveDevice } from '@/store';
import { useProbeCenter } from '@/features/machine/probeCenter';
import { jogBlockedReason } from '@/store/machine';
import { SECTION_IDS } from '@/lib/panelSections';
import { revealPanelSection } from '@/ui/PanelSection';
import type { CommandDef } from '../types';

/** Una macro por comando (`machine.macro1` a `machine.macro10`): así cada una puede tener su atajo propio en Ajustes. */
const MACRO_LABELS: I18nKey[] = [
  'machine.cmd.macro1',
  'machine.cmd.macro2',
  'machine.cmd.macro3',
  'machine.cmd.macro4',
  'machine.cmd.macro5',
  'machine.cmd.macro6',
  'machine.cmd.macro7',
  'machine.cmd.macro8',
  'machine.cmd.macro9',
  'machine.cmd.macro10',
];

const macroReason = (s: AppState, index: number): true | I18nKey => {
  if (!selectActiveDevice(s).macros[index]) return 'machine.macros.empty';
  if (!s.connection.connected) return 'reason.needMachine';
  if (isJobBusy(s)) return 'reason.jobRunning';
  return true;
};

export const machineExtraCommands: CommandDef[] = [
  // Buscador de centro con el cabezal (M48): el diálogo explica por qué no se puede si falta algo.
  {
    id: 'machine.probeCenter',
    label: 'machine.probe.cmd',
    category: 'machine',
    run: () => useProbeCenter.getState().openDialog(),
  },
  // Abren la sección del panel Máquina, la dejan a la vista y mueven el foco a su encabezado.
  { id: 'machine.showLive', label: 'machine.cmd.showLive', category: 'machine', run: () => revealPanelSection(SECTION_IDS.job) },
  { id: 'machine.showPositions', label: 'machine.cmd.showPositions', category: 'machine', run: () => revealPanelSection(SECTION_IDS.positions) },
  ...MACRO_LABELS.map(
    (label, i): CommandDef => ({
      id: `machine.macro${i + 1}`,
      label,
      category: 'machine',
      enabled: (s) => macroReason(s, i),
      run: (s) => s.runMacro(i),
    }),
  ),
  {
    id: 'machine.focus',
    label: 'machine.cmd.focus',
    category: 'machine',
    enabled: (s) => (selectActiveDevice(s).z_axis ? (jogBlockedReason(s) ?? true) : 'machine.axis.notEnabled'),
    run: (s) => s.goFocus(),
  },
];
