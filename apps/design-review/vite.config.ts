import { defineConfig } from "vitest/config";
import react from "@vitejs/plugin-react";

export default defineConfig({
  plugins: [react()],
  server: {
    host: "0.0.0.0",
    port: 1430,
    strictPort: true,
  },
  preview: {
    host: "0.0.0.0",
  },
  test: {
    environment: "jsdom",
    include: ["src/**/*.test.{ts,tsx}"],
    setupFiles: "./src/test/setup.ts",
    restoreMocks: true,
    testTimeout: 10_000,
  },
});
