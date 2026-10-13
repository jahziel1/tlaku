import { describe, expect, it } from 'vitest';
import { evalExpression, formatValue, resolveInput } from './expr';

describe('expresiones numéricas', () => {
  it('números simples, con coma o punto decimal', () => {
    expect(evalExpression('12')).toBe(12);
    expect(evalExpression('12.5')).toBe(12.5);
    expect(evalExpression('12,5')).toBe(12.5);
    expect(evalExpression('.5')).toBe(0.5);
    expect(evalExpression('-3')).toBe(-3);
    expect(evalExpression('1e3')).toBe(1000);
  });

  it('operaciones con precedencia y paréntesis', () => {
    expect(evalExpression('100/3')).toBeCloseTo(33.3333333, 6);
    expect(evalExpression('2+3*4')).toBe(14);
    expect(evalExpression('(10+2)*4+1')).toBe(49);
    expect(evalExpression('2^3^2')).toBe(512); // asociativa a la derecha
    expect(evalExpression('-(2+3)')).toBe(-5);
    expect(evalExpression('2*-3')).toBe(-6);
    expect(evalExpression('10 - 4 - 3')).toBe(3);
  });

  it('constantes y funciones', () => {
    expect(evalExpression('pi')).toBeCloseTo(Math.PI, 12);
    expect(evalExpression('2*pi')).toBeCloseTo(Math.PI * 2, 12);
    expect(evalExpression('sqrt(16)')).toBe(4);
    expect(evalExpression('abs(-7.5)')).toBe(7.5);
    expect(evalExpression('sin(0)')).toBe(0);
  });

  it('unidades de longitud se convierten a mm', () => {
    expect(evalExpression('5in')).toBeCloseTo(127, 9);
    expect(evalExpression('2.5cm')).toBe(25);
    expect(evalExpression('10mm')).toBe(10);
    expect(evalExpression('1m')).toBe(1000);
    expect(evalExpression('1in + 5')).toBeCloseTo(30.4, 9);
  });

  it('unidades de ángulo', () => {
    expect(evalExpression('90deg', { mode: 'angle' })).toBe(90);
    expect(evalExpression('pi rad', { mode: 'angle' })).toBeCloseTo(180, 9);
  });

  it('porcentaje del valor actual', () => {
    expect(evalExpression('50%', { current: 80 })).toBe(40);
    expect(evalExpression('150 %', { current: 20 })).toBe(30);
  });

  it('rechaza lo que no es una expresión válida', () => {
    for (const bad of ['', '  ', 'abc', '1+', '(1+2', '1/0', '2**3', '5 5', 'sqrt', 'sqrt 4', '1..2', '3 xx']) {
      expect(evalExpression(bad), bad).toBeNull();
    }
    expect(evalExpression('1e999')).toBeNull(); // infinito
  });

  it('no ejecuta código', () => {
    expect(evalExpression('process.exit()')).toBeNull();
    expect(evalExpression('alert(1)')).toBeNull();
  });
});

describe('valor final de un campo', () => {
  it('+n suma al valor actual', () => {
    expect(resolveInput('+5', 20)).toBe(25);
    expect(resolveInput('+5+3', 20)).toBe(28);
    expect(resolveInput('+2in', 10)).toBeCloseTo(60.8, 9);
  });

  it('-n es un valor negativo, no una resta', () => {
    expect(resolveInput('-5', 20)).toBe(-5);
  });

  it('*n y /n multiplican o dividen el actual', () => {
    expect(resolveInput('*2', 20)).toBe(40);
    expect(resolveInput('/4', 20)).toBe(5);
    expect(resolveInput('/0', 20)).toBeNull();
  });

  it('un valor suelto o una expresión es absoluto', () => {
    expect(resolveInput('100/3', 7)).toBeCloseTo(33.3333, 4);
    expect(resolveInput('12,5', 7)).toBe(12.5);
    expect(resolveInput('50%', 7)).toBe(3.5);
  });

  it('vacío o inválido da null', () => {
    expect(resolveInput('', 5)).toBeNull();
    expect(resolveInput('+', 5)).toBeNull();
    expect(resolveInput('abc', 5)).toBeNull();
  });

  it('formatValue redondea a 3 decimales sin ceros de más', () => {
    expect(formatValue(33.33333333)).toBe('33.333');
    expect(formatValue(10)).toBe('10');
    expect(formatValue(-0.0001)).toBe('0');
    expect(formatValue(1.5)).toBe('1.5');
  });
});

describe('expresiones en pulgadas', () => {
  it('los números sin sufijo son pulgadas y el resultado va en mm', () => {
    expect(resolveInput('1', 0, 'length', 'in')).toBeCloseTo(25.4, 9);
    expect(resolveInput('100/3', 0, 'length', 'in')).toBeCloseTo((100 / 3) * 25.4, 9);
    expect(resolveInput('(1+1)*2', 0, 'length', 'in')).toBeCloseTo(101.6, 9);
  });
  it('un sufijo manda sobre la unidad del campo', () => {
    expect(resolveInput('25.4mm', 0, 'length', 'in')).toBeCloseTo(25.4, 9);
    expect(resolveInput('2cm', 0, 'length', 'in')).toBeCloseTo(20, 9);
    expect(resolveInput('1in', 0, 'length', 'mm')).toBeCloseTo(25.4, 9);
  });
  it('+, * y % se aplican sobre el valor actual en mm', () => {
    expect(resolveInput('+0.5', 25.4, 'length', 'in')).toBeCloseTo(38.1, 9);
    expect(resolveInput('*2', 25.4, 'length', 'in')).toBeCloseTo(50.8, 9);
    expect(resolveInput('50%', 25.4, 'length', 'in')).toBeCloseTo(12.7, 9);
  });
  it('lee fracciones y números mixtos de pulgada', () => {
    expect(resolveInput('3/8"', 0, 'length', 'in')).toBeCloseTo(9.525, 9);
    expect(resolveInput('3/8', 0, 'length', 'in')).toBeCloseTo(9.525, 9);
    expect(resolveInput('1 1/2', 0, 'length', 'in')).toBeCloseTo(38.1, 9);
    expect(resolveInput('1 1/2 in', 0, 'length', 'mm')).toBeCloseTo(38.1, 9);
    // En mm una división sigue siendo una división.
    expect(resolveInput('3/8', 0, 'length', 'mm')).toBeCloseTo(0.375, 9);
  });
  it('los ángulos no cambian con la unidad', () => {
    expect(resolveInput('90', 0, 'angle', 'in')).toBe(90);
  });
});
