Fuente de prueba
================
BebasNeue-Regular.ttf: Bebas Neue 2019, copyright The Bebas Neue Project Authors
(https://github.com/dharmatype/Bebas-Neue), licencia SIL Open Font License 1.1
(texto completo en OFL.txt). Se incluye sin modificar, obtenida de google/fonts (ofl/bebasneue).

Se eligió por ser pequeña (61 KB), TrueType con contornos cuadráticos, con kerning (par "AV")
y con letras con huecos ("O", "B"). Solo tiene el peso Regular, lo que permite probar el
fallback de negrita y cursiva. Las pruebas no dependen de las fuentes del sistema.

TlakuTest-Var.ttf
=================
Fuente sintética del proyecto Tlaku (licencia MIT, la del repositorio): cada letra es un rectángulo
de altura conocida, no deriva de ninguna fuente existente. La genera `make_test_font.py`
(`python3 make_test_font.py`, necesita fonttools) y se incluye ya generada (6 KB). Sirve para probar:
- GSUB: liga, dlig, salt, ss01, ss02, swsh, smcp, lnum, onum, tnum, pnum y frac; GPOS: kern.
- Cinco ejes variables: wght 400-900, wdth 100-150, slnt -12-0, opsz 8-72 y uno propio (TLKX 0-100),
  con HVAR (los avances cambian con el eje) y dos instancias con nombre (Regular y Bold Wide).
Familia: "Tlaku Test". Las pruebas están en `../opentype.rs`.
