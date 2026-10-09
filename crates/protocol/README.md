# code-racer-protocol

The messages code-racer clients and the race server exchange, as Rust types shared by both sides.
Each message is one JSON object in a WebSocket text frame, shaped as `{"type": ..., "data": ...}`.
Messages without a payload have no `data`.

The format is plain enough to drive a server by hand with any WebSocket tool, which is the easiest
way to see it at work:

```text
→ {"type":"hello","data":{"version":3,"username":"jean"}}
← {"type":"welcome","data":{"version":3,"player_id":7}}
→ {"type":"create_room","data":{"text":{"kind":"words","language":"english","count":25,"punctuation":true,"numbers":false}}}
← {"type":"room","data":{"code":"FK72AD","host":7,"phase":"lobby", ...}}
```

## Handshake and versions

The first message of every connection is `hello`, carrying the protocol version and the player's
name. The server answers `welcome` with the id it gives the player, or an `incompatible_version`
error and closes the connection. Anything sent before `hello` gets `handshake_required`.

Only peers speaking the same version talk to each other, so no message ever has to accept the shape
of an older one. `PROTOCOL_VERSION` goes up whenever a change would break an older peer. It is
currently 3: that version stopped counting auto-filled indentation as keystrokes, and made
`indentation` a required field of `progress`.

## From the client

| `type`            | `data`                     | When                                               |
| ----------------- | -------------------------- | -------------------------------------------------- |
| `hello`           | `version`, `username`      | first, and only once                               |
| `create_room`     | `text`                     | to open a room, becoming its host                  |
| `join_room`       | `code`                     | to join a room in its lobby                        |
| `leave_room`      |                            | to leave the current room                          |
| `set_ready`       | `ready`                    | in the lobby                                       |
| `start_race`      |                            | host only, once every player is ready              |
| `progress`        | the counters below         | regularly while racing                             |
| `return_to_lobby` |                            | host only, once the race is over                   |

`text` says what to race on, never the text itself: the server generates it. It is one of
`{"kind":"words","language":"english","count":25,"punctuation":false,"numbers":false}`,
`{"kind":"quote","language":"french"}` or `{"kind":"code","language":"rust"}`. A race takes 5 to
200 words.

A `progress` report carries cumulative counters, never speeds or times:

```json
{"type":"progress","data":{"typed":42,"correct":40,"indentation":0,"keystrokes":45,"errors":3}}
```

`typed` is the cursor position, `correct` the characters that currently match the text,
`indentation` the part of `correct` that auto-indentation filled in, `keystrokes` every key that
was pressed, and `errors` the keys that did not match, corrected or not. The server checks each
report against the previous one and the text, then derives speed, accuracy and finishing time with
its own clock.

## From the server

| `type`      | `data`                         | When                                                |
| ----------- | ------------------------------ | --------------------------------------------------- |
| `welcome`   | `version`, `player_id`         | in answer to `hello`                                |
| `room`      | the whole room, see below      | whenever anything in the player's room changes      |
| `countdown` | `text`, `duration_ms`          | once per race, when the host starts it              |
| `error`     | `code`, `message`              | to whoever sent a refused request                   |

Rather than describing changes, the server sends the complete room every time, so a client simply
draws the latest one it received. During a race, progress is coalesced and sent at most 20 times a
second. When a room is closed for inactivity, all its members get a `room_not_found` error saying
so.

```json
{
  "type": "room",
  "data": {
    "code": "FK72AD",
    "host": 7,
    "text": { "kind": "quote", "language": "english" },
    "text_length": 45,
    "phase": "racing",
    "max_players": 8,
    "players": [
      {
        "id": 7,
        "name": "jean",
        "ready": true,
        "connected": true,
        "progress": { "typed": 20, "correct": 19, "errors": 1, "wpm": 87.5, "accuracy": 95.0, "finish_ms": null }
      }
    ]
  }
}
```

`phase` goes from `lobby` to `countdown`, `racing` and `finished`, then back to `lobby` when the
host asks. `text_length` counts characters as the player sees them (grapheme clusters), and is 0
before the first race. Players are listed in the order they joined. One who leaves during a race
stays listed with `connected: false` until the room is back in the lobby. `finish_ms` is the race
time the server measured, once the player has typed the whole text. `RoomView::standings` ranks
players the way every client shows them: finished players by time, then the others by how much
they typed correctly.

The `message` of an error is meant for people, such as `room FK72AD not found`. The `code` is for
programs:

| Code                   | Meaning                                                      |
| ---------------------- | ------------------------------------------------------------ |
| `incompatible_version` | the `hello` announced another protocol version              |
| `handshake_required`   | something other than `hello` came first, or nothing came     |
| `invalid_message`      | not valid JSON, an unknown message or an invalid field       |
| `rate_limited`         | too many messages in a short time                            |
| `server_full`          | no room for another connection or another room              |
| `room_not_found`       | no room has this code                                        |
| `room_full`            | the room has as many players as it allows                    |
| `race_in_progress`     | the room is not in its lobby                                 |
| `not_in_room`          | the request needs a room and the player is in none           |
| `not_host`             | only the host can do this                                    |
| `players_not_ready`    | someone in the room is not ready yet                         |
| `race_not_running`     | progress or a return to the lobby outside a race             |
| `invalid_progress`     | a report the server cannot believe                           |
| `invalid_settings`     | a text the server will not race on                           |

A code a client does not know decodes as `ErrorCode::Unknown` instead of failing, so a newer server
can add codes without breaking older clients.

## Validated identifiers

Room codes and player names cannot be built invalid, and decoding a message fails if one is:

- a `RoomCode` is six characters from `ABCDEFGHJKLMNPQRSTUVWXYZ23456789`, leaving out `0`, `O`, `1`
  and `I`, which are easy to confuse. Parsing accepts lowercase and surrounding spaces.
- a `Username` has 1 to 24 visible characters and at most 128 bytes. Control characters, line
  breaks and characters that take no room on screen (zero-width spaces, bidirectional controls,
  lone combining marks) are refused, so a name can never look blank or scramble the line it is
  shown on.

## Size limits

Neither side accepts a message over `MAX_MESSAGE_BYTES`, 16 KiB. Every valid message fits: names
are bounded in bytes and rooms to `MAX_ROOM_PLAYERS`, 32 players, precisely so that the largest
room view stays under the limit. The `message_size` test builds that worst case and checks it,
along with the countdown of every quote and code snippet in the corpus.
