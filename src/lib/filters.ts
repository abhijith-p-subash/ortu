export function buildSearchQuery(
  selectedGroup: string | null,
  searchQuery: string
): string {
  const q = searchQuery.trim();
  if (!selectedGroup) return q;
  return `category:${selectedGroup} ${q}`.trim();
}

export function clipPreview(raw: string, contentType: string): string {
  if (contentType === "image") return "[Image]";
  if (contentType === "files") {
    try {
      const parsed = JSON.parse(raw) as string[];
      if (Array.isArray(parsed) && parsed.length > 0) {
        return parsed.length === 1
          ? `[File] ${parsed[0]}`
          : `[${parsed.length} files] ${parsed[0]}`;
      }
    } catch {
      return "[Files]";
    }
  }
  if (contentType === "html") return "[HTML] " + raw.replace(/<[^>]+>/g, " ").trim();
  if (contentType === "rtf") return "[RTF]";
  return raw;
}
