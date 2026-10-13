import { startCamera, stopCamera, setFrozen } from '@/features/camera/session';
import { updateCameraPrefs } from '@/features/camera/actions';
import type { CommandDef } from '../types';

/** Comandos de cámara: aparecen solos en el menú Máquina (categoría `camera`). */
export const cameraCommands: CommandDef[] = [
  {
    id: 'camera.toggle',
    label: 'camera.cmd.start',
    labelFor: (s) => (s.cameraStatus === 'off' ? 'camera.cmd.start' : 'camera.cmd.stop'),
    category: 'camera',
    run: (s) => {
      if (s.cameraStatus === 'off') void startCamera();
      else stopCamera();
    },
  },
  {
    id: 'camera.backdrop',
    label: 'camera.cmd.backdrop',
    category: 'camera',
    enabled: (s) => (s.cameraSetup.alignment ? true : 'camera.reason.noAlignment'),
    checked: (s) => s.cameraSetup.prefs.visible,
    run: (s) => updateCameraPrefs({ visible: !s.cameraSetup.prefs.visible }),
  },
  {
    id: 'camera.freeze',
    label: 'camera.cmd.freeze',
    category: 'camera',
    enabled: (s) => (s.cameraStatus === 'live' || s.cameraFrozen ? true : 'camera.reason.notLive'),
    checked: (s) => s.cameraFrozen,
    run: (s) => setFrozen(!s.cameraFrozen),
  },
  {
    id: 'camera.calibrate',
    label: 'camera.cmd.calibrate',
    category: 'camera',
    run: (s) => s.setCameraCalibrationOpen(true),
  },
];
