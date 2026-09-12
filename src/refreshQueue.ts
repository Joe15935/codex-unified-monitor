/** Coalesce updates without overlapping reads or starving under continuous events. */
export function createRefreshQueue<T>(options: {
  read: () => Promise<T>;
  commit: (value: T) => void;
  error: (error: unknown) => void;
  interval?: number;
  clock?: () => number;
  schedule?: (run: () => void, delay: number) => ReturnType<typeof setTimeout>;
  cancel?: (timer: ReturnType<typeof setTimeout>) => void;
}) {
  const clock = options.clock ?? Date.now;
  const schedule = options.schedule ?? setTimeout;
  const cancel = options.cancel ?? clearTimeout;
  let generation = 0;
  let running = false;
  let disposed = false;
  let lastStarted = -Infinity;
  let timer: ReturnType<typeof setTimeout> | undefined;
  let pending: (() => void)[] = [];
  let urgent = false;

  const start = () => {
    timer = undefined;
    if (disposed || running || !pending.length) return;
    running = true;
    urgent = false;
    lastStarted = clock();
    const revision = generation;
    const waiters = pending;
    pending = [];
    // Defer until after the current effect stack, so StrictMode cleanup can
    // cancel an obsolete read before it reaches the database.
    void Promise.resolve()
      .then(async () => {
        if (disposed || revision !== generation) return;
        try {
          const value = await options.read();
          if (!disposed && revision === generation) options.commit(value);
        } catch (error) {
          if (!disposed && revision === generation) options.error(error);
        }
      })
      .finally(() => {
        running = false;
        waiters.forEach((resolve) => resolve());
        if (!disposed && pending.length) plan();
      });
  };
  const plan = () => {
    if (disposed || running || !pending.length) return;
    if (timer !== undefined) {
      if (!urgent) return;
      cancel(timer);
      timer = undefined;
    }
    const delay = urgent
      ? 0
      : Math.max(0, (options.interval ?? 750) - (clock() - lastStarted));
    if (delay === 0) start();
    else timer = schedule(start, delay);
  };
  return {
    request(immediate = false): Promise<void> {
      if (disposed) return Promise.resolve();
      urgent ||= immediate;
      const done = new Promise<void>((resolve) => pending.push(resolve));
      plan();
      return done;
    },
    invalidate() {
      generation += 1;
    },
    dispose() {
      disposed = true;
      generation += 1;
      if (timer !== undefined) cancel(timer);
      timer = undefined;
      pending.forEach((resolve) => resolve());
      pending = [];
    },
  };
}
