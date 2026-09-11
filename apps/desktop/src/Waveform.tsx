import { useEffect, useRef } from "react";
import type { Session, Viewport, Waveform as WaveformData } from "./model";

export function Waveform({
  session,
  view,
  data,
  resize,
}: {
  session: Session;
  view: Viewport;
  data: WaveformData | null;
  resize: (width: number) => void;
}) {
  const canvas = useRef<HTMLCanvasElement>(null);
  useEffect(() => {
    const observer = new ResizeObserver((entries) =>
      resize(
        Math.min(4096, Math.max(1, Math.floor(entries[0].contentRect.width))),
      ),
    );
    observer.observe(canvas.current!);
    return () => observer.disconnect();
  }, [resize]);
  useEffect(() => {
    const element = canvas.current!;
    const ratio = window.devicePixelRatio || 1;
    const width = element.clientWidth,
      height = element.clientHeight;
    element.width = Math.round(width * ratio);
    element.height = Math.round(height * ratio);
    const ctx = element.getContext("2d");
    if (!ctx) return;
    ctx.scale(ratio, ratio);
    ctx.clearRect(0, 0, width, height);
    const row = height / session.channels;
    for (let channel = 0; channel < session.channels; channel++) {
      const middle = row * (channel + 0.5);
      ctx.strokeStyle = "#32434b";
      ctx.lineWidth = 1;
      ctx.beginPath();
      ctx.moveTo(0, middle);
      ctx.lineTo(width, middle);
      ctx.stroke();
      ctx.fillStyle = "#81949d";
      ctx.font = "11px monospace";
      ctx.fillText(
        session.channels === 1 ? "MONO" : channel === 0 ? "L" : "R",
        10,
        row * channel + 20,
      );
      if (!data) continue;
      ctx.save();
      ctx.beginPath();
      ctx.rect(0, row * channel, width, row);
      ctx.clip();
      for (const point of data.channels[channel]) {
        const x =
          ((point.start / session.sampleRate - view.start) /
            (view.end - view.start)) *
          width;
        const end =
          ((point.end / session.sampleRate - view.start) /
            (view.end - view.start)) *
          width;
        const gain = row * 0.42;
        ctx.fillStyle = "#6cd5bb";
        ctx.fillRect(
          x,
          middle - point.max * gain,
          Math.max(1, end - x),
          Math.max(1, (point.max - point.min) * gain),
        );
        ctx.fillStyle = "#b9f3d9";
        ctx.fillRect(
          x,
          middle - point.rms * gain,
          Math.max(1, end - x),
          Math.max(1, point.rms * 2 * gain),
        );
      }
      ctx.restore();
    }
  }, [session, view, data]);
  return (
    <canvas
      ref={canvas}
      className="waveform"
      role="img"
      aria-label={`${session.channels} 声道波形，${view.start.toFixed(3)} 至 ${view.end.toFixed(3)} 秒`}
    />
  );
}
