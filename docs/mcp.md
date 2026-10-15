# MCP server

NoteCraft speaks the [Model Context Protocol](https://modelcontextprotocol.io) (JSON-RPC 2.0, one
message per line over stdio, revision `2025-06-18`; `2025-03-26` and `2024-11-05` are accepted), so
Claude and other agents can take and organise notes.

```sh
notecraft-cli mcp                              # headless session (no window), empty
notecraft-cli mcp --sample                     # headless, with the sample notebook
notecraft-cli mcp --notebook ~/Notebooks/Work  # headless, on a notebook folder (created if missing)
notecraft --control 7990 &                     # or: drive the running app…
notecraft-cli mcp --connect 127.0.0.1:7990     # …including keys, clicks and screenshots
```

Claude Code: `claude mcp add notecraft -- notecraft-cli mcp` (add `--notebook DIR` to work on a
notebook folder). Other clients:

```json
{"mcpServers": {"notecraft": {"command": "/abs/path/target/release/notecraft-cli", "args": ["mcp", "--notebook", "/home/me/Notebooks/Work"]}}}
```

stdout carries only protocol messages; warnings go to stderr (`NOTECRAFT_LOG=info|debug` shows
more). The server ends when stdin closes.

With `--connect`, the bridge sends the app's control key, which it reads from
`NOTECRAFT_CONTROL_KEY` or from the key file the app writes at start, and only to loopback
addresses; see [Keys](control-protocol.md#keys). `--sample`, `--notebook` and `--author` belong
to the headless server and are refused with `--connect` (the app keeps its own notebooks and
settings).

## Saving

Every change is a command run through the engine, as the ribbon or keyboard would run it, with
the same undo steps. A notebook stored in a folder (`new_notebook` with a `path`,
`open_notebook`, `--notebook DIR`) is saved after every command that changed it; a failed save
is reported in the result's `saveErrors` and retried after the next command. A notebook created
without a `path` lives in memory and is gone when the server ends.

## Authorship

Pages and paragraphs record who last changed them. Name the agent so people can tell its edits
from their own: `notecraft-cli mcp --author "Claude (meeting notes)"` (also `run` and `batch`).

## Confining file access

```sh
notecraft-cli mcp --automation-read-root /work/notes --automation-write-root /work/notes
```

The two flags (the other Craft apps take the same ones, so one client config works across the
suite) keep the agent to the folders you give it:

- Every path a command reads must lie inside the **read root**: `open_notebook` /
  `file.openNotebook` (the notebook is saved in place, so it needs the write root too) and
  `file.openBackups`.
- Every path a command writes must lie inside the **write root**: `new_notebook` /
  `file.newNotebook` with a `path`, `file.exportNotebook`, `file.backupNow` and the notebooks
  opened through automation.
- Read and write access are separate. Give the same folder twice for both; a root left out grants
  none of its access. Without either flag nothing is confined.
- A path is made absolute against the server's working directory, then its links are followed: a
  link that leads out of a root is outside it. For a path that doesn't exist yet, its nearest
  existing folder's links are followed and the rest must be plain names, so `..` below a folder
  that doesn't exist is refused, as is a link that leads nowhere. The result is compared with the
  root by whole folder names: `/work2/x` is not inside `/work`.
- A command that takes a `path`, `folder`, `file` or `dir` parameter the roots don't know is
  refused, so a new file command can't slip past them.
- A refused path is an ordinary tool error the agent can read and correct, for example
  `automation path rejected: outside the write root /work/notes: /etc/x` or
  `automation filesystem access is not granted: write authority is absent: /work/notes/x`.
- The roots must be existing folders; a missing one stops the server with an error. Each flag is
  given once (`--flag DIR` or `--flag=DIR`).

The flags confine the headless server; `--connect` with them is an error, since the running app
is not confined by them. A notebook named on the command line (`--notebook DIR`) is the
operator's choice and is not checked. A path is checked, then used: another program that swaps
a link in between can still win that race, so the roots keep an agent to the folders it was
given rather than guarding against other software on the computer.

## Tools

| Tool | What it does |
|---|---|
| `list_commands` | every command (filter with `query`): id, label, ribbon location, shortcut, params, enabled now and why not |
| `execute` | run one command: `{command: "tag.toggle", params: {key: "todo"}}` |
| `batch` | run several commands in order; stops at the first error unless `keepGoing` |
| `new_notebook` | a notebook in memory, or as a folder at `path` |
| `open_notebook` | open a `.ncraft` notebook folder |
| `new_page` | a page at the end of the current section, with an optional `title`; the caret ends in the title |
| `go_to_page` | by `page` id, or by `title` (first case-insensitive match) |
| `type_text` | type at the caret; a newline starts a paragraph (in the title it moves into the body) |
| `inspect` | notebooks, section groups, sections and pages; the selection; a page's note containers with paragraphs, levels, styles, list labels and tags (`runs: true` adds formatting) |
| `get_page_text` | a page as text: `# Title`, then paragraphs indented by level with tags and list labels |
| `search` | search notes in a scope (`page`, `section`, `sectionGroup`, `notebook`, `all`) |
| `find_tags` | tagged notes, grouped (`tag`, `section`, `title`, `date`, `text`), optionally only unchecked To Do items |
| `save` | save changed folder notebooks now |
| `screenshot`, `press_key`, `type_keys`, `click` | app only (`--connect`) |

Resources: `notecraft://notebooks` (inspect) and `notecraft://commands`.

A typical session:

```text
new_notebook {name: "Work", path: "/work/notes/Work"}
new_page {title: "Standup"}
type_text {text: "\nShip the build"}        # newline: out of the title into the body
execute {command: "tag.todo"}                # Ctrl+1
get_page_text {}                             # "# Standup\n[To Do] Ship the build\n"
```

The tests in `crates/automation/src/tests.rs` and `apps/notecraft-cli/tests/mcp.rs` run these
workflows through the protocol.
