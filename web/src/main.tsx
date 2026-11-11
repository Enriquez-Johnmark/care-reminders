import { StrictMode } from "react"
import { createRoot } from "react-dom/client"

import "@/styles/globals.css"
import { App } from "@/app/app"

const root = document.getElementById("root")
if (!root) throw new Error("index.html has no #root element")

createRoot(root).render(
  <StrictMode>
    <App />
  </StrictMode>
)
