import { X } from 'lucide-react';
import { useEffect, useId, useState } from 'react';
import { t, locale, type I18nKey } from '@/i18n';
import { errorMessage } from '@/lib/errors';
import { loadMaterials } from '@/features/materials/actions';
import { useArtPanel } from '@/features/art/artStore';
import { loadUserArt } from '@/features/art/actions';
import { loadCameraSetup } from '@/features/camera/actions';
import { useMaterials } from '@/features/materials/state';
import { useStore } from '@/store';
import { Modal } from '@/ui/Modal';
import { applyImport, currentState, importBlockedReason } from './apply';
import { camerasSummary } from './cameras';
import { baseName, exportUserBundle, pickBundleToOpen, readUserBundle } from './files';
import {
  allSections,
  BUNDLE_SECTIONS,
  planImport,
  presentSections,
  sectionChanges,
  type BundleSection,
  type ImportPlan,
  type InvalidItem,
  type SectionChoice,
} from './model';
import { useBundleUi, type BundleMode } from './uiStore';
import './bundle.css';

const sectionLabel = (s: BundleSection) => t(`bundle.section.${s}` as I18nKey);

function reasonText(item: InvalidItem): string {
  const reason = t(`bundle.reason.${item.reason}` as I18nKey, { fields: (item.fields ?? []).join(', ') });
  return t('bundle.invalid.item', { id: item.id, reason });
}

function Section({ id, checked, disabled, detail, onChange, children }: {
  id: BundleSection;
  checked: boolean;
  disabled?: boolean;
  detail?: string;
  onChange: (value: boolean) => void;
  children?: React.ReactNode;
}) {
  return (
    <div className="bnd-section" data-section={id} data-off={!checked || undefined}>
      <label className="bnd-check">
        <input type="checkbox" checked={checked} disabled={disabled} onChange={(e) => onChange(e.target.checked)} />
        <span>{sectionLabel(id)}</span>
      </label>
      {detail && <p className="bnd-detail">{detail}</p>}
      {children}
    </div>
  );
}

// ---------------------------------------------------------------------------
// Exportar
// ---------------------------------------------------------------------------

export function ExportBody({ choice, onChange, counts }: { choice: SectionChoice; onChange: (c: SectionChoice) => void; counts: { devices: number; materials: number; art?: number; presets?: number; lens?: boolean; alignment?: boolean; head?: boolean } }) {
  const parts = [counts.lens && t('bundle.export.cameras.lens'), counts.alignment && t('bundle.export.cameras.alignment'), counts.head && t('bundle.export.cameras.head')].filter((x): x is string => !!x);
  const detail: Record<BundleSection, string> = {
    devices: t('bundle.export.devices', { n: counts.devices }),
    materials: t('bundle.export.materials', { n: counts.materials }),
    cameras: parts.length + (counts.presets ?? 0) > 0 ? t('bundle.export.cameras', { parts: parts.join(', ') || t('bundle.export.cameras.noParts'), n: counts.presets ?? 0 }) : t('bundle.export.cameras.none'),
    art: t('bundle.export.art', { n: counts.art ?? 0 }),
    prefs: t('bundle.export.prefs'),
  };
  return (
    <div className="bnd-body">
      <p className="hint">{t('bundle.export.hint')}</p>
      {BUNDLE_SECTIONS.map((s) => (
        <Section key={s} id={s} checked={choice[s]} detail={detail[s]} onChange={(v) => onChange({ ...choice, [s]: v })} />
      ))}
    </div>
  );
}

// ---------------------------------------------------------------------------
// Importar: vista previa
// ---------------------------------------------------------------------------

/** Vista previa de lo que aplicaría un paquete, con una casilla por sección. No escribe nada. */
export function ImportPreview({ plan, choice, onChange }: { plan: ImportPlan; choice: SectionChoice; onChange: (c: SectionChoice) => void }) {
  const present = presentSections(plan);
  if (present.length === 0) return <p className="hint">{t('bundle.import.empty')}</p>;
  const changes = sectionChanges(plan);
  const toggle = (s: BundleSection) => (v: boolean) => onChange({ ...choice, [s]: v });
  const d = plan.devices;
  const m = plan.materials;
  const p = plan.prefs?.change;
  const c = plan.cameras;
  const a = plan.art;
  const activeName = d?.activeDevice ? ([...d.added, ...d.replaced, ...d.unchanged].find((x) => x.id === d.activeDevice)?.name ?? d.activeDevice) : null;
  return (
    <>
      <Section id="devices" checked={choice.devices && !!d} disabled={!d} onChange={toggle('devices')} detail={d ? undefined : t('bundle.import.absent')}>
        {d && (
          <>
            <p className="bnd-detail">{t('bundle.devices.summary', { added: d.added.length, replaced: d.replaced.length, unchanged: d.unchanged.length })}</p>
            {d.added.length > 0 && <ul className="bnd-list">{d.added.map((x) => <li key={x.id}>{x.name}</li>)}</ul>}
            {d.replaced.length > 0 && <p className="bnd-detail bnd-detail--warn">{t('bundle.devices.replaced', { names: d.replaced.map((x) => x.name).join(', ') })}</p>}
            {activeName && <p className="bnd-detail">{t('bundle.devices.active', { name: activeName })}</p>}
            {d.invalid.length > 0 && (
              <>
                <p className="bnd-detail bnd-detail--warn">{t('bundle.devices.invalid', { n: d.invalid.length })}</p>
                <ul className="bnd-list">{d.invalid.map((i, k) => <li key={k}>{reasonText(i)}</li>)}</ul>
              </>
            )}
          </>
        )}
      </Section>
      <Section id="materials" checked={choice.materials && !!m} disabled={!m} onChange={toggle('materials')} detail={m ? undefined : t('bundle.import.absent')}>
        {m && (
          <>
            <p className="bnd-detail">{t('bundle.materials.summary', { added: m.added, skipped: m.skipped })}</p>
            {m.renamed > 0 && <p className="bnd-detail">{t('bundle.materials.renamed', { n: m.renamed })}</p>}
            {m.invalid > 0 && <p className="bnd-detail bnd-detail--warn">{t('bundle.materials.invalid', { n: m.invalid })}</p>}
          </>
        )}
      </Section>
      <Section id="cameras" checked={choice.cameras && !!c} disabled={!c} onChange={toggle('cameras')} detail={c ? undefined : t('bundle.import.absent')}>
        {c && (
          <>
            {([['calibration', c.calibration], ['alignment', c.alignment], ['head', c.head]] as const).map(([part, state]) =>
              state ? <p key={part} className={`bnd-detail${state === 'replace' ? ' bnd-detail--warn' : ''}`}>{t(`bundle.cameras.${part}.${state}` as I18nKey)}</p> : null,
            )}
            {c.cleared.map((part) => <p key={part} className="bnd-detail bnd-detail--warn">{t(`bundle.cameras.cleared.${part}` as I18nKey)}</p>)}
            {c.presets && <p className="bnd-detail">{t('bundle.cameras.presets', { added: c.presets.added.length, skipped: c.presets.skipped })}</p>}
            {c.presets && c.presets.renamed > 0 && <p className="bnd-detail">{t('bundle.cameras.renamed', { n: c.presets.renamed })}</p>}
            {c.presets && c.presets.strippedUrls > 0 && <p className="bnd-detail bnd-detail--warn">{t('bundle.cameras.stripped', { n: c.presets.strippedUrls })}</p>}
            {c.presets && c.presets.dropped > 0 && <p className="bnd-detail bnd-detail--warn">{t('bundle.cameras.dropped', { n: c.presets.dropped })}</p>}
            {(c.presets?.invalid ?? 0) + c.invalid.length > 0 && <p className="bnd-detail bnd-detail--warn">{t('bundle.cameras.invalid', { n: (c.presets?.invalid ?? 0) + c.invalid.length })}</p>}
            {!c.calibration && !c.alignment && !c.head && !c.presets && <p className="bnd-detail">{t('bundle.cameras.empty')}</p>}
          </>
        )}
      </Section>
      <Section id="art" checked={choice.art && !!a} disabled={!a} onChange={toggle('art')} detail={a ? undefined : t('bundle.import.absent')}>
        {a && (
          <>
            <p className="bnd-detail">{t('bundle.art.summary', { added: a.items.length, skipped: a.skipped })}</p>
            {a.items.length > 0 && <ul className="bnd-list">{a.items.slice(0, 8).map((x, i) => <li key={`${x.name}-${i}`}>{x.name}</li>)}{a.items.length > 8 && <li>{t('bundle.art.more', { n: a.items.length - 8 })}</li>}</ul>}
            {a.renamed > 0 && <p className="bnd-detail">{t('bundle.art.renamed', { n: a.renamed })}</p>}
            {a.invalid > 0 && <p className="bnd-detail bnd-detail--warn">{t('bundle.art.invalid', { n: a.invalid })}</p>}
          </>
        )}
      </Section>
      <Section id="prefs" checked={choice.prefs && !!p} disabled={!p} onChange={toggle('prefs')} detail={p ? undefined : t('bundle.import.absent')}>
        {p && (
          <>
            <p className="bnd-detail">{t('bundle.prefs.summary', { prefs: p.appliedPrefs, shortcuts: p.appliedShortcuts })}</p>
            {p.appliedShortcuts > 0 && <p className="bnd-detail">{t('bundle.prefs.replaceShortcuts')}</p>}
            {p.ignoredPrefs > 0 && <p className="bnd-detail bnd-detail--warn">{t('bundle.prefs.ignored', { n: p.ignoredPrefs })}</p>}
            {p.unknownCommands > 0 && <p className="bnd-detail bnd-detail--warn">{t('bundle.prefs.unknown', { n: p.unknownCommands })}</p>}
          </>
        )}
      </Section>
      {BUNDLE_SECTIONS.every((s) => plan[s] === null || changes[s] === 0) && <p className="hint">{t('bundle.import.nothing')}</p>}
    </>
  );
}

/** Secciones marcadas que el paquete trae y que cambian algo. */
export function applicableSections(plan: ImportPlan, choice: SectionChoice): SectionChoice {
  const changes = sectionChanges(plan);
  return {
    devices: choice.devices && plan.devices !== null && changes.devices > 0,
    materials: choice.materials && plan.materials !== null && changes.materials > 0,
    cameras: choice.cameras && plan.cameras !== null && changes.cameras > 0,
    art: choice.art && plan.art !== null && changes.art > 0,
    prefs: choice.prefs && plan.prefs !== null,
  };
}

/** Marco común: cabecera con título y cierre, cuerpo y pie. */
function Frame({ title, onClose, children, foot }: { title: string; onClose: () => void; children: React.ReactNode; foot: React.ReactNode }) {
  const titleId = useId();
  return (
    <Modal labelId={titleId} onClose={onClose} backdropClassName="modal" cardClassName="modal__card bnd-card" initialFocus="[data-autofocus]" fallbackFocus="first" guardOutside>
      <div className="modal__head">
        <h2 id={titleId} className="modal__title">{title}</h2>
        <button type="button" className="icon-btn" aria-label={t('bundle.close')} title={t('bundle.close')} onClick={onClose}>
          <X size={16} aria-hidden="true" />
        </button>
      </div>
      {children}
      <div className="modal__foot bnd-foot">{foot}</div>
    </Modal>
  );
}

function ImportDialog({ onClose }: { onClose: () => void }) {
  const [file, setFile] = useState<{ name: string; plan: ImportPlan } | null>(null);
  const [choice, setChoice] = useState<SectionChoice>(allSections());
  const [busy, setBusy] = useState(false);
  const blocked = useStore((s) => importBlockedReason(s));
  const notify = useStore((s) => s.notify);

  const pick = async () => {
    const path = await pickBundleToOpen();
    if (!path) return;
    const name = baseName(path);
    try {
      const bundle = await readUserBundle(path);
      const plan = planImport(bundle, await currentState());
      setFile({ name, plan });
      setChoice(Object.fromEntries(BUNDLE_SECTIONS.map((s) => [s, plan[s] !== null])) as SectionChoice);
    } catch (e) {
      notify('error', t('bundle.notice.importFailed', { name, error: errorMessage(e) }));
    }
  };

  const apply = async () => {
    if (!file) return;
    setBusy(true);
    try {
      const result = await applyImport(file.plan, applicableSections(file.plan, choice), file.name);
      if (!result.blocked) onClose();
    } finally {
      setBusy(false);
    }
  };

  const chosen = file ? applicableSections(file.plan, choice) : null;
  const canApply = !!chosen && !busy && !blocked && BUNDLE_SECTIONS.some((s) => chosen[s]);
  const date = file?.plan.created ? new Date(file.plan.created).toLocaleString(locale()) : '';
  const problem: I18nKey | null = blocked ?? (file && !canApply && !busy ? 'bundle.import.noneChecked' : null);

  const foot = (
    <>
      {problem && <span className="hint bnd-foot__hint">{t(problem)}</span>}
      <button type="button" className="btn" data-autofocus={file ? undefined : true} disabled={busy} onClick={() => void pick()}>
        {file ? t('bundle.import.another') : t('bundle.import.pick')}
      </button>
      <button type="button" className="btn" onClick={onClose}>{t('bundle.cancel')}</button>
      {file && (
        <button type="button" className="btn btn--accent" data-autofocus aria-disabled={!canApply} onClick={() => canApply && void apply()}>
          {busy ? t('bundle.import.applying') : t('bundle.import.apply')}
        </button>
      )}
    </>
  );
  return (
    <Frame title={t('bundle.title.import')} onClose={onClose} foot={foot}>
      <div className="bnd-body">
        <p className="hint">{t('bundle.import.hint')}</p>
        {file && (
          <>
            <p className="bnd-meta hint">
              {file.plan.app ? t('bundle.import.meta', { name: file.name, date, app: file.plan.app }) : t('bundle.import.metaNoApp', { name: file.name, date })}
            </p>
            <h3 className="field__label">{t('bundle.import.preview')}</h3>
            <ImportPreview plan={file.plan} choice={choice} onChange={setChoice} />
          </>
        )}
      </div>
    </Frame>
  );
}

function ExportDialog({ onClose }: { onClose: () => void }) {
  const [choice, setChoice] = useState<SectionChoice>(allSections());
  const [busy, setBusy] = useState(false);
  const devices = useStore((s) => s.devices.length);
  const materials = useMaterials((s) => s.library.presets.length);
  const art = useArtPanel((s) => s.user.length);
  const camera = useStore((s) => s.cameraSetup);
  const none = !BUNDLE_SECTIONS.some((s) => choice[s]);
  const cams = camerasSummary(camera);
  // Los números salen de la biblioteca, la cámara y el arte; se leen del disco si todavía no se habían leído.
  useEffect(() => {
    void loadMaterials();
    void loadCameraSetup();
    void loadUserArt();
  }, []);

  const doExport = async () => {
    setBusy(true);
    try {
      if (await exportUserBundle(choice)) onClose();
    } finally {
      setBusy(false);
    }
  };
  const foot = (
    <>
      {none && <span className="hint bnd-foot__hint">{t('bundle.export.none')}</span>}
      <button type="button" className="btn" onClick={onClose}>{t('bundle.cancel')}</button>
      <button type="button" className="btn btn--accent" data-autofocus aria-disabled={none || busy} onClick={() => !none && !busy && void doExport()}>
        {t('bundle.export.action')}
      </button>
    </>
  );
  return (
    <Frame title={t('bundle.title.export')} onClose={onClose} foot={foot}>
      <ExportBody choice={choice} onChange={setChoice} counts={{ devices, materials, art, presets: cams.presets, lens: cams.lens, alignment: cams.alignment, head: cams.head }} />
    </Frame>
  );
}

export function BundleDialog({ mode, onClose }: { mode: BundleMode; onClose: () => void }) {
  return mode === 'export' ? <ExportDialog onClose={onClose} /> : <ImportDialog onClose={onClose} />;
}

/** Montaje del diálogo: se pinta solo cuando algún comando lo abrió. */
export function BundleHost() {
  const mode = useBundleUi((s) => s.mode);
  const close = useBundleUi((s) => s.closeBundle);
  return mode ? <BundleDialog mode={mode} onClose={close} /> : null;
}
