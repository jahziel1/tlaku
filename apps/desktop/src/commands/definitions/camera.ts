import { startCamera, stopCamera, setFrozen } from '@/features/camera/session';
import { updateCameraPrefs } from '@/features/camera/actions';
import { openHeadAlignDialog } from '@/features/camera/headAlignState';
import { openMarksDialog } from '@/features/camera/marksState';
import { captureBlockedReason, overlayBlockedReason, requestAction } from '@/features/camera/bedImage';
import { useCenter } from '@/features/camera/centerFinder';
import { useViewTool } from '@/features/camera/headState';
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
    enabled: (s) => (s.cameraSetup.alignment || (s.cameraSetup.head?.enabled && s.cameraSetup.head.calibration) ? true : 'camera.reason.noAlignment'),
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
  {
    id: 'camera.alignMarks',
    label: 'camera.cmd.alignMarks',
    category: 'camera',
    run: () => openMarksDialog(),
  },
  {
    id: 'camera.alignHead',
    label: 'camera.cmd.alignHead',
    category: 'camera',
    run: () => openHeadAlignDialog(),
  },
  {
    id: 'camera.capture',
    label: 'camera.cmd.capture',
    category: 'camera',
    enabled: (s) => captureBlockedReason(s) ?? true,
    run: () => requestAction('capture'),
  },
  {
    id: 'camera.vectorize',
    label: 'camera.cmd.vectorize',
    category: 'camera',
    enabled: (s) => captureBlockedReason(s) ?? true,
    run: () => requestAction('vectorize'),
  },
  {
    id: 'camera.updateOverlay',
    label: 'camera.cmd.updateOverlay',
    category: 'camera',
    enabled: (s) => overlayBlockedReason(s) ?? true,
    run: () => requestAction('overlay'),
  },
  {
    id: 'camera.centerFinder',
    label: 'camera.cmd.centerFinder',
    category: 'camera',
    run: (s) => {
      s.setDockTab('camera');
      useCenter.getState().reset();
      useViewTool.getState().setTool('center');
    },
  },
];
