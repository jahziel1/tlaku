import { TriangleAlert } from 'lucide-react';
import { useMemo } from 'react';
import { t } from '@/i18n';
import { fmt } from '@/lib/platform';
import { NumberField } from '@/ui/NumberField';
import { selectActiveDevice, useStore } from '@/store';
import { bedBounds } from './align';
import {
  buildArray, centerForRadius, MAX_ARRAY_PIECES, planCircular, planGrid, radiusOf, type ArrayPlan, type CircularParams, type GridParams,
} from './arrays';
import { Check, Choice, Dialog, Footer } from './Dialog';
import { useArrangeUi } from './uiStore';
import { blockBounds, selectionUnits } from './units';

const MAX_DRAWN = 600;

/** Cama y cajas de cada pieza; la original va resaltada y, si algo se sale de la cama, el arreglo se pinta en aviso. */
function LayoutPreview({ plan, bed, center }: { plan: ArrayPlan; bed: readonly [number, number]; center?: [number, number] }) {
  const xs = [0, bed[0], plan.bounds[0][0], plan.bounds[1][0], center?.[0] ?? 0];
  const ys = [0, bed[1], plan.bounds[0][1], plan.bounds[1][1], center?.[1] ?? 0];
  const [x0, x1, y0, y1] = [Math.min(...xs), Math.max(...xs), Math.min(...ys), Math.max(...ys)];
  const pad = Math.max(x1 - x0, y1 - y0) * 0.04;
  const poly = (pts: [number, number][]) => pts.map(([x, y]) => `${x.toFixed(2)},${y.toFixed(2)}`).join(' ');
  return (
    <svg
      className="ar-preview"
      role="img"
      aria-label={t('arrange.array.previewLabel')}
      viewBox={`${x0 - pad} ${-(y1 + pad)} ${x1 - x0 + 2 * pad} ${y1 - y0 + 2 * pad}`}
      preserveAspectRatio="xMidYMid meet"
    >
      <g transform="scale(1 -1)">
        <rect className="ar-preview__bed" x={0} y={0} width={bed[0]} height={bed[1]} />
        {plan.outlines.slice(0, MAX_DRAWN).map((o, i) => (
          <polygon key={i} className={`ar-preview__cell${i === 0 ? ' ar-preview__cell--origin' : ''}${plan.outOfBed ? ' ar-preview__cell--warn' : ''}`} points={poly(o)} />
        ))}
        {center && <circle className="ar-preview__center" cx={center[0]} cy={center[1]} r={Math.max(x1 - x0, y1 - y0) * 0.012} />}
      </g>
    </svg>
  );
}

/** Arreglo en rejilla o circular con vista previa del número de piezas, el tamaño total y la cama. */
export function ArrayDialog({ kind }: { kind: 'grid' | 'circular' }) {
  const doc = useStore((s) => s.doc);
  const selectedIds = useStore((s) => s.selectedIds);
  const bed = useStore((s) => selectActiveDevice(s).bed);
  const grid = useArrangeUi((s) => s.grid);
  const circular = useArrangeUi((s) => s.circular);
  const patchGrid = useArrangeUi((s) => s.patchGrid);
  const patchCircular = useArrangeUi((s) => s.patchCircular);
  const close = useArrangeUi((s) => s.close);

  const units = useMemo(() => selectionUnits(doc, selectedIds), [doc, selectedIds]);
  const block = useMemo(() => blockBounds(units), [units]);
  const plan = useMemo(
    () => (block ? (kind === 'grid' ? planGrid(grid, block, bedBounds(bed)) : planCircular(circular, block, bedBounds(bed))) : null),
    [block, kind, grid, circular, bed],
  );
  const group = kind === 'grid' ? grid.group : circular.group;
  const issue = !block || !plan ? 'arrange.reason.noSelection' : plan.issue;

  const apply = () => {
    if (!plan || issue) return;
    const s = useStore.getState();
    const created: string[] = [];
    s.applyDocChange(kind === 'grid' ? 'arrange.history.grid' : 'arrange.history.circular', (d) => {
      created.length = 0;
      const r = buildArray(d, units, plan.matrices, group);
      created.push(...r.created);
      return r.doc;
    });
    s.select([...units.flatMap((u) => u.ids), ...created]);
    close();
  };

  const approx = kind === 'circular' && circular.rotate;
  const center: [number, number] | undefined = kind === 'circular' ? [circular.centerX, circular.centerY] : undefined;

  return (
    <Dialog
      title={t(kind === 'grid' ? 'arrange.grid.title' : 'arrange.circular.title')}
      onClose={close}
      wide
      initialFocus=".field__input"
      footer={<Footer onClose={close} onApply={apply} disabled={issue !== null} reason={issue ? t(issue, { max: MAX_ARRAY_PIECES }) : null} applyLabel={t('arrange.array.create')} />}
    >
      <div className="ar-cols">
        <div className="ar-form">
          {kind === 'grid' ? (
            <GridFields params={grid} patch={patchGrid} />
          ) : (
            <CircularFields params={circular} patch={patchCircular} block={block} />
          )}
          <Check label={t('arrange.array.group')} checked={group} onChange={(v) => (kind === 'grid' ? patchGrid({ group: v }) : patchCircular({ group: v }))} />
        </div>
        <div className="ar-preview-box">
          <span className="field__label">{t('arrange.array.preview')}</span>
          {plan && <LayoutPreview plan={plan} bed={bed} center={center} />}
          {plan && !plan.issue && (
            <p className="ar-stats" role="status">
              <strong className="num">{t('arrange.array.summary', { count: plan.count, extra: plan.count - 1 })}</strong>
              <span className="num">{t(approx ? 'arrange.array.sizeApprox' : 'arrange.array.size', { w: fmt(plan.size[0]), h: fmt(plan.size[1]) })}</span>
            </p>
          )}
          {plan && !plan.issue && plan.outOfBed && (
            <p className="warn-line" role="alert">
              <TriangleAlert size={14} aria-hidden="true" />
              <span>{t('arrange.array.outOfBed', { bw: fmt(bed[0], 0), bh: fmt(bed[1], 0) })}</span>
            </p>
          )}
        </div>
      </div>
    </Dialog>
  );
}

function GridFields({ params, patch }: { params: GridParams; patch: (p: Partial<GridParams>) => void }) {
  const mm = t('arrange.dlg.mm');
  return (
    <>
      <div className="field-grid">
        <NumberField label={t('arrange.grid.columns')} value={params.columns} min={1} max={500} step={1} onCommit={(v) => patch({ columns: Math.round(v) })} />
        <NumberField label={t('arrange.grid.rows')} value={params.rows} min={1} max={500} step={1} onCommit={(v) => patch({ rows: Math.round(v) })} />
      </div>
      <Choice<GridParams['mode']>
        legend={t('arrange.grid.spacing')}
        value={params.mode}
        options={[
          { value: 'gap', label: t('arrange.grid.mode.gap') },
          { value: 'pitch', label: t('arrange.grid.mode.pitch') },
        ]}
        onChange={(mode) => patch({ mode })}
      />
      <div className="field-grid">
        <NumberField label={t('arrange.grid.horizontal')} value={params.spacingX} min={-10000} max={10000} step={0.5} unit={mm} onCommit={(spacingX) => patch({ spacingX })} />
        <NumberField label={t('arrange.grid.vertical')} value={params.spacingY} min={-10000} max={10000} step={0.5} unit={mm} onCommit={(spacingY) => patch({ spacingY })} />
      </div>
    </>
  );
}

function CircularFields({ params, patch, block }: { params: CircularParams; patch: (p: Partial<CircularParams>) => void; block: ReturnType<typeof blockBounds> }) {
  const mm = t('arrange.dlg.mm');
  const radius = block ? radiusOf(block, params) : 0;
  return (
    <>
      <div className="field-grid">
        <NumberField label={t('arrange.circular.copies')} value={params.copies} min={2} max={MAX_ARRAY_PIECES} step={1} onCommit={(v) => patch({ copies: Math.round(v) })} />
        <NumberField label={t('arrange.circular.angle')} value={params.angle} min={-3600} max={3600} step={15} unit="°" onCommit={(angle) => patch({ angle })} />
      </div>
      <p className="hint">{t('arrange.circular.angleHint')}</p>
      <NumberField
        label={t('arrange.circular.radius')}
        value={Math.round(radius * 1000) / 1000}
        min={0}
        max={10000}
        step={1}
        unit={mm}
        onCommit={(r) => {
          if (!block) return;
          const [centerX, centerY] = centerForRadius(block, params, r);
          patch({ centerX, centerY });
        }}
      />
      <div className="field-grid">
        <NumberField label={t('arrange.circular.centerX')} value={params.centerX} step={1} unit={mm} onCommit={(centerX) => patch({ centerX })} />
        <NumberField label={t('arrange.circular.centerY')} value={params.centerY} step={1} unit={mm} onCommit={(centerY) => patch({ centerY })} />
      </div>
      <p className="hint">{t('arrange.circular.centerHint')}</p>
      <Check label={t('arrange.circular.rotate')} checked={params.rotate} onChange={(rotate) => patch({ rotate })} />
    </>
  );
}

