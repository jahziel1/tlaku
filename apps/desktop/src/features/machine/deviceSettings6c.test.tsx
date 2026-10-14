import { renderToString } from 'react-dom/server';
import { describe, expect, it, vi } from 'vitest';
import { getApi, setApi } from '@/api';
import { createMockApi } from '@/api/mock';
import { FALLBACK_DEVICE } from '@/lib/device';
import { DEVICE_ICONS, defaultIconId, deviceIcon } from './deviceIcons';
import { ExtraAxesFields } from './ExtraAxesFields';
import { fetchLinkWarning } from './useLinkWarning';
import { DeviceFrameFields, DeviceIdentityFields, DeviceLinkFields, DeviceOrderButtons, SpeedInput } from './DeviceSettings6C';

const noop = () => {};

describe('ajustes de perfil de la ola 6C', () => {
  it('el ícono elegido existe en el juego; sin elegir se usa el del tipo', () => {
    expect(DEVICE_ICONS.length).toBeGreaterThanOrEqual(8);
    expect(defaultIconId('galvo')).toBe('sun');
    expect(defaultIconId('ruida')).toBe('flame');
    expect(deviceIcon({ icon: '', kind: 'grbl' })).toBe(DEVICE_ICONS[0].Icon);
    // Un nombre desconocido (de otra versión) cae al del tipo en vez de romper la lista.
    expect(deviceIcon({ icon: 'no-existe', kind: 'grbl' })).toBe(DEVICE_ICONS[0].Icon);
    expect(deviceIcon({ icon: 'printer', kind: 'grbl' })).toBe(DEVICE_ICONS.find((i) => i.id === 'printer')!.Icon);
  });

  it('identidad: selector de ícono accesible y casilla de perfil predeterminado', () => {
    const h = renderToString(<DeviceIdentityFields draft={{ ...FALLBACK_DEVICE, icon: 'zap', is_default: true }} edit={noop} />);
    expect(h).toContain('role="radiogroup"');
    expect(h.match(/role="radio"/g)).toHaveLength(DEVICE_ICONS.length);
    expect(h).toContain('Activar este perfil al abrir Tlaku');
    expect(h).toMatch(/aria-checked="true"[^>]*aria-label="Rayo"|aria-label="Rayo"[^>]*aria-checked="true"/);
  });

  it('subir y bajar se deshabilitan en los extremos y con un perfil sin guardar', () => {
    const at = (index: number, count: number, disabled = false) => renderToString(<DeviceOrderButtons index={index} count={count} disabled={disabled} onMove={noop} />);
    const disabled = (h: string) => (h.match(/disabled=""/g) ?? []).length;
    expect(disabled(at(0, 3))).toBe(1);
    expect(disabled(at(1, 3))).toBe(0);
    expect(disabled(at(2, 3))).toBe(1);
    expect(disabled(at(1, 3, true))).toBe(2);
  });

  it('área: ejes cruzados, desfase del puntero y unidad de velocidad', () => {
    const h = renderToString(<DeviceFrameFields draft={{ ...FALLBACK_DEVICE, pointer_offset: [8, 3] }} edit={noop} />);
    for (const s of ['Intercambiar X e Y en la máquina', 'Desfase del puntero rojo', 'Puntero X', 'Puntero Y', 'Unidad de velocidad', 'mm/s']) expect(h).toContain(s);
    expect(h).toContain('value="8"');
  });

  it('la velocidad se escribe en la unidad del perfil', () => {
    const h = renderToString(<SpeedInput unit="mm_per_sec" value={6000} onCommit={noop} />);
    expect(h).toContain('value="100"');
    expect(h).toContain('mm/s');
    expect(renderToString(<SpeedInput unit="mm_per_min" value={6000} onCommit={noop} />)).toContain('value="6000"');
  });

  it('enlace: auto-home, DTR y envío síncrono solo en la familia GRBL; DTR solo por serie', () => {
    const grbl = renderToString(<DeviceLinkFields draft={FALLBACK_DEVICE} edit={noop} />);
    for (const s of ['Ofrecer el homing', 'Abrir el puerto serie sin subir DTR', 'Mandar una línea y esperar']) expect(grbl).toContain(s);
    const net = renderToString(<DeviceLinkFields draft={{ ...FALLBACK_DEVICE, connection: { type: 'tcp', host: '192.168.0.5', port: 23 } }} edit={noop} />);
    expect(net).not.toContain('sin subir DTR');
    expect(net).toContain('Mandar una línea y esperar');
    expect(renderToString(<DeviceLinkFields draft={{ ...FALLBACK_DEVICE, kind: 'marlin' }} edit={noop} />)).toBe('');
  });

  it('el desplazamiento U de los trabajos aparece solo con el eje U', () => {
    expect(renderToString(<ExtraAxesFields draft={FALLBACK_DEVICE} edit={noop} />)).not.toContain('Desplazamiento U');
    const h = renderToString(<ExtraAxesFields draft={{ ...FALLBACK_DEVICE, u_axis: true, u_offset: 12.5 }} edit={noop} />);
    expect(h).toContain('Desplazamiento U de los trabajos');
    expect(h).toContain('value="12.5"');
  });
});

describe('aviso de enlace en el diálogo de inicio', () => {
  it('manda el trabajo actual al motor y devuelve su aviso (o nada)', async () => {
    setApi(createMockApi({ latencyMs: 0 }));
    const spy = vi.spyOn(getApi(), 'jobLinkWarning').mockResolvedValueOnce('en su tramo más denso el trabajo pide 1200 líneas por segundo').mockResolvedValueOnce(null);
    expect(await fetchLinkWarning()).toContain('líneas por segundo');
    expect(await fetchLinkWarning()).toBeNull();
    expect(spy.mock.calls[0]![0]).toHaveProperty('device');
    expect(spy.mock.calls[0]![0]).toHaveProperty('options');
  });
});
