use std::net::IpAddr;

use super::{Invite, RaceClient};
use crate::network;

/**
 * Stands for the host of the invite when the server runs on this computer
 * and its address on the local network is unknown.
 */
const LAN_ADDRESS_PLACEHOLDER: &str = "<your LAN address>";
const LOCAL_SERVER_NOTE: &str = "start the server with --host 0.0.0.0 for teammates to reach it";

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct SharedServer {
    address: String,
    note: Option<&'static str>,
}

impl SharedServer {
    /**
     * How teammates reach `server`. An address only this computer reaches,
     * such as `ws://127.0.0.1:8080`, would send them to their own computer,
     * so this computer's LAN address from `lan_address`, or a placeholder
     * for it, takes its place.
     */
    pub(super) fn of(server: &str, lan_address: impl FnOnce() -> Option<IpAddr>) -> Self {
        if !network::is_local_only(server) {
            return Self {
                address: server.to_owned(),
                note: None,
            };
        }
        let host = match lan_address() {
            Some(IpAddr::V6(address)) => format!("[{address}]"),
            Some(IpAddr::V4(address)) => address.to_string(),
            None => LAN_ADDRESS_PLACEHOLDER.to_owned(),
        };
        Self {
            address: network::with_host(server, &host).unwrap_or_else(|| server.to_owned()),
            note: Some(LOCAL_SERVER_NOTE),
        }
    }
}

impl RaceClient {
    pub fn invite(&self) -> Option<Invite> {
        let room = self.room.as_ref()?;
        Some(Invite {
            command: format!(
                "code-racer join {} --server {}",
                room.code, self.shared_server.address
            ),
            note: self.shared_server.note,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_invite_gives_teammates_an_address_they_can_reach() {
        let lan: IpAddr = [192, 168, 1, 42].into();
        let shared = SharedServer::of("ws://127.0.0.1:8080", || Some(lan));
        assert_eq!(shared.address, "ws://192.168.1.42:8080");
        assert_eq!(shared.note, Some(LOCAL_SERVER_NOTE));

        let unknown = SharedServer::of("ws://localhost:9000", || None);
        assert_eq!(
            unknown.address,
            format!("ws://{LAN_ADDRESS_PLACEHOLDER}:9000")
        );
        assert_eq!(unknown.note, Some(LOCAL_SERVER_NOTE));

        let probed = std::cell::Cell::new(false);
        let remote = SharedServer::of("ws://10.0.0.9:8080", || {
            probed.set(true);
            None
        });
        assert_eq!(remote.address, "ws://10.0.0.9:8080");
        assert_eq!(remote.note, None);
        assert!(!probed.get(), "no need to look for this computer's address");
    }
}
