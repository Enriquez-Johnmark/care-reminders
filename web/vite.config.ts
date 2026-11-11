import { resolve } from "node:path"
import tailwindcss from "@tailwindcss/vite"
import react from "@vitejs/plugin-react"
import { defineConfig } from "vite"

// In development the app runs on Vite's server and passes these paths to
// the Rust server (`cargo run -- serve`). Set PUBLIC_BASE_URL in .env to
// this server's address (http://localhost:5173) so login links open here.
const rust = "http://127.0.0.1:3000"

export default defineConfig({
  plugins: [react(), tailwindcss()],
  resolve: {
    alias: {
      "@": resolve(import.meta.dirname, "./src"),
    },
  },
  server: {
    proxy: {
      "/api": rust,
      "/hooks": rust,
      "/health": rust,
    },
  },
  build: {
    // No source maps in production: the app's code stays readable only to us.
    sourcemap: false,
    // Fonts and images stay files: the Content-Security-Policy allows no
    // data: addresses.
    assetsInlineLimit: 0,
  },
})
