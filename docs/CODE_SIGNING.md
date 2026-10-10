# Code signing the Windows installers

Unsigned installers make Windows show "Windows protected your PC" with an **Unknown publisher**. The release workflow (`.github/workflows/windows-installer.yml`) signs the app executable, the NSIS installer and the MSI with **Azure Artifact Signing**, Microsoft's code-signing service (formerly Trusted Signing). The signing key stays in Microsoft's cloud HSM, so there is no certificate file to protect.

## How it works

1. `scripts/configure-windows-signing.ps1` checks the six settings below. If all of them are present, it writes a Tauri config overlay that sets `bundle.windows.signCommand`. If any is missing, the build stays unsigned and the run shows a warning naming what is missing.
2. `tauri build --config <overlay>` calls `scripts/sign-windows.ps1` for every file it signs. That script calls `artifact-signing-cli` (installed in the workflow, pinned to 0.11.0). The CLI timestamps each signature through `http://timestamp.acs.microsoft.com`.
3. `scripts/verify-windows-signatures.ps1` fails the release if any binary lacks a valid, timestamped signature.

Local builds and the MS Store edition are unaffected; only the release workflow adds the overlay.

## One-time setup (owner)

1. In the Azure portal, create an **Artifact Signing account** and complete **identity validation** for the publisher name that Windows should show. Validation is done by Microsoft and can take a few days.
2. Create a **Public Trust** certificate profile in that account.
3. Register an app in Microsoft Entra ID, create a client secret, and give the app the **Artifact Signing Certificate Profile Signer** role on the account.
4. In GitHub, go to **Settings > Secrets and variables > Actions** and add:

| Kind | Name | Value |
| --- | --- | --- |
| Secret | `AZURE_TENANT_ID` | Directory (tenant) ID of the app registration |
| Secret | `AZURE_CLIENT_ID` | Application (client) ID |
| Secret | `AZURE_CLIENT_SECRET` | The client secret value |
| Variable | `AZURE_ARTIFACT_SIGNING_ENDPOINT` | The account's region endpoint, for example `https://eus.codesigning.azure.net` |
| Variable | `AZURE_ARTIFACT_SIGNING_ACCOUNT` | Artifact Signing account name |
| Variable | `AZURE_ARTIFACT_SIGNING_CERTIFICATE_PROFILE` | Certificate profile name |

The next tagged release (or a manual run of the workflow) is then signed.

## What users see

- Signed installers show your validated name instead of "Unknown publisher".
- SmartScreen builds reputation per publisher. A brand-new signing identity can still get the blue SmartScreen prompt on its first releases, until enough people have installed it. Signing is what lets that reputation build at all.

## Using a different certificate

Certificates from other authorities (DigiCert, Sectigo, SSL.com and others) now also keep their keys in hardware or a cloud HSM, each with its own signing tool. To switch, replace the `artifact-signing-cli` call in `scripts/sign-windows.ps1` and the settings it checks; the workflow wiring stays the same.
