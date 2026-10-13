import { designReason, flattenArrays, setVirtualArray, virtualTarget } from '@/features/arrange/designOps';
import { useDesignUi } from '@/features/arrange/designUi';
import { geometryReason } from '@/features/arrange/geomOps';
import { t } from '@/i18n';
import { autoGroupObjects, containmentGroups } from '@/lib/autogroup';
import { groupIdsOf } from '@/lib/groups';
import { selectSelectedObjects } from '@/store';
import type { AppState } from '@/store';
import type { CommandDef } from '../types';

/** Objetos seleccionados que tienen un arreglo virtual. */
const withArray = (s: AppState) => selectSelectedObjects(s).filter((o) => o.array && !o.locked);

/**
 * Diseño avanzado (ola 3): deformar, contorno de banda elástica, círculo tangente, asistente
 * booleano, copiar a lo largo de un trazo, arreglo virtual, acoplar y agrupar automáticamente.
 * Las operaciones con parámetros abren un diálogo (`DesignDialogHost`).
 */
export const designAdvancedCommands: CommandDef[] = [
  {
    id: 'arrange.warp',
    label: 'design.cmd.warp',
    category: 'arrange',
    enabled: (s) => designReason(s),
    run: () => useDesignUi.getState().open('warp'),
  },
  {
    id: 'arrange.hull',
    label: 'design.cmd.hull',
    category: 'arrange',
    enabled: (s) => designReason(s),
    run: () => useDesignUi.getState().open('hull'),
  },
  {
    id: 'arrange.tangent',
    label: 'design.cmd.tangent',
    category: 'arrange',
    enabled: (s) => designReason(s, 2, 3),
    run: () => useDesignUi.getState().open('tangent'),
  },
  {
    id: 'arrange.boolAssistant',
    label: 'design.cmd.boolAssistant',
    category: 'arrange',
    shortcuts: ['Mod+B'],
    enabled: (s) => {
      const base = designReason(s, 2, Infinity, false);
      return base === true ? geometryReason(s, 'union') : base;
    },
    run: () => {
      const ui = useDesignUi.getState();
      ui.setAssistantPrimary(null);
      ui.open('boolAssistant');
    },
  },
  {
    id: 'arrange.alongPath',
    label: 'design.cmd.alongPath',
    category: 'arrange',
    enabled: (s) => designReason(s, 2),
    run: () => {
      useDesignUi.getState().setAlongPath(null);
      useDesignUi.getState().open('alongPath');
    },
  },
  {
    id: 'arrange.dock',
    label: 'design.cmd.dock',
    category: 'arrange',
    enabled: (s) => designReason(s, 2),
    run: () => {
      useDesignUi.getState().setDockRef(null);
      useDesignUi.getState().open('dock');
    },
  },
  {
    id: 'arrange.virtualArray',
    label: 'design.cmd.virtualArray',
    category: 'arrange',
    enabled: (s) => {
      const target = virtualTarget(s);
      return 'reason' in target ? target.reason : true;
    },
    run: () => useDesignUi.getState().open('virtualArray'),
  },
  {
    id: 'arrange.virtualFlatten',
    label: 'design.cmd.virtualFlatten',
    category: 'arrange',
    enabled: (s) => (withArray(s).length > 0 ? true : 'design.reason.noVirtual'),
    run: (s) => {
      const ids = new Set(withArray(s).map((o) => o.id));
      const created: string[] = [];
      s.applyDocChange('design.history.virtualFlatten', (doc) => {
        created.length = 0;
        const r = flattenArrays(doc, ids);
        created.push(...r.created);
        return r.doc;
      });
      if (created.length > 0) s.select([...ids, ...created]);
    },
  },
  {
    id: 'arrange.virtualRemove',
    label: 'design.cmd.virtualRemove',
    category: 'arrange',
    enabled: (s) => (withArray(s).length > 0 ? true : 'design.reason.noVirtual'),
    run: (s) => {
      for (const o of withArray(s)) setVirtualArray(s, o.id, null);
    },
  },
  {
    id: 'edit.autoGroup',
    label: 'design.cmd.autoGroup',
    category: 'edit',
    enabled: (s) => (selectSelectedObjects(s).length >= 2 ? true : 'design.reason.autoGroupNeedsTwo'),
    run: (s) => {
      const picked = selectSelectedObjects(s);
      if (containmentGroups(picked).length === 0) {
        s.notifyKey('info', 'design.autoGroup.none');
        return;
      }
      let groups = 0;
      s.applyDocChange('design.history.autoGroup', (doc) => {
        const pickedIds = new Set(picked.map((o) => o.id));
        const taken = groupIdsOf(doc.objects);
        const grouped = autoGroupObjects(doc.objects.filter((o) => pickedIds.has(o.id)), taken);
        groups = grouped.groups;
        const by = new Map(grouped.objects.map((o) => [o.id, o]));
        return groups === 0 ? doc : { ...doc, objects: doc.objects.map((o) => by.get(o.id) ?? o) };
      });
      if (groups > 0) s.notify('success', t('design.autoGroup.done', { n: groups }));
    },
  },
];
