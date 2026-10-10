# Security policy

## Supported versions

| Version | Supported |
|---|---|
| 1.0.2 Pre-Beta (Build 102) | Yes |
| 1.0.1 Pre-Beta (Build 101) | No |
| Earlier builds | No |

## Reporting a vulnerability

Please report security issues privately through GitHub: open the repository's **Security** tab and choose **Report a vulnerability**. Do not open a public issue for a security problem.

Include the version (Settings > About), the steps to reproduce the problem, and what an attacker could do with it. You will get an acknowledgement, and a fix will ship in the next build once the report is confirmed.

## How the app protects you

- **Secrets:** metadata provider keys and the parental PIN are never stored in this repository. Keys are kept in the Windows credential store, and the PIN and API keys are stored only as salted hashes.
- **Media server:** remote clients must sign in or present an API key, router ports and the relay open only when you start remote connectivity, and turning off the Remote Access switch refuses remote clients outright.
- **Playback:** the built-in player streams over loopback with a per-session token.
- **Supply chain:** dependencies are locked, CodeQL runs on every pull request, and a scheduled job audits dependencies weekly.
