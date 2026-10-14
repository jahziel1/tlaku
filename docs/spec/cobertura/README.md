# 04 - Cobertura funcional real de Tlaku frente al inventario de LightBurn

Este documento se partió en un archivo por área (antes era un solo archivo de 128 KB). Aquí están el resumen y los totales; el detalle de cada fila está en los archivos de la tabla de abajo. La nota de auditoría, cómo leer las tablas, el historial por ola y las discrepancias vigentes están en [auditoria.md](auditoria.md).

## 1. Resumen

### Por prioridad

|  | Hecho | Parcial | Solo motor/CLI | Falta | No aplica | Total |
|---|---:|---:|---:|---:|---:|---:|
| P0 | 64 | 4 | 0 | 0 | 0 | 68 |
| P1 | 75 | 13 | 0 | 0 | 0 | 88 |
| P2 | 39 | 10 | 10 | 2 | 0 | 61 |
| Total | 178 | 27 | 10 | 2 | 0 | 217 |

### Por área

|  | Hecho | Parcial | Solo motor/CLI | Falta | No aplica | Total |
|---|---:|---:|---:|---:|---:|---:|
| Diseño y edición | 90 | 8 | 0 | 2 | 0 | 100 |
| Capas, corte e imagen | 56 | 2 | 2 | 0 | 0 | 60 |
| Máquinas y avanzadas | 32 | 17 | 8 | 0 | 0 | 57 |

### Por área y prioridad

| Área | Prio | Hecho | Parcial | Solo motor/CLI | Falta | No aplica | Total |
|---|---|---:|---:|---:|---:|---:|---:|
| Diseño y edición | P0 | 32 | 0 | 0 | 0 | 0 | 32 |
| Diseño y edición | P1 | 35 | 4 | 0 | 0 | 0 | 39 |
| Diseño y edición | P2 | 23 | 4 | 0 | 2 | 0 | 29 |
| Capas, corte e imagen | P0 | 21 | 1 | 0 | 0 | 0 | 22 |
| Capas, corte e imagen | P1 | 22 | 1 | 0 | 0 | 0 | 23 |
| Capas, corte e imagen | P2 | 13 | 0 | 2 | 0 | 0 | 15 |
| Máquinas y avanzadas | P0 | 11 | 3 | 0 | 0 | 0 | 14 |
| Máquinas y avanzadas | P1 | 18 | 8 | 0 | 0 | 0 | 26 |
| Máquinas y avanzadas | P2 | 3 | 6 | 8 | 0 | 0 | 17 |

Antes de la ola 5.5: 175 hechas, 27 parciales, 10 solo motor/CLI y 2 faltantes (P0: 64 / 4 / 0 / 0). Antes de la ola 5: 173 hechas, 26 parciales, 0 solo motor/CLI y 15 faltantes (P0: 64 / 4 / 0 / 0). Antes de la ola 4: 165 hechas, 23 parciales, 3 solo motor/CLI y 23 faltantes (P0: 62 / 6 / 0 / 0). Antes de la ola 3: 143 hechas, 22 parciales, 3 solo motor/CLI y 46 faltantes (P0 igual: 62 / 6 / 0 / 0). Antes de la ola 2: 120 hechas, 27 parciales, 6 solo motor/CLI y 61 faltantes contando las filas (el resumen de entonces decía 26 parciales y 62 faltantes: una fila de capas P1 figuraba como Falta en el resumen y como Parcial en su tabla). Antes de la ola 1: 88 hechas, 31 parciales, 6 solo motor/CLI y 89 faltantes (P0: 53 / 13 / 1 / 1).

**Lectura rápida.** De 217 funciones, 178 están hechas (82 %), 27 parciales, 10 solo en el motor o el CLI y 2 faltan. En P0 (el MVP) hay 64 de 68 hechas (94 %), igual que tras la ola 4; las 4 parciales son C51, M3, M5 y M6, y no queda ninguna P0 solo en el motor ni faltante. El área de diseño y edición tiene hechas 90 de 100 y el de capas, corte e imagen 56 de 60. En máquinas y avanzadas hay 32 hechas de 57 (de 29 de 54 antes de la ola 5.5, y de 27 antes de la ola 5), 17 parciales y 8 solo en el motor o el CLI. Las dos funciones que faltan son D90 (otros formatos de máquina) y D94 (exportar AI), ambas P2. Las 10 funciones "solo motor/CLI" son todas del galvo (M30, M40 a M46, C47 y C59): existen en el motor y en `tlaku galvo`, pero la app todavía no las ofrece. <!-- GALVO-APP --> Sigue sin haber ninguna prueba con hardware real: lo que la ola 4 sumó para Ruida, el K40, el ojo de pez, la cámara en el cabezal y el buscador de centro, y lo que la ola 5 sumó para el galvo, Print & Cut y el tiling, se probó solo contra emuladores, máquinas virtuales e imágenes sintéticas.

## Archivos por área

| Archivo | Contenido | Filas |
|---|---|---|
| [diseno.md](diseno.md) | 2.1 Diseño y edición (inventario 01) | D01 a D100 |
| [capas.md](capas.md) | 2.2 Capas, corte e imagen (inventario 02) | C01 a C60 |
| [maquinas.md](maquinas.md) | 2.3 Máquinas y funciones avanzadas (inventario 03) | M1 a M57 |
| [usuario-real.md](usuario-real.md) | 2.4 Lo que un usuario real necesita y el inventario no cubre bien (no cuenta en los totales) | |
| [lanzamiento.md](lanzamiento.md) | 3. Lo que falta para un primer lanzamiento público | |
| [diferenciales.md](diferenciales.md) | 4. Lo que Tlaku tiene y LightBurn no | |
| [auditoria.md](auditoria.md) | Nota de auditoría, cómo leer, historial por ola y discrepancias | |
