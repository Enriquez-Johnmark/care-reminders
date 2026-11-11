import { defineConfig } from "@playwright/test"

// Screenshots and the walkthrough video of the made-up clinic, written to
// docs/showcase/. Not tests: run them on purpose with scripts/showcase.sh,
// which starts a fresh server; see docs/showcase/README.md.
export default defineConfig({
  testDir: ".",
  testMatch: "*.showcase.ts",
  fullyParallel: false,
  workers: 1,
  retries: 0,
  timeout: 6 * 60_000,
  reporter: "list",
  use: {
    baseURL: process.env.E2E_BASE_URL ?? "http://127.0.0.1:3004",
    launchOptions: { executablePath: process.env.CHROMIUM_PATH },
  },
})
