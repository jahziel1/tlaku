import { invoke, type InvokeArgs } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';
import { COMMAND_NAMES, type LaserApi } from './api';

type CommandKey = keyof typeof COMMAND_NAMES;

/** Cada método de LaserApi es un `invoke` al comando homónimo de src-tauri (snake_case). */
function call<T>(name: CommandKey, args?: object): Promise<T> {
  return invoke<T>(COMMAND_NAMES[name], args as InvokeArgs | undefined);
}

/** Implementación real. Los comandos del motor se conectan en la oleada B; hoy solo `app_version` existe. */
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
    on: (event, handler) => listen(event, (e) => handler(e.payload as never)),
  };
}
