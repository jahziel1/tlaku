import { useId, useState } from 'react';
import { t } from '@/i18n';
import { Modal } from '@/ui/Modal';
import { NumberField } from '@/ui/NumberField';
import { CheckRow, ChoiceRow } from './Fields';
import './fileio.css';
import { DEFAULT_IMPORT_OPTIONS, type ImportDialogOptions } from './types';

/** Qué opciones tienen sentido según los formatos de los archivos elegidos. */
export interface ImportKinds {
  /** SVG o imágenes: el DPI cuenta. */
  dpi: boolean;
  dxf: boolean;
  pdf: boolean;
  gcode: boolean;
  /** Hay algún formato vectorial: la capa destino cuenta. */
  vector: boolean;
}

const SVG_IMAGE = ['svg', 'png', 'jpg', 'jpeg', 'bmp', 'webp', 'tif', 'tiff', 'gif', 'tga'];
const GCODE = ['nc', 'gcode', 'gc', 'ngc'];
const NON_VECTOR = ['png', 'jpg', 'jpeg', 'bmp', 'webp', 'tif', 'tiff', 'gif', 'tga'];

/** Opciones que corresponden a las extensiones (en minúsculas, sin punto) de los archivos. */
export function kindsOf(extensions: readonly string[]): ImportKinds {
  const has = (list: readonly string[]) => extensions.some((e) => list.includes(e));
  return {
    dpi: has(SVG_IMAGE),
    dxf: extensions.includes('dxf'),
    pdf: has(['pdf', 'ai']),
    gcode: has(GCODE),
    vector: extensions.some((e) => !NON_VECTOR.includes(e)),
  };
}

interface Props {
  fileCount: number;
  kinds: ImportKinds;
  /** Capas del diseño actual para elegir destino. */
  layers: { color: string; name: string }[];
  onDone: (options: ImportDialogOptions | null) => void;
}

/** Opciones de importación (D91): escala, DPI, unidades del DXF, página del PDF, agrupar y capa destino. */
export function ImportOptionsDialog({ fileCount, kinds, layers, onDone }: Props) {
  const labelId = useId();
  const [o, setO] = useState<ImportDialogOptions>(DEFAULT_IMPORT_OPTIONS);
  const patch = (p: Partial<ImportDialogOptions>) => setO((cur) => ({ ...cur, ...p }));

  return (
    <Modal
      labelId={labelId}
      onClose={() => onDone(null)}
      backdropClassName="modal"
      cardClassName="modal__card"
      initialFocus="[data-autofocus]"
      guardOutside
    >
      <div className="modal__head">
        <h2 id={labelId} className="modal__title">{t('fileio.import.title')}</h2>
        <span className="fio-hint">{t('fileio.import.files', { n: fileCount })}</span>
      </div>
      <div className="modal__body fio-body">
        <NumberField label={t('fileio.import.scale')} value={o.scale_percent} min={0.1} max={100000} step={10} unit="%" onCommit={(scale_percent) => patch({ scale_percent })} />
        {kinds.dpi && (
          <ChoiceRow
            legend={t('fileio.import.dpi')}
            value={o.dpi}
            options={[
              { value: null, label: t('fileio.import.dpi.auto') },
              { value: 72, label: '72' },
              { value: 150, label: '150' },
              { value: 300, label: '300' },
            ]}
            onChange={(dpi) => patch({ dpi })}
          />
        )}
        {kinds.dxf && (
          <ChoiceRow
            legend={t('fileio.import.dxfUnit')}
            value={o.dxf_unit_mm}
            options={[
              { value: null, label: t('fileio.import.dxfUnit.auto') },
              { value: 25.4, label: t('fileio.import.dxfUnit.inch') },
              { value: 10, label: t('fileio.import.dxfUnit.cm') },
            ]}
            onChange={(dxf_unit_mm) => patch({ dxf_unit_mm })}
          />
        )}
        {kinds.pdf && (
          <NumberField label={t('fileio.import.pdfPage')} value={o.pdf_page} min={1} max={9999} step={1} onCommit={(pdf_page) => patch({ pdf_page: Math.round(pdf_page) })} />
        )}
        {kinds.gcode && (
          <ChoiceRow
            legend={t('fileio.import.gcode')}
            value={o.gcode_by_power ? 'power' : 'single'}
            options={[
              { value: 'power', label: t('fileio.import.gcode.power') },
              { value: 'single', label: t('fileio.import.gcode.single') },
            ]}
            onChange={(v) => patch({ gcode_by_power: v === 'power' })}
          />
        )}
        {kinds.vector && (
          <div className="field">
            <label className="field__label" htmlFor={`${labelId}-layer`}>{t('fileio.import.layer')}</label>
            <select
              id={`${labelId}-layer`}
              className="fio-select"
              value={o.target_layer ?? ''}
              onChange={(e) => patch({ target_layer: e.target.value || null })}
            >
              <option value="">{t('fileio.import.layer.auto')}</option>
              {layers.map((l) => (
                <option key={l.color} value={l.color}>{l.name}</option>
              ))}
            </select>
          </div>
        )}
        <CheckRow label={t('fileio.import.group')} checked={o.group} onChange={(group) => patch({ group, auto_group: group ? false : o.auto_group })} />
        {kinds.vector && (
          <CheckRow label={t('fileio.import.autoGroup')} checked={o.auto_group} onChange={(auto_group) => patch({ auto_group, group: auto_group ? false : o.group })} />
        )}
      </div>
      <div className="modal__foot">
        <button type="button" className="btn" onClick={() => onDone(null)}>{t('fileio.import.cancel')}</button>
        <button type="button" className="btn btn--accent" data-autofocus onClick={() => onDone(o)}>{t('fileio.import.accept')}</button>
      </div>
    </Modal>
  );
}
