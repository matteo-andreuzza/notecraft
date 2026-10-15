# Control protocol

Start the desktop app with a control port: `notecraft --control 7990` (or
`NOTECRAFT_CONTROL_PORT=7990`). It listens on `127.0.0.1` only. Send one JSON object per line,
with the window's key (see [Keys](#keys)); each gets one reply line:

```json
{"id": 1, "key": "…", "method": "engine.execute", "params": {"command": "text.insert", "params": {"text": "Hello"}}}
{"id": 1, "ok": true, "result": {}}
```

Errors come back as `{"id": …, "ok": false, "error": "message"}`. A failed command leaves the
notes unchanged. A line that isn't valid JSON gets one error reply, then the app closes the
connection (so an HTTP request sent to the port can't run a command); reconnect to continue.
Requests are answered on the window's thread, in order, between frames; a request the app does
not answer within 60 s gets `{"ok": false, "error": "timeout"}`.

`notecraft-cli mcp --connect 127.0.0.1:7990` wraps this channel as an MCP server
([MCP server](mcp.md)).

## Keys

Any local program (and possibly a web page) can reach a loopback port, so every request needs the
window's key. Once the port is bound, the app writes a random key (256 bits, as hex) to
`<settings>/control-key.<instance>` and removes the file when the window closes; a new start
writes a new key. `<settings>` is the folder that holds the preferences: on Linux
`$XDG_CONFIG_HOME/notecraft` (by default `~/.config/notecraft`), on macOS
`~/Library/Application Support/NoteCraft`, on Windows `%APPDATA%\NoteCraft`. `<instance>` is the
Flatpak instance id inside Flatpak, and `local-<port>` otherwise. On Unix the file has mode 0600.

```sh
cat ~/.config/notecraft/control-key.local-7990   # Linux, outside Flatpak, port 7990
```

- Send the key as top-level `"key"` with every request, or once per connection with
  `{"method": "auth", "key": "…"}` (reply `{"ok": true, "result": {}}`); later requests on that
  connection may leave `key` out. Only `auth` reads `params.key`; for other methods `params.key`
  is an ordinary parameter (`ui.key {"key": "Enter"}` presses Enter).
- The key is checked on every request. A missing or wrong key gets
  `{"ok": false, "error": "unauthorized"}`, and the app closes the connection. When the app could
  not write its key file it refuses every request (and logs why).
- A line over 1 MiB gets `{"ok": false, "error": "line too long"}`, and the app closes the
  connection.
- A connection that sends no authorised request within 30 s of being accepted is closed. That is
  one limit: blank lines and bad JSON do not extend it. After an authorised request the
  connection may stay idle.

`notecraft-cli mcp --connect` takes the key from `NOTECRAFT_CONTROL_KEY`, or else from the key
file for the port it connects to, and reads it again on every new connection (the app may have
restarted). It sends the key only to loopback addresses (`127.0.0.0/8`, `::1`, `localhost`). For
any other host it sends no key, says so, and the app refuses its requests: to drive an app on
another machine, forward its port to `127.0.0.1` (for example `ssh -L 7990:127.0.0.1:7990 host`)
and set `NOTECRAFT_CONTROL_KEY` to that app's key.

## Methods

| Method | Params | Result |
|---|---|---|
| `engine.execute` | `command`, `params?` | the command's result; any id from `engine.commands` |
| `engine.commands` | — | every command: id, catalog id, label, ribbon location, shortcut, params, `enabled`, `disabledReason` |
| `engine.parity` | — | catalog rows, wired and verified counts, number of commands |
| `notebook.inspect` | `page?` (id), `runs?`, `tree?` (default true) | notebooks → section groups → sections → pages, selection, view, undo labels, dirty, and a page's note containers with paragraphs (text, level, style, alignment, list label, tags) |
| `page.text` | `page?` (id) | `{page, text}`: the page as plain text |
| `notebook.save` | — | save changed folder notebooks now (`{"saved": true}`, or the errors) |
| `ui.key` | `key` (`A`–`Z`, `0`–`9`, `Enter`, `Tab`, `Escape`, `Backspace`, `Delete`, arrows, `Home`, `End`, `PageUp`, `PageDown`, `F1`–`F12`…), `shift?`, `ctrl?`, `alt?` | a key press through the window (shortcuts apply) |
| `ui.text` | `text` | text typed through the window |
| `ui.click` | `x`, `y` (window points), `button?` (`primary`/`secondary`), `count?` (1 or 2) | a real pointer click |
| `ui.screenshot` | `path?` | PNG of the window: written to `path`, or without one returned as base64 `png` with `width` and `height` |
| `ui.inspect` | — | window size, ribbon tab, open dialog, backstage, theme |
| `app.quit` | — | save and close the window |

Any other method name is treated as a command id: `{"method": "format.bold"}` works.

Without a window (`notecraft-cli mcp` without `--connect`), `ui.*` methods return an error that
says how to start the app.
