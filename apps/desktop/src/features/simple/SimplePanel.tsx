import { ArrowLeft, ArrowRight, Check, FolderOpen, Layers, Move, Eye, Rocket, type LucideIcon } from 'lucide-react';
import { useEffect, useRef } from 'react';
import { loadMaterials } from '@/features/materials/actions';
import { t, type I18nKey } from '@/i18n';
import { useStore } from '@/store';
import { goToStep, nextStep, prevStep } from './actions';
import { useSimpleData } from './hooks';
import { advanceBlock, STEPS, type SimpleStep } from './model';
import { StepMaterial } from './StepMaterial';
import { StepOpen } from './StepOpen';
import { StepPlace } from './StepPlace';
import { StepReview } from './StepReview';
import { StepStart } from './StepStart';
import { useSimple } from './store';
import './simple.css';

const STEP_UI: Record<SimpleStep, { icon: LucideIcon; label: I18nKey; title: I18nKey; lead: I18nKey }> = {
  open: { icon: FolderOpen, label: 'simple.step.open', title: 'simple.step.open.title', lead: 'simple.step.open.lead' },
  material: { icon: Layers, label: 'simple.step.material', title: 'simple.step.material.title', lead: 'simple.step.material.lead' },
  place: { icon: Move, label: 'simple.step.place', title: 'simple.step.place.title', lead: 'simple.step.place.lead' },
  review: { icon: Eye, label: 'simple.step.review', title: 'simple.step.review.title', lead: 'simple.step.review.lead' },
  start: { icon: Rocket, label: 'simple.step.start', title: 'simple.step.start.title', lead: 'simple.step.start.lead' },
};

/** Barra de pasos: se puede volver a cualquier paso ya recorrido; para avanzar está "Siguiente". */
function Stepper({ step }: { step: number }) {
  return (
    <ol className="simple__steps" aria-label={t('simple.steps')}>
      {STEPS.map((name, i) => {
        const Icon = STEP_UI[name].icon;
        const done = i < step;
        return (
          <li key={name} className="simple__step" data-state={i === step ? 'current' : done ? 'done' : 'todo'}>
            <button
              type="button"
              className="simple__step-btn"
              aria-current={i === step ? 'step' : undefined}
              disabled={i > step}
              onClick={() => goToStep(i)}
            >
              <span className="simple__step-num" aria-hidden="true">{done ? <Check size={14} /> : <Icon size={14} />}</span>
              <span className="simple__step-label">{i + 1}. {t(STEP_UI[name].label)}</span>
            </button>
          </li>
        );
      })}
    </ol>
  );
}

/**
 * El modo sencillo: un paso a la vez, con tarjetas grandes. Reemplaza al panel acoplado; el lienzo
 * sigue a la vista para colocar y revisar. Todo lo que hace se aplica a las capas y los objetos reales.
 */
export function SimplePanel() {
  // La lista de materiales necesita la biblioteca del usuario y la de fábrica ya leídas.
  useEffect(() => {
    void loadMaterials();
  }, []);
  const step = useSimple((s) => s.step);
  // Al cambiar de paso el foco pasa al título del paso: el lector de pantalla lo lee y el teclado sigue desde ahí.
  const title = useRef<HTMLHeadingElement>(null);
  const first = useRef(true);
  useEffect(() => {
    if (first.current) first.current = false;
    else title.current?.focus();
  }, [step]);
  const actions = useSimple((s) => s.actions);
  const doc = useStore((s) => s.doc);
  const previewReady = useStore((s) => s.previewStatus === 'ready' && !s.previewStale);
  const data = useSimpleData();
  const name = STEPS[step]!;
  const ui = STEP_UI[name];
  const block = advanceBlock(name, { doc, choice: data.choice, actions, problems: data.problems, previewReady });
  const last = step === STEPS.length - 1;
  return (
    <aside className="simple" aria-label={t('simple.title')}>
      <Stepper step={step} />
      <section className="simple__card" aria-labelledby="simple-title" data-step={name}>
        <h2 id="simple-title" ref={title} tabIndex={-1} className="simple__title">{t(ui.title)}</h2>
        <p className="simple__lead">{t(ui.lead)}</p>
        {name === 'open' && <StepOpen data={data} />}
        {name === 'material' && <StepMaterial data={data} />}
        {name === 'place' && <StepPlace data={data} />}
        {name === 'review' && <StepReview data={data} />}
        {name === 'start' && <StepStart data={data} />}
      </section>
      <footer className="simple__nav">
        <button type="button" className="btn simple__btn simple__btn--big" disabled={step === 0} onClick={prevStep}>
          <ArrowLeft size={18} aria-hidden="true" />
          {t('simple.back')}
        </button>
        {!last && (
          <button
            type="button"
            className="btn btn--accent simple__btn simple__btn--big"
            disabled={block !== null}
            title={block ? t(block) : undefined}
            onClick={nextStep}
          >
            {t('simple.next')}
            <ArrowRight size={18} aria-hidden="true" />
          </button>
        )}
      </footer>
      {!last && block && <p className="hint simple__block" role="status">{t(block)}</p>}
    </aside>
  );
}
