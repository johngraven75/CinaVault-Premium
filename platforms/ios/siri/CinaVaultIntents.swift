// CinaVault Premium — Siri App Intents (iOS 16+, App Intents framework).
//
// These are the modern, non-deprecated App Intents API (not the legacy SiriKit
// INIntent classes). Each intent opens the app and hands the running web app a
// `cinavault://` deep link through SiriLink; the in-app router
// (src/services/siriBridge.ts) does the real work — fuzzy title match, search,
// transport control, or routing a question into the holo agent. Resolution is
// entirely local; no network call is made to build or run an intent.
//
// Titles are taken as spoken strings and matched in-app, so the intents work on
// a fresh install with no pre-indexed catalogue. (A future enhancement can add
// an AppEntity backed by an app-group title cache for richer Siri suggestions;
// see README.md — it is intentionally not stubbed in here.)

import AppIntents

// MARK: Play

struct PlayItemIntent: AppIntent {
    static var title: LocalizedStringResource = "Play on CinaVault"
    static var description = IntentDescription("Play a title from your CinaVault library.")
    static var openAppWhenRun: Bool = true

    @Parameter(title: "Title", requestValueDialog: "What would you like to play?")
    var title: String

    @MainActor
    func perform() async throws -> some IntentResult & ProvidesDialog {
        SiriLink.deliver(SiriLink.url(action: "play", param: "title", value: title))
        return .result(dialog: "Playing \(title) on CinaVault.")
    }
}

// MARK: Search

struct SearchLibraryIntent: AppIntent {
    static var title: LocalizedStringResource = "Search CinaVault"
    static var description = IntentDescription("Search your CinaVault library.")
    static var openAppWhenRun: Bool = true

    @Parameter(title: "Query", requestValueDialog: "What should I search for?")
    var query: String

    @MainActor
    func perform() async throws -> some IntentResult & ProvidesDialog {
        SiriLink.deliver(SiriLink.url(action: "search", param: "q", value: query))
        return .result(dialog: "Searching CinaVault for \(query).")
    }
}

// MARK: Transport

struct PauseIntent: AppIntent {
    static var title: LocalizedStringResource = "Pause CinaVault"
    static var description = IntentDescription("Pause playback in CinaVault.")
    static var openAppWhenRun: Bool = true

    @MainActor
    func perform() async throws -> some IntentResult {
        SiriLink.deliver(SiriLink.url(action: "pause"))
        return .result()
    }
}

struct ResumeIntent: AppIntent {
    static var title: LocalizedStringResource = "Resume CinaVault"
    static var description = IntentDescription("Resume playback in CinaVault.")
    static var openAppWhenRun: Bool = true

    @MainActor
    func perform() async throws -> some IntentResult {
        SiriLink.deliver(SiriLink.url(action: "resume"))
        return .result()
    }
}

struct SkipIntent: AppIntent {
    static var title: LocalizedStringResource = "Skip on CinaVault"
    static var description = IntentDescription("Skip to the next title in CinaVault.")
    static var openAppWhenRun: Bool = true

    @MainActor
    func perform() async throws -> some IntentResult {
        SiriLink.deliver(SiriLink.url(action: "skip"))
        return .result()
    }
}

// MARK: Ask the holo agent

struct AskHoloIntent: AppIntent {
    static var title: LocalizedStringResource = "Ask CinaVault"
    static var description = IntentDescription(
        "Ask the CinaVault holographic agent a question. CinaVault opens and speaks the answer."
    )
    static var openAppWhenRun: Bool = true

    @Parameter(title: "Question", requestValueDialog: "What would you like to ask CinaVault?")
    var question: String

    @MainActor
    func perform() async throws -> some IntentResult & ProvidesDialog {
        SiriLink.deliver(SiriLink.url(action: "ask", param: "q", value: question))
        return .result(dialog: "Asking CinaVault. The agent will answer out loud.")
    }
}
