export interface AdultMetadataProviderDefinition {
  id: string;
  name: string;
  category: string;
  enabled: boolean;
  bridge: "native" | "phoenixadult" | "metadataapi" | "exact" | "limited";
}

export const ADULT_METADATA_PROVIDERS: AdultMetadataProviderDefinition[] = [
  { id: "theporndb", name: "ThePornDB", category: "Adult / Native API", enabled: false, bridge: "native" },
  { id: "stashdb", name: "StashDB", category: "Adult / Native GraphQL", enabled: false, bridge: "native" },
  { id: "metadataapi", name: "MetadataAPI", category: "Adult / Bridge API", enabled: false, bridge: "metadataapi" },
  { id: "phoenixadult", name: "PhoenixAdult", category: "Adult / PhoenixAdult", enabled: false, bridge: "phoenixadult" },
  { id: "iafd", name: "IAFD", category: "Adult / Reference", enabled: false, bridge: "metadataapi" },
  { id: "adultdvdempire", name: "AdultDVDEmpire", category: "Adult / PhoenixAdult", enabled: false, bridge: "phoenixadult" },
  { id: "javlibrary", name: "JavLibrary", category: "Adult / JAV", enabled: false, bridge: "limited" },
  { id: "r18", name: "R18", category: "Adult / JAV", enabled: false, bridge: "phoenixadult" },
  { id: "heyzo", name: "Heyzo", category: "Adult / JAV", enabled: false, bridge: "phoenixadult" },
  { id: "caribbeancom", name: "Caribbeancom", category: "Adult / JAV", enabled: false, bridge: "phoenixadult" },
  { id: "hegre", name: "Hegre", category: "Adult / PhoenixAdult", enabled: false, bridge: "phoenixadult" },
  { id: "porndoe", name: "Porndoe", category: "Adult / PhoenixAdult", enabled: false, bridge: "phoenixadult" },
  { id: "nubiles", name: "Nubiles", category: "Adult / PhoenixAdult", enabled: false, bridge: "phoenixadult" },
  { id: "pornhub", name: "Pornhub", category: "Adult / PhoenixAdult", enabled: false, bridge: "phoenixadult" },
  { id: "porncz", name: "PornCZ", category: "Adult / PhoenixAdult", enabled: false, bridge: "phoenixadult" },
  { id: "clips4sale", name: "Clips4Sale", category: "Adult / Exact Match", enabled: false, bridge: "exact" },
  { id: "manyvids", name: "ManyVids", category: "Adult / Exact Match", enabled: false, bridge: "exact" },
  { id: "data18", name: "Data18", category: "Adult / PhoenixAdult", enabled: false, bridge: "phoenixadult" },
  { id: "brazzers", name: "Brazzers Network", category: "Adult / Network", enabled: false, bridge: "phoenixadult" },
  { id: "julesjordan", name: "JulesJordan Network", category: "Adult / Network", enabled: false, bridge: "phoenixadult" },
  { id: "naughtyamerica", name: "Naughty America", category: "Adult / Network", enabled: false, bridge: "phoenixadult" },
  { id: "bangbros", name: "Bang Bros Network", category: "Adult / Network", enabled: false, bridge: "phoenixadult" },
  { id: "babes", name: "Babes Network", category: "Adult / Network", enabled: false, bridge: "phoenixadult" },
  { id: "digitalplayground", name: "DigitalPlayground", category: "Adult / Network", enabled: false, bridge: "phoenixadult" },
  { id: "evilangel", name: "EvilAngel", category: "Adult / Network", enabled: false, bridge: "phoenixadult" },
  { id: "kink", name: "Kink Network", category: "Adult / Network", enabled: false, bridge: "phoenixadult" },
  { id: "milehigh", name: "MileHigh Network", category: "Adult / Network", enabled: false, bridge: "phoenixadult" },
  { id: "mofos", name: "Mofos Network", category: "Adult / Network", enabled: false, bridge: "phoenixadult" },
  { id: "mylf", name: "MYLF Network", category: "Adult / Network", enabled: false, bridge: "phoenixadult" },
  { id: "girlsway", name: "Girlsway", category: "Adult / Network", enabled: false, bridge: "phoenixadult" },
  { id: "fakehub", name: "FakeHub", category: "Adult / Network", enabled: false, bridge: "phoenixadult" },
  { id: "dogfart", name: "Dogfart", category: "Adult / Limited", enabled: false, bridge: "limited" },
  { id: "teamskeet", name: "TeamSkeet", category: "Adult / MetadataAPI", enabled: false, bridge: "metadataapi" },
  { id: "realitykings", name: "Reality Kings", category: "Adult / MetadataAPI", enabled: false, bridge: "metadataapi" },
  { id: "vixen", name: "Vixen Media Group", category: "Adult / MetadataAPI", enabled: false, bridge: "metadataapi" },
  { id: "adulttime", name: "Adult Time", category: "Adult / PhoenixAdult", enabled: false, bridge: "phoenixadult" },
  { id: "21naturals", name: "21Naturals", category: "Adult / PhoenixAdult", enabled: false, bridge: "phoenixadult" },
  { id: "21sextury", name: "21Sextury", category: "Adult / PhoenixAdult", enabled: false, bridge: "phoenixadult" },
  { id: "legalporno", name: "LegalPorno", category: "Adult / Limited", enabled: false, bridge: "limited" },
  { id: "hentaipros", name: "HentaiPros", category: "Adult / PhoenixAdult", enabled: false, bridge: "phoenixadult" },
];
