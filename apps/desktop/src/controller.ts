import { createStore } from "zustand/vanilla";
import {
  validatePlayback,
  validateWaveform,
  type Backend,
  type Workspace,
  type Track,
  type Viewport,
  type Waveform,
  type Job,
  type ProjectRef,
  type Playback,
} from "./model";
interface State {
  workspace: Workspace;
  activeTrackId: string | null;
  viewport: Viewport;
  waveform: Waveform | null;
  busy: boolean;
  querying: boolean;
  error: string | null;
  queryError: string | null;
  playback: Playback | null;
  transportBusy: boolean;
  playbackError: string | null;
  notice: string;
}
const message = (e: unknown) => (e instanceof Error ? e.message : String(e));
export const terminal = (job: Job) =>
  !["running", "cancelling"].includes(job.phase);
export function createController(api: Backend) {
  const store = createStore<State>(() => ({
    workspace: { generation: 0, project: null, job: null },
    activeTrackId: null,
    viewport: { start: 0, end: 1, width: 800 },
    waveform: null,
    busy: false,
    querying: false,
    error: null,
    queryError: null,
    playback: null,
    transportBusy: false,
    playbackError: null,
    notice: "新建或打开工程",
  }));
  let alive = true,
    request = 0,
    operation = 0,
    playbackRequest = 0;
  let timer: ReturnType<typeof setTimeout> | undefined;
  let playbackTimer: ReturnType<typeof setTimeout> | undefined;
  const active = () =>
    store
      .getState()
      .workspace.project?.tracks.find(
        (t) => t.id === store.getState().activeTrackId,
      ) ?? null;
  const ref = (): ProjectRef => {
    const w = store.getState().workspace;
    if (!w.project) throw new Error("请先新建或打开工程");
    return { projectId: w.project.id, generation: w.generation };
  };
  const current = (revision: number) => alive && operation === revision;
  function publish(workspace: Workspace, preferred?: string | null) {
    const old = store.getState();
    const changed =
      old.workspace.generation !== workspace.generation ||
      old.workspace.project?.id !== workspace.project?.id;
    const tracks = workspace.project?.tracks ?? [];
    const selected =
      tracks.find(
        (t) => t.id === (preferred ?? (changed ? null : old.activeTrackId)),
      ) ?? tracks[0];
    const reset = changed || selected?.id !== old.activeTrackId;
    ++request;
    if (reset) {
      ++playbackRequest;
      clearTimeout(playbackTimer);
    }
    store.setState({
      workspace,
      activeTrackId: selected?.id ?? null,
      waveform: null,
      querying: false,
      queryError: null,
      ...(reset
        ? { playback: null, transportBusy: false, playbackError: null }
        : {}),
      ...(reset
        ? {
            viewport: {
              ...old.viewport,
              start: 0,
              end: selected?.duration ?? 1,
            },
          }
        : {}),
    });
  }
  function playbackIdentity() {
    const track = active();
    if (!track) throw new Error("请选择轨道");
    return { reference: ref(), track };
  }
  function acceptPlayback(
    raw: unknown,
    reference: ProjectRef,
    track: Track,
    stamp: number,
  ) {
    if (!alive || stamp !== playbackRequest || active()?.id !== track.id)
      return null;
    const value = validatePlayback(raw, reference, track);
    store.setState({
      playback: value,
      transportBusy: false,
      playbackError: null,
    });
    return value;
  }
  async function pollPlayback(
    reference: ProjectRef,
    track: Track,
    stamp: number,
  ) {
    try {
      const raw = await api.playbackStatus(reference, track.id);
      const value = acceptPlayback(raw, reference, track, stamp);
      if (value?.phase === "playing")
        playbackTimer = setTimeout(
          () => void pollPlayback(reference, track, stamp),
          50,
        );
    } catch (error) {
      if (alive && stamp === playbackRequest)
        store.setState({
          transportBusy: false,
          playbackError: message(error),
        });
    }
  }
  async function query() {
    const track = active(),
      stamp = ++request;
    if (!track?.analysis) return;
    const reference = ref(),
      view = store.getState().viewport;
    store.setState({ querying: true, waveform: null });
    try {
      const value = await api.waveform(reference, track, view);
      if (alive && stamp === request)
        store.setState({
          waveform: validateWaveform(value, reference, track, view),
          querying: false,
          queryError: null,
        });
    } catch (e) {
      if (alive && stamp === request)
        store.setState({ querying: false, queryError: message(e) });
    }
  }
  function begin(): number | null {
    if (store.getState().busy) return null;
    store.setState({ busy: true, error: null });
    return ++operation;
  }
  async function poll(id: string, revision: number) {
    try {
      const job = await api.jobStatus(id);
      if (!current(revision)) return;
      const reference = ref();
      if (
        job.id !== id ||
        job.projectId !== reference.projectId ||
        job.generation !== reference.generation
      )
        throw new Error("任务响应与当前工程不匹配");
      store.setState({ workspace: { ...store.getState().workspace, job } });
      if (!terminal(job)) {
        timer = setTimeout(() => void poll(id, revision), 200);
        return;
      }
      const workspace = await api.currentProject();
      if (!current(revision)) return;
      if (
        workspace.generation !== reference.generation ||
        workspace.project?.id !== reference.projectId
      )
        throw new Error("工程已切换，请重新打开");
      publish(
        workspace,
        job.phase === "succeeded"
          ? job.trackId
          : store.getState().activeTrackId,
      );
      store.setState({
        busy: false,
        error: job.phase === "failed" ? job.error : null,
        notice:
          job.phase === "succeeded"
            ? "波形已就绪"
            : job.phase === "cancelled"
              ? "已取消，工程未被分析任务修改"
              : "分析失败，请检查源文件状态",
      });
      void query();
    } catch (e) {
      if (current(revision)) {
        store.setState({
          error: message(e),
          notice: "无法读取任务状态，正在重试",
        });
        timer = setTimeout(() => void poll(id, revision), 1000);
      }
    }
  }
  function jobStarted(
    id: string,
    revision: number,
    trackId: string | null,
    preserve = false,
  ) {
    const reference = ref();
    const workspace = store.getState().workspace;
    store.setState({
      workspace: {
        ...workspace,
        project: workspace.project
          ? {
              ...workspace.project,
              tracks: workspace.project.tracks.map((t) =>
                t.id === trackId && !preserve
                  ? { ...t, status: "analyzing", analysis: null, error: null }
                  : t,
              ),
            }
          : null,
        job: { id, ...reference, trackId, phase: "running", error: null },
      },
      notice: "正在核验音频并分析",
      playback: null,
      transportBusy: false,
      playbackError: null,
    });
    ++playbackRequest;
    clearTimeout(playbackTimer);
    void poll(id, revision);
  }
  async function ensureWaveform() {
    const track = active();
    if (!track) return;
    if (track.status === "ready") {
      void query();
      return;
    }
    if (track.status !== "unchecked") return;
    const revision = begin();
    if (revision === null) return;
    try {
      const id = await api.analyzeTrack(ref(), track.id);
      if (current(revision)) jobStarted(id, revision, track.id);
      else await api.cancelJob(id);
    } catch (e) {
      if (current(revision)) store.setState({ busy: false, error: message(e) });
    }
  }
  async function switchProject(
    kind: "new" | "open" | "close",
    name = "Untitled",
  ): Promise<boolean> {
    const revision = begin();
    if (revision === null) return false;
    try {
      const state = store.getState().workspace;
      // File selection first; cancelling Open must not discard the current project.
      const path = kind === "open" ? await api.chooseFile("project") : null;
      if (!current(revision)) return false;
      if (kind === "open" && !path) {
        store.setState({ busy: false });
        return false;
      }
      const discard = state.project?.dirty ? await api.confirmDiscard() : false;
      if (!current(revision)) return false;
      if (state.project?.dirty && !discard) {
        store.setState({ busy: false });
        return false;
      }
      const result =
        kind === "new"
          ? await api.newProject(name, state.generation, discard)
          : kind === "open"
            ? await api.openProject(path!, state.generation, discard)
            : await api.closeProject(state.generation, discard);
      if (!current(revision)) return false;
      publish(result);
      store.setState({
        busy: false,
        notice:
          kind === "close"
            ? "工程已关闭"
            : kind === "new"
              ? "工程已新建"
              : "工程已打开",
      });
      void ensureWaveform();
      return true;
    } catch (e) {
      if (current(revision)) store.setState({ busy: false, error: message(e) });
      return false;
    }
  }
  return {
    store,
    activeTrack: active,
    async initialize() {
      const revision = begin();
      if (revision === null) return;
      try {
        const result = await api.currentProject();
        if (!current(revision)) return;
        publish(result);
        if (result.job && !terminal(result.job)) {
          jobStarted(result.job.id, revision, result.job.trackId, true);
        } else {
          store.setState({
            busy: false,
            notice: result.project ? "工程已恢复" : "新建或打开工程",
          });
          void ensureWaveform();
        }
      } catch (e) {
        if (current(revision))
          store.setState({ busy: false, error: message(e) });
      }
    },
    newProject: (name: string) => switchProject("new", name),
    openProject: () => switchProject("open"),
    closeProject: () => switchProject("close"),
    async saveProject(as = false) {
      const revision = begin();
      if (revision === null) return;
      try {
        const reference = ref(),
          project = store.getState().workspace.project!;
        let path: string | null = null;
        if (as || !project.path) {
          path = await api.chooseSavePath(project.name);
          if (!current(revision)) return;
          if (!path) {
            store.setState({ busy: false });
            return;
          }
        }
        const saved = await api.saveProject(reference, path);
        if (!current(revision)) return;
        // Save does not change derived state or viewport; retain the currently rendered wave.
        store.setState({ workspace: saved, busy: false, notice: "工程已保存" });
      } catch (e) {
        if (current(revision))
          store.setState({ busy: false, error: message(e) });
      }
    },
    async importFile() {
      const revision = begin();
      if (revision === null) return;
      try {
        const reference = ref(),
          path = await api.chooseFile("audio");
        if (!current(revision)) return;
        if (!path) {
          store.setState({ busy: false });
          return;
        }
        const id = await api.startImport(reference, path);
        if (!current(revision)) {
          await api.cancelJob(id);
          return;
        }
        jobStarted(id, revision, null);
      } catch (e) {
        if (current(revision))
          store.setState({ busy: false, error: message(e) });
      }
    },
    async relinkTrack() {
      const track = active();
      if (!track) return;
      const revision = begin();
      if (revision === null) return;
      try {
        const reference = ref(),
          path = await api.chooseFile("audio");
        if (!current(revision)) return;
        if (!path) {
          store.setState({ busy: false });
          return;
        }
        const id = await api.relinkTrack(reference, track.id, path);
        if (current(revision)) jobStarted(id, revision, track.id, true);
        else await api.cancelJob(id);
      } catch (e) {
        if (current(revision))
          store.setState({ busy: false, error: message(e) });
      }
    },
    async retryTrack() {
      const track = active();
      if (!track) return;
      const revision = begin();
      if (revision === null) return;
      try {
        ++request;
        store.setState({ waveform: null });
        const id = await api.analyzeTrack(ref(), track.id);
        if (current(revision)) jobStarted(id, revision, track.id);
        else await api.cancelJob(id);
      } catch (e) {
        if (current(revision)) {
          store.setState({ busy: false, error: message(e) });
          void query();
        }
      }
    },
    async togglePlayback() {
      const { reference, track } = playbackIdentity();
      if (track.status !== "ready" || store.getState().transportBusy) return;
      const stamp = ++playbackRequest;
      clearTimeout(playbackTimer);
      store.setState({ transportBusy: true, playbackError: null });
      try {
        const currentPlayback = store.getState().playback;
        const raw =
          currentPlayback?.trackId === track.id &&
          currentPlayback.phase === "playing"
            ? await api.pause(reference, track.id)
            : currentPlayback?.trackId === track.id
              ? await api.play(reference, track.id)
              : (await api.loadPlayback(
                  reference,
                  track.id,
                  store.getState().viewport.start,
                ),
                await api.play(reference, track.id));
        const value = acceptPlayback(raw, reference, track, stamp);
        if (value?.phase === "playing") void pollPlayback(reference, track, stamp);
      } catch (error) {
        if (alive && stamp === playbackRequest)
          store.setState({
            transportBusy: false,
            playbackError: message(error),
          });
      }
    },
    async stopPlayback() {
      const currentPlayback = store.getState().playback;
      const track = active();
      if (!track || currentPlayback?.trackId !== track.id) return;
      const reference = ref(),
        stamp = ++playbackRequest;
      clearTimeout(playbackTimer);
      store.setState({ transportBusy: true, playbackError: null });
      try {
        acceptPlayback(
          await api.stop(reference, track.id),
          reference,
          track,
          stamp,
        );
      } catch (error) {
        if (alive && stamp === playbackRequest)
          store.setState({
            transportBusy: false,
            playbackError: message(error),
          });
      }
    },
    async seekPlayback(position: number) {
      const { reference, track } = playbackIdentity();
      if (
        track.status !== "ready" ||
        !Number.isFinite(position) ||
        position < 0 ||
        position > track.duration
      )
        return;
      const stamp = ++playbackRequest;
      clearTimeout(playbackTimer);
      store.setState({ transportBusy: true, playbackError: null });
      try {
        const currentPlayback = store.getState().playback;
        const raw =
          currentPlayback?.trackId === track.id
            ? await api.seek(reference, track.id, position)
            : await api.loadPlayback(reference, track.id, position);
        const value = acceptPlayback(raw, reference, track, stamp);
        if (value?.phase === "playing") void pollPlayback(reference, track, stamp);
      } catch (error) {
        if (alive && stamp === playbackRequest)
          store.setState({
            transportBusy: false,
            playbackError: message(error),
          });
      }
    },
    selectTrack(id: string) {
      if (store.getState().busy) return;
      const previous = active(),
        previousPlayback = store.getState().playback;
      const track = store
        .getState()
        .workspace.project?.tracks.find((t) => t.id === id);
      if (!track) return;
      if (previous && previousPlayback?.trackId === previous.id)
        void api.stop(ref(), previous.id).catch(() => undefined);
      ++request;
      ++playbackRequest;
      clearTimeout(playbackTimer);
      store.setState({
        activeTrackId: id,
        waveform: null,
        queryError: null,
        playback: null,
        transportBusy: false,
        playbackError: null,
        viewport: {
          ...store.getState().viewport,
          start: 0,
          end: track.duration,
        },
      });
      void ensureWaveform();
    },
    async cancel() {
      const job = store.getState().workspace.job;
      if (!job || terminal(job)) return;
      try {
        await api.cancelJob(job.id);
        if (alive) store.setState({ notice: "正在取消" });
      } catch (e) {
        if (alive) store.setState({ error: message(e) });
      }
    },
    view(start: number, end: number, width = store.getState().viewport.width) {
      const track = active();
      if (
        !track ||
        !Number.isFinite(start) ||
        !Number.isFinite(end) ||
        end <= start ||
        !Number.isFinite(width)
      )
        return;
      const span = Math.min(track.duration, end - start);
      start = Math.max(0, Math.min(start, track.duration - span));
      store.setState({
        viewport: {
          start,
          end: start + span,
          width: Math.max(1, Math.min(4096, Math.round(width))),
        },
      });
      void query();
    },
    dispose() {
      alive = false;
      ++operation;
      ++request;
      clearTimeout(timer);
      clearTimeout(playbackTimer);
    },
  };
}
export type Controller = ReturnType<typeof createController>;
