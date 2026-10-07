// Adult metadata providers for the first-run wizard.
//
// Lives in its own module so the store-safe (MS-v1) build, where the wizard
// only references it behind `IS_STORE_SAFE ? [] : ...`, tree-shakes the
// whole file and none of these provider names ship in that bundle.
import type { KeylessProvider, SetupProvider } from "./firstRunSetup";

export const ADULT_SETUP_PROVIDERS: readonly SetupProvider[] = [
  {
    id: "tpdb",
    name: "ThePornDB",
    description: "Scene, performer and studio matching with posters and backdrops. Used with CinaVault Plus.",
    signupUrl: "https://theporndb.net/register",
    adult: true,
    minLength: 16,
    optional: true,
  },
  {
    id: "stashdb",
    name: "StashDB",
    description: "Community scene fingerprints and performer data. Used with CinaVault Plus.",
    signupUrl: "https://stashdb.org/register",
    adult: true,
    minLength: 16,
    optional: true,
  },
];

export const ADULT_KEYLESS_PROVIDERS: readonly KeylessProvider[] = [
  {
    id: "iafd",
    name: "IAFD",
    description: "Performer and title database lookups.",
    adult: true,
  },
  {
    id: "pgma",
    name: "PGMA bridge",
    description: "Reads existing local sidecar metadata, nothing to configure.",
    adult: true,
  },
];
