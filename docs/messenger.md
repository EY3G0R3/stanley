# Messenger outpost

Add `messenger icon extra` to the Stanley bar file to show the Messenger
outpost. A left click opens its popup. Start and Stop act only on the user
unit `quiver-outpost-messenger.service`; they do not enable it at login.
The popup also offers its service log.

The bar shows `off`, `starting ?`, `stopping ?`, `failed ?`, or `unavailable ?`
when the service is not running. `running ?` means the service is active but
there is no current connection report. A running process alone does not prove
that Messenger or the Quiver bridge is connected.

A connected outpost shows its unread message count, including `0` when the
source reports a real zero. `online ?` means connected with unread unavailable;
`offline ?`, `connecting ?`, and `error ?` describe the connection report.
The popup labels whether the count is native Messenger unread or unread
Messenger messages relayed into Quiver for the owner. Those are
different read positions. A Quiver count must count canonical Messenger
messages once across breakouts. A thread count, activity timestamp, or sum of
breakout notification badges is not an unread message count.

## Local status source

The outpost publishes a small credential-free JSON snapshot at
`$XDG_RUNTIME_DIR/quiver-outpost-messenger/status.json`. Stanley only reads it;
it never reads Messenger cookies, tokens, message bodies, or the login store.
The producer must write atomically with mode 0600 and refresh at least every
10 seconds. The v1 envelope is:

```json
{
  "version": 1,
  "updatedAt": 1800000000,
  "invocationId": "the inherited systemd INVOCATION_ID",
  "connection": "online",
  "unreadSource": "messenger",
  "unreadMessages": null,
  "unreadReason": "Complete unread state is not available",
  "quiverUrl": "https://example.test",
  "appId": "quiver-messenger",
  "channelId": "baseChannelId",
  "ownerHandle": "owner",
  "ownerMemberId": "stableHumanMemberId"
}
```

`updatedAt` is integer Unix time in seconds. The invocation must match
`systemctl --user show` for this service. Snapshots older than 30 seconds,
future snapshots, malformed snapshots, and snapshots from previous service
runs are unavailable. `connection` is `online`, `offline`, `connecting`, or
`error`; online requires both Messenger transports and the Quiver bridge.
Counts are accepted only while online and with a recognized `unreadSource`
(`messenger` or `quiver`). `unreadMessages` is an exact nonnegative integer,
or null when coverage, read markers, identity, or the source is unavailable.
A bounded or partial count must remain null. `unreadReason` explains that in
the popup.

When native unread is unavailable, Stanley queries `quiver messages app-unread`
using the acknowledged owner, stable human member ID, endpoint and base channel.
It uses the owner's existing saved session, without login or session renewal.
The read-only query counts `in:` relay posts once by canonical key across the
base channel and its current delegates, using that human's raw read positions.
This includes posts relayed from the owner's other Messenger devices; it excludes
history blocks and delivery notices. Removed delegates are outside this metric,
which the popup labels “Quiver relayed unread (current chats)”.

Queries run at most every 15 seconds per view, with an eight-second deadline
and a 20-second result cache. Context changes and offline state discard counts.
The CLI needs the compatible server query and saved human session; missing
identity, incomplete scans, query failures and unavailable APIs remain unknown.
The widget requires coreutils `timeout` and `quiver` on PATH. The outpost
publisher and compatible Quiver API/CLI are separate installation dependencies.

Without a compatible publisher, service status and controls still work and
unread remains unavailable. Stanley never calls `quiver messages unread`,
which consumes messages by marking them read.

## Isolated checks

`make test` runs the Rust block fixtures and Python popup fixtures. The latter
use a fake UI and subprocess runner, so they need no display, running service,
credentials, or GTK installation and cannot start or stop the live service.
They cover service and source availability, zero/nonzero/invalid counts,
source age and identity, visible labels, and explicit start/stop arguments.
