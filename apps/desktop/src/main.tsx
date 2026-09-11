import { createRoot } from "react-dom/client";
import { App } from "./App";
import { backend, desktopAvailable } from "./backend";
import { createController } from "./controller";
import "./style.css";

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
  window.addEventListener("pagehide", () => controller.dispose(), {
    once: true,
  });
}
void boot();
