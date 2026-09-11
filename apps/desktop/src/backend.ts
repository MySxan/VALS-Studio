import { invoke, isTauri } from "@tauri-apps/api/core";
import { open } from "@tauri-apps/plugin-dialog";
import {
  jobSchema,
  sessionSchema,
  validateWaveform,
  type Backend,
} from "./model";

export const desktopAvailable = isTauri();
export const backend: Backend = {
  currentSession: async () =>
    sessionSchema.nullable().parse(await invoke("current_session")),
  chooseFile: async () => {
    const result = await open({
      multiple: false,
      directory: false,
      filters: [{ name: "WAV audio", extensions: ["wav"] }],
    });
    return typeof result === "string" ? result : null;
  },
  startImport: (path) => invoke<string>("start_import", { path }),
  jobStatus: async (id) => jobSchema.parse(await invoke("job_status", { id })),
  cancelJob: (id) => invoke("cancel_job", { id }),
  waveform: async (session, view) =>
    validateWaveform(
      await invoke("waveform_slice", { sessionId: session.id, ...view }),
      session,
      view,
    ),
};
