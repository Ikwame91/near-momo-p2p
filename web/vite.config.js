import { defineConfig } from "vite";

export default defineConfig({
  base: "./", // relative asset paths, so the build works from any folder (e.g. GitHub Pages)
  define: {
    // The NEAR wallet libraries reference Node's `global`; in a browser that is `globalThis`.
    global: "globalThis",
  },
});
