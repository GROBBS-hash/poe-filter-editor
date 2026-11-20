import { defineConfig, type Plugin } from "vite";
import react from "@vitejs/plugin-react";
import fs from "node:fs";
import path from "node:path";

/** Dev only: lets preview.html read the app's saved price checks and item pictures from its data folder. */
function previewData(): Plugin {
  const dir = path.join(process.env.APPDATA ?? "", "com.grobbs.poefiltereditor");
  return {
    name: "preview-data",
    apply: "serve",
    configureServer(server) {
      server.middlewares.use("/preview-data", (req, res) => {
        const url = decodeURIComponent((req.url ?? "/").split("?")[0]);
        const send = (body: unknown) => { res.setHeader("Content-Type", "application/json"); res.end(JSON.stringify(body)); };
        const samples = path.join(dir, "samples");
        if (url === "/samples") {
          // Newest first; ".result" files are the trade results that go with a check.
          const list = fs.existsSync(samples) ? fs.readdirSync(samples).filter((f) => f.endsWith(".json") && !f.endsWith(".result.json"))
            .map((f) => ({ f, t: fs.statSync(path.join(samples, f)).mtimeMs })).sort((a, b) => b.t - a.t).map((x) => x.f.replace(/\.json$/, "")) : [];
          return send(list);
        }
        const file = url === "/icons" ? path.join(dir, "icons_poe1.json")
          : url.startsWith("/samples/") ? path.join(samples, path.basename(url) + ".json") : "";
        if (!file || !fs.existsSync(file)) return send(null);
        res.setHeader("Content-Type", "application/json");
        res.end(fs.readFileSync(file));
      });
    },
  };
}

export default defineConfig({
  plugins: [react(), previewData()],
  clearScreen: false,
  server: {
    port: 1420,
    strictPort: true,
    // Don't watch the Rust build folder; Windows locks files in it mid-build.
    watch: { ignored: ["**/src-tauri/**", "**/target/**", "**/core/**"] },
  },
});
