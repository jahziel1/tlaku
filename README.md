# Tlaku

**Software libre para cortadoras y grabadoras láser.** Tlaku es una alternativa de código abierto
(licencia MIT) a los programas comerciales de control láser: diseñas o importas un trabajo, ajustas
las capas, ves cómo se va a cortar y lo envías a la máquina con revisiones de seguridad antes de
que se mueva nada.

El nombre viene del náhuatl *tlacuilo*: el pintor-escriba que dibujaba y escribía los códices. Un
tlacuilo convertía ideas en trazos sobre una superficie; Tlaku convierte un diseño en movimientos
de un láser.

**Para quién es:** personas con una cortadora o grabadora láser de diodo o CO2 que quieren un
programa abierto y que explique lo que hace, sobre todo quienes vienen de LightBurn y quienes
empiezan. Hay una guía específica para el cambio: [venir desde LightBurn](docs/manual/14-desde-lightburn.md).

> **Estado honesto:** Tlaku ya tiene aplicación de escritorio completa, pero **ninguna función de
> máquina se ha probado todavía con un láser real**. Todo se ha validado con un emulador de
> controladoras y con máquinas virtuales dentro de la app. Hasta que haya pruebas con hardware,
> trata cada trabajo como una prueba: empieza con poca potencia, material de desecho y la mano
> cerca del botón de paro. Detalle en [controladoras soportadas](#controladoras-soportadas).

![Ventana principal de Tlaku con un diseño de ejemplo y sus capas](apps/desktop/docs/screenshots/01-principal-oscuro.png)

## Funciones principales

- **Diseño y edición:** lienzo con reglas y cama de la máquina, selección, transformar con
  manijas o con valores exactos, rectángulos, elipses, líneas, texto con fuentes del sistema
  (incluido texto sobre un trayecto), editor de nodos, agrupar, alinear y distribuir, arreglos en
  rejilla y circulares, deshacer con historial.
- **Operaciones de geometría:** unión, diferencia, intersección, soldar, offset, unir trazos,
  cerrar trayectos y quitar duplicados.
- **Importar:** SVG (con recorte `clip-path` real), DXF, imágenes (PNG, JPG, BMP) y vectorizar una
  imagen con vista previa.
- **Capas de corte:** modos Línea, Relleno e Imagen; velocidad, potencia, pasadas, intervalo,
  ángulo, bidireccional, overscan calculado con la física de la máquina, kerf, puentes (tabs),
  aire, eje Z por capa y por pasada.
- **Imagen:** dithering (Floyd-Steinberg, Jarvis, Stucki, Atkinson, Bayer), semitono, periódico,
  boceto, umbral, escala de grises y ajustes de brillo, contraste y gamma.
- **Vista previa:** reproducción del recorrido con tiempo estimado y desglose por capa.
- **Máquina:** asistente "Agregar máquina" con detección de solo lectura, conexión serie, mover el
  cabezal, origen y "Iniciar desde", origen del trabajo de 9 puntos, encuadre, pausa y paro,
  consola con respuestas explicadas y exportar G-code.
- **Taller:** biblioteca de materiales y prueba de materiales, rotativo (rodillo o mandril),
  corrección de barrido (scanning offset), cámara con calibración de lente, alineación y marcas
  grabadas.

![Vista previa del trabajo con reproducción y resultados por capa](apps/desktop/docs/screenshots/04-vista-previa.png)

## Lo que lo distingue

- **Planificador de corte explicable.** Cada paso del orden de corte indica qué regla lo decidió
  (capa, interior antes que contenedor, solape, cercanía) y los conflictos se avisan en vez de
  resolverse en silencio. Se ve en la vista previa y con `tlaku plan`.
- **Máquina virtual.** Puedes conectar la app a una máquina simulada (Falcon2 Pro, GRBL, grblHAL,
  FluidNC, Marlin y otras) y provocar fallas a propósito: llama, inclinación, alarma, desconexión.
  Sirve para aprender, probar un flujo completo y reportar errores sin tocar un láser.
- **Revisión de seguridad antes de enviar.** Antes de mover la máquina se consulta su estado y no
  se transmite nada si está en alarma, en pausa, con la puerta abierta o en marcha. Se avisa antes
  de empezar, no a mitad del trabajo, si el overscan se sale de la cama. La app nunca desbloquea
  ni hace *homing* por su cuenta, y la consola pide confirmación y explica cada `ALARM:N` y
  `error:N`.
- **Importadores que avisan.** Unidades ausentes en un DXF, SVG a 96 o a 72 DPI, texto sin contorno,
  recortes y máscaras: nada se pierde en silencio, cada omisión sale como advertencia.
- **Código abierto y formatos abiertos.** Licencia MIT; proyecto `.tlaku` en JSON versionado,
  materiales `.tlakumat`, y todo lo que hace la app se puede ejecutar desde la línea de comandos
  (`tlaku`) y comprobar con pruebas.
- **Una interfaz que explica.** Un comando no disponible dice por qué, en vez de quedar en gris
  sin motivo.

![Máquina virtual conectada, con el panel de fallas simuladas](apps/desktop/docs/screenshots/25-maquina-virtual.png)

| | | |
|---|---|---|
| ![Editor de nodos](apps/desktop/docs/screenshots/11-editor-de-nodos.png) | ![Panel de cámara](apps/desktop/docs/screenshots/17-panel-camara.png) | ![Biblioteca de materiales](apps/desktop/docs/screenshots/19-materiales.png) |
| Editor de nodos | Cámara | Materiales |

### En camino (próximamente)

Estas funciones están en el plan inmediato y todavía no son parte de la app: exportar SVG, DXF e
imágenes; importar PDF, AI y más formatos de imagen; importar proyectos de LightBurn (`.lbrn`,
`.lbrn2`); preferencias, autoguardado y atajos configurables; voltear, bloquear y ordenar objetos;
más herramientas de dibujo; inglés. El estado real, función por función, está en
[docs/spec/04-cobertura.md](docs/spec/04-cobertura.md) y el plan en
[docs/plan-completo.md](docs/plan-completo.md).

## Controladoras soportadas

Ninguna se ha probado con una máquina real; "estable" significa que funciona de punta a punta
contra el emulador y las máquinas virtuales de la app, con pruebas automáticas. La primera prueba
con hardware real será una Creality Falcon2. Guía completa:
[docs/manual/13-controladoras.md](docs/manual/13-controladoras.md).

| Controladora o firmware | Estado | Notas |
|---|---|---|
| GRBL 1.1 (M4) y GRBL-M3, por USB serie | Estable en emulador; **por validar con máquina real** | La mayoría de los láseres de diodo (Creality Falcon2 incluida). Catálogo de 21 modelos con fuentes. |
| grblHAL y FluidNC | Estable en emulador; **por validar con máquina real** | Dialecto de G-code propio y máquina virtual. |
| Marlin y Smoothie | Estable en emulador (básico); **por validar con máquina real** | Envío línea a línea, aviso de posición perdida y máquina virtual. |
| GRBL antiguo (sin M4), GRBL-LPC y G-code personalizado | **Por validar con máquina real** | Dialecto propio; sin máquina virtual dedicada. El tipo personalizado trae un editor de dialecto. |
| Ruida (CO2) | **Experimental**, solo por línea de comandos | Exporta `.rd` y habla por red (UDP); no está en la app. |
| K40 con placa M2 Nano, galvo/fibra (EZCAD), Trocen, TopWisdom | No soportado | Un K40 con la placa cambiada a GRBL-LPC o Smoothie sí puede funcionar. |

## Instalación

### Descargar la aplicación

Los instaladores están en la página de **Releases** del repositorio (la versión más reciente,
sección "Assets"):

| Sistema | Archivo |
|---|---|
| macOS 11 o posterior (Apple Silicon e Intel) | `.dmg`: ábrelo y arrastra Tlaku a Aplicaciones |
| Windows 10 o posterior | `.exe` (instalador NSIS) o `.msi` |
| Linux | `.deb` (`sudo apt install ./Tlaku_*.deb`) o `.AppImage` (`chmod +x` y ejecútalo) |

**Aplicación sin firmar.** Mientras el proyecto no tenga certificados de firma, el sistema avisará
la primera vez que abras el instalador:

- **macOS:** clic derecho sobre Tlaku > Abrir, o Ajustes del Sistema > Privacidad y seguridad >
  "Abrir de todos modos". Si dice que la app está dañada:
  `xattr -dr com.apple.quarantine /Applications/Tlaku.app`.
- **Windows (SmartScreen):** "Más información" y luego "Ejecutar de todas formas".
- **Linux:** sin avisos. Para usar el puerto serie del láser, añade tu usuario al grupo
  `dialout` (`sudo usermod -aG dialout "$USER"`) y vuelve a iniciar sesión.

Más detalle, y cómo se publica cada versión, en [docs/publicar.md](docs/publicar.md).

### Compilar desde el código fuente

Requiere Rust estable reciente (edition 2024). En Linux, `serialport` necesita
`libudev-dev` y `pkg-config`.

```sh
git clone <url-del-repositorio> tlaku && cd tlaku
cargo build --release
# el binario del CLI queda en target/release/tlaku
```

Para compilar la aplicación de escritorio hace falta además Node 22.13 o superior y las
dependencias de Tauri de tu sistema (en Linux, `libwebkit2gtk-4.1-dev` y otras; ver la
[guía de Tauri](https://v2.tauri.app/start/prerequisites/)):

```sh
cd apps/desktop
npm ci
npm run tauri build     # instaladores en target/release/bundle/
```

Para desarrollar, `npm run tauri dev`. Detalles en [apps/desktop/README.md](apps/desktop/README.md).

## Contribuir y donar

Tlaku es de todos y lo hace posible quien lo prueba, lo mejora y lo difunde.

- **Probar con una máquina real** y contar qué pasó es hoy la ayuda más valiosa.
- **Reportar un problema:** enlace del repositorio **(PENDIENTE: publicar y poner aquí la
  dirección)**. Mientras tanto, el menú Ayuda de la app tiene "Reportar un problema" con los datos
  que conviene incluir.
- **Código y documentación:** lee [CONTRIBUTING.md](CONTRIBUTING.md). El proyecto es de sala limpia:
  no se copia código, textos ni capturas de programas comerciales.
- **Donar:** el proyecto no tiene empresa detrás y se sostiene con donaciones, que pagan las pruebas
  con máquinas reales, la firma de los instaladores y tiempo de desarrollo. Enlace de donaciones
  **(PENDIENTE: publicar y poner aquí la dirección)**. Donar es voluntario: el programa es el mismo.

## Documentación

- [Manual de usuario](docs/manual/00-indice.md): de los primeros pasos a la solución de problemas.
- [docs/arquitectura.md](docs/arquitectura.md): pipeline, responsabilidad de cada crate y convenciones.
- [docs/app-api.md](docs/app-api.md): contrato entre la interfaz y el motor.
- [docs/spec/00-resumen.md](docs/spec/00-resumen.md) y [04-cobertura.md](docs/spec/04-cobertura.md): inventario funcional y estado real.
- [docs/plan-completo.md](docs/plan-completo.md): plan por olas.
- [docs/hardware/](docs/hardware/): investigación de controladoras y máquinas, con fuentes.
- [docs/publicar.md](docs/publicar.md): cómo se publica una versión.
- [docs/ideas-mejoras.md](docs/ideas-mejoras.md): ideas por investigar.
- [THIRD_PARTY.md](THIRD_PARTY.md): código de terceros.

## El motor

El motor es una biblioteca en Rust organizada en crates pequeños que no conocen la interfaz;
la aplicación (`apps/desktop`, Tauri 2 y React) y el CLI usan los mismos.

| Crate | Qué hace |
|---|---|
| `tlaku-core` | Modelo del trabajo (puntos, polilíneas, capas, ajustes de corte), orden de corte básico y `Toolpath`, la representación común que consumen los drivers. |
| `tlaku-svg`, `tlaku-dxf` | Importan SVG y DXF a polilíneas en mm, una capa por color, y avisan de lo que no pueden importar. |
| `tlaku-geom` | Booleanas, soldar, offset, compensación de kerf, auto-join, cierre de trayectos, quitar duplicados y puentes. |
| `tlaku-plan` | Planificador del orden de corte explicable. |
| `tlaku-fill`, `tlaku-raster` | Relleno por barrido o por contornos; imágenes con dithering y barrido de bitmaps. |
| `tlaku-trace` | Vectorizado de imágenes. |
| `tlaku-text` | Texto a contornos con fuentes del sistema, kerning y texto sobre trayecto. |
| `tlaku-sim` | Estimación de tiempo con aceleración por eje y cálculo físico del overscan. |
| `tlaku-grbl` | G-code por dialecto, puertos serie, envío con conteo de caracteres y revisión previa. |
| `tlaku-ruida` | Experimental: archivos `.rd` y sesión por red para controladoras Ruida. |
| `tlaku-emu` | Emulador de controladoras (GRBL, Falcon2 Pro, grblHAL, FluidNC, GRBL-LPC, Marlin, Smoothie). |
| `tlaku-camera` | Calibración de lente, alineación y corrección por altura. |
| `tlaku-doc` | Tipos del contrato con la app, exportados a TypeScript. |
| `tlaku-pipeline` | Documento y máquina a `Toolpath`, estimaciones, límites de la cama y G-code. |
| `tlaku-cli` | Binario `tlaku`. |


## Línea de comandos

`tlaku <comando> --help` es la referencia completa de cada comando.

| Comando | Qué hace |
|---|---|
| `gcode` | Convierte un SVG, DXF o imagen (PNG, JPEG, BMP) en G-code para GRBL y muestra el resumen del trabajo (capas, tiempo estimado, área ocupada). |
| `plan` | Imprime el orden de capas y de corte con la razón de cada paso, sin generar nada. |
| `send` | Envía un archivo de G-code a la máquina con conteo de caracteres. Ctrl+C cancela con feed hold y soft reset. |
| `frame` | Recorre con el láser apagado el rectángulo que ocupa el diseño (incluido el overscan). |
| `cmd` | Envía comandos sueltos de GRBL (`$X`, `$H`, `$I`, G-code...) y muestra la respuesta completa, con errores y alarmas traducidos. |
| `ports` | Lista los puertos serie. |
| `info` | Lee la versión (`$I`) y la configuración (`$$`) de la máquina, sin moverla. |

Ejemplos, ejecutados desde la raíz del repositorio (usan el diseño de prueba
`crates/tlaku-cli/tests/fixtures/design.svg`: un contorno rojo y un rectángulo azul):

```sh
# SVG -> G-code. Cada color del diseño es una capa: el contorno rojo se corta en dos
# pasadas a 300 mm/min y el rectángulo azul se rellena a 3000 mm/min.
target/release/tlaku gcode crates/tlaku-cli/tests/fixtures/design.svg -o trabajo.gcode \
  --layer "#ff0000:speed=300,power=100,passes=2,air" \
  --layer "#0000ff:mode=fill,interval=0.2,speed=3000,power=30"

# Orden de las capas y de las figuras, con la razón de cada paso (no escribe archivos)
target/release/tlaku plan crates/tlaku-cli/tests/fixtures/design.svg \
  --layer "#ff0000:kerf=0.1" --layer "#0000ff:mode=fill,interval=2"

# Con la máquina conectada: busca el puerto, encuádrala y envía el trabajo
target/release/tlaku ports
target/release/tlaku frame crates/tlaku-cli/tests/fixtures/design.svg --port /dev/cu.usbserial-XXXX
target/release/tlaku send trabajo.gcode --port /dev/cu.usbserial-XXXX

# Máquina en alarma: lee su versión (sin preguntar) y haz homing (pide confirmación)
target/release/tlaku cmd --port /dev/cu.usbserial-XXXX '$I' '$H'
```

**Orden de las capas.** Para que la pieza no se suelte antes de grabar, por defecto
(`--layer-order auto`) los rellenos e imágenes van primero, luego las líneas y al final los
cortes: las líneas con kerf, o con potencia de 80 % o más y varias pasadas. Dentro de cada grupo
se conserva el orden del archivo. `--layer-order file` respeta solo el orden del archivo, y
`order=N` dentro de `--layer` fija la posición de una capa (desde 1) por encima de lo anterior.
El resumen muestra el orden final y por qué.

**Antes de enviar.** `send` y `frame` abren el puerto, consultan el estado (`?`) y solo
transmiten si la máquina está en reposo (`Idle`). En alarma, pausa (`Hold`), puerta abierta o en
marcha (`Run`) no transmiten nada y explican qué hacer; `$X` (desbloquear) y `$H` (homing) los
sugieren con el comando exacto de `tlaku cmd` pero nunca los ejecutan. Ambos piden
confirmación antes de mover la máquina (`--yes` la omite).

**`cmd`.** Cada argumento es un comando, se envían en orden y se detiene en el primer error o
alarma. Escribe los `$` entre comillas simples. Los de solo lectura (`$I`, `$$`, `$#`, `$G`, `$N`,
`?`) se envían sin preguntar. `$X` (advierte que desbloquear sin hacer home puede dejar la
posición desconocida), `$H` (la máquina se moverá; espera hasta 60 s a que termine) y todo lo
demás (G0/G1/G2/G3, `$J=`, `M3`/`M4`, ajustes `$N=valor`) piden confirmación mostrando exactamente
qué se enviará; sin respuesta (stdin cerrado) se rechaza. `--yes` omite las preguntas. Tiempo
real por nombre: `status`, `hold`, `resume` y `reset`.

Otras opciones de `gcode`: `--speed`, `--power`, `--passes`, `--air`, `--mode line|fill`,
`--s-max` (`$30`), `--bed` (área de trabajo, `400x415` por defecto), `--tolerance`, `--no-flip`,
`--power-mode dynamic|constant` (M4 o M3), `--start-gcode` y `--end-gcode`. Las imágenes
necesitan `--image-size` y `--image-pos`. `plan` y `frame` aceptan las mismas opciones de diseño.

Sin instalar, se puede usar con `cargo run --bin tlaku -- <comando>`.

## Pruebas

```sh
cargo test --workspace
```

Varias pruebas comparan su salida con archivos dorados (`tests/fixtures` de cada crate:
`tlaku-grbl`, `tlaku-fill`, `tlaku-raster`). Si un cambio es intencional, regenéralos con
`UPDATE_GOLDEN=1 cargo test --workspace` y revisa el diff antes de confirmar. Más detalles en
[CONTRIBUTING.md](CONTRIBUTING.md).

## Licencia

[MIT](LICENSE). Las dependencias deben tener licencias permisivas; `cargo deny check` lo
verifica (ver [deny.toml](deny.toml)). El código de terceros que se adaptó está en
[THIRD_PARTY.md](THIRD_PARTY.md).
