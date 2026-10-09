# code-racer-server

The race server behind code-racer's multiplayer mode. It keeps rooms in memory, runs the countdown,
times every finish and checks what each player reports. Clients only send their typing counters:
speed, accuracy and finishing times are worked out here, with the same engine the client uses for
solo sessions.

## Running it

```bash
cargo install --path crates/server       # from the repository root

code-racer-server                                  # 127.0.0.1:8080, this machine only
code-racer-server --host 0.0.0.0                   # reachable from the local network
code-racer-server --max-players 12 --countdown 5
RUST_LOG=debug code-racer-server                   # more detail on stderr
```

| Option                          | Default     | Meaning                                                  |
| ------------------------------- | ----------- | -------------------------------------------------------- |
| `--host`                        | `127.0.0.1` | address to listen on; `0.0.0.0` accepts the whole LAN    |
| `--port`                        | `8080`      | TCP port                                                 |
| `--max-players`                 | `8`         | players per room, from 2 to 32                           |
| `--room-ttl`                    | `1800`      | seconds without activity before a room is closed         |
| `--race-timeout`                | `300`       | seconds after which a race ends, finished or not         |
| `--countdown`                   | `3`         | seconds between the host's start and the first keystroke |
| `--max-connections-per-address` | `16`        | connections one IP address may hold; `0` lifts the limit |

Logs go to stderr, in colour on a terminal and as plain text everywhere else (files, pipes,
`docker logs`, journald). `NO_COLOR` is honoured. Ctrl+C and `SIGTERM` stop the server cleanly,
giving open connections two seconds to close.

## On a local network

On the machine that hosts the races, say `192.168.1.42`:

```bash
code-racer-server --host 0.0.0.0
sudo firewall-cmd --add-port=8080/tcp      # Fedora, if the firewall is on
```

Players then run `code-racer --server ws://192.168.1.42:8080`, or save the address once with
`:set server=ws://192.168.1.42:8080`. When the host creates a room from that same machine, the
client shows teammates its LAN address instead of `127.0.0.1`.

Plain `ws://` is fine on a network you trust. The client also speaks `wss://`, so the server can sit
behind a TLS reverse proxy such as `wss://race.example.com`. Every player then seems to come from
the proxy's address, so start the server with `--max-connections-per-address 0`.

## With Docker

```bash
docker build -t code-racer-server .
docker run -p 8080:8080 code-racer-server
```

The [Dockerfile](../../Dockerfile) builds in two stages. Dependencies are compiled in their own
cached layer, and the final image is a slim Debian with the binary running as an unprivileged
user, listening on `0.0.0.0:8080`. Arguments after the image name replace the defaults, so
`docker run -p 9000:9000 code-racer-server --host 0.0.0.0 --port 9000` works as you would expect.
CI smoke-tests the image (a WebSocket upgrade, then a clean exit on `SIGTERM`) and publishes it to
`ghcr.io/klugko/code-racer-server` on every push to `main`.

## Rooms and races

A room is created by its host with the text settings for its races, and gets a six-character code
such as `FK72AD`, without the easily confused `0`, `O`, `1` and `I`. Anyone who knows the code can
join while the room is in its lobby.

Players mark themselves ready and the host starts. The server picks the text, sends it with the
countdown, and from then on measures time from its own clock. A race ends when every connected
player has finished, when nobody is left, or at the race timeout. Unfinished players are ranked by
how much of the text they typed correctly.

People leave, and the server keeps going:

- a player who disconnects in the lobby is removed from the room;
- one who disconnects during a race stays in the standings, marked offline, and the race goes on;
- when the host leaves, the next player to have joined becomes host;
- a room with no activity for `--room-ttl` seconds is closed, and its members are told why.

## What it does not trust

The server is meant for a team on a LAN, but it never takes a client's word for anything:

- **Progress is checked.** A report cannot go past the end of the text, counters can only grow, and
  typing faster than 30 characters a second (about 360 WPM, with a five-character allowance for
  bursts) is refused.
- **Messages are bounded.** Nothing over 16 KiB is accepted in either direction, and a test proves
  that the largest possible room view still fits. A connection may send 40 messages per second.
- **Connections are bounded.** 512 at once in total, 16 per IP address by default, 256 rooms. A
  client has 10 seconds to say hello, is pinged every 30 seconds and dropped after 75 seconds of
  silence, and a write that blocks for 10 seconds closes the connection.
- **The code is safe Rust.** `unsafe` is forbidden across the workspace.

There is no authentication: anyone who can reach the port and guesses or learns a room code can
join. Keep the server on a network you trust, or behind a reverse proxy you control.

## How it is organised

Each connection runs in its own task. It handles the WebSocket handshake, the `Hello` exchange,
pings, rate limiting and timeouts, and forwards valid messages to a single hub task over a channel.
The hub owns every room, so room state is never shared and nothing is ever locked across an
`.await`. It ticks 20 times a second to move countdowns and races along and to broadcast progress,
coalesced, to the rooms that changed.

The rules themselves live in `room`, a plain state machine that performs no I/O and never reads the
clock: every operation takes the current `Instant`. That keeps it deterministic, and its tests walk
through whole races with simulated time.

The messages are defined in [code-racer-protocol](../protocol), and the WebSocket tests in
`tests/` drive real clients against a real server on a random port.

## As a library

The binary is a thin wrapper around `serve`, which the client's own tests also use to race through
a real server:

```rust
use code_racer_server::{ServerConfig, serve};
use tokio::net::TcpListener;

let listener = TcpListener::bind("127.0.0.1:0").await?;
let shutdown = async { tokio::signal::ctrl_c().await.ok(); };
serve(listener, ServerConfig::default(), shutdown).await?;
```
