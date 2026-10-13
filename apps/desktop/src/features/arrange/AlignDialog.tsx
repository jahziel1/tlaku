import {
  AlignCenterHorizontal, AlignCenterVertical, AlignEndHorizontal, AlignEndVertical, AlignHorizontalSpaceBetween,
  AlignStartHorizontal, AlignStartVertical, AlignVerticalSpaceBetween, Focus, type LucideIcon,
} from 'lucide-react';
import { useMemo } from 'react';
import { useCommand } from '@/commands/useCommand';
import type { I18nKey } from '@/i18n';
import { t } from '@/i18n';
import { useStore } from '@/store';
import { Choice, Dialog } from './Dialog';
import { useArrangeUi, type AlignReference } from './uiStore';
import { selectionUnits } from './units';

interface Item {
  id: string;
  icon: LucideIcon;
  label: I18nKey;
}

const ALIGN_ITEMS: Item[] = [
  { id: 'arrange.align.left', icon: AlignStartVertical, label: 'arrange.align.left' },
  { id: 'arrange.align.hcenter', icon: AlignCenterVertical, label: 'arrange.align.hcenter' },
  { id: 'arrange.align.right', icon: AlignEndVertical, label: 'arrange.align.right' },
  { id: 'arrange.align.top', icon: AlignStartHorizontal, label: 'arrange.align.top' },
  { id: 'arrange.align.vcenter', icon: AlignCenterHorizontal, label: 'arrange.align.vcenter' },
  { id: 'arrange.align.bottom', icon: AlignEndHorizontal, label: 'arrange.align.bottom' },
];

const DISTRIBUTE_ITEMS: Item[] = [
  { id: 'arrange.distribute.h', icon: AlignHorizontalSpaceBetween, label: 'arrange.align.distH' },
  { id: 'arrange.distribute.v', icon: AlignVerticalSpaceBetween, label: 'arrange.align.distV' },
];

/** Botón de un comando con etiqueta corta; deshabilitado, el motivo queda en el tooltip y en el texto del grupo. */
function CommandChip({ item }: { item: Item }) {
  const cmd = useCommand(item.id);
  const Icon = item.icon;
  return (
    <button type="button" className="btn ar-cmd" title={cmd.tooltip} aria-disabled={!cmd.enabled} onClick={() => cmd.enabled && cmd.run()}>
      <Icon size={15} aria-hidden="true" />
      <span>{t(item.label)}</span>
    </button>
  );
}

function Group({ legend, items, probe }: { legend: string; items: Item[]; probe: string }) {
  const reason = useCommand(probe).reason;
  return (
    <div className="ar-group" role="group" aria-label={legend}>
      <span className="field__label">{legend}</span>
      <div className="ar-group__row">{items.map((i) => <CommandChip key={i.id} item={i} />)}</div>
      {reason && <p className="hint ar-group__reason">{reason}</p>}
    </div>
  );
}

/** Alinear y distribuir con la referencia a la vista: la selección, un objeto elegido o la cama. */
export function AlignDialog() {
  const doc = useStore((s) => s.doc);
  const selectedIds = useStore((s) => s.selectedIds);
  const reference = useArrangeUi((s) => s.alignReference);
  const objectId = useArrangeUi((s) => s.alignObjectId);
  const setAlign = useArrangeUi((s) => s.setAlign);
  const close = useArrangeUi((s) => s.close);
  const units = useMemo(() => selectionUnits(doc, selectedIds), [doc, selectedIds]);
  const key = units.find((u) => u.ids.includes(objectId ?? '')) ?? units[0];
  const center = useCommand('arrange.centerOnBed');

  return (
    <Dialog
      title={t('arrange.align.title')}
      onClose={close}
      initialFocus="[aria-checked='true']"
      footer={<button type="button" className="btn" onClick={close}>{t('arrange.dlg.close')}</button>}
    >
      <Choice<AlignReference>
        legend={t('arrange.align.reference')}
        value={reference}
        options={[
          { value: 'selection', label: t('arrange.align.ref.selection') },
          { value: 'object', label: t('arrange.align.ref.object') },
          { value: 'bed', label: t('arrange.align.ref.bed') },
        ]}
        onChange={(ref) => setAlign(ref, ref === 'object' ? (key?.id ?? null) : undefined)}
      />
      {reference === 'object' && (
        <div className="field">
          <label className="field__label" htmlFor="arrange-align-object">{t('arrange.align.object')}</label>
          <select id="arrange-align-object" className="select" value={key?.id ?? ''} onChange={(e) => setAlign('object', e.target.value)}>
            {units.map((u) => <option key={u.id} value={u.id}>{u.id}</option>)}
          </select>
        </div>
      )}
      <Group legend={t('arrange.align.alignGroup')} items={ALIGN_ITEMS} probe="arrange.align.left" />
      <Group legend={t('arrange.align.distributeGroup')} items={DISTRIBUTE_ITEMS} probe="arrange.distribute.h" />
      <div className="ar-group">
        <button type="button" className="btn ar-cmd" title={center.tooltip} aria-disabled={!center.enabled} onClick={() => center.enabled && center.run()}>
          <Focus size={15} aria-hidden="true" />
          <span>{t('arrange.align.centerOnBed')}</span>
        </button>
        {center.reason && <p className="hint ar-group__reason">{center.reason}</p>}
      </div>
      <p className="hint">{t('arrange.align.hint')}</p>
    </Dialog>
  );
}
