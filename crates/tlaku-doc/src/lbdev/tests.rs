//! Pruebas del lector de `.lbdev`. Los JSON son **sintéticos**, escritos aquí con las claves
//! observadas en el ejemplo público (perfil «Creality Falcon2» del foro de Creality); no se
//! incluye ningún archivo de terceros.

use super::*;

/// Perfil con la estructura del ejemplo público (valores propios).
const FALCON_LIKE: &str = r#"{
  "DeviceList": [
    {
      "DefaultCutList": [],
      "DefaultToolCutList": [],
      "DisplayName": "Mi láser de prueba",
      "EnableLaser2Offset": false,
      "EnableProcessOffset": false,
      "Height": 415,
      "HomeOnStartup": true,
      "Info": "",
      "Laser2OffsetX": 0,
      "Laser2OffsetY": 0,
      "LastCamera": "cam",
      "MirrorX": false,
      "MirrorY": false,
      "Name": "GRBL",
      "ProcessOffsetX": 0,
      "ProcessOffsetY": 0,
      "ReverseIntervalCompensation": false,
      "Settings": {
        "AirAssistM7": false,
        "BaudRate": 115200,
        "CommPort": "COM9",
        "EnableBoundsCheck": true,
        "EnableDTR": false,
        "EnableGrblJCommand": false,
        "EnableZ": false,
        "EndGCode": "",
        "LaserFire_Enable": true,
        "LaserFire_Power": 3,
        "LastExportExt": "gcode",
        "LastMachineFilePath": "H:/",
        "NegativeZ": false,
        "S_Scale": 1000,
        "Sim_CornerTolerance": 0.01,
        "Sim_MaxAccelX": 3000,
        "Sim_MaxAccelY": 2500,
        "Sim_MaxSpeedX": 500,
        "Sim_MaxSpeedY": 400,
        "Sim_RapidSpeed": 350,
        "StartGCode": "",
        "TransferMode": 0,
        "UserFinishX": 0,
        "UserFinishY": 0
      },
      "Type": "Serial",
      "Width": 400
    }
  ]
}"#;

fn parse(text: &str) -> LbdevImport {
    parse_lbdev(text, &[]).expect("debe importar")
}

fn kinds(i: &LbdevImport) -> Vec<LbdevNoticeKind> {
    i.notices.iter().map(|n| n.kind).collect()
}

fn find(i: &LbdevImport, kind: LbdevNoticeKind) -> Option<&LbdevNotice> {
    i.notices.iter().find(|n| n.kind == kind)
}

#[test]
fn a_public_style_grbl_profile_maps_every_field() {
    let i = parse(FALCON_LIKE);
    assert_eq!(i.devices.len(), 1);
    let d = &i.devices[0];
    assert_eq!(d.name, "Mi láser de prueba");
    assert_eq!(d.id, "mi-laser-de-prueba");
    assert_eq!(d.kind, DeviceKind::Grbl);
    assert_eq!(d.dialect.dynamic_on.as_deref(), Some("M4 S0"));
    assert_eq!(d.bed, [400.0, 415.0]);
    assert_eq!(d.origin, Origin::BottomLeft);
    assert_eq!((d.baud, d.s_max), (115_200, 1000.0));
    assert!(d.home_on_connect && !d.z_axis && !d.sync_streaming);
    assert_eq!(d.connection, Connection::Serial);
    assert_eq!(d.dialect.air_on.as_deref(), Some("M8"));
    // mm/s a mm/min; la aceleración queda en mm/s².
    assert_eq!(d.max_rate, [30_000.0, 24_000.0]);
    assert_eq!(d.rapid_rate, 21_000.0);
    assert_eq!(d.accel, [3000.0, 2500.0]);
    assert!(d.start_gcode.is_empty() && d.end_gcode.is_empty());
}

#[test]
fn simulator_values_warn_and_local_keys_are_silent() {
    let i = parse(FALCON_LIKE);
    assert!(find(&i, LbdevNoticeKind::SimulatorValues).is_some());
    let not = find(&i, LbdevNoticeKind::NotImported).expect("claves sin equivalente");
    // Activas y sin equivalente: se listan; las locales y las apagadas, no.
    assert_eq!(
        not.detail,
        "LaserFire_Enable, LaserFire_Power, Sim_CornerTolerance"
    );
    assert!(!not.detail.contains("CommPort") && !not.detail.contains("LastCamera"));
    assert!(!not.detail.contains("UserFinish") && !not.detail.contains("Laser2"));
    assert_eq!(
        i.notices
            .iter()
            .filter(|n| n.device != "Mi láser de prueba")
            .count(),
        0
    );
}

#[test]
fn nonzero_offsets_and_finish_position_are_reported_as_not_imported() {
    let text = r#"{"DeviceList":[{"DisplayName":"A","Name":"GRBL","Width":300,"Height":200,
        "EnableLaser2Offset":true,"Laser2OffsetX":4.5,"EnableProcessOffset":true,
        "Settings":{"UserFinishX":10,"UserFinishY":0,"EnableBoundsCheck":false}}]}"#;
    let i = parse(text);
    let d = &find(&i, LbdevNoticeKind::NotImported).unwrap().detail;
    for key in [
        "EnableBoundsCheck",
        "EnableLaser2Offset",
        "EnableProcessOffset",
        "Laser2OffsetX",
        "UserFinishX",
    ] {
        assert!(d.contains(key), "{key} en {d}");
    }
    assert!(!d.contains("UserFinishY"));
}

#[test]
fn missing_keys_fall_back_to_the_factory_profile() {
    let i = parse(r#"{"DeviceList":[{"Name":"GRBL"}]}"#);
    let d = &i.devices[0];
    let base = DeviceProfile::for_kind(DeviceKind::Grbl);
    assert_eq!(d.name, "GRBL");
    assert_eq!((d.bed, d.s_max, d.baud), (base.bed, base.s_max, base.baud));
    assert_eq!((d.max_rate, d.accel), (base.max_rate, base.accel));
    assert!(!d.home_on_connect);
    let missing: Vec<_> = i
        .notices
        .iter()
        .filter(|n| n.kind == LbdevNoticeKind::MissingValue)
        .map(|n| n.detail.as_str())
        .collect();
    assert_eq!(missing, ["Width", "Height"]);
    assert!(find(&i, LbdevNoticeKind::SimulatorValues).is_none());
}

#[test]
fn odd_types_are_tolerated_and_bad_values_warn() {
    let text = r#"{"DeviceList":[{"DisplayName":"X","Name":"GRBL","Width":"350","Height":-5,
        "MirrorY":"True","HomeOnStartup":1,
        "Settings":{"BaudRate":"250000","S_Scale":0.5,"Sim_MaxSpeedX":"abc","Sim_MaxSpeedY":200,
                    "TransferMode":1,"EnableZ":"false"}}]}"#;
    let i = parse(text);
    let d = &i.devices[0];
    assert_eq!(d.bed[0], 350.0);
    assert_eq!(d.bed[1], DeviceProfile::default().bed[1]);
    assert_eq!(d.origin, Origin::TopLeft);
    assert!(d.home_on_connect && d.sync_streaming && !d.z_axis);
    assert_eq!(d.baud, 250_000);
    assert_eq!(d.s_max, 1000.0);
    assert_eq!(d.max_rate[0], DeviceProfile::default().max_rate[0]);
    assert_eq!(d.max_rate[1], 12_000.0);
    let bad: Vec<_> = i
        .notices
        .iter()
        .filter(|n| n.kind == LbdevNoticeKind::InvalidValue)
        .map(|n| n.detail.as_str())
        .collect();
    assert_eq!(bad, ["Height: -5", "S_Scale: 0.5", "Sim_MaxSpeedX: abc"]);
    assert_eq!(
        find(&i, LbdevNoticeKind::OriginInferred).unwrap().detail,
        "Y"
    );
}

#[test]
fn origin_follows_the_two_mirror_flags() {
    assert_eq!(origin_from_mirror(false, false), Origin::BottomLeft);
    assert_eq!(origin_from_mirror(true, false), Origin::BottomRight);
    assert_eq!(origin_from_mirror(false, true), Origin::TopLeft);
    assert_eq!(origin_from_mirror(true, true), Origin::TopRight);
}

#[test]
fn invalid_json_is_a_clear_error() {
    let err = parse_lbdev("{ \"DeviceList\": [ ", &[]).unwrap_err();
    assert!(err.contains("JSON"), "{err}");
    assert!(parse_lbdev("", &[]).is_err());
    let err = parse_lbdev("42", &[]).unwrap_err();
    assert!(err.contains("DeviceList"), "{err}");
    let err = parse_lbdev(r#"{"DeviceList":[]}"#, &[]).unwrap_err();
    assert!(err.contains("ningún"), "{err}");
}

#[test]
fn text_pasted_from_a_forum_still_loads() {
    let pasted = "\u{feff}{\n\u{201c}DeviceList\u{201d}: [ { \u{201c}DisplayName\u{201d}: \u{201c}Foro\u{201d}, \
        \u{201c}Name\u{201d}: \u{201c}GRBL\u{201d}, \u{201c}Width\u{201d}: 410, \u{201c}Height\u{201d}: 400, \
        \u{201c}Settings\u{201d}: { \u{201c}S\\_Scale\u{201d}: 255 } } ] }";
    let i = parse(pasted);
    assert_eq!(i.devices[0].bed, [410.0, 400.0]);
    assert_eq!(i.devices[0].s_max, 255.0);
}

#[test]
fn several_devices_get_unique_ids_and_names() {
    let text = r#"{"DeviceList":[
        {"DisplayName":"Taller","Name":"GRBL","Width":300,"Height":300},
        {"DisplayName":"Taller","Name":"GRBL-M3","Width":200,"Height":200,"Settings":{"S_Scale":255}},
        {"DisplayName":"Otro","Name":"Smoothieware"},
        "no soy un dispositivo"
    ]}"#;
    let existing = vec![("taller".to_string(), "Taller".to_string())];
    let i = parse_lbdev(text, &existing).unwrap();
    let names: Vec<_> = i.devices.iter().map(|d| d.name.as_str()).collect();
    let ids: Vec<_> = i.devices.iter().map(|d| d.id.as_str()).collect();
    assert_eq!(names, ["Taller (2)", "Taller (3)", "Otro"]);
    assert_eq!(ids, ["taller-2", "taller-3", "otro"]);
    assert_eq!(i.devices[1].kind, DeviceKind::GrblM3);
    assert_eq!(i.devices[1].s_max, 255.0);
    assert_eq!(i.devices[2].kind, DeviceKind::Smoothie);
    let skipped = find(&i, LbdevNoticeKind::SkippedEntry).unwrap();
    assert_eq!(skipped.detail, "4");
}

#[test]
fn other_envelopes_are_accepted() {
    let single = r#"{"DisplayName":"Suelto","Name":"GRBL","Width":100,"Height":100}"#;
    assert_eq!(parse(single).devices[0].bed, [100.0, 100.0]);
    let list = r#"[{"DisplayName":"Uno","Name":"Marlin"},{"DisplayName":"Dos","Name":"GCode"}]"#;
    let i = parse(list);
    assert_eq!(i.devices[0].kind, DeviceKind::Marlin);
    assert_eq!(i.devices[0].s_max, 255.0);
    assert_eq!(i.devices[1].kind, DeviceKind::CustomGcode);
}

#[test]
fn ruida_uses_udp_and_ignores_gcode_keys() {
    let text = r#"{"DeviceList":[{"DisplayName":"CO2","Name":"Ruida","Type":"Ethernet",
        "Width":600,"Height":400,"MirrorX":true,"MirrorY":true,"IPAddress":" 192.168.1.50 ",
        "Settings":{"BaudRate":9600,"S_Scale":255,"StartGCode":"G90","Sim_MaxSpeedX":300,
                    "Sim_MaxSpeedY":300,"Sim_RapidSpeed":400,"Sim_MaxAccelX":2000,"Sim_MaxAccelY":2000}}]}"#;
    let i = parse(text);
    let d = &i.devices[0];
    assert_eq!(d.kind, DeviceKind::Ruida);
    assert_eq!(d.origin, Origin::TopRight);
    assert_eq!(d.bed, [600.0, 400.0]);
    assert_eq!(
        d.connection,
        Connection::Udp {
            host: "192.168.1.50".into()
        }
    );
    // No toma baudios, S ni G-code de inicio.
    assert_eq!(d.baud, DeviceKind::Ruida.default_baud());
    assert_eq!(d.s_max, DeviceKind::Ruida.default_s_max());
    assert!(d.start_gcode.is_empty());
    assert_eq!(d.max_rate, [18_000.0, 18_000.0]);
    assert_eq!(d.rapid_rate, 24_000.0);
    assert!(find(&i, LbdevNoticeKind::RuidaHost).is_none());
    assert_eq!(
        find(&i, LbdevNoticeKind::OriginInferred).unwrap().detail,
        "X, Y"
    );
    let not = &find(&i, LbdevNoticeKind::NotImported).unwrap().detail;
    assert_eq!(not, "BaudRate, S_Scale, StartGCode");
}

#[test]
fn ruida_without_an_address_asks_for_it() {
    let i = parse(r#"{"DeviceList":[{"DisplayName":"CO2","Name":"ruida","Type":"USB"}]}"#);
    assert_eq!(
        i.devices[0].connection,
        Connection::Udp {
            host: String::new()
        }
    );
    assert!(find(&i, LbdevNoticeKind::RuidaHost).is_some());
    assert_eq!(
        find(&i, LbdevNoticeKind::ConnectionType).unwrap().detail,
        "USB"
    );
}

#[test]
fn unsupported_controllers_are_skipped_never_turned_into_grbl() {
    let text = r#"{"DeviceList":[
        {"DisplayName":"Trocen","Name":"Trocen"},
        {"DisplayName":"TW","Name":"TopWisdom"},
        {"DisplayName":"Bueno","Name":"GRBL"},
        {"DisplayName":"Raro","Name":"Foo 9000"},
        {"DisplayName":"Sin tipo"}]}"#;
    let i = parse(text);
    assert_eq!(i.devices.len(), 1);
    assert_eq!(i.devices[0].name, "Bueno");
    let unsupported: Vec<_> = i
        .notices
        .iter()
        .filter(|n| n.kind == LbdevNoticeKind::UnsupportedController)
        .map(|n| (n.device.as_str(), n.detail.as_str()))
        .collect();
    assert_eq!(
        unsupported,
        [
            ("Trocen", "Trocen"),
            ("TW", "TopWisdom"),
            ("Raro", "Foo 9000"),
            ("Sin tipo", "?")
        ]
    );
    let err = parse_lbdev(r#"{"DeviceList":[{"Name":"Trocen"}]}"#, &[]).unwrap_err();
    assert!(err.contains("Trocen"), "{err}");
}

#[test]
fn controller_names_ignore_case_and_separators() {
    assert_eq!(kind_from_name("GRBL"), Some(DeviceKind::Grbl));
    assert_eq!(kind_from_name("GRBL-M3"), Some(DeviceKind::GrblM3));
    assert_eq!(kind_from_name("grbl m3"), Some(DeviceKind::GrblM3));
    assert_eq!(kind_from_name("GRBL-LPC"), Some(DeviceKind::GrblLpc));
    assert_eq!(kind_from_name("Smoothieware"), Some(DeviceKind::Smoothie));
    assert_eq!(kind_from_name("Marlin"), Some(DeviceKind::Marlin));
    assert_eq!(kind_from_name("GCode"), Some(DeviceKind::CustomGcode));
    assert_eq!(kind_from_name("Ruida"), Some(DeviceKind::Ruida));
    assert_eq!(kind_from_name("Trocen"), None);
    assert_eq!(kind_from_name("TopWisdom"), None);
    assert_eq!(kind_from_name(""), None);
}

#[test]
fn air_z_scripts_and_network_type_are_handled() {
    let text = r#"{"DeviceList":[{"DisplayName":"Aire","Name":"GRBL","Type":"Ethernet",
        "Settings":{"AirAssistM7":true,"EnableZ":true,"NegativeZ":true,
                    "StartGCode":"G21\r\nG90\r\n","EndGCode":"M5"}}]}"#;
    let i = parse(text);
    let d = &i.devices[0];
    assert_eq!(d.dialect.air_on.as_deref(), Some("M7"));
    assert_eq!(d.dialect.air_off.as_deref(), Some("M9"));
    assert!(d.z_axis);
    assert!(!d.z_invert, "el sentido de Z no se deduce");
    assert_eq!(d.start_gcode, "G21\nG90");
    assert_eq!(d.end_gcode, "M5");
    assert_eq!(d.connection, Connection::Serial);
    assert_eq!(
        find(&i, LbdevNoticeKind::ScriptCode).unwrap().detail,
        "start, end"
    );
    assert!(find(&i, LbdevNoticeKind::ZAxisCheck).is_some());
    assert_eq!(
        find(&i, LbdevNoticeKind::ConnectionType).unwrap().detail,
        "Ethernet"
    );
    assert!(
        find(&i, LbdevNoticeKind::NotImported)
            .unwrap()
            .detail
            .contains("NegativeZ")
    );
}

#[test]
fn imported_profiles_survive_a_json_roundtrip() {
    let i = parse(FALCON_LIKE);
    let back: LbdevImport = serde_json::from_str(&serde_json::to_string(&i).unwrap()).unwrap();
    assert_eq!(back, i);
    assert!(kinds(&i).contains(&LbdevNoticeKind::SimulatorValues));
}
