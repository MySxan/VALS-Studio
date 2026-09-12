import { afterEach, expect, it, vi } from "vitest";
import fixture from "./test-fixtures/session.json";
import { createController, type Controller } from "./controller";
import {
  workspaceSchema,
  validateWaveform,
  type Backend,
  type Workspace,
  type Job,
  type Waveform,
  type Playback,
} from "./model";
const sample = workspaceSchema.parse(fixture.workspace),
  sampleProject = sample.project!,
  sampleTrack = sampleProject.tracks[0];
const jobId = "00000000-0000-4000-8000-000000000001";
const controllers: Controller[] = [];
it("reanalysis shows analyzing instead of stale ready evidence", async () => {
  vi.useFakeTimers();
  const { c, api } = setup(sample);
  await c.initialize();
  await settle();
  vi.mocked(api.jobStatus).mockResolvedValue({
    id: jobId,
    projectId: sampleProject.id,
    generation: sample.generation,
    trackId: sampleTrack.id,
    phase: "running",
    error: null,
  });
  await c.retryTrack();
  await settle();
  expect(c.store.getState().workspace.project?.tracks[0].status).toBe(
    "analyzing",
  );
  expect(c.store.getState().workspace.project?.tracks[0].analysis).toBeNull();
  expect(c.store.getState().waveform).toBeNull();
});
const copy = <T>(x: T): T => structuredClone(x);
async function settle() {
  for (let i = 0; i < 20; i++) await Promise.resolve();
}
function deferred<T>() {
  let resolve!: (x: T) => void, reject!: (e: unknown) => void;
  const promise = new Promise<T>((yes, no) => {
    resolve = yes;
    reject = no;
  });
  return { promise, resolve, reject };
}
function setup(
  initial: Workspace = { generation: 0, project: null, job: null },
) {
  let state = copy(initial),
    disk: Workspace | null = null,
    pending: "import" | "analyze" | "relink" = "import";
  let playback: Playback = {
    projectId: sampleProject.id,
    generation: initial.generation,
    trackId: sampleTrack.id,
    phase: "stopped",
    position: 0,
    duration: sampleTrack.duration,
  };
  let outcome: "succeeded" | "failed" | "cancelled" = "succeeded";
  let failureState: "offline" | "changed" | "error" = "offline";
  const api: Backend = {
    currentProject: vi.fn(async () => copy(state)),
    newProject: vi.fn(async (name, generation) => {
      state = {
        generation: generation + 1,
        project: {
          ...copy(sampleProject),
          name,
          path: null,
          dirty: true,
          tracks: [],
        },
        job: null,
      };
      return copy(state);
    }),
    closeProject: vi.fn(async (generation) => {
      state = { generation: generation + 1, project: null, job: null };
      return copy(state);
    }),
    openProject: vi.fn(async (_, generation) => {
      state = copy(disk!);
      state.generation = generation + 1;
      state.job = null;
      state.project!.tracks.forEach((t) => {
        t.status = "unchecked";
        t.analysis = null;
      });
      return copy(state);
    }),
    saveProject: vi.fn(async (_, path) => {
      state.project!.path = path ?? state.project!.path;
      state.project!.dirty = false;
      disk = copy(state);
      return copy(state);
    }),
    chooseFile: vi.fn(async (kind) =>
      kind === "audio" ? "audio.wav" : "saved.vocalproj",
    ),
    chooseSavePath: vi.fn(async () => "saved.vocalproj"),
    confirmDiscard: vi.fn(async () => false),
    startImport: vi.fn(async () => {
      pending = "import";
      return jobId;
    }),
    analyzeTrack: vi.fn(async () => {
      pending = "analyze";
      return jobId;
    }),
    relinkTrack: vi.fn(async () => {
      pending = "relink";
      return jobId;
    }),
    jobStatus: vi.fn(async () => {
      if (outcome === "succeeded") {
        if (pending === "import") {
          state.project!.tracks.push(copy(sampleTrack));
          state.project!.dirty = true;
        } else if (pending === "relink") {
          state.project!.tracks[0] = {
            ...copy(sampleTrack),
            sourcePath: "audio.wav",
          };
          state.project!.dirty = true;
        } else {
          state.project!.tracks[0].status = "ready";
          state.project!.tracks[0].analysis = copy(sampleTrack.analysis);
          state.project!.tracks[0].error = null;
        }
      } else if (pending === "analyze") {
        state.project!.tracks[0].status =
          outcome === "cancelled" ? "unchecked" : failureState;
        state.project!.tracks[0].analysis = null;
        state.project!.tracks[0].error = "source unavailable";
      }
      state.job = {
        id: jobId,
        projectId: state.project!.id,
        generation: state.generation,
        trackId:
          pending !== "import" || outcome === "succeeded"
            ? sampleTrack.id
            : null,
        phase: outcome,
        error: outcome === "failed" ? "source unavailable" : null,
      };
      return copy(state.job);
    }),
    cancelJob: vi.fn(async () => {
      outcome = "cancelled";
    }),
    waveform: vi.fn(async (ref) => ({ ...fixture.waveform, ...ref })),
    loadPlayback: vi.fn(async (ref, trackId, position) => {
      playback = {
        ...playback,
        ...ref,
        trackId,
        position,
        phase: position === 0 ? "stopped" : "paused",
      };
      return copy(playback);
    }),
    play: vi.fn(async () => {
      playback.phase = "playing";
      return copy(playback);
    }),
    pause: vi.fn(async () => {
      playback.phase = playback.position === 0 ? "stopped" : "paused";
      return copy(playback);
    }),
    seek: vi.fn(async (_, __, position) => {
      playback.position = position;
      return copy(playback);
    }),
    stop: vi.fn(async () => {
      playback.phase = "stopped";
      playback.position = 0;
      return copy(playback);
    }),
    playbackStatus: vi.fn(async () => copy(playback)),
  };
  const c = createController(api);
  controllers.push(c);
  return {
    c,
    api,
    setOutcome: (
      next: typeof outcome,
      status: typeof failureState = "offline",
    ) => {
      outcome = next;
      failureState = status;
    },
    state: () => state,
  };
}
afterEach(() => {
  controllers.splice(0).forEach((c) => c.dispose());
  vi.useRealTimers();
});
it("New → Import → Save → Close/Open → verify/analyze uses project/track identity", async () => {
  const { c, api } = setup();
  await c.initialize();
  await c.newProject("Song");
  await c.importFile();
  await settle();
  expect(c.store.getState().workspace.project?.dirty).toBe(true);
  expect(c.store.getState().waveform?.trackId).toBe(sampleTrack.id);
  await c.saveProject();
  expect(c.store.getState().workspace.project?.dirty).toBe(false);
  await c.closeProject();
  expect(c.store.getState().workspace.project).toBeNull();
  await c.openProject();
  await settle();
  expect(api.analyzeTrack).toHaveBeenCalledWith(
    { projectId: sampleProject.id, generation: 3 },
    sampleTrack.id,
  );
  expect(c.store.getState().workspace.project?.dirty).toBe(false);
  expect(c.store.getState().waveform?.generation).toBe(3);
  await c.saveProject(true);
  expect(api.chooseSavePath).toHaveBeenCalledTimes(2);
});
it("dirty close requires explicit discard; cancelled dialog preserves project", async () => {
  const { c, api } = setup(sample);
  await c.initialize();
  await settle();
  expect(await c.closeProject()).toBe(false);
  expect(api.closeProject).not.toHaveBeenCalled();
  vi.mocked(api.confirmDiscard).mockResolvedValue(true);
  expect(await c.closeProject()).toBe(true);
  expect(c.store.getState().workspace.project).toBeNull();
});
it("cancelled Open or Save As does not mutate project or dirty state", async () => {
  const { c, api } = setup(sample);
  await c.initialize();
  await settle();
  vi.mocked(api.chooseFile).mockResolvedValue(null);
  vi.mocked(api.chooseSavePath).mockResolvedValue(null);
  await c.openProject();
  await c.saveProject(true);
  expect(api.openProject).not.toHaveBeenCalled();
  expect(api.saveProject).not.toHaveBeenCalled();
  expect(c.store.getState().workspace.project?.dirty).toBe(true);
});
it("failed import preserves existing tracks and waveform", async () => {
  const { c, setOutcome } = setup(sample);
  await c.initialize();
  await settle();
  setOutcome("failed");
  await c.importFile();
  await settle();
  expect(c.store.getState().workspace.project?.tracks).toHaveLength(1);
  expect(c.store.getState().waveform).not.toBeNull();
  expect(c.store.getState().error).toBe("source unavailable");
});
it.each(["offline", "changed"] as const)(
  "opened project remains clean when source is %s",
  async (status) => {
    const data = copy(sample);
    data.project!.dirty = false;
    data.project!.tracks[0].status = "unchecked";
    data.project!.tracks[0].analysis = null;
    data.job = null;
    const { c, setOutcome } = setup(data);
    setOutcome("failed", status);
    await c.initialize();
    await settle();
    expect(c.store.getState().workspace.project?.tracks[0].status).toBe(status);
    expect(c.store.getState().workspace.project?.dirty).toBe(false);
    expect(c.store.getState().waveform).toBeNull();
  },
);
it("old viewport response cannot populate a newly opened generation of the same project", async () => {
  const old = deferred<Waveform>();
  const { c, api } = setup(sample);
  vi.mocked(api.waveform).mockReturnValueOnce(old.promise);
  await c.initialize();
  vi.mocked(api.confirmDiscard).mockResolvedValue(true);
  await c.newProject("Next");
  old.resolve(fixture.waveform);
  await settle();
  expect(c.store.getState().workspace.generation).toBe(sample.generation + 1);
  expect(c.store.getState().waveform).toBeNull();
});
it("latest viewport wins, bounds are clamped and analysis is not restarted", async () => {
  const old = deferred<Waveform>();
  const { c, api } = setup(sample);
  vi.mocked(api.waveform).mockReturnValueOnce(old.promise);
  await c.initialize();
  c.view(-1, -0.99, 9999);
  await settle();
  old.reject(new Error("stale"));
  await settle();
  expect(c.store.getState().viewport.width).toBe(4096);
  expect(c.store.getState().viewport.start).toBe(0);
  expect(c.store.getState().queryError).toBeNull();
  expect(api.analyzeTrack).not.toHaveBeenCalled();
});
it("cancel remains busy until terminal status; duplicate imports are blocked", async () => {
  vi.useFakeTimers();
  const { c, api } = setup(sample);
  await c.initialize();
  await settle();
  const running: Job = {
    id: jobId,
    projectId: sampleProject.id,
    generation: sample.generation,
    trackId: null,
    phase: "running",
    error: null,
  };
  vi.mocked(api.jobStatus).mockResolvedValueOnce(running);
  await c.importFile();
  await settle();
  await c.importFile();
  expect(api.startImport).toHaveBeenCalledTimes(1);
  await c.cancel();
  expect(c.store.getState().busy).toBe(true);
  await vi.advanceTimersByTimeAsync(200);
  expect(c.store.getState().busy).toBe(false);
  expect(c.store.getState().workspace.project?.tracks).toHaveLength(1);
});
it("reload resumes polling an existing job instead of enabling a second import", async () => {
  vi.useFakeTimers();
  const data = copy(sample);
  data.job = {
    id: jobId,
    projectId: sampleProject.id,
    generation: sample.generation,
    trackId: null,
    phase: "running",
    error: null,
  };
  const { c, api } = setup(data);
  vi.mocked(api.jobStatus).mockResolvedValue(data.job);
  await c.initialize();
  await settle();
  expect(c.store.getState().busy).toBe(true);
  expect(api.jobStatus).toHaveBeenCalledWith(jobId);
  c.dispose();
  await vi.advanceTimersByTimeAsync(2000);
  expect(api.jobStatus).toHaveBeenCalledTimes(1);
});
it("transport errors retry without enabling conflicting operations", async () => {
  vi.useFakeTimers();
  const { c, api } = setup(sample);
  await c.initialize();
  await settle();
  vi.mocked(api.jobStatus).mockRejectedValueOnce(new Error("transport"));
  await c.importFile();
  await settle();
  expect(c.store.getState().busy).toBe(true);
  await vi.advanceTimersByTimeAsync(1000);
  expect(c.store.getState().busy).toBe(false);
});
it("save error preserves dirty/path and current waveform", async () => {
  const { c, api } = setup(sample);
  await c.initialize();
  await settle();
  vi.mocked(api.saveProject).mockRejectedValue(new Error("disk"));
  await c.saveProject();
  expect(c.store.getState().workspace.project?.dirty).toBe(true);
  expect(c.store.getState().workspace.project?.path).toBeNull();
  expect(c.store.getState().waveform).not.toBeNull();
});
it("DTO checks real Rust evidence and rejects wrong project/generation/track or bounds", () => {
  const ref = { projectId: sampleProject.id, generation: sample.generation },
    view = { start: 0, end: 0.02, width: 2 };
  expect(
    validateWaveform(fixture.waveform, ref, sampleTrack, view).trackId,
  ).toBe(sampleTrack.id);
  for (const change of [
    { projectId: jobId },
    { generation: 999 },
    { trackId: jobId },
    { channels: [Array(4).fill(fixture.waveform.channels[0][0]), []] },
  ])
    expect(() =>
      validateWaveform(
        { ...fixture.waveform, ...change },
        ref,
        sampleTrack,
        view,
      ),
    ).toThrow();
  const invalid = copy(sample);
  invalid.project!.tracks[0].analysis!.confidence.score = 1;
  expect(() => workspaceSchema.parse(invalid)).toThrow();
});

it("relink restores an offline track and saves its new location", async () => {
  const offline = copy(sample);
  offline.project!.dirty = false;
  offline.project!.tracks[0].status = "offline";
  offline.project!.tracks[0].analysis = null;
  const { c, api } = setup(offline);
  await c.initialize();
  await c.relinkTrack();
  await settle();
  expect(api.relinkTrack).toHaveBeenCalledWith(
    { projectId: sampleProject.id, generation: sample.generation },
    sampleTrack.id,
    "audio.wav",
  );
  const project = c.store.getState().workspace.project!;
  expect(project.id).toBe(sampleProject.id);
  expect(project.dirty).toBe(true);
  expect(project.tracks[0]).toMatchObject({
    id: sampleTrack.id,
    sourceId: sampleTrack.sourceId,
    sourcePath: "audio.wav",
    status: "ready",
    analysis: sampleTrack.analysis,
  });
  expect(c.store.getState().waveform).not.toBeNull();
  await c.saveProject();
  expect(c.store.getState().workspace.project!.dirty).toBe(false);
});
it("playback loads from the viewport, polls identity, seeks, pauses and stops", async () => {
  vi.useFakeTimers();
  const { c, api } = setup(sample);
  await c.initialize();
  await settle();
  c.view(0.005, 0.015);
  await settle();
  await c.togglePlayback();
  await settle();
  expect(api.loadPlayback).toHaveBeenCalledWith(
    { projectId: sampleProject.id, generation: sample.generation },
    sampleTrack.id,
    0.005,
  );
  expect(c.store.getState().playback?.phase).toBe("playing");
  expect(api.playbackStatus).toHaveBeenCalled();
  await c.togglePlayback();
  expect(c.store.getState().playback?.phase).toBe("paused");
  await c.seekPlayback(0.01);
  expect(c.store.getState().playback?.position).toBe(0.01);
  await c.stopPlayback();
  expect(c.store.getState().playback).toMatchObject({
    phase: "stopped",
    position: 0,
  });
  c.dispose();
  await vi.advanceTimersByTimeAsync(1000);
  expect(api.playbackStatus).toHaveBeenCalledTimes(1);
});
it.each(["failed", "cancelled"] as const)(
  "%s relink preserves the original track and evidence",
  async (outcome) => {
    vi.useFakeTimers();
    const { c, api, setOutcome } = setup(sample);
    await c.initialize();
    await settle();
    const before = copy(c.store.getState().workspace.project);
    const pending = deferred<Job>();
    vi.mocked(api.jobStatus).mockReturnValueOnce(pending.promise);
    setOutcome(outcome);
    await c.relinkTrack();
    expect(c.store.getState().workspace.project).toEqual(before);
    expect(c.store.getState().waveform).not.toBeNull();
    await c.importFile();
    expect(api.startImport).not.toHaveBeenCalled();
    if (outcome === "cancelled") await c.cancel();
    pending.resolve({
      id: jobId,
      projectId: sampleProject.id,
      generation: sample.generation,
      trackId: sampleTrack.id,
      phase: "running",
      error: null,
    });
    await settle();
    await vi.advanceTimersByTimeAsync(200);
    await settle();
    expect(c.store.getState().workspace.project).toEqual(before);
    expect(c.store.getState().busy).toBe(false);
    expect(c.store.getState().waveform).not.toBeNull();
  },
);
it("cancelled relink picker and transport failure retain the current projection", async () => {
  const { c, api } = setup(sample);
  await c.initialize();
  await settle();
  const before = copy(c.store.getState().workspace.project);
  vi.mocked(api.chooseFile).mockResolvedValueOnce(null);
  await c.relinkTrack();
  expect(api.relinkTrack).not.toHaveBeenCalled();
  vi.mocked(api.relinkTrack).mockRejectedValueOnce(new Error("unavailable"));
  await c.relinkTrack();
  expect(c.store.getState().workspace.project).toEqual(before);
  expect(c.store.getState().waveform).not.toBeNull();
  expect(c.store.getState().busy).toBe(false);
});
it("disposed relink request cancels its eventual backend job", async () => {
  const { c, api } = setup(sample);
  await c.initialize();
  await settle();
  const pending = deferred<string>();
  vi.mocked(api.relinkTrack).mockReturnValueOnce(pending.promise);
  const work = c.relinkTrack();
  await settle();
  c.dispose();
  pending.resolve(jobId);
  await work;
  expect(api.cancelJob).toHaveBeenCalledWith(jobId);
  expect(api.jobStatus).not.toHaveBeenCalled();
});

it("opened moved project keeps the backend resolved source path without semantic edits", async () => {
  const moved = copy(sample);
  moved.project!.dirty = false;
  moved.project!.path = "D:/moved/song.vocalproj";
  moved.project!.tracks[0].sourcePath = "D:/moved/audio/voice.wav";
  moved.project!.tracks[0].status = "unchecked";
  moved.project!.tracks[0].analysis = null;
  const { c, api } = setup(moved);
  await c.initialize();
  await settle();
  expect(api.analyzeTrack).toHaveBeenCalledWith(
    { projectId: sampleProject.id, generation: sample.generation },
    sampleTrack.id,
  );
  expect(c.store.getState().workspace.project!.dirty).toBe(false);
  expect(c.activeTrack()!.sourcePath).toBe("D:/moved/audio/voice.wav");
  expect(c.store.getState().waveform).not.toBeNull();
  await c.saveProject(true);
  expect(c.activeTrack()!.sourcePath).toBe("D:/moved/audio/voice.wav");
  expect(c.store.getState().waveform).not.toBeNull();
});
