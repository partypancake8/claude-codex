# Validation

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

The terminal checks use isolated local response fixtures. Rendered terminal captures cover startup, conversation, composer, status, agents, menus, and window sizes. Physical terminal zoom was separately confirmed fixed by the user.

The Rust checks ran before the public source export. Publication added documentation, demo media, the installer, and comment cleanup. A fresh compilation of the exported checkout was not repeated. Installer build arguments were checked with a stubbed package builder; installation was also exercised with the complete tested package. GNU Linux installation has not been validated.

The public snapshot was scanned with Gitleaks 8.30.1. Its 16 initial findings were reviewed as public upstream test fixtures, example identifiers, dependency names, or hash constants. `.gitleaksignore` records exact fingerprints with explanations. The scan then reported no unreviewed findings. Local account files, session history, review artifacts, and personal terminal screenshots are excluded.

The GIFs use scripted conversations and local tools. See [media provenance](media/README.md) for how they were recorded.
