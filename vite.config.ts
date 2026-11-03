import { defineConfig } from "vite";
import vue from "@vitejs/plugin-vue";

// Tauri 开发时前端由 vite 提供（http://localhost:1420），
// 打包时构建到 ../dist，再由 src-tauri 嵌入二进制。
export default defineConfig({
  plugins: [vue()],
  clearScreen: false,
  server: {
    port: 1420,
    strictPort: true,
  },
  build: {
    target: "safari15",
    outDir: "dist",
    emptyOutDir: true,
  },
});
