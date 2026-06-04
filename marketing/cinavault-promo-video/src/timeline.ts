export const fps = 30;
export const durationSeconds = 40;
export const durationInFrames = fps * durationSeconds;

export type PromoScene = {
  id: string;
  start: number;
  end: number;
  title: string;
  subtitle: string;
  voiceover: string;
  visual: string;
  facts: string[];
};

export const scenes: PromoScene[] = [
  {
    id: "library-growth",
    start: 0,
    end: 5,
    title: "Your library is exploding.",
    subtitle: "Index, enrich, and command thousands of titles without the drag.",
    voiceover:
      "Your media library should not feel like a pile of folders. CinaVault turns growth into fast indexing, clean artwork, and searchable control.",
    visual:
      "Animated media tiles flow into a server grid while the collection counter climbs.",
    facts: ["10k+ item library proof", "Responsive paging", "Poster-first UI"],
  },
  {
    id: "product-reveal",
    start: 5,
    end: 12,
    title: "CinaVault Premium Media Server",
    subtitle: "A premium media server with library intelligence built in.",
    voiceover:
      "CinaVault Premium Media Server brings local drives, NAS shares, cloud folders, live TV, remote access, security, and plugins into one command center.",
    visual:
      "Brand mark reveal with connected nodes for library, server, NAS, cloud, and remote access.",
    facts: ["Local + NAS + Cloud", "Live TV ready", "Plugin architecture"],
  },
  {
    id: "ai-automation",
    start: 12,
    end: 19,
    title: "AI-assisted organization",
    subtitle: "Metadata, posters, duplicates, diagnostics, and safer cleanup.",
    voiceover:
      "AI tools enrich metadata, recover posters, apply embedded titles, check providers, and clean duplicates while preserving the details that matter.",
    visual:
      "Feature lanes light up as simulated tasks complete and poster cards snap into place.",
    facts: ["Embedded-title fallback", "Provider health checks", "Safe duplicate removal"],
  },
  {
    id: "access-layer",
    start: 19,
    end: 26,
    title: "Built for every source",
    subtitle: "Local drives, NAS, cloud folders, live TV, VPN, and remote users.",
    voiceover:
      "From desktop storage to NAS vaults and remote users, CinaVault keeps sources visible, permissions clear, and playback workflows connected.",
    visual:
      "Storage endpoints orbit the server core and resolve into a single dashboard map.",
    facts: ["Remote access controls", "Cloud and NAS paths", "Security status at a glance"],
  },
  {
    id: "scalability",
    start: 26,
    end: 31,
    title: "Designed to scale",
    subtitle: "Large libraries stay responsive as collections expand.",
    voiceover:
      "The difference is architecture: paged loading, stable poster caches, provider-aware enrichment, and diagnostics that show what the server is doing.",
    visual:
      "Performance bars compare slow legacy workflows against CinaVault's responsive library path.",
    facts: ["Paged loading", "Stable poster cache", "Live task progress"],
  },
  {
    id: "interface-proof",
    start: 31,
    end: 36,
    title: "Choose the best in class.",
    subtitle: "Join the future with CinaVault Media Server.",
    voiceover:
      "Don't settle for outdated technology. Choose the best in class. Join the future with CinaVault Media Server.",
    visual:
      "Real CinaVault interface shot pushes in, showing scalability and ease of use.",
    facts: ["Real product UI", "AI diagnostics", "Build-proven workflow"],
  },
  {
    id: "cta",
    start: 36,
    end: 40,
    title: "Upgrade to CinaVault today.",
    subtitle: "Experience the difference for yourself.",
    voiceover:
      "Upgrade to CinaVault today and experience the difference for yourself. Learn more at your CinaVault site.",
    visual:
      "End screen with logo, tagline, and call to action.",
    facts: ["Premium media control", "Cleaner libraries", "Ready for growth"],
  },
];

export const fullVoiceover = scenes.map((scene) => scene.voiceover).join(" ");
