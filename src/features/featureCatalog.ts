// What each switch in Advanced > Feature Matrix does. Every key here exists in
// featureDefaults.json and has a working implementation; tests/featureMatrix
// checks the two lists stay identical.
import type { FeatureKey } from "./featureFlags";

export interface FeatureEntry {
  key: FeatureKey;
  label: string;
  description: string;
}

export interface FeatureCategory {
  name: string;
  features: FeatureEntry[];
}

export const FEATURE_MATRIX: FeatureCategory[] = [
  {
    name: "Playback & Experience",
    features: [
      { key: "cinema_mode", label: "Cinema Mode", description: "Dims everything around the built-in player while a title plays." },
      { key: "skip_intro", label: "Skip Intro Detection", description: "Offers Skip Intro from intro chapters, or for the opening seconds you set." },
      { key: "skip_credits", label: "Skip Credits Detection", description: "Offers Skip Credits from credits chapters, or for the closing seconds you set." },
      { key: "next_up", label: "Next Up Notification", description: "Shows the next episode near the end and plays it after a countdown." },
      { key: "auto_resume", label: "Auto Resume Playback", description: "Remembers where you stopped and picks up from there." },
      { key: "crossfade", label: "Audio Crossfade", description: "Fades audio in and out when playback starts, stops or moves to the next title." },
      { key: "gapless", label: "Gapless Playback", description: "Preloads the next title so it starts without a gap." },
    ],
  },
  {
    name: "UI & Customization",
    features: [
      { key: "glass_effects", label: "Glassmorphism Effects", description: "Frosted, blurred panels. Off gives solid panels and lighter rendering." },
      { key: "particle_bg", label: "Animated Particle Background", description: "Meteor and particle animation behind the library." },
      { key: "holo_agent", label: "Holographic AI Agent", description: "The 3D holographic assistant in the corner." },
      { key: "custom_css", label: "Custom CSS Injection", description: "Applies your own CSS on top of the theme." },
      { key: "compact_mode", label: "Compact View Mode", description: "Smaller posters and tighter spacing to fit more on screen." },
      { key: "poster_hover", label: "Poster Hover Preview", description: "Hovering a poster lifts it and shows the synopsis." },
      { key: "shelf_carousel", label: "Shelf Carousel Mode", description: "Library shelves scroll sideways as carousels instead of wrapping." },
    ],
  },
  {
    name: "Library & Metadata",
    features: [
      { key: "auto_metadata", label: "Auto Metadata Fetch", description: "Looks up metadata for new titles as soon as a scan adds them." },
      { key: "smart_match", label: "Smart Title Matching", description: "Cleans release names (quality tags, groups, years) before matching." },
      { key: "nfo_import", label: "NFO File Import", description: "Reads Kodi-style .nfo files next to media during scans." },
      { key: "subtitle_fetch", label: "Auto Subtitle Download", description: "Downloads subtitles for new titles in your languages from OpenSubtitles." },
      { key: "poster_sync", label: "Cloud Poster Sync", description: "Copies artwork into a folder you choose, such as OneDrive, so other PCs share it." },
      { key: "chapter_thumbs", label: "Chapter Thumbnails", description: "Generates chapter preview images for new titles after a scan." },
      { key: "collection_auto", label: "Auto Collections", description: "Groups the library into collections by series, franchise and genre." },
    ],
  },
  {
    name: "User & Server Management",
    features: [
      { key: "user_profiles", label: "Multiple User Profiles", description: "Separate profiles, each with its own progress and watchlist." },
      { key: "parental_ctrl", label: "Parental Controls", description: "Restricted profiles only see titles within the limits you set, behind a PIN." },
      { key: "activity_log", label: "Activity Logging", description: "Keeps a log of playback, scans, profile and server events." },
      { key: "remote_access", label: "Remote Access", description: "Lets signed-in devices outside your network reach the media server." },
      { key: "api_keys", label: "API Key Management", description: "Issue and revoke API keys that apps can use with the media server." },
      { key: "webhook", label: "Webhook Notifications", description: "Posts events as JSON to URLs you choose." },
    ],
  },
  {
    name: "Performance & Connectivity",
    features: [
      { key: "hw_transcode", label: "Hardware Transcoding", description: "Uses the GPU encoder (NVENC, Quick Sync or AMF) when the server transcodes." },
      { key: "stream_buffer", label: "Stream Buffering Control", description: "Sets how far ahead the player and server buffer." },
      { key: "bandwidth_limit", label: "Bandwidth Limiter", description: "Caps the speed of each stream the server sends." },
      { key: "cdn_cache", label: "CDN / Cache Layer", description: "Caches artwork and lets browsers and proxies cache server responses." },
      { key: "direct_play", label: "Force Direct Play", description: "Always sends the original file and never transcodes." },
      { key: "gpu_accel", label: "GPU Acceleration", description: "Hardware-accelerated rendering and video decoding in the app window." },
    ],
  },
  {
    name: "Library & Discovery",
    features: [
      { key: "trending", label: "Trending Content", description: "A Trending shelf from TMDB, or your most played titles offline." },
      { key: "recommendations", label: "AI Recommendations", description: "Picks titles like the ones you watch, ranked on this PC." },
      { key: "similar_titles", label: "Similar Titles", description: "Shows titles like the one you are looking at." },
      { key: "genre_radio", label: "Genre Radio Stations", description: "Plays a shuffled stream of a genre from your library." },
      { key: "watchlist", label: "Watchlist Management", description: "Save titles for later and keep them on their own shelf." },
      { key: "continue_watching", label: "Continue Watching", description: "A shelf of titles you started and did not finish." },
      { key: "new_releases", label: "New Releases Alerts", description: "Tells you what was added since you last opened the app." },
    ],
  },
];
