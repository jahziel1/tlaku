# Control desde el celular

Con el control remoto puedes **ver el avance de un trabajo y pausarlo o detenerlo desde el celular**, sin instalar nada: el celular abre una página en su navegador. Sirve, por ejemplo, para vigilar un trabajo largo desde el otro lado del taller, o para detenerlo sin tener que correr hasta la computadora.

> **Ojo:** es una ayuda, no un reemplazo de estar junto a la máquina. **Úsalo solo en una red de confianza y solo estando cerca del láser y viéndolo.** Nunca dejes un láser trabajando sin vigilancia. Tampoco se probó todavía con una máquina real: empieza con una máquina virtual.

## Antes de empezar: dos avisos de seguridad

- **No hay HTTPS.** La conexión entre el celular y la computadora **no está cifrada**: en una red local no se puede conseguir un certificado de confianza para una dirección privada. Quien pueda espiar tu red local (por ejemplo, un Wi-Fi sin cifrar o público) podría ver lo que pasa por ella. Úsalo en tu red de casa o del taller, con contraseña, **nunca en un Wi-Fi público**.
- **La clave cambia cada vez que abres la app.** El código QR lleva una clave aleatoria que se genera de nuevo cada vez que se abre Tlaku (y cuando pulsas **Regenerar clave**). Por eso el QR de ayer ya no sirve: escanea el de hoy. Nada se queda guardado en el celular que permita volver a entrar sin escanear.

El control remoto además viene **apagado de fábrica**, solo escucha en una dirección de tu **red local** (nunca en internet) y exige la clave. Un archivo de preferencias o un paquete `.tlakubundle` que importes jamás lo enciende.

## Paso a paso

1. En la computadora, abre **Preferencias** (el engranaje o `Ctrl/Cmd+,`) y entra a **Control remoto**.
2. Marca **Activar el control remoto desde el celular**. Debe aparecer "Escuchando en 192.168.x.x:47802" y, debajo, un código QR.

   ![Preferencias, Control remoto: puerto, dirección de red y el código QR](../../apps/desktop/docs/screenshots/62-control-remoto.png)

   Si la computadora está en varias redes, elige la correcta en **Dirección de red** (la **Automática** elige la más probable, normalmente una 192.168.x.x). Si el puerto está ocupado, cámbialo en **Puerto**.
3. Conecta el celular **a la misma red Wi-Fi** que la computadora.
4. Abre la cámara del celular, apunta al código QR y toca el enlace. Se abre la página de Tlaku. Si prefieres no usar la cámara, **Copiar enlace** copia la dirección con la clave para mandártela. La clave va dentro del código: no lo compartas ni lo fotografíes en público.
5. La computadora lista abajo **Quién está conectado** (dirección y aparato). Con **Desconectar** sacas a uno; con **Regenerar clave** invalidas el QR y sacas a todos (el celular muestra "Te desconectaron" y hay que escanear de nuevo).

## Qué ves en el celular

![La página del celular con la máquina lista, el avance y el recorrido](../../apps/desktop/docs/screenshots/63-celular.png)

- **Máquina**: el estado (**Listo**, **Trabajando**, **En pausa**, **Terminado**, **Detenido**, **Falló**, **Alarma**...) y el nombre de la máquina y del documento.
- **Avance**: el porcentaje, el tiempo transcurrido, el tiempo restante y la capa por la que va ("1 de 2").
- **Recorrido**: el dibujo del trabajo con un punto que sigue al cabezal.
- **Vista en vivo**: un interruptor que cambia el recorrido por la simulación del trabajo (ver abajo). Apagado de fábrica.
- **Cámara de la PC**: si la cámara está encendida en la computadora, una imagen que se actualiza cada pocos segundos (sin celulares conectados no se manda nada).
- Una **barra fija abajo**, siempre a la vista sin desplazar la página: con el trabajo en marcha o en pausa lleva **Pausar** (que pasa a **Reanudar**) y **Detener** (en rojo); en reposo lleva **Detener** e **Iniciar trabajo…** (si no se puede iniciar, arriba de la barra dice por qué). **Detener** pide un segundo toque en menos de 3 segundos y la barra muestra un aviso mientras espera.

![Trabajo en curso: 78 % de avance y el cabezal sobre el recorrido](../../apps/desktop/docs/screenshots/63d-celular-trabajando.png)

![Pausar, Detener e Iniciar trabajo con un trabajo en curso: Iniciar queda bloqueado y dice por qué](../../apps/desktop/docs/screenshots/63e-celular-trabajando-botones.png)

Cuando el trabajo termina o falla, la página muestra un aviso a pantalla completa (**¡Trabajo terminado!** o **El trabajo falló**), suena un pitido, parpadea el título de la pestaña y mantiene la pantalla encendida mientras trabaja. Además **vibra el celular**, salvo en **Safari de iPhone y iPad (iOS)**, que no permite que una página haga vibrar el aparato: allí solo verás y oirás el aviso (el pitido necesita que hayas tocado la página al menos una vez).

### Vista en vivo en el celular

El interruptor **Vista en vivo del trabajo** de la página dibuja lo mismo que la computadora: lo ya quemado en oscuro, lo pendiente tenue y el cabezal (retícula, con un punto rojo cuando el láser emite), siguiendo la posición real de la máquina y moviéndose con fluidez entre informes. Es **solo para mirar**: no añade ninguna acción y usa la misma clave y las mismas reglas de red local que el resto.

- **Apagado de fábrica.** Tu elección se guarda en el navegador de ese celular. Apagado no descarga la ruta ni anima nada.
- **Al encenderlo** (también con el trabajo a la mitad) la página baja una sola vez una versión simplificada de la ruta (unos cientos de KB como máximo; en rellenos e imágenes se agrupan las líneas y se dibujan más anchas) y se pone al día con todo lo ya hecho. Después solo recibe la posición.
- Si la ruta es demasiado grande, o la máquina no reporta su posición mientras trabaja (K40, galvo), la página lo dice y se ve el cabezal sobre el recorrido o un estimado por el tiempo.
- Funciona en Safari de iPhone y iPad y en Chrome de Android. Con **reducir movimiento** activado en el sistema, el cabezal salta de informe en informe en vez de animarse. El dibujo tiene una descripción en texto con el porcentaje y el estado; el porcentaje y el estado también están arriba, en **Avance** y **Máquina**.
- La computadora solo sigue el trabajo para el celular mientras **haya algún celular conectado**.

## Qué se puede hacer y qué no

| Acción | Desde el celular |
|---|---|
| **Pausar y reanudar** | Siempre, con cualquier máquina que tenga pausa. |
| **Detener** | **Siempre.** Va directo a la máquina, sin pasar por la ventana de la computadora: si Tlaku se congela, detener sigue funcionando. No mira el estado, el encuadre ni la lista de comprobación. En la página pide un segundo toque ("Toca otra vez para DETENER") para evitar toques sin querer. |
| **Iniciar un trabajo** | Solo si se cumple **todo** lo siguiente (abajo). |
| **Ver la cola de trabajos** | Sí, en una tarjeta de solo lectura: estado, avance, tiempo que falta, el trabajo actual y la lista. |
| **Confirmar el siguiente trabajo de la cola** | Con las mismas reglas que iniciar (abajo) y una casilla más en el checklist del teléfono: *Ya cambié el material o la pieza*. El documento que la cola espera debe estar **encuadrado en la computadora** y ser el de la pestaña activa; si no, solo se confirma desde la computadora. La cola nunca avanza sola. Ver [Cola e historial de trabajos](09f-cola-e-historial-de-trabajos.md). |
| **Ver avance, recorrido, vista en vivo y cámara** | Sí. |
| **Iniciar un láser galvo** | **Nunca.** El haz de un galvo es invisible: solo se inicia desde la computadora. El galvo tampoco tiene pausa, solo detener. |
| **Cambiar el diseño, los ajustes o la máquina** | No: eso se hace en la computadora. |

### Iniciar desde el celular

Para que **Iniciar trabajo…** funcione se necesita, a la vez:

1. **Marcar el checklist en el celular**: *Estoy junto a la máquina*, *El área está despejada* y *El aire o el extractor están encendidos*.

   ![Lista de comprobación en el celular antes de iniciar](../../apps/desktop/docs/screenshots/63c-celular-checklist.png)

2. **Que el trabajo esté encuadrado en la computadora después del último cambio.** Antes, en la computadora, usa **Máquina > Encuadrar** (ver [Enviar trabajos con seguridad](09-enviar-con-seguridad.md)). Cualquier cambio posterior al diseño, a la máquina o a las opciones del trabajo exige encuadrar de nuevo, y reconectar la máquina también. Es el seguro de que alguien miró dónde va a trabajar el láser.
3. Máquina conectada y en reposo, sin otro trabajo en curso, algo que enviar, y la ventana de Tlaku abierta y respondiendo.

El celular no inicia nada por su cuenta: **le pide a Tlaku en la computadora que inicie**, y Tlaku hace su revisión previa de siempre y vuelve a comprobar el encuadre. Si algo no se cumple, el celular dice el motivo con claridad ("Primero encuadra el trabajo en la PC", "La máquina no está en reposo", "El galvo solo se inicia desde la PC"...). Si Tlaku no contesta en un minuto, avisa que revises la computadora antes de volver a intentar.

En la computadora, la sección **Control remoto** de Preferencias recuerda estas reglas (**Qué puede hacer el celular**) y dice si el trabajo está encuadrado o no.

![Quién está conectado, las reglas del celular y el aviso de trabajo sin encuadrar](../../apps/desktop/docs/screenshots/62b-control-remoto-reglas.png)

## Cómo se protege

- Solo escucha en **una** dirección privada de tu red local (jamás en todas las interfaces) y rechaza a quien no venga de la red local.
- La clave es aleatoria, de 128 bits, y viaja en la parte del enlace que el navegador **no envía** al servidor ni guarda en el historial; la página la entrega una sola vez y recibe una sesión.
- Cinco claves incorrectas seguidas bloquean esa dirección dos minutos. Las sesiones sin actividad caducan a los 30 minutos y hay un máximo de 8 aparatos.
- **Detener** y **Pausar** nunca dependen del encuadre ni de la lista de comprobación: la seguridad es que se pueda parar siempre, y arrancar cueste más.

El detalle técnico (rutas, cabeceras, límites) está en el repositorio, en la sección «Control remoto desde el celular» de `docs/app-api/control-remoto.md`.

## Solución de problemas

- **El celular no abre la página.** Revisa que estén en la misma red Wi-Fi (no en la red de invitados, que suele aislar los aparatos entre sí), que el control remoto diga "Escuchando en…" y que el firewall de la computadora deje entrar al puerto. Si la computadora tiene varias redes, elige la correcta en **Dirección de red**.
- **"Clave incorrecta o vencida".** Escanea otra vez el QR actual: la clave cambia cada vez que se abre Tlaku o se pulsa **Regenerar clave**.
- **"Demasiados intentos".** Espera unos minutos y escanea de nuevo.
- **"La PC no está enviando datos".** Tlaku está cerrado o sin responder. Detener sigue funcionando, pero iniciar queda bloqueado.
- **Iniciar trabajo… no se deja pulsar.** Debajo del botón dice el motivo; casi siempre es que falta **encuadrar** en la computadora.

Siguiente: [Cámara](10-camara.md)
