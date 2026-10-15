import { TriangleAlert } from 'lucide-react';
import { t } from '@/i18n';
import { priceBeforeRounding } from './costModel';
import { WARNING_KEYS } from './costReport';
import { formatMoney } from './currency';
import type { CostCalc } from './useCostCalc';

/** Fórmula del precio con los números de ahora, para que se vea de dónde sale. */
function priceExpression(calc: CostCalc, money: (v: number) => string): string | null {
  const { sales } = calc.inputs;
  const { cost, price } = calc.result;
  const raw = priceBeforeRounding(cost, sales);
  if (raw === null || price === null) return null;
  const pct = (v: number) => new Intl.NumberFormat(calc.tag, { maximumFractionDigits: 2 }).format(v / 100);
  const c = pct(sales.commission);
  const x = pct(sales.tax);
  const m = pct(sales.margin);
  const body =
    sales.marginBase === 'price'
      ? `${money(cost)} ÷ (1 − ${c} − ${x} − ${m})`
      : `${money(cost)} × (1 + ${m}) ÷ (1 − ${c} − ${x})`;
  return `${body} = ${new Intl.NumberFormat(calc.tag, { minimumFractionDigits: calc.digits, maximumFractionDigits: calc.digits + 2 }).format(raw)} → ${money(price)}`;
}

/** Precio sugerido, desglose del costo y del precio, avisos y la fórmula. */
export function CostResultPanel({ calc }: { calc: CostCalc }) {
  const { result, inputs, tag } = calc;
  const code = inputs.sales.currency;
  const money = (v: number) => formatMoney(v, code, tag);
  const num = (v: number, d = 1) => new Intl.NumberFormat(tag, { minimumFractionDigits: d, maximumFractionDigits: d }).format(v);
  const expr = priceExpression(calc, money);
  const lines = result.lines.filter((l) => l.perPiece !== 0 || l.batch !== 0);

  return (
    <section className="cost__result" aria-labelledby="cost-result-title">
      <h3 id="cost-result-title" className="mc-sub">{t('costs.result.title')}</h3>
      <div className="cost__price" aria-live="polite" aria-atomic="true">
        <span className="cost__price-label">{t('costs.result.price')}</span>
        <span className="cost__price-value num" data-testid="cost-price">{result.price === null ? t('costs.result.noPrice') : money(result.price)}</span>
        {result.batchPrice !== null && <span className="hint">{t('costs.result.batchLine', { n: inputs.quantity, price: money(result.batchPrice) })}</span>}
      </div>
      <p className="cost__cost">
        <span>{t('costs.result.cost')}</span>
        <span className="num" data-testid="cost-cost">{money(result.cost)}</span>
      </p>

      <table className="cost__table" aria-label={t('costs.result.table')}>
        <thead>
          <tr>
            <th scope="col">{t('costs.result.col.item')}</th>
            <th scope="col" className="num">{t('costs.result.col.piece')}</th>
            <th scope="col" className="num">{t('costs.result.col.batch')}</th>
          </tr>
        </thead>
        <tbody>
          {lines.map((l) => (
            <tr key={l.id}>
              <th scope="row">{t(`costs.line.${l.id}`)}</th>
              <td className="num">{money(l.perPiece)}</td>
              <td className="num">{money(l.batch)}</td>
            </tr>
          ))}
        </tbody>
        <tfoot>
          <tr>
            <th scope="row">{t('costs.result.cost')}</th>
            <td className="num">{money(result.cost)}</td>
            <td className="num">{money(result.batchCost)}</td>
          </tr>
        </tfoot>
      </table>

      {result.price !== null && (
        <dl className="cost__split">
          <div>
            <dt>{t('costs.result.commission')}</dt>
            <dd className="num">{money(result.commission)}</dd>
          </div>
          <div>
            <dt>{t('costs.result.tax')}</dt>
            <dd className="num">{money(result.tax)}</dd>
          </div>
          <div>
            <dt>{t('costs.result.profit')}</dt>
            <dd className="num">{money(result.profit)}</dd>
          </div>
        </dl>
      )}
      {result.profitOnCostPct !== null && result.profitOnPricePct !== null && (
        <p className="hint">{t('costs.result.profitPct', { onCost: num(result.profitOnCostPct), onPrice: num(result.profitOnPricePct) })}</p>
      )}
      {result.sheets !== null && <p className="hint">{t('costs.result.sheets', { n: result.sheets })}</p>}

      {result.warnings.length > 0 && (
        <ul className="cost__warns" aria-label={t('costs.report.section.notes')}>
          {result.warnings.map((w) => (
            <li key={w} className="warn-line">
              <TriangleAlert size={14} aria-hidden="true" />
              <span>{t(WARNING_KEYS[w])}</span>
            </li>
          ))}
        </ul>
      )}

      <details className="cost__formula">
        <summary>{t('costs.formula.title')}</summary>
        <p>{t('costs.formula.cost')}</p>
        <p>{inputs.sales.marginBase === 'price' ? t('costs.formula.priceSale') : t('costs.formula.priceCost')}</p>
        <p className="hint">{t('costs.formula.why')}</p>
        {expr && <p className="num">{t('costs.formula.values', { expr })}</p>}
      </details>
    </section>
  );
}
