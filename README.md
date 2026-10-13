# lasersoft

Software abierto (licencia MIT) para cortadoras y grabadoras láser. El objetivo a largo plazo es
ofrecer una alternativa libre a los programas comerciales de control láser; hoy es un motor
en Rust que se usa desde la línea de comandos.

## Estado

**Fase 1: motor sin interfaz.** Todo lo que hará la aplicación se puede ejecutar y probar desde
la línea de comandos, con archivos dorados. Todavía no hay interfaz gráfica, y el CLI cubre solo
una parte de lo que el motor ya sabe hacer. No es todavía un sustituto de ningún programa
comercial. La hoja de ruta está en [docs/spec/00-resumen.md](docs/spec/00-resumen.md).

## Qué funciona hoy

| Crate | Qué hace |
|---|---|
| `laser-core` | Modelo del trabajo (puntos, polilíneas, capas, ajustes de corte), orden de corte básico y `Toolpath`, la representación común que consumen los drivers. |
| `laser-svg` | Importa SVG: curvas a polilíneas en mm con tolerancia, una capa por color. |
| `laser-dxf` | Importa DXF: entidades a polilíneas en mm, color ACI o true color; avisa de lo que no puede importar. |
| `laser-geom` | Booleanas, soldar, offset, compensación de kerf, auto-join y cierre de trayectos. |
| `laser-plan` | Planificador del orden de corte explicable: capas, prioridad, interior antes que contenedor, recorrido corto (2-opt/Or-opt). |
| `laser-fill` | Modo Fill: relleno por barrido con ángulo, serpentina, crosshatch y overscan. |
| `laser-raster` | Modo Image: ajustes de imagen, remuestreo, dithering y umbral, barrido de bitmaps. |
| `laser-sim` | Estimación de tiempo con aceleración por eje y desviación de unión; cálculo físico del overscan. |
| `laser-grbl` | G-code para GRBL 1.1 (M3/M4), puertos serie y envío por conteo de caracteres. |
| `laser-doc` | Tipos del contrato con la app (documento, capas, máquina, opciones) en `serde`, exportados a TypeScript con `ts-rs`. |
| `laser-pipeline` | Documento + máquina a `Toolpath`, estimaciones, límites de la cama y G-code; importa SVG, DXF e imágenes. |
| `laser-cli` | Binario `lasersoft`; arma un documento con los argumentos y usa `laser-pipeline`. |

## Instalación desde el código fuente

Requiere Rust estable reciente (edition 2024). En Linux, `serialport` necesita
`libudev-dev` y `pkg-config`.

```sh
git clone <url-del-repositorio> lasersoft && cd lasersoft
cargo build --release
# el binario queda en target/release/lasersoft
```

## Uso del CLI

`lasersoft <comando> --help` es la referencia completa de cada comando.

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
`crates/laser-cli/tests/fixtures/design.svg`: un contorno rojo y un rectángulo azul):

```sh
# SVG -> G-code. Cada color del diseño es una capa: el contorno rojo se corta en dos
# pasadas a 300 mm/min y el rectángulo azul se rellena a 3000 mm/min.
target/release/lasersoft gcode crates/laser-cli/tests/fixtures/design.svg -o trabajo.gcode \
  --layer "#ff0000:speed=300,power=100,passes=2,air" \
  --layer "#0000ff:mode=fill,interval=0.2,speed=3000,power=30"

# Orden de las capas y de las figuras, con la razón de cada paso (no escribe archivos)
target/release/lasersoft plan crates/laser-cli/tests/fixtures/design.svg \
  --layer "#ff0000:kerf=0.1" --layer "#0000ff:mode=fill,interval=2"

# Con la máquina conectada: busca el puerto, encuádrala y envía el trabajo
target/release/lasersoft ports
target/release/lasersoft frame crates/laser-cli/tests/fixtures/design.svg --port /dev/cu.usbserial-XXXX
target/release/lasersoft send trabajo.gcode --port /dev/cu.usbserial-XXXX

# Máquina en alarma: lee su versión (sin preguntar) y haz homing (pide confirmación)
target/release/lasersoft cmd --port /dev/cu.usbserial-XXXX '$I' '$H'
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
sugieren con el comando exacto de `lasersoft cmd` pero nunca los ejecutan. Ambos piden
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

Sin instalar, se puede usar con `cargo run --bin lasersoft -- <comando>`.

## Pruebas

```sh
cargo test --workspace
```

Varias pruebas comparan su salida con archivos dorados (`tests/fixtures` de cada crate:
`laser-grbl`, `laser-fill`, `laser-raster`). Si un cambio es intencional, regenéralos con
`UPDATE_GOLDEN=1 cargo test --workspace` y revisa el diff antes de confirmar. Más detalles en
[CONTRIBUTING.md](CONTRIBUTING.md).

## Documentación

- [docs/arquitectura.md](docs/arquitectura.md): pipeline, responsabilidad de cada crate y convenciones.
- [docs/spec/00-resumen.md](docs/spec/00-resumen.md): inventario funcional y hoja de ruta.
- [docs/ideas-mejoras.md](docs/ideas-mejoras.md): ideas de mejora por investigar.
- [CONTRIBUTING.md](CONTRIBUTING.md): cómo contribuir (sala limpia, licencias, DCO).

## Licencia

[MIT](LICENSE). Las dependencias deben tener licencias permisivas; `cargo deny check` lo
verifica (ver [deny.toml](deny.toml)).
