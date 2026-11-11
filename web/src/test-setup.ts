import "@testing-library/jest-dom/vitest"
import { cleanup } from "@testing-library/react"
import { afterEach } from "vitest"

// Vitest's globals are off, so Testing Library can't unmount by itself.
afterEach(cleanup)
