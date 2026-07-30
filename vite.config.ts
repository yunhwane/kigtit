import { defineConfig } from "vite";
import react from "@vitejs/plugin-react";

export default defineConfig({
  plugins: [react()],
  // Tauri가 이 포트를 열어 붙는다.
  server: { port: 1420, strictPort: true },
  build: { outDir: "dist", target: "safari15" },
  clearScreen: false,
});
