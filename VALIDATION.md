# Validation

## Codex 0.166.0 release

Validated on Apple Silicon macOS on November 5, 2026:

| Check | Result |
| --- | --- |
| Rust TUI suite | 5,986 passed, 8 configured skips |
| Scoped Clippy fixes and repository formatting | Completed |
| Final packaged binary | 213 terminal checks across 10 suites passed |
| Agent panel placement | 29 checks passed |
| Public installer and release updater | 15 tests passed |
| Local updater | 12 tests passed |
| Temporary package installation | Version, launcher with spaces in its path, and update dispatch passed |
| Secret scan | Gitleaks 8.30.1: no unreviewed findings |

The packaged executable SHA-256 is `56a9e2406cb52e8b1cc1c49e598b96d1cdc6942c9778523855c1d718f5581c4b`. Terminal checks cover startup, mascot clipping and geometry, narrow panes, multiline input, suggestions in three-row windows, image attachments, real child agents, approval acceptance/rejection/cancellation, Code Mode execution and resume, account reset counters, streaming Markdown, errors, and transcript navigation in both inline and alternate-screen modes. Tests use isolated local fixtures without live account credentials or remote model inference.

The reconnect snapshot normalizes animated frames. The final package also checks the reset-counter refresh against an isolated authenticated account fixture. Eight representative images were inspected from 197 reconstructed captures, including startup, narrow agent panels, child views, approvals, Markdown, errors, and Code Mode. These checks do not establish physical iTerm font/zoom behavior. macOS Reduce Motion was enabled, so animation behavior in this run was intentionally reduced; animated states also have Rust coverage.

The published package contains the CLI, matching Code Mode host and execution server, and ripgrep. It follows the 0.166.0 upstream package layout, which no longer bundles zsh. Source builds use the `dev-small` profile. Only the Apple Silicon macOS binary was validated; Linux, Intel macOS, and physical terminal zoom remain unverified.

The source export excludes personal account files, session history, local review artifacts, and terminal screenshots. Newly flagged upstream test fixtures were compared with upstream source and recorded as exact fingerprints in `.gitleaksignore`.

## Earlier releases

The initial public source snapshot contains the UI that passed the following checks on macOS:

| Check | Result |
| --- | --- |
| Rust TUI tests | 4,151 passed, 6 configured skips |
| Installed terminal behavior | 160 checks across six suites |
| Visual review | 28 inspected images from 147 rendered captures |
| Agent panel placement | 29 checks passed |
| CLI and app-server compatibility | 8 checks passed |
| Code Mode execution and resume | 7 checks passed |
| Public installer tests | 6 passed |
| Temporary installation of the tested package | Launcher, version, status, and Code Mode host passed |

The terminal checks use isolated local response fixtures. Rendered terminal captures cover startup, conversation, composer, status, agents, menus, and window sizes. An earlier physical terminal zoom confirmation was followed by a new clipping report. Overall visual acceptance remains open.

## Composer spacing correction

The effort annotation now has a blank cell before its icon, separating it from the input rule. This corrects the visible border/icon contact; it does not establish that the entire reported clipping issue is resolved.

The updated local build passed 4,152 Rust TUI tests with six configured skips and 16 installed terminal checks. Resize coverage checks the effort label and editable prompt from 23 through 100 columns; installed checks verify the gap at 49, 60, and 120 columns. Narrow and wide terminal reconstructions were visually inspected. Physical terminal clipping remains unverified.

The initial Rust checks ran before the public source export. For the spacing correction, the TUI suite was compiled and rerun from the public checkout: all 4,152 tests passed, with six configured skips. Installer build arguments were previously checked with a stubbed package builder; installation was also exercised with the complete tested package. GNU Linux installation has not been validated.

The public snapshot was scanned with Gitleaks 8.30.1. Its 16 initial findings were reviewed as public upstream test fixtures, example identifiers, dependency names, or hash constants. `.gitleaksignore` records exact fingerprints with explanations. The scan then reported no unreviewed findings. Local account files, session history, review artifacts, and personal terminal screenshots are excluded.

The GIFs use scripted conversations and local tools. See [media provenance](media/README.md) for how they were recorded.
