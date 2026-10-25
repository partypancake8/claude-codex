# UI comparison

claude-codex adapts the compact terminal presentation of Claude Code to the Codex runtime. The original swyr mascot, muted palette, built-in usage footer, and persistent child-agent panel are this project's choices.

The comparison below uses Anthropic's public documentation. Claude Code's interface changes over time, and its appearance depends on renderer, terminal, and settings. These references describe interaction patterns, not a claim of pixel-for-pixel parity.

| Area | Claude Code reference | claude-codex |
| --- | --- | --- |
| Window layout | The fullscreen renderer fixes input at the bottom and handles conversation scrolling within the app. [Fullscreen rendering](https://code.claude.com/docs/en/fullscreen) | Input stays at the bottom, with compact rules and an effort annotation. Finalized output remains in native terminal scrollback; `Ctrl+O` opens the detailed transcript. |
| Conversation | `Ctrl+O` exposes detailed tool activity and expanded messages. [Interactive mode](https://code.claude.com/docs/en/interactive-mode) | `❯` marks user input, `●` marks responses and tools, and `⎿` introduces results. Markdown, streaming, errors, and detailed transcript navigation use Codex. |
| Composer | Supports multiline input, image paste, command history, and shell input with `!`. [Interactive mode](https://code.claude.com/docs/en/interactive-mode) | Preserves Codex multiline editing, pasted images, attachments, history, file suggestions, and shell mode inside a compact ruled composer. |
| Status | A configurable shell script can render session data in a status line above the built-in footer badges. [Status line](https://code.claude.com/docs/en/statusline) | A built-in compact footer shows reset availability, context and weekly usage, directory, model, and edit counts. Unknown reset availability is shown as `resets:n/a`. |
| Model and effort | `/model` selects a model and exposes supported effort settings. [Model configuration](https://code.claude.com/docs/en/model-config) | Uses Codex's model and effort picker. The current effort appears in the input's top border. |
| Background activity | `/tasks` lists background shells and subagents. `Ctrl+T` toggles Claude's task checklist. [Interactive mode](https://code.claude.com/docs/en/interactive-mode) | `/tasks` and `/subagents` open Codex's task picker. `Ctrl+T` is a shortcut to that picker unless already bound. Up to three children remain visible below the input and footer. |
| Agent views | Experimental agent teams offer an in-process agent panel or separate terminal panes. [Agent teams](https://code.claude.com/docs/en/agent-teams) | Selecting a child opens Codex's native thread viewer. The persistent panel shows activity, completion, and failure; the parent draft survives switching views. |

## Project-specific details

The Working indicator is a ten-frame braille loop with a 90 ms frame interval. It occupies one character cell followed by a space, so the label stays aligned as it animates. Elapsed time and the interrupt hint sit beside it. Motion follows the task clock and pauses during approvals; reduced-motion mode omits the spinner.

The swyr mascot uses original pixel artwork through supported terminal graphics. Its text-only fallback keeps the model and directory visible when images cannot be displayed. The composer retains both horizontal rules while placing effort in the top rule, saving a separate status row.

The child panel stays below the prompt, usage bar, and mode row. Active children take priority; completed or failed children retain a visible state and recent activity. Agent creation, permissions, and whether a child accepts direct input remain Codex decisions. This panel does not implement Claude Code's agent-team protocol or split-pane orchestration.

## Runtime boundaries

Authentication, model requests, tools, approvals, sandboxing, session files, and integrations belong to Codex. Claude Code provides the visual and interaction references linked above. The project includes no Claude runtime, Claude credentials, or copied Claude session data.

The demos in [the README](README.md) show this interface. They are scripted examples, not screenshots of Claude Code or a model-performance comparison.
