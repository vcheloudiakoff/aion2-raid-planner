import { defineConfig } from "vite";
import react from "@vitejs/plugin-react";

// Le build sort dans web/dist/, embarqué par le binaire (AD-1). `public/.gitkeep` y est recopié à
// chaque build pour que le dossier reste versionné malgré le vidage d'outDir.
export default defineConfig({
  plugins: [react()],
  build: {
    outDir: "dist",
    emptyOutDir: true,
    assetsDir: "assets",
  },
});
