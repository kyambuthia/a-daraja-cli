# Roadmap

This roadmap tracks the next implementation steps for the current codebase. It is intentionally short and stays aligned with the shipped CLI behavior.

## Current State

- `pesaraja init` creates the active `personal` profile.
- The CLI persists encrypted credentials and a local SDK key under `~/.config/daraja/`.
- `config` and `doctor` already expose the active profile state and setup health.
- The docs folder is current and points to install, setup, and usage guides.
- User-facing `setup`, `use`, and `--profile` flows are not implemented yet.

## Next Steps

### 1. Finish Profile Architecture

Make profile handling explicit and stable before adding more commands.

- Keep `personal` as the default active profile created by `init`.
- Keep profile registry loading, saving, and summary output consistent across the CLI.
- Make profile context resolution a single code path used by setup and action commands.
- Preserve the existing on-disk layout unless a migration is necessary.

### 2. Add the Setup Command Flow

Add a proper merchant setup flow on top of the existing personal profile flow.

- Implement `pesaraja setup paybill`.
- Implement `pesaraja setup till`.
- Implement `pesaraja setup pochi`.
- Support interactive and non-interactive setup for each merchant type.
- Validate merchant number, environment, passkey, and initiator credentials before saving.
- Persist each merchant profile without overwriting the active personal profile.

### 3. Add Profile Switching

Let users choose which saved profile the CLI should use.

- Implement `pesaraja use <profile>`.
- Store the selected profile as the active profile.
- Show the active profile name, type, identifier, and environment after switching.
- Add `--profile <name>` to commands that depend on account context.

### 4. Make Command Output Explicit

Remove ambiguity when commands run against different account types.

- Echo the resolved profile before account-sensitive commands run.
- Keep `config` output readable in human mode and stable in JSON mode.
- Make `doctor` report profile-specific issues, especially missing keys or certificates.
- Keep balance and transaction commands clear about which profile they used.

### 5. Tighten Tests

Protect the new flow as it lands.

- Add CLI tests for `init`, `setup`, `use`, `config`, and `doctor`.
- Add tests for profile resolution precedence.
- Add tests for JSON output stability.
- Add tests for non-interactive setup validation and failure cases.

### 6. Update Documentation Last

Keep the docs in sync with the implemented commands.

- Update the root README when the command surface changes.
- Update `docs/setup.md` with the final setup flow.
- Update `docs/usage.md` with profile-aware examples once `use` and `--profile` exist.

## Suggested Order

1. Finish profile resolution and registry handling.
2. Implement `setup paybill|till|pochi`.
3. Add `use <profile>` and `--profile`.
4. Make command output explicit.
5. Expand tests.
6. Refresh docs after the behavior is stable.
