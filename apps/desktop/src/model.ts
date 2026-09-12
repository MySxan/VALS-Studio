import { z } from "zod";
const hash = z.string().regex(/^[a-f0-9]{64}$/),
  uuid = z.string().uuid();
const finite = z.number().finite(),
  integer = z.number().int().safe();
const analysisSchema = z.object({
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
});
export const trackSchema = z
  .object({
    id: uuid,
    name: z.string(),
    sourceId: uuid,
    sourcePath: z.string(),
    sourceHash: hash,
    sampleRate: integer.positive(),
    channels: integer.positive(),
    duration: finite.positive(),
    status: z.enum([
      "unchecked",
      "analyzing",
      "ready",
      "offline",
      "changed",
      "error",
    ]),
    error: z.string().nullable(),
    analysis: analysisSchema.nullable(),
  })
  .refine(
    (t) =>
      (t.status === "ready") === (t.analysis !== null) &&
      (!t.analysis || t.analysis.provenance.sourceHash === t.sourceHash),
  );
export const jobSchema = z.object({
  id: uuid,
  projectId: uuid,
  generation: integer.nonnegative(),
  trackId: uuid.nullable(),
  phase: z.enum(["running", "cancelling", "succeeded", "cancelled", "failed"]),
  error: z.string().nullable(),
});
export const workspaceSchema = z.object({
  generation: integer.nonnegative(),
  project: z
    .object({
      id: uuid,
      name: z.string(),
      path: z.string().nullable(),
      dirty: z.boolean(),
      tracks: z.array(trackSchema),
    })
    .nullable(),
  job: jobSchema.nullable(),
});
export const waveformSchema = z.object({
  projectId: uuid,
  generation: integer.nonnegative(),
  trackId: uuid,
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
export type Track = z.infer<typeof trackSchema>;
export type Workspace = z.infer<typeof workspaceSchema>;
export type Project = NonNullable<Workspace["project"]>;
export type Job = z.infer<typeof jobSchema>;
export type Waveform = z.infer<typeof waveformSchema>;
export interface ProjectRef {
  projectId: string;
  generation: number;
}
export interface Viewport {
  start: number;
  end: number;
  width: number;
}
export interface Backend {
  currentProject(): Promise<Workspace>;
  newProject(
    name: string,
    generation: number,
    discard: boolean,
  ): Promise<Workspace>;
  openProject(
    path: string,
    generation: number,
    discard: boolean,
  ): Promise<Workspace>;
  closeProject(generation: number, discard: boolean): Promise<Workspace>;
  saveProject(ref: ProjectRef, path: string | null): Promise<Workspace>;
  chooseFile(kind: "audio" | "project"): Promise<string | null>;
  chooseSavePath(name: string): Promise<string | null>;
  confirmDiscard(): Promise<boolean>;
  startImport(ref: ProjectRef, path: string): Promise<string>;
  analyzeTrack(ref: ProjectRef, trackId: string): Promise<string>;
  relinkTrack(ref: ProjectRef, trackId: string, path: string): Promise<string>;
  jobStatus(id: string): Promise<Job>;
  cancelJob(id: string): Promise<void>;
  waveform(ref: ProjectRef, track: Track, view: Viewport): Promise<Waveform>;
}
export function validateWaveform(
  raw: unknown,
  ref: ProjectRef,
  track: Track,
  view: Viewport,
): Waveform {
  const wave = waveformSchema.parse(raw);
  if (
    wave.projectId !== ref.projectId ||
    wave.generation !== ref.generation ||
    wave.trackId !== track.id ||
    wave.artifactHash !== track.analysis?.artifactHash ||
    wave.channels.length !== track.channels ||
    wave.channels.some((c) => c.length > view.width + 1)
  )
    throw new Error("波形响应与工程、轨道或视口不匹配");
  return wave;
}
