import { describe, expect, it } from 'vitest';
import { currencyDigits, currencySymbol, defaultCurrency, formatMoney, formatTag, plainNumber, regionOf } from './currency';

describe('moneda por omisión', () => {
  it('es-MX da pesos mexicanos', () => {
    expect(defaultCurrency('es', 'es-MX')).toBe('MXN');
  });

  it('usa la región del sistema cuando la conoce', () => {
    expect(defaultCurrency('es', 'es-CO')).toBe('COP');
    expect(defaultCurrency('es', 'es-AR')).toBe('ARS');
    expect(defaultCurrency('es', 'es-ES')).toBe('EUR');
    expect(defaultCurrency('en', 'en-GB')).toBe('GBP');
    expect(defaultCurrency('en', 'en-CA')).toBe('CAD');
    expect(defaultCurrency('es', 'es_CL')).toBe('CLP');
  });

  it('sin región: pesos mexicanos en español y dólares en inglés', () => {
    expect(defaultCurrency('es', null)).toBe('MXN');
    expect(defaultCurrency('es', 'es')).toBe('MXN');
    expect(defaultCurrency('en', undefined)).toBe('USD');
    expect(defaultCurrency('en', 'en')).toBe('USD');
    expect(defaultCurrency('es', 'es-ZZ')).toBe('MXN');
  });

  it('regionOf lee la región o devuelve null', () => {
    expect(regionOf('es-MX')).toBe('MX');
    expect(regionOf('en_us')).toBe('US');
    expect(regionOf('es')).toBeNull();
    expect(regionOf(null)).toBeNull();
  });
});

describe('formato regional', () => {
  it('la etiqueta sigue al sistema solo si coincide con el idioma de la interfaz', () => {
    expect(formatTag('es', 'es-CO')).toBe('es-CO');
    expect(formatTag('es', 'en-US')).toBe('es-MX');
    expect(formatTag('en', 'en-GB')).toBe('en-GB');
    expect(formatTag('en', 'es-MX')).toBe('en-US');
    expect(formatTag('es', null)).toBe('es-MX');
  });

  it('pesos mexicanos con el formato de México', () => {
    expect(formatMoney(1234.5, 'MXN', 'es-MX')).toBe('$1,234.50');
  });

  it('dólares, euros y una moneda extranjera con su símbolo', () => {
    expect(formatMoney(1234.5, 'USD', 'en-US')).toBe('$1,234.50');
    expect(formatMoney(1234.5, 'EUR', 'es-ES').replace(/\s/g, ' ')).toMatch(/^1234,50 €$/);
    expect(formatMoney(1234.5, 'EUR', 'en-US')).toBe('€1,234.50');
    expect(formatMoney(12, 'USD', 'es-MX')).toMatch(/12\.00/);
  });

  it('sin decimales donde la moneda no los usa', () => {
    expect(currencyDigits('MXN')).toBe(2);
    expect(currencyDigits('CLP')).toBe(0);
    expect(currencyDigits('PYG')).toBe(0);
    expect(formatMoney(1500, 'CLP', 'es-CL').replace(/\s/g, '')).toMatch(/^\$1\.500$/);
  });

  it('una moneda desconocida no rompe nada', () => {
    expect(currencyDigits('???')).toBe(2);
    expect(formatMoney(5, '???', 'es-MX')).toBe('??? 5.00');
    expect(currencySymbol('???', 'es-MX')).toBe('???');
  });

  it('símbolo corto para etiquetar los campos', () => {
    expect(currencySymbol('MXN', 'es-MX')).toBe('$');
    expect(currencySymbol('EUR', 'es-ES')).toBe('€');
  });

  it('números para hojas de cálculo: punto decimal y sin separador de miles', () => {
    expect(plainNumber(1234.5, 2)).toBe('1234.50');
    expect(plainNumber(1234.5, 0)).toBe('1235');
    expect(plainNumber(Number.NaN, 2)).toBe('');
  });
});
