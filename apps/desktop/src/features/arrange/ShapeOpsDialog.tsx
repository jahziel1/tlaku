import { useEffect, useMemo, useState } from 'react';
import type { DesignObject, ShapeOp, ShapeParams, ShapeStats } from '@/api/types';
import { selectedNodeCount } from '@/features/canvas/nodeActions';
import { t } from '@/i18n';
import { errorMessage } from '@/lib/errors';
import { fmt } from '@/lib/platform';
import { useStore } from '@/store';
import { NumberField } from '@/ui/NumberField';
import { LengthField } from '@/ui/LengthField';
import { cleanupOperands } from './cleanup';
import { Check, Choice, Dialog, Footer } from './Dialog';
import { knifeCandidates, resolveKnife, runShapeOp, shapeParams, shapeReason, previewShapeOp } from './shapeOps';
import { useShapeOps, type ShapeDialogOp } from './shapeOpsUi';
import { useArrangeUi } from './uiStore';
import { unitLabel, unitThumbnail } from './unitLabel';

type Preview = { status: 'computing' } | { status: 'error'; msg: string } | { status: 'ready'; stats: ShapeStats };

const PREVIEW_DELAY_MS = 150;

/**
 * Calcula la operación con retardo, sin aplicarla: cada cambio cancela el cálculo anterior (solo vale
 * el último). Sirve para mostrar el número de nodos antes y después, cuántas ranuras se encontraron...
 */
function usePreview(op: ShapeOp, params: ShapeParams, enabled: boolean): Preview {
  const doc = useStore((s) => s.doc);
  const selectedIds = useStore((s) => s.selectedIds);
  const [preview, setPreview] = useState<Preview>({ status: 'computing' });
  const key = JSON.stringify(params);
  useEffect(() => {
    if (!enabled) return;
    let current = true;
    setPreview({ status: 'computing' });
    const timer = setTimeout(() => {
      previewShapeOp({ doc, selectedIds }, op, params)
        .then((stats) => current && setPreview({ status: 'ready', stats }))
        .catch((e: unknown) => current && setPreview({ status: 'error', msg: errorMessage(e) }));
    }, PREVIEW_DELAY_MS);
    return () => {
      current = false;
      clearTimeout(timer);
    };
    // `params` se compara por su contenido (`key`).
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [doc, selectedIds, op, key, enabled]);
  return preview;
}

/** Parámetros actuales de la operación según el estado de la selección. */
function useParams(op: ShapeOp): ShapeParams {
  const doc = useStore((s) => s.doc);
  const selectedIds = useStore((s) => s.selectedIds);
  const nodeSel = useStore((s) => s.nodeSel);
  const ui = useShapeOps();
  // Los valores de `ui` cambian la identidad en cada parche: se recalcula con ellos.
  return useMemo(() => shapeParams(op, { doc, selectedIds, nodeSel }), [op, doc, selectedIds, nodeSel, ui.breakApart, ui.optimize, ui.fillet, ui.cut, ui.slots]); // eslint-disable-line react-hooks/exhaustive-deps
}

interface Shell {
  op: ShapeOp;
  title: string;
  /** Motivo extra por el que no se puede aplicar (por ejemplo, la vista previa no cambia nada). */
  extraReason: string | null;
  children: React.ReactNode;
  applyLabel?: string;
}

/** Marco común: pie con el motivo de deshabilitado y la acción de aplicar. */
function Shell({ op, title, extraReason, children, applyLabel }: Shell) {
  const doc = useStore((s) => s.doc);
  const selectedIds = useStore((s) => s.selectedIds);
  const busy = useArrangeUi((s) => s.busy);
  const close = useShapeOps((s) => s.close);
  const reason = shapeReason({ doc, selectedIds }, op);
  const why = reason !== true ? t(reason) : extraReason;
  const blocked = why !== null || busy;
  const apply = async () => {
    if (await runShapeOp(useStore.getState(), op)) close();
  };
  return (
    <Dialog
      title={title}
      onClose={close}
      initialFocus=".field__input, [aria-checked='true']"
      footer={<Footer onClose={close} onApply={() => void apply()} disabled={blocked} reason={why} applyLabel={busy ? t('arrange.dlg.working') : applyLabel} />}
    >
      {children}
    </Dialog>
  );
}

const errorOf = (p: Preview): string | null => (p.status === 'error' ? p.msg : null);

// --- Separar ------------------------------------------------------------------------------------------

function BreakApartDialog() {
  const params = useShapeOps((s) => s.breakApart);
  const patch = useShapeOps((s) => s.patchBreakApart);
  const preview = usePreview('break_apart', useParams('break_apart'), true);
  const stats = preview.status === 'ready' ? preview.stats : null;
  return (
    <Shell op="break_apart" title={t('shapes.break.title')} extraReason={errorOf(preview)}>
      <p className="hint">{t('shapes.break.hint')}</p>
      <Check label={t('shapes.break.keepHoles')} checked={params.keepHoles} onChange={(keepHoles) => patch({ keepHoles })} />
      <p className="hint">{t('shapes.break.keepHolesHint')}</p>
      {stats && <p className="hint ar-summary" role="status">{t('shapes.break.preview', { from: stats.count, to: stats.produced })}</p>}
    </Shell>
  );
}

// --- Optimizar ----------------------------------------------------------------------------------------

function OptimizeDialog() {
  const params = useShapeOps((s) => s.optimize);
  const patch = useShapeOps((s) => s.patchOptimize);
  const preview = usePreview('optimize', useParams('optimize'), true);
  const stats = preview.status === 'ready' ? preview.stats : null;
  const same = stats !== null && stats.count === 0;
  return (
    <Shell op="optimize" title={t('shapes.optimize.title')} extraReason={errorOf(preview) ?? (same ? t('shapes.optimize.nothing') : null)}>
      <p className="hint">{t('shapes.optimize.hint')}</p>
      <LengthField label={t('shapes.optimize.tolerance')} value={params.tolerance} min={0} max={5} step={0.01} onCommit={(tolerance) => patch({ tolerance })} />
      <p className="hint">{t('shapes.optimize.toleranceHint')}</p>
      <Check label={t('shapes.optimize.smooth')} checked={params.smooth} onChange={(smooth) => patch({ smooth })} />
      <p className="hint">{t('shapes.optimize.smoothHint')}</p>
      <NumberField label={t('shapes.optimize.corner')} value={params.cornerAngle} min={5} max={120} step={5} unit="°" onCommit={(cornerAngle) => patch({ cornerAngle })} />
      <div className="ar-stats" role="status" aria-live="polite">
        {stats ? (
          <>
            <span>{t('shapes.optimize.nodesBefore')} <strong className="num">{stats.nodes_before}</strong></span>
            <span>{t('shapes.optimize.nodesAfter')} <strong className="num">{stats.nodes_after}</strong></span>
            <span className="num">{stats.nodes_before > 0 ? t('shapes.optimize.saved', { pct: Math.round(((stats.nodes_before - stats.nodes_after) / stats.nodes_before) * 100) }) : ''}</span>
          </>
        ) : (
          <span className="hint">{preview.status === 'computing' ? t('arrange.dlg.working') : ''}</span>
        )}
      </div>
    </Shell>
  );
}

// --- Redondear esquinas -------------------------------------------------------------------------------

function FilletDialog() {
  const params = useShapeOps((s) => s.fillet);
  const patch = useShapeOps((s) => s.patchFillet);
  const doc = useStore((s) => s.doc);
  const selectedIds = useStore((s) => s.selectedIds);
  const nodeSel = useStore((s) => s.nodeSel);
  const nodeCount = selectedNodeCount({ doc, selectedIds, nodeSel });
  const preview = usePreview('fillet', useParams('fillet'), true);
  const stats = preview.status === 'ready' ? preview.stats : null;
  const none = stats !== null && stats.count === 0;
  const onlyNodes = params.onlyNodes && nodeCount > 0;
  return (
    <Shell op="fillet" title={t('shapes.fillet.title')} extraReason={errorOf(preview) ?? (none ? t(stats.skipped > 0 ? 'shapes.fillet.noSpace' : 'shapes.fillet.nothing') : null)}>
      <p className="hint">{t('shapes.fillet.hint')}</p>
      <Choice
        legend={t('shapes.fillet.style')}
        value={params.style}
        options={[
          { value: 'round', label: t('shapes.fillet.round') },
          { value: 'chamfer', label: t('shapes.fillet.chamfer') },
          { value: 'bite', label: t('shapes.fillet.bite') },
        ]}
        onChange={(style) => patch({ style })}
      />
      <LengthField label={t(`shapes.fillet.radius.${params.style}`)} value={params.radius} min={0.01} max={500} step={0.5} onCommit={(radius) => patch({ radius })} />
      <p className="hint">{t(`shapes.fillet.radiusHint.${params.style}`)}</p>
      <Check
        label={nodeCount > 0 ? t('shapes.fillet.onlyNodes', { n: nodeCount }) : t('shapes.fillet.onlyNodesNone')}
        checked={onlyNodes}
        onChange={(v) => nodeCount > 0 && patch({ onlyNodes: v })}
      />
      {stats && !none && (
        <p className="hint ar-summary" role="status">
          {t('shapes.fillet.preview', { n: stats.count })}
          {stats.clamped > 0 ? ` ${t('shapes.fillet.previewClamped', { n: stats.clamped })}` : ''}
          {stats.skipped > 0 ? ` ${t('shapes.fillet.previewSkipped', { n: stats.skipped })}` : ''}
        </p>
      )}
    </Shell>
  );
}

// --- Cortar formas ------------------------------------------------------------------------------------

function KnifeThumb({ object }: { object: DesignObject }) {
  const thumb = useMemo(() => unitThumbnail({ objects: [object] }), [object]);
  if (!thumb) return <span className="ar-swatch" style={{ background: object.layer }} aria-hidden="true" />;
  return (
    <svg className="ar-thumb" viewBox={`0 0 ${thumb.width} ${thumb.height}`} preserveAspectRatio="xMidYMid meet" aria-hidden="true">
      <path d={thumb.d} fill="none" stroke={object.layer} strokeWidth="6" strokeLinejoin="round" strokeLinecap="round" vectorEffect="non-scaling-stroke" />
    </svg>
  );
}

function CutDialog() {
  const params = useShapeOps((s) => s.cut);
  const patch = useShapeOps((s) => s.patchCut);
  const doc = useStore((s) => s.doc);
  const selectedIds = useStore((s) => s.selectedIds);
  const operands = useMemo(() => cleanupOperands({ doc, selectedIds }), [doc, selectedIds]);
  const candidates = knifeCandidates(operands);
  const knife = resolveKnife(operands, params.knifeId, selectedIds);
  const preview = usePreview('cut_shapes', useParams('cut_shapes'), knife !== null);
  const stats = preview.status === 'ready' ? preview.stats : null;
  const none = stats !== null && stats.count === 0;
  return (
    <Shell op="cut_shapes" title={t('shapes.cut.title')} extraReason={errorOf(preview) ?? (none ? t('shapes.cut.nothing') : null)}>
      <p className="hint">{t('shapes.cut.hint')}</p>
      <div className="ar-list" role="radiogroup" aria-label={t('shapes.cut.knife')}>
        {candidates.map((o) => {
          const isKnife = o.id === knife?.id;
          return (
            <button key={o.id} type="button" role="radio" aria-checked={isKnife} className="ar-item" onClick={() => patch({ knifeId: o.id })}>
              <span className="ar-item__role" aria-hidden="true">{isKnife ? t('shapes.cut.roleKnife') : t('shapes.cut.roleShape')}</span>
              <KnifeThumb object={o} />
              <span className="ar-item__name">{unitLabel({ objects: [o] })}</span>
            </button>
          );
        })}
      </div>
      <Check label={t('shapes.cut.closePieces')} checked={params.closePieces} onChange={(closePieces) => patch({ closePieces })} />
      <p className="hint">{t(params.closePieces ? 'shapes.cut.closePiecesHint' : 'shapes.cut.openPiecesHint')}</p>
      <Check label={t('shapes.cut.keepKnife')} checked={params.keepKnife} onChange={(keepKnife) => patch({ keepKnife })} />
      {stats && !none && <p className="hint ar-summary" role="status">{t('shapes.cut.preview', { n: stats.count, pieces: stats.produced })}</p>}
    </Shell>
  );
}

// --- Redimensionar ranuras ----------------------------------------------------------------------------

function SlotsDialog() {
  const params = useShapeOps((s) => s.slots);
  const patch = useShapeOps((s) => s.patchSlots);
  const preview = usePreview('resize_slots', useParams('resize_slots'), true);
  const stats = preview.status === 'ready' ? preview.stats : null;
  const none = stats !== null && stats.count === 0;
  const why = errorOf(preview) ?? (none ? t(stats.found > 0 ? 'shapes.slots.blocked' : 'shapes.slots.nothing', { width: fmt(params.fromWidth, 2) }) : null);
  return (
    <Shell op="resize_slots" title={t('shapes.slots.title')} extraReason={why}>
      <p className="hint">{t('shapes.slots.hint')}</p>
      <LengthField label={t('shapes.slots.from')} value={params.fromWidth} min={0.05} max={100} step={0.1} onCommit={(fromWidth) => patch({ fromWidth })} />
      <LengthField label={t('shapes.slots.tolerance')} value={params.tolerance} min={0} max={5} step={0.05} onCommit={(tolerance) => patch({ tolerance })} />
      {stats && stats.widths.length > 0 && (
        <div className="ar-choice">
          <span className="field__label">{t('shapes.slots.detected')}</span>
          <div className="ar-choice__row">
            {stats.widths.map(([width, count]) => (
              <button key={width} type="button" className="chip" aria-pressed={Math.abs(width - params.fromWidth) < 0.005} onClick={() => patch({ fromWidth: width })}>
                {t('shapes.slots.detectedItem', { width: fmt(width, 2), n: count })}
              </button>
            ))}
          </div>
        </div>
      )}
      <LengthField label={t('shapes.slots.to')} value={params.toWidth} min={0.05} max={100} step={0.1} onCommit={(toWidth) => patch({ toWidth })} />
      <p className="hint">{t('shapes.slots.toHint')}</p>
      <Choice
        legend={t('shapes.slots.anchor')}
        value={params.anchor}
        options={[
          { value: 'center', label: t('shapes.slots.anchor.center') },
          { value: 'start', label: t('shapes.slots.anchor.start') },
          { value: 'end', label: t('shapes.slots.anchor.end') },
        ]}
        onChange={(anchor) => patch({ anchor })}
      />
      <Check label={t('shapes.slots.notches')} checked={params.notches} onChange={(notches) => patch({ notches })} />
      <Check label={t('shapes.slots.holes')} checked={params.holes} onChange={(holes) => patch({ holes })} />
      <Check label={t('shapes.slots.tabs')} checked={params.tabs} onChange={(tabs) => patch({ tabs })} />
      {stats && !none && <p className="hint ar-summary" role="status">{t('shapes.slots.preview', { n: stats.count })}</p>}
    </Shell>
  );
}

/** Un componente por diálogo (los exporta para probarlos sin pasar por el store). */
export const SHAPE_DIALOGS: Record<ShapeDialogOp, () => React.JSX.Element> = {
  break_apart: BreakApartDialog,
  optimize: OptimizeDialog,
  fillet: FilletDialog,
  cut_shapes: CutDialog,
  resize_slots: SlotsDialog,
};

/** Dibuja el diálogo de forma abierto, si lo hay (lo monta `ArrangeDialogHost`). */
export function ShapeOpsHost() {
  const dialog = useShapeOps((s) => s.dialog);
  if (!dialog) return null;
  const Component = SHAPE_DIALOGS[dialog];
  return <Component key={dialog} />;
}
