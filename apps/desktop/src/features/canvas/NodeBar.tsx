import { useMemo } from 'react';
import { Circle, Diamond, Link, Pentagon, Plus, Scissors, Slash, Spline, Square, Trash2, type LucideIcon } from 'lucide-react';
import { t } from '@/i18n';
import { useCommand } from '@/commands/useCommand';
import { useStore } from '@/store';
import { ExprField } from './ExprField';
import { useInteraction } from './interaction';
import { selectedNodeCenter, setNodeCoord, type NodeOp } from './nodeActions';
import { isNodeEditable } from './nodeview';

const GROUPS: { label: Parameters<typeof t>[0]; ops: [NodeOp, LucideIcon][] }[] = [
  { label: 'nodes.bar.type', ops: [['corner', Square], ['smooth', Circle], ['symmetric', Diamond]] },
  { label: 'nodes.bar.segment', ops: [['insert', Plus], ['toLine', Slash], ['toCurve', Spline]] },
  { label: 'nodes.bar.path', ops: [['break', Scissors], ['join', Link], ['close', Pentagon], ['delete', Trash2]] },
];

function NodeButton({ op, Icon }: { op: NodeOp; Icon: LucideIcon }) {
  const cmd = useCommand(`nodes.${op}`);
  return (
    <button type="button" className="icon-btn" title={cmd.tooltip} aria-label={cmd.label} aria-disabled={!cmd.enabled} onClick={() => cmd.enabled && cmd.run()}>
      <Icon size={16} aria-hidden="true" />
    </button>
  );
}

/**
 * Barra contextual de la herramienta de nodos (ocupa el lugar de la barra numérica): posición X e Y
 * EXACTAS del nodo elegido (con varios, el centro de su caja), tipo de nodo, segmento y trayecto.
 * Los botones son los mismos comandos que los atajos: etiqueta, tecla y motivo salen del registro.
 */
export function NodeBar() {
  const doc = useStore((s) => s.doc);
  const selectedIds = useStore((s) => s.selectedIds);
  const nodeSel = useStore((s) => s.nodeSel);
  const preview = useInteraction((s) => s.nodePreview);

  const state = useMemo(() => ({ doc, selectedIds, nodeSel }), [doc, selectedIds, nodeSel]);
  const center = useMemo(() => selectedNodeCenter(state, preview), [state, preview]);
  const count = useMemo(
    () => selectedIds.reduce((n, id) => n + (nodeSel[id]?.length ?? 0), 0),
    [selectedIds, nodeSel],
  );
  const hasVector = useMemo(
    () => doc.objects.some((o) => selectedIds.includes(o.id) && isNodeEditable(o)),
    [doc.objects, selectedIds],
  );

  const empty = center === null;
  const apply = (axis: 'x' | 'y') => (v: number) => void setNodeCoord(useStore.getState(), axis, v);
  const status =
    count === 1 ? t('nodes.bar.one')
    : count > 1 ? t('nodes.bar.many', { n: count })
    : hasVector ? t('nodes.bar.none')
    : selectedIds.length > 0 ? t('nodes.bar.notVector')
    : t('nodes.bar.pick');

  return (
    <div className="numeric-bar node-bar" role="group" aria-label={t('nodes.bar.label')}>
      <ExprField label={t('nodes.bar.x')} unit="mm" value={center?.[0] ?? 0} disabled={empty} title={t('nodes.bar.hint')} onApply={apply('x')} />
      <ExprField label={t('nodes.bar.y')} unit="mm" value={center?.[1] ?? 0} disabled={empty} title={t('nodes.bar.hint')} onApply={apply('y')} />
      {GROUPS.map((g) => (
        <div key={g.label} className="node-bar__group" role="group" aria-label={t(g.label)}>
          {g.ops.map(([op, Icon]) => (
            <NodeButton key={op} op={op} Icon={Icon} />
          ))}
        </div>
      ))}
      <span className="node-bar__status" role="status">{status}</span>
    </div>
  );
}
