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
    probePort: (a) => call('probePort', a),
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
    frameHull: (a) => call('frameHull', a),
    jogTo: (a) => call('jogTo', a),
    setOverride: (a) => call('setOverride', a),
    simulateFault: (a) => call('simulateFault', a),
    listFonts: () => call('listFonts'),
    textOutline: (a) => call('textOutline', a),
    importShxFonts: (a) => call('importShxFonts', a),
    readCsv: (a) => call('readCsv', a),
    generateCode: (a) => call('generateCode', a),
    geometryOp: (a) => call('geometryOp', a),
    shapeOp: (a) => call('shapeOp', a),
    warpObjects: (a) => call('warpObjects', a),
    hullOutline: (a) => call('hullOutline', a),
    tangentCircles: (a) => call('tangentCircles', a),
    traceImage: (a) => call('traceImage', a),
    vectorsToBitmap: (a) => call('vectorsToBitmap', a),
    saveProcessedImage: (a) => call('saveProcessedImage', a),
    artImportSvg: (a) => call('artImportSvg', a),
    artListUser: () => call('artListUser'),
    artAddUser: (a) => call('artAddUser', a),
    artRemoveUser: (a) => call('artRemoveUser', a),
    nestStart: (a) => call('nestStart', a),
    nestPoll: (a) => call('nestPoll', a),
    nestCancel: (a) => call('nestCancel', a),
    detectBoard: (a) => call('detectBoard', a),
    calibrateLens: (a) => call('calibrateLens', a),
    loadCameraSetup: () => call('loadCameraSetup'),
    saveCameraSetup: (a) => call('saveCameraSetup', a),
    saveBoardFile: (a) => call('saveBoardFile', a),
    cameraMarks: (a) => call('cameraMarks', a),
    detectCameraMarks: (a) => call('detectCameraMarks', a),
    loadMaterialLibrary: () => call('loadMaterialLibrary'),
    saveMaterialLibrary: (a) => call('saveMaterialLibrary', a),
    importMaterialLibrary: (a) => call('importMaterialLibrary', a),
    exportMaterialLibrary: (a) => call('exportMaterialLibrary', a),
    materialTest: (a) => call('materialTest', a),
    deviceDefaults: (a) => call('deviceDefaults', a),
    rotaryTest: (a) => call('rotaryTest', a),
    importFileWith: (a) => call('importFileWith', a),
    exportDesign: (a) => call('exportDesign', a),
    printDesign: (a) => call('printDesign', a),
    loadPreferences: () => call('loadPreferences'),
    savePreferences: (a) => call('savePreferences', a),
    exportPreferences: (a) => call('exportPreferences', a),
    importPreferences: (a) => call('importPreferences', a),
    pathsExist: (a) => call('pathsExist', a),
    autosaveWrite: (a) => call('autosaveWrite', a),
    autosaveList: () => call('autosaveList'),
    autosaveRead: (a) => call('autosaveRead', a),
    autosaveDiscard: (a) => call('autosaveDiscard', a),
    listTemplates: () => call('listTemplates'),
    saveTemplate: (a) => call('saveTemplate', a),
    loadTemplate: (a) => call('loadTemplate', a),
    deleteTemplate: (a) => call('deleteTemplate', a),
    newWindow: () => call('newWindow'),
    on: (event, handler) => listen(event, (e) => handler(e.payload as never)),
  };
}
