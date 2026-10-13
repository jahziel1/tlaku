import type { LaserApi } from '../api';
import type { CsvTable } from '../types';

type Lag = <T>(value: () => T) => Promise<T>;

/** CSV de ejemplo que devuelve el modo simulado para cualquier ruta que no termine en `vacio.csv`. */
const SAMPLE: CsvTable = {
  headers: ['Nombre', 'Cargo', 'Código'],
  rows: [
    ['Ana Torres', 'Jefa de taller', 'A-001'],
    ['Luis Pérez', 'Técnico', 'A-002'],
    ['Marta Ríos', 'Diseño', 'A-003'],
    ['Iván Soto', 'Ventas', 'A-004'],
    ['Eva Luna', 'Calidad', 'A-005'],
  ],
  delimiter: ',',
};

/** Métodos simulados del área `variables`: un CSV de ejemplo que respeta `required`. */
export function mockVariables(lag: Lag): Pick<LaserApi, 'readCsv'> {
  return {
    readCsv: ({ path, required }) =>
      lag(() => {
        if (/vacio\.csv$/i.test(path)) throw new Error('El CSV está vacío.');
        const table = structuredClone(SAMPLE);
        const missing = (required ?? []).filter((c) => {
          const n = c.trim();
          return !table.headers.some((h) => h.toLowerCase() === n.toLowerCase()) && !(/^\d+$/.test(n) && Number(n) >= 1 && Number(n) <= table.headers.length);
        });
        if (missing.length > 0) {
          throw new Error(`Al CSV le faltan columnas que usa el diseño: ${missing.join(', ')}. Columnas disponibles: ${table.headers.join(', ')}.`);
        }
        return table;
      }),
  };
}
