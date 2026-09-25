import { useCurrentFile } from "@/stores/files";
import { type ImageTransform, useImageTransform } from "@/stores/transform";

/**
 * How the current photograph is being looked at.
 *
 * What the drawer's zoom control reads: a control reports the image the user has chosen, which is the
 * current one whether or not its pixels have arrived yet.
 *
 * **A hook rather than a third accessor in `stores/transform.ts`**, where it lived while the two stores
 * pointed one way. Closing an image made the file store the one that knows a photograph has gone, so it
 * is the store that forgets that photograph's view - and the import it needs would close a cycle with
 * this composition, which reads the *other* store. Neither store owns a question about both of them, so
 * it sits here with the other hooks that read across regions.
 */
export const useCurrentTransform = (): ImageTransform => useImageTransform(useCurrentFile()?.identity);
