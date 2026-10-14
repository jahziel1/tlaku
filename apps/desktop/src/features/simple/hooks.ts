import { useMemo } from 'react';
import { selectActiveDevice, useStore } from '@/store';
import { useMachineInfo, useMaterialChoices, type MachineInfo } from './actions';
import { colorGroups, validate, type ColorGroup, type MaterialChoice, type Problem } from './model';
import { useSimple } from './store';

export interface SimpleData {
  groups: ColorGroup[];
  choices: MaterialChoice[];
  choice: MaterialChoice | null;
  problems: Problem[];
  machine: MachineInfo;
}

/** Lo que los pasos necesitan saber del diseño, el material elegido y la máquina, siempre al día. */
export function useSimpleData(): SimpleData {
  const doc = useStore((s) => s.doc);
  const device = useStore(selectActiveDevice);
  const connected = useStore((s) => s.connection.connected);
  const materialKey = useSimple((s) => s.materialKey);
  const actions = useSimple((s) => s.actions);
  const choices = useMaterialChoices();
  const choice = useMemo(() => choices.find((c) => c.key === materialKey) ?? null, [choices, materialKey]);
  const groups = useMemo(() => colorGroups(doc), [doc]);
  const problems = useMemo(() => validate({ doc, device, connected, choice, actions }), [doc, device, connected, choice, actions]);
  const machine = useMachineInfo();
  return { groups, choices, choice, problems, machine };
}
