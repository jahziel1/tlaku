import { Bold, Eye, Italic, Settings2, X } from 'lucide-react';
import { useMemo } from 'react';
import { createPortal } from 'react-dom';
import { t } from '@/i18n';
import { useStore } from '@/store';
import { objectsByIds } from '@/store/objectIndex';
import { editText } from '@/features/text/convert';
import { FontPicker } from '@/features/text/FontPicker';
import { isLineFamily, pickDefaultFont, useFonts } from '@/features/text/fonts';
import { ImagePreviewPanel } from '@/features/images/ImagePreviewPanel';
import { ImageVariableField } from '@/features/variables/ImageVariableField';
import { usePreviewUi } from '@/features/images/previewStore';
import { openEditorFor } from '@/features/layers/actions';
import { useLayerUi } from '@/features/layers/layerUi';
import type { FontFamily, TextObject } from '@/api/types';
import { formatDpi, imageInfo, type ImageInfo } from './imageProps';
import { ExprField } from './ExprField';
import { commonValue, setCutProps, type CutPatch } from './cutProps';
import { OptionalField } from './OptionalField';
import { editShape, effectiveShape, type ShapeSpec, type ShapeType } from '@/lib/paramShape';

const TITLES: Record<ShapeType, Parameters<typeof t>[0]> = {
  rect: 'canvas.shape.rect',
  ellipse: 'canvas.shape.ellipse',
  polygon: 'canvas.shape.polygon',
  star: 'canvas.shape.star',
};

/** Prioridad y escala de potencia de la selección (valor común o `'mixed'`). */
export interface CutValues {
  priority: number | null | 'mixed';
  power: number | null | 'mixed';
}

interface ViewProps {
  /** Parámetros de la forma seleccionada, o `null` si la selección no son formas del mismo tipo. */
  params: ShapeSpec | null;
  /** Hay seleccionados objetos que no son formas editables. */
  mixed: boolean;
  /** Propiedades de corte de la selección. */
  cut?: CutValues;
  /** Texto seleccionado (un solo objeto de texto): contenido, fuente, tamaño y espaciados. */
  text?: Pick<TextObject, 'text' | 'font_family' | 'size_mm' | 'letter_spacing_mm' | 'line_spacing' | 'bold' | 'italic'>;
  /** Fuentes instaladas para el selector (`fallback` es la que se usa si no hay una elegida). */
  fonts?: { families: FontFamily[]; loading: boolean; fallback: string };
  onText?: (patch: Partial<TextObject>, field: string) => void;
  /** Imagen seleccionada (una sola): tamaño, resolución, modo y acceso a sus ajustes y su vista previa. */
  image?: ImageInfo;
  previewOpen?: boolean;
  onImageSettings?: () => void;
  onImagePreview?: () => void;
  onApply: (patch: Partial<ShapeSpec>) => void;
  onCut?: (patch: CutPatch) => void;
  onClose: () => void;
  onLock: () => void;
}

/** Contenido de la ventana: los campos de cada tipo de forma. */
export function ShapePropsView({ params: first, mixed, cut, text, fonts, onText, image, previewOpen, onImageSettings, onImagePreview, onApply, onCut, onClose, onLock }: ViewProps) {
  const field = (key: string, label: string, value: number, unit?: string, hintKey?: Parameters<typeof t>[0]) => (
    <ExprField
      key={key}
      label={label}
      unit={unit}
      value={value}
      title={hintKey ? t(hintKey) : undefined}
      onApply={(v) => onApply({ [key]: key === 'ratio' ? v / 100 : v } as Partial<ShapeSpec>)}
    />
  );
  return (
    <section className="shape-props" aria-label={first ? t('canvas.shape.title') : t('canvas.obj.title')}>
      <header className="shape-props__head">
        <h2 className="shape-props__title">{first ? t(TITLES[first.type]) : t('canvas.obj.title')}</h2>
        <button type="button" className="icon-btn" title={t('canvas.shape.close')} aria-label={t('canvas.shape.close')} onClick={onClose}>
          <X size={14} aria-hidden="true" />
        </button>
      </header>
      {first && (
      <div className="shape-props__grid">
        {first.type === 'rect' && (
          <>
            {field('w', t('canvas.bar.w'), first.w, 'mm')}
            {field('h', t('canvas.bar.h'), first.h, 'mm')}
            {field('radius', t('canvas.bar.radius'), first.radius, 'mm', 'canvas.shape.radiusHint')}
          </>
        )}
        {first.type === 'ellipse' && (
          <>
            {field('w', t('canvas.bar.w'), first.w, 'mm')}
            {field('h', t('canvas.bar.h'), first.h, 'mm')}
          </>
        )}
        {first.type === 'polygon' && (
          <>
            {field('sides', t('canvas.bar.sides'), first.sides, undefined, 'canvas.bar.sidesHint')}
            {field('radius', t('canvas.shape.radius'), first.radius, 'mm', 'canvas.shape.radiusPolyHint')}
            {field('rounding', t('canvas.bar.rounding'), first.rounding, 'mm', 'canvas.bar.roundingHint')}
          </>
        )}
        {first.type === 'star' && (
          <>
            {field('points', t('canvas.bar.points'), first.points, undefined, 'canvas.bar.pointsHint')}
            {field('outer', t('canvas.shape.outer'), first.outer, 'mm')}
            {field('ratio', t('canvas.bar.inner'), Math.round(first.ratio * 1000) / 10, '%', 'canvas.bar.innerHint')}
            {field('rounding', t('canvas.bar.rounding'), first.rounding, 'mm', 'canvas.bar.roundingHint')}
          </>
        )}
      </div>
      )}
      {text && onText && (
        <>
          <h3 className="shape-props__sub">{t('canvas.obj.text')}</h3>
          <div className="field">
            <label className="field__label" htmlFor="shape-props-text">{t('text.field.text')}</label>
            <textarea
              id="shape-props-text"
              className="input shape-props__area"
              rows={2}
              value={text.text}
              placeholder={t('text.field.textPlaceholder')}
              spellCheck={false}
              onChange={(e) => onText({ text: e.target.value }, 'text')}
              onKeyDown={(e) => {
                if (e.key === 'Escape') e.currentTarget.blur();
              }}
            />
          </div>
          {fonts && (
            <div className="field">
              <span className="field__label">{t('text.field.font')}</span>
              <FontPicker value={text.font_family} fallback={fonts.fallback} families={fonts.families} loading={fonts.loading} onPick={(family) => onText({ font_family: family }, 'font')} />
              <div className="shape-props__row">
                <button type="button" className="icon-btn icon-btn--boxed" aria-pressed={text.bold} disabled={isLineFamily(fonts.families, text.font_family.trim() || fonts.fallback)} title={t('text.style.bold')} aria-label={t('text.style.bold')} onClick={() => onText({ bold: !text.bold }, 'bold')}>
                  <Bold size={15} aria-hidden="true" />
                </button>
                <button type="button" className="icon-btn icon-btn--boxed" aria-pressed={text.italic} disabled={isLineFamily(fonts.families, text.font_family.trim() || fonts.fallback)} title={t('text.style.italic')} aria-label={t('text.style.italic')} onClick={() => onText({ italic: !text.italic }, 'italic')}>
                  <Italic size={15} aria-hidden="true" />
                </button>
              </div>
            </div>
          )}
          <div className="shape-props__grid">
            <ExprField label={t('text.field.size')} unit="mm" value={text.size_mm} onApply={(v) => v > 0 && onText({ size_mm: v }, 'size')} />
            <ExprField label={t('text.field.letterSpacing')} unit="mm" value={text.letter_spacing_mm} onApply={(v) => onText({ letter_spacing_mm: v }, 'spacing')} />
            <ExprField label={t('text.field.lineSpacing')} unit="×" value={text.line_spacing} onApply={(v) => v > 0 && onText({ line_spacing: v }, 'lines')} />
          </div>
          <p className="shape-props__note">{t('canvas.obj.textHelp')}</p>
        </>
      )}
      {image && (
        <>
          <h3 className="shape-props__sub">{t('canvas.obj.image')}</h3>
          <dl className="shape-props__facts">
            <dt>{t('canvas.obj.imagePixels')}</dt>
            <dd>{t('canvas.obj.imagePixelsValue', { w: image.widthPx, h: image.heightPx })}</dd>
            <dt>{t('canvas.obj.imageSize')}</dt>
            <dd>{t('canvas.obj.imageSizeValue', { w: image.widthMm.toFixed(2), h: image.heightMm.toFixed(2) })}</dd>
            <dt>{t('canvas.obj.imageDpi')}</dt>
            <dd>{Math.abs(image.dpiX - image.dpiY) <= Math.max(image.dpiX, image.dpiY) * 0.01 ? formatDpi(image.dpiX) : `${formatDpi(image.dpiX)} × ${formatDpi(image.dpiY)}`}</dd>
            <dt>{t('canvas.obj.imageLayer')}</dt>
            <dd>{image.layerName || image.layerColor}</dd>
            <dt>{t('canvas.obj.imageMode')}</dt>
            <dd>{image.layerMode ? t(`layers.mode.${image.layerMode}` as Parameters<typeof t>[0]) : '–'}</dd>
            {image.dither && (
              <>
                <dt>{t('canvas.obj.imageDither')}</dt>
                <dd>{t(`layers.dither.${image.dither}` as Parameters<typeof t>[0])}</dd>
              </>
            )}
          </dl>
          {image.layerDpi !== null ? (
            <p className="shape-props__note">{t('canvas.obj.imageLayerDpi', { dpi: formatDpi(image.layerDpi) })}</p>
          ) : (
            <p className="shape-props__note">{t('canvas.obj.imageNotImageLayer')}</p>
          )}
          <div className="shape-props__row">
            <button type="button" className="btn" onClick={onImageSettings}>
              <Settings2 size={14} aria-hidden="true" />{t('canvas.obj.imageSettings')}
            </button>
            <button type="button" className="btn" aria-pressed={previewOpen} disabled={image.layerMode !== 'image'} title={image.layerMode !== 'image' ? t('canvas.obj.imageNotImageLayer') : t('layers.preview.hint')} onClick={onImagePreview}>
              <Eye size={14} aria-hidden="true" />{t('layers.preview.open')}
            </button>
          </div>
          <ImageVariableField />
        </>
      )}
      {cut && onCut && (
        <>
          <h3 className="shape-props__sub">{t('canvas.obj.cut')}</h3>
          <div className="shape-props__grid">
            <OptionalField
              label={t('canvas.obj.priority')}
              value={cut.priority}
              placeholder={t('canvas.obj.auto')}
              title={t('canvas.obj.priorityHint')}
              onApply={(v) => onCut({ priority: v })}
            />
            <OptionalField
              label={t('canvas.obj.powerScale')}
              value={cut.power}
              placeholder="100"
              unit="%"
              title={t('canvas.obj.powerScaleHint')}
              onApply={(v) => onCut({ power_scale: v })}
            />
          </div>
          <p className="shape-props__note">{t('canvas.obj.help')}</p>
        </>
      )}
      <label className="shape-props__check">
        <input type="checkbox" checked={false} onChange={onLock} />
        <span>{t('canvas.shape.locked')}</span>
      </label>
      {first && mixed && <p className="shape-props__note">{t('canvas.shape.mixed')}</p>}
      {first && <p className="shape-props__note">{t('canvas.shape.note')}</p>}
    </section>
  );
}

/**
 * Propiedades del objeto: parámetros de rectángulos, elipses, polígonos y estrellas mientras sigan
 * siendo formas (no se les hayan editado los nodos), y la prioridad de corte y la escala de
 * potencia de cualquier selección. Con varias formas del mismo tipo el cambio se aplica a todas;
 * con tipos distintos no hay campos de forma comunes y solo se muestran los de corte.
 */
export function ShapeProps() {
  const open = useStore((s) => s.shapePropsOpen);
  const ids = useStore((s) => s.selectedIds);
  const objects = useStore((s) => s.doc.objects);

  const picked = useMemo(() => {
    return objectsByIds(objects, ids);
  }, [ids, objects]);
  const editable = useMemo(
    () => picked.flatMap((o) => {
      const params = effectiveShape(o);
      return params ? [{ obj: o, params }] : [];
    }),
    [picked],
  );
  // Un único texto: se editan aquí su tamaño y espaciados (lo demás, en el panel de texto).
  const textObj = picked.length === 1 && picked[0]!.kind.type === 'text' ? picked[0]! : null;
  const imageObj = picked.length === 1 && picked[0]!.kind.type === 'image' ? picked[0]! : null;
  const imageLayer = useStore((s) => (imageObj ? s.doc.layers.find((l) => l.color === imageObj.layer) : undefined));
  const info = useMemo(() => (imageObj ? imageInfo(imageObj, imageLayer) : null), [imageObj, imageLayer]);
  const previewOpen = usePreviewUi((s) => s.open);
  // Si el editor de la capa está abierto, él ya dibuja el panel de vista previa.
  const editorShowsPreview = useLayerUi((s) => s.editing !== null && s.editing === imageObj?.layer);
  const { fonts, loading } = useFonts();
  const fallbackFont = useMemo(() => pickDefaultFont(fonts), [fonts]);
  const cut = useMemo<CutValues>(
    () => ({ priority: commonValue(picked, (o) => o.priority), power: commonValue(picked, (o) => o.power_scale) }),
    [picked],
  );

  if (!open || picked.length === 0) return null;
  const type = editable[0]?.params.type;
  const sameType = editable.length > 0 && editable.every((x) => x.params.type === type);
  const shapeParams = sameType ? editable[0]!.params : null;

  const apply = (patch: Partial<ShapeSpec>) => {
    const byId = new Map(editable.map((x) => [x.obj.id, editShape(x.obj, patch)]));
    useStore.getState().applyDocChange('canvas.history.shapeProps', (doc) => ({
      ...doc,
      objects: doc.objects.map((o) => byId.get(o.id) ?? o),
    }));
  };
  const lock = () => {
    const set = new Set(ids);
    useStore.getState().applyDocChange('arrange.history.lock', (doc) => ({ ...doc, objects: doc.objects.map((o) => (set.has(o.id) ? { ...o, locked: true } : o)) }));
  };
  return (
    <>
    <ShapePropsView
      params={shapeParams}
      mixed={editable.length !== picked.length}
      cut={cut}
      text={textObj && textObj.kind.type === 'text' ? textObj.kind : undefined}
      fonts={textObj ? { families: fonts, loading, fallback: fallbackFont } : undefined}
      onText={textObj ? (patch, field) => editText(textObj.id, patch, field) : undefined}
      image={info ?? undefined}
      previewOpen={previewOpen}
      onImageSettings={imageObj ? () => openEditorFor(useStore.getState(), imageObj.layer) : undefined}
      onImagePreview={imageObj ? () => usePreviewUi.getState().set({ open: !previewOpen, imageId: imageObj.id }) : undefined}
      onApply={apply}
      onCut={(patch) => setCutProps(useStore.getState(), ids, patch)}
      onClose={() => useStore.getState().setShapePropsOpen(false)}
      onLock={lock}
    />
    {previewOpen && info && imageLayer && !editorShowsPreview && typeof document !== 'undefined' && createPortal(<ImagePreviewPanel layer={imageLayer} />, document.body)}
    </>
  );
}
