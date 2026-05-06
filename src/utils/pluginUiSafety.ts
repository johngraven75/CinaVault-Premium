import type { PluginEntry } from "../data/pluginRegistry";
import type { MetadataProvider } from "../store/appStore";

type MetadataProviderLike = Partial<MetadataProvider> | null | undefined;
type PluginSearchCandidate = Partial<Pick<PluginEntry, "name" | "description" | "tags">>;

export function matchesPluginSearch(plugin: PluginSearchCandidate, rawSearch: string): boolean {
  const search = rawSearch.trim().toLowerCase();
  if (!search) return true;

  const name = typeof plugin.name === "string" ? plugin.name.toLowerCase() : "";
  const description = typeof plugin.description === "string" ? plugin.description.toLowerCase() : "";
  const tags = Array.isArray(plugin.tags)
    ? plugin.tags.filter((tag): tag is string => typeof tag === "string").map((tag) => tag.toLowerCase())
    : [];

  return name.includes(search) || description.includes(search) || tags.some((tag) => tag.includes(search));
}

export function getMetadataProviderInitials(name?: string | null): string {
  const trimmed = typeof name === "string" ? name.trim() : "";
  return trimmed ? trimmed.slice(0, 2).toUpperCase() : "??";
}

export function sanitizeMetadataProviders(
  persisted: unknown,
  defaults: MetadataProvider[],
): MetadataProvider[] {
  const merged = new Map<string, MetadataProvider>(
    defaults.map((provider) => [provider.id, { ...provider }]),
  );

  if (!Array.isArray(persisted)) {
    return Array.from(merged.values());
  }

  for (const candidate of persisted as MetadataProviderLike[]) {
    if (!candidate || typeof candidate !== "object" || typeof candidate.id !== "string") {
      continue;
    }

    const fallback = merged.get(candidate.id);
    const name = typeof candidate.name === "string" && candidate.name.trim()
      ? candidate.name.trim()
      : fallback?.name;
    const category = typeof candidate.category === "string" && candidate.category.trim()
      ? candidate.category.trim()
      : fallback?.category;

    if (!name || !category) {
      continue;
    }

    merged.set(candidate.id, {
      id: candidate.id,
      name,
      category,
      enabled: typeof candidate.enabled === "boolean" ? candidate.enabled : fallback?.enabled ?? false,
    });
  }

  return Array.from(merged.values());
}
