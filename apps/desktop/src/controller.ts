import { createStore } from "zustand/vanilla";
import type { Backend, Job, Session, Viewport, Waveform } from "./model";
import { validateWaveform } from "./model";

interface State {
  session: Session | null;
  viewport: Viewport;
  waveform: Waveform | null;
  busy: boolean;
  querying: boolean;
  job: Job | null;
  error: string | null;
  queryError: string | null;
  notice: string;
}
const message = (error: unknown) =>
  error instanceof Error ? error.message : String(error);

export function createController(api: Backend) {
  const store = createStore<State>(() => ({
    session: null,
    viewport: { start: 0, end: 1, width: 800 },
    waveform: null,
    busy: false,
    querying: false,
    job: null,
    error: null,
    queryError: null,
    notice: "准备就绪",
  }));
  let alive = true;
  let generation = 0;
  let sessionGeneration = 0;
  let timer: ReturnType<typeof setTimeout> | undefined;
  function publish(session: Session | null) {
    generation++;
    store.setState({
      session,
      waveform: null,
      viewport: {
        ...store.getState().viewport,
        start: 0,
        end: session?.duration ?? 1,
      },
    });
    void query();
  }
  async function query() {
    const request = ++generation;
    const { session, viewport } = store.getState();
    if (!session) return;
    store.setState({ querying: true, waveform: null });
    try {
      const raw = await api.waveform(session, viewport);
      if (!alive || request !== generation) return;
      store.setState({
        waveform: validateWaveform(raw, session, viewport),
        querying: false,
        queryError: null,
      });
    } catch (error) {
      if (alive && request === generation)
        store.setState({ querying: false, queryError: message(error) });
    }
  }
  async function poll(id: string) {
    if (!alive) return;
    try {
      const job = await api.jobStatus(id);
      if (!alive) return;
      if (job.id !== id) throw new Error("任务响应 ID 不匹配");
      store.setState({ job });
      if (job.phase === "running" || job.phase === "cancelling") {
        timer = setTimeout(() => void poll(id), 200);
        return;
      }
      if (job.phase === "succeeded") {
        const session = await api.currentSession();
        if (!alive) return;
        if (!session) throw new Error("分析完成但会话不可用");
        publish(session);
      }
      store.setState({
        busy: false,
        error: job.phase === "failed" ? (job.error ?? "分析失败") : null,
        notice:
          job.phase === "succeeded"
            ? "波形已就绪"
            : job.phase === "cancelled"
              ? "已取消，保留原会话"
              : "导入失败，保留原会话",
      });
    } catch (error) {
      // Keep busy while job state is unknown, allowing cancellation and status recovery.
      if (alive) {
        store.setState({
          error: message(error),
          notice: "无法读取任务状态，正在重试",
        });
        timer = setTimeout(() => void poll(id), 1000);
      }
    }
  }
  return {
    store,
    async initialize() {
      const revision = sessionGeneration;
      try {
        const session = await api.currentSession();
        if (alive && revision === sessionGeneration) publish(session);
      } catch (error) {
        if (alive && revision === sessionGeneration)
          store.setState({ error: message(error) });
      }
    },
    async importFile() {
      if (store.getState().busy) return;
      sessionGeneration++;
      store.setState({
        busy: true,
        error: null,
        job: null,
        notice: "选择 WAV 文件",
      });
      try {
        const path = await api.chooseFile();
        if (!alive) return;
        if (!path) {
          store.setState({ busy: false, notice: "未选择文件" });
          return;
        }
        const id = await api.startImport(path);
        if (!alive) {
          await api.cancelJob(id);
          return;
        }
        store.setState({
          job: { id, phase: "running", error: null },
          notice: "正在导入并分析",
        });
        void poll(id);
      } catch (error) {
        if (alive) store.setState({ busy: false, error: message(error) });
      }
    },
    async cancel() {
      const job = store.getState().job;
      if (!job || !store.getState().busy) return;
      try {
        await api.cancelJob(job.id);
        if (alive) store.setState({ notice: "正在取消" });
      } catch (error) {
        if (alive) store.setState({ error: message(error) });
      }
    },
    view(start: number, end: number, width = store.getState().viewport.width) {
      const duration = store.getState().session?.duration;
      if (
        !duration ||
        !Number.isFinite(start) ||
        !Number.isFinite(end) ||
        end <= start ||
        !Number.isFinite(width)
      )
        return;
      const span = Math.min(duration, end - start);
      start = Math.max(0, Math.min(start, duration - span));
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
      generation++;
      clearTimeout(timer);
    },
  };
}
export type Controller = ReturnType<typeof createController>;
