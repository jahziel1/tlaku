//! Pruebas del sondeo y del magic.

use super::*;

#[test]
fn probe_reads_card_id_and_bed_and_only_sends_reads() {
    let (mut s, link, emu) = mem_session(EmuConfig::default(), fast_cfg());
    let info = s.probe().unwrap();
    assert_eq!(info.card_id, 0x6510_6510);
    assert_eq!(info.model, Some("RDC6442S"));
    assert_eq!(info.bed_mm, Some([600.0, 400.0]));
    assert!(!info.magic_changed);
    // Solo lecturas: ningún D8, D9, DA 01...
    let e = emu.lock().unwrap();
    let reads = e.read_requests().unwrap();
    assert_eq!(reads, [0x2FE, 0x026, 0x036]);
    assert!(
        e.commands()
            .unwrap()
            .iter()
            .all(|c| matches!(c, Command::MemRead { .. }))
    );
    assert_eq!(link.sent().len(), 3);
}

#[test]
fn probe_detects_a_different_magic_from_the_ack_byte() {
    for m in [0x11u8, 0x38, 0x77, 0xA1] {
        let cfg = EmuConfig {
            packets: PacketConfig {
                magic: Magic(m),
                ..Default::default()
            },
            ..Default::default()
        };
        let (mut s, _l, _e) = mem_session(cfg, fast_cfg());
        let info = s.probe().unwrap();
        assert_eq!(info.magic, Magic(m));
        assert!(info.magic_changed, "{m:#x}");
        assert_eq!(s.magic(), Magic(m));
        assert_eq!(info.card_id, 0x6510_6510);
    }
}

#[test]
fn probe_times_out_when_nobody_answers() {
    let link = MemoryLink::new();
    let mut s = RuidaSession::new(
        link.clone(),
        SessionConfig {
            probe_all_magics: false,
            ..fast_cfg()
        },
    );
    assert!(matches!(s.probe(), Err(SessionError::Timeout { .. })));
    // 1 intento + 2 reintentos, siempre la misma consulta.
    assert_eq!(link.sent().len(), 3);
}

#[test]
fn probe_without_answer_tries_the_other_magics_once_each() {
    let link = MemoryLink::new();
    let mut s = RuidaSession::new(link.clone(), fast_cfg());
    assert!(matches!(s.probe(), Err(SessionError::Timeout { .. })));
    // 3 intentos con el magic configurado y uno con cada uno de los demás conocidos.
    assert_eq!(link.sent().len(), 3 + probe_candidates().len() - 1);
    assert_eq!(
        s.magic(),
        Magic::default(),
        "el magic configurado no cambia"
    );
}

#[test]
fn a_wrong_magic_is_reported_clearly_not_misread() {
    let emu_cfg = EmuConfig {
        packets: PacketConfig {
            magic: Magic(0x11),
            ..Default::default()
        },
        ..Default::default()
    };
    let (mut s, _l, _e) = mem_session(
        emu_cfg,
        SessionConfig {
            require_idle: false,
            ..fast_cfg()
        },
    );
    // Sin sondear: el magic configurado (0x88) no es el de la máquina.
    let e = s
        .send_job(
            &square_job(),
            &SendMode::Run,
            &AtomicBool::new(false),
            &mut no_progress(),
        )
        .unwrap_err();
    assert!(
        matches!(e.cause, SessionError::Protocol(ref m) if m.contains("magic")),
        "{e}"
    );
}

/// Si el magic del sondeo no es el de la máquina, ésta interpreta los 4 bytes de la consulta
/// como basura. Esta prueba comprueba que, para todo magic de la máquina (0..=255) y todo
/// magic de los que sondeamos, esa basura nunca forma un comando completo que mueva, corte,
/// inicie o controle (un comando incompleto se descarta con el siguiente byte de opcode).
#[test]
fn probing_with_the_wrong_magic_never_forms_a_dangerous_command() {
    let q = cmd::encode_stream(&[Command::MemRead {
        addr: addr::CARD_ID,
    }])
    .unwrap();
    let mut checked = 0;
    for sender in probe_candidates() {
        let wire = sender.swizzle_all(&q);
        for machine in 0..=255u8 {
            if Magic(machine) == sender {
                continue;
            }
            let garbage = Magic(machine).unswizzle_all(&wire);
            let start = garbage
                .iter()
                .position(|&b| b >= 0x80)
                .unwrap_or(garbage.len());
            for r in cmd::command_ranges(&garbage[start..]).unwrap() {
                let Ok((c, _)) = cmd::decode_one(&garbage[start..][r.clone()]) else {
                    continue; // incompleto
                };
                let dangerous = matches!(
                    c,
                    Command::MoveAbs { .. }
                        | Command::CutAbs { .. }
                        | Command::MoveRel { .. }
                        | Command::CutRel { .. }
                        | Command::MoveRelX(_)
                        | Command::MoveRelY(_)
                        | Command::CutRelX(_)
                        | Command::CutRelY(_)
                        | Command::RapidX { .. }
                        | Command::RapidY { .. }
                        | Command::RapidXY { .. }
                        | Command::Process(_)
                ) || matches!(&c, Command::Unknown { op: 0xA5, data }
                        if data.len() == 2 && (data[0] == 0x50 || data[0] == 0x51));
                assert!(
                    !dangerous,
                    "sondeo {sender:?} contra máquina {machine:#x}: {garbage:02x?} -> {c}"
                );
                checked += 1;
            }
        }
    }
    assert!(checked > 100, "la prueba debe examinar comandos completos");
}
