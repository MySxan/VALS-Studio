import { afterEach, describe, expect, it, vi } from "vitest";
import fixture from "./test-fixtures/session.json";
import { createController, type Controller } from "./controller";
import {
  sessionSchema,
  validateWaveform,
  type Backend,
  type Waveform,
} from "./model";

const session = sessionSchema.parse(fixture.session);
const jobId = "00000000-0000-4000-8000-000000000001";
const controllers: Controller[] = [];
function setup(overrides: Partial<Backend> = {}) {
  const api: Backend = {
    currentSession: vi.fn(async () => session),
    chooseFile: vi.fn(async () => "test.wav"),
    startImport: vi.fn(async () => jobId),
    jobStatus: vi.fn(async () => ({
      id: jobId,
      phase: "running" as const,
      error: null,
    })),
    cancelJob: vi.fn(async () => {}),
    waveform: vi.fn(async () => fixture.waveform),
    ...overrides,
  };
  const controller = createController(api);
  controllers.push(controller);
  return { api, controller };
}
async function settle() {
  for (let i = 0; i < 8; i++) await Promise.resolve();
}
function deferred<T>() {
  let resolve!: (value: T) => void;
  let reject!: (reason: unknown) => void;
  const promise = new Promise<T>((yes, no) => {
    resolve = yes;
    reject = no;
  });
  return { promise, resolve, reject };
}
afterEach(() => {
  controllers.splice(0).forEach((c) => c.dispose());
  vi.useRealTimers();
});

describe("Rust DTO contract", () => {
  it("accepts real Rust output and preserves measurement semantics", () => {
    expect(session.confidence.score).toBeNull();
    expect(session.provenance.dependencyHashes).toHaveLength(1);
    expect(
      validateWaveform(fixture.waveform, session, {
        start: 0,
        end: session.duration,
        width: 2,
      }),
    ).toEqual(fixture.waveform);
  });
  it("rejects mismatched identity, oversized payloads and malformed measurements", () => {
    const view = { start: 0, end: 1, width: 2 };
    expect(() =>
      validateWaveform(
        { ...fixture.waveform, sessionId: jobId },
        session,
        view,
      ),
    ).toThrow();
    expect(() =>
      validateWaveform(
        { ...fixture.waveform, artifactHash: "0".repeat(64) },
        session,
        view,
      ),
    ).toThrow();
    expect(() =>
      validateWaveform(
        {
          ...fixture.waveform,
          channels: [Array(4).fill(fixture.waveform.channels[0][0]), []],
        },
        session,
        view,
      ),
    ).toThrow();
    expect(() =>
      sessionSchema.parse({
        ...session,
        confidence: { ...session.confidence, score: 1 },
      }),
    ).toThrow();
    expect(() =>
      sessionSchema.parse({ ...session, duration: Infinity }),
    ).toThrow();
  });
});

describe("session and viewport lifecycle", () => {
  it("ignores stale waveform responses and errors after a newer viewport", async () => {
    const old = deferred<Waveform>();
    const latest = deferred<Waveform>();
    const waveform = vi
      .fn()
      .mockReturnValueOnce(old.promise)
      .mockReturnValueOnce(latest.promise);
    const { controller } = setup({ waveform });
    await controller.initialize();
    controller.view(0, session.duration / 2);
    latest.resolve(fixture.waveform);
    await settle();
    old.reject(new Error("outdated error"));
    await settle();
    expect(controller.store.getState().waveform).toEqual(fixture.waveform);
    expect(controller.store.getState().error).toBeNull();
    expect(controller.store.getState().viewport.end).toBe(session.duration / 2);
  });
  it("clamps pan and width, rejects invalid viewports, never restarts analysis", async () => {
    const { controller, api } = setup();
    await controller.initialize();
    await settle();
    controller.view(-10, -10 + session.duration / 2, 9000);
    expect(controller.store.getState().viewport).toEqual({
      start: 0,
      end: expect.closeTo(session.duration / 2),
      width: 4096,
    });
    const before = controller.store.getState().viewport;
    controller.view(NaN, 10);
    expect(controller.store.getState().viewport).toEqual(before);
    expect(api.startImport).not.toHaveBeenCalled();
  });
  it("disposal prevents late responses and stops polling", async () => {
    vi.useFakeTimers();
    const result = deferred<Waveform>();
    const { controller, api } = setup({ waveform: () => result.promise });
    await controller.initialize();
    await controller.importFile();
    await settle();
    controller.dispose();
    result.resolve(fixture.waveform);
    await settle();
    const count = vi.mocked(api.jobStatus).mock.calls.length;
    await vi.advanceTimersByTimeAsync(2000);
    expect(api.jobStatus).toHaveBeenCalledTimes(count);
    expect(controller.store.getState().waveform).toBeNull();
  });
});

describe("background import", () => {
  it("late initialization cannot overwrite a successfully imported session", async () => {
    const initial = deferred<typeof session | null>();
    const next = { ...session, id: jobId };
    const { controller } = setup({
      currentSession: vi
        .fn()
        .mockReturnValueOnce(initial.promise)
        .mockResolvedValue(next),
      jobStatus: async () => ({ id: jobId, phase: "succeeded", error: null }),
      waveform: async () => ({ ...fixture.waveform, sessionId: jobId }),
    });
    const initialize = controller.initialize();
    await controller.importFile();
    await settle();
    initial.resolve(session);
    await initialize;
    await settle();
    expect(controller.store.getState().session?.id).toBe(jobId);
  });
  it("a late successful viewport does not erase an import error", async () => {
    const wave = deferred<Waveform>();
    const { controller } = setup({
      waveform: () => wave.promise,
      jobStatus: async () => ({
        id: jobId,
        phase: "failed",
        error: "bad source",
      }),
    });
    await controller.initialize();
    await controller.importFile();
    await settle();
    wave.resolve(fixture.waveform);
    await settle();
    expect(controller.store.getState().error).toBe("bad source");
    expect(controller.store.getState().waveform).not.toBeNull();
  });
  it("preserves the old session on failure and prevents duplicate imports", async () => {
    vi.useFakeTimers();
    const { controller, api } = setup();
    await controller.initialize();
    await settle();
    await controller.importFile();
    await controller.importFile();
    await settle();
    expect(api.startImport).toHaveBeenCalledTimes(1);
    vi.mocked(api.jobStatus).mockResolvedValue({
      id: jobId,
      phase: "failed",
      error: "invalid WAV",
    });
    await vi.advanceTimersByTimeAsync(200);
    expect(controller.store.getState().session).toEqual(session);
    expect(controller.store.getState().waveform).not.toBeNull();
    expect(controller.store.getState().error).toBe("invalid WAV");
    expect(controller.store.getState().busy).toBe(false);
  });
  it("retains busy until cancellation reaches a terminal status", async () => {
    vi.useFakeTimers();
    const { controller, api } = setup();
    await controller.initialize();
    await settle();
    await controller.importFile();
    await settle();
    await controller.cancel();
    expect(api.cancelJob).toHaveBeenCalledWith(jobId);
    expect(controller.store.getState().busy).toBe(true);
    vi.mocked(api.jobStatus).mockResolvedValue({
      id: jobId,
      phase: "cancelled",
      error: null,
    });
    await vi.advanceTimersByTimeAsync(200);
    expect(controller.store.getState().busy).toBe(false);
    expect(controller.store.getState().session).toEqual(session);
  });
  it("publishes a successful replacement and resets the viewport", async () => {
    vi.useFakeTimers();
    const { controller, api } = setup();
    await controller.initialize();
    await settle();
    controller.view(0, session.duration / 2);
    await settle();
    const next = { ...session, id: jobId };
    vi.mocked(api.currentSession).mockResolvedValue(next);
    vi.mocked(api.waveform).mockResolvedValue({
      ...fixture.waveform,
      sessionId: jobId,
    });
    vi.mocked(api.jobStatus).mockResolvedValue({
      id: jobId,
      phase: "succeeded",
      error: null,
    });
    await controller.importFile();
    await settle();
    expect(controller.store.getState().session?.id).toBe(jobId);
    expect(controller.store.getState().viewport.end).toBe(session.duration);
    expect(controller.store.getState().waveform?.sessionId).toBe(jobId);
    expect(controller.store.getState().busy).toBe(false);
  });
  it("recovers status polling failures without allowing another import", async () => {
    vi.useFakeTimers();
    const { controller, api } = setup();
    vi.mocked(api.jobStatus).mockRejectedValueOnce(
      new Error("transport unavailable"),
    );
    await controller.importFile();
    await settle();
    expect(controller.store.getState().busy).toBe(true);
    vi.mocked(api.jobStatus).mockResolvedValue({
      id: jobId,
      phase: "cancelled",
      error: null,
    });
    await vi.advanceTimersByTimeAsync(1000);
    expect(controller.store.getState().busy).toBe(false);
    expect(controller.store.getState().error).toBeNull();
  });
  it("file dialog cancellation does not start a job", async () => {
    const { controller, api } = setup({ chooseFile: async () => null });
    await controller.importFile();
    expect(api.startImport).not.toHaveBeenCalled();
    expect(controller.store.getState().busy).toBe(false);
  });
});
