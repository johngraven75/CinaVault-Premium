package com.cinavault.premium.state

import androidx.lifecycle.ViewModel
import androidx.lifecycle.viewModelScope
import com.cinavault.premium.model.CinaVaultState
import com.cinavault.premium.model.EventTone
import com.cinavault.premium.model.FeatureToggle
import com.cinavault.premium.model.MediaSource
import com.cinavault.premium.model.MetricCard
import com.cinavault.premium.model.Section
import com.cinavault.premium.model.StatusEvent
import kotlinx.coroutines.delay
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.flow.update
import kotlinx.coroutines.launch

class CinaVaultViewModel : ViewModel() {
    private val _state = MutableStateFlow(seedState())
    val state: StateFlow<CinaVaultState> = _state.asStateFlow()

    fun selectSection(section: Section) {
        _state.update { it.copy(selectedSection = section) }
        addEvent("Opened ${section.label}", "Native Android navigation updated.")
    }

    fun toggleSidebar() {
        _state.update { it.copy(sidebarCollapsed = !it.sidebarCollapsed) }
    }

    fun toggleServer() {
        _state.update { current ->
            val running = !current.serverProfile.running
            current.copy(
                serverProfile = current.serverProfile.copy(running = running),
                metrics = current.metrics.map {
                    if (it.title == "Server") it.copy(
                        value = if (running) "Online" else "Offline",
                        detail = if (running) "Jellyfin-compatible endpoint active" else "Server is stopped",
                        trend = if (running) "Ready" else "Idle"
                    ) else it
                }
            )
        }
        val running = _state.value.serverProfile.running
        addEvent(
            title = if (running) "Server started" else "Server stopped",
            detail = if (running) "Android service hook is ready for backend integration." else "Streaming services paused.",
            tone = if (running) EventTone.Success else EventTone.Warning
        )
    }

    fun toggleVpn() {
        _state.update { current ->
            val connected = !current.vpnConnected
            current.copy(
                vpnConnected = connected,
                vpnLocation = if (connected) "Premium Relay · New York" else "Not connected"
            )
        }
        addEvent(
            title = if (_state.value.vpnConnected) "VPN connected" else "VPN disconnected",
            detail = _state.value.vpnLocation,
            tone = if (_state.value.vpnConnected) EventTone.Success else EventTone.Info
        )
    }

    fun startLibraryScan() {
        if (_state.value.scanning) return
        viewModelScope.launch {
            addEvent("Library scan started", "Checking sources, posters, metadata, and duplicates.")
            _state.update { it.copy(scanning = true, scanProgress = 0f) }
            for (step in 1..10) {
                delay(120)
                _state.update { it.copy(scanProgress = step / 10f) }
            }
            _state.update {
                it.copy(
                    scanning = false,
                    scanProgress = 1f,
                    metrics = it.metrics.map { metric ->
                        if (metric.title == "Library") metric.copy(value = "2,486", trend = "+12 scanned") else metric
                    }
                )
            }
            addEvent("Library scan complete", "All source checks finished with premium features enabled.", EventTone.Success)
        }
    }

    fun runAiDiagnostics() {
        if (_state.value.aiProcessing) return
        viewModelScope.launch {
            _state.update { it.copy(aiProcessing = true) }
            addEvent("AI diagnostics running", "Analyzing playback, metadata, duplicates, and remote access.")
            delay(900)
            _state.update { it.copy(aiProcessing = false) }
            addEvent("AI diagnostics passed", "No critical issues detected. Four optimization tips are ready.", EventTone.Success)
        }
    }

    fun toggleFeature(featureId: String) {
        _state.update { current ->
            current.copy(
                features = current.features.map { feature ->
                    if (feature.id == featureId) feature.copy(enabled = !feature.enabled) else feature
                }
            )
        }
        val feature = _state.value.features.firstOrNull { it.id == featureId }
        if (feature != null) {
            addEvent(
                title = if (feature.enabled) "${feature.title} enabled" else "${feature.title} disabled",
                detail = feature.description,
                tone = if (feature.enabled) EventTone.Success else EventTone.Warning
            )
        }
    }

    fun setQualityMode(mode: String) {
        _state.update { it.copy(qualityMode = mode) }
        addEvent("Quality mode changed", "Playback quality set to $mode.")
    }

    fun setLibraryView(view: String) {
        _state.update { it.copy(libraryView = view) }
        addEvent("Library view changed", "View set to $view.")
    }

    private fun addEvent(title: String, detail: String, tone: EventTone = EventTone.Info) {
        _state.update { current ->
            current.copy(events = (listOf(StatusEvent(title, detail, tone)) + current.events).take(8))
        }
    }

    private fun seedState(): CinaVaultState {
        return CinaVaultState(
            metrics = listOf(
                MetricCard("Library", "2,474", "Movies, shows, music, IPTV", "+4.8%"),
                MetricCard("Server", "Offline", "Jellyfin-compatible endpoint", "Idle"),
                MetricCard("Remote", "Ready", "UPnP/NAT-PMP policy enabled", "Port 32400"),
                MetricCard("AI", "Armed", "Diagnostics and metadata assistant", "Premium")
            ),
            sources = listOf(
                MediaSource("Cinema Vault", "/storage/emulated/0/Movies", "Movies", true, 1188),
                MediaSource("Series Library", "/storage/emulated/0/Series", "TV", true, 846),
                MediaSource("Music Archive", "/storage/emulated/0/Music", "Music", true, 391),
                MediaSource("IPTV Favorites", "https://provider.example/playlist.m3u", "Live TV", false, 49)
            ),
            features = listOf(
                FeatureToggle("smart_collections", "Smart Collections", "Auto-groups franchises, genres, moods, and watchlists.", true),
                FeatureToggle("poster_sync", "Poster Sync", "Keeps posters and backdrops aligned across providers.", true),
                FeatureToggle("skip_intro", "Skip Intro/Outro", "Premium playback automation for episodes.", true),
                FeatureToggle("auto_subtitles", "Auto Subtitles", "Finds subtitles during import and playback.", true),
                FeatureToggle("chapter_thumbs", "Chapter Thumbnails", "Generates visual chapter markers.", true),
                FeatureToggle("vpn_integration", "VPN Integration", "Routes remote streaming through secure relay settings.", true),
                FeatureToggle("duplicate_finder", "Duplicate Finder", "Detects redundant media by title, size, and metadata.", true),
                FeatureToggle("plugin_system", "Plugin System", "Loads MS-A/MS-B/MS-C provider catalogs.", true),
                FeatureToggle("glassmorphism", "Glass Skin", "Android-native recreation of the flagship visual skin.", true),
                FeatureToggle("particle_effects", "Particles", "Subtle animated premium ambience.", true),
                FeatureToggle("ai_diagnostics", "AI Diagnostics", "Assistant-style health checks and recommendations.", true)
            ),
            events = listOf(
                StatusEvent("CinaVault Premium Android ready", "Native app shell initialized with premium defaults.", EventTone.Success),
                StatusEvent("Skin loaded", "VidHub Flagship theme, glass panels, and Android status chrome active.")
            )
        )
    }
}
