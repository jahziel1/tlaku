import { ExternalLink } from 'lucide-react';
import { useEffect, useMemo, useState } from 'react';
import type { MaterialPreset } from '@/api/types';
import { getApi } from '@/api';
import { myMachine } from '@/features/materials/communityModel';
import { useMaterials } from '@/features/materials/state';
import { t, type I18nKey } from '@/i18n';
import { selectActiveDevice, useStore } from '@/store';
import { selectIsVirtual } from '@/store/machine';
import { CopyButton } from './CopyButton';
import { bugReportUrl, contributionDraft, machineContext, machineReportUrl, materialFormValues, materialShareUrl, FORM_FIELDS, type MachineContext } from './githubForms';
import { Frame } from './HelpFrame';
import { useHelpUi } from './helpStore';
import { openExternal } from './openExternal';

/** Lo que la app ya sabe de la máquina activa; se recalcula solo cuando cambia. */
function useMachineContext(): MachineContext {
  const device = useStore(selectActiveDevice);
  const connected = useStore((s) => s.connection.connected);
  const isVirtual = useStore(selectIsVirtual);
  const firmware = useStore((s) => s.firmware);
  const log = useStore((s) => s.log);
  return useMemo(() => machineContext(device, connected, isVirtual, firmware, log), [device, connected, isVirtual, firmware, log]);
}

function useNav(): { platform?: string; userAgent?: string } {
  return typeof navigator === 'undefined' ? {} : navigator;
}

/** Dirección del formulario de errores con versión, sistema y máquina. */
export function useBugReportUrl(version: string | null): string {
  const machine = useMachineContext();
  const nav = useNav();
  return useMemo(() => bugReportUrl({ version, nav, machine }), [version, nav, machine]);
}

/** Botón que abre un formulario en el navegador; si no se pudo, lo dice (queda «copiar enlace»). */
export function OpenFormButton({ url, label, autoFocus }: { url: string; label: string; autoFocus?: boolean }) {
  const [failed, setFailed] = useState(false);
  return (
    <>
      <button
        type="button"
        className="btn btn--accent"
        data-autofocus={autoFocus ? '' : undefined}
        onClick={() => {
          void openExternal(url).then((ok) => setFailed(!ok));
        }}
      >
        <ExternalLink size={14} aria-hidden="true" />
        {label}
      </button>
      {failed && <span role="alert" className="help__pending">{t('help.share.openFailed')}</span>}
    </>
  );
}

const FIELD_LABEL = (id: string): I18nKey => `help.share.field.${id}` as I18nKey;

/** Vista común de los diálogos de compartir: qué se prellena, abrir y copiar. Nada se envía. */
export function ShareView({ id, title, children, fields, url, onClose }: {
  id: string;
  title: string;
  children?: React.ReactNode;
  fields: { id: string; value: string }[];
  url: string;
  onClose: () => void;
}) {
  return (
    <Frame id={id} title={title} onClose={onClose}>
      <div className="help__scroll help__prose">
        {children}
        <h3 className="help__sub">{t('help.share.willFill')}</h3>
        {fields.length === 0 ? (
          <p className="hint">{t('help.share.fillNone')}</p>
        ) : (
          <dl className="help__facts">
            {fields.map((f) => (
              <div key={f.id} className="help__fact">
                <dt>{t(FIELD_LABEL(f.id))}</dt>
                <dd>{f.value.includes('\n') ? <pre className="md__pre"><code>{f.value}</code></pre> : f.value}</dd>
              </div>
            ))}
          </dl>
        )}
        <p className="help__warnnote">{t('help.share.review')}</p>
        <p className="hint">{t('help.share.nothingSent')}</p>
      </div>
      <div className="help__foot help__foot--end">
        <button type="button" className="btn" onClick={onClose}>{t('help.close')}</button>
        <CopyButton text={url} label={t('help.share.copyLink')} />
        <OpenFormButton url={url} label={t('help.share.open')} autoFocus />
      </div>
    </Frame>
  );
}

/** Los campos con valor, en el orden del formulario, para mostrarlos. */
function visibleFields(form: keyof typeof FORM_FIELDS, values: Record<string, string | undefined>): { id: string; value: string }[] {
  return FORM_FIELDS[form].flatMap((id) => (values[id]?.trim() ? [{ id, value: values[id]!.trim() }] : []));
}

export function ShareMachineDialog() {
  const close = useHelpUi((s) => s.close);
  const machine = useMachineContext();
  const nav = useNav();
  const [version, setVersion] = useState<string | null>(null);
  useEffect(() => {
    let alive = true;
    getApi()
      .appVersion()
      .then((v) => alive && setVersion(v))
      .catch(() => alive && setVersion(null));
    return () => {
      alive = false;
    };
  }, []);
  const url = useMemo(() => machineReportUrl({ version, nav, machine }), [version, nav, machine]);
  const params = new URL(url).searchParams;
  const values: Record<string, string | undefined> = {};
  for (const id of FORM_FIELDS.machine) values[id] = params.get(id) ?? undefined;
  return (
    <ShareView id="help-share-machine-title" title={t('help.shareMachine.title')} fields={visibleFields('machine', values)} url={url} onClose={close}>
      <p>{t('help.shareMachine.intro')}</p>
      <p>{t('help.shareMachine.levels')}</p>
      {machine.virtual && <p className="help__warnnote">{t('help.shareMachine.virtual')}</p>}
      {!machine.virtual && !machine.info && <p className="hint">{t('help.shareMachine.noInfo')}</p>}
    </ShareView>
  );
}

/** Versión de la app (para el aporte); `null` mientras se lee o si no se pudo. */
function useAppVersion(): string | null {
  const [version, setVersion] = useState<string | null>(null);
  useEffect(() => {
    let alive = true;
    getApi()
      .appVersion()
      .then((v) => alive && setVersion(v))
      .catch(() => alive && setVersion(null));
    return () => {
      alive = false;
    };
  }, []);
  return version;
}

export function ShareMaterialDialog() {
  const close = useHelpUi((s) => s.close);
  const preset: MaterialPreset | null = useHelpUi((s) => s.sharePreset);
  const device = useStore(selectActiveDevice);
  const chosen = useMaterials((s) => s.machineClass);
  // La clase que eligió la persona o, si no, la potencia que dice el nombre del modelo («Falcon2 22 W»).
  const chosenClass = useMemo(() => myMachine(device, chosen).machineClass, [device, chosen]);
  const version = useAppVersion();
  const url = useMemo(() => materialShareUrl(preset, device, chosenClass, version), [preset, device, chosenClass, version]);
  const values = preset ? materialFormValues(preset, device, chosenClass, version) : { machine: device.name, ...(version ? { tlaku_version: version } : {}) };
  const draft = useMemo(() => (preset ? JSON.stringify(contributionDraft(preset, device, chosenClass, version), null, 2) : null), [preset, device, chosenClass, version]);
  return (
    <ShareView id="help-share-material-title" title={t('help.shareMaterial.title')} fields={visibleFields('material', values)} url={url} onClose={close}>
      <p>{t('help.shareMaterial.intro')}</p>
      {preset ? <p className="hint">{t('help.shareMaterial.classNote')}</p> : <p className="hint">{t('help.shareMaterial.noPreset')}</p>}
      {draft && (
        <details className="help__draft">
          <summary>{t('help.shareMaterial.draft')}</summary>
          <p className="hint">{t('help.shareMaterial.draftHint')}</p>
          <pre className="md__pre"><code>{draft}</code></pre>
          <CopyButton text={draft} label={t('help.shareMaterial.draftCopy')} />
        </details>
      )}
    </ShareView>
  );
}
