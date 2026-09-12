import { createRoot } from "react-dom/client";
import { App } from "./App";
import { backend, desktopAvailable } from "./backend";
import { createController } from "./controller";
import "./style.css";
import { getCurrentWindow } from "@tauri-apps/api/window";

async function boot() {
  const preview =
    import.meta.env.DEV &&
    new URLSearchParams(location.search).get("preview") === "1";
  const api = preview ? (await import("./preview")).previewBackend : backend;
  const controller = createController(api);
  createRoot(document.getElementById("root")!).render(
    <App
      controller={controller}
      available={desktopAvailable || preview}
      preview={preview}
    />,
  );
  if (desktopAvailable || preview) void controller.initialize();
  if (desktopAvailable) {
    await getCurrentWindow().onCloseRequested(async (event) => {
      event.preventDefault();
      // Application Close uses the same dirty/busy guard as the toolbar.
      if (await controller.closeProject()) await getCurrentWindow().destroy();
    });
  }
  window.addEventListener("pagehide", () => controller.dispose(), {
    once: true,
  });
}
void boot();
