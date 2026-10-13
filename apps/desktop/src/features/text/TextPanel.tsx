import { AlignCenter, AlignLeft, AlignRight, Bold, Italic, LoaderCircle, TriangleAlert } from 'lucide-react';
import { useEffect, useMemo, useRef } from 'react';
import { useShallow } from 'zustand/react/shallow';
import type { TextAlign } from '@/api/types';
import { t } from '@/i18n';
import { objectBounds } from '@/lib/geometry';
import { useStore } from '@/store';
import { NumberField } from '@/ui/NumberField';
import { convertTextToPaths, editText, guideCandidates } from './convert';
import { FontPicker } from './FontPicker';
import { pickDefaultFont, useFonts } from './fonts';
import { useOutlineSync } from './outlineSync';
import { choosePanelSide, PANEL_WIDTH, screenBox } from './placement';
import { isTextObject, type TextDesignObject } from './signature';
import { selectStatus, useTextStatus } from './status';
import './text.css';

const ALIGNS: { value: TextAlign; icon: typeof AlignLeft; key: 'text.align.left' | 'text.align.center' | 'text.align.right' }[] = [
  { value: 'left', icon: AlignLeft, key: 'text.align.left' },
  { value: 'center', icon: AlignCenter, key: 'text.align.center' },
  { value: 'right', icon: AlignRight, key: 'text.align.right' },
];

/**
 * Panel que flota sobre el lienzo cuando la selección es un solo texto. Además mantiene al día
 * el contorno de todos los textos del documento (aunque no estén seleccionados).
 */
export function TextPanel() {
  useOutlineSync();
  const id = useStore((s) => (s.selectedIds.length === 1 ? s.selectedIds[0]! : null));
  const obj = useStore((s) => (id ? s.doc.objects.find((o) => o.id === id) : undefined));
  if (!obj || !isTextObject(obj)) return null;
  return <TextEditor key={obj.id} obj={obj} />;
}

/** Contenido del panel para un texto (exportado para pruebas). */
export function TextEditor({ obj }: { obj: TextDesignObject }) {
  const text = obj.kind;
  const id = obj.id;
  const { fonts, loading } = useFonts();
  const status = useTextStatus(selectStatus(id));
  const areaRef = useRef<HTMLTextAreaElement>(null);
  const view = useStore(useShallow((s) => ({ zoom: s.zoom, panX: s.panX, panY: s.panY, w: s.viewport.width, h: s.viewport.height })));
  const candidates = useStore(useShallow((s) => guideCandidates(s.doc, id).map((o) => o.id)));

  // El foco va al contenido en cuanto aparece el panel y cada vez que el lienzo lo pide
  // (`textFocusToken`: doble clic sobre un texto ya seleccionado), con el cursor al final.
  const focusToken = useStore((s) => s.textFocusToken);
  useEffect(() => {
    const el = areaRef.current;
    if (!el) return;
    el.focus({ preventScroll: true });
    el.setSelectionRange(el.value.length, el.value.length);
  }, [focusToken]);

  const side = useMemo(() => {
    const b = objectBounds(obj);
    return choosePanelSide(b ? screenBox(view, b) : null, view.w);
  }, [obj, view]);

  const fallbackFont = useMemo(() => pickDefaultFont(fonts), [fonts]);
  const onPath = text.on_path;
  const target = onPath?.object_id ?? '';
  const hasOutline = text.outline.length > 0;
  const convertReason = status.busy ? t('text.convert.reasonBusy') : !hasOutline ? t('text.convert.reasonEmpty') : null;

  const set = (patch: Parameters<typeof editText>[1], field: string) => editText(id, patch, field);

  return (
    <div className="text-host" style={{ width: view.w, height: view.h, marginTop: -view.h }}>
      <section
        className={`text-panel text-panel--${side}`}
        style={{ width: Math.min(PANEL_WIDTH, Math.max(0, view.w - 16)) }}
        aria-label={t('text.panel.label')}
      >
        <header className="text-panel__head">
          <h2 className="panel-title">{t('text.panel.title')}</h2>
          {status.busy && (
            <span className="text-panel__busy" role="status">
              <LoaderCircle size={13} className="text-panel__spin" aria-hidden="true" />
              {t('text.status.busy')}
            </span>
          )}
        </header>

        <div className="field">
          <label className="field__label" htmlFor={`text-content-${id}`}>{t('text.field.text')}</label>
          <textarea
            id={`text-content-${id}`}
            ref={areaRef}
            className="input text-panel__area"
            rows={3}
            value={text.text}
            placeholder={t('text.field.textPlaceholder')}
            spellCheck={false}
            onChange={(e) => set({ text: e.target.value }, 'text')}
            onKeyDown={(e) => {
              if (e.key === 'Escape') e.currentTarget.blur();
            }}
          />
        </div>

        <div className="field">
          <span className="field__label">{t('text.field.font')}</span>
          <FontPicker
            value={text.font_family}
            fallback={fallbackFont}
            families={fonts}
            loading={loading}
            onPick={(family) => set({ font_family: family }, 'font')}
          />
        </div>

        <div className="text-panel__row">
          <button type="button" className="icon-btn icon-btn--boxed" aria-pressed={text.bold} title={t('text.style.bold')} aria-label={t('text.style.bold')} onClick={() => set({ bold: !text.bold }, 'bold')}>
            <Bold size={15} aria-hidden="true" />
          </button>
          <button type="button" className="icon-btn icon-btn--boxed" aria-pressed={text.italic} title={t('text.style.italic')} aria-label={t('text.style.italic')} onClick={() => set({ italic: !text.italic }, 'italic')}>
            <Italic size={15} aria-hidden="true" />
          </button>
          <span className="text-panel__sep" aria-hidden="true" />
          <div role="group" aria-label={t('text.align.label')} className="text-panel__aligns">
            {ALIGNS.map(({ value, icon: Icon, key }) => (
              <button key={value} type="button" className="icon-btn icon-btn--boxed" aria-pressed={text.align === value} title={t(key)} aria-label={t(key)} onClick={() => set({ align: value }, 'align')}>
                <Icon size={15} aria-hidden="true" />
              </button>
            ))}
          </div>
        </div>

        <div className="text-panel__grid">
          <NumberField label={t('text.field.size')} unit="mm" value={text.size_mm} min={0.1} max={2000} step={1} onCommit={(v) => set({ size_mm: v }, 'size')} />
          <NumberField label={t('text.field.letterSpacing')} unit="mm" value={text.letter_spacing_mm} min={-100} max={500} step={0.1} onCommit={(v) => set({ letter_spacing_mm: v }, 'spacing')} />
          <NumberField label={t('text.field.lineSpacing')} unit="×" value={text.line_spacing} min={0.1} max={10} step={0.1} onCommit={(v) => set({ line_spacing: v }, 'lines')} />
        </div>
        <p className="hint text-panel__hint">{t('text.field.sizeHint')} {t('text.field.lineSpacingHint')}</p>

        <fieldset className="text-panel__path">
          <label className="text-panel__check">
            <input
              type="checkbox"
              checked={onPath !== null}
              disabled={onPath === null && candidates.length === 0}
              onChange={(e) =>
                set({ on_path: e.target.checked ? { object_id: candidates[0] ?? '', offset_mm: 0, flip: false } : null }, 'onpath')
              }
            />
            {t('text.onPath.enable')}
          </label>
          {onPath === null && candidates.length === 0 && <p className="hint">{t('text.onPath.none')}</p>}
          {onPath && (
            <>
              <div className="field">
                <label className="field__label" htmlFor={`text-guide-${id}`}>{t('text.onPath.target')}</label>
                <select
                  id={`text-guide-${id}`}
                  className="select"
                  value={target}
                  onChange={(e) => set({ on_path: { ...onPath, object_id: e.target.value } }, 'onpath')}
                >
                  {!candidates.includes(target) && <option value={target}>{target || '—'}</option>}
                  {candidates.map((cid) => (
                    <option key={cid} value={cid}>{cid}</option>
                  ))}
                </select>
              </div>
              <div className="text-panel__grid text-panel__grid--2">
                <NumberField label={t('text.onPath.offset')} unit="mm" value={onPath.offset_mm} step={1} onCommit={(v) => set({ on_path: { ...onPath, offset_mm: v } }, 'offset')} />
                <label className="text-panel__check text-panel__check--end">
                  <input type="checkbox" checked={onPath.flip} onChange={(e) => set({ on_path: { ...onPath, flip: e.target.checked } }, 'flip')} />
                  {t('text.onPath.flip')}
                </label>
              </div>
            </>
          )}
        </fieldset>

        {status.error && (
          <p className="warn-line text-panel__error" role="alert">
            <TriangleAlert size={14} aria-hidden="true" />
            <span>{status.error.startsWith('El objeto guía') ? status.error : t('text.status.error', { message: status.error })}</span>
          </p>
        )}
        {status.warnings.length > 0 && (
          <ul className="text-panel__warns" aria-label={t('text.warn.title')}>
            {status.warnings.map((w) => (
              <li key={w} className="warn-line">
                <TriangleAlert size={14} aria-hidden="true" />
                <span>{w}</span>
              </li>
            ))}
          </ul>
        )}

        <div className="text-panel__foot">
          <button
            type="button"
            className="btn"
            aria-disabled={convertReason !== null}
            title={convertReason ?? t('text.convert.title')}
            onClick={() => convertReason === null && convertTextToPaths(id)}
          >
            {t('text.convert')}
          </button>
          {convertReason && <p className="hint">{convertReason}</p>}
        </div>
      </section>
    </div>
  );
}
