import { useEffect, useMemo, useState } from 'react';
import { getApi } from '@/api';
import type { DesignObject, GeometryOp, Point } from '@/api/types';
import { t, type I18nKey } from '@/i18n';
import { errorMessage } from '@/lib/errors';
import { pointsBounds } from '@/lib/geometry';
import { fmt } from '@/lib/platform';
import { useStore } from '@/store';
import { Dialog } from './Dialog';
import { Thumb, type Outline } from './DesignPreview';
import { designReason, outlinesOf } from './designOps';
import { useDesignUi } from './designUi';
import { geometryReason, operandOf, orderedUnits, runGeometry } from './geomOps';
import { unitLabel, unitThumbnail } from './unitLabel';
import { selectionUnits, unitSize, type Unit } from './units';

/** Las cinco operaciones del asistente, en el orden de las tarjetas. */
export const ASSISTANT_OPS = ['union', 'difference', 'intersection', 'xor', 'weld'] as const satisfies readonly GeometryOp[];
type AssistantOp = (typeof ASSISTANT_OPS)[number];

type Result = { status: 'computing' } | { status: 'error'; msg: string } | { status: 'ready'; outlines: Outline[]; count: number };

/** Miniatura del contorno de una unidad (para elegir cuál es A). */
function UnitThumb({ unit }: { unit: Unit }) {
  const thumb = useMemo(() => unitThumbnail(unit), [unit]);
  const color = unit.objects[0]!.layer;
  if (!thumb) return <span className="ar-swatch" style={{ background: color }} aria-hidden="true" />;
  return (
    <svg className="ar-thumb" viewBox={`0 0 ${thumb.width} ${thumb.height}`} preserveAspectRatio="xMidYMid meet" aria-hidden="true">
      <path d={thumb.d} fill="none" stroke={color} strokeWidth="6" strokeLinejoin="round" strokeLinecap="round" vectorEffect="non-scaling-stroke" />
    </svg>
  );
}

/**
 * Asistente booleano: muestra el resultado de las cinco operaciones sobre la selección actual,
 * cada una en miniatura, y se aplica con un clic en la elegida. La forma principal (A) se elige
 * arriba; el resto forma B. Reutiliza `geometry_op` del motor, el mismo que los comandos del menú.
 */
export function BooleanAssistantDialog() {
  const doc = useStore((s) => s.doc);
  const selectedIds = useStore((s) => s.selectedIds);
  const primaryId = useDesignUi((s) => s.assistantPrimaryId);
  const setPrimary = useDesignUi((s) => s.setAssistantPrimary);
  const busy = useDesignUi((s) => s.busy);
  const close = useDesignUi((s) => s.close);
  const [results, setResults] = useState<Partial<Record<AssistantOp, Result>>>({});

  const units = useMemo(() => selectionUnits(doc, selectedIds), [doc, selectedIds]);
  const ordered = useMemo(() => orderedUnits(units, primaryId), [units, primaryId]);
  const primary = ordered[0];
  const reason = designReason({ doc, selectedIds }, 2, Infinity, false);
  const original = useMemo(() => outlinesOf(units.flatMap((u) => u.objects)), [units]);
  // Una caja común para que las cinco miniaturas se comparen a la misma escala.
  const box = useMemo<[Point, Point]>(() => pointsBounds(original.flatMap((o) => o.points)) ?? [[0, 0], [1, 1]], [original]);

  useEffect(() => {
    if (reason !== true) return;
    let current = true;
    setResults({});
    const objects = ordered.map(operandOf);
    for (const op of ASSISTANT_OPS) {
      getApi()
        .geometryOp({ op, objects })
        .then((r: DesignObject[]) => current && setResults((cur) => ({ ...cur, [op]: { status: 'ready', outlines: outlinesOf(r), count: r.length } })))
        .catch((e: unknown) => current && setResults((cur) => ({ ...cur, [op]: { status: 'error', msg: errorMessage(e) } })));
    }
    return () => {
      current = false;
    };
  }, [ordered, reason]);

  const choose = async (op: AssistantOp) => {
    const s = useStore.getState();
    if (busy || geometryReason(s, op) !== true) return;
    if (await runGeometry(s, { op, primaryId: primary?.id })) close();
  };

  return (
    <Dialog
      title={t('design.assistant.title')}
      onClose={close}
      wide
      initialFocus=".dg-card"
      footer={
        <>
          {reason !== true && <p className="ar-reason" role="status">{t(reason)}</p>}
          <button type="button" className="btn" onClick={close}>{t('arrange.dlg.cancel')}</button>
        </>
      }
    >
      <p className="hint">{t('design.assistant.hint')}</p>
      <div className="ar-list" role="radiogroup" aria-label={t('arrange.bool.primary')}>
        {units.map((u) => {
          const isA = u === primary;
          const [w, h] = unitSize(u);
          return (
            <button key={u.id} type="button" role="radio" aria-checked={isA} className="ar-item" onClick={() => setPrimary(u.id)}>
              <span className="ar-item__role" aria-hidden="true">{isA ? t('arrange.bool.roleA') : t('arrange.bool.roleB')}</span>
              <UnitThumb unit={u} />
              <span className="ar-item__name">{unitLabel(u)}</span>
              <span className="num ar-item__meta">{t('arrange.bool.size', { w: fmt(w), h: fmt(h) })}</span>
            </button>
          );
        })}
      </div>
      <div className="dg-cards">
        {ASSISTANT_OPS.map((op) => {
          const r = results[op];
          const empty = r?.status === 'ready' && r.outlines.length === 0;
          const disabled = reason !== true || busy || r?.status !== 'ready' || empty;
          return (
            <button key={op} type="button" className="dg-card" aria-disabled={disabled} title={r?.status === 'error' ? r.msg : undefined} onClick={() => !disabled && void choose(op)}>
              {r?.status === 'ready' && !empty ? (
                <Thumb outlines={r.outlines} box={box} label={t('design.assistant.thumbLabel', { op: t(`design.assistant.op.${op}` as I18nKey) })} />
              ) : (
                <span className="dg-empty">{r?.status === 'error' ? t('design.assistant.failed') : empty ? t('design.assistant.empty') : t('arrange.dlg.working')}</span>
              )}
              <span className="dg-card__title">{t(`design.assistant.op.${op}` as I18nKey)}</span>
              <span className="dg-card__hint">{t(`design.assistant.desc.${op}` as I18nKey)}</span>
              {r?.status === 'ready' && !empty && <span className="dg-card__hint num">{t('design.assistant.pieces', { n: r.count })}</span>}
            </button>
          );
        })}
      </div>
    </Dialog>
  );
}
