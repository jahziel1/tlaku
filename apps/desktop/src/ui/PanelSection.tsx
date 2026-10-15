import { ChevronRight } from 'lucide-react';
import { useCallback, useEffect, useId, useRef, type ReactNode } from 'react';
import { isSectionOpen, SECTION_PARENT, sectionTab } from '@/lib/panelSections';
import { usePref, useStore } from '@/store';

/** Evento interno: alguien pidió mostrar una sección (`revealPanelSection`). */
const REVEAL_EVENT = 'tlaku:reveal-section';

/** Sección pedida que todavía no se mostró (la pestaña o la sección pueden montarse después de pedirla). */
let pendingReveal: { id: string; focus: boolean } | null = null;

/**
 * Estado abierto/cerrado de una sección, recordado por sección en las preferencias de la app
 * (`panelSections`). Sin elección del usuario manda `defaultOpen`.
 */
export function useSectionOpen(id: string, defaultOpen: boolean): [boolean, (open?: boolean) => void] {
  const saved = usePref('panelSections');
  const setPref = useStore((s) => s.setPref);
  const open = isSectionOpen(saved, id, defaultOpen);
  const set = useCallback(
    (next?: boolean) => {
      const cur = useStore.getState().prefs.panelSections;
      const value = next ?? !isSectionOpen(cur, id, defaultOpen);
      setPref('panelSections', { ...cur, [id]: value });
    },
    [id, defaultOpen, setPref],
  );
  return [open, set];
}

/** Vuelve todas las secciones a su estado por omisión. */
export function resetPanelSections(): void {
  useStore.getState().setPref('panelSections', {});
}

/**
 * Abre la sección `id` (y la que la contiene), muestra su pestaña y la deja visible dentro del panel
 * (sin desplazar la ventana: solo se mueve el contenedor `.panel-scroll`). El foco va a su encabezado.
 */
export function revealPanelSection(id: string, opts: { focus?: boolean } = {}): void {
  const s = useStore.getState();
  const open: Record<string, boolean> = {};
  for (let cur: string | undefined = id; cur; cur = SECTION_PARENT[cur]) open[cur] = true;
  s.setPref('panelSections', { ...s.prefs.panelSections, ...open });
  const tab = sectionTab(id);
  if (tab) s.setDockTab(tab);
  pendingReveal = { id, focus: opts.focus ?? true };
  window.dispatchEvent(new Event(REVEAL_EVENT));
}

/** Desplaza solo el contenedor del panel para que `el` quede a la vista. */
function scrollIntoPanel(el: HTMLElement): void {
  const box = el.closest<HTMLElement>('.panel-scroll');
  if (!box) return;
  const b = box.getBoundingClientRect();
  const r = el.getBoundingClientRect();
  const pad = 8;
  if (r.top < b.top + pad) box.scrollTop += r.top - b.top - pad;
  else if (r.bottom > b.bottom - pad) box.scrollTop += Math.min(r.bottom - b.bottom + pad, r.top - b.top - pad);
}

interface Props {
  /** Id estable (`machine.job`...): es la clave con que se recuerda el estado. */
  id: string;
  title: ReactNode;
  /** Abierta por omisión. */
  defaultOpen?: boolean;
  /** Una línea que resume el contenido; se ve con la sección cerrada (o siempre con `summaryAlways`). */
  summary?: ReactNode;
  summaryAlways?: boolean;
  /** Controles junto al título (por ejemplo «Desconectar»); siempre visibles. */
  actions?: ReactNode;
  /** Sección dentro de otra: encabezado de nivel 4 y sin borde propio. */
  nested?: boolean;
  /** Sin pliegue: el título es un encabezado normal y el contenido siempre se ve. */
  alwaysOpen?: boolean;
  className?: string;
  /** Contenido que se ve aunque la sección esté cerrada (avisos de seguridad). */
  always?: ReactNode;
  children: ReactNode;
}

/**
 * Sección plegable de un panel (acordeón accesible): el encabezado es un botón con `aria-expanded` y
 * `aria-controls`, se opera con Enter o Espacio y el estado se recuerda por sección. El contenido
 * cerrado sigue montado (no pierde lo escrito) pero oculto.
 */
export function PanelSection({ id, title, defaultOpen = false, summary, summaryAlways = false, actions, nested = false, alwaysOpen = false, className, always, children }: Props) {
  const uid = useId();
  const headId = `${uid}-h`;
  const bodyId = `${uid}-b`;
  const [savedOpen, setOpen] = useSectionOpen(id, defaultOpen);
  const open = alwaysOpen || savedOpen;
  const ref = useRef<HTMLElement>(null);
  const openRef = useRef(open);
  openRef.current = open;

  useEffect(() => {
    const run = () => {
      const p = pendingReveal;
      const el = ref.current;
      if (!p || p.id !== id || !el || !openRef.current) return;
      pendingReveal = null;
      // Esperar al pintado: la pestaña o la sección pueden acabar de abrirse.
      requestAnimationFrame(() => {
        scrollIntoPanel(el);
        if (p.focus) el.querySelector<HTMLElement>('[data-psec-toggle]')?.focus({ preventScroll: true });
      });
    };
    run();
    window.addEventListener(REVEAL_EVENT, run);
    return () => window.removeEventListener(REVEAL_EVENT, run);
  }, [id, open]);

  const Heading = nested ? 'h4' : 'h3';
  const showSummary = summary != null && (summaryAlways || !open);
  return (
    <section ref={ref} className={`${nested ? 'psec psec--nested' : 'section psec'}${className ? ` ${className}` : ''}`} aria-labelledby={headId} data-section={id} data-open={open}>
      <div className="psec__bar">
        <Heading id={headId} className={nested ? 'mc-sub psec__head' : 'panel-title psec__head'}>
          {alwaysOpen ? (
            title
          ) : (
            <button type="button" className="psec__toggle" data-psec-toggle aria-expanded={open} aria-controls={bodyId} onClick={() => setOpen()}>
              <ChevronRight size={14} className="psec__chev" aria-hidden="true" />
              <span>{title}</span>
            </button>
          )}
        </Heading>
        {actions}
      </div>
      {showSummary && <div className="psec__summary">{summary}</div>}
      {always}
      <div id={bodyId} className="psec__body" hidden={!open}>
        {children}
      </div>
    </section>
  );
}
