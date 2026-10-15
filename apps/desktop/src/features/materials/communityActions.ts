/**
 * Acciones del navegador de ajustes de la comunidad. Nada de aquí toca la máquina ni envía datos:
 * descargar el catálogo es un `GET` de solo lectura que hace Rust a un destino fijo; «Usar» copia un
 * ajuste a la biblioteca local; «Aplicar» cambia una capa del diseño (con confirmación y paso de
 * deshacer); «Compartir» solo abre el formulario de GitHub en el navegador.
 */
import { getApi } from '@/api';
import type { CatalogEntry, CatalogState } from '@/api/types';
import { askChoice } from '@/features/history/prompt';
import { useHelpUi } from '@/features/help/helpStore';
import { t, tn, locale } from '@/i18n';
import { errorMessage } from '@/lib/errors';
import { useStore } from '@/store';
import { activeLayerOf, addPreset, applyMaterial, loadMaterials } from './actions';
import { compatOf, entrySummary, isOnlyIndicative, presetFromEntry, type Compat, type MyMachine } from './communityModel';
import { useCommunity } from './communityStore';
import { idsOf, presetFromLayer, uniqueId } from './library';
import { useMaterials } from './state';

const ui = () => useCommunity.getState();
const app = () => useStore.getState();

// ---------------------------------------------------------------------------
// Abrir y actualizar
// ---------------------------------------------------------------------------

let inflight: Promise<void> | null = null;

/**
 * Baja el catálogo (la única llamada de red del navegador). La interfaz la hace solo al abrirlo o
 * cuando la persona pulsa «Actualizar»; si ya hay una en curso, espera esa.
 */
export function refreshCommunity(): Promise<void> {
  if (inflight) return inflight;
  ui().setStatus('loading');
  inflight = getApi()
    .communityCatalogRefresh()
    .then((state) => ui().setCatalog(state))
    .catch((e: unknown) => {
      // El motor no falla por la red (la deja en `error`); esto es un fallo de la propia llamada.
      const previous = ui().catalog;
      const error = errorMessage(e);
      ui().setCatalog(previous ? { ...previous, error } : { index: null, source: 'none', fetched_at: null, error });
    })
    .finally(() => {
      inflight = null;
      ui().setStatus('done');
    });
  return inflight;
}

/**
 * Abre el navegador: primero muestra la última copia guardada (sin red) y enseguida pide el catálogo
 * al sitio. Es la única vía por la que la app descarga el catálogo.
 */
export async function openCommunity(): Promise<void> {
  ui().setOpen(true);
  void loadMaterials();
  try {
    const cached: CatalogState = await getApi().communityCatalogCached();
    if (ui().open) ui().setCatalog(cached);
  } catch (e) {
    if (ui().open) ui().setCatalog({ index: null, source: 'none', fetched_at: null, error: errorMessage(e) });
  }
  if (ui().open) await refreshCommunity();
}

export function closeCommunity(): void {
  ui().setOpen(false);
}

// ---------------------------------------------------------------------------
// Usar un ajuste
// ---------------------------------------------------------------------------

/** Notas del ajuste que entra a la biblioteca: de dónde viene, en qué máquina se probó y lo que dijo quien lo aportó. */
export function entryNotes(e: CatalogEntry): string {
  const parts = [
    e.contributor ? t('materials.community.notes.from', { alias: e.contributor }) : t('materials.community.notes.fromAnon'),
    `${e.machine} · ${e.laser_watts} W`,
    e.verified_by > 0 ? tn('materials.community.verified', e.verified_by) : '',
    e.notes ?? '',
  ];
  return parts.filter(Boolean).join(' · ');
}

/** Resultado de copiar un ajuste a la biblioteca. */
export type CopyResult = 'added' | 'exists' | 'blocked';

/**
 * «Usar»: copia el ajuste a «Mis materiales» con su origen (la dirección del aporte, confianza «foro»).
 * No toca ninguna capa ni la máquina. Si ya está (mismo aporte con los mismos valores) no lo duplica.
 */
export async function copyEntryToLibrary(e: CatalogEntry): Promise<CopyResult> {
  await loadMaterials();
  const { loadError, library } = useMaterials.getState();
  if (loadError) {
    app().notify('error', t('materials.community.use.blocked'));
    return 'blocked';
  }
  const preset = presetFromEntry(e, uniqueId(idsOf(library.presets)), entryNotes(e));
  const same = library.presets.find(
    (p) => p.source === e.source && p.settings.speed === e.speed && p.settings.power === e.power && p.settings.passes === (e.passes ?? 1) && p.settings.mode === e.mode,
  );
  if (same) {
    app().notify('info', t('materials.community.use.exists'));
    return 'exists';
  }
  addPreset(preset);
  app().notify('success', t('materials.community.use.added', { name: `${preset.material} · ${preset.title}` }));
  return 'added';
}

const COMPAT_WARNING: Record<Compat, Parameters<typeof t>[0]> = {
  model: 'materials.community.compat.model.warn',
  class: 'materials.community.compat.class.warn',
  kind: 'materials.community.compat.kind.warn',
  other: 'materials.community.compat.other.warn',
};

/**
 * «Aplicar a la capa seleccionada»: pregunta antes (con los valores y, si la máquina del ajuste no es la
 * mía, el aviso de que es solo orientativo) y recién después copia los ajustes a la capa, con un paso
 * de deshacer. Nunca envía nada a la máquina: el trabajo sigue pasando por la revisión y el envío normales.
 */
export async function applyEntryToLayer(e: CatalogEntry, me: MyMachine): Promise<boolean> {
  const layer = activeLayerOf(app());
  if (!layer) {
    app().notify('warning', t('materials.reason.noLayer'));
    return false;
  }
  const compat = compatOf(e, me);
  const preset = presetFromEntry(e, 'community-apply', entryNotes(e));
  const name = layer.name || layer.color;
  const message = [
    t('materials.community.apply.ask', { layer: name, summary: entrySummary(e) }),
    isOnlyIndicative(compat) ? t(COMPAT_WARNING[compat], { mine: me.name, theirs: e.machine, watts: e.laser_watts }) : t(COMPAT_WARNING.model),
    t('materials.community.apply.test'),
  ].join('\n\n');
  const answer = await askChoice({
    title: t('materials.community.apply.title'),
    message,
    choices: [
      { id: 'apply', label: t('materials.community.apply.confirm'), tone: isOnlyIndicative(compat) ? 'danger' : 'accent' },
      { id: 'cancel', label: t('materials.community.apply.cancel') },
    ],
    cancelId: 'cancel',
  });
  if (answer !== 'apply') return false;
  return applyMaterial(preset);
}

// ---------------------------------------------------------------------------
// Compartir
// ---------------------------------------------------------------------------

/**
 * «Compartir un ajuste…»: abre el diálogo de compartir con los valores de la capa seleccionada (sin el
 * material, que lo escribe quien comparte). El diálogo solo arma la dirección del formulario de GitHub;
 * no envía nada.
 */
export function shareFromActiveLayer(): void {
  const layer = activeLayerOf(app());
  const preset = layer
    ? presetFromLayer(layer, { material: '', thickness: null, title: layer.name, notes: '', device_id: null }, 'share-draft')
    : null;
  closeCommunity();
  useHelpUi.getState().openShareMaterial(preset);
}

/** Fecha y hora de la copia, en el idioma de la interfaz. */
export function formatFetched(ms: number): string {
  return new Intl.DateTimeFormat(locale(), { dateStyle: 'medium', timeStyle: 'short' }).format(new Date(ms));
}
