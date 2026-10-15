import { useEffect, useId, useState } from 'react';
import { getApi } from '@/api';
import { t, locale, type I18nKey } from '@/i18n';
import { errorMessage } from '@/lib/errors';
import { loadMaterials } from '@/features/materials/actions';
import { useArtPanel } from '@/features/art/artStore';
import { loadUserArt } from '@/features/art/actions';
import { loadCameraSetup } from '@/features/camera/actions';
import { useMaterials } from '@/features/materials/state';
import { loadRemnants, useRemnants } from '@/features/nest/remnantStore';
import { BackupDialog } from '@/features/backup/BackupDialog';
import { NewerBackupPrompt } from '@/features/backup/NewerBackupPrompt';
import { useBackupStartup } from '@/features/backup/startup';
import { useStore } from '@/store';
import { applyImport, currentState, importBlockedReason } from './apply';
import { camerasSummary } from './cameras';
import { baseName, exportUserBundle, pickBundleToOpen, readUserBundle } from './files';
import { CONFLICT_POLICIES, defaultPolicies, type ConflictPolicy, type ConflictSection, type PolicyChoice } from './extras';
import {
  BUNDLE_SECTIONS,
  conflictCounts,
  defaultSections,
  planImport,
  presentSections,
  sectionChanges,
  type BundleSection,
  type ImportPlan,
  type InvalidItem,
  type SectionChoice,
} from './model';
import { currentCosts } from './sources';
import { Frame } from './Frame';
import { useBundleUi, type BundleMode, type ImportSource } from './uiStore';
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

export function ExportBody({ choice, onChange, counts }: { choice: SectionChoice; onChange: (c: SectionChoice) => void; counts: { devices: number; materials: number; art?: number; presets?: number; lens?: boolean; alignment?: boolean; head?: boolean; costProfiles?: number; remnants?: number } }) {
  const parts = [counts.lens && t('bundle.export.cameras.lens'), counts.alignment && t('bundle.export.cameras.alignment'), counts.head && t('bundle.export.cameras.head')].filter((x): x is string => !!x);
  const detail: Record<BundleSection, string> = {
    devices: t('bundle.export.devices', { n: counts.devices }),
    materials: t('bundle.export.materials', { n: counts.materials }),
    cameras: parts.length + (counts.presets ?? 0) > 0 ? t('bundle.export.cameras', { parts: parts.join(', ') || t('bundle.export.cameras.noParts'), n: counts.presets ?? 0 }) : t('bundle.export.cameras.none'),
    art: t('bundle.export.art', { n: counts.art ?? 0 }),
    prefs: t('bundle.export.prefs'),
    costs: t('bundle.export.costs', { n: counts.costProfiles ?? 0 }),
    remnants: t('bundle.export.remnants', { n: counts.remnants ?? 0 }),
    templates: t('bundle.export.templates'),
    history: t('bundle.export.history'),
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

/** Selector de qué hacer con los elementos que chocan por id con uno local distinto. */
function PolicyPick({ section, n, value, onChange }: { section: ConflictSection; n: number; value: ConflictPolicy; onChange: (p: ConflictPolicy) => void }) {
  const id = useId();
  if (n === 0) return null;
  return (
    <div className="bnd-policy" data-policy-for={section}>
      <label htmlFor={id} className="bnd-policy__label">{t('bundle.policy.label', { n })}</label>
      <select id={id} className="select" value={value} onChange={(e) => onChange(e.target.value as ConflictPolicy)}>
        {CONFLICT_POLICIES.map((p) => (
          <option key={p} value={p}>{t(`bundle.policy.${p}` as I18nKey)}</option>
        ))}
      </select>
    </div>
  );
}

const listNames = (names: string[], max = 8) => (
  <ul className="bnd-list">
    {names.slice(0, max).map((n, i) => <li key={`${n}-${i}`}>{n}</li>)}
    {names.length > max && <li>{t('bundle.art.more', { n: names.length - max })}</li>}
  </ul>
);

/**
 * Vista previa de lo que aplicaría un paquete, con una casilla por sección y, donde hay conflictos
 * por id, qué hacer con ellos (reemplazar, conservar ambos u omitir). No escribe nada.
 */
export function ImportPreview({ plan, choice, onChange, policies = defaultPolicies(), onPolicies }: {
  plan: ImportPlan;
  choice: SectionChoice;
  onChange: (c: SectionChoice) => void;
  policies?: PolicyChoice;
  onPolicies?: (p: PolicyChoice) => void;
}) {
  const present = presentSections(plan);
  if (present.length === 0) return <p className="hint">{t('bundle.import.empty')}</p>;
  const changes = sectionChanges(plan, policies);
  const conflicts = conflictCounts(plan);
  const toggle = (s: BundleSection) => (v: boolean) => onChange({ ...choice, [s]: v });
  const pick = (s: ConflictSection) => <PolicyPick section={s} n={conflicts[s]} value={policies[s]} onChange={(p) => onPolicies?.({ ...policies, [s]: p })} />;
  const d = plan.devices;
  const m = plan.materials;
  const p = plan.prefs;
  const c = plan.cameras;
  const a = plan.art;
  const co = plan.costs;
  const rem = plan.remnants;
  const tpl = plan.templates;
  const h = plan.history;
  const activeName = d?.activeDevice ? ([...d.added, ...d.replaced, ...d.unchanged].find((x) => x.id === d.activeDevice)?.name ?? d.activeDevice) : null;
  const sec = (id: BundleSection, present: boolean, body: React.ReactNode) => (
    <Section key={id} id={id} checked={choice[id] && present} disabled={!present} onChange={toggle(id)} detail={present ? undefined : t('bundle.import.absent')}>
      {present && body}
    </Section>
  );
  const sections: Record<BundleSection, React.ReactNode> = {
    devices: sec('devices', !!d, d && (
      <>
        <p className="bnd-detail">{t('bundle.devices.summary', { added: d.added.length, replaced: d.replaced.length, unchanged: d.unchanged.length })}</p>
        {d.added.length > 0 && listNames(d.added.map((x) => x.name))}
        {d.replaced.length > 0 && <p className="bnd-detail bnd-detail--warn">{t('bundle.devices.replaced', { names: d.replaced.map((x) => x.name).join(', ') })}</p>}
        {pick('devices')}
        {activeName && <p className="bnd-detail">{t('bundle.devices.active', { name: activeName })}</p>}
        {d.invalid.length > 0 && (
          <>
            <p className="bnd-detail bnd-detail--warn">{t('bundle.devices.invalid', { n: d.invalid.length })}</p>
            <ul className="bnd-list">{d.invalid.map((i, k) => <li key={k}>{reasonText(i)}</li>)}</ul>
          </>
        )}
      </>
    )),
    materials: sec('materials', !!m, m && (
      <>
        <p className="bnd-detail">{t('bundle.materials.summary', { added: m.diff.added.length, conflicts: m.diff.conflicts.length, same: m.diff.unchanged.length })}</p>
        {m.diff.added.length > 0 && listNames(m.diff.added.map((x) => x.title || x.material))}
        {pick('materials')}
        {m.invalid > 0 && <p className="bnd-detail bnd-detail--warn">{t('bundle.materials.invalid', { n: m.invalid })}</p>}
      </>
    )),
    costs: sec('costs', !!co, co && (
      <>
        <p className="bnd-detail">{t('bundle.costs.machines', { added: co.machines.added.length, conflicts: co.machines.conflicts.length, same: co.machines.unchanged.length })}</p>
        <p className="bnd-detail">{t('bundle.costs.materials', { added: co.materials.added.length, conflicts: co.materials.conflicts.length, same: co.materials.unchanged.length })}</p>
        {co.sales && <p className={`bnd-detail${co.sales === 'different' ? ' bnd-detail--warn' : ''}`}>{t(co.sales === 'different' ? 'bundle.costs.sales.different' : 'bundle.costs.sales.same')}</p>}
        {pick('costs')}
        {(co.machines.conflicts.length > 0 || co.sales === 'different') && policies.costs !== 'replace' && <p className="bnd-detail">{t('bundle.costs.keepLocal')}</p>}
        {co.invalid > 0 && <p className="bnd-detail bnd-detail--warn">{t('bundle.costs.invalid', { n: co.invalid })}</p>}
      </>
    )),
    remnants: sec('remnants', !!rem, rem && (
      <>
        <p className="bnd-detail">{t('bundle.remnants.summary', { added: rem.diff.added.length, conflicts: rem.diff.conflicts.length, same: rem.diff.unchanged.length })}</p>
        {rem.diff.added.length > 0 && listNames(rem.diff.added.map((x) => x.name))}
        {pick('remnants')}
        {rem.invalid > 0 && <p className="bnd-detail bnd-detail--warn">{t('bundle.remnants.invalid', { n: rem.invalid })}</p>}
      </>
    )),
    templates: sec('templates', !!tpl, tpl && (
      <>
        <p className="bnd-detail">{t('bundle.templates.summary', { added: tpl.diff.added.length, conflicts: tpl.diff.conflicts.length, same: tpl.diff.unchanged.length })}</p>
        {tpl.diff.added.length > 0 && listNames(tpl.diff.added.map((x) => x.name))}
        {pick('templates')}
        {tpl.invalid > 0 && <p className="bnd-detail bnd-detail--warn">{t('bundle.templates.invalid', { n: tpl.invalid })}</p>}
      </>
    )),
    cameras: sec('cameras', !!c, c && (
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
    )),
    art: sec('art', !!a, a && (
      <>
        <p className="bnd-detail">{t('bundle.art.summary', { added: a.items.length, skipped: a.skipped })}</p>
        {a.items.length > 0 && listNames(a.items.map((x) => x.name))}
        {a.renamed > 0 && <p className="bnd-detail">{t('bundle.art.renamed', { n: a.renamed })}</p>}
        {a.invalid > 0 && <p className="bnd-detail bnd-detail--warn">{t('bundle.art.invalid', { n: a.invalid })}</p>}
      </>
    )),
    prefs: sec('prefs', !!p, p && (
      <>
        <p className="bnd-detail">{t('bundle.prefs.summary', { prefs: p.change.appliedPrefs, shortcuts: p.change.appliedShortcuts })}</p>
        <p className="bnd-detail">{t('bundle.prefs.diff', { changed: p.changedPrefs, same: Math.max(0, p.change.appliedPrefs - p.changedPrefs) })}</p>
        {p.change.appliedShortcuts > 0 && <p className="bnd-detail">{t(p.shortcutsChanged ? 'bundle.prefs.shortcutsChanged' : 'bundle.prefs.shortcutsSame')}</p>}
        {p.change.appliedShortcuts > 0 && <p className="bnd-detail">{t('bundle.prefs.replaceShortcuts')}</p>}
        {p.change.ignoredPrefs > 0 && <p className="bnd-detail bnd-detail--warn">{t('bundle.prefs.ignored', { n: p.change.ignoredPrefs })}</p>}
        {p.change.unknownCommands > 0 && <p className="bnd-detail bnd-detail--warn">{t('bundle.prefs.unknown', { n: p.change.unknownCommands })}</p>}
      </>
    )),
    history: sec('history', !!h, h && (
      <>
        <p className="bnd-detail">{t('bundle.history.summary', { added: h.added.length, same: h.unchanged })}</p>
        {h.invalid > 0 && <p className="bnd-detail bnd-detail--warn">{t('bundle.history.invalid', { n: h.invalid })}</p>}
      </>
    )),
  };
  return (
    <>
      {BUNDLE_SECTIONS.filter((s) => s !== 'history' || h).map((s) => sections[s])}
      {BUNDLE_SECTIONS.every((s) => plan[s] === null || changes[s] === 0) && <p className="hint">{t('bundle.import.nothing')}</p>}
    </>
  );
}

/** Secciones marcadas que el paquete trae y que cambian algo. */
export function applicableSections(plan: ImportPlan, choice: SectionChoice, policies: PolicyChoice = defaultPolicies()): SectionChoice {
  const changes = sectionChanges(plan, policies);
  return Object.fromEntries(
    BUNDLE_SECTIONS.map((s) => [s, choice[s] && plan[s] !== null && (s === 'prefs' || changes[s] > 0)]),
  ) as SectionChoice;
}

function ImportDialog({ onClose, source }: { onClose: () => void; source?: ImportSource | null }) {
  const [file, setFile] = useState<{ name: string; plan: ImportPlan } | null>(null);
  const [choice, setChoice] = useState<SectionChoice>(defaultSections());
  const [policies, setPolicies] = useState<PolicyChoice>(defaultPolicies());
  const [busy, setBusy] = useState(false);
  const [seen, setSeen] = useState<number | undefined>(source?.createdMs);
  const blocked = useStore((s) => importBlockedReason(s));
  const notify = useStore((s) => s.notify);

  const open = async (path: string, createdMs?: number) => {
    const name = baseName(path);
    try {
      const bundle = await readUserBundle(path);
      const plan = planImport(bundle, await currentState(bundle));
      setFile({ name, plan });
      setSeen(createdMs);
      setPolicies(defaultPolicies());
      setChoice(Object.fromEntries(BUNDLE_SECTIONS.map((s) => [s, plan[s] !== null])) as SectionChoice);
    } catch (e) {
      notify('error', t('bundle.notice.importFailed', { name, error: errorMessage(e) }));
    }
  };

  const pick = async () => {
    const path = await pickBundleToOpen();
    if (path) await open(path);
  };

  // Un respaldo elegido en otro diálogo (o ofrecido al abrir la app) se abre solo, con su vista previa.
  useEffect(() => {
    if (source?.path) void open(source.path, source.createdMs);
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [source?.path]);

  const apply = async () => {
    if (!file) return;
    setBusy(true);
    try {
      const result = await applyImport(file.plan, applicableSections(file.plan, choice, policies), file.name, policies);
      if (!result.blocked) {
        if (seen) await getApi().backupMarkSeen({ created_ms: seen }).catch(() => undefined);
        onClose();
      }
    } finally {
      setBusy(false);
    }
  };

  const chosen = file ? applicableSections(file.plan, choice, policies) : null;
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
            <ImportPreview plan={file.plan} choice={choice} onChange={setChoice} policies={policies} onPolicies={setPolicies} />
            <p className="hint">{t('bundle.import.safetyHint')}</p>
          </>
        )}
      </div>
    </Frame>
  );
}

function ExportDialog({ onClose }: { onClose: () => void }) {
  const [choice, setChoice] = useState<SectionChoice>(defaultSections());
  const [busy, setBusy] = useState(false);
  const devices = useStore((s) => s.devices.length);
  const materials = useMaterials((s) => s.library.presets.length);
  const art = useArtPanel((s) => s.user.length);
  const camera = useStore((s) => s.cameraSetup);
  const remnants = useRemnants((s) => s.library.remnants.length);
  const [costProfiles, setCostProfiles] = useState(0);
  const none = !BUNDLE_SECTIONS.some((s) => choice[s]);
  const cams = camerasSummary(camera);
  // Los números salen de la biblioteca, la cámara y el arte; se leen del disco si todavía no se habían leído.
  useEffect(() => {
    void loadMaterials();
    void loadCameraSetup();
    void loadUserArt();
    void loadRemnants();
    setCostProfiles(currentCosts().materials.length);
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
      <ExportBody choice={choice} onChange={setChoice} counts={{ devices, materials, art, presets: cams.presets, lens: cams.lens, alignment: cams.alignment, head: cams.head, costProfiles, remnants }} />
    </Frame>
  );
}

export function BundleDialog({ mode, onClose, source }: { mode: BundleMode; onClose: () => void; source?: ImportSource | null }) {
  if (mode === 'export') return <ExportDialog onClose={onClose} />;
  if (mode === 'backup') return <BackupDialog onClose={onClose} />;
  return <ImportDialog onClose={onClose} source={source} />;
}

/** Montaje del diálogo: se pinta solo cuando algún comando lo abrió. También vigila el respaldo automático. */
export function BundleHost() {
  const mode = useBundleUi((s) => s.mode);
  const source = useBundleUi((s) => s.source);
  const close = useBundleUi((s) => s.closeBundle);
  useBackupStartup();
  return (
    <>
      {mode ? <BundleDialog mode={mode} onClose={close} source={source} /> : null}
      <NewerBackupPrompt />
    </>
  );
}
