package com.cinavault.premium.model

enum class Section(val label: String, val glyph: String) {
    Home("Home", "⌂"),
    Sources("Sources", "+"),
    Downloads("Downloads", "↓"),
    LiveTv("Live TV", "▣"),
    Server("Server", "●"),
    Security("Security", "◆"),
    Remote("Remote", "⇄"),
    Advanced("Advanced", "⚙"),
    CloudNas("Cloud NAS", "☁"),
    Plugins("Plugins", "◇"),
    Ai("AI", "✦"),
    Settings("Settings", "◉")
}

data class MetricCard(
    val title: String,
    val value: String,
    val detail: String,
    val trend: String
)

data class MediaSource(
    val name: String,
    val path: String,
    val type: String,
    val enabled: Boolean,
    val items: Int
)

data class FeatureToggle(
    val id: String,
    val title: String,
    val description: String,
    val enabled: Boolean
)

data class StatusEvent(
    val title: String,
    val detail: String,
    val tone: EventTone = EventTone.Info
)

enum class EventTone {
    Info,
    Success,
    Warning,
    Critical
}

data class ServerProfile(
    val name: String,
    val url: String,
    val running: Boolean,
    val secureConnections: String,
    val uploadLimitMbps: Int
)

data class CinaVaultState(
    val selectedSection: Section = Section.Home,
    val sidebarCollapsed: Boolean = false,
    val serverProfile: ServerProfile = ServerProfile(
        name = "CinaVault Premium Server",
        url = "http://localhost:8096",
        running = false,
        secureConnections = "Preferred",
        uploadLimitMbps = 20
    ),
    val vpnConnected: Boolean = false,
    val vpnLocation: String = "Not connected",
    val scanning: Boolean = false,
    val scanProgress: Float = 0f,
    val aiProcessing: Boolean = false,
    val currentTheme: String = "VidHub Flagship",
    val qualityMode: String = "Auto",
    val libraryView: String = "Card",
    val metrics: List<MetricCard> = emptyList(),
    val sources: List<MediaSource> = emptyList(),
    val features: List<FeatureToggle> = emptyList(),
    val events: List<StatusEvent> = emptyList()
)
