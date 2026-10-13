import { TriangleAlert } from 'lucide-react';
import { useEffect, useMemo, useState } from 'react';
import { getApi } from '@/api';
import type { TangentCircle } from '@/api/types';
import { t } from '@/i18n';
import { errorMessage } from '@/lib/errors';
import { fmt } from '@/lib/platform';
import { NumberField } from '@/ui/NumberField';
import { useStore } from '@/store';
import { Dialog, Footer } from './Dialog';
import { DesignPreview } from './DesignPreview';
import { addCircles, designReason, operandsOf, outlinesOf } from './designOps';
import { useDesignUi } from './designUi';
import { selectionUnits } from './units';

type Found = { status: 'computing' } | { status: 'error'; msg: string } | { status: 'ready'; circles: TangentCircle[] };

const DELAY_MS = 150;

/**
 * Círculo tangente a dos o tres formas o líneas. Con dos hace falta el radio; con tres el radio sale
 * de las formas. Se listan los candidatos y se eligen los que se quieren agregar.
 */
export function TangentDialog() {
  const doc = useStore((s) => s.doc);
  const selectedIds = useStore((s) => s.selectedIds);
  const radius = useDesignUi((s) => s.tangentRadius);
  const setRadius = useDesignUi((s) => s.setTangentRadius);
  const close = useDesignUi((s) => s.close);
  const [found, setFound] = useState<Found>({ status: 'computing' });
  const [off, setOff] = useState<ReadonlySet<number>>(new Set());

  const units = useMemo(() => selectionUnits(doc, selectedIds), [doc, selectedIds]);
  const original = useMemo(() => outlinesOf(units.flatMap((u) => u.objects)), [units]);
  const reason = designReason({ doc, selectedIds }, 2, 3);
  const three = units.length === 3;

  useEffect(() => {
    if (reason !== true) return;
    let current = true;
    setFound({ status: 'computing' });
    setOff(new Set());
    const timer = setTimeout(() => {
      getApi()
        .tangentCircles({ objects: operandsOf(units), radius: three ? null : radius })
        .then((circles) => current && setFound({ status: 'ready', circles }))
        .catch((e: unknown) => current && setFound({ status: 'error', msg: errorMessage(e) }));
    }, DELAY_MS);
    return () => {
      current = false;
      clearTimeout(timer);
    };
  }, [units, radius, three, reason]);

  const circles = found.status === 'ready' ? found.circles : [];
  const chosen = circles.filter((_, i) => !off.has(i));
  const empty = found.status === 'ready' && circles.length === 0;
  const noRadius = !three && !(radius > 0);
  const blocked = reason !== true || found.status !== 'ready' || chosen.length === 0 || noRadius;
  const why =
    reason !== true ? t(reason) : noRadius ? t('design.tangent.noRadius') : found.status === 'error' ? t('design.tangent.error', { msg: found.msg }) : empty ? t('design.tangent.none') : chosen.length === 0 && found.status === 'ready' ? t('design.tangent.pickOne') : null;

  const apply = () => {
    const layer = units[0]?.objects[0]?.layer;
    if (!layer || blocked) return;
    addCircles(useStore.getState(), chosen, layer);
    close();
  };
  const toggle = (i: number) =>
    setOff((cur) => {
      const next = new Set(cur);
      if (!next.delete(i)) next.add(i);
      return next;
    });
  const mm = t('arrange.dlg.mm');

  return (
    <Dialog
      title={t('design.tangent.title')}
      onClose={close}
      wide
      initialFocus=".field__input"
      footer={<Footer onClose={close} onApply={apply} disabled={blocked} reason={why} applyLabel={t('design.tangent.apply', { n: chosen.length })} />}
    >
      <div className="ar-cols">
        <div className="ar-form">
          <NumberField label={t('design.tangent.radius')} value={radius} min={0.01} max={100000} step={1} unit={mm} disabled={three} onCommit={setRadius} />
          <p className="hint">{t(three ? 'design.tangent.hintThree' : 'design.tangent.hintTwo')}</p>
          {circles.length > 0 && (
            <fieldset className="ar-group">
              <legend className="field__label">{t('design.tangent.candidates', { n: circles.length })}</legend>
              <div className="dg-cands">
                {circles.map((c, i) => (
                  <label key={i} className="ar-check">
                    <input type="checkbox" checked={!off.has(i)} onChange={() => toggle(i)} />
                    <span className="num">{t('design.tangent.candidate', { n: i + 1, r: fmt(c.radius), x: fmt(c.center[0]), y: fmt(c.center[1]) })}</span>
                  </label>
                ))}
              </div>
            </fieldset>
          )}
        </div>
        <div className="ar-preview-box">
          <span className="field__label">{t('design.tangent.preview')}</span>
          <DesignPreview
            label={t('design.tangent.previewLabel')}
            layers={[{ outlines: original, tone: 'original' }]}
            circles={circles.map((c, i) => ({ ...c, tone: off.has(i) ? 'ghost' : 'accent' }))}
          />
          {found.status === 'computing' && reason === true && <span className="hint" role="status">{t('arrange.dlg.working')}</span>}
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
