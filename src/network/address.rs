use std::net::{IpAddr, Ipv4Addr, SocketAddr, UdpSocket};

use thiserror::Error;

/**
 * An address on another network, from a range reserved for documentation:
 * the route to it is the one to every other network.
 */
const OTHER_NETWORK: SocketAddr = SocketAddr::new(IpAddr::V4(Ipv4Addr::new(192, 0, 2, 1)), 9);

#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum InvalidServerUrl {
    #[error("the server address is empty")]
    Empty,
    #[error("`{0}://` is not supported, use ws:// or wss://")]
    Scheme(String),
    #[error("`{0}` does not name a host")]
    Host(String),
    #[error("`{0}` has an invalid port, use a number up to 65535")]
    Port(String),
}

/**
 * Turns what a user typed into a WebSocket URL.
 *
 * An address without scheme gets `ws://`, `http` and `https` become `ws` and
 * `wss`, and any other scheme is refused. Normalising twice changes nothing.
 */
pub fn server_url(input: &str) -> Result<String, InvalidServerUrl> {
    let input = input.trim();
    if input.is_empty() {
        return Err(InvalidServerUrl::Empty);
    }
    let (scheme, address) = match input.split_once("://") {
        Some((scheme, address)) => (websocket_scheme(scheme)?, address),
        None => ("ws", input),
    };
    let invalid_host = || InvalidServerUrl::Host(input.to_owned());
    let (host, port) = host_and_port(address).ok_or_else(invalid_host)?;
    if host.is_empty() || host.contains(char::is_whitespace) {
        return Err(invalid_host());
    }
    if port.is_some_and(|port| !is_port(port)) {
        return Err(InvalidServerUrl::Port(input.to_owned()));
    }
    Ok(format!("{scheme}://{address}"))
}

fn websocket_scheme(scheme: &str) -> Result<&'static str, InvalidServerUrl> {
    match scheme.to_ascii_lowercase().as_str() {
        "ws" | "http" => Ok("ws"),
        "wss" | "https" => Ok("wss"),
        _ => Err(InvalidServerUrl::Scheme(scheme.to_owned())),
    }
}

/**
 * Whether the server at `url` is only reachable from this computer: its
 * host is `localhost`, or a loopback or unspecified address, which another
 * computer would take for itself.
 */
pub fn is_local_only(url: &str) -> bool {
    let address = url.split_once("://").map_or(url, |(_, address)| address);
    let Some((host, _)) = host_and_port(address) else {
        return false;
    };
    let host = host.to_ascii_lowercase();
    host == "localhost"
        || host.ends_with(".localhost")
        || host
            .parse::<IpAddr>()
            .is_ok_and(|ip| ip.is_loopback() || ip.is_unspecified())
}

/**
 * `url` with `host`, an IPv6 address given in brackets, in place of its
 * host, keeping the scheme, user, port and path. `None` without a scheme.
 */
pub fn with_host(url: &str, host: &str) -> Option<String> {
    let (scheme, address) = url.split_once("://")?;
    let (authority, path) = split_authority(address);
    let user = authority
        .rsplit_once('@')
        .map(|(user, _)| format!("{user}@"))
        .unwrap_or_default();
    let (_, port) = host_and_port(authority)?;
    let port = port.map(|port| format!(":{port}")).unwrap_or_default();
    Some(format!("{scheme}://{user}{host}{port}{path}"))
}

/**
 * This computer's address on the local network: the one the system would
 * send from to reach other networks. Connecting a UDP socket only picks
 * that route, nothing is sent. `None` without such a route.
 */
pub fn lan_address() -> Option<IpAddr> {
    let socket = UdpSocket::bind((Ipv4Addr::UNSPECIFIED, 0)).ok()?;
    socket.connect(OTHER_NETWORK).ok()?;
    let address = socket.local_addr().ok()?.ip();
    (!address.is_loopback() && !address.is_unspecified()).then_some(address)
}

/**
 * The authority of `address`, then what follows it: path, query and
 * fragment.
 */
fn split_authority(address: &str) -> (&str, &str) {
    let end = address.find(['/', '?', '#']).unwrap_or(address.len());
    address.split_at(end)
}

/**
 * Host and port of the authority in `address`, `None` when an IPv6 address
 * is not properly bracketed.
 */
fn host_and_port(address: &str) -> Option<(&str, Option<&str>)> {
    let (authority, _) = split_authority(address);
    let host_and_port = authority
        .rsplit_once('@')
        .map_or(authority, |(_, host_and_port)| host_and_port);
    let Some(bracketed) = host_and_port.strip_prefix('[') else {
        return Some(match host_and_port.split_once(':') {
            Some((host, port)) => (host, Some(port)),
            None => (host_and_port, None),
        });
    };
    let (host, rest) = bracketed.split_once(']')?;
    match rest.strip_prefix(':') {
        Some(port) => Some((host, Some(port))),
        None => rest.is_empty().then_some((host, None)),
    }
}

fn is_port(port: &str) -> bool {
    port.bytes().all(|byte| byte.is_ascii_digit()) && port.parse::<u16>().is_ok()
}
