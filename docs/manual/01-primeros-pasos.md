# Primeros pasos

En esta página vas de cero a un trabajo listo: instalas Tlaku, importas un diseño, revisas sus capas, miras la vista previa, lo pruebas con una máquina virtual y guardas el proyecto. No necesitas un láser para seguirla.

> **Ojo:** nada de Tlaku se ha probado todavía con una máquina real. Lo que ves aquí funciona contra el emulador y las máquinas virtuales de la app. Cuando conectes un láser de verdad, hazlo con supervisión y con potencias bajas.

## 1. Instalar

Los instaladores se publican en la página **Releases** del repositorio:

| Sistema | Archivo |
|---|---|
| macOS 11 o posterior | `.dmg`: ábrelo y arrastra Tlaku a Aplicaciones |
| Windows 10 o posterior | `.exe` (instalador NSIS) o `.msi` |
| Linux | `.deb` o `.AppImage` |

La aplicación todavía no está firmada, así que el sistema avisará la primera vez. En macOS, clic derecho sobre Tlaku y **Abrir**; en Windows, **Más información** y **Ejecutar de todas formas**. En Linux, para usar el puerto serie añade tu usuario al grupo `dialout` (ver [Solución de problemas](15-solucion-de-problemas.md)).

## 2. Abrir y reconocer la ventana

Al abrir Tlaku verás la cama de la máquina vacía, con el mensaje **La cama está vacía**. Arriba están los menús y los botones, a la izquierda las herramientas, a la derecha el panel con pestañas y abajo la franja de colores de capa. El detalle está en [La interfaz](02-interfaz.md).

![Ventana principal de Tlaku con un diseño importado, el panel Capas a la derecha y la franja de colores abajo](../../apps/desktop/docs/screenshots/01-principal-oscuro.png)

## 3. Importar un diseño

1. Elige **Archivo > Importar diseño…** (`Ctrl/Cmd+I`), o arrastra el archivo sobre la ventana.
2. Formatos que acepta: SVG, DXF, PDF, AI, HPGL (`.plt`), G-code, y las imágenes PNG, JPG, BMP, WebP, TIFF, GIF y TGA.
3. Tlaku crea **una capa por cada color** del archivo. Una imagen va a su propia capa gris, en modo **Imagen**.
4. Si el importador avisa de algo (formas que no pudo leer, por ejemplo), aparece un aviso persistente: léelo y comprueba que el diseño esté completo.

Si necesitas elegir la escala, el DPI, la unidad de un DXF o la página de un PDF, usa **Archivo > Importar con opciones…**. Todos los formatos y sus opciones están en [Importar y exportar](03b-importar-y-exportar.md). Los proyectos de LightBurn (`.lbrn`, `.lbrn2`) también se importan: ver [Venir desde LightBurn](14-desde-lightburn.md). Más sobre imágenes en [Imágenes](06-imagenes.md).

## 4. Revisar las capas

Abre la pestaña **Capas** (`Ctrl/Cmd+1`). Cada fila es una operación: color, modo, velocidad, potencia y su etiqueta (**Grabado**, **Línea** o **Corte**).

- Clic en una fila: la vuelve la capa activa. `Intro` o doble clic abre sus ajustes.
- El ojo oculta la capa solo en el lienzo; el rayo decide si se **envía a la máquina**.
- Una capa nueva nace con valores prudentes: 1000 mm/min, 20 % de potencia y 1 pasada. No son valores para ningún material: ajústalos tú.

Todos los parámetros están en [Capas y ajustes de corte](05-capas-y-ajustes.md).

## 5. Colocar el diseño

Selecciona con la herramienta **Seleccionar** (`V`) y mueve con el ratón, con las flechas (1 mm; `Mayús` 10 mm; `Alt` 0,1 mm) o escribe valores exactos en la barra numérica de arriba (X, Y, Ancho, Alto, Rotación). Comprueba que todo quede dentro de la cama: si algo se sale, la vista previa lo dice.

## 6. Ver la vista previa

Pulsa **Vista previa** (o `P`). Tlaku calcula el recorrido, lo dibuja sobre el lienzo y muestra el tiempo estimado. Reproduce el recorrido con el botón de reproducir o con `K`. Detalles en [Vista previa y orden de corte](07-vista-previa.md).

## 7. Probar con la máquina virtual

Así recorres el flujo completo sin hardware:

1. Abre la pestaña **Máquina** (`Ctrl/Cmd+2`).
2. En **Puerto** elige **Máquina virtual · Falcon2 Pro** (las virtuales aparecen al final de la lista).
3. Pulsa **Conectar**. Verás la etiqueta **Virtual** en el panel y en la barra de estado.
4. Pulsa **Encuadrar** (`F`) y luego **Iniciar** (`Ctrl/Cmd+Intro`). Iniciar nunca envía directo: antes aparece **¿Láser listo y área despejada?** con una lista de comprobaciones; solo **Iniciar trabajo** lo envía.

![Panel Máquina conectado a una máquina virtual, con la opción de simular fallas](../../apps/desktop/docs/screenshots/25-maquina-virtual.png)

Nada se graba: el láser es simulado. Para conectar una máquina real, ver [Conectar tu máquina](08-conectar-maquina.md) y [Enviar trabajos con seguridad](09-enviar-con-seguridad.md).

## 8. Guardar

- **Archivo > Guardar** (`Ctrl/Cmd+S`) guarda el proyecto como `.tlaku`. **Guardar como…** es `Ctrl/Cmd+Shift+S`.
- **Archivo > Exportar G-code…** (`Ctrl/Cmd+Shift+E`) escribe el trabajo como G-code, por si quieres enviarlo con otro programa. Necesita al menos una capa con salida y objetos.
- Para llevar tu **diseño** a otro programa, **Archivo > Exportar SVG…**, **DXF…**, **imagen PNG…** o **PDF…** (ver [Importar y exportar](03b-importar-y-exportar.md)).
- Si cierras con cambios, Tlaku pregunta si quieres guardar.
- **Autoguardado:** mientras trabajas, Tlaku guarda una copia de recuperación cada minuto (si hay cambios). Si la aplicación se cierra de golpe, al abrirla te ofrece **recuperar** tu trabajo. No sustituye a guardar: ver [Preferencias, atajos y autoguardado](02b-preferencias-y-atajos.md).

Siguiente: [Modo sencillo](01b-modo-sencillo.md)
