import { useCostStore } from '@/features/costs/costStore';
import type { CommandDef } from '../types';

/**
 * «Costos y precio…»: la calculadora de costo y precio por pieza (`features/costs`). Sin atajo; se
 * ofrece en el panel del trabajo, en la vista previa y en el menú Máquina. Usa el tiempo y el área
 * que la app ya calcula, así que no necesita conexión ni máquina.
 */
export const costsCommands: CommandDef[] = [
  { id: 'costs.open', label: 'costs.cmd.open', category: 'machine', run: () => useCostStore.getState().openDialog() },
];
