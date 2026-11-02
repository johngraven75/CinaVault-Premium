# Contributing to CinaVault Premium

## Scope

This guide covers the Windows desktop app in this repository (Rust/Tauri 2,
React 19, SQLite, Windows installers). Other CinaVault repositories are
separate projects with their own contribution rules.

## Workflow

1. Work on a branch and open a pull request for review. Do not push
   directly to `main`.
2. Keep a change focused: one feature, fix, or docs cleanup per pull
   request, with the files it touches and the reason for each removal or
   rename stated in the pull request body.
3. Do not merge, and do not describe work as complete, while required
   checks are failing. A failing check is fixed or explained, never
   weakened away — in particular, do not weaken a security control or
   delete a meaningful test just to get a green build.
4. Preserve accepted functionality unless its removal is explicitly
   approved and documented in the pull request.

## Validation gate

Run the checks that apply to your change before opening the pull request:

```powershell
npm run lint        # TypeScript type check
npm test            # contract gates and Node test suites
cargo test --manifest-path src-tauri/Cargo.toml   # Rust tests
npm run verify:contracts
npm run scan:preventive
```

Docs-only changes still go through a pull request and the same merge
rules; check every internal link you change, and do not claim a pull
request is merged or released until it actually is.

## Engineering rules

The full rules live in [AGENTS.md](AGENTS.md): define the purpose and
contracts before implementing, keep presentation, transport, and domain
logic separated, and report front end, integration, and back end changes
separately (or state explicitly that a layer does not apply).

## Secrets

Never commit provider keys, Azure signing secrets, API keys, PINs, or
credential-store contents — see [SECURITY.md](SECURITY.md). If a secret is
leaked, revoke or rotate it immediately and report it privately through
the repository's Security tab.

## Reporting a vulnerability

Do not open a public issue for a security problem. Report it privately
through the repository's **Security** tab, as described in
[SECURITY.md](SECURITY.md).
