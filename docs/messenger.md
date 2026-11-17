# Messenger outpost

Add `messenger icon extra` to the Stanley bar file to show the Messenger
outpost. A left click opens its popup. Start and Stop act only on the user
unit `quiver-outpost-messenger.service`; they do not enable it at login.
The popup also offers its service log.

The bar icon is white when a fresh report confirms the active outpost is
online, and gray when offline or unavailable. It shows no connection text or
unknown-count marker. A running process alone does not prove that Messenger
or the Quiver bridge is connected. Connection details remain in the popup.

A connected outpost shows its unread message count only when the source
reports an accurate count, including a real zero. Without one, it shows the
exact unread chat count when the source reports it. Otherwise `extra` is
omitted. The popup labels a chat count as chats, never as messages.
The popup labels whether the count is native Messenger unread or unread
Messenger messages relayed into Quiver for the owner. Those are
different read positions. A Quiver count must count canonical Messenger
messages once across breakouts. A thread count, activity timestamp, or sum of
breakout notification badges is not an unread message count.

## Recent chats and replies

When the popup opens, it loads the five most recent chats once, off the GTK
thread, with a 10 second limit. Each row shows the chat name, the last message
on one shortened line, and its age (`now`, `5m`, `3h`, `2d`). If the call
fails or times out, one short line replaces the list; Start, Stop, and the log
still work. The bar widget does not run these calls.

Click a chat to open a reply line under it. Enter or Send sends the text;
nothing is sent any other way, including when the popup closes. The line then
shows `Sent` or the error. `Open chat` opens that chat's breakout in the
browser. `Open in Quiver` opens the Messenger app's DM.

The popup runs the quiver CLI from `~/src/morkovka.quiver.city`, the same
workspace the service uses, so it acts as that workspace's member. Quiver
fills in `caller` itself:

| Control | Command |
| --- | --- |
| Open popup | `quiver call -c @quiver-messenger quiver-messenger.chats '{"limit": 5}'` |
| Send | `quiver call -c @quiver-messenger quiver-messenger.sendMessage -` with `{"threadId", "text"}` on stdin |
| Open chat | `quiver call -c @quiver-messenger quiver-messenger.openChat '{"threadId": ...}'`, then `xdg-open <baseUrl>/~/c/+<breakoutChannelId>` |
| Open in Quiver | `xdg-open <baseUrl>/~/c/@quiver-messenger` |

`baseUrl` comes from the workspace's `.quiver/config.json`. The reply goes on
stdin so it does not appear in the process list. Chat names, previews, and
replies stay in the popup's memory only; nothing writes them to disk or a log.

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
  "unreadChats": 3,
  "unreadChatsReason": null
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
the popup. `unreadChats` is the exact nonnegative number of Messenger chats
with unread messages, or null. It is accepted only while online. Negative,
fractional, boolean, and string values are unavailable. A missing field is
null, so older snapshots behave as before. A message count wins when both are
present. When both are null, the popup shows `unreadChatsReason` if it is a
string, else `unreadReason`.

The current outpost cannot prove complete native unread message coverage, so
`unreadMessages` remains null. It reports `unreadChats` instead, which the
bar shows as the count and the popup labels as unread chats. No new Quiver API, database state or CLI feature is
required. The widget does not query Quiver unread or infer it from partial
local message caches.

Without a compatible publisher, service status and controls still work and
unread remains unavailable. Stanley never calls `quiver messages unread`,
which consumes messages by marking them read.

## Isolated checks

`make test` runs the Rust block fixtures and Python popup fixtures. The latter
use a fake UI and subprocess runner, so they need no display, running service,
credentials, or GTK installation and cannot start or stop the live service.
They cover service and source availability, zero/nonzero/invalid counts,
source age and identity, visible labels, and explicit start/stop arguments.
Chat fixtures cover the listed and shortened chats, an empty list, a failed
or timed-out query, a sent and a failed reply, and the exact arguments for
`chats`, `sendMessage`, `openChat`, and the browser links.
