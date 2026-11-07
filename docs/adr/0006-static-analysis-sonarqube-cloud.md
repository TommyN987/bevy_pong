# ADR 0006: Static analysis with SonarQube Cloud

Date: 2026-11-08
Status: accepted

## Context

The project holds itself to a commercial quality bar (CLAUDE.md); its gates were clippy pedantic with warnings denied, the tests, `cargo-deny`, and GitGuardian on PRs.
None of them reads the CI workflow, the shell hooks, or duplication across files.
The user created a SonarQube Cloud project for the repository on 2026-11-07 and installed SonarSource's Claude Code plugin.

- SonarQube Cloud analyses this repository's Rust, shell, and YAML; it does not analyse `just` (4,905, 116, and 53 lines on 2026-11-08).
- Its first PR analysis (PR 5) failed the quality gate on rule `githubactions:S7637`: CI actions referenced by branch or tag rather than commit.

## Decision

SonarQube Cloud analyses every push and PR through its automatic analysis, and its quality gate reports on each PR.
Only one option was considered: the user chose SonarQube Cloud.

- Project `TommyN987_bevy_pong` in organization `tommyn987`, analysed by the GitHub app; no scanner runs in CI.
- The gate is SonarQube Cloud's default on new code: security, reliability, and maintainability rated A, at most 3% duplication, every security hotspot reviewed.
- `sonar-project.properties` at the root holds the project identity for the SonarQube CLI and skills; automatic analysis ignores it, and its own settings would go in `.sonarcloud.properties`, which reads only an allow list of keys.
- Claude Code reaches SonarQube through the `sonar` CLI (`sonar integrate claude`): an MCP server for issues and rules, and hooks that scan prompts and files for secrets. Both live in the user's Claude Code config, not in this repository.

## Alternatives considered

None; the user chose SonarQube Cloud.

## Consequences

- Third-party CI actions are pinned to full commit SHAs (phase 4 report); moving a pin is a deliberate change.
- Shell hooks follow its shell rules (`[[` over `[`, fixed 2026-11-08).
- No coverage condition: automatic analysis receives no coverage report. Adding one needs a CI scanner instead.
- Not a Cargo dependency: nothing changes the build or the Bevy treadmill.
- On the development machine the CLI's token lives in the system keychain (KWallet), which must be running for the CLI and the MCP server.
- The SonarQube MCP server runs in a container: Docker must be running for Claude Code to reach it.
