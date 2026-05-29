import '../altered-card-renderer-minified.js';

let initPromise: Promise<void> | null = null;

export function ensureAlteredRenderInit(): Promise<void> {
  if (!initPromise) {
    initPromise = window.AlteredRender.init()
      .then(() => undefined)
      .catch((err: unknown) => {
        initPromise = null;
        throw err;
      });
  }
  return initPromise;
}
