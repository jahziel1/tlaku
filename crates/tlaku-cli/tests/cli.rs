//! Pruebas de integración: ejecutan el binario `tlaku` sobre fixtures pequeños y comparan
//! el G-code con archivos aprobados. Si un cambio es intencional, regenera con
//! `UPDATE_GOLDEN=1 cargo test -p tlaku-cli` y revisa el diff antes de confirmarlo.

use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Output, Stdio};

fn fixtures() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures")
}

fn fixture(name: &str) -> String {
    fixtures().join(name).to_str().unwrap().to_string()
}

/// Carpeta de trabajo propia de cada prueba.
fn workdir(test: &str) -> PathBuf {
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR"))
        .join("tlaku-cli")
        .join(test);
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

fn run(args: &[&str]) -> Output {
    run_with_stdin(args, "")
}

fn run_with_stdin(args: &[&str], stdin: &str) -> Output {
    let mut child = Command::new(env!("CARGO_BIN_EXE_tlaku"))
        .args(args)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    child
        .stdin
        .take()
        .unwrap()
        .write_all(stdin.as_bytes())
        .unwrap();
    child.wait_with_output().unwrap()
}

fn out(o: &Output) -> String {
    String::from_utf8_lossy(&o.stdout).into_owned()
}

fn err(o: &Output) -> String {
    String::from_utf8_lossy(&o.stderr).into_owned()
}

fn assert_ok(o: &Output) {
    assert!(o.status.success(), "falló:\n{}\n{}", out(o), err(o));
}

/// El comando debe fallar y su stderr incluir `needle`.
fn assert_fails(o: &Output, needle: &str) {
    assert!(!o.status.success(), "debía fallar:\n{}", out(o));
    let e = err(o);
    assert!(e.contains(needle), "stderr no contiene {needle:?}:\n{e}");
}

/// Compara `actual` con `tests/fixtures/<golden>`; con UPDATE_GOLDEN lo reescribe.
fn check_golden(golden: &str, actual: &str) {
    let path = fixtures().join(golden);
    if std::env::var_os("UPDATE_GOLDEN").is_some() {
        std::fs::write(&path, actual).unwrap();
    }
    let expected = std::fs::read_to_string(&path)
        .unwrap_or_else(|_| panic!("falta {}; ejecuta con UPDATE_GOLDEN=1", path.display()));
    assert_eq!(actual, expected, "'{golden}' cambió");
}

// ---------------------------------------------------------------------------
// Imágenes generadas a mano (sin dependencias nuevas)
// ---------------------------------------------------------------------------

fn crc32(data: &[u8]) -> u32 {
    let mut crc = 0xFFFF_FFFFu32;
    for &b in data {
        crc ^= u32::from(b);
        for _ in 0..8 {
            crc = if crc & 1 == 1 {
                (crc >> 1) ^ 0xEDB8_8320
            } else {
                crc >> 1
            };
        }
    }
    !crc
}

fn adler32(data: &[u8]) -> u32 {
    let (mut a, mut b) = (1u32, 0u32);
    for &x in data {
        a = (a + u32::from(x)) % 65521;
        b = (b + a) % 65521;
    }
    (b << 16) | a
}

fn png_chunk(out: &mut Vec<u8>, kind: &[u8; 4], data: &[u8]) {
    out.extend((data.len() as u32).to_be_bytes());
    let mut body = kind.to_vec();
    body.extend(data);
    out.extend(&body);
    out.extend(crc32(&body).to_be_bytes());
}

/// PNG en escala de grises de 8 bits con deflate sin comprimir (bloques "stored").
fn gray_png(width: u32, height: u32, pixel: impl Fn(u32, u32) -> u8) -> Vec<u8> {
    let mut raw = Vec::new();
    for y in 0..height {
        raw.push(0); // filtro: ninguno
        raw.extend((0..width).map(|x| pixel(x, y)));
    }
    let mut z = vec![0x78, 0x01];
    let blocks: Vec<&[u8]> = raw.chunks(65535).collect();
    for (i, block) in blocks.iter().enumerate() {
        z.push(u8::from(i + 1 == blocks.len()));
        z.extend((block.len() as u16).to_le_bytes());
        z.extend((!(block.len() as u16)).to_le_bytes());
        z.extend(*block);
    }
    z.extend(adler32(&raw).to_be_bytes());

    let mut png = b"\x89PNG\r\n\x1a\n".to_vec();
    let mut ihdr = Vec::new();
    ihdr.extend(width.to_be_bytes());
    ihdr.extend(height.to_be_bytes());
    ihdr.extend([8, 0, 0, 0, 0]);
    png_chunk(&mut png, b"IHDR", &ihdr);
    png_chunk(&mut png, b"IDAT", &z);
    png_chunk(&mut png, b"IEND", &[]);
    png
}

/// BMP de 24 bits; la fila 0 de `pixel` queda arriba.
fn gray_bmp(width: u32, height: u32, pixel: impl Fn(u32, u32) -> u8) -> Vec<u8> {
    let row_bytes = (width * 3).div_ceil(4) * 4;
    let size = 54 + row_bytes * height;
    let mut bmp = b"BM".to_vec();
    bmp.extend(size.to_le_bytes());
    bmp.extend([0u8; 4]);
    bmp.extend(54u32.to_le_bytes());
    bmp.extend(40u32.to_le_bytes());
    bmp.extend(width.to_le_bytes());
    bmp.extend(height.to_le_bytes());
    bmp.extend(1u16.to_le_bytes());
    bmp.extend(24u16.to_le_bytes());
    bmp.extend([0u8; 24]);
    for y in (0..height).rev() {
        let mut row: Vec<u8> = (0..width).flat_map(|x| [pixel(x, y); 3]).collect();
        row.resize(row_bytes as usize, 0);
        bmp.extend(row);
    }
    bmp
}

/// Degradado horizontal de 16x16: negro a la izquierda, casi blanco a la derecha.
fn gradient(x: u32, _y: u32) -> u8 {
    (x * 16).min(255) as u8
}

fn write_png(dir: &Path) -> String {
    let path = dir.join("degradado.png");
    std::fs::write(&path, gray_png(16, 16, gradient)).unwrap();
    path.to_str().unwrap().to_string()
}

// ---------------------------------------------------------------------------
// gcode: archivos dorados
// ---------------------------------------------------------------------------

const SVG_LAYERS: [&str; 4] = [
    "--layer",
    "#ff0000:speed=300,power=100,passes=2,air",
    "--layer",
    "#0000ff:mode=fill,interval=2,speed=3000,power=15,angle=45,crosshatch",
];

#[test]
fn golden_svg_line_and_fill() {
    let dir = workdir("svg");
    let output = dir.join("out.gcode");
    let o = run(&[
        &[
            "gcode",
            &fixture("design.svg"),
            "-o",
            output.to_str().unwrap(),
        ],
        &SVG_LAYERS[..],
    ]
    .concat());
    assert_ok(&o);
    check_golden("design.gcode", &std::fs::read_to_string(&output).unwrap());

    let text = out(&o);
    assert!(text.contains("#ff0000  line  2 trayectorias"), "{text}");
    assert!(text.contains("#0000ff  fill"), "{text}");
    assert!(text.contains("ahorro"), "{text}");
    assert!(text.contains("Tiempo total estimado"), "{text}");
    assert!(text.contains("overscan"), "{text}");
}

#[test]
fn svg_y_axis_flips_unless_no_flip() {
    let dir = workdir("flip");
    let a = dir.join("a.gcode");
    let b = dir.join("b.gcode");
    let svg = fixture("design.svg");
    assert_ok(&run(&["gcode", &svg, "-o", a.to_str().unwrap()]));
    assert_ok(&run(&[
        "gcode",
        &svg,
        "-o",
        b.to_str().unwrap(),
        "--no-flip",
    ]));
    let (ga, gb) = (
        std::fs::read_to_string(a).unwrap(),
        std::fs::read_to_string(b).unwrap(),
    );
    assert_ne!(ga, gb);
    // El rectángulo azul está en y=10..20 del SVG (60 mm de alto): con volteo queda en Y40..50.
    assert!(ga.contains("G0 X75 Y40") && !ga.contains("Y10"), "{ga}");
    assert!(gb.contains("G0 X75 Y10") && !gb.contains("Y40"), "{gb}");
}

#[test]
fn golden_dxf_is_not_flipped() {
    let dir = workdir("dxf");
    let output = dir.join("out.gcode");
    let o = run(&[
        "gcode",
        &fixture("mini.dxf"),
        "-o",
        output.to_str().unwrap(),
        "--layer",
        "#0000ff:mode=fill,interval=2,power=10",
    ]);
    assert_ok(&o);
    let gcode = std::fs::read_to_string(&output).unwrap();
    check_golden("mini.gcode", &gcode);
    // La línea roja está en Y=10 en el DXF y así debe salir.
    assert!(gcode.contains("Y10"), "{gcode}");
    assert!(out(&o).contains("#ff0000  line"));
}

#[test]
fn golden_png_image() {
    let dir = workdir("png");
    let png = write_png(&dir);
    let output = dir.join("out.gcode");
    let o = run(&[
        "gcode",
        &png,
        "-o",
        output.to_str().unwrap(),
        "--image-size",
        "8x8",
        "--image-pos",
        "10,20",
        "--interval",
        "0.5",
        "--image-mode",
        "threshold",
        "--power",
        "40",
        "--speed",
        "6000",
        "--overscan",
        "1",
    ]);
    assert_ok(&o);
    check_golden(
        "degradado.gcode",
        &std::fs::read_to_string(&output).unwrap(),
    );
    let text = out(&o);
    assert!(text.contains("imagen   image"), "{text}");
    assert!(text.contains("16 líneas"), "{text}");
}

#[test]
fn bmp_and_all_image_options_are_accepted() {
    let dir = workdir("bmp");
    let bmp = dir.join("degradado.bmp");
    std::fs::write(&bmp, gray_bmp(16, 16, gradient)).unwrap();
    for mode in [
        "threshold",
        "floyd",
        "jarvis",
        "stucki",
        "atkinson",
        "bayer4",
        "grayscale",
    ] {
        let output = dir.join(format!("{mode}.gcode"));
        let o = run(&[
            "gcode",
            bmp.to_str().unwrap(),
            "-o",
            output.to_str().unwrap(),
            "--image-size",
            "8x8",
            "--image-pos",
            "5,5",
            "--interval",
            "0.5",
            "--image-mode",
            mode,
            "--min-power",
            "5",
            "--brightness",
            "0.1",
            "--contrast",
            "0.2",
            "--gamma",
            "1.2",
            "--invert",
            "--dot-width",
            "0.05",
        ]);
        assert_ok(&o);
        assert!(
            std::fs::read_to_string(&output).unwrap().contains("G1"),
            "{mode}"
        );
    }
}

#[test]
fn auto_overscan_follows_machine_acceleration() {
    let dir = workdir("accel");
    let output = dir.join("out.gcode");
    let run_with = |accel: &str| {
        out(&run(&[
            "gcode",
            &fixture("design.svg"),
            "-o",
            output.to_str().unwrap(),
            "--layer",
            "#0000ff:mode=fill,interval=2,speed=6000",
            "--machine-accel",
            accel,
        ]))
    };
    // 6000 mm/min = 100 mm/s: v²/2a = 5 mm con 1000 mm/s² y 50 mm con 100 mm/s².
    assert!(run_with("1000").contains("overscan 5.00 mm"));
    assert!(run_with("100").contains("overscan 50.00 mm"));
}

#[test]
fn explicit_overscan_and_global_mode() {
    let dir = workdir("overscan");
    let output = dir.join("out.gcode");
    let o = run(&[
        "gcode",
        &fixture("design.svg"),
        "-o",
        output.to_str().unwrap(),
        "--mode",
        "fill",
        "--interval",
        "3",
        "--overscan",
        "0.5",
    ]);
    assert_ok(&o);
    let text = out(&o);
    assert!(
        text.contains("#ff0000  fill") && text.contains("#0000ff  fill"),
        "{text}"
    );
    assert!(text.contains("overscan 0.50 mm"), "{text}");
}

#[test]
fn open_paths_warn_in_fill_and_kerf_and_kerf_changes_output() {
    let dir = workdir("warnings");
    let output = dir.join("out.gcode");
    // La capa roja del DXF son dos líneas abiertas: el relleno las ignora y avisa.
    let o = run(&[
        "gcode",
        &fixture("mini.dxf"),
        "-o",
        output.to_str().unwrap(),
        "--layer",
        "#ff0000:mode=fill",
    ]);
    assert_ok(&o);
    let text = out(&o);
    assert!(text.contains("Advertencias"), "{text}");
    assert!(
        text.contains("2 trayectoria(s) abierta(s) ignorada(s)"),
        "{text}"
    );
    assert!(text.contains("no produjo ninguna línea"), "{text}");

    // El kerf tampoco se aplica a abiertas.
    let o = run(&[
        "gcode",
        &fixture("mini.dxf"),
        "-o",
        output.to_str().unwrap(),
        "--layer",
        "#ff0000:kerf=0.2",
    ]);
    assert_ok(&o);
    assert!(out(&o).contains("no se compensan por kerf"), "{}", out(&o));

    let plain = dir.join("plain.gcode");
    let kerf = dir.join("kerf.gcode");
    let base = ["gcode", &fixture("design.svg"), "-o"];
    assert_ok(&run(&[&base[..], &[plain.to_str().unwrap()]].concat()));
    assert_ok(&run(&[
        &base[..],
        &[kerf.to_str().unwrap(), "--layer", "#ff0000:kerf=0.1"],
    ]
    .concat()));
    assert_ne!(
        std::fs::read_to_string(plain).unwrap(),
        std::fs::read_to_string(kerf).unwrap()
    );
}

#[test]
fn unknown_layer_color_is_a_warning() {
    let dir = workdir("unknown-layer");
    let output = dir.join("out.gcode");
    let o = run(&[
        "gcode",
        &fixture("design.svg"),
        "-o",
        output.to_str().unwrap(),
        "--layer",
        "#123456:speed=10",
    ]);
    assert_ok(&o);
    assert!(
        out(&o).contains("la capa #123456 de --layer no existe"),
        "{}",
        out(&o)
    );
}

#[test]
fn dxf_without_units_warns() {
    let dir = workdir("dxf-units");
    let dxf = dir.join("sin_unidades.dxf");
    let text = std::fs::read_to_string(fixture("mini.dxf"))
        .unwrap()
        .replace("$INSUNITS\n70\n4\n", "$INSUNITS\n70\n0\n");
    std::fs::write(&dxf, text).unwrap();
    let o = run(&[
        "gcode",
        dxf.to_str().unwrap(),
        "-o",
        dir.join("o.gcode").to_str().unwrap(),
    ]);
    assert_ok(&o);
    assert!(out(&o).contains("no declara unidades"), "{}", out(&o));
}

// ---------------------------------------------------------------------------
// Orden de capas
// ---------------------------------------------------------------------------

/// Genera design.svg (azul = relleno) con los argumentos extra. Devuelve la salida en pantalla y
/// la posición, en el G-code, de la capa roja y del relleno.
fn layer_positions(test: &str, extra: &[&str]) -> (String, usize, usize) {
    let dir = workdir(test);
    let output = dir.join("out.gcode");
    let o = run(&[
        &[
            "gcode",
            &fixture("design.svg"),
            "-o",
            output.to_str().unwrap(),
            "--layer",
            "#0000ff:mode=fill,interval=2",
        ][..],
        extra,
    ]
    .concat());
    assert_ok(&o);
    let g = std::fs::read_to_string(&output).unwrap();
    let red = g.find("; Capa #ff0000").expect("falta la capa roja");
    let blue = g.find("; Relleno").expect("falta el relleno");
    (out(&o), red, blue)
}

#[test]
fn auto_order_engraves_first_and_cuts_last() {
    // Roja = contorno con potencia 100 y 2 pasadas (corte); azul = relleno.
    let args = ["--layer", "#ff0000:power=100,passes=2"];
    let (text, red, blue) = layer_positions("order-auto", &args);
    assert!(blue < red, "el grabado debe ir antes que el corte");
    assert!(
        text.contains("Orden automático: grabados primero, cortes al final"),
        "{text}"
    );
    assert!(
        text.contains("#ff0000 por potencia ≥ 80 % con varias pasadas"),
        "{text}"
    );
    let (a, b) = (
        text.find("1. #0000ff").unwrap(),
        text.find("2. #ff0000").unwrap(),
    );
    assert!(a < b, "{text}");
}

#[test]
fn kerf_makes_a_line_layer_a_cut() {
    let args = ["--layer", "#ff0000:kerf=0.1"];
    let (text, red, blue) = layer_positions("order-kerf", &args);
    assert!(blue < red);
    assert!(text.contains("#ff0000 por kerf"), "{text}");
}

#[test]
fn plain_lines_go_between_fills_and_cuts() {
    // Con la potencia por defecto la capa roja es una línea normal, no un corte; la azul
    // (relleno) va antes de ella aunque aparezca después en el archivo.
    let (text, red, blue) = layer_positions("order-line", &[]);
    assert!(blue < red);
    assert!(
        text.contains("Orden automático: grabados primero, líneas al final"),
        "{text}"
    );
    assert!(!text.contains("(corte:"), "{text}");
}

#[test]
fn file_order_keeps_the_design_order() {
    let args = [
        "--layer",
        "#ff0000:power=100,passes=2",
        "--layer-order",
        "file",
    ];
    let (text, red, blue) = layer_positions("order-file", &args);
    assert!(
        red < blue,
        "con --layer-order file manda el orden del archivo"
    );
    assert!(text.contains("Orden del archivo"), "{text}");
    assert!(!text.contains("Orden automático"), "{text}");
}

#[test]
fn explicit_order_beats_auto_order() {
    // El corte se fuerza a la posición 1: gana sobre "cortes al final".
    let args = ["--layer", "#ff0000:power=100,passes=2,order=1"];
    let (text, red, blue) = layer_positions("order-explicit", &args);
    assert!(red < blue);
    assert!(text.contains("#ff0000 en la posición 1"), "{text}");
    // Dos capas no pueden pedir la misma posición.
    let o = run(&[
        "plan",
        &fixture("design.svg"),
        "--layer",
        "#ff0000:order=1",
        "--layer",
        "#0000ff:mode=fill,order=1",
    ]);
    assert_fails(&o, "misma posición: order=1");
    assert_fails(
        &run(&["plan", &fixture("design.svg"), "--layer", "#ff0000:order=0"]),
        "entero desde 1",
    );
    assert_fails(
        &run(&["plan", &fixture("design.svg"), "--layer-order", "otro"]),
        "otro",
    );
}

#[test]
fn plan_and_frame_show_the_layer_order() {
    let args = ["--layer", "#ff0000:kerf=0.1"];
    let blue = ["--layer", "#0000ff:mode=fill,interval=2"];
    let o = run(&[&["plan", &fixture("design.svg")][..], &args, &blue].concat());
    assert_ok(&o);
    let text = out(&o);
    assert!(
        text.starts_with("Orden automático: grabados primero, cortes al final"),
        "{text}"
    );
    assert!(
        text.find("Capa 1 de 2: #0000ff").unwrap() < text.find("Capa 2 de 2: #ff0000").unwrap()
    );

    let o = run_with_stdin(
        &[
            &["frame", &fixture("design.svg"), "--port", "/dev/no-existe"][..],
            &args,
            &blue,
        ]
        .concat(),
        "n\n",
    );
    assert_fails(&o, "cancelado");
    assert!(
        out(&o).contains("Capas en orden: 1. #0000ff (fill), 2. #ff0000 (line)."),
        "{}",
        out(&o)
    );
}

// ---------------------------------------------------------------------------
// gcode: errores
// ---------------------------------------------------------------------------

#[test]
fn design_outside_bed_fails_without_writing() {
    let dir = workdir("bounds");
    let output = dir.join("out.gcode");
    let o = run(&[
        "gcode",
        &fixture("design.svg"),
        "-o",
        output.to_str().unwrap(),
        "--bed",
        "50x50",
    ]);
    assert_fails(&o, "se sale del área de trabajo de 50x50 mm");
    assert!(err(&o).contains("#ff0000"));
    assert!(
        !output.exists(),
        "no debe escribirse G-code fuera de límites"
    );
}

#[test]
fn bed_check_includes_overscan() {
    let dir = workdir("bounds-overscan");
    let output = dir.join("out.gcode");
    let args = |bed: &'static str| {
        vec![
            "gcode",
            Box::leak(fixture("design.svg").into_boxed_str()),
            "-o",
            Box::leak(output.to_str().unwrap().to_string().into_boxed_str()),
            "--layer",
            "#0000ff:mode=fill,interval=2,overscan=1.25",
            "--bed",
            bed,
        ]
    };
    // El rectángulo azul llega a X95: cabe en 95.5 mm, pero con el overscan (96.25) no.
    let o = run(&args("95.5x60"));
    assert_fails(&o, "incluye 1.25 mm de overscan");
    assert!(err(&o).contains("#0000ff"));
    assert_ok(&run(&args("96.5x60")));
}

#[test]
fn invalid_options_have_clear_errors() {
    let dir = workdir("invalid");
    let output = dir.join("out.gcode");
    let o = output.to_str().unwrap();
    let svg = fixture("design.svg");
    let base = |extra: &[&str]| {
        let mut a = vec!["gcode", svg.as_str(), "-o", o];
        a.extend_from_slice(extra);
        run(&a)
    };
    assert_fails(
        &base(&["--layer", "#ff0000:foo=1"]),
        "clave desconocida 'foo'",
    );
    assert_fails(
        &base(&["--layer", "#ff0000:speed=abc"]),
        "valor inválido en 'speed=abc'",
    );
    assert_fails(
        &base(&["--layer", "#ff0000:power=150"]),
        "potencia fuera de 0-100",
    );
    assert_fails(&base(&["--layer", "#ff0000:passes=0"]), "al menos 1");
    assert_fails(
        &base(&["--layer", "#ff0000:interval=1"]),
        "solo aplica en modo fill",
    );
    assert_fails(
        &base(&["--layer", "#0000ff:mode=fill,kerf=1"]),
        "solo aplica en modo line",
    );
    assert_fails(&base(&["--layer", "#zzzzzz:speed=1"]), "color inválido");
    assert_fails(
        &base(&["--layer", "#ff0000:speed=1", "--layer", "#ff0000:power=1"]),
        "más de una vez",
    );
    assert_fails(
        &base(&["--layer", "#0000ff:mode=fill,interval=0"]),
        "intervalo inválido",
    );
    assert_fails(&base(&["--speed", "abc"]), "no es un número");
    assert_fails(&base(&["--speed=-5"]), "mayor que 0");
    assert_fails(&base(&["--bed", "400"]), "ANCHOxALTO");
    assert_fails(&base(&["--overscan", "mucho"]), "usa \"auto\" o un número");
    assert_fails(&base(&["--machine-accel", "0"]), "mayor que 0");
    assert!(!output.exists());

    let o2 = run(&["gcode", "no_existe.svg", "-o", o]);
    assert_fails(&o2, "no_existe.svg");
    let pdf = dir.join("a.pdf");
    std::fs::write(&pdf, b"%PDF").unwrap();
    assert_fails(
        &run(&["gcode", pdf.to_str().unwrap(), "-o", o]),
        "formato de entrada no soportado",
    );
    let bad = dir.join("roto.svg");
    std::fs::write(&bad, b"esto no es un svg").unwrap();
    assert_fails(
        &run(&["gcode", bad.to_str().unwrap(), "-o", o]),
        "SVG inválido",
    );
    let bad = dir.join("roto.dxf");
    std::fs::write(&bad, b"esto no es un dxf").unwrap();
    assert_fails(
        &run(&["gcode", bad.to_str().unwrap(), "-o", o]),
        "DXF inválido",
    );
}

#[test]
fn image_requires_size_and_position_and_valid_adjustments() {
    let dir = workdir("image-errors");
    let png = write_png(&dir);
    let output = dir.join("out.gcode");
    let o = output.to_str().unwrap();
    assert_fails(&run(&["gcode", &png, "-o", o]), "requieren --image-size");
    assert_fails(
        &run(&["gcode", &png, "-o", o, "--image-size", "8x8"]),
        "--image-pos",
    );
    let sized = [
        "gcode",
        png.as_str(),
        "-o",
        o,
        "--image-size",
        "8x8",
        "--image-pos",
        "0,0",
    ];
    let with = |extra: &[&str]| run(&[&sized[..], extra].concat());
    let no_size = |size: &str, pos: &str| {
        run(&[
            "gcode",
            &png,
            "-o",
            o,
            "--image-size",
            size,
            "--image-pos",
            pos,
        ])
    };
    assert_fails(&with(&["--gamma", "0"]), "gamma");
    assert_fails(&with(&["--brightness", "2"]), "brillo");
    assert_fails(
        &with(&["--threshold", "0.3"]),
        "solo aplica con --image-mode threshold",
    );
    assert_fails(&with(&["--image-mode", "otro"]), "otro");
    assert_fails(&with(&["--min-power", "50", "--power", "10"]), "mínima");
    assert_fails(&no_size("0x5", "0,0"), "tamaño inválido");
    assert_fails(&no_size("8x8", "5"), "X,Y");
    // Una imagen que se sale de la cama.
    assert_fails(&no_size("8x8", "395,0"), "se sale del área");
    let broken = dir.join("rota.png");
    std::fs::write(&broken, b"no soy un png").unwrap();
    assert_fails(
        &run(&[
            "gcode",
            broken.to_str().unwrap(),
            "-o",
            o,
            "--image-size",
            "8x8",
            "--image-pos",
            "0,0",
        ]),
        "imagen inválida",
    );
}

#[test]
fn image_options_on_vector_input_are_reported() {
    let dir = workdir("image-ignored");
    let o = run(&[
        "gcode",
        &fixture("design.svg"),
        "-o",
        dir.join("o.gcode").to_str().unwrap(),
        "--invert",
        "--image-size",
        "5x5",
    ]);
    assert_ok(&o);
    assert!(out(&o).contains("se ignoraron las opciones de imagen (--image-size, --invert)"));
}

// ---------------------------------------------------------------------------
// plan
// ---------------------------------------------------------------------------

#[test]
fn golden_plan_explains_each_step() {
    let o = run(&[
        "plan",
        &fixture("design.svg"),
        "--layer",
        "#0000ff:mode=fill,interval=2",
    ]);
    assert_ok(&o);
    let text = out(&o);
    check_golden("design.plan.txt", &text);
    assert!(text.contains("primera figura de"));
    assert!(text.contains("la más cercana"));
    assert!(text.contains("Recorrido en vacío"));
}

#[test]
fn plan_without_gcode_output_and_with_open_paths() {
    let o = run(&["plan", &fixture("mini.dxf")]);
    assert_ok(&o);
    let text = out(&o);
    assert!(text.contains("abierta"), "{text}");
    assert!(!text.contains("G1"));
    assert_fails(&run(&["plan", "no_existe.svg"]), "no_existe.svg");
}

// ---------------------------------------------------------------------------
// send y frame sin hardware
// ---------------------------------------------------------------------------

#[test]
fn send_validates_the_file_before_touching_the_port() {
    let dir = workdir("send");
    let o = run(&[
        "send",
        "no_existe.gcode",
        "--port",
        "/dev/no-existe",
        "--yes",
    ]);
    assert_fails(&o, "no_existe.gcode");

    let bad = dir.join("mal.gcode");
    std::fs::write(&bad, "G1 X1 ?\n").unwrap();
    assert_fails(
        &run(&[
            "send",
            bad.to_str().unwrap(),
            "--port",
            "/dev/no-existe",
            "--yes",
        ]),
        "tiempo real",
    );

    let empty = dir.join("vacio.gcode");
    std::fs::write(&empty, "; solo un comentario\n").unwrap();
    assert_fails(
        &run(&[
            "send",
            empty.to_str().unwrap(),
            "--port",
            "/dev/no-existe",
            "--yes",
        ]),
        "ninguna línea de G-code",
    );
    assert_fails(&run(&["send", "x.gcode"]), "--port");
}

#[test]
fn send_asks_for_confirmation_and_declining_does_not_open_the_port() {
    let dir = workdir("send-confirm");
    let file = dir.join("ok.gcode");
    std::fs::write(&file, "G90\nG21\nM5\n").unwrap();
    let args = ["send", file.to_str().unwrap(), "--port", "/dev/no-existe"];
    let o = run_with_stdin(&args, "n\n");
    assert_fails(&o, "cancelado: no se movió la máquina");
    assert!(out(&o).contains("¿El láser está listo y el área despejada? [s/N]"));
    assert!(!out(&o).contains("Conectando"));
    // Sin respuesta (stdin cerrado) tampoco se mueve nada.
    assert_fails(&run_with_stdin(&args, ""), "cancelado");
    // Con confirmación se intenta abrir el puerto, que no existe.
    let o = run_with_stdin(&args, "s\n");
    assert_fails(&o, "/dev/no-existe");
    assert!(out(&o).contains("Conectando"));
}

#[test]
fn frame_prints_corners_including_overscan_before_asking() {
    let o = run_with_stdin(
        &[
            "frame",
            &fixture("design.svg"),
            "--layer",
            "#0000ff:mode=fill,interval=2,overscan=1.25",
            "--speed",
            "1500",
            "--port",
            "/dev/no-existe",
        ],
        "n\n",
    );
    assert_fails(&o, "cancelado");
    let text = out(&o);
    for corner in [
        "inferior izquierda  X5.00 Y5.00",
        "inferior derecha    X96.25 Y5.00",
        "superior derecha    X96.25 Y55.00",
        "superior izquierda  X5.00 Y55.00",
    ] {
        assert!(text.contains(corner), "falta {corner:?} en:\n{text}");
    }
    assert!(text.contains("velocidad 1500 mm/min"), "{text}");
    assert!(text.contains("¿El láser está listo"));
}

#[test]
fn frame_rejects_designs_outside_the_bed() {
    let o = run_with_stdin(
        &[
            "frame",
            &fixture("design.svg"),
            "--bed",
            "50x50",
            "--port",
            "/dev/no-existe",
            "--yes",
        ],
        "",
    );
    assert_fails(&o, "se sale del área");
}

// ---------------------------------------------------------------------------
// Del G-code real a la máquina simulada
// ---------------------------------------------------------------------------

/// Dispositivo que responde `ok` a cada línea y `Idle` a cada `?`; guarda lo recibido.
fn ack_device(
    mut dev: tlaku_grbl::serial::MemoryTransport,
) -> std::thread::JoinHandle<Vec<String>> {
    use std::time::Duration;
    use tlaku_grbl::serial::Transport;
    std::thread::spawn(move || {
        let (mut seen, mut partial) = (Vec::new(), String::new());
        let mut buf = [0u8; 256];
        while let Ok(n) = dev.read_timeout(&mut buf, Duration::from_millis(50)) {
            for &b in &buf[..n] {
                match b {
                    b'?' => {
                        let _ = dev.write_all(b"<Idle|MPos:0.000,0.000,0.000|FS:0,0>\r\n");
                    }
                    b'\n' => {
                        seen.push(std::mem::take(&mut partial));
                        let _ = dev.write_all(b"ok\r\n");
                    }
                    c => partial.push(c as char),
                }
            }
        }
        seen
    })
}

#[test]
fn golden_gcode_streams_completely_through_a_memory_transport() {
    use std::sync::atomic::AtomicBool;
    use tlaku_cli::link::{stream_lines, streamer_config, validate_gcode};

    let text = std::fs::read_to_string(fixtures().join("design.gcode")).unwrap();
    let count = validate_gcode(&text, 127).unwrap();
    let lines: Vec<String> = text.lines().map(str::to_owned).collect();
    let (host, dev) = tlaku_grbl::serial::memory_pair();
    let device = ack_device(dev);
    let mut shown = Vec::new();
    stream_lines(
        host,
        &lines,
        streamer_config(127),
        &AtomicBool::new(false),
        false,
        &mut shown,
    )
    .unwrap();
    let shown = String::from_utf8(shown).unwrap();
    assert!(
        shown.contains("100 %") && shown.contains(&format!("({count}/{count} líneas)")),
        "{shown}"
    );
    assert_eq!(device.join().unwrap().len(), count);
}

#[test]
fn frame_gcode_streams_through_a_memory_transport() {
    use std::sync::atomic::AtomicBool;
    use tlaku_cli::link::{frame_gcode, stream_lines, streamer_config};
    use tlaku_core::{Bounds, Point};

    let b = Bounds {
        min: Point::new(5.0, 5.0),
        max: Point::new(96.25, 55.0),
    };
    let lines = frame_gcode(b, 1500.0);
    let (host, dev) = tlaku_grbl::serial::memory_pair();
    let device = ack_device(dev);
    stream_lines(
        host,
        &lines,
        streamer_config(127),
        &AtomicBool::new(false),
        false,
        &mut Vec::new(),
    )
    .unwrap();
    let seen = device.join().unwrap();
    assert!(seen.iter().all(|l| !l.contains("M3") && !l.contains("M4")));
    assert_eq!(seen.last().map(String::as_str), Some("M5"));
    assert!(seen.contains(&"G1X96.25Y55S0F1500".to_string()), "{seen:?}");
}

// ---------------------------------------------------------------------------
// send: comprobación del estado antes de transmitir
// ---------------------------------------------------------------------------

/// Máquina falsa en un estado fijo: responde `status` a cada `?`, `settings` a `$$` y `ok` a
/// lo demás. Devuelve todo lo que recibió (`?` incluido, una entrada por cada uno).
fn state_device(
    mut dev: tlaku_grbl::serial::MemoryTransport,
    status: &'static str,
    settings: Option<&'static str>,
) -> std::thread::JoinHandle<Vec<String>> {
    use std::time::Duration;
    use tlaku_grbl::serial::Transport;
    std::thread::spawn(move || {
        let (mut seen, mut partial) = (Vec::new(), String::new());
        let mut buf = [0u8; 256];
        while let Ok(n) = dev.read_timeout(&mut buf, Duration::from_millis(20)) {
            for &b in &buf[..n] {
                match b {
                    b'?' => {
                        seen.push("?".to_string());
                        let _ = dev.write_all(format!("{status}\r\n").as_bytes());
                    }
                    b'\n' => {
                        let line = std::mem::take(&mut partial);
                        if line == "$$" {
                            if let Some(s) = settings {
                                let _ = dev.write_all(s.as_bytes());
                                let _ = dev.write_all(b"ok\r\n");
                            }
                        } else {
                            let _ = dev.write_all(b"ok\r\n");
                        }
                        seen.push(line);
                    }
                    c => partial.push(c as char),
                }
            }
        }
        seen
    })
}

/// Intenta enviar un G-code mínimo a una máquina falsa en el estado dado. Devuelve el
/// resultado, lo que se mostró y lo que la máquina recibió.
fn send_to_state(
    status: &'static str,
    settings: Option<&'static str>,
) -> (Result<(), String>, String, Vec<String>) {
    use std::sync::atomic::AtomicBool;
    use std::time::Duration;
    use tlaku_cli::link::{PreflightConfig, send_job, streamer_config};

    let (host, dev) = tlaku_grbl::serial::memory_pair();
    let device = state_device(dev, status, settings);
    let lines = ["G90", "G21", "M5", "G0 X1 Y1"].map(String::from);
    let mut shown = Vec::new();
    let result = send_job(
        host,
        &lines,
        streamer_config(127),
        &PreflightConfig {
            status_timeout: Duration::from_secs(2),
            settings_timeout: Duration::from_millis(300),
            port: "/dev/cu.test".into(),
        },
        &AtomicBool::new(false),
        false,
        &mut shown,
    );
    (
        result,
        String::from_utf8(shown).unwrap(),
        device.join().unwrap(),
    )
}

/// Ni G-code ni comandos que cambien el estado: solo consultas (`?`, `$$`).
fn assert_only_queries(seen: &[String]) {
    assert!(
        seen.iter().all(|l| l == "?" || l == "$$"),
        "se transmitió algo: {seen:?}"
    );
}

#[test]
fn send_goes_ahead_when_the_machine_is_idle() {
    let (result, shown, seen) = send_to_state("<Idle|MPos:0.000,0.000,0.000|FS:0,0>", None);
    result.unwrap();
    assert!(shown.contains("Estado de la máquina: reposo"), "{shown}");
    assert!(shown.contains("Trabajo terminado"), "{shown}");
    for l in ["G90", "G21", "M5", "G0X1Y1"] {
        assert!(seen.iter().any(|s| s == l), "falta {l} en {seen:?}");
    }
}

#[test]
fn send_refuses_alarm_and_suggests_home_when_homing_is_enabled() {
    let (result, shown, seen) = send_to_state(
        "<Alarm|MPos:0.000,0.000,0.000|FS:0,0>",
        Some("$21=0\r\n$22=1\r\n$30=1000\r\n"),
    );
    let e = result.unwrap_err();
    assert!(
        e.contains("ALARMA") && e.contains("No se transmitió nada"),
        "{e}"
    );
    assert!(
        e.contains("$22=1") && e.contains("$H") && e.contains("$X"),
        "{e}"
    );
    assert!(shown.contains("Estado de la máquina: ALARMA"), "{shown}");
    assert!(!shown.contains("Enviando"), "{shown}");
    assert_only_queries(&seen);
}

#[test]
fn send_refuses_alarm_and_suggests_unlock_when_homing_is_off() {
    let (result, _, seen) =
        send_to_state("<Alarm|MPos:0.000,0.000,0.000|FS:0,0>", Some("$22=0\r\n"));
    let e = result.unwrap_err();
    assert!(
        e.contains("$22=0") && e.contains("lo normal es desbloquear con $X"),
        "{e}"
    );
    assert!(!e.contains("lo normal es hacer $H"), "{e}");
    assert_only_queries(&seen);
}

#[test]
fn send_refuses_alarm_with_both_options_when_homing_is_unknown() {
    // La máquina no contesta a `$$`: no se sabe si tiene homing.
    let (result, _, seen) = send_to_state("<Alarm|MPos:0.000,0.000,0.000|FS:0,0>", None);
    let e = result.unwrap_err();
    assert!(e.contains("finales de carrera (homing): $H"), "{e}");
    assert!(e.contains("si no los tiene: $X"), "{e}");
    assert_only_queries(&seen);
}

#[test]
fn send_refuses_hold_door_and_run() {
    for (status, needle) in [
        ("<Hold:0|MPos:0.000,0.000,0.000|FS:0,0>", "en pausa (Hold)"),
        ("<Door:1|MPos:0.000,0.000,0.000|FS:0,0>", "puerta"),
        (
            "<Run|MPos:1.000,0.000,0.000|FS:1000,500>",
            "trabajo en curso",
        ),
    ] {
        let (result, shown, seen) = send_to_state(status, None);
        let e = result.unwrap_err();
        assert!(e.contains("no está en reposo") && e.contains(needle), "{e}");
        assert!(e.contains("No se transmitió nada"), "{e}");
        assert!(!shown.contains("Enviando"), "{shown}");
        assert_only_queries(&seen);
    }
}

#[test]
fn send_fails_clearly_when_the_machine_does_not_answer_status() {
    use std::sync::atomic::AtomicBool;
    use std::time::Duration;
    use tlaku_cli::link::{PreflightConfig, send_job, streamer_config};

    // Extremo vivo pero mudo: nadie contesta al `?`.
    let (host, _silent) = tlaku_grbl::serial::memory_pair();
    let e = send_job(
        host,
        &["G90".to_string()],
        streamer_config(127),
        &PreflightConfig {
            status_timeout: Duration::from_millis(300),
            settings_timeout: Duration::from_millis(100),
            ..PreflightConfig::default()
        },
        &AtomicBool::new(false),
        false,
        &mut Vec::new(),
    )
    .unwrap_err();
    assert!(e.contains("no respondió a la consulta de estado"), "{e}");
}

// ---------------------------------------------------------------------------
// cmd: comandos sueltos de GRBL
// ---------------------------------------------------------------------------

/// Máquina falsa para `cmd`: por cada línea recibida llama a `reply` y envía lo que devuelva;
/// `?` responde con un reporte Idle y el soft reset (0x18) con el saludo de GRBL. Guarda
/// líneas y bytes de tiempo real en orden (`<0x18>`, `<!>`, `<~>`, `?`).
fn cmd_device(
    mut dev: tlaku_grbl::serial::MemoryTransport,
    reply: fn(&str) -> (u64, String),
) -> std::thread::JoinHandle<Vec<String>> {
    use std::time::Duration;
    use tlaku_grbl::serial::Transport;
    std::thread::spawn(move || {
        let (mut seen, mut partial) = (Vec::new(), String::new());
        let mut buf = [0u8; 256];
        while let Ok(n) = dev.read_timeout(&mut buf, Duration::from_millis(20)) {
            for &b in &buf[..n] {
                match b {
                    b'?' => {
                        seen.push("?".to_string());
                        let _ = dev.write_all(b"<Idle|MPos:1.000,2.000,0.000|FS:0,0>\r\n");
                    }
                    0x18 => {
                        seen.push("<0x18>".to_string());
                        let _ = dev.write_all(b"\r\nGrbl 1.1h ['$' for help]\r\n");
                    }
                    b'!' => seen.push("<!>".to_string()),
                    b'~' => seen.push("<~>".to_string()),
                    b'\n' => {
                        let line = std::mem::take(&mut partial);
                        let (delay_ms, text) = reply(&line);
                        std::thread::sleep(Duration::from_millis(delay_ms));
                        let _ = dev.write_all(text.as_bytes());
                        seen.push(line);
                    }
                    c => partial.push(c as char),
                }
            }
        }
        seen
    })
}

fn grbl_reply(line: &str) -> (u64, String) {
    let text = match line {
        "$I" => "[VER:1.1h.20190825:]\r\n[OPT:V,15,128]\r\nok\r\n",
        "$X" => "[MSG:Caution: Unlocked]\r\nok\r\n",
        "G99" => "error:20\r\n",
        "$H" => return (400, "ok\r\n".into()),
        "$J=G91X1000" => "ALARM:2\r\n",
        _ => "ok\r\n",
    };
    (0, text.into())
}

/// Ejecuta `args` de `cmd` contra la máquina falsa. Devuelve resultado, salida y lo recibido.
fn run_cmd(
    args: &[&str],
    config: tlaku_cli::link::CmdConfig,
) -> (Result<(), String>, String, Vec<String>) {
    use tlaku_cli::link::{parse_command, run_commands};
    let cmds: Vec<_> = args
        .iter()
        .map(|a| parse_command(a, 127).unwrap())
        .collect();
    let (mut host, dev) = tlaku_grbl::serial::memory_pair();
    let device = cmd_device(dev, grbl_reply);
    let mut shown = Vec::new();
    let result = run_commands(&mut host, &cmds, &config, &mut shown);
    drop(host);
    (
        result,
        String::from_utf8(shown).unwrap(),
        device.join().unwrap(),
    )
}

#[test]
fn cmd_read_only_commands_run_without_confirmation_and_show_the_whole_reply() {
    use tlaku_cli::link::{CmdConfig, confirm_commands, parse_command};

    let cmds = [
        parse_command("$I", 127).unwrap(),
        parse_command("$$", 127).unwrap(),
    ];
    // Sin confirmar nada: stdin vacío y sin --yes, y aun así pasa.
    let mut asked = Vec::new();
    confirm_commands(&cmds, false, &mut "".as_bytes(), &mut asked).unwrap();
    assert!(asked.is_empty());

    let (result, shown, seen) = run_cmd(&["$I", "$$"], CmdConfig::default());
    result.unwrap();
    assert!(shown.contains("> $I"), "{shown}");
    assert!(
        shown.contains("[VER:1.1h.20190825:]") && shown.contains("[OPT:V,15,128]"),
        "{shown}"
    );
    assert!(shown.contains("> $$"), "{shown}");
    assert_eq!(seen, ["$I", "$$"]);
}

#[test]
fn cmd_unlock_is_refused_without_yes_in_non_interactive_mode() {
    // stdin vacío: ni siquiera se abre el puerto (que no existe).
    for stdin in ["", "\n", "n\n"] {
        let o = run_with_stdin(&["cmd", "--port", "/dev/no-existe", "$X"], stdin);
        assert_fails(&o, "cancelado: no se envió nada");
        let text = out(&o);
        assert!(text.contains("Se enviará exactamente: $X"), "{text}");
        assert!(text.contains("posición puede quedar desconocida"), "{text}");
        assert!(!text.contains("Conectando"), "{text}");
    }
    // Un comando delicado cancela todo el lote, aunque otros sean de solo lectura.
    let o = run(&["cmd", "--port", "/dev/no-existe", "$I", "$H"]);
    assert_fails(&o, "cancelado");
    assert!(out(&o).contains("la máquina se moverá"), "{}", out(&o));
    // Con confirmación se intenta abrir el puerto; con --yes también, sin preguntar.
    let o = run_with_stdin(&["cmd", "--port", "/dev/no-existe", "$X"], "s\n");
    assert_fails(&o, "/dev/no-existe");
    assert!(out(&o).contains("Conectando"));
    let o = run(&["cmd", "--port", "/dev/no-existe", "--yes", "$X"]);
    assert_fails(&o, "/dev/no-existe");
    assert!(!out(&o).contains("¿Enviarlo?"));
}

#[test]
fn cmd_read_only_does_not_ask_in_the_binary() {
    let o = run(&["cmd", "--port", "/dev/no-existe", "$I", "$G", "?"]);
    assert_fails(&o, "/dev/no-existe");
    assert!(!out(&o).contains("¿Enviarlo?"));
    assert!(out(&o).contains("Conectando"));
}

#[test]
fn cmd_asks_for_everything_that_moves_fires_or_changes_settings() {
    use tlaku_cli::link::{Risk, parse_command};
    for (cmd, risk) in [
        ("g0 x10 y10", Risk::Moves),
        ("G1 X5 F1000", Risk::Moves),
        ("G2 X5 Y5 I1 J0", Risk::Moves),
        ("G3 X5 Y5 R2", Risk::Moves),
        ("$J=G91 X10 F500", Risk::Moves),
        ("M3 S500", Risk::Laser),
        ("M4 S1", Risk::Laser),
        ("M3", Risk::Laser),
        ("$30=1000", Risk::Setting),
        ("$N0=G54", Risk::Setting),
        ("$RST=$", Risk::Other),
        ("$X", Risk::Unlock),
        ("$H", Risk::Home),
        ("$I", Risk::Safe),
        ("$$", Risk::Safe),
        ("$#", Risk::Safe),
        ("$G", Risk::Safe),
        ("$N", Risk::Safe),
        ("M5", Risk::Safe),
    ] {
        let parsed = parse_command(cmd, 127).unwrap();
        assert_eq!(parsed.risk(), risk, "{cmd}");
        assert_eq!(risk.warning().is_none(), risk == Risk::Safe, "{cmd}");
    }
    // M3 S0 no enciende el láser, pero tampoco está en la lista de solo lectura.
    assert_eq!(parse_command("M3 S0", 127).unwrap().risk(), Risk::Other);

    // La confirmación muestra exactamente lo que se enviará (normalizado).
    let cmds = [parse_command("g1 x5 s0 f100", 127).unwrap()];
    let mut shown = Vec::new();
    let e = tlaku_cli::link::confirm_commands(&cmds, false, &mut "n\n".as_bytes(), &mut shown)
        .unwrap_err();
    assert!(e.contains("cancelado"));
    assert!(
        String::from_utf8(shown)
            .unwrap()
            .contains("exactamente: G1X5S0F100")
    );
}

#[test]
fn cmd_realtime_commands_by_name() {
    use tlaku_cli::link::CmdConfig;
    let config = CmdConfig {
        reset_wait: std::time::Duration::from_millis(300),
        ..CmdConfig::default()
    };
    let (result, shown, seen) = run_cmd(&["status", "hold", "resume", "reset"], config);
    result.unwrap();
    assert!(
        shown.contains("<Idle|MPos:1.000,2.000,0.000|FS:0,0>"),
        "{shown}"
    );
    assert!(shown.contains("Estado: reposo"), "{shown}");
    assert!(shown.contains("Grbl 1.1h"), "{shown}");
    // Orden: ?, ! (+ ? para mostrar el estado), ~ (+ ?), reset sin `\n` ni `?`.
    assert_eq!(seen, ["?", "<!>", "?", "<~>", "?", "<0x18>"]);

    // Los caracteres de tiempo real crudos no se aceptan como comando de línea.
    let e = tlaku_cli::link::parse_command("!", 127).unwrap_err();
    assert!(e.contains("status, hold, resume o reset"), "{e}");
    assert!(tlaku_cli::link::parse_command("~", 127).is_err());
    // `status` no pide confirmación ni cabe en una línea de G-code.
    let o = run(&[
        "cmd",
        "--port",
        "/dev/no-existe",
        "status",
        "hold",
        "resume",
    ]);
    assert!(!out(&o).contains("¿Enviarlo?"));
}

#[test]
fn cmd_translates_errors_and_alarms_and_stops_at_the_first() {
    use tlaku_cli::link::CmdConfig;
    let (result, shown, seen) = run_cmd(&["G99", "$I"], CmdConfig::default());
    let e = result.unwrap_err();
    assert!(
        e.contains("error:20") && e.contains("comando G-code no soportado"),
        "{e}"
    );
    assert!(!shown.contains("> $I"), "{shown}");
    assert_eq!(seen, ["G99"], "no debe seguir tras el error");

    let (result, _, seen) = run_cmd(&["$J=G91 X1000", "$I"], CmdConfig::default());
    let e = result.unwrap_err();
    assert!(
        e.contains("ALARM:2") && e.contains("excede el recorrido"),
        "{e}"
    );
    assert_eq!(seen, ["$J=G91X1000"]);
}

#[test]
fn cmd_home_waits_longer_than_other_commands() {
    use std::time::Duration;
    use tlaku_cli::link::CmdConfig;
    // La máquina tarda 400 ms en responder `ok` a `$H`.
    let short = CmdConfig {
        timeout: Duration::from_millis(100),
        home_timeout: Duration::from_secs(5),
        ..CmdConfig::default()
    };
    let (result, shown, _) = run_cmd(&["$H"], short);
    result.unwrap();
    assert!(shown.contains("> $H") && shown.contains("ok"), "{shown}");
    // Un comando normal con el mismo plazo corto vence; el mensaje lo dice.
    let (host, dev) = tlaku_grbl::serial::memory_pair();
    let device = cmd_device(dev, |_| (400, "ok\r\n".into()));
    let mut host = host;
    let cmds = [tlaku_cli::link::parse_command("$G", 127).unwrap()];
    let e = tlaku_cli::link::run_commands(&mut host, &cmds, &short, &mut Vec::new()).unwrap_err();
    assert!(e.contains("sin respuesta a '$G'"), "{e}");
    drop(host);
    device.join().unwrap();
    // Los plazos por defecto: 60 s para el homing.
    assert_eq!(CmdConfig::default().home_timeout, Duration::from_secs(60));
}

#[test]
fn cmd_warns_that_the_laser_stays_on() {
    use tlaku_cli::link::CmdConfig;
    let (result, shown, seen) = run_cmd(&["M3 S100", "m5"], CmdConfig::default());
    result.unwrap();
    assert!(shown.contains("sigue encendido"), "{shown}");
    assert_eq!(seen, ["M3S100", "M5"]);
}

#[test]
fn cmd_rejects_empty_and_too_long_commands() {
    assert_fails(
        &run(&["cmd", "--port", "/dev/x", "; nada"]),
        "ningún comando",
    );
    let long = format!("G1 X{}", "1".repeat(200));
    assert_fails(&run(&["cmd", "--port", "/dev/x", &long]), "no cabe");
    assert_fails(&run(&["cmd", "--port", "/dev/x"]), "COMANDO");
}

#[test]
fn send_alarm_suggests_the_exact_cmd_invocation() {
    let (result, _, _) = send_to_state("<Alarm|MPos:0.000,0.000,0.000|FS:0,0>", Some("$22=1\r\n"));
    let e = result.unwrap_err();
    assert!(e.contains("tlaku cmd --port /dev/cu.test '$H'"), "{e}");
    assert!(e.contains("tlaku cmd --port /dev/cu.test '$X'"), "{e}");
}

#[test]
fn ruida_export_rd_writes_a_decodable_swizzled_file() {
    let dir = workdir("ruida_export_rd");
    let rd = dir.join("out.rd");
    let o = run(&[
        "ruida",
        "export-rd",
        &fixture("design.svg"),
        rd.to_str().unwrap(),
        "--mode",
        "line",
    ]);
    assert!(o.status.success(), "{}", String::from_utf8_lossy(&o.stderr));
    let wire = std::fs::read(&rd).unwrap();
    // El archivo está swizzleado: el último byte es D7 -> 0x60.
    assert_eq!(*wire.last().unwrap(), 0x60);
    let plain = tlaku_ruida::rd::from_swizzled(&wire, tlaku_ruida::swizzle::Magic::default());
    let cmds = tlaku_ruida::cmd::decode_stream(&plain).unwrap();
    assert_eq!(cmds.last(), Some(&tlaku_ruida::cmd::Command::EndOfFile));
    // Dos colores = dos capas.
    assert!(cmds.contains(&tlaku_ruida::cmd::Command::MaxLayer(1)));
}

#[test]
fn ruida_export_rd_rejects_variable_power_fill_with_clear_error() {
    let dir = workdir("ruida_export_rd_fill");
    let rd = dir.join("out.rd");
    let o = run(&[
        "ruida",
        "export-rd",
        &fixture("design.svg"),
        rd.to_str().unwrap(),
        "--mode",
        "fill",
    ]);
    assert!(!o.status.success());
    assert!(
        String::from_utf8_lossy(&o.stderr).contains("potencia"),
        "{}",
        String::from_utf8_lossy(&o.stderr)
    );
    assert!(!rd.exists());
}
