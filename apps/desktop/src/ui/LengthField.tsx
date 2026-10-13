import { useCallback, type ComponentProps } from 'react';
import { usePref } from '@/store';
import { fieldText, parseLength, stepInMm, unitSymbol } from '@/lib/units';
import { NumberField } from './NumberField';

type Props = Omit<ComponentProps<typeof NumberField>, 'unit' | 'parse' | 'format'>;

/**
 * Campo de longitud: `value`, `min`, `max` y `step` van siempre en mm (el documento y el motor
 * trabajan en mm); se muestra y se lee en la unidad de las preferencias (mm o pulgadas). Al
 * escribir acepta sufijos (`5 mm`, `0,25 in`, `1/8"`) y fracciones de pulgada, vía `parseLength`.
 */
export function LengthField({ step = 1, ...rest }: Props) {
  const units = usePref('units');
  const parse = useCallback((text: string) => parseLength(text, units), [units]);
  const format = useCallback((mm: number) => fieldText(mm, units), [units]);
  return <NumberField {...rest} step={stepInMm(step, units)} unit={unitSymbol(units)} parse={parse} format={format} />;
}
