import { renderToString } from 'react-dom/server';
import { afterEach, describe, expect, it } from 'vitest';
import { setApi } from '@/api';
import { createMockApi } from '@/api/mock';
import { commands } from '@/commands';
import { setLanguage } from '@/i18n';
import { useStore } from '@/store';
import { EditConfirmBody, RestorePreviewBody } from './MachineSettingsConfirm';
import { diffBackup, parseRawSettings } from './MachineSettingsLogic';
import { MachineSettingsSection } from './MachineSettingsSection';
import { MachineSettingsTable } from './MachineSettingsTable';

const noop = () => {};
const settings = parseRawSettings(['$30=1000', '$32=1', '$130=400.000', '$300=mi-laser', '$99=5', 'ok']).settings;

const table = (over: Partial<Parameters<typeof MachineSettingsTable>[0]> = {}) =>
  renderToString(
    <MachineSettingsTable settings={settings} family="grbl" filter="" blocked={null} editing={null} onEditStart={noop} onEditChange={noop} onEditCancel={noop} onReview={noop} {...over} />,
  );

afterEach(() => setLanguage('es'));

describe('tabla de ajustes', () => {
  it('muestra número, nombre, descripción, valor y unidad de cada ajuste', () => {
    const h = table();
    for (const s of ['$30', 'S máxima (potencia 100 %)', '1000', '$130', 'Recorrido máximo del eje X', '400.000', 'mm', 'Editar $30']) expect(h).toContain(s);
  });

  it('marca como críticos los peligrosos (con texto, no solo color) y deja los demás sin marca', () => {
    const h = table();
    expect((h.match(/data-tone="critical"/g) ?? []).length).toBe(3);
    expect(h).toContain('Crítico');
    expect(h).toContain('data-critical=""');
  });

  it('un ajuste desconocido dice «Ajuste sin descripción»; uno de texto no se puede editar', () => {
    const h = table();
    expect(h).toContain('Ajuste sin descripción');
    expect(h).toMatch(/disabled=""[^>]*title="Este valor no es un número[^"]*"[^>]*aria-label="Editar \$300"/);
  });

  it('los de grblHAL llevan la etiqueta «Solo grblHAL»', () => {
    const h = table({ settings: parseRawSettings(['$33=5000']).settings, family: 'grblhal' });
    expect(h).toContain('Solo grblHAL');
  });

  it('bloqueado: los botones de editar están deshabilitados y el motivo está en el título', () => {
    const h = table({ blocked: 'reason.jobRunning' });
    expect(h).toContain('Hay un trabajo en curso.');
    expect(h).not.toMatch(/<button(?![^>]*disabled)[^>]*aria-label="Editar \$30"/);
  });

  it('el filtro busca por número y por nombre', () => {
    expect(table({ filter: '$130' })).not.toContain('$32');
    expect(table({ filter: 'modo láser' })).toContain('Modo láser');
    expect(table({ filter: 'modo láser' })).not.toContain('Recorrido');
    expect(table({ filter: 'zzzz' })).toContain('Ningún ajuste coincide');
  });

  it('en edición muestra el campo con el error de validación accesible', () => {
    const h = table({ editing: { id: 32, text: '5' } });
    expect(h).toContain('aria-label="Nuevo valor de $32"');
    expect(h).toContain('aria-invalid="true"');
    expect(h).toContain('Solo puede ser 0 o 1.');
    expect(table({ editing: { id: 32, text: '0' } })).not.toContain('aria-invalid');
  });

  it('también sale en inglés', () => {
    setLanguage('en');
    const h = table();
    expect(h).toContain('Maximum S (100 % power)');
    expect(h).toContain('Critical');
    expect(h).toContain('Edit $30');
  });
});

describe('confirmación de una escritura', () => {
  it('muestra la línea exacta, el valor anterior y la advertencia de la EEPROM', () => {
    const h = renderToString(<EditConfirmBody id={32} previous="1" line="$32=0" />);
    expect(h).toContain('$32=0');
    expect(h).toContain('Valor actual:');
    expect(h).toContain('EEPROM');
    expect(h).toContain('ajuste crítico');
  });

  it('un ajuste no crítico no lleva la advertencia reforzada', () => {
    expect(renderToString(<EditConfirmBody id={0} previous="10" line="$0=12" />)).not.toContain('ajuste crítico');
  });
});

describe('vista previa de la restauración', () => {
  const diff = diffBackup(settings, [{ id: 30, raw: '800' }, { id: 32, raw: '1' }, { id: 130, raw: '410' }, { id: 77, raw: '1' }, { id: 99, raw: 'x' }]);

  it('lista solo las diferencias con «ahora → respaldo» y casillas marcadas', () => {
    const h = renderToString(
      <RestorePreviewBody fileName="mi.tlakuset" backupFirmware="Grbl 1.1h" familyMismatch={false} diff={diff} selected={new Set([30, 130])} onToggle={noop} onToggleAll={noop} />,
    );
    expect(h).toContain('mi.tlakuset');
    expect(h).toContain('2 ajuste(s) cambiarían; 1 ya valen lo mismo');
    expect(h).toContain('1000');
    expect(h).toContain('<strong>800</strong>');
    expect(h).toContain('<strong>410</strong>');
    expect(h).not.toContain('Restaurar $32');
    expect(h).toContain('aria-label="Restaurar $30"');
    expect((h.match(/checked=""/g) ?? []).length).toBe(3); // las dos filas y la casilla de «todas»
    expect(h).toContain('$77');
    expect(h).toContain('Valores del respaldo que no son válidos');
    expect(h).toContain('Grbl 1.1h');
  });

  it('avisa si el respaldo es de otro firmware y si no hay diferencias', () => {
    const none = diffBackup(settings, [{ id: 30, raw: '1000' }]);
    const h = renderToString(
      <RestorePreviewBody fileName="a.txt" backupFirmware={null} familyMismatch diff={none} selected={new Set()} onToggle={noop} onToggleAll={noop} />,
    );
    expect(h).toContain('otra familia de firmware');
    expect(h).toContain('nada que restaurar');
    expect(h).not.toContain('<table');
  });
});

describe('botón del panel Máquina', () => {
  it('el comando existe y, sin conexión, queda deshabilitado con la razón visible', () => {
    setApi(createMockApi({ latencyMs: 0 }));
    expect(commands.get('machine.settings')).toBeDefined();
    expect(commands.availability('machine.settings', useStore.getState())).toBe(useStore.getState().connection.connected ? true : 'reason.needMachine');
    const h = renderToString(<MachineSettingsSection />);
    expect(h).toContain('Ajustes de la máquina');
    expect(h).toContain(useStore.getState().connection.connected ? 'ajustes ($n)' : 'Conecta la máquina');
  });
});

describe('respaldo automático y calibración de ejes de GRBL', () => {
  it('el respaldo automático guarda el texto con la fecha en el nombre y devuelve la ruta', async () => {
    const api = createMockApi({ latencyMs: 0 });
    setApi(api);
    const path = await api.grblSettingsAutobackup({ text: '$30=1000\n' });
    expect(path).toMatch(/grbl-\d{8}-\d{6}\.tlakuset$/);
    expect(await api.readSettingsBackup({ path })).toBe('$30=1000\n');
    api.dispose();
  });

  it('la calibración ofrece X, Y y Z solo si la controladora los informa', async () => {
    const { GrblAxisCalibration } = await import('./GrblAxisCalibration');
    const all = parseRawSettings(['$100=80.000', '$101=80.000', '$102=400.000', 'ok']).settings;
    const h = renderToString(<GrblAxisCalibration settings={all} disabled={false} onUse={noop} />).replace(/<!-- -->/g, '');
    expect(h).toContain('Calibrar ejes');
    for (const s of ['X ($100)', 'Y ($101)', 'Z ($102)']) expect(h).toContain(s);
    const none = renderToString(<GrblAxisCalibration settings={settings} disabled={false} onUse={noop} />);
    expect(none).toBe('');
  });
});
