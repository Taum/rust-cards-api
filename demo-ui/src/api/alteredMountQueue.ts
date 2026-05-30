/** Max simultaneous `AlteredRender.mountFromApi` calls; raise if mounts feel too slow. */
export const ALTERED_MOUNT_CONCURRENCY = 4;

let inFlight = 0;
const waitQueue: Array<() => void> = [];

function acquireSlot(): Promise<void> {
  if (inFlight < ALTERED_MOUNT_CONCURRENCY) {
    inFlight += 1;
    return Promise.resolve();
  }
  return new Promise((resolve) => {
    waitQueue.push(() => {
      inFlight += 1;
      resolve();
    });
  });
}

function releaseSlot(): void {
  inFlight = Math.max(0, inFlight - 1);
  const next = waitQueue.shift();
  if (next) {
    next();
  }
}

/** Run an Altered card mount while respecting the global concurrency limit. */
export async function withAlteredMountSlot<T>(fn: () => Promise<T>): Promise<T> {
  await acquireSlot();
  try {
    return await fn();
  } finally {
    releaseSlot();
  }
}
