import { TriangleAlert } from 'lucide-react';
import { useEffect, useMemo, useState } from 'react';
import type { DesignObject, Point } from '@/api/types';
import { t } from '@/i18n';
import { errorMessage } from '@/lib/errors';
import { NumberField } from '@/ui/NumberField';
import { LengthField } from '@/ui/LengthField';
import { useStore } from '@/store';
import { Dialog, Footer } from './Dialog';
import { DesignPreview, type Outline } from './DesignPreview';
import { designReason, outlinesOf, requestWarp, runWarp } from './designOps';
import { useDesignUi } from './designUi';
import { blockBounds, selectionUnits } from './units';
import { isNoWarp, NO_WARP, presetParams, WARP_PRESETS, type WarpParams, type WarpPreset } from './warpPresets';

type Preview = { status: 'computing' } | { status: 'error'; msg: string } | { status: 'ready'; outlines: Outline[] };

const PREVIEW_DELAY_MS = 150;
const CORNER_KEYS = ['bl', 'br', 'tr', 'tl'] as const;
const EDGE_KEYS = ['bottom', 'right', 'top', 'left'] as const;

/**
 * Deformar: la selección se mete en una envolvente de cuatro esquinas y cuatro bordes curvos. Los
 * presets (arco, abombar, pellizcar, perspectiva, sesgo) rellenan los números, que se pueden
 * afinar uno por uno. La vista previa viene del motor, con la deformación real de las curvas.
 */
export function WarpDialog() {
  const doc = useStore((s) => s.doc);
  const selectedIds = useStore((s) => s.selectedIds);
  const { params, amount } = useDesignUi((s) => s.warp);
  const patch = useDesignUi((s) => s.patchWarp);
  const busy = useDesignUi((s) => s.busy);
  const close = useDesignUi((s) => s.close);
  const [preset, setPreset] = useState<WarpPreset | null>(null);
  const [preview, setPreview] = useState<Preview>({ status: 'computing' });

  const units = useMemo(() => selectionUnits(doc, selectedIds), [doc, selectedIds]);
  const bounds = useMemo(() => blockBounds(units), [units]);
  const original = useMemo(() => outlinesOf(units.flatMap((u) => u.objects)), [units]);
  const reason = designReason({ doc, selectedIds });
  const noop = isNoWarp(params);

  const choose = (p: WarpPreset, a = amount) => {
    if (!bounds) return;
    setPreset(p);
    patch({ params: presetParams(p, a, bounds), amount: a });
  };
  const edit = (next: WarpParams) => {
    setPreset(null);
    patch({ params: next });
  };

  // Vista previa con retardo: cada cambio cancela el cálculo anterior (solo vale el último).
  useEffect(() => {
    if (reason !== true || noop) return;
    let current = true;
    setPreview({ status: 'computing' });
    const timer = setTimeout(() => {
      requestWarp(units, params)
        .then((r: DesignObject[]) => current && setPreview({ status: 'ready', outlines: outlinesOf(r) }))
        .catch((e: unknown) => current && setPreview({ status: 'error', msg: errorMessage(e) }));
    }, PREVIEW_DELAY_MS);
    return () => {
      current = false;
      clearTimeout(timer);
    };
  }, [units, params, reason, noop]);

  const blocked = reason !== true || busy || noop || preview.status !== 'ready';
  const why = reason !== true ? t(reason) : noop ? t('design.warp.noop') : preview.status === 'error' ? t('design.warp.error', { msg: preview.msg }) : null;
  // El modo de edición del lienzo (malla de control con manijas) sustituye a este diálogo.
  const editOnCanvas = () => {
    close();
    useStore.getState().setTool('warp');
  };
  const apply = async () => {
    if (await runWarp(useStore.getState(), params)) close();
  };
  const setCorner = (i: number, axis: 0 | 1, v: number) => {
    const corners = params.corners.map((c) => [...c] as Point) as WarpParams['corners'];
    corners[i]![axis] = v;
    edit({ ...params, corners });
  };
  const setBulge = (i: number, v: number) => {
    const bulge = [...params.bulge] as WarpParams['bulge'];
    bulge[i] = v;
    edit({ ...params, bulge });
  };

  return (
    <Dialog
      title={t('design.warp.title')}
      onClose={close}
      wide
      initialFocus="[aria-pressed]"
      footer={<Footer onClose={close} onApply={() => void apply()} disabled={blocked} reason={why} applyLabel={busy ? t('arrange.dlg.working') : t('design.warp.apply')} />}
    >
      <div className="ar-cols">
        <div className="ar-form">
          <div className="ar-choice">
            <span className="field__label">{t('design.warp.presets')}</span>
            <div className="dg-presets">
              {WARP_PRESETS.map((p) => (
                <button key={p} type="button" className="chip" aria-pressed={preset === p} onClick={() => choose(p)}>
                  {t(`design.warp.preset.${p}`)}
                </button>
              ))}
              <button type="button" className="chip" onClick={() => { setPreset(null); patch({ params: NO_WARP }); }}>
                {t('design.warp.reset')}
              </button>
            </div>
          </div>
          <NumberField label={t('design.warp.amount')} value={amount} min={0} max={100} step={5} unit="%" onCommit={(a) => (preset ? choose(preset, a) : patch({ amount: a }))} />
          <p className="hint">{t('design.warp.hint')}</p>
          <button type="button" className="btn" disabled={reason !== true} title={t('design.warp.onCanvasHint')} onClick={editOnCanvas}>
            {t('design.warp.onCanvas')}
          </button>
          <details className="dg-advanced">
            <summary>{t('design.warp.advanced')}</summary>
            <p className="hint">{t('design.warp.cornersHint')}</p>
            <div className="dg-grid4">
              {CORNER_KEYS.map((k, i) => (
                <div key={k} className="field-grid">
                  <LengthField label={t(`design.warp.corner.${k}.x`)} value={params.corners[i]![0]} step={0.5} onCommit={(v) => setCorner(i, 0, v)} />
                  <LengthField label={t(`design.warp.corner.${k}.y`)} value={params.corners[i]![1]} step={0.5} onCommit={(v) => setCorner(i, 1, v)} />
                </div>
              ))}
            </div>
            <p className="hint">{t('design.warp.bulgeHint')}</p>
            <div className="dg-grid4">
              {EDGE_KEYS.map((k, i) => (
                <LengthField key={k} label={t(`design.warp.bulge.${k}`)} value={params.bulge[i]!} step={0.5} onCommit={(v) => setBulge(i, v)} />
              ))}
            </div>
          </details>
        </div>
        <div className="ar-preview-box">
          <span className="field__label">{t('design.warp.preview')}</span>
          <DesignPreview
            label={t('design.warp.previewLabel')}
            layers={[
              { outlines: original, tone: 'original' },
              { outlines: preview.status === 'ready' && !noop ? preview.outlines : [], tone: 'result' },
            ]}
          />
          {preview.status === 'computing' && reason === true && !noop && <span className="hint" role="status">{t('arrange.dlg.working')}</span>}
          {why && reason === true && !noop && (
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
