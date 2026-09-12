import { useCallback, useState } from "react";
import { useStore } from "zustand";
import { terminal, type Controller } from "./controller";
import { Waveform } from "./Waveform";
const labels = {
  unchecked: "待核验",
  analyzing: "分析中",
  ready: "已就绪",
  offline: "源文件离线",
  changed: "源文件已变化",
  error: "分析失败",
};
export function App({
  controller,
  available,
  preview,
}: {
  controller: Controller;
  available: boolean;
  preview: boolean;
}) {
  const state = useStore(controller.store),
    project = state.workspace.project,
    track = controller.activeTrack(),
    view = state.viewport,
    analysis = track?.analysis;
  const [name, setName] = useState("Untitled");
  const disabled = !available || state.busy || preview;
  const resize = useCallback(
    (width: number) => {
      const current = controller.store.getState().viewport;
      if (width !== current.width)
        controller.view(current.start, current.end, width);
    },
    [controller],
  );
  function zoom(factor: number) {
    if (!track) return;
    const middle = (view.start + view.end) / 2,
      span = Math.max(1 / track.sampleRate, (view.end - view.start) * factor);
    controller.view(middle - span / 2, middle + span / 2);
  }
  function pan(direction: number) {
    const delta = (view.end - view.start) * direction * 0.5;
    controller.view(view.start + delta, view.end + delta);
  }
  return (
    <main>
      <header>
        <div className="brand">
          <span className="mark">V</span>
          <div>
            VALS <b>Studio</b>
            <small>VOCAL ANALYSIS WORKSPACE</small>
          </div>
        </div>
        <span className="local">● 本地处理</span>
        <button
          className="primary"
          disabled={disabled || !project}
          onClick={() => void controller.importFile()}
        >
          ＋ Import WAV
        </button>
      </header>
      <section className="project-actions" aria-label="工程操作">
        <input
          aria-label="新工程名称"
          value={name}
          onChange={(e) => setName(e.target.value)}
          placeholder="工程名称"
        />
        <button
          disabled={disabled}
          onClick={() => void controller.newProject(name)}
        >
          New
        </button>
        <button
          disabled={disabled}
          onClick={() => void controller.openProject()}
        >
          Open
        </button>
        <button
          disabled={disabled || !project}
          onClick={() => void controller.saveProject()}
        >
          Save
        </button>
        <button
          disabled={disabled || !project}
          onClick={() => void controller.saveProject(true)}
        >
          Save As
        </button>
        <button
          disabled={disabled || !project}
          onClick={() => void controller.closeProject()}
        >
          Close
        </button>
      </section>
      <section className="title">
        <div>
          <p className="eyebrow">PROJECT WORKSPACE</p>
          <h1>
            {project?.name ?? "新建或打开工程"}
            {project?.dirty ? " *" : ""}
          </h1>
          <p>
            {project
              ? project.dirty
                ? "有未保存修改"
                : "已保存"
              : "WAV · Waveform · VocalProject"}
            {project?.path ? ` · ${project.path}` : ""}
          </p>
        </div>
        <span className="badge">
          {preview
            ? "开发预览 · Rust fixture"
            : `${project?.tracks.length ?? 0} tracks`}
        </span>
      </section>
      {!available && (
        <p className="banner">
          请通过 Tauri 桌面应用打开，以使用本地工程与音频文件。
        </p>
      )}
      {(state.error || state.queryError) && (
        <div role="alert" className="error">
          {state.error || state.queryError}
        </div>
      )}
      <div className="workspace">
        <section className="editor" aria-label="波形视口">
          <div className="toolbar">
            <strong>波形</strong>
            <select
              aria-label="当前轨道"
              disabled={state.busy || !project?.tracks.length}
              value={state.activeTrackId ?? ""}
              onChange={(e) => controller.selectTrack(e.target.value)}
            >
              {!project?.tracks.length && <option value="">尚无轨道</option>}
              {project?.tracks.map((t) => (
                <option key={t.id} value={t.id}>
                  {t.name} · {labels[t.status]}
                </option>
              ))}
            </select>
            <div className="controls">
              <button
                disabled={!analysis}
                onClick={() => pan(-1)}
                aria-label="向左平移"
              >
                ←
              </button>
              <button
                disabled={!analysis}
                onClick={() => pan(1)}
                aria-label="向右平移"
              >
                →
              </button>
              <button
                disabled={!analysis}
                onClick={() => zoom(2)}
                aria-label="缩小"
              >
                −
              </button>
              <button
                disabled={!analysis}
                onClick={() => zoom(0.5)}
                aria-label="放大"
              >
                ＋
              </button>
              <button
                disabled={!analysis}
                onClick={() => track && controller.view(0, track.duration)}
              >
                适合窗口
              </button>
            </div>
          </div>
          {track && analysis ? (
            <>
              <div className="ruler">
                {Array.from({ length: 5 }, (_, i) => (
                  <span key={i}>
                    {(view.start + ((view.end - view.start) * i) / 4).toFixed(
                      4,
                    )}{" "}
                    s
                  </span>
                ))}
              </div>
              <Waveform
                track={track}
                view={view}
                data={state.waveform}
                resize={resize}
              />
              <div className="wave-footer">
                <span>
                  <i />
                  峰值 <i className="rms" />
                  RMS
                </span>
                <span>
                  {state.querying
                    ? "读取视口…"
                    : `${state.waveform?.framesPerBlock ?? "—"} frames / block`}
                </span>
              </div>
            </>
          ) : (
            <div className="empty">
              <div className="empty-wave">▂ ▄ ▇ ▅ ▃ ▆ █ ▄ ▂</div>
              <h2>
                {track
                  ? labels[track.status]
                  : project
                    ? "导入第一段人声"
                    : "从一个工程开始"}
              </h2>
              <p>
                {track
                  ? (track.error ?? "显示波形时核验音频源；工程数据仍保留。")
                  : project
                    ? "导入单声道或立体声 PCM WAV，然后保存 .vocalproj。"
                    : "点击 New 创建工程，或 Open 打开已有工程。"}
              </p>
              <small>原声道 / 原采样率 · 当前解码上限 64 MiB PCM</small>
            </div>
          )}
          <footer role="status">
            <span className={state.busy ? "pulse" : ""}>●</span>
            {state.notice}
            {state.busy &&
              state.workspace.job &&
              !terminal(state.workspace.job) && (
                <button onClick={() => void controller.cancel()}>
                  取消分析
                </button>
              )}
          </footer>
        </section>
        <aside>
          <p className="eyebrow">INSPECTOR</p>
          <h2>轨道与分析依据</h2>
          {track ? (
            <>
              <dl>
                <dt>源状态</dt>
                <dd>{labels[track.status]}</dd>
                <dt>音频源</dt>
                <dd>{track.sourcePath}</dd>
                <dt>格式 / 时长</dt>
                <dd>
                  {track.sampleRate.toLocaleString()} Hz · {track.channels} CH ·{" "}
                  {track.duration.toFixed(4)} s
                </dd>
              </dl>
              <button
                className="retry"
                disabled={disabled}
                onClick={() => void controller.retryTrack()}
              >
                核验并重新分析
              </button>
              <button
                className="retry"
                disabled={disabled}
                onClick={() => void controller.relinkTrack()}
              >
                重新关联音频
              </button>
              <p className="hint">
                仅接受与原音频内容完全一致的 WAV；成功后需保存工程。
              </p>
              {analysis && (
                <>
                  <dl>
                    <dt>置信类型</dt>
                    <dd>{analysis.confidence.kind}</dd>
                    <dt>概率分数</dt>
                    <dd>
                      {analysis.confidence.score === null
                        ? "不适用（测量值）"
                        : analysis.confidence.score}
                    </dd>
                    <dt>Provider</dt>
                    <dd>{analysis.provenance.provider}</dd>
                  </dl>
                  <p className="explanation">
                    {analysis.confidence.explanation}
                  </p>
                  <details>
                    <summary>完整 provenance</summary>
                    <pre>{JSON.stringify(analysis.provenance, null, 2)}</pre>
                    <code>{analysis.artifactHash}</code>
                  </details>
                </>
              )}
            </>
          ) : (
            <p className="muted">导入音频后显示源状态与分析依据。</p>
          )}
          <div className="aside-note">
            工程保存音频关联与轨道
            <br />
            <span>波形是可重算的派生结果。</span>
          </div>
        </aside>
      </div>
      <div className="bottom">
        VALS STUDIO<span>LOCAL FIRST · PROVIDER INDEPENDENT</span>
      </div>
    </main>
  );
}
