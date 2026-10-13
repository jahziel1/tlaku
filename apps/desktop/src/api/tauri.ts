import { invoke, type InvokeArgs } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';
import { COMMAND_NAMES, type LaserApi } from './api';

type CommandKey = keyof typeof COMMAND_NAMES;

/** Cada método de LaserApi es un `invoke` al comando homónimo de src-tauri (snake_case). */
function call<T>(name: CommandKey, args?: object): Promise<T> {
  return invoke<T>(COMMAND_NAMES[name], args as InvokeArgs | undefined);
}

/**
 * Implementación real: cada método es el `invoke` del comando homónimo de src-tauri y `on` es
 * `listen` de los eventos `machine://*`. Los errores de Rust llegan como `string` rechazado
 * (mensajes en español listos para mostrar).
 */
export function createTauriApi(): LaserApi {
  return {
    appVersion: () => call('appVersion'),
    importFile: (a) => call('importFile', a),
    saveProject: (a) => call('saveProject', a),
    loadProject: (a) => call('loadProject', a),
    computeJob: (a) => call('computeJob', a),
    exportGcode: (a) => call('exportGcode', a),
    listDevices: () => call('listDevices'),
    saveDevice: (a) => call('saveDevice', a),
    deleteDevice: (a) => call('deleteDevice', a),
    listPorts: () => call('listPorts'),
    connect: (a) => call('connect', a),
    disconnect: () => call('disconnect'),
    machineCommand: (a) => call('machineCommand', a),
    machineRealtime: (a) => call('machineRealtime', a),
    jog: (a) => call('jog', a),
    startJob: (a) => call('startJob', a),
    pauseJob: () => call('pauseJob'),
    resumeJob: () => call('resumeJob'),
    cancelJob: () => call('cancelJob'),
    frameJob: (a) => call('frameJob', a),
    listFonts: () => call('listFonts'),
    textOutline: (a) => call('textOutline', a),
    geometryOp: (a) => call('geometryOp', a),
    traceImage: (a) => call('traceImage', a),
    detectBoard: (a) => call('detectBoard', a),
    calibrateLens: (a) => call('calibrateLens', a),
    loadCameraSetup: () => call('loadCameraSetup'),
    saveCameraSetup: (a) => call('saveCameraSetup', a),
    saveBoardFile: (a) => call('saveBoardFile', a),
    on: (event, handler) => listen(event, (e) => handler(e.payload as never)),
  };
}
