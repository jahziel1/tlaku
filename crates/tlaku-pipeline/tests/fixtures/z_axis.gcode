; Generado por Tlaku
G90
G21
M4 S0
; Relleno intervalo=2 ángulo=0 potencia=80%
G0 Z3
G0 X30 Y11
G1 X38 S800 F600
G0 Y13
G1 X30
G0 Y15
G1 X38
; Capa #ff0000 velocidad=600 potencia=80% pasadas=3 pasada 1/3 z=1
G0 Z1
G0 X22 Y18
G1 X10
G1 Y10
G1 X22
G1 Y18
; Capa #ff0000 velocidad=600 potencia=80% pasadas=3 pasada 2/3 z=1.5
G0 Z1.5
G1 X10
G1 Y10
G1 X22
G1 Y18
; Capa #ff0000 velocidad=600 potencia=80% pasadas=3 pasada 3/3 z=2
G0 Z2
G1 X10
G1 Y10
G1 X22
G1 Y18
M5
G0 X0 Y0
