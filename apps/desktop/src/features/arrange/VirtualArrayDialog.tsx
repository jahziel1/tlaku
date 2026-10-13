import { useEffect, useMemo } from 'react';
import { t } from '@/i18n';
import { applyAffine, objectBounds } from '@/lib/geometry';
import { fmt } from '@/lib/platform';
import { MAX_VIRTUAL_COPIES, virtualArrayIssue, virtualCopies } from '@/lib/virtualArray';
import { NumberField } from '@/ui/NumberField';
import { useStore } from '@/store';
import { centerForRadius, DEFAULT_RADIUS } from './arrays';
import { Check, Choice, Dialog } from './Dialog';
import { boxOutline, DesignPreview } from './DesignPreview';
import { flattenArrays, setVirtualArray, virtualTarget } from './designOps';
import { arrayOf, draftOf, useDesignUi } from './designUi';
import { boundsCenter } from './units';

/**
 * Arreglo virtual: copias que no existen como objetos; se dibujan como fantasmas y se generan al
 * enviar el trabajo. Se asigna a un solo objeto, y se puede quitar o convertir en objetos reales.
 */
export function VirtualArrayDialog() {
  const doc = useStore((s) => s.doc);
  const selectedIds = useStore((s) => s.selectedIds);
  const draft = useDesignUi((s) => s.virtual);
  const patch = useDesignUi((s) => s.patchVirtual);
  const close = useDesignUi((s) => s.close);

  const target = useMemo(() => virtualTarget({ doc, selectedIds }), [doc, selectedIds]);
  const object = 'object' in target ? target.object : null;
  const objectId = object?.id ?? null;

  // Al abrir con un objeto que ya tiene arreglo se parte de él; si no, el centro de giro propuesto
  // queda `DEFAULT_RADIUS` mm debajo del objeto.
  useEffect(() => {
    if (!object) return;
    const box = objectBounds(object);
    if (object.array) {
      patch(draftOf(object.array, useDesignUi.getState().virtual));
    } else if (box) {
      const [bx, by] = boundsCenter(box);
      const [centerX, centerY] = centerForRadius(box, { centerX: bx, centerY: by }, DEFAULT_RADIUS);
      patch({ centerX, centerY });
    }
    // Solo cuando cambia el objeto: seguir al formulario pisaría lo que el usuario escribe.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [objectId]);

  const array = useMemo(() => arrayOf(draft), [draft]);
  const problem = virtualArrayIssue(array);
  const copies = useMemo(() => (object ? virtualCopies({ ...object, array }) : []), [object, array]);
  const box = object ? objectBounds(object) : null;
  const outlines = useMemo(() => {
    if (!object || !box) return [];
    const corners: [number, number][] = [box[0], [box[1][0], box[0][1]], box[1], [box[0][0], box[1][1]]];
    return copies.slice(0, 600).map((m) => ({ points: corners.map((c) => applyAffine(m, c)), closed: true }));
  }, [object, box, copies]);

  const total = copies.length + 1;
  const single = problem === null && total === 1;
  const issue = !object ? ('reason' in target ? target.reason : 'arrange.reason.noSelection') : problem ? (`design.virtual.err.${problem}` as const) : single ? 'design.virtual.err.single' : null;
  const mm = t('arrange.dlg.mm');

  const apply = () => {
    if (!object || issue) return;
    setVirtualArray(useStore.getState(), object.id, array);
    close();
  };
  const remove = () => {
    if (!object) return;
    setVirtualArray(useStore.getState(), object.id, null);
    close();
  };
  const flatten = () => {
    if (!object?.array) return;
    const s = useStore.getState();
    const created: string[] = [];
    s.applyDocChange('design.history.virtualFlatten', (d) => {
      created.length = 0;
      const r = flattenArrays(d, new Set([object.id]));
      created.push(...r.created);
      return r.doc;
    });
    s.select([object.id, ...created]);
    close();
  };

  return (
    <Dialog
      title={t('design.virtual.title')}
      onClose={close}
      wide
      initialFocus="[aria-checked='true']"
      footer={
        <>
          {issue && <p className="ar-reason" role="status">{t(issue, { max: MAX_VIRTUAL_COPIES })}</p>}
          {object?.array && (
            <>
              <button type="button" className="btn" onClick={remove}>{t('design.virtual.remove')}</button>
              <button type="button" className="btn" onClick={flatten}>{t('design.virtual.flatten')}</button>
            </>
          )}
          <button type="button" className="btn" onClick={close}>{t('arrange.dlg.cancel')}</button>
          <button type="button" className="btn btn--accent" data-autofocus aria-disabled={issue !== null} title={issue ? t(issue, { max: MAX_VIRTUAL_COPIES }) : undefined} onClick={apply}>
            {t(object?.array ? 'design.virtual.update' : 'design.virtual.apply')}
          </button>
        </>
      }
    >
      <div className="ar-cols">
        <div className="ar-form">
          <Choice<'grid' | 'circular'>
            legend={t('design.virtual.kind')}
            value={draft.kind}
            options={[
              { value: 'grid', label: t('design.virtual.grid') },
              { value: 'circular', label: t('design.virtual.circular') },
            ]}
            onChange={(kind) => patch({ kind })}
          />
          {draft.kind === 'grid' ? (
            <>
              <div className="field-grid">
                <NumberField label={t('arrange.grid.columns')} value={draft.cols} min={1} max={500} step={1} onCommit={(v) => patch({ cols: Math.round(v) })} />
                <NumberField label={t('arrange.grid.rows')} value={draft.rows} min={1} max={500} step={1} onCommit={(v) => patch({ rows: Math.round(v) })} />
              </div>
              <div className="field-grid">
                <NumberField label={t('design.virtual.gapX')} value={draft.dx} min={-10000} max={10000} step={0.5} unit={mm} onCommit={(dx) => patch({ dx })} />
                <NumberField label={t('design.virtual.gapY')} value={draft.dy} min={-10000} max={10000} step={0.5} unit={mm} onCommit={(dy) => patch({ dy })} />
              </div>
              <p className="hint">{t('design.virtual.gapHint')}</p>
            </>
          ) : (
            <>
              <div className="field-grid">
                <NumberField label={t('arrange.circular.copies')} value={draft.count} min={2} max={MAX_VIRTUAL_COPIES} step={1} onCommit={(v) => patch({ count: Math.round(v) })} />
                <NumberField label={t('arrange.circular.angle')} value={draft.sweep} min={-3600} max={3600} step={15} unit="°" onCommit={(sweep) => patch({ sweep })} />
              </div>
              <p className="hint">{t('arrange.circular.angleHint')}</p>
              <div className="field-grid">
                <NumberField label={t('arrange.circular.centerX')} value={draft.centerX} step={1} unit={mm} onCommit={(centerX) => patch({ centerX })} />
                <NumberField label={t('arrange.circular.centerY')} value={draft.centerY} step={1} unit={mm} onCommit={(centerY) => patch({ centerY })} />
              </div>
              <Check label={t('arrange.circular.rotate')} checked={draft.rotate} onChange={(rotate) => patch({ rotate })} />
            </>
          )}
          <p className="hint">{t('design.virtual.hint')}</p>
        </div>
        <div className="ar-preview-box">
          <span className="field__label">{t('design.virtual.preview')}</span>
          <DesignPreview
            label={t('design.virtual.previewLabel')}
            layers={[
              { outlines: outlines, tone: 'ghost' },
              { outlines: box ? [boxOutline(box)] : [], tone: 'accent' },
            ]}
            circles={draft.kind === 'circular' ? [{ center: [draft.centerX, draft.centerY], radius: 0.8, tone: 'warn' }] : []}
          />
          {!problem && object && (
            <p className="ar-stats" role="status">
              <strong className="num">{t('design.virtual.summary', { count: total, extra: total - 1 })}</strong>
            </p>
          )}
          {object && box && <span className="hint num">{t('design.virtual.object', { w: fmt(box[1][0] - box[0][0]), h: fmt(box[1][1] - box[0][1]) })}</span>}
        </div>
      </div>
    </Dialog>
  );
}
