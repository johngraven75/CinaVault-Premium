// Library metadata provider policy — pure, so it can be unit tested under node.
//
// The full edition looks up library metadata from adult providers only
// (ThePornDB, StashDB and the keyless adult bridges). Regular movie/TV
// providers such as TMDb and OMDb stay listed but are marked "not in use"
// and cannot be switched on. The store-safe edition has no adult providers,
// so it keeps the regular ones. Mirrors src-tauri/src/library_metadata_policy.rs.

export interface PolicyProvider {
  id: string;
  name: string;
  category: string;
  enabled: boolean;
  inUse?: boolean;
}

export const NOT_IN_USE_LABEL = "Not in use";

export function isAdultCategory(category: string): boolean {
  return category.trim().toLowerCase() === "adult";
}

/** True when the library may use this provider in the given edition. */
export function providerInUse(provider: Pick<PolicyProvider, "category">, storeSafe: boolean): boolean {
  return storeSafe || isAdultCategory(provider.category);
}

/** Marks regular providers as disabled and not in use for the full edition. */
export function applyLibraryProviderPolicy<T extends PolicyProvider>(providers: T[], storeSafe: boolean): T[] {
  return providers.map((provider) =>
    providerInUse(provider, storeSafe)
      ? { ...provider, inUse: true }
      : { ...provider, enabled: false, inUse: false },
  );
}
