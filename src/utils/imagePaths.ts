// P61b (D4): extension-based image detection. Shared by RepoWorkspace (which
// fetches getImageDiff instead of a text file diff) and DiffOverlay (which swaps
// the File/Diff/Split toolbar for the image-mode switcher). SVG is deliberately
// NOT in the set — it stays a text diff (OQ3).
const IMAGE_EXTENSIONS: ReadonlySet<string> = new Set([
  'png',
  'jpg',
  'jpeg',
  'gif',
  'webp',
  'bmp',
  'ico',
  'avif',
]);

/** True when `path`'s file extension is a raster image type (D4). Keys off the
 *  basename so a dot in a parent directory never counts; a dotfile with no
 *  extension (".gitignore") is not an image. Case-insensitive. */
export function isImagePath(path: string): boolean {
  const base = path.slice(path.lastIndexOf('/') + 1);
  const dot = base.lastIndexOf('.');
  if (dot <= 0) return false; // no extension, or a leading-dot dotfile
  return IMAGE_EXTENSIONS.has(base.slice(dot + 1).toLowerCase());
}
