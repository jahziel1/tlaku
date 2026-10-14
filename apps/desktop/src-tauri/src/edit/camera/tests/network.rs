//! Cámaras de red: el proxy HTTP contra un servidor de prueba local (sin cámara real).

use super::*;
use std::io::{Read, Write};
use std::net::{Ipv4Addr, Ipv6Addr, TcpListener};
use std::sync::{Arc, Mutex};
use std::thread;

/// JPEG de verdad (pequeño) generado con el codificador del crate `image`.
fn jpeg(shade: u8) -> Vec<u8> {
    let img = image::RgbImage::from_pixel(16, 12, image::Rgb([shade, 120, 200]));
    let mut out = Cursor::new(Vec::new());
    img.write_to(&mut out, ImageFormat::Jpeg).unwrap();
    out.into_inner()
}

fn png() -> Vec<u8> {
    let img = image::RgbImage::from_pixel(8, 8, image::Rgb([10, 20, 30]));
    let mut out = Cursor::new(Vec::new());
    img.write_to(&mut out, ImageFormat::Png).unwrap();
    out.into_inner()
}

/// Servidor HTTP de una sola conexión: guarda la petición recibida y responde con `reply`.
struct Server {
    url: String,
    request: Arc<Mutex<String>>,
    handle: thread::JoinHandle<()>,
}

impl Server {
    fn start(reply: Vec<u8>) -> Self {
        let listener = TcpListener::bind((Ipv4Addr::LOCALHOST, 0)).unwrap();
        let port = listener.local_addr().unwrap().port();
        let request = Arc::new(Mutex::new(String::new()));
        let seen = request.clone();
        let handle = thread::spawn(move || {
            let (mut sock, _) = listener.accept().unwrap();
            let mut got = Vec::new();
            let mut buf = [0u8; 1024];
            while !got.windows(4).any(|w| w == b"\r\n\r\n") {
                let n = sock.read(&mut buf).unwrap();
                if n == 0 {
                    break;
                }
                got.extend_from_slice(&buf[..n]);
            }
            *seen.lock().unwrap() = String::from_utf8_lossy(&got).into_owned();
            let _ = sock.write_all(&reply);
            let _ = sock.flush();
        });
        Self {
            url: format!("127.0.0.1:{port}"),
            request,
            handle,
        }
    }

    fn finish(self) -> String {
        self.handle.join().unwrap();
        self.request.lock().unwrap().clone()
    }
}

fn response(status: &str, headers: &[&str], body: &[u8]) -> Vec<u8> {
    let mut out = format!("HTTP/1.1 {status}\r\n");
    for h in headers {
        out.push_str(h);
        out.push_str("\r\n");
    }
    out.push_str("\r\n");
    let mut bytes = out.into_bytes();
    bytes.extend_from_slice(body);
    bytes
}

fn decode(frame: &NetworkFrame) -> Vec<u8> {
    STANDARD.decode(&frame.data_base64).unwrap()
}

#[test]
fn un_snapshot_con_largo_se_entrega_completo() {
    let data = jpeg(40);
    let len = format!("Content-Length: {}", data.len());
    let srv = Server::start(response(
        "200 OK",
        &["Content-Type: image/jpeg", &len],
        &data,
    ));
    let frame = grab_network_frame(&format!("http://{}/snapshot.jpg?ch=1", srv.url)).unwrap();
    let req = srv.finish();
    assert!(
        req.starts_with("GET /snapshot.jpg?ch=1 HTTP/1.1\r\n"),
        "{req}"
    );
    assert!(!req.to_lowercase().contains("authorization"));
    assert_eq!(frame.mime, "image/jpeg");
    assert_eq!(decode(&frame), data);
    // Lo entregado se decodifica como imagen.
    let img = image::load_from_memory(&decode(&frame)).unwrap();
    assert_eq!((img.width(), img.height()), (16, 12));
}

#[test]
fn el_tipo_sale_de_los_bytes_no_del_encabezado() {
    let data = png();
    let len = format!("Content-Length: {}", data.len());
    let srv = Server::start(response(
        "200 OK",
        &["Content-Type: application/octet-stream", &len],
        &data,
    ));
    let frame = grab_network_frame(&format!("http://{}/", srv.url)).unwrap();
    srv.finish();
    assert_eq!(frame.mime, "image/png");
    assert_eq!(decode(&frame), data);
}

#[test]
fn un_snapshot_por_fragmentos_se_reensambla() {
    let data = jpeg(90);
    let (a, b) = data.split_at(data.len() / 2);
    let mut body = Vec::new();
    for part in [a, b] {
        body.extend_from_slice(format!("{:x};ext=1\r\n", part.len()).as_bytes());
        body.extend_from_slice(part);
        body.extend_from_slice(b"\r\n");
    }
    body.extend_from_slice(b"0\r\n\r\n");
    let srv = Server::start(response(
        "200 OK",
        &["Content-Type: image/jpeg", "Transfer-Encoding: chunked"],
        &body,
    ));
    let frame = grab_network_frame(&format!("http://{}/cam", srv.url)).unwrap();
    srv.finish();
    assert_eq!(decode(&frame), data);
}

#[test]
fn mjpeg_entrega_el_primer_cuadro_completo() {
    let (f1, f2) = (jpeg(30), jpeg(200));
    let mut body = Vec::new();
    for f in [&f1, &f2] {
        body.extend_from_slice(
            format!(
                "--frame\r\nContent-Type: image/jpeg\r\nContent-Length: {}\r\n\r\n",
                f.len()
            )
            .as_bytes(),
        );
        body.extend_from_slice(f);
        body.extend_from_slice(b"\r\n");
    }
    let srv = Server::start(response(
        "200 OK",
        &["Content-Type: multipart/x-mixed-replace; boundary=frame"],
        &body,
    ));
    let frame = grab_network_frame(&format!("http://{}/video.mjpg", srv.url)).unwrap();
    srv.finish();
    assert_eq!(frame.mime, "image/jpeg");
    assert_eq!(decode(&frame), f1, "debe ser el primero, no el segundo");
}

#[test]
fn mjpeg_sin_largo_en_la_parte_corta_en_el_fin_de_imagen() {
    let (f1, f2) = (jpeg(10), jpeg(250));
    let mut body = b"--b\r\nContent-Type: image/jpeg\r\n\r\n".to_vec();
    body.extend_from_slice(&f1);
    body.extend_from_slice(b"\r\n--b\r\nContent-Type: image/jpeg\r\n\r\n");
    body.extend_from_slice(&f2);
    let srv = Server::start(response(
        "200 OK",
        &["Content-Type: multipart/x-mixed-replace;boundary=b"],
        &body,
    ));
    let frame = grab_network_frame(&format!("http://{}/", srv.url)).unwrap();
    srv.finish();
    assert_eq!(decode(&frame), f1);
}

#[test]
fn las_credenciales_de_la_url_salen_como_basic_y_no_en_la_ruta() {
    let data = jpeg(77);
    let len = format!("Content-Length: {}", data.len());
    let srv = Server::start(response(
        "200 OK",
        &["Content-Type: image/jpeg", &len],
        &data,
    ));
    grab_network_frame(&format!("http://ana:s%40creta@{}/snap", srv.url)).unwrap();
    let req = srv.finish();
    let expected = format!("Authorization: Basic {}", STANDARD.encode("ana:s@creta"));
    assert!(req.contains(&expected), "{req}");
    assert!(req.starts_with("GET /snap HTTP/1.1"), "{req}");
    assert!(!req.contains("s%40creta") && !req.contains("ana:"), "{req}");
}

#[test]
fn un_rechazo_de_clave_no_repite_la_clave_en_el_error() {
    let srv = Server::start(response(
        "401 Unauthorized",
        &["WWW-Authenticate: Basic realm=\"cam\"", "Content-Length: 0"],
        b"",
    ));
    let err = grab_network_frame(&format!("http://ana:topsecret@{}/", srv.url)).unwrap_err();
    srv.finish();
    assert!(err.contains("rechazó"), "{err}");
    assert!(!err.contains("topsecret"), "{err}");
}

#[test]
fn sin_clave_el_401_explica_que_hay_que_ponerla() {
    let srv = Server::start(response(
        "401 Unauthorized",
        &["WWW-Authenticate: Basic realm=\"cam\"", "Content-Length: 0"],
        b"",
    ));
    let err = grab_network_frame(&format!("http://{}/", srv.url)).unwrap_err();
    srv.finish();
    assert!(err.contains("usuario y clave"), "{err}");
}

#[test]
fn digest_se_avisa_como_no_admitido() {
    let srv = Server::start(response(
        "401 Unauthorized",
        &[
            "WWW-Authenticate: Digest realm=\"cam\", nonce=\"x\"",
            "Content-Length: 0",
        ],
        b"",
    ));
    let err = grab_network_frame(&format!("http://u:p@{}/", srv.url)).unwrap_err();
    srv.finish();
    assert!(err.contains("Digest"), "{err}");
}

#[test]
fn errores_http_y_respuestas_que_no_son_imagen() {
    let srv = Server::start(response("404 Not Found", &["Content-Length: 0"], b""));
    let err = grab_network_frame(&format!("http://{}/x", srv.url)).unwrap_err();
    srv.finish();
    assert!(err.contains("404"), "{err}");

    let html = b"<html>hola</html>";
    let len = format!("Content-Length: {}", html.len());
    let srv = Server::start(response("200 OK", &["Content-Type: text/html", &len], html));
    let err = grab_network_frame(&format!("http://{}/", srv.url)).unwrap_err();
    srv.finish();
    assert!(err.contains("no es una imagen"), "{err}");

    let srv = Server::start(response(
        "302 Found",
        &["Location: http://otra/", "Content-Length: 0"],
        b"",
    ));
    let err = grab_network_frame(&format!("http://{}/", srv.url)).unwrap_err();
    srv.finish();
    assert!(err.contains("redirige"), "{err}");
}

#[test]
fn un_cuadro_enorme_se_rechaza() {
    // Cabecera de 9 MiB declarados; el servidor manda solo el principio y cierra.
    let big = MAX_FRAME_BYTES + 10;
    let len = format!("Content-Length: {big}");
    let mut body = vec![0xFF, 0xD8, 0xFF];
    body.resize(MAX_FRAME_BYTES + 10, 0);
    let srv = Server::start(response(
        "200 OK",
        &["Content-Type: image/png", &len],
        &body,
    ));
    let err = grab_network_frame(&format!("http://{}/", srv.url)).unwrap_err();
    srv.finish();
    assert!(err.contains("demasiado grande"), "{err}");
}

#[test]
fn solo_se_acepta_la_red_local_y_http() {
    // Direcciones públicas: se rechazan sin intentar conectar.
    let err = grab_network_frame("http://8.8.8.8/snap.jpg").unwrap_err();
    assert!(err.contains("red local"), "{err}");
    let err = grab_network_frame("http://1.1.1.1:8080/").unwrap_err();
    assert!(err.contains("red local"), "{err}");
    assert!(
        grab_network_frame("https://192.168.1.50/")
            .unwrap_err()
            .contains("http://")
    );
    assert!(grab_network_frame("rtsp://192.168.1.50/").is_err());
    assert!(grab_network_frame("").is_err());
    assert!(grab_network_frame("http://").is_err());
    assert!(is_local_ip(Ipv4Addr::new(192, 168, 1, 10).into()));
    assert!(is_local_ip(Ipv4Addr::new(10, 0, 0, 5).into()));
    assert!(is_local_ip(Ipv4Addr::new(172, 16, 0, 1).into()));
    assert!(is_local_ip(Ipv4Addr::new(169, 254, 3, 3).into()));
    assert!(is_local_ip(Ipv4Addr::LOCALHOST.into()));
    assert!(!is_local_ip(Ipv4Addr::new(172, 32, 0, 1).into()));
    assert!(!is_local_ip(Ipv4Addr::new(93, 184, 216, 34).into()));
    assert!(is_local_ip(Ipv6Addr::LOCALHOST.into()));
    assert!(is_local_ip("fd12::1".parse::<Ipv6Addr>().unwrap().into()));
    assert!(is_local_ip("fe80::1".parse::<Ipv6Addr>().unwrap().into()));
    assert!(!is_local_ip(
        "2606:4700::1111".parse::<Ipv6Addr>().unwrap().into()
    ));
}

#[test]
fn lee_las_partes_de_la_url() {
    let u = parse_http_url("http://user:p%3Aw@cam.local:8081/a/b.jpg?x=1#frag").unwrap();
    assert_eq!(u.host, "cam.local");
    assert_eq!(u.port, 8081);
    assert_eq!(u.path, "/a/b.jpg?x=1");
    assert_eq!(u.credentials.as_deref(), Some("user:p:w"));
    let u = parse_http_url("HTTP://192.168.0.9").unwrap();
    assert_eq!((u.port, u.path.as_str(), u.credentials), (80, "/", None));
    let u = parse_http_url("http://192.168.0.9?snap=1").unwrap();
    assert_eq!(u.path, "/?snap=1");
    let u = parse_http_url("http://[fe80::1]:81/s").unwrap();
    assert_eq!((u.host.as_str(), u.port), ("fe80::1", 81));
    assert!(parse_http_url("http://host:99999/").is_err());
}

#[test]
fn sin_servidor_el_error_no_cuelga_ni_filtra_la_clave() {
    // Un puerto cerrado en la máquina local: se rechaza la conexión enseguida.
    let port = {
        let l = TcpListener::bind((Ipv4Addr::LOCALHOST, 0)).unwrap();
        l.local_addr().unwrap().port()
    };
    let err = grab_network_frame(&format!("http://ana:topsecret@127.0.0.1:{port}/")).unwrap_err();
    assert!(err.contains("No se pudo conectar"), "{err}");
    assert!(!err.contains("topsecret"), "{err}");
}

#[test]
fn extract_jpeg_espera_a_tener_el_cuadro_entero() {
    let f = jpeg(55);
    assert!(extract_jpeg(&f[..f.len() - 1]).is_none());
    assert_eq!(extract_jpeg(&f).unwrap(), f);
    let mut with_head = b"--x\r\nContent-Length: ".to_vec();
    with_head.extend_from_slice(format!("{}\r\n\r\n", f.len()).as_bytes());
    with_head.extend_from_slice(&f[..f.len() - 3]);
    assert!(extract_jpeg(&with_head).is_none());
    with_head.extend_from_slice(&f[f.len() - 3..]);
    assert_eq!(extract_jpeg(&with_head).unwrap(), f);
}

#[test]
fn las_preferencias_de_red_y_las_configuraciones_se_validan_y_persisten() {
    let tmp = TempDir::new();
    let mut s = CameraSetup::default();
    s.prefs.source = CameraSource::Network;
    s.prefs.network_url = Some("  http://u:p@192.168.1.20/snap.jpg ".into());
    s.prefs.dpm = Some(f64::NAN);
    s.prefs.offset_x_mm = 5000.0;
    s.prefs.offset_y_mm = f64::INFINITY;
    let preset = |name: &str| CameraPreset {
        name: name.into(),
        source: CameraSource::Network,
        device_id: None,
        device_label: None,
        width: Some(0),
        height: Some(480),
        network_url: Some("http://192.168.1.20/mjpg".into()),
        dpm: Some(8.5),
        offset_x_mm: 1.5,
        offset_y_mm: -2.0,
        grayscale: true,
    };
    s.presets = vec![preset("Taller"), preset(" taller "), preset("Garage")];
    let saved = store_setup(&tmp.0, s).unwrap();
    assert_eq!(saved.prefs.source, CameraSource::Network);
    assert_eq!(
        saved.prefs.network_url.as_deref(),
        Some("http://u:p@192.168.1.20/snap.jpg")
    );
    assert_eq!(saved.prefs.dpm, None);
    assert_eq!(
        (saved.prefs.offset_x_mm, saved.prefs.offset_y_mm),
        (1000.0, 0.0)
    );
    // El nombre repetido (sin distinguir mayúsculas) deja una sola; el ancho 0 no vale.
    assert_eq!(saved.presets.len(), 2);
    assert_eq!(saved.presets[0].name, "taller");
    assert_eq!(saved.presets[0].width, None);
    assert_eq!(load_setup(&tmp.0).unwrap(), saved);

    let mut bad = CameraSetup::default();
    bad.prefs.network_url = Some("ftp://192.168.1.20/".into());
    assert!(store_setup(&tmp.0, bad).is_err());
    let bad = CameraSetup {
        presets: vec![preset("   ")],
        ..CameraSetup::default()
    };
    assert!(store_setup(&tmp.0, bad).is_err());
    let bad = CameraSetup {
        presets: (0..40).map(|i| preset(&format!("p{i}"))).collect(),
        ..CameraSetup::default()
    };
    assert!(store_setup(&tmp.0, bad).is_err());
}
