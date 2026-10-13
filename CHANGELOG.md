# Changelog

## Unreleased

## v1.0.5 Public Beta · Build 105

### Changed

- **The library only uses adult metadata providers now.** On the full edition, library scans and metadata updates read from ThePornDB, StashDB and the adult fallbacks (Porn Site Nuxt, IAFD, PhoenixAdult, PGMA). TMDB, OMDb, Cinemeta and TVMaze are never called, and first-run setup only asks for the adult provider keys. The Microsoft Store edition keeps the regular providers.

- **The app reads your posters with free AI vision.** After each scan, a bundled offline model (CLIP) reads every poster to fill in empty genre and year fields without overwriting anything you already have. With a Hugging Face token it also reads printed titles and years with Qwen3-VL. The holo agent can now look at an attached image and describe it with no paid key.

### New

- **AI vision reads your posters for free.** After each scan, CinaVault looks at every new poster with a free AI model that runs on your computer and fills in a missing genre (and, with a free Hugging Face token, a missing year printed on the poster). Genres and years from TMDB, OMDb, NFO files or your own edits are never replaced. Adult artwork is only ever read on your computer.
- **The agent can look at pictures without an Anthropic key.** Attach or paste an image in the agent panel and it describes what it sees. With a free Hugging Face token it can also answer questions about the image, like what title is printed on a poster.

## v1.0.4 Pre-Beta · Build 104

### New

- **Your phone can talk to the agent.** The CinaVault Android and iOS apps can now chat with the agent through your server once they're signed in. From a phone the agent can search and describe what's in the library, but it can't play, rename or change anything. It uses the API key on your computer, which never leaves it. Each account gets 40 messages an hour, and phones also get your agent face, if you've set one up.

### Fixed

- **API keys you enter now stay saved.** Every pre-beta build accepted TMDB, OMDb, ThePornDB, StashDB and other provider keys, and the agent's Anthropic key, but never actually stored them in the Windows Credential Manager, so they were gone right away. That is why the provider check said 0 providers were configured and why lookups that need a key found nothing. Keys are now saved in the Credential Manager and checked after saving. **Please enter your provider keys once more** after updating, since keys saved in earlier builds were not kept. The provider check now also says how many providers are ready, including the ones that need no key, and names any key that has to be re-entered.
- **Scanned titles now keep their metadata and posters.** A title whose lookup never finished, for example because the app was closed while it ran in the background or the provider was offline, was never looked up again, so it stayed without a poster or details however many times you rescanned. Every scan now retries those titles. Posters from TMDB and OMDb are saved in CinaVault's own artwork folder instead of being kept as web links, and when a poster can't be written next to the video (a read-only drive, a NAS share or a protected Windows folder) it is saved in that artwork folder instead of being dropped.

## v1.0.3 Pre-Beta · Build 103

### New

- **The holographic agent can wear your face.** In the agent panel, open the gear icon and choose **Use my photo** under "Agent face", then drag the three markers onto your eyes and mouth. The agent's head becomes your photo, animated on your computer: it blinks, turns slightly, raises its brows while listening, looks up while thinking and moves its jaw as it answers. The photo is saved in the app's data folder with location data removed and is never uploaded. **Remove photo** brings back the hologram head.

## v1.0.2 Pre-Beta · Build 102

### New

- **The holographic agent can now use Claude.** Add your Anthropic API key under the gear icon in the agent panel and pick Opus, Sonnet or Haiku. It searches your library, looks at posters and images you attach or paste, runs source, provider and network checks, and offers buttons to play, rename, refresh metadata, mark watched, add discovered folders or organize the library. Nothing changes until you press a button. Your key stays in the system keychain and never reaches the app window. Without a key, the agent keeps its offline answers.
- **Each profile gets its own agent conversation**, and the agent only sees titles the active profile's parental controls allow.

### Fixed

- **MediaInfo no longer freezes the app or pops up on screen.** On Windows the startup tool check could open the MediaInfo desktop program and wait until you closed it. CinaVault now only uses the hidden command-line version, runs every media tool in the background with a time limit, and opens the window without waiting for tool setup to finish.

## v1.0.1 Pre-Beta · Build 101

The first pre-beta of CinaVault Premium for Windows.

### New

- **Every Feature Matrix switch works.** All 40 switches in Settings > Advanced change real behavior, each has its own settings panel where it needs one, and a test fails the build if any switch is left unwired.
- **Built-in player.** It plays files directly when it can and transcodes on the fly (with NVENC, Quick Sync or AMF) when it can't. It adds resume, skip intro and credits, a Next Up countdown, gapless playback, audio crossfade, cinema mode and buffering control.
- **Discovery shelves:** Continue Watching, Watchlist, Trending, Recommendations, Similar Titles, Genre Radio and New Releases.
- **Library automation:** smart title matching, Kodi `.nfo` import, metadata lookup after scans, OpenSubtitles downloads, chapter thumbnails, a poster sync folder and automatic collections.
- **Subtitles in the built-in player:** `.srt` and `.vtt` files next to a title, including ones Auto Subtitle Download saves, show up behind a CC button, and stay in sync after seeking a transcoded stream.
- **Households:**
  - Profiles with their own progress and watchlist, plus a profile switcher.
  - Parental controls based on content ratings, behind a PIN.
  - An activity log and webhooks.
- **Media server:**
  - A Remote Access switch, and API keys you can issue and revoke.
  - A per-stream bandwidth cap and a cache layer for artwork and the library.
  - A GPU acceleration setting for the app window.
- **Holographic 3D AI agent** that can search, play and manage the library.
- **Offline vision model** that identifies films and checks posters with no key or network.
- **Unified library engine**, which shows each work once across every source, and a first-run wizard for provider keys.

### Changed

- The paywall has been removed, so every feature is available.
- Installers are signed with Azure Artifact Signing once the signing secrets are configured, and are unsigned until then.

### Fixed

- Rescans no longer reset the titles of verified or already-identified items to the filename.
- Windows CI now checks out text files with LF line endings, which fixed source-reading tests that failed on Windows.
- The SonarCloud workflow file was invalid and failed on every run. It now skips cleanly when SonarCloud isn't configured.

### Repository

- Docs are organized under `docs/`, with an index, and past build reports are moved to `docs/history/`.
- A new README with architecture, scan, playback and release diagrams.
- Old installers, build artifacts, test logs and one-off release workflows from builds 140 to 170 are removed; they remain in git history.
