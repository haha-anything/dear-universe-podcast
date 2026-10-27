import { defineConfig } from "vite";
import { svelte } from "@sveltejs/vite-plugin-svelte";
import tailwindcss from "@tailwindcss/vite";

// Tauri 要求固定端口，端口被占用就直接失败，不要静默换端口
const host = process.env["TAURI_DEV_HOST"];

export default defineConfig({
  plugins: [tailwindcss(), svelte()],

  // Vite 不要清屏，否则 Rust 的编译错误会被刷掉
  clearScreen: false,

  server: {
    port: 1420,
    strictPort: true,
    host: host || "127.0.0.1",
    hmr: host
      ? {
          protocol: "ws",
          host,
          port: 1421,
        }
      : undefined,
    watch: {
      // 别监听 src-tauri，否则编译产物一变就触发前端热更
      ignored: ["**/src-tauri/**"],
    },
  },

  build: {
    // Tauri dev/build 都会读 sourcemap
    sourcemap: !!process.env["TAURI_ENV_DEBUG"],
    target: process.env["TAURI_ENV_PLATFORM"] == "windows" ? "chrome105" : "safari13",
    minify: !process.env["TAURI_ENV_DEBUG"] ? "esbuild" : false,
  },
});
