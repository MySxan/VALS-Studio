import { useCallback } from "react";
import { useStore } from "zustand";
import type { Controller } from "./controller";
import { Waveform } from "./Waveform";

export function App({
  controller,
  available,
  preview,
}: {
  controller: Controller;
  available: boolean;
  preview: boolean;
}) {
  const state = useStore(controller.store);
  const { session, viewport: view } = state;
  const resize = useCallback(
    (width: number) => {
      const current = controller.store.getState().viewport;
      if (width !== current.width)
        controller.view(current.start, current.end, width);
    },
    [controller],
  );
  function zoom(factor: number) {
    if (!session) return;
    const middle = (view.start + view.end) / 2;
    const span = Math.max(
      1 / session.sampleRate,
      (view.end - view.start) * factor,
    );
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
          disabled={!available || state.busy || preview}
          onClick={() => void controller.importFile()}
        >
          ＋ 导入 WAV
        </button>
      </header>
      <section className="title">
        <div>
          <p className="eyebrow">PHASE 0 / WAVEFORM</p>
          <h1>声音，从这里展开。</h1>
          <p>原始声道 · min / max / RMS · 可追溯分析</p>
        </div>
        <span className="badge">
          {preview ? "开发预览 · Rust fixture" : "单轨预览"}
        </span>
      </section>
      {!available && (
        <p className="banner">
          请通过 Tauri 桌面应用打开，以选择本地 WAV 文件。
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
            <span className="secondary">
              {session
                ? `${session.sampleRate.toLocaleString()} Hz · ${session.channels} CH`
                : "尚无音频"}
            </span>
            <div className="controls">
              <button
                disabled={!session}
                onClick={() => pan(-1)}
                aria-label="向左平移"
              >
                ←
              </button>
              <button
                disabled={!session}
                onClick={() => pan(1)}
                aria-label="向右平移"
              >
                →
              </button>
              <button
                disabled={!session}
                onClick={() => zoom(2)}
                aria-label="缩小"
              >
                −
              </button>
              <button
                disabled={!session}
                onClick={() => zoom(0.5)}
                aria-label="放大"
              >
                ＋
              </button>
              <button
                disabled={!session}
                onClick={() => session && controller.view(0, session.duration)}
              >
                适合窗口
              </button>
            </div>
          </div>
          {session ? (
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
                session={session}
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
              <h2>导入第一段人声</h2>
              <p>
                选择单声道或立体声 PCM WAV。
                <br />
                分析保留原始采样率与声道，原文件不会被修改。
              </p>
              <small>当前解码上限：64 MiB PCM</small>
            </div>
          )}
          <footer role="status">
            <span className={state.busy ? "pulse" : ""}>●</span> {state.notice}
            {state.busy && state.job && (
              <button onClick={() => void controller.cancel()}>取消分析</button>
            )}
          </footer>
        </section>
        <aside>
          <p className="eyebrow">INSPECTOR</p>
          <h2>分析依据</h2>
          {session ? (
            <>
              <dl>
                <dt>时长</dt>
                <dd>{session.duration.toFixed(4)} s</dd>
                <dt>置信类型</dt>
                <dd>{session.confidence.kind}</dd>
                <dt>概率分数</dt>
                <dd>
                  {session.confidence.score === null
                    ? "不适用（测量值）"
                    : session.confidence.score}
                </dd>
                <dt>Provider</dt>
                <dd>{session.provenance.provider}</dd>
                <dt>Analyzer</dt>
                <dd>
                  {session.provenance.analyzerId} /{" "}
                  {session.provenance.analyzerVersion}
                </dd>
              </dl>
              <p className="explanation">{session.confidence.explanation}</p>
              <details>
                <summary>完整 provenance</summary>
                <pre>{JSON.stringify(session.provenance, null, 2)}</pre>
                <p>Artifact</p>
                <code>{session.artifactHash}</code>
              </details>
            </>
          ) : (
            <p className="muted">导入后显示分析来源、置信语义和依赖标识。</p>
          )}
          <div className="aside-note">
            预览会话
            <br />
            <span>本阶段不写入或修改工程。</span>
          </div>
        </aside>
      </div>
      <div className="bottom">
        VALS STUDIO <span>LOCAL FIRST · PROVIDER INDEPENDENT</span>
      </div>
    </main>
  );
}
