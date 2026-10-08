# Adult Provider Research Notes

## ThePornDB
The official ThePornDatabase/Jellyfin.Plugin.ThePornDB repository describes a Jellyfin/Emby metadata provider for adult videos that pulls from ThePornDB. Its README documents repository or manual installation and links to the current release. This supports treating ThePornDB as an adult metadata provider, but the CinaVault configuration still requires a usable API credential for direct runtime calls.

Source: https://github.com/ThePornDatabase/Jellyfin.Plugin.ThePornDB

## StashDB
The official StashDB Guidelines state that access requires an account and a unique API key. The documented endpoint is `https://stashdb.org/graphql`, and the key is entered with the endpoint when connecting a Stash instance. Therefore, an empty API key in CinaVault is not a ready-to-use StashDB integration.

Source: https://guidelines.stashdb.org/docs/faq_getting-started/stashdb/accessing-stashdb/

## Engineering implication
Only providers that are actually implemented in CinaVault and have the required credentials or local service available can be truthfully marked ready. Manifest-only providers, empty-key API providers, or an uninstalled localhost scraper must remain explicitly not-ready rather than being presented as working.
