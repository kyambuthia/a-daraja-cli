# Roadmap

This roadmap covers the next implementation steps for making `pesaraja` a profile-based CLI/SDK tool that starts with a personal M-Pesa account and then adds merchant accounts on top.

## Goal

Replace the current single-global `shortcode` model with explicit M-Pesa profiles, where:

- `pesaraja init` sets up the personal/root M-Pesa phone account by default.
- `pesaraja setup` adds merchant accounts such as PayBill, Till, and Pochi.
- `pesaraja use` selects the active profile.
- commands always make it clear which account context they are using.

## Current State

- The CLI supports one encrypted credential set plus one SDK key file.
- The current config model stores a single merchant identifier under `shortcode`.
- Commands use that single config implicitly.
- `balance`, `receive`, `register`, `simulate`, `prompt`, `send`, `query`, and `refund` operate against the implicit active config.
- JSON output exists for most inspection and response commands.
- `--dry-run` exists for the money-moving commands.

## Target Model

The tool should support:

- One root/personal M-Pesa profile created by `pesaraja init`.
- Zero or more merchant profiles created by `pesaraja setup`.
- One active profile at a time.
- Explicit profile-aware commands.
- Clear separation between:
  - personal wallet context
  - merchant account context
  - customer phone number
  - account/reference number

## Terminology

Use these user-facing names:

- `personal`: the root M-Pesa phone account.
- `paybill`: a merchant collection account.
- `till`: a merchant till account.
- `pochi`: a merchant collection account with Pochi semantics.
- `profile`: a named M-Pesa configuration.
- `active profile`: the profile that commands use when `--profile` is not provided.
- `merchant_number`: the PayBill / Till / Pochi business-side identifier.
- `customer_phone`: the M-Pesa phone number that pays or receives.
- `account_number`: the school/student/customer reference used for allocations.

Keep Daraja payload field names unchanged at the API boundary.

## Delivery Strategy

Build this in small, shippable slices:

- Each slice should leave the CLI usable.
- Prefer additive changes first.
- Preserve existing commands until the new profile flow is complete.
- Expose new behavior behind clear flags and commands before removing legacy paths.
- Ship profile listing and active-profile visibility before any destructive migration.

## Phase 1: Profile Foundation

### 1.1 Add profile storage

Implement a profile store in the CLI config layer.

Requirements:

- Persist multiple profiles.
- Persist one active profile pointer.
- Store the following per profile:
  - profile name
  - profile type: `personal`, `paybill`, `till`, or `pochi`
  - merchant number where relevant
  - environment: `sandbox` or `production`
  - consumer key
  - consumer secret
  - passkey
  - optional initiator name/password
  - callback URLs where relevant
  - certificate path or certificate resolution metadata

Recommended storage shape:

- Continue using `~/.config/daraja/`
- Store a profile registry file, for example:
  - `~/.config/daraja/profiles.json`
  - or `~/.config/daraja/profiles.enc` if all profile metadata must remain encrypted
- Keep secret material encrypted at rest.

### 1.2 Add a profile type enum

Define a profile type enum in both CLI and SDK-adjacent types:

- `personal`
- `paybill`
- `till`
- `pochi`

This type should drive defaults, labels, and validation.

### 1.3 Add active profile resolution

Implement resolution order:

1. `--profile <name>`
2. active profile from config
3. fail with a clear error if nothing is selected

Every command that depends on account context should use the same resolution logic.

### 1.4 Add read-only profile listing

Before changing command behavior, add:

- `pesaraja config` support for listing profiles
- `pesaraja --json config` support for the same list
- active profile display in human and JSON output

This is the first visible step and should land before setup migration.

## Phase 2: Root Account Setup

### 2.1 Make `init` create the personal profile

Change `pesaraja init` to be the default onboarding flow.

It should create the root/personal M-Pesa profile and ask for:

- profile name
- environment
- personal phone number / MSISDN
- consumer key
- consumer secret
- optional passkey if the flow needs it
- optional initiator credentials if relevant for the user’s setup

The output should clearly say:

- this is the personal/root account
- this is the active profile after init

### 2.2 Keep legacy init behavior as compatibility

If needed, keep the old single-config behavior as a compatibility path during migration, but make the personal-profile flow the default.

### 2.3 Add profile creation for root account

The personal profile should be stored exactly like other profiles, but it becomes the initial active profile.

This means:

- `pesaraja init` creates the first profile
- merchant profiles are optional additions

## Phase 3: Merchant Setup Commands

### 3.1 Introduce typed merchant setup commands

Add:

- `pesaraja setup paybill`
- `pesaraja setup till`
- `pesaraja setup pochi`

Each subcommand should create or update a merchant profile of that type.

The setup flow should ask for:

- profile name
- environment
- merchant number
- consumer key
- consumer secret
- passkey
- optional initiator credentials
- callback URLs where relevant

### 3.2 Add profile switching

Add:

- `pesaraja use <profile>`

This should set the active profile and print:

- active profile name
- type
- merchant number or personal phone number
- environment

### 3.3 Keep merchant setup additive

Merchant setup should not replace the root account.

It should:

- add a new profile
- leave the personal profile intact
- make switching explicit

## Phase 4: Action Commands

### 4.1 Make action commands profile-aware

Update these commands to use the active profile by default:

- `prompt`
- `prompt-status`
- `receive`
- `register`
- `simulate`
- `send`
- `query`
- `balance`
- `refund`
- `webhook`

### 4.2 Add `--profile` override to action commands

Allow every account-context command to target a specific profile explicitly.

Resolution order:

1. `--profile <name>`
2. active profile
3. fail with a helpful error

### 4.3 Add explicit context echo

Before running any action command, print:

- active profile name
- profile type
- merchant number or personal phone number
- environment

This prevents ambiguity in multi-profile usage.

## Phase 5: Balance and Account Semantics

### 5.1 Label balances by account type

Show balance output as:

- Personal M-Pesa balance
- PayBill balance
- Till balance
- Pochi balance

depending on the active profile type.

### 5.2 Clarify the separation of balances

Make it obvious in docs and CLI output that:

- the personal phone wallet balance is distinct from merchant balances
- the selected profile determines which account is queried

### 5.3 Show balance context in output

`pesaraja balance` should print:

- active profile
- profile type
- personal phone number or merchant number
- environment
- the fact that the result is the selected account balance

## Phase 6: SDK Alignment

### 6.1 Rename public config fields

Introduce user-facing field names that match the profile model:

- `profileName`
- `profileType`
- `merchantNumber`
- `personalPhoneNumber`
- `activeProfile`

Keep compatibility aliases for `shortcode` where needed during migration.

### 6.2 Add profile-aware constructors

Extend the Rust and TypeScript SDKs so they can:

- load a named profile
- use the active profile
- optionally override profile context per request

### 6.3 Preserve Daraja payload compatibility

Do not change:

- `ShortCode`
- `BusinessShortCode`
- `PartyA`
- `PartyB`
- `MSISDN`
- `BillRefNumber`

Only the user-facing configuration and CLI model should change.

## Phase 7: Backward Compatibility

### 7.1 Keep old commands working during migration

Keep the current commands functional while introducing the new model.

Examples:

- `pesaraja init` becomes the personal-account onboarding path.
- `--shortcode` can remain accepted as an alias for `--merchant-number`.

### 7.2 Add migration warnings

If the user still has legacy single-profile config, the CLI should:

- detect it
- migrate it into a personal profile when possible
- print a one-time warning

### 7.3 Add config migration helpers

Implement a migration path from:

- single `shortcode` config

to:

- personal profile
- additional merchant profiles
- active profile pointer

## Phase 8: Docs and UX

### 8.1 Update README

Document:

- personal profile onboarding via `init`
- merchant profile setup via `setup`
- active profile selection
- balance semantics
- supported profile types

### 8.2 Update CLI help text

Update `--help` output to reflect:

- `init`
- `setup`
- `use`
- profile selection
- `--profile`
- `--json`
- `--dry-run`

### 8.3 Add examples

Add examples for:

- personal onboarding
- PayBill school collections
- Till point-of-sale flows
- Pochi collection flows
- checking personal balance
- checking merchant balance
- switching profiles

## Phase 9: Tests

### 9.1 CLI tests

Add tests for:

- personal profile creation
- merchant profile creation
- profile switching
- config listing
- active profile echo
- JSON output schema
- dry-run short-circuiting

### 9.2 SDK tests

Add tests for:

- loading a named profile
- profile resolution precedence
- migration aliases
- merchant number terminology
- personal profile terminology

### 9.3 Regression tests

Protect current behavior while refactoring:

- existing single-config flows
- current JSON output
- current dry-run behavior
- phone normalization

## Implementation Order

Recommended sequence:

1. Add profile storage and profile type enum.
2. Add read-only profile listing in `config`.
3. Make `init` create the personal profile.
4. Add `setup paybill`, `setup till`, and `setup pochi`.
5. Add `use` and active profile switching.
6. Make one read-only command profile-aware end to end, then expand to all action commands.
7. Update balance semantics and labels.
8. Add `--profile` overrides.
9. Keep `shortcode` compatibility during migration.
10. Rename user-facing terminology to `merchant_number` and `personalPhoneNumber`.
11. Update SDK surfaces and docs.
12. Add migration helpers and tests.

## Acceptance Criteria by Milestone

### Milestone A

- Profiles can be listed.
- One active profile is visible.
- Legacy config still works.

### Milestone B

- `init` creates the personal profile.
- A user can create paybill/till/pochi profiles.
- A user can switch active profiles.

### Milestone C

- Action commands use the active profile.
- `--profile` overrides work.
- JSON output reflects profile state.

### Milestone D

- Personal balance and merchant balance are clearly distinguished.
- Legacy `shortcode` terminology is migrated to user-facing M-Pesa profile language.
- SDK and docs match the new model.

## Non-Goals

This roadmap does not add:

- Safaricom-side provisioning of PayBill, Till, or Pochi
- a ledger or business domain layer
- multi-tenant SaaS account management
- school or payroll business logic

It stays strictly within the CLI/SDK and M-Pesa profile model.
