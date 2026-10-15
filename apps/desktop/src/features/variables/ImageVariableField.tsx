import { useId } from 'react';
import type { DesignObject, ImageFit, VariableImage } from '@/api/types';
import { t } from '@/i18n';
import { useStore } from '@/store';
import { ColumnPicker } from './ColumnPicker';
import { useVariablesUi } from './context';
import { columnSnippet } from './insert';
import './variables.css';

/** Plantilla de ejemplo para una imagen variable nueva: la columna que parezca de imágenes, o una genérica. */
export function defaultImageTemplate(headers: readonly string[] | undefined): string {
  const idx = (headers ?? []).findIndex((h) => /logo|imagen|image|foto|photo|archivo|file|ruta|path|img/i.test(h));
  return idx >= 0 ? columnSnippet(headers![idx]!, idx) : '{csv:Logo}';
}

/** Cambia (o quita, con `null`) la marca de imagen variable del objeto `id`, con un paso de deshacer. */
export function setImageVariable(id: string, variable: VariableImage | null): void {
  useStore.getState().applyDocChange(
    'variables.history.imageVar',
    (doc) => {
      let changed = false;
      const objects = doc.objects.map((o): DesignObject => {
        if (o.id !== id || o.kind.type !== 'image') return o;
        const cur = o.kind.variable ?? null;
        if (cur === variable || (cur && variable && cur.source === variable.source && cur.fit === variable.fit)) return o;
        changed = true;
        const { variable: _drop, ...rest } = o.kind;
        void _drop;
        return { ...o, kind: variable ? { ...rest, variable } : rest };
      });
      return changed ? { ...doc, objects } : doc;
    },
  );
}

/**
 * Sección «Imagen variable» de las propiedades de una imagen: marca la imagen como variable, elige
 * de qué columna sale el archivo y cómo se ajusta a su caja. Solo con una imagen seleccionada.
 */
export function ImageVariableField() {
  const id = useId();
  const selected = useStore((s) => (s.selectedIds.length === 1 ? s.doc.objects.find((o) => o.id === s.selectedIds[0]) : undefined));
  const main = useVariablesUi((s) => s.csv.table?.headers);
  if (!selected || selected.kind.type !== 'image') return null;
  const variable = selected.kind.variable ?? null;
  const objId = selected.id;

  return (
    <div className="vt-imgvar">
      <label className="vt-check">
        <input
          type="checkbox"
          checked={variable !== null}
          onChange={(e) => setImageVariable(objId, e.target.checked ? { source: defaultImageTemplate(main), fit: 'contain' } : null)}
        />
        {t('variables.image.toggle')}
      </label>
      {variable && (
        <>
          <p className="shape-props__note">{t('variables.image.hint')}</p>
          <div className="field">
            <label className="field__label" htmlFor={`${id}-src`}>{t('variables.image.source')}</label>
            <input
              id={`${id}-src`}
              key={variable.source}
              className="input"
              defaultValue={variable.source}
              spellCheck={false}
              onBlur={(e) => e.target.value !== variable.source && setImageVariable(objId, { ...variable, source: e.target.value })}
              onKeyDown={(e) => {
                if (e.key === 'Enter') e.currentTarget.blur();
              }}
            />
          </div>
          <ColumnPicker
            label={t('variables.image.column')}
            placeholder={t('variables.image.columnPlaceholder')}
            onPick={(snippet) => setImageVariable(objId, { ...variable, source: snippet })}
          />
          <div className="field">
            <label className="field__label" htmlFor={`${id}-fit`}>{t('variables.image.fit')}</label>
            <select id={`${id}-fit`} className="select" value={variable.fit} onChange={(e) => setImageVariable(objId, { ...variable, fit: e.target.value as ImageFit })}>
              <option value="contain">{t('variables.image.fit.contain')}</option>
              <option value="cover">{t('variables.image.fit.cover')}</option>
            </select>
          </div>
          <p className="shape-props__note">{variable.fit === 'cover' ? t('variables.image.fit.coverHint') : t('variables.image.fit.containHint')}</p>
        </>
      )}
    </div>
  );
}
