import { beforeEach, describe, expect, it } from 'vitest';
import { setApi } from '@/api';
import { createMockApi } from '@/api/mock';
import { setLanguage } from '@/i18n';
import { FALLBACK_DEVICE } from '@/lib/device';
import { LayerEditor } from '@/features/layers/LayerEditor';
import { makeLayer } from '@/lib/layers';
import { useStore } from '@/store';
import { ConnectionSection } from './ConnectionSection';
import { ConsolePanel } from './ConsolePanel';
import { FrameOptions } from './FrameOptions';
import { GalvoStopNotice } from './GalvoNotice';
import { MachinePanel } from './MachinePanel';
import { MachineSettingsSection } from './MachineSettingsSection';
import { ExtraAxesControls, FireControls } from './MoveWindowExtras';
import { OverridesSection } from './OverridesSection';
import { StatusSection } from './StatusSection';
import { VirtualSection } from './VirtualSection';
import { galvoDevice, ssr } from './galvoFixtures';

const CONNECTED = { connected: true, port: 'virtual:galvo', error: null };

function useGalvo(connected = false) {
  useStore.setState({
    devices: [galvoDevice()],
    activeDeviceId: 'galvo-1',
    connection: connected ? CONNECTED : { connected: false, port: null, error: null },
    ports: ['usb:galvo:0', 'virtual:galvo', '/dev/ttyUSB0'],
    selectedPort: 'virtual:galvo',
    galvoLaserWarning: null,
  });
}

beforeEach(() => {
  setLanguage('es');
  setApi(createMockApi({ latencyMs: 0 }));
  useStore.setState({ devices: [FALLBACK_DEVICE], activeDeviceId: FALLBACK_DEVICE.id, galvoLaserWarning: null, connection: { connected: false, port: null, error: null } });
});

describe('el panel Máquina con un galvo oculta lo que no aplica y dice por qué', () => {
  it('overrides, ajustes «$», disparo de prueba y ejes extra', () => {
    useGalvo(true);
    expect(ssr(<OverridesSection />)).toContain('no tiene ajustes en vivo de velocidad ni de potencia');
    expect(ssr(<OverridesSection />)).not.toContain('Velocidad (feed)');
    expect(ssr(<MachineSettingsSection />)).toContain('no tiene ajustes «$» de GRBL');
    expect(ssr(<FireControls />)).toContain('no existe en el galvo');
    expect(ssr(<FireControls />)).not.toContain('Disparo de prueba…');
    expect(ssr(<ExtraAxesControls />)).toContain('no tiene ejes Z ni extra');
  });

  it('la consola no deja escribir G-code', () => {
    useGalvo(true);
    const h = ssr(<ConsolePanel />);
    expect(h).toContain('no usa G-code');
    expect(h).toMatch(/<input[^>]*id="console-input"[^>]*disabled=""/);
  });

  it('el encuadre solo con el puntero rojo y sin repetir desde la interfaz', () => {
    useGalvo(true);
    const h = ssr(<FrameOptions />);
    expect(h).toContain('solo con el puntero rojo');
    expect(h).not.toContain('Repetir el encuadre');
    expect(h).toMatch(/<input type="checkbox"[^>]*disabled=""/);
  });

  it('el estado no muestra F ni S, y no hay panel de fallas simuladas', () => {
    useGalvo(true);
    expect(ssr(<StatusSection />)).not.toContain('mc-strip__kv');
    expect(ssr(<VirtualSection />)).toBe('');
  });

  it('la conexión por la tarjeta USB avisa del diálogo de seguridad; la virtual, que es simulada', () => {
    useGalvo(false);
    useStore.setState({ selectedPort: 'usb:galvo:0' });
    const usb = ssr(<ConnectionSection />);
    expect(usb).toContain('Galvo · tarjeta USB 1');
    expect(usb).toContain('confirmar la seguridad láser');
    expect(usb).toContain('Experimental');
    // Sin baudios ni velocidad virtual.
    expect(usb).not.toContain('Baudios');
    useStore.setState({ selectedPort: 'virtual:galvo' });
    const virtual = ssr(<ConnectionSection />);
    expect(virtual).toContain('Tarjeta simulada');
    expect(virtual).not.toContain('/dev/ttyUSB0');
  });

  it('el panel completo trae el del galvo, el aviso de energía y no el del K40 ni el de Ruida', () => {
    useGalvo(true);
    const h = ssr(<MachinePanel />);
    expect(h).toContain('data-section="machine.galvo"');
    expect(h).not.toContain('data-section="machine.lhy"');
    expect(h).toContain('Experimental');
    // Sin aviso pendiente no se muestra el de cortar la energía.
    expect(h).not.toContain('Ya corté la energía');
    useStore.setState({ galvoLaserWarning: 'NO hubo confirmación de que la fuente se apagara' });
    const warned = ssr(<MachinePanel />);
    expect(warned).toContain('La fuente pudo quedar ENCENDIDA');
    expect(warned).toContain('Ya corté la energía');
    expect(ssr(<GalvoStopNotice />)).toContain('CORTA LA ENERGÍA');
  });

  it('con un perfil GRBL el panel de siempre', () => {
    const h = ssr(<MachinePanel />);
    expect(h).not.toContain('data-section="machine.galvo"');
    expect(h).toContain('data-section="machine.overrides"');
  });
});

describe('editor de capa completo', () => {
  it('con un galvo activo trae la sección Galvo y la velocidad en mm/s; con GRBL, no', () => {
    const layer = makeLayer('#ff0000', 'Marca', { speed: 6000 });
    expect(ssr(<LayerEditor layer={layer} />)).not.toContain('Repeticiones en la tarjeta');
    useGalvo();
    const h = ssr(<LayerEditor layer={layer} />);
    expect(h).toContain('Repeticiones en la tarjeta');
    expect(h).toContain('value="100"');
    expect(h).toContain('<span class="lyr-field__unit">mm/s</span>');
  });
});
