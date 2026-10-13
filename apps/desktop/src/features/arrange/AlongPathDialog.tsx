import { useMemo } from 'react';
import { t } from '@/i18n';
import { fmt } from '@/lib/platform';
import { applyAffine } from '@/lib/geometry';
import { NumberField } from '@/ui/NumberField';
import { LengthField } from '@/ui/LengthField';
import { useStore } from '@/store';
import { buildAlongPath, contentCenter, pathOfUnit, planAlongPath, samplePath, type AlongMode } from './alongPath';
import { Check, Choice, Dialog, Footer } from './Dialog';
import { boxOutline, DesignPreview } from './DesignPreview';
import { designReason } from './designOps';
import { useDesignUi } from './designUi';
import { unitLabel } from './unitLabel';
import { blockBounds, selectionUnits } from './units';

/**
 * Copiar a lo largo de un trazo: se elige cuál de las formas seleccionadas es el trayecto y el resto
 * se copia sobre él, por número de copias o por distancia, con o sin giro según la tangente.
 */
export function AlongPathDialog() {
  const doc = useStore((s) => s.doc);
  const selectedIds = useStore((s) => s.selectedIds);
  const params = useDesignUi((s) => s.along);
  const patch = useDesignUi((s) => s.patchAlong);
  const pathId = useDesignUi((s) => s.alongPathId);
  const setPathId = useDesignUi((s) => s.setAlongPath);
  const close = useDesignUi((s) => s.close);

  const units = useMemo(() => selectionUnits(doc, selectedIds), [doc, selectedIds]);
  // Por omisión el trayecto es la última forma elegida (se dibuja después de lo que se copia).
  const pathUnit = units.find((u) => u.id === pathId) ?? units[units.length - 1];
  const content = useMemo(() => units.filter((u) => u !== pathUnit), [units, pathUnit]);
  const raw = useMemo(() => (pathUnit ? pathOfUnit(pathUnit) : null), [pathUnit]);
  const path = useMemo(() => (raw ? samplePath(raw.points, raw.closed) : null), [raw]);
  const ref = useMemo(() => contentCenter(content), [content]);
  const hasImage = content.some((u) => u.objects.some((o) => o.kind.type === 'image'));
  const plan = useMemo(() => (ref ? planAlongPath(ref, path, params) : null), [ref, path, params]);
  const block = useMemo(() => blockBounds(content), [content]);

  const reason = designReason({ doc, selectedIds }, 2);
  const issue = reason !== true ? reason : !plan ? 'design.reason.needMore' : hasImage && params.rotate ? 'design.along.err.imageRotate' : plan.issue;
  const blocked = issue !== null;

  const apply = () => {
    if (issue !== null || !plan) return;
    const s = useStore.getState();
    const created: string[] = [];
    s.applyDocChange('design.history.along', (d) => {
      created.length = 0;
      const r = buildAlongPath(d, content, plan.matrices, params.keepOriginal);
      created.push(...r.created);
      return r.doc;
    });
    s.select(created);
    close();
  };

  const outlines = useMemo(() => {
    if (!plan || !block) return [];
    const corners = [block[0], [block[1][0], block[0][1]], block[1], [block[0][0], block[1][1]]] as [number, number][];
    return plan.matrices.slice(0, 400).map((m) => ({ points: corners.map((c) => applyAffine(m, c)), closed: true }));
  }, [plan, block]);

  return (
    <Dialog
      title={t('design.along.title')}
      onClose={close}
      wide
      initialFocus="[aria-checked='true']"
      footer={<Footer onClose={close} onApply={apply} disabled={blocked} reason={issue ? t(issue) : null} applyLabel={t('design.along.apply')} />}
    >
      <div className="ar-cols">
        <div className="ar-form">
          <div className="ar-list" role="radiogroup" aria-label={t('design.along.pathPick')}>
            {units.map((u) => (
              <button key={u.id} type="button" role="radio" aria-checked={u === pathUnit} className="ar-item" onClick={() => setPathId(u.id)}>
                <span className="ar-item__role" aria-hidden="true">{u === pathUnit ? t('design.along.rolePath') : t('design.along.roleCopy')}</span>
                <span className="ar-item__name">{unitLabel(u)}</span>
              </button>
            ))}
          </div>
          <Choice<AlongMode>
            legend={t('design.along.mode')}
            value={params.mode}
            options={[
              { value: 'count', label: t('design.along.mode.count') },
              { value: 'spacing', label: t('design.along.mode.spacing') },
            ]}
            onChange={(mode) => patch({ mode })}
          />
          {params.mode === 'count' ? (
            <NumberField label={t('design.along.count')} value={params.count} min={1} max={2000} step={1} onCommit={(v) => patch({ count: Math.round(v) })} />
          ) : (
            <LengthField label={t('design.along.spacing')} value={params.spacing} min={0.01} max={100000} step={1} onCommit={(spacing) => patch({ spacing })} />
          )}
          <Check label={t('design.along.rotate')} checked={params.rotate} onChange={(rotate) => patch({ rotate })} />
          <NumberField label={t('design.along.angle')} value={params.angle} min={-360} max={360} step={15} unit="°" onCommit={(angle) => patch({ angle })} />
          <p className="hint">{t('design.along.angleHint')}</p>
          <Check label={t('design.along.keep')} checked={params.keepOriginal} onChange={(keepOriginal) => patch({ keepOriginal })} />
        </div>
        <div className="ar-preview-box">
          <span className="field__label">{t('design.along.preview')}</span>
          <DesignPreview
            label={t('design.along.previewLabel')}
            layers={[
              { outlines: raw ? [{ points: raw.points, closed: raw.closed }] : [], tone: 'accent' },
              { outlines: outlines, tone: 'result' },
              { outlines: block ? [boxOutline(block)] : [], tone: 'original' },
            ]}
          />
          {plan && !plan.issue && (
            <p className="ar-stats" role="status">
              <strong className="num">{t('design.along.summary', { n: plan.matrices.length })}</strong>
              {path && <span className="num">{t('design.along.length', { len: fmt(path.length) })}</span>}
            </p>
          )}
        </div>
      </div>
    </Dialog>
  );
}
