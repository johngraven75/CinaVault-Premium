# Security policy

## Supported versions

| Version | Supported |
|---|---|
| 2.1.7 Build 217 (published as GitHub prerelease `v2.1.7-beta-2`) | Yes |
| All earlier builds | No |

If you are on an earlier build, upgrade to the supported release before
reporting: a problem that only affects an unsupported build will normally be
answered with an upgrade, not a backported fix.

## Reporting a vulnerability

Report security issues **privately** through GitHub: open this repository's
**Security** tab and choose **Report a vulnerability** (GitHub Security
Advisories / private vulnerability reporting). Do not open a public issue,
pull request, or discussion for a security problem, and do not post a
proof of concept publicly before a fix is released.

There is no security contact email in this repository's records, so the
Security tab route above is the reporting route — do not guess an email
address for it.

Include, without including any secret or credential:

- The version (Settings > About) and the installer you used
- Steps to reproduce the problem
- Whether it needs local access, a LAN position, or remote access
- What an attacker could do with it (read, change, or delete data; run
  code; bypass parental controls or authentication; and so on)

You will get an acknowledgement once the report is confirmed as received.
A fix ships in the next build once the report is confirmed as a real
vulnerability. No fixed disclosure timeline is promised in this
repository's records, and reporters are asked not to publish details until
a fixed release is available (coordinated disclosure).

## What the project will and will not do

- **Will:** confirm reports, fix confirmed vulnerabilities in the supported
  version, and credit the reporter in the release notes if they want credit.
- **Will not:** pay a bounty (there is no bounty program in this
  repository's records), backport fixes to unsupported builds as a matter
  of course, or treat a report about a user's own media files, a
  third-party provider's service, or another repository as in scope for
  this repository.

### Scope

In scope: this Windows desktop app, its installers, and its embedded media
server (authentication, API keys, remote access, parental controls, and the
loopback player stream). Out of scope: other CinaVault repositories, the
user's own files and network, and third-party services the app talks to.

## Secrets and signing credentials — never commit them

Never commit any of the following to this repository, in code, docs,
tests, fixtures, or build output:

- Metadata provider keys (TMDB, OMDb, Fanart, adult providers, and others)
- Azure Artifact Signing secrets or variables (`AZURE_TENANT_ID`,
  `AZURE_CLIENT_ID`, `AZURE_CLIENT_SECRET`, and the signing account,
  endpoint, and certificate profile names)
- API keys, parental PINs, or credential-store contents

Provider keys belong in the Windows credential store (entered in the
first-run wizard), and signing credentials belong in GitHub Actions
secrets/variables only. `.env` files are ignored by `.gitignore`; the one
exception, `.env.store-safe`, contains only the `VITE_STORE_SAFE=true`
edition flag and no secret.

If a secret is ever committed or otherwise leaked: revoke or rotate it
immediately, then report it privately through the Security tab route
above. Deleting a commit from a branch does not reliably remove a secret
from Git history, so rotation — not deletion alone — is the fix.

## How the app protects you

- **Secrets:** metadata provider keys and the parental PIN are never stored
  in this repository. Keys are kept in the Windows credential store, and
  the PIN and API keys are stored only as salted hashes.
- **Media server:** remote clients must sign in or present an API key,
  router ports and the relay open only when you start remote
  connectivity, and turning off the Remote Access switch refuses remote
  clients outright.
- **Playback:** the built-in player streams over loopback with a
  per-session token.
- **Supply chain:** dependencies are locked, CodeQL runs on every pull
  request, and a scheduled job audits dependencies weekly.
- **Installers:** installers are currently published **unsigned** because
  the Azure Artifact Signing secrets are not configured, so Windows
  SmartScreen will warn on first run. Signing turns on as soon as those
  secrets are configured (see [docs/CODE_SIGNING.md](docs/CODE_SIGNING.md));
  an unsigned installer is not, by itself, evidence that the installer
  was tampered with — verify it against the release's `SHA256SUMS.txt`.
