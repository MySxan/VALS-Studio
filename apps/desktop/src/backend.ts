import { invoke, isTauri } from "@tauri-apps/api/core";
import { open, save, confirm } from "@tauri-apps/plugin-dialog";
import {
  workspaceSchema,
  jobSchema,
  validateWaveform,
  type Backend,
} from "./model";
export const desktopAvailable = isTauri();
export const backend: Backend = {
  currentProject: async () =>
    workspaceSchema.parse(await invoke("current_project")),
  newProject: async (name, generation, discard) =>
    workspaceSchema.parse(
      await invoke("new_project", { name, generation, discard }),
    ),
  openProject: async (path, generation, discard) =>
    workspaceSchema.parse(
      await invoke("open_project", { path, generation, discard }),
    ),
  closeProject: async (generation, discard) =>
    workspaceSchema.parse(
      await invoke("close_project", { generation, discard }),
    ),
  saveProject: async (ref, path) =>
    workspaceSchema.parse(await invoke("save_project", { ...ref, path })),
  chooseFile: async (kind) => {
    const result = await open({
      multiple: false,
      directory: false,
      filters: [
        {
          name: kind === "audio" ? "WAV audio" : "VALS project",
          extensions: [kind === "audio" ? "wav" : "vocalproj"],
        },
      ],
    });
    return typeof result === "string" ? result : null;
  },
  chooseSavePath: (name) =>
    save({
      defaultPath: `${name.replace(/[<>:"/\\|?*]/g, "_") || "Untitled"}.vocalproj`,
      filters: [{ name: "VALS project", extensions: ["vocalproj"] }],
    }),
  confirmDiscard: () =>
    confirm("当前工程有未保存的修改。放弃修改并继续？", {
      title: "未保存的工程",
      kind: "warning",
      okLabel: "放弃修改",
      cancelLabel: "返回",
    }),
  startImport: (ref, path) => invoke("start_import", { ...ref, path }),
  analyzeTrack: (ref, trackId) => invoke("analyze_track", { ...ref, trackId }),
  relinkTrack: (ref, trackId, path) =>
    invoke("relink_track", { ...ref, trackId, path }),
  jobStatus: async (id) => jobSchema.parse(await invoke("job_status", { id })),
  cancelJob: (id) => invoke("cancel_job", { id }),
  waveform: async (ref, track, view) =>
    validateWaveform(
      await invoke("waveform_slice", { ...ref, trackId: track.id, ...view }),
      ref,
      track,
      view,
    ),
};
