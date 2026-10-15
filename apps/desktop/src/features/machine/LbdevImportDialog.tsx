import { useState } from 'react';
import type { DeviceProfile } from '@/api/types';
import { t, tn, type I18nKey } from '@/i18n';
import { fmt } from '@/lib/platform';
import { lbdevNoticeText, saveLbdevDevices, type LbdevPreview } from './lbdevImport';
import { Modal } from './Modal';

/** Resumen de un archivo `.lbdev`: qué perfiles se crearán y qué avisos dejó la lectura. */
export function LbdevImportDialog({
  preview,
  onDone,
  onCancel,
}: {
  preview: LbdevPreview;
  /** Perfiles que sí se guardaron (puede ser una parte si alguno falló). */
  onDone: (saved: DeviceProfile[]) => void;
  onCancel: () => void;
}) {
  const { devices, notices } = preview.result;
  const [saving, setSaving] = useState(false);
  const confirm = async () => {
    setSaving(true);
    const saved = await saveLbdevDevices(devices);
    setSaving(false);
    onDone(saved);
  };
  return (
    <Modal
      title={t('devices.lbdev.title')}
      subtitle={t('devices.lbdev.subtitle')}
      size="md"
      onClose={onCancel}
      footer={
        <>
          <button type="button" className="btn" onClick={onCancel}>{t('machine.dlg.cancel')}</button>
          <span className="topbar__spacer" />
          <button type="button" className="btn btn--accent" data-autofocus disabled={saving || devices.length === 0} onClick={() => void confirm()}>
            {tn('devices.lbdev.confirm', devices.length)}
          </button>
        </>
      }
    >
      <p>{tn('devices.lbdev.intro', devices.length, { file: preview.file })}</p>
      <ul className="mc-lbdev__list" aria-label={t('devices.lbdev.list')}>
        {devices.map((d) => (
          <li key={d.id}>
            <strong>{d.name}</strong>
            <span className="hint num">
              {' '}· {t(`devices.kind.${d.kind}` as I18nKey)} · {fmt(d.bed[0], 0)} × {fmt(d.bed[1], 0)} mm · {t(`machine.dev.origin.${d.origin}` as I18nKey)}
            </span>
          </li>
        ))}
      </ul>
      <h3 className="mc-sub">{t('devices.lbdev.noticesTitle')}</h3>
      {notices.length === 0 ? (
        <p className="hint">{t('devices.lbdev.noNotices')}</p>
      ) : (
        <ul className="mc-lbdev__notices">
          {notices.map((n, i) => (
            <li key={i}>{lbdevNoticeText(n)}</li>
          ))}
        </ul>
      )}
    </Modal>
  );
}
