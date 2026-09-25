/**
 * A published size, as the design draws it: whole megabytes in the monospace face, in decimal units,
 * untranslated.
 */
export const formatSize = (bytes: number) => {
    // Decimal rather than binary, which is what the archive listings publish and what the design's own
    // figures are. Untranslated for the same reason the component names are - it sits in the
    // identifier column, and "MB" is not a word that changes.
    if (bytes >= 1_000_000_000) return `${(bytes / 1_000_000_000).toFixed(1)} GB`;
    if (bytes >= 1_000_000) return `${Math.round(bytes / 1_000_000)} MB`;

    return `${Math.round(bytes / 1_000)} KB`;
};
