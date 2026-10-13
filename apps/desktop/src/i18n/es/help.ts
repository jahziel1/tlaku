/** Textos de la ayuda dentro de la app: manual, novedades, reportes, donaciones y Acerca de. */
export const help = {
  'help.cmd.manual': 'Manual de usuario',
  'help.cmd.news': 'Novedades',
  'help.cmd.report': 'Reportar un problema…',
  'help.cmd.donate': 'Donar al proyecto…',
  'help.cmd.about': 'Acerca de Tlaku',

  'help.close': 'Cerrar',
  'help.copy': 'Copiar',
  'help.copied': 'Copiado',
  'help.copyLink': 'Copiar el enlace',
  'help.linkPending': 'Enlace pendiente de publicar',
  'help.linkNote': 'La app no abre enlaces por su cuenta: copia la dirección y pégala en tu navegador.',

  'help.manual.title': 'Manual de usuario',
  'help.manual.search': 'Buscar en el manual',
  'help.manual.toc': 'Contenido del manual',
  'help.manual.none': 'Ninguna página menciona «{q}».',
  'help.manual.page': 'Página del manual',
  'help.manual.prev': 'Anterior: {title}',
  'help.manual.next': 'Siguiente: {title}',
  'help.manual.missing': 'Esta página no está en el manual.',
  'help.manual.imageMissing': 'Captura no disponible: {alt}',

  'help.news.title': 'Novedades',

  'help.about.title': 'Acerca de Tlaku',
  'help.about.tagline': 'Programa libre para cortadoras y grabadoras láser.',
  'help.about.version': 'Versión',
  'help.about.versionUnknown': 'No disponible',
  'help.about.license': 'Licencia',
  'help.about.licenseText':
    'MIT. Puedes usar, copiar, modificar y repartir Tlaku, incluso con fines comerciales, conservando el aviso de copyright. El programa se ofrece sin garantía: revisa siempre tu trabajo antes de encender un láser.',
  'help.about.name': 'Qué significa el nombre',
  'help.about.nameText':
    'Tlaku viene del náhuatl «tlacuilo»: el pintor-escriba que dibujaba y escribía los códices. Un tlacuilo traducía ideas a trazos sobre una superficie, igual que este programa traduce un diseño a movimientos de un láser.',
  'help.about.donate': 'Tlaku es software libre y se sostiene con donaciones.',
  'help.about.credits': 'Créditos y código de terceros',
  'help.about.creditsHint': 'Tlaku usa y adapta código abierto con licencia compatible. Esto es lo que se registra en THIRD_PARTY.md:',
  'help.about.honest':
    'Estado: la app todavía no se ha probado con una máquina láser real. Lo validado hasta hoy es contra emuladores y máquinas virtuales.',

  'help.report.title': 'Reportar un problema',
  'help.report.intro':
    'Un buen reporte permite arreglar el error más rápido. Cuéntanos qué hacías, qué esperabas y qué pasó en realidad.',
  'help.report.include': 'Incluye, si puedes:',
  'help.report.item1': 'Los pasos para repetir el problema, uno por uno.',
  'help.report.item2': 'Tu máquina y su controladora (por ejemplo «Falcon2 22 W, GRBL 1.1»).',
  'help.report.item3': 'El archivo .tlaku o el diseño que lo provoca, si puedes compartirlo.',
  'help.report.item4': 'Lo que muestra la pestaña Consola y los avisos de la vista previa.',
  'help.report.warning':
    'Si el problema ocurrió con una máquina real, no repitas el trabajo hasta entenderlo. Prueba primero con una máquina virtual (Máquina, Puerto: Máquina virtual).',
  'help.report.where': 'Dónde reportarlo',
  'help.report.info': 'Información de tu instalación',
  'help.report.copyInfo': 'Copiar la información',

  'help.donate.title': 'Donar al proyecto',
  'help.donate.text':
    'Tlaku es de código abierto (MIT) y no tiene empresa detrás: se sostiene con donaciones. Con ellas se pagan las pruebas con máquinas reales, los certificados de firma de los instaladores y el tiempo para seguir desarrollándolo. Es voluntario: el programa es el mismo con o sin donación.',
  'help.donate.where': 'Dónde donar',
} as const;
