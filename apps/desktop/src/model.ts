import { z } from "zod";

const hash = z.string().regex(/^[a-f0-9]{64}$/);
const finite = z.number().finite();
const integer = z.number().int().safe();
export const sessionSchema = z
  .object({
    id: z.string().uuid(),
    sourceHash: hash,
    sampleRate: integer.positive(),
    channels: z.union([z.literal(1), z.literal(2)]),
    duration: finite.positive(),
    artifactHash: hash,
    confidence: z
      .object({
        kind: z.enum(["measurement", "modelProbability", "heuristic"]),
        score: finite.min(0).max(1).nullable(),
        explanation: z.string().min(1),
      })
      .refine((c) => c.kind !== "measurement" || c.score === null),
    provenance: z.object({
      analyzerId: z.string(),
      analyzerVersion: z.string(),
      provider: z.string(),
      model: z.object({ id: z.string(), version: z.string(), hash }).nullable(),
      settings: z.record(z.string(), z.string()),
      settingsHash: hash,
      sourceHash: hash,
      dependencyHashes: z.array(hash),
      createdAtUnixMs: integer.nonnegative().nullable(),
      runtime: z.string(),
    }),
  })
  .refine((s) => s.sourceHash === s.provenance.sourceHash);
export const jobSchema = z.object({
  id: z.string().uuid(),
  phase: z.enum(["running", "cancelling", "succeeded", "cancelled", "failed"]),
  error: z.string().nullable(),
});
export const waveformSchema = z.object({
  sessionId: z.string().uuid(),
  artifactHash: hash,
  framesPerBlock: integer.positive(),
  channels: z
    .array(
      z
        .array(
          z
            .object({
              start: integer.nonnegative(),
              end: integer.positive(),
              min: finite,
              max: finite,
              rms: finite.nonnegative(),
            })
            .refine((p) => p.end > p.start && p.min <= p.max),
        )
        .max(4097),
    )
    .min(1)
    .max(2),
});
export type Session = z.infer<typeof sessionSchema>;
export type Job = z.infer<typeof jobSchema>;
export type Waveform = z.infer<typeof waveformSchema>;
export interface Viewport {
  start: number;
  end: number;
  width: number;
}
export interface Backend {
  currentSession(): Promise<Session | null>;
  chooseFile(): Promise<string | null>;
  startImport(path: string): Promise<string>;
  jobStatus(id: string): Promise<Job>;
  cancelJob(id: string): Promise<void>;
  waveform(session: Session, viewport: Viewport): Promise<Waveform>;
}
export function validateWaveform(
  raw: unknown,
  session: Session,
  view: Viewport,
): Waveform {
  const wave = waveformSchema.parse(raw);
  if (
    wave.sessionId !== session.id ||
    wave.artifactHash !== session.artifactHash ||
    wave.channels.length !== session.channels ||
    wave.channels.some((c) => c.length > view.width + 1)
  ) {
    throw new Error("波形响应与当前会话或视口不匹配");
  }
  return wave;
}
