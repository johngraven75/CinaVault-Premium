// CinaVault Premium — Siri link-up helper.
//
// The CinaVault iOS app is a Tauri (WKWebView) build of the same React/Rust
// product as Windows, so there is no native Swift view layer to drive. Siri
// requests therefore reach the running web app as `cinavault://…` deep links,
// which the in-app router (src/services/siriBridge.ts) turns into real actions.
//
// SiriLink is the one bridge both halves agree on:
//   • Every App Intent builds a `cinavault://` URL here and calls `deliver`.
//   • `deliver` stores the URL in the shared app group and, when the app is
//     already foregrounded, opens it immediately.
//   • The generated Tauri app delegate drains the stored URL on launch / when
//     it becomes active and hands it to `window.__cinavaultSiriIntent(url)`
//     (see README.md for the two-line wiring).
//
// Nothing here calls the network: App Intents resolve locally, exactly as the
// brief requires.

import Foundation
#if canImport(UIKit)
import UIKit
#endif

enum SiriLink {
    /// Custom URL scheme the app registers (CFBundleURLTypes in Info.plist).
    static let scheme = "cinavault"
    /// App group shared between the App Intents and the WKWebView app process.
    static let appGroup = "group.com.cinavault.premium"
    /// Key under which the most recent pending intent URL is stored.
    static let pendingKey = "siri.pendingIntentURL"

    /// Builds `cinavault://<action>?<param>=<value>` with proper escaping.
    static func url(action: String, param: String? = nil, value: String? = nil) -> URL {
        var components = URLComponents()
        components.scheme = scheme
        components.host = action
        if let param, let value {
            components.queryItems = [URLQueryItem(name: param, value: value)]
        }
        // `components.url` only returns nil for a malformed scheme/host, which
        // our fixed inputs never are; fall back defensively all the same.
        return components.url ?? URL(string: "\(scheme)://\(action)")!
    }

    /// Hands a built URL to the app: persisted for the webview to drain, and
    /// opened immediately when the app is already in the foreground.
    static func deliver(_ url: URL) {
        if let defaults = UserDefaults(suiteName: appGroup) {
            defaults.set(url.absoluteString, forKey: pendingKey)
        }
        #if canImport(UIKit)
        DispatchQueue.main.async {
            UIApplication.shared.open(url, options: [:], completionHandler: nil)
        }
        #endif
    }

    /// Drains and clears the pending URL. The app delegate calls this on launch
    /// and on `applicationDidBecomeActive`, then evaluates
    /// `window.__cinavaultSiriIntent(<url>)` in the webview.
    static func takePending() -> String? {
        guard let defaults = UserDefaults(suiteName: appGroup) else { return nil }
        guard let value = defaults.string(forKey: pendingKey) else { return nil }
        defaults.removeObject(forKey: pendingKey)
        return value
    }
}
