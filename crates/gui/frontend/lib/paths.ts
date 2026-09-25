/**
 * The file's own name, whichever separator the platform spelled its path with.
 *
 * Shared rather than written twice: the navbar answers "which photograph am I looking at" with it,
 * and the drop route names a refused file with it. Both mean the same thing by a file's name, and a
 * path long enough to push the rest of the navbar off the window answers the question worse.
 *
 * Both separators in one expression because a path crosses the boundary as text: Windows hands this
 * application backslashes, and the two front ends must not disagree about what a name is.
 */
export const fileName = (path: string) => path.split(/[\\/]/).pop() ?? path;

/**
 * The extension a path claims, lowercase and without its dot, or `""` for a name that has none.
 *
 * A name is judged by what follows its **last** dot, so `holiday.tar.gz` claims `gz`, a folder called
 * `Holidays 2026` claims nothing, and so does a dotfile. Here rather than beside either caller: the drop
 * route refuses a file by it, and the usage events name the file types a batch held by it, and the two
 * must not disagree about what a file's type is.
 */
export const extensionOf = (path: string) => {
    const name = fileName(path);
    const dot = name.lastIndexOf(".");

    return dot > 0 ? name.slice(dot + 1).toLowerCase() : "";
};

/**
 * The directory the file is in, spelled as the path spells it, whichever separator that is.
 *
 * Beside {@link fileName} and for its reason: a path crosses the boundary as text, and `@tauri-apps/api/path` is
 * asynchronous where the export queue draws its names synchronously. A path with no separator is in no directory
 * this can name, and answers the empty string. A file at the root answers the root.
 */
export const directoryOf = (path: string) => {
    const last = Math.max(path.lastIndexOf("/"), path.lastIndexOf("\\"));

    if (last < 0) return "";

    // `/photo.jpg` is in `/`, not in the empty string, which would join back as a relative name.
    return last === 0 ? path.slice(0, 1) : path.slice(0, last);
};
