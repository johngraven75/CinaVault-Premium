// CinaVault Premium — App Shortcuts registration.
//
// AppShortcutsProvider makes every intent discoverable in the Shortcuts app,
// Spotlight and "Siri Suggestions" with a spoken phrase, no user setup. Each
// phrase MUST contain \(.applicationName); iOS matches "CinaVault" (and the
// bundle display name) in the spot that token marks.

import AppIntents

struct CinaVaultShortcuts: AppShortcutsProvider {
    static var appShortcuts: [AppShortcut] {
        AppShortcut(
            intent: PlayItemIntent(),
            phrases: [
                "Play \(.applicationName)",
                "Play on \(.applicationName)",
                "Play \(\.$title) on \(.applicationName)"
            ],
            shortTitle: "Play",
            systemImageName: "play.circle"
        )
        AppShortcut(
            intent: SearchLibraryIntent(),
            phrases: [
                "Search \(.applicationName)",
                "Search for \(\.$query) on \(.applicationName)",
                "Find \(\.$query) on \(.applicationName)"
            ],
            shortTitle: "Search",
            systemImageName: "magnifyingglass"
        )
        AppShortcut(
            intent: PauseIntent(),
            phrases: [
                "Pause \(.applicationName)",
                "Pause playback on \(.applicationName)"
            ],
            shortTitle: "Pause",
            systemImageName: "pause.circle"
        )
        AppShortcut(
            intent: ResumeIntent(),
            phrases: [
                "Resume \(.applicationName)",
                "Continue playback on \(.applicationName)"
            ],
            shortTitle: "Resume",
            systemImageName: "play.circle"
        )
        AppShortcut(
            intent: SkipIntent(),
            phrases: [
                "Skip on \(.applicationName)",
                "Next on \(.applicationName)"
            ],
            shortTitle: "Skip",
            systemImageName: "forward.end"
        )
        AppShortcut(
            intent: AskHoloIntent(),
            phrases: [
                "Ask \(.applicationName)",
                "Ask \(.applicationName) \(\.$question)"
            ],
            shortTitle: "Ask the agent",
            systemImageName: "sparkles"
        )
    }
}
