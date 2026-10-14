# Política de seguridad

[English](SECURITY.md) · Español

Tlaku controla máquinas que pueden quemar, cortar y provocar incendios. En este proyecto, **los fallos de
seguridad no son solo errores de software**: algunos pueden lastimar a personas o dañar bienes. Por favor
repórtalos en privado.

## Cómo reportar una vulnerabilidad (en privado)

Usa los **Reportes privados de vulnerabilidades** de GitHub:
<https://github.com/jahziel1/tlaku/security/advisories/new>

1. Abre el enlace (hace falta una cuenta de GitHub).
2. Describe qué pasa, cómo repetirlo y, si lo sabes, la máquina y la controladora.
3. **No** abras un issue, una discusión ni un pull request públicos por esto, ni lo publiques en foros
   hasta que esté corregido.

Incluye lo que puedas: versión de Tlaku, sistema, máquina, controladora (y firmware), los pasos, el
registro de la consola y, si es seguro compartirlos, el diseño o el G-code.

Recibirás respuesta en cuanto alguien del proyecto pueda leerlo. Es un proyecto de voluntarios, sin
empresa detrás, así que no podemos prometer plazos fijos, pero los reportes de seguridad láser se atienden
primero (ver abajo). Te mantendremos al tanto, corregiremos el problema, publicaremos un aviso y te daremos
crédito si quieres.

## Seguridad láser: se trata como urgente

Un fallo que pueda hacer cualquiera de lo siguiente se trata como **vulnerabilidad urgente**, aunque no
haga falta ningún atacante (por ejemplo, un error común que lo provoca por sí solo):

- **Encender el láser donde no debe**: disparar durante movimientos de traslado, al encuadrar con el
  láser que debería estar apagado, después de una pausa, un paro, una alarma o una desconexión, o con más
  potencia que la configurada.
- **No detenerlo**: una pausa, un paro o un paro de emergencia que no surte efecto, un paro que se da por
  hecho cuando no se confirmó, o un láser que queda encendido tras un error o al perderse la conexión.
- **Mover la máquina sin querer**: enviar movimiento sin las confirmaciones que Tlaku exige (revisión
  previa, diálogos de confirmación, el requisito de encuadrar), hacer homing o desbloquear sin que el
  usuario lo pida, coordenadas, sentidos o escala equivocados que sacan el cabezal del área de trabajo, o
  iniciar un trabajo desde el control del celular o el servidor de automatización sin las confirmaciones
  requeridas.

Si dudas de si algo cuenta, repórtalo en privado de todos modos.

Recuerda que Tlaku **nunca se ha probado con un láser real** (solo con emuladores), así que estos fallos
podrían existir. Si pruebas con hardware real, sigue los niveles de la
[lista de validación](docs/validacion-hardware.md), usa protección ocular y material de desecho, y ten la
mano en el paro de emergencia. Si durante una prueba pasa algo inesperado, primero detén la máquina y
después reporta.

## Otros temas de seguridad

También son bienvenidos como reportes privados:

- El **control desde el celular** (servidor web local) y el **servidor de automatización** (UDP local):
  saltarse la clave, alcanzarlos desde fuera de la red permitida o iniciar un trabajo sin confirmación.
- Abrir archivos no confiables (`.tlaku`, SVG, DXF, PDF, proyectos de LightBurn, G-code, `.tlakubundle`,
  `.tlakumat`) de forma que la app falle, ejecute código, o lea o escriba archivos que no debe.
- El **actualizador** y los instaladores (verificación de firmas, manipulación).
- Dependencias con vulnerabilidades conocidas que afecten a Tlaku.

Para errores comunes (sin impacto de seguridad), usa el
[formulario de errores](https://github.com/jahziel1/tlaku/issues/new?template=bug_report.yml).

## Versiones con soporte

Tlaku todavía no tiene una versión estable. Las correcciones van a la última versión de la rama `main` y
a la última versión publicada.
