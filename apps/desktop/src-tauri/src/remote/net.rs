//! Direcciones de la red local: cuáles hay, cuál conviene y qué pares se aceptan.
//!
//! El servidor remoto **nunca** escucha en `0.0.0.0`: se ata a una sola dirección privada de la red
//! local (la que elige el usuario o la recomendada), de modo que una interfaz con IP pública o una
//! VPN no queda expuesta sin que nadie lo pida. Además, cada petición se rechaza si el par que la
//! manda no es una dirección local (ver [`is_local_peer`]).

use serde::Serialize;
use std::net::{IpAddr, Ipv4Addr};

/// Una dirección de la red local de este equipo.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct LocalAddr {
    /// IPv4 en texto (`192.168.1.20`).
    pub ip: String,
    /// Nombre de la interfaz (`en0`, `Wi-Fi`).
    pub name: String,
    /// La que se elige sola si el usuario no escoge ninguna.
    pub recommended: bool,
}

/// ¿Es una dirección que solo existe dentro de una red local? (privada RFC 1918 o enlace local).
pub fn is_lan_v4(ip: Ipv4Addr) -> bool {
    ip.is_private() || ip.is_link_local()
}

/// ¿Puede venir una petición de este par? Solo la red local y el propio equipo; el resto (una IP
/// pública que llegó por un reenvío de puertos del router, por ejemplo) se rechaza.
pub fn is_local_peer(ip: IpAddr) -> bool {
    match ip {
        IpAddr::V4(v4) => v4.is_loopback() || is_lan_v4(v4),
        IpAddr::V6(v6) => {
            if let Some(v4) = v6.to_ipv4_mapped() {
                return v4.is_loopback() || is_lan_v4(v4);
            }
            let first = v6.segments()[0];
            // ::1, fe80::/10 (enlace local) y fc00::/7 (única local).
            v6.is_loopback() || (first & 0xffc0) == 0xfe80 || (first & 0xfe00) == 0xfc00
        }
    }
}

/// Orden de preferencia: las redes domésticas típicas primero.
fn rank(ip: Ipv4Addr) -> u8 {
    let o = ip.octets();
    match (o[0], o[1]) {
        (192, 168) => 0,
        (10, _) => 1,
        (172, 16..=31) => 2,
        _ => 3,
    }
}

/// Interfaces que casi nunca son la red a la que se conecta el celular (contenedores, máquinas
/// virtuales, túneles).
fn looks_virtual(name: &str) -> bool {
    let n = name.to_ascii_lowercase();
    const PREFIXES: [&str; 8] = ["docker", "veth", "utun", "awdl", "llw", "tun", "tap", "br-"];
    const PARTS: [&str; 4] = ["vbox", "vmnet", "vmware", "vethernet"];
    PREFIXES.iter().any(|p| n.starts_with(p)) || PARTS.iter().any(|p| n.contains(p))
}

/// Ordena y marca la recomendada. Separado de [`local_addrs`] para probarlo sin red.
pub fn rank_addrs(mut found: Vec<(String, Ipv4Addr)>) -> Vec<LocalAddr> {
    found.retain(|(_, ip)| is_lan_v4(*ip));
    found.sort_by_key(|(name, ip)| (looks_virtual(name), rank(*ip), *ip));
    found.dedup_by_key(|(_, ip)| *ip);
    found
        .into_iter()
        .enumerate()
        .map(|(i, (name, ip))| LocalAddr {
            ip: ip.to_string(),
            name,
            recommended: i == 0,
        })
        .collect()
}

/// Direcciones IPv4 de la red local de este equipo (sin loopback), la recomendada primero.
pub fn local_addrs() -> Vec<LocalAddr> {
    let found = if_addrs::get_if_addrs()
        .map(|list| {
            list.into_iter()
                .filter(|i| !i.is_loopback())
                .filter_map(|i| match i.ip() {
                    IpAddr::V4(v4) => Some((i.name, v4)),
                    IpAddr::V6(_) => None,
                })
                .collect()
        })
        .unwrap_or_default();
    rank_addrs(found)
}

/// Dirección donde escuchar: `wanted` si está entre las de este equipo; si no (o sin preferencia),
/// la recomendada. Falla si el equipo no está en ninguna red local.
pub fn pick_addr(wanted: Option<&str>, available: &[LocalAddr]) -> Result<Ipv4Addr, String> {
    let chosen = wanted
        .and_then(|w| available.iter().find(|a| a.ip == w.trim()))
        .or_else(|| available.first());
    chosen
        .and_then(|a| a.ip.parse().ok())
        .ok_or_else(|| tlaku_i18n::tr!("desktop.remote.no_network"))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn v4(s: &str) -> Ipv4Addr {
        s.parse().unwrap()
    }

    #[test]
    fn only_private_and_link_local_ranges_are_lan() {
        for ok in [
            "192.168.0.5",
            "10.1.2.3",
            "172.16.0.1",
            "172.31.255.1",
            "169.254.1.1",
        ] {
            assert!(is_lan_v4(v4(ok)), "{ok}");
        }
        for bad in [
            "8.8.8.8",
            "172.32.0.1",
            "100.64.0.1",
            "203.0.113.9",
            "0.0.0.0",
        ] {
            assert!(!is_lan_v4(v4(bad)), "{bad}");
        }
    }

    #[test]
    fn peers_must_be_local() {
        for ok in [
            "127.0.0.1",
            "192.168.1.9",
            "::1",
            "fe80::1",
            "fd12:3456::1",
            "::ffff:192.168.1.4",
        ] {
            assert!(is_local_peer(ok.parse().unwrap()), "{ok}");
        }
        for bad in ["8.8.8.8", "2001:db8::1", "::ffff:8.8.8.8", "93.184.216.34"] {
            assert!(!is_local_peer(bad.parse().unwrap()), "{bad}");
        }
    }

    #[test]
    fn the_home_network_wins_over_virtual_interfaces() {
        let list = rank_addrs(vec![
            ("docker0".into(), v4("172.17.0.1")),
            ("en0".into(), v4("192.168.1.20")),
            ("utun3".into(), v4("10.8.0.2")),
            ("en1".into(), v4("8.8.4.4")),
            ("en0".into(), v4("192.168.1.20")),
        ]);
        let ips: Vec<_> = list.iter().map(|a| a.ip.as_str()).collect();
        // La pública se descarta, la repetida se junta y las virtuales van al final.
        assert_eq!(ips, ["192.168.1.20", "10.8.0.2", "172.17.0.1"]);
        assert!(list[0].recommended && !list[1].recommended);
    }

    #[test]
    fn picking_prefers_the_request_and_falls_back() {
        let list = rank_addrs(vec![
            ("en0".into(), v4("192.168.1.20")),
            ("en1".into(), v4("10.0.0.7")),
        ]);
        assert_eq!(pick_addr(Some("10.0.0.7"), &list).unwrap(), v4("10.0.0.7"));
        assert_eq!(
            pick_addr(Some("1.2.3.4"), &list).unwrap(),
            v4("192.168.1.20")
        );
        assert_eq!(pick_addr(None, &list).unwrap(), v4("192.168.1.20"));
        assert!(pick_addr(None, &[]).unwrap_err().contains("red local"));
    }
}
