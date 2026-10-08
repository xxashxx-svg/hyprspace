// Where a phone can reach this computer: its addresses on the local network and on Tailscale,
// the ones a phone is likeliest to reach first. Virtual adapters (WSL, Hyper-V, Docker, VMs)
// are left out, since a phone can never reach them.

use std::net::{IpAddr, Ipv4Addr};

/// Tailscale gives each machine an address in 100.64.0.0/10.
pub fn tailscale(ip: &IpAddr) -> bool {
    match ip {
        IpAddr::V4(v4) => {
            let o = v4.octets();
            o[0] == 100 && (64..128).contains(&o[1])
        }
        IpAddr::V6(v6) => v6.segments()[..3] == [0xfd7a, 0x115c, 0xa1e0],
    }
}

fn virtual_adapter(name: &str) -> bool {
    let n = name.to_ascii_lowercase();
    [
        "vethernet",
        "wsl",
        "hyper-v",
        "docker",
        "virtualbox",
        "vmware",
        "vmnet",
        "vboxnet",
        "bridge",
        "utun",
        "awdl",
        "llw",
    ]
    .iter()
    .any(|v| n.contains(v))
}

/// This computer's addresses a phone could use, local network first, then Tailscale.
pub fn addresses(only_tailscale: bool) -> Vec<IpAddr> {
    if let Some(ip) = pinned() {
        return vec![ip];
    }
    let Ok(ifaces) = if_addrs::get_if_addrs() else {
        return Vec::new();
    };
    let mut lan = Vec::new();
    let mut ts = Vec::new();
    for i in ifaces {
        let ip = i.ip();
        if ip.is_loopback() || ip.is_unspecified() {
            continue;
        }
        if tailscale(&ip) {
            ts.push(ip);
            continue;
        }
        if only_tailscale || virtual_adapter(&i.name) {
            continue;
        }
        if let IpAddr::V4(v4) = ip
            && v4.is_private()
            && !v4.is_link_local()
        {
            lan.push(ip);
        }
    }
    // a Tailscale IPv6 address only adds length to the QR code; the IPv4 one does the job
    if ts.iter().any(|ip| ip.is_ipv4()) {
        ts.retain(|ip| ip.is_ipv4());
    }
    lan.sort();
    lan.dedup();
    ts.sort();
    ts.dedup();
    lan.into_iter().chain(ts).take(4).collect()
}

fn pinned() -> Option<IpAddr> {
    std::env::var("HYPRSPACE_PHONE_BIND").ok()?.parse().ok()
}

/// What to bind: every address, or each Tailscale one. `HYPRSPACE_PHONE_BIND` pins one address
/// instead, so tests and a test copy can stay on loopback, where no firewall asks about them.
pub fn binds(only_tailscale: bool) -> Vec<IpAddr> {
    if let Some(ip) = pinned() {
        return vec![ip];
    }
    if only_tailscale {
        addresses(true)
    } else {
        vec![IpAddr::V4(Ipv4Addr::UNSPECIFIED)]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tailscale_addresses_are_told_apart() {
        assert!(tailscale(&"100.101.5.6".parse().unwrap()));
        assert!(!tailscale(&"100.20.5.6".parse().unwrap()));
        assert!(!tailscale(&"192.168.1.4".parse().unwrap()));
        assert!(tailscale(&"fd7a:115c:a1e0::1".parse().unwrap()));
        assert!(virtual_adapter("vEthernet (WSL (Hyper-V firewall))"));
        assert!(!virtual_adapter("Wi-Fi"));
    }
}
