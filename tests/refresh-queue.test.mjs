import test from "node:test";
import assert from "node:assert/strict";
import fs from "node:fs";
import path from "node:path";
import ts from "typescript";

const root = path.resolve(import.meta.dirname, "..");
const exported = {};
new Function(
  "exports",
  ts.transpileModule(
    fs.readFileSync(path.join(root, "src/refreshQueue.ts"), "utf8"),
    {
      compilerOptions: {
        module: ts.ModuleKind.CommonJS,
        target: ts.ScriptTarget.ES2021,
      },
    },
  ).outputText,
)(exported);
const { createRefreshQueue } = exported;

// Only drain promise jobs here; all refresh intervals use the virtual clock.
const flush = () => new Promise((resolve) => setImmediate(resolve));

function deferred() {
  let resolve;
  let reject;
  const promise = new Promise((yes, no) => {
    resolve = yes;
    reject = no;
  });
  return { promise, resolve, reject };
}

function fakeTime() {
  let now = 0;
  let nextId = 0;
  const timers = new Map();
  return {
    clock: () => now,
    schedule(run, delay) {
      const id = ++nextId;
      timers.set(id, { at: now + delay, run });
      return id;
    },
    cancel: (id) => timers.delete(id),
    timers,
    async advance(milliseconds) {
      const end = now + milliseconds;
      let iterations = 0;
      for (;;) {
        const next = [...timers.entries()]
          .filter(([, timer]) => timer.at <= end)
          .sort(([idA, a], [idB, b]) => a.at - b.at || idA - idB)[0];
        if (!next) break;
        assert.ok(++iterations < 1000, "virtual timers must make progress");
        const [id, timer] = next;
        now = timer.at;
        timers.delete(id);
        timer.run();
        await flush();
      }
      now = end;
      await flush();
    },
  };
}

function harness(interval = 750) {
  const time = fakeTime();
  const reads = [];
  const committed = [];
  const errors = [];
  let active = 0;
  let peak = 0;
  const queue = createRefreshQueue({
    ...time,
    interval,
    read() {
      const read = { ...deferred(), at: time.clock() };
      reads.push(read);
      peak = Math.max(peak, ++active);
      return read.promise.finally(() => active--);
    },
    commit: (value) => committed.push(value),
    error: (error) => errors.push(error),
  });
  return { time, queue, reads, committed, errors, peak: () => peak };
}

test("a burst during a read coalesces into one follow-up without overlap", async () => {
  const h = harness();
  const first = h.queue.request();
  await flush();
  let finished = 0;
  const burst = Array.from({ length: 25 }, () =>
    h.queue.request().then(() => finished++),
  );
  await flush();
  assert.equal(h.reads.length, 1);
  assert.equal(h.time.timers.size, 0);
  h.reads[0].resolve("first");
  await first;
  await flush();
  assert.equal(finished, 0, "queued callers await their follow-up data");
  assert.equal(h.time.timers.size, 1);
  await h.time.advance(749);
  assert.equal(h.reads.length, 1);
  await h.time.advance(1);
  assert.equal(h.reads.length, 2);
  h.reads[1].resolve("latest");
  await Promise.all(burst);
  assert.equal(finished, 25);
  assert.equal(h.peak(), 1);
  assert.deepEqual(h.committed, ["first", "latest"]);
  assert.equal(h.time.timers.size, 0);
  h.queue.dispose();
});

test("continuous events refresh at a bounded interval without debounce starvation", async () => {
  const time = fakeTime();
  const starts = [];
  const queue = createRefreshQueue({
    ...time,
    interval: 300,
    read: async () => starts.push(time.clock()),
    commit() {},
    error: assert.fail,
  });
  const requests = [queue.request()];
  await flush();
  for (let elapsed = 50; elapsed <= 1000; elapsed += 50) {
    await time.advance(50);
    requests.push(queue.request());
    await flush();
    assert.ok(time.timers.size <= 1);
  }
  assert.deepEqual(starts, [0, 300, 600, 900]);
  await time.advance(200);
  await Promise.all(requests);
  assert.deepEqual(starts, [0, 300, 600, 900, 1200]);
  assert.equal(time.timers.size, 0);
  queue.dispose();
});

for (const outcome of ["success", "error"]) {
  test(`invalidation suppresses an old ${outcome} while a new generation can commit`, async () => {
    const h = harness();
    const old = h.queue.request();
    await flush();
    h.queue.invalidate();
    const current = h.queue.request(true);
    if (outcome === "success") h.reads[0].resolve("obsolete");
    else h.reads[0].reject(new Error("obsolete failure"));
    await old;
    await flush();
    assert.deepEqual(h.committed, []);
    assert.deepEqual(h.errors, []);
    assert.equal(h.reads.length, 2);
    h.reads[1].resolve("current");
    await current;
    assert.deepEqual(h.committed, ["current"]);
    assert.deepEqual(h.errors, []);
    assert.equal(h.peak(), 1);
    h.queue.dispose();
  });
}

test("invalidation before deferred startup skips the obsolete loader", async () => {
  const h = harness();
  const old = h.queue.request();
  h.queue.invalidate();
  const current = h.queue.request(true);
  await old;
  await flush();
  assert.equal(h.reads.length, 1);
  h.reads[0].resolve("current");
  await current;
  assert.deepEqual(h.committed, ["current"]);
  h.queue.dispose();
});

test("immediate disposal prevents obsolete startup and permits an independent remount", async () => {
  const old = harness();
  const oldRequest = old.queue.request();
  old.queue.dispose();
  const remounted = harness();
  const current = remounted.queue.request();
  await oldRequest;
  await flush();
  assert.equal(old.reads.length, 0);
  assert.deepEqual(old.committed, []);
  assert.deepEqual(old.errors, []);
  assert.equal(old.time.timers.size, 0);
  assert.equal(remounted.reads.length, 1);
  remounted.reads[0].resolve("remounted");
  await current;
  assert.deepEqual(remounted.committed, ["remounted"]);
  remounted.queue.dispose();
});

test("disposing a scheduled refresh cancels its timer and resolves pending callers", async () => {
  const h = harness();
  const first = h.queue.request();
  await flush();
  h.reads[0].resolve("first");
  await first;
  const pending = [h.queue.request(), h.queue.request()];
  assert.equal(h.time.timers.size, 1);
  h.queue.dispose();
  h.queue.dispose(); // Cleanup is idempotent.
  await Promise.all(pending);
  await h.queue.request(true);
  assert.equal(h.time.timers.size, 0);
  await h.time.advance(10000);
  assert.equal(h.reads.length, 1);
  assert.deepEqual(h.committed, ["first"]);
});

for (const outcome of ["success", "error"]) {
  test(`disposing in flight suppresses late ${outcome} and releases queued callers`, async () => {
    const h = harness();
    let activeFinished = false;
    const active = h.queue.request().then(() => (activeFinished = true));
    await flush();
    const pending = h.queue.request(true);
    h.queue.dispose();
    await pending;
    await h.queue.request();
    assert.equal(activeFinished, false);
    if (outcome === "success") h.reads[0].resolve("late");
    else h.reads[0].reject(new Error("late failure"));
    await active;
    await h.time.advance(10000);
    assert.equal(h.reads.length, 1);
    assert.deepEqual(h.committed, []);
    assert.deepEqual(h.errors, []);
    assert.equal(h.time.timers.size, 0);
  });
}

test("manual refresh bypasses a scheduled delay and shares the pending read", async () => {
  const h = harness();
  const first = h.queue.request();
  await flush();
  h.reads[0].resolve("first");
  await first;
  await h.time.advance(100);
  const automatic = h.queue.request();
  assert.equal(h.time.timers.size, 1);
  const manual = h.queue.request(true);
  await flush();
  assert.equal(h.time.timers.size, 0);
  assert.deepEqual(
    h.reads.map((read) => read.at),
    [0, 100],
  );
  h.reads[1].resolve("manual");
  await Promise.all([automatic, manual]);
  await h.time.advance(1000);
  assert.equal(
    h.reads.length,
    2,
    "the canceled timer cannot cause an extra read",
  );
  h.queue.dispose();
});

test("manual refresh during a read has follow-up priority without overlap", async () => {
  const h = harness();
  const first = h.queue.request();
  await flush();
  await h.time.advance(100);
  const automatic = h.queue.request();
  const manual = h.queue.request(true);
  await h.time.advance(100);
  assert.equal(h.reads.length, 1);
  h.reads[0].resolve("first");
  await first;
  await flush();
  assert.deepEqual(
    h.reads.map((read) => read.at),
    [0, 200],
  );
  assert.equal(h.time.timers.size, 0);
  h.reads[1].resolve("manual");
  await Promise.all([automatic, manual]);
  assert.equal(h.peak(), 1);
  h.queue.dispose();
});

test("a rejected read reports once and still runs its queued refresh", async () => {
  const h = harness();
  const first = h.queue.request();
  await flush();
  const followup = h.queue.request();
  const failure = new Error("read rejected");
  h.reads[0].reject(failure);
  await first;
  await flush();
  assert.deepEqual(h.errors, [failure]);
  assert.deepEqual(h.committed, []);
  await h.time.advance(750);
  assert.equal(h.reads.length, 2);
  h.reads[1].resolve("recovered");
  await followup;
  assert.deepEqual(h.committed, ["recovered"]);
  assert.equal(h.peak(), 1);
  h.queue.dispose();
});

test("a synchronous loader throw follows cleanup and permits a later successful refresh", async () => {
  const time = fakeTime();
  const failure = new Error("read threw before returning a promise");
  const errors = [];
  const committed = [];
  let calls = 0;
  const queue = createRefreshQueue({
    ...time,
    read() {
      if (++calls === 1) throw failure;
      return Promise.resolve("recovered");
    },
    commit: (value) => committed.push(value),
    error: (error) => errors.push(error),
  });
  await queue.request();
  assert.deepEqual(errors, [failure]);
  assert.deepEqual(committed, []);
  await queue.request(true);
  assert.equal(calls, 2);
  assert.deepEqual(committed, ["recovered"]);
  assert.equal(time.timers.size, 0);
  queue.dispose();
});
