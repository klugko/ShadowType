# Running a race server

```bash
code-racer-server                                  # 127.0.0.1:8080
code-racer-server --host 0.0.0.0 --port 8080       # reachable from the LAN
code-racer-server --max-players 8 --room-ttl 1800 --race-timeout 300 --countdown 3
code-racer-server --max-connections-per-address 0  # behind a proxy that hides client addresses
RUST_LOG=debug code-racer-server                   # verbose logs, on stderr
```

| Option                          | Default     | Meaning                                       |
| ------------------------------- | ----------- | --------------------------------------------- |
| `--host`                        | `127.0.0.1` | address to listen on, `0.0.0.0` for the LAN   |
| `--port`                        | `8080`      | TCP port                                      |
| `--max-players`                 | `8`         | players per room                              |
| `--room-ttl`                    | `1800`      | seconds of inactivity before a room is closed |
| `--race-timeout`                | `300`       | seconds after which a race ends anyway        |
| `--countdown`                   | `3`         | seconds between the start and the first key   |
| `--max-connections-per-address` | `16`        | connections one IP may hold, `0` for no limit |

Rooms live in memory only; restarting the server closes them.

## On a LAN

On the host (here `192.168.1.42`):

```bash
code-racer-server --host 0.0.0.0 --port 8080
sudo firewall-cmd --add-port=8080/tcp      # Fedora, if the firewall is on
```

On every player's machine, either pass `--server ws://192.168.1.42:8080` or save it once with
`:set server=ws://192.168.1.42:8080`.

`ws://` is fine on a trusted LAN. The client also speaks `wss://`, so the server can sit behind a TLS
reverse proxy (e.g. `wss://race.example.com`); in that case use `--max-connections-per-address 0`,
since every client appears to come from the proxy.

## Docker

```bash
docker run -p 8080:8080 ghcr.io/klugko/code-racer-server:latest
# or build it yourself
docker build -t code-racer-server . && docker run -p 8080:8080 code-racer-server
```

The image is a two-stage build with a cached dependency layer, and runs the server as an unprivileged
user. CI smoke-tests it (WebSocket upgrade and clean shutdown on `SIGTERM`) and publishes it to
`ghcr.io/klugko/code-racer-server` on every push to `main`, tagged `latest` and with the commit SHA.

## Server hardening

The server is designed for trusted LANs but does not trust its clients:

- **Authoritative race**: progress cannot exceed the text, counters cannot go backwards, speeds above
  about 360 WPM are rejected, and finishing times, speed and accuracy are computed server-side.
- **Bounded resources**: messages and frames are capped at 16 KiB (a test proves the largest possible
  room view fits), each connection is limited to 40 messages per sliding second, each IP may hold 16
  connections by default, and idle rooms expire.
- **Defensive timeouts**: idle connections, stuck writes and unanswered closes are all dropped.
- **Unprivileged container**, and `unsafe_code = "forbid"` across the whole workspace.

There is no authentication: anyone who can reach the port and knows a room code can join. Expose the
server beyond a trusted network only behind a TLS reverse proxy you control.

