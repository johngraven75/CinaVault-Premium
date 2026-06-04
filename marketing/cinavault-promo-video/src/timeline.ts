export const fps = 30;
export const durationSeconds = 61;
export const durationInFrames = fps * durationSeconds;

export type PromoScene = {
  id: string;
  start: number;
  end: number;
  kicker: string;
  title: string;
  subtitle: string;
  voiceover: string;
  facts: string[];
};

export const scenes: PromoScene[] = [
  {
    id: "opening-wow",
    start: 0,
    end: 7,
    kicker: "HIGH-VOLUME LIBRARY CONTROL",
    title: "Your library should feel instant.",
    subtitle: "Not scattered. Not sluggish. Not stuck in yesterday.",
    voiceover:
      "Your media library ought to feel calm, fast, and under control.",
    facts: ["Thousands of titles", "Poster-first browsing", "Responsive library flow"],
  },
  {
    id: "command-center",
    start: 7,
    end: 16,
    kicker: "ONE POLISHED COMMAND CENTER",
    title: "CinaVault brings every source together.",
    subtitle: "Local drives, NAS shares, cloud folders, live TV, and remote access.",
    voiceover:
      "CinaVault Premium takes local drives, NAS shares, cloud folders, live TV, and remote access, and brings it all into one polished command center.",
    facts: ["Local + NAS + Cloud", "Live TV ready", "Remote access controls"],
  },
  {
    id: "library-intelligence",
    start: 16,
    end: 28,
    kicker: "LIBRARY INTELLIGENCE",
    title: "Metadata, posters, titles, and provider checks.",
    subtitle: "A smarter library without constant manual cleanup.",
    voiceover:
      "It indexes big collections, keeps posters out front, recovers metadata, applies embedded titles, checks providers, and shows real diagnostics while work is running.",
    facts: ["AI metadata gather", "Embedded-title fallback", "Provider health checks"],
  },
  {
    id: "scale-architecture",
    start: 28,
    end: 40,
    kicker: "BUILT TO SCALE",
    title: "No guessing. No frozen library screen.",
    subtitle: "Architecture that keeps large collections moving.",
    voiceover:
      "No guessing. No waiting on a frozen library screen. Just clear source health, safer duplicate cleanup, responsive paging, stable poster caches, and live task progress.",
    facts: ["Paged loading", "Stable poster cache", "Safe duplicate cleanup"],
  },
  {
    id: "real-ui",
    start: 40,
    end: 51,
    kicker: "REAL PRODUCT PROOF",
    title: "The dashboard tells you what is happening.",
    subtitle: "Diagnostics, enrichment, source checks, and visible progress.",
    voiceover:
      "The dashboard tells you what the server is doing, where the sources are, and how your library is improving in real time.",
    facts: ["Real CinaVault UI", "AI Diagnostics", "Build-proven workflow"],
  },
  {
    id: "final-cta",
    start: 51,
    end: 61,
    kicker: "STEP INTO THE FUTURE",
    title: "Give your library the vault it deserves.",
    subtitle: "Upgrade to CinaVault Premium Media Server.",
    voiceover:
      "Don't settle for yesterday's media server. Step into CinaVault, and give your library the vault it deserves.",
    facts: ["Premium media control", "Cleaner libraries", "Ready for growth"],
  },
];

export const fullVoiceover = scenes.map((scene) => scene.voiceover).join(" ");
