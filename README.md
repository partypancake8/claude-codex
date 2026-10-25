# claude-codex by swyr

A Claude Code-inspired terminal interface with compact status and live agent views.

This fork gives Codex a compact composer, a muted 8-bit palette, an animated swyr mascot, and a persistent view of its child agents. It uses Codex's models, authentication, tools, approvals, and session handling.

![claude-codex startup, composer, and working indicator](media/demo.gif?v=2)

## Interface

- **Compact input.** A bottom-anchored prompt with thin rules, the effort setting in its top border, and native multiline editing, image attachments, file suggestions, and history.
- **Readable conversation.** Distinct prompt and response markers, compact tool results, streamed Markdown, and an expandable transcript.
- **Quiet status.** A small braille spinner beside Working and elapsed time. The footer shows reset availability, context and weekly usage, directory, model, and edit counts when available.
- **Visible agents.** A panel below the input and footer shows running, finished, and failed children with their latest activity. The task picker opens individual agent views without losing the parent draft.
- **Original artwork.** The swyr pixel mascot animates beside the model and directory in terminals with supported graphics. Other terminals use a text header.

![Live child activity and completion](media/agents.gif?v=2)

The GIFs show scripted example conversations. See the [UI comparison](UI-COMPARISON.md) for the Claude Code references and the differences in behavior.

## Build and run

The build helper supports native x86_64 and ARM64 macOS or GNU Linux. It requires Python 3.11+, rustup with the pinned Rust 1.95 toolchain, and a native C/C++ build toolchain. Building downloads dependencies and runtime resources. macOS has been exercised; GNU Linux installation has not yet been validated. Windows and cross-compilation are not supported.

This repository contains the full source. From its root:

```sh
git clone https://github.com/partypancake8/claude-codex.git
cd claude-codex
./scripts/claude-codex build
./scripts/claude-codex install
~/.local/bin/claude-codex
```

The installer selects the package through `~/.local/share/claude-codex/current` and creates a separate `~/.local/bin/claude-codex` launcher. Your existing `codex` command remains available. To replace an existing installation after rebuilding, use `./scripts/claude-codex install --apply`. Run `./scripts/claude-codex --help` for options.

`claude-codex build|install|run|status [options] [-- CODEX_ARGS...]` is the source helper interface. The installed launcher opens the UI directly; use `~/.local/bin/claude-codex -- --help` for the underlying CLI help.

Use your normal Codex authentication and configuration, including `CODEX_HOME` if set. The launcher applies its footer layout for the current invocation without editing your configuration files. Model availability, usage limits, permissions, and enabled integrations follow your Codex account and configuration.

## Controls

| Action | Control |
| --- | --- |
| Browse commands | `/` |
| Select model and effort | `/model` |
| Select permissions | `/permissions` |
| Open the detailed transcript | `Ctrl+O` |
| Open tasks and child agents | `/tasks`, `/subagents`, or `Ctrl+T` |
| Open the child picker from an empty prompt | Left arrow, when children exist |
| Interrupt the current response | `Esc` |
| Copy the last response | `/copy` or `Alt+C` |
| Resume a session | `/resume` |

Custom key bindings take precedence. Slash commands remain available when a terminal intercepts a key combination. Child views indicate whether direct input is allowed.

## Terminal behavior

The prompt stays at the bottom as output arrives. Narrow panes wrap conversation and agent activity; very short windows give the editable prompt priority over suggestions and footer content.

The native sprite requires Kitty graphics support and at least 32 terminal columns. It pauses when hidden or unfocused. With animation disabled, the sprite stays still and the Working spinner is omitted. Text remains available without image support.

This fork keeps Codex's native command execution, sandboxing, approvals, model selection, session resume, and integrations. It does not provide Claude models or convert Claude Code sessions. See [UI-COMPARISON.md](UI-COMPARISON.md) for the scope of the visual adaptation.

See [validation](VALIDATION.md) for test coverage and platform limits.

## License

Licensed under [Apache 2.0](LICENSE).

This is an independent project by swyr, with no affiliation with or endorsement from OpenAI or Anthropic. Codex and Claude Code belong to their respective owners.
