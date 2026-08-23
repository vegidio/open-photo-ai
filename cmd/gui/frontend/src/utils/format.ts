// Formats a byte count the way the export UI shows file sizes. Decimal units (1000, not 1024) to match what the OS
// file manager reports for the same file, so the two never appear to disagree.
export const formatBytes = (bytes: number): string =>
    bytes < 1_000_000 ? `${(bytes / 1_000).toFixed(2)} KB` : `${(bytes / 1_000_000).toFixed(2)} MB`;
