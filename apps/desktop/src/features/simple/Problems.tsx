import { CircleAlert, FlaskConical, Grid2x2, Minimize2, Plug, TriangleAlert } from 'lucide-react';
import { t } from '@/i18n';
import { useMaterials } from '@/features/materials/state';
import { connectMachine, connectVirtual, fitToBed, splitInTiles } from './actions';
import type { Problem, SimpleStep } from './model';

/** Botones que arreglan (o ayudan con) un problema. */
function Fixes({ problem }: { problem: Problem }) {
  switch (problem.fix) {
    case 'connect':
      return (
        <div className="simple__row">
          <button type="button" className="btn simple__btn" onClick={() => void connectMachine()}>
            <Plug size={16} aria-hidden="true" />
            {t('simple.fix.connect')}
          </button>
          <button type="button" className="btn simple__btn" onClick={() => void connectVirtual()}>
            {t('simple.fix.virtual')}
          </button>
        </div>
      );
    case 'fit':
      return (
        <div className="simple__row">
          <button type="button" className="btn simple__btn" onClick={() => fitToBed()}>
            <Minimize2 size={16} aria-hidden="true" />
            {t('simple.fix.fit')}
          </button>
          <button type="button" className="btn simple__btn" onClick={splitInTiles}>
            <Grid2x2 size={16} aria-hidden="true" />
            {t('simple.fix.tiling')}
          </button>
        </div>
      );
    case 'test':
      return (
        <div className="simple__row">
          <button type="button" className="btn simple__btn" onClick={() => useMaterials.getState().openTest()}>
            <FlaskConical size={16} aria-hidden="true" />
            {t('simple.fix.test')}
          </button>
        </div>
      );
    default:
      return null;
  }
}

/** Avisos del paso, en lenguaje simple: los que impiden seguir en rojo y los demás como aviso. */
export function ProblemList({ step, problems }: { step: SimpleStep; problems: readonly Problem[] }) {
  const shown = problems.filter((p) => p.steps.includes(step));
  if (shown.length === 0) return null;
  return (
    <ul className="simple__problems" aria-label={t('simple.problems')}>
      {shown.map((p) => (
        <li key={p.id} className="simple__problem" data-level={p.level} data-problem={p.id}>
          <span className="simple__problem-icon" aria-hidden="true">
            {p.level === 'block' ? <CircleAlert size={18} /> : <TriangleAlert size={18} />}
          </span>
          <div className="simple__problem-body">
            <p>{t(p.key, p.params)}</p>
            <Fixes problem={p} />
          </div>
        </li>
      ))}
    </ul>
  );
}
