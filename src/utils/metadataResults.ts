type MetadataResultLike = {
  type?: string;
  status?: string;
};

export function shouldRefreshLibraryAfterMetadataResult(result: MetadataResultLike | null | undefined): boolean {
  if (!result || result.status === "error") {
    return false;
  }

  return result.type === "library_enrichment" || result.type === "adult_metadata_gather";
}
