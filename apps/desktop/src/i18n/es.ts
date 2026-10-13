/**
 * Diccionario en español. Todo texto visible de la interfaz sale de aquí a través de `t()`.
 * Claves por área: cmd.* (comandos), menu.*, reason.* (por qué algo está deshabilitado),
 * y una sección por panel. Los marcadores `{nombre}` se reemplazan con los parámetros de `t()`.
 */
import { canvas } from './es/canvas';
import { history } from './es/history';
import { layers } from './es/layers';
import { machine } from './es/machine';
import { preview } from './es/preview';

const base = {
  // ---- comandos -----------------------------------------------------------
  'cmd.file.new': 'Nuevo proyecto',
  'cmd.file.open': 'Abrir proyecto…',
  'cmd.file.save': 'Guardar',
  'cmd.file.saveAs': 'Guardar como…',
  'cmd.file.import': 'Importar diseño…',
  'cmd.file.exportGcode': 'Exportar G-code…',

  'cmd.edit.undo': 'Deshacer',
  'cmd.edit.redo': 'Rehacer',
  'cmd.edit.cut': 'Cortar',
  'cmd.edit.copy': 'Copiar',
  'cmd.edit.paste': 'Pegar',
  'cmd.edit.duplicate': 'Duplicar',
  'cmd.edit.delete': 'Eliminar',
  'cmd.edit.selectAll': 'Seleccionar todo',
  'cmd.edit.group': 'Agrupar',
  'cmd.edit.ungroup': 'Desagrupar',
  'cmd.edit.nudge.left': 'Mover a la izquierda',
  'cmd.edit.nudge.right': 'Mover a la derecha',
  'cmd.edit.nudge.up': 'Mover hacia arriba',
  'cmd.edit.nudge.down': 'Mover hacia abajo',

  'cmd.tool.select': 'Seleccionar',
  'cmd.tool.nodes': 'Editar nodos',
  'cmd.tool.line': 'Dibujar línea',
  'cmd.tool.rect': 'Rectángulo',
  'cmd.tool.ellipse': 'Elipse',
  'cmd.tool.text': 'Texto',
  'cmd.tool.pan': 'Mano (mover la vista)',
  'cmd.tool.measure': 'Medir',

  'cmd.view.zoomIn': 'Acercar',
  'cmd.view.zoomOut': 'Alejar',
  'cmd.view.fitBed': 'Ajustar a la cama',
  'cmd.view.fitContent': 'Ajustar al diseño',
  'cmd.view.wireframe': 'Solo contornos',
  'cmd.view.preview': 'Vista previa',
  'cmd.view.theme': 'Cambiar tema claro u oscuro',

  'cmd.machine.connect': 'Conectar',
  'cmd.machine.disconnect': 'Desconectar',
  'cmd.machine.frame': 'Encuadrar',
  'cmd.machine.start': 'Iniciar',
  'cmd.machine.pause': 'Pausar',
  'cmd.machine.resume': 'Reanudar',
  'cmd.machine.stop': 'Detener',

  'cmd.panel.layers': 'Mostrar capas',
  'cmd.panel.machine': 'Mostrar máquina',
  'cmd.panel.console': 'Mostrar consola',

  // ---- menús y barras -----------------------------------------------------
  'menu.label': 'Menú principal',
  'menu.file': 'Archivo',
  'menu.edit': 'Editar',
  'menu.tools': 'Herramientas',
  'menu.view': 'Ver',
  'menu.machine': 'Máquina',
  'toolbar.file': 'Archivo',
  'toolbar.edit': 'Edición',
  'toolbar.tools': 'Herramientas',
  'toolbar.job': 'Trabajo',
  'theme.toLight': 'Cambiar a tema claro',
  'theme.toDark': 'Cambiar a tema oscuro',
  'doc.untitled': 'Sin título',
  'doc.unsaved': 'Cambios sin guardar',

  // ---- por qué un comando no está disponible ------------------------------
  'reason.soon': 'Todavía no está disponible.',
  'reason.noSelection': 'Selecciona uno o más objetos.',
  'reason.locked': 'Los objetos seleccionados están bloqueados.',
  'reason.nothingToUndo': 'No hay nada que deshacer.',
  'reason.nothingToRedo': 'No hay nada que rehacer.',
  'reason.needMachine': 'Conecta la máquina en la pestaña Máquina.',
  'reason.nothingToSend': 'No hay nada que enviar: activa la salida de una capa que tenga objetos.',
  'reason.jobRunning': 'Hay un trabajo en curso.',
  'reason.noJob': 'No hay un trabajo en curso.',
  'reason.connecting': 'Conectando…',

  // ---- historial, avisos y diálogos ----------------------------------------
  'history.import': 'Importar diseño',
  'history.editLayer': 'Cambiar ajustes de capa',
  'history.addLayer': 'Crear capa',
  'history.changeLayer': 'Cambiar de capa',
  'history.move': 'Mover',
  'history.delete': 'Eliminar',
  'history.duplicate': 'Duplicar',
  'history.editObjects': 'Editar objetos',
  'history.transform': 'Transformar',
  'history.add': 'Agregar objetos',
  'history.reorderLayers': 'Reordenar capas',
  'notice.saved': 'Guardado en {path}',
  'notice.imported': 'Se importaron {n} objetos.',
  'notice.importedWithWarnings': 'Se importaron {n} objetos con {w} advertencias.',
  'notice.exported': 'G-code exportado: {lines} líneas, {time} estimados. {path}',
  'toast.label': 'Avisos',
  'toast.dismiss': 'Cerrar aviso',
  'dialog.discard.title': 'Cambios sin guardar',
  'dialog.discard.message': 'Hay cambios sin guardar. Si continúas, se perderán.',
  'dialog.discard.confirm': 'Descartar cambios',
  'dialog.discard.cancel': 'Cancelar',

  // ---- lienzo --------------------------------------------------------------
  'canvas.label': 'Lienzo de diseño con la cama de la máquina',
  'canvas.zoom': 'Zoom',
  'canvas.empty.title': 'La cama está vacía',
  'canvas.empty.text': 'Importa un SVG, un DXF o una imagen para empezar. Los colores del archivo se convierten en capas.',

  // ---- panel acoplado -------------------------------------------------------
  'dock.label': 'Paneles',
  'dock.resize': 'Cambiar el ancho del panel',
  'dock.layers': 'Capas',
  'dock.machine': 'Máquina',
  'dock.console': 'Consola',

  // ---- capas ----------------------------------------------------------------
  'layers.empty': 'Aún no hay capas. Importa un diseño o elige un color en la franja de abajo.',
  'layers.selectHint': 'Selecciona los objetos de esta capa (con Mayús se suman a la selección)',
  'layers.objects': '{n} objetos en esta capa',
  'layers.show': 'Mostrar la capa en el lienzo',
  'layers.hide': 'Ocultar la capa del lienzo',
  'layers.outputOn': 'Enviar esta capa a la máquina',
  'layers.outputOff': 'No enviar esta capa a la máquina',
  'layers.editor': 'Ajustes de la capa {name}',
  'layers.mode': 'Modo de la capa',
  'layers.mode.line': 'Línea',
  'layers.mode.fill': 'Relleno',
  'layers.mode.image': 'Imagen',
  'layers.speed': 'Velocidad',
  'layers.power': 'Potencia',
  'layers.passes': 'Pasadas',
  'layers.interval': 'Intervalo',
  'palette.label': 'Colores de capa',
  'palette.layer': 'Capa {n}',
  'palette.toolLayer': 'Capa de herramienta {n}',
  'palette.moveHint': 'Mover la selección a la capa {n}',
  'palette.pickHint': 'Dibujar en la capa {n}',

  // ---- máquina ----------------------------------------------------------------
  'machine.connection': 'Conexión',
  'machine.device': 'Máquina',
  'machine.port': 'Puerto',
  'machine.noPorts': 'No se encontraron puertos',
  'machine.noPort': 'Elige un puerto antes de conectar.',
  'machine.refreshPorts': 'Buscar puertos de nuevo',
  'machine.baud': 'Baudios',
  'machine.connecting': 'Conectando…',
  'machine.status': 'Estado',
  'machine.state.off': 'Sin conexión',
  'machine.state.idle': 'Lista',
  'machine.state.run': 'En marcha',
  'machine.state.hold': 'En pausa',
  'machine.state.jog': 'Moviendo',
  'machine.state.alarm': 'Alarma',
  'machine.state.door': 'Puerta abierta',
  'machine.state.check': 'Modo de prueba',
  'machine.state.home': 'Buscando origen',
  'machine.state.sleep': 'Reposo',
  'machine.jog': 'Mover cabezal',
  'machine.jog.up': 'Mover hacia arriba',
  'machine.jog.down': 'Mover hacia abajo',
  'machine.jog.left': 'Mover a la izquierda',
  'machine.jog.right': 'Mover a la derecha',
  'machine.jog.step': 'Distancia por paso',
  'machine.job': 'Trabajo',
  'machine.progress': 'Progreso del trabajo',

  // ---- consola ------------------------------------------------------------------
  'console.log': 'Registro de la máquina',
  'console.empty': 'Aquí aparecen los comandos enviados y las respuestas de la máquina.',
  'console.in': 'Respuesta',
  'console.out': 'Enviado',
  'console.input': 'Comando para la máquina',
  'console.placeholder': 'Escribe un comando y pulsa Intro',
  'console.clear': 'Borrar el registro',

  // ---- vista previa -----------------------------------------------------------
  'preview.title': 'Vista previa del trabajo',
  'preview.close': 'Cerrar la vista previa',
  'preview.computing': 'Calculando…',
  'preview.scrub': 'Posición en el recorrido',
  'preview.empty': 'Importa un diseño para ver el recorrido.',

  // ---- barra de estado -----------------------------------------------------------
  'status.cursor': 'Posición del cursor en milímetros',
  'status.zoom': 'Zoom (100 % = tamaño real en pantalla)',
  'status.selected': '{n} seleccionados',
  'status.nothingSelected': 'Nada seleccionado',
  'status.connected': '{name} conectada',
  'status.disconnected': 'Máquina sin conectar',
  'status.estimate': 'Tiempo estimado del trabajo',
  'status.computing': 'calculando',
} as const;

/** Diccionario completo: la base más un archivo por área en `es/`, para que cada área edite solo el suyo. */
export const es = { ...base, ...canvas, ...layers, ...machine, ...preview, ...history };
