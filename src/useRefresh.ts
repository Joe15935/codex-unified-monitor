import { useCallback, useEffect, useRef } from "react";
import { listen } from "@tauri-apps/api/event";
import { native } from "./data";
import { createRefreshQueue } from "./refreshQueue";

/** One read at a time per mounted view. Native events only read local snapshots. */
export function useRefresh<T>(options: {
  key: string;
  read: () => Promise<T>;
  commit: (value: T) => void;
  error: (error: unknown) => void;
  interval?: number;
  poll?: number;
}) {
  const latest = useRef(options);
  latest.current = options;
  const queue = useRef<ReturnType<
    typeof createRefreshQueue<{ key: string; value: T }>
  > | null>(null);
  const selected = useRef(options.key);
  const refresh = useCallback(
    () => queue.current?.request(true) ?? Promise.resolve(),
    [],
  );
  const invalidate = useCallback(() => queue.current?.invalidate(), []);

  useEffect(() => {
    let active = true;
    const current = createRefreshQueue({
      interval: latest.current.interval,
      read: async () => {
        const { key, read } = latest.current;
        try {
          return { key, value: await read() };
        } catch (error) {
          throw { key, error };
        }
      },
      commit: ({ key, value }) => {
        if (key === latest.current.key) latest.current.commit(value);
      },
      error: (failure) => {
        const { key, error } = failure as { key: string; error: unknown };
        if (key === latest.current.key) latest.current.error(error);
      },
    });
    queue.current = current;
    selected.current = latest.current.key;
    void current.request(true);
    const refreshVisible = () => {
      if (!document.hidden) void current.request();
    };
    const foreground = () => {
      // WebKit visibility can lag behind the native window becoming visible.
      if (active) void current.request();
    };
    const subscriptions = native
      ? [
          listen("monitor-updated", refreshVisible).catch(() => undefined),
          listen("monitor-visible", foreground).catch(() => undefined),
        ]
      : [];
    document.addEventListener("visibilitychange", refreshVisible);
    window.addEventListener("focus", foreground);
    // Visible-only snapshot reconciliation also recovers from a lost event listener.
    const poll = latest.current.poll ?? 30000;
    const timer = poll ? setInterval(refreshVisible, poll) : undefined;
    return () => {
      active = false;
      current.dispose();
      if (queue.current === current) queue.current = null;
      document.removeEventListener("visibilitychange", refreshVisible);
      window.removeEventListener("focus", foreground);
      if (timer !== undefined) clearInterval(timer);
      // Subscriptions can resolve after React StrictMode cleanup/unmount.
      subscriptions.forEach(
        (subscription) =>
          void subscription.then((dispose) => dispose?.()).catch(() => {}),
      );
    };
  }, []);
  useEffect(() => {
    if (selected.current !== options.key) {
      selected.current = options.key;
      queue.current?.invalidate();
      void refresh();
    }
  }, [options.key, refresh]);
  return { refresh, invalidate };
}
