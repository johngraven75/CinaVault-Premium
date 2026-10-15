// CinaVault Premium — App Intents unit tests.
//
// These cover the happy path of each intent's URL building plus the shared-
// app-group deliver/drain round trip, without needing the running webview.
// They compile and run on macOS/Xcode as part of the app's test target (see
// README.md); they are not run in the cloud CI container, which has no Swift
// toolchain or Xcode.

import XCTest
import AppIntents
@testable import cinavault

final class CinaVaultIntentsTests: XCTestCase {
    override func setUp() {
        super.setUp()
        UserDefaults(suiteName: SiriLink.appGroup)?.removeObject(forKey: SiriLink.pendingKey)
    }

    func testPlayIntentDeliversEncodedTitle() async throws {
        let intent = PlayItemIntent()
        intent.title = "Blade Runner 2049"
        _ = try await intent.perform()
        let pending = SiriLink.takePending()
        XCTAssertEqual(pending, "cinavault://play?title=Blade%20Runner%202049")
    }

    func testSearchIntentDeliversEncodedQuery() async throws {
        let intent = SearchLibraryIntent()
        intent.query = "sci fi"
        _ = try await intent.perform()
        XCTAssertEqual(SiriLink.takePending(), "cinavault://search?q=sci%20fi")
    }

    func testAskIntentRoutesQuestion() async throws {
        let intent = AskHoloIntent()
        intent.question = "what is in my library?"
        _ = try await intent.perform()
        XCTAssertEqual(SiriLink.takePending(), "cinavault://ask?q=what%20is%20in%20my%20library%3F")
    }

    func testTransportIntents() async throws {
        _ = try await PauseIntent().perform()
        XCTAssertEqual(SiriLink.takePending(), "cinavault://pause")
        _ = try await ResumeIntent().perform()
        XCTAssertEqual(SiriLink.takePending(), "cinavault://resume")
        _ = try await SkipIntent().perform()
        XCTAssertEqual(SiriLink.takePending(), "cinavault://skip")
    }

    func testPendingDrainsOnce() {
        SiriLink.deliver(SiriLink.url(action: "pause"))
        XCTAssertEqual(SiriLink.takePending(), "cinavault://pause")
        XCTAssertNil(SiriLink.takePending(), "pending URL must clear after being taken")
    }

    func testShortcutsCoverEveryIntent() {
        XCTAssertEqual(CinaVaultShortcuts.appShortcuts.count, 6)
    }
}
