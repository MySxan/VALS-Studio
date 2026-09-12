// Development-only static projection of the real Rust project workflow.
import fixture from "./test-fixtures/session.json";
import { workspaceSchema, validateWaveform, type Backend } from "./model";
const workspace = workspaceSchema.parse(fixture.workspace);
const unavailable = async (): Promise<never> => {
  throw new Error("开发预览不修改工程");
};
export const previewBackend: Backend = {
  currentProject: async () => workspace,
  newProject: unavailable,
  openProject: unavailable,
  closeProject: unavailable,
  saveProject: unavailable,
  chooseFile: async () => null,
  chooseSavePath: async () => null,
  confirmDiscard: async () => false,
  startImport: unavailable,
  analyzeTrack: unavailable,
  relinkTrack: unavailable,
  jobStatus: unavailable,
  cancelJob: unavailable,
  waveform: async (ref, track, view) =>
    validateWaveform(fixture.waveform, ref, track, view),
};
