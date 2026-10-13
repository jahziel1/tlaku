import { TriangleAlert } from 'lucide-react';
import { useEffect, useMemo, useState } from 'react';
import { t } from '@/i18n';
import { errorMessage } from '@/lib/errors';
import { LengthField } from '@/ui/LengthField';
import { useStore } from '@/store';
import { Check, Choice, Dialog, Footer } from './Dialog';
import { DesignPreview, type Outline } from './DesignPreview';
import { designReason, outlinesOf, requestHull, runHull } from './designOps';
import { useDesignUi } from './designUi';
import { selectionUnits } from './units';

type Preview = { status: 'computing' } | { status: 'error'; msg: string } | { status: 'ready'; outlines: Outline[] };

const PREVIEW_DELAY_MS = 150;

/**
 * Contorno de banda elástica: una forma cerrada que envuelve la selección. Convexo (el contorno más
 * ajustado sin entrantes) o cóncavo, que se mete en los huecos mientras las aristas midan más que
 * el largo máximo.
 */
export function HullDialog() {
  const doc = useStore((s) => s.doc);
  const selectedIds = useStore((s) => s.selectedIds);
  const activeLayer = useStore((s) => s.activeLayer);
  const params = useDesignUi((s) => s.hull);
  const patch = useDesignUi((s) => s.patchHull);
  const busy = useDesignUi((s) => s.busy);
  const close = useDesignUi((s) => s.close);
  const [preview, setPreview] = useState<Preview>({ status: 'computing' });

  const units = useMemo(() => selectionUnits(doc, selectedIds), [doc, selectedIds]);
  const original = useMemo(() => outlinesOf(units.flatMap((u) => u.objects)), [units]);
  const reason = designReason({ doc, selectedIds });
  const maxEdge = params.concave ? params.maxEdge : null;

  useEffect(() => {
    if (reason !== true) return;
    let current = true;
    setPreview({ status: 'computing' });
    const timer = setTimeout(() => {
      requestHull(units, maxEdge, activeLayer)
        .then((r) => current && setPreview({ status: 'ready', outlines: outlinesOf(r) }))
        .catch((e: unknown) => current && setPreview({ status: 'error', msg: errorMessage(e) }));
    }, PREVIEW_DELAY_MS);
    return () => {
      current = false;
      clearTimeout(timer);
    };
  }, [units, maxEdge, activeLayer, reason]);

  const blocked = reason !== true || busy || preview.status !== 'ready';
  const why = reason !== true ? t(reason) : preview.status === 'error' ? t('design.hull.error', { msg: preview.msg }) : null;
  const apply = async () => {
    if ((await runHull(useStore.getState(), params.concave, params.maxEdge, params.keepOriginal)) !== null) close();
  };

  return (
    <Dialog
      title={t('design.hull.title')}
      onClose={close}
      wide
      initialFocus="[aria-checked='true']"
      footer={<Footer onClose={close} onApply={() => void apply()} disabled={blocked} reason={why} applyLabel={busy ? t('arrange.dlg.working') : t('design.hull.apply')} />}
    >
      <div className="ar-cols">
        <div className="ar-form">
          <Choice<'convex' | 'concave'>
            legend={t('design.hull.kind')}
            value={params.concave ? 'concave' : 'convex'}
            options={[
              { value: 'convex', label: t('design.hull.convex') },
              { value: 'concave', label: t('design.hull.concave') },
            ]}
            onChange={(k) => patch({ concave: k === 'concave' })}
          />
          <LengthField label={t('design.hull.maxEdge')} value={params.maxEdge} min={0.1} max={10000} step={1} disabled={!params.concave} onCommit={(maxEdge) => patch({ maxEdge })} />
          <p className="hint">{t('design.hull.maxEdgeHint')}</p>
          <Check label={t('design.hull.keep')} checked={params.keepOriginal} onChange={(keepOriginal) => patch({ keepOriginal })} />
        </div>
        <div className="ar-preview-box">
          <span className="field__label">{t('design.hull.preview')}</span>
          <DesignPreview
            label={t('design.hull.previewLabel')}
            layers={[
              { outlines: original, tone: 'original' },
              { outlines: preview.status === 'ready' ? preview.outlines : [], tone: 'accent' },
            ]}
          />
          {preview.status === 'computing' && reason === true && <span className="hint" role="status">{t('arrange.dlg.working')}</span>}
          {why && reason === true && (
            <p className="warn-line" role="alert">
              <TriangleAlert size={14} aria-hidden="true" />
              <span>{why}</span>
            </p>
          )}
        </div>
      </div>
    </Dialog>
  );
}
