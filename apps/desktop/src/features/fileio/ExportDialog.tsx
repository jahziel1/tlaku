import { useId, useState } from 'react';
import { t, type I18nKey } from '@/i18n';
import { NumberField } from '@/ui/NumberField';
import { Modal } from '@/ui/Modal';
import { CheckRow, ChoiceRow, Hint } from './Fields';
import './fileio.css';
import {
  DEFAULT_DXF,
  DEFAULT_PDF,
  DEFAULT_PNG,
  DEFAULT_SVG,
  type DxfOptions,
  type ExportFormat,
  type ExportRequest,
  type PdfOptions,
  type PngOptions,
  type SvgOptions,
} from './types';

export type ExportScope = 'all' | 'selection';

/** Lo que el usuario eligió en el diálogo. */
export interface ExportChoice {
  scope: ExportScope;
  request: ExportRequest;
}

/** Opciones de la última vez, para repetir la exportación sin volver a configurarla. */
export interface RememberedOptions {
  svg: SvgOptions;
  dxf: DxfOptions;
  png: PngOptions;
  pdf: PdfOptions;
  ai: PdfOptions;
}

export function defaultRemembered(): RememberedOptions {
  return { svg: { ...DEFAULT_SVG }, dxf: { ...DEFAULT_DXF }, png: { ...DEFAULT_PNG }, pdf: { ...DEFAULT_PDF }, ai: { ...DEFAULT_PDF } };
}

const DPI_PRESETS = [150, 300, 600] as const;

interface Props {
  format: ExportFormat;
  /** Imprimir usa las opciones del PDF y otro texto de botón. */
  print?: boolean;
  /** Objetos seleccionados; con 0 no se ofrece "solo la selección". */
  selectionCount: number;
  initial: RememberedOptions;
  onDone: (choice: (ExportChoice & { options: RememberedOptions }) | null) => void;
}

/** Diálogo de exportación: qué exportar (todo o selección) y las opciones del formato. */
export function ExportDialog({ format, print = false, selectionCount, initial, onDone }: Props) {
  const labelId = useId();
  const [scope, setScope] = useState<ExportScope>(selectionCount > 0 ? 'selection' : 'all');
  const [opts, setOpts] = useState<RememberedOptions>(initial);
  const patch = <K extends ExportFormat>(k: K, p: Partial<RememberedOptions[K]>) =>
    setOpts((o) => ({ ...o, [k]: { ...o[k], ...p } }));

  const accept = () => {
    const request = { format, ...opts[format] } as ExportRequest;
    onDone({ scope, request, options: opts });
  };
  const title = print ? t('fileio.export.title.print') : t(`fileio.export.title.${format}`);

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
        <h2 id={labelId} className="modal__title">{title}</h2>
      </div>
      <div className="modal__body fio-body">
        {selectionCount > 0 && (
          <ChoiceRow
            legend={t('fileio.scope.legend')}
            value={scope}
            options={[
              { value: 'all', label: t('fileio.scope.all') },
              { value: 'selection', label: t('fileio.scope.selection', { n: selectionCount }) },
            ]}
            onChange={setScope}
          />
        )}
        {format === 'svg' && <SvgFields value={opts.svg} onChange={(p) => patch('svg', p)} />}
        {format === 'dxf' && <DxfFields value={opts.dxf} onChange={(p) => patch('dxf', p)} />}
        {format === 'png' && <PngFields value={opts.png} onChange={(p) => patch('png', p)} />}
        {format === 'pdf' && <PdfFields value={opts.pdf} onChange={(p) => patch('pdf', p)} />}
        {format === 'ai' && <PdfFields value={opts.ai} onChange={(p) => patch('ai', p)} hint="fileio.ai.hint" />}
      </div>
      <div className="modal__foot">
        <button type="button" className="btn" onClick={() => onDone(null)}>{t('fileio.export.cancel')}</button>
        <button type="button" className="btn btn--accent" data-autofocus onClick={accept}>
          {print ? t('fileio.export.acceptPrint') : t('fileio.export.accept')}
        </button>
      </div>
    </Modal>
  );
}

type FieldsProps<T> = { value: T; onChange: (p: Partial<T>) => void };

function SvgFields({ value, onChange }: FieldsProps<SvgOptions>) {
  return (
    <>
      <Hint>{t('fileio.svg.hint')}</Hint>
      <ChoiceRow
        legend={t('fileio.svg.images')}
        value={value.images}
        options={[
          { value: 'embed', label: t('fileio.svg.images.embed') },
          { value: 'link', label: t('fileio.svg.images.link') },
          { value: 'skip', label: t('fileio.svg.images.skip') },
        ]}
        onChange={(images) => onChange({ images })}
      />
      <div className="fio-grid">
        <NumberField label={t('fileio.margin')} value={value.margin_mm} min={0} max={1000} step={1} unit="mm" onCommit={(margin_mm) => onChange({ margin_mm })} />
        <NumberField label={t('fileio.stroke')} value={value.stroke_mm} min={0.01} max={10} step={0.05} unit="mm" onCommit={(stroke_mm) => onChange({ stroke_mm })} />
      </div>
      <CheckRow label={t('fileio.svg.keepPosition')} checked={value.keep_position} onChange={(keep_position) => onChange({ keep_position })} />
      <CheckRow label={t('fileio.fillLayers')} checked={value.fill_layers} onChange={(fill_layers) => onChange({ fill_layers })} />
    </>
  );
}

function DxfFields({ value, onChange }: FieldsProps<DxfOptions>) {
  return (
    <>
      <Hint>{t('fileio.dxf.hint')}</Hint>
      <ChoiceRow
        legend={t('fileio.dxf.curves')}
        value={value.curves}
        options={[
          { value: 'splines', label: t('fileio.dxf.curves.splines') },
          { value: 'flatten', label: t('fileio.dxf.curves.flatten') },
        ]}
        onChange={(curves) => onChange({ curves })}
      />
      <NumberField label={t('fileio.dxf.tolerance')} value={value.tolerance_mm} min={0.0005} max={1} step={0.001} unit="mm" onCommit={(tolerance_mm) => onChange({ tolerance_mm })} />
      <CheckRow label={t('fileio.dxf.trueColor')} checked={value.true_color} onChange={(true_color) => onChange({ true_color })} />
    </>
  );
}

function PngFields({ value, onChange }: FieldsProps<PngOptions>) {
  return (
    <>
      <Hint>{t('fileio.png.hint')}</Hint>
      <ChoiceRow
        legend={t('fileio.png.dpi')}
        value={value.dpi}
        options={DPI_PRESETS.map((d) => ({ value: d as number, label: String(d) }))}
        onChange={(dpi) => onChange({ dpi })}
      />
      <div className="fio-grid">
        <NumberField label={t('fileio.png.dpi')} value={value.dpi} min={1} max={2400} step={10} unit="ppp" onCommit={(dpi) => onChange({ dpi })} />
        <NumberField label={t('fileio.margin')} value={value.margin_mm} min={0} max={1000} step={1} unit="mm" onCommit={(margin_mm) => onChange({ margin_mm })} />
      </div>
      <CheckRow label={t('fileio.png.transparent')} checked={value.transparent} onChange={(transparent) => onChange({ transparent })} />
      <CheckRow label={t('fileio.fillLayers')} checked={value.fill_layers} onChange={(fill_layers) => onChange({ fill_layers })} />
    </>
  );
}

function PdfFields({ value, onChange, hint = 'fileio.pdf.hint' }: FieldsProps<PdfOptions> & { hint?: I18nKey }) {
  return (
    <>
      <Hint>{t(hint)}</Hint>
      <ChoiceRow
        legend={t('fileio.pdf.page')}
        value={value.page}
        options={[
          { value: 'fit', label: t('fileio.pdf.page.fit') },
          { value: 'a4', label: t('fileio.pdf.page.a4') },
          { value: 'a3', label: t('fileio.pdf.page.a3') },
          { value: 'letter', label: t('fileio.pdf.page.letter') },
          { value: 'legal', label: t('fileio.pdf.page.legal') },
        ]}
        onChange={(page) => onChange({ page })}
      />
      <div className="fio-grid">
        <NumberField label={t('fileio.margin')} value={value.margin_mm} min={0} max={1000} step={1} unit="mm" onCommit={(margin_mm) => onChange({ margin_mm })} />
        <NumberField label={t('fileio.stroke')} value={value.stroke_mm} min={0.01} max={10} step={0.05} unit="mm" onCommit={(stroke_mm) => onChange({ stroke_mm })} />
      </div>
      <CheckRow label={t('fileio.pdf.images')} checked={value.images} onChange={(images) => onChange({ images })} />
      <CheckRow label={t('fileio.fillLayers')} checked={value.fill_layers} onChange={(fill_layers) => onChange({ fill_layers })} />
    </>
  );
}
