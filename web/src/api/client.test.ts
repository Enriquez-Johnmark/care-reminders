import { afterEach, describe, expect, it, vi } from "vitest"

import { api, ApiError, apiFile } from "@/api/client"

function answer(status: number, body: unknown) {
  vi.stubGlobal("fetch", vi.fn().mockResolvedValue(new Response(JSON.stringify(body), { status })))
}

afterEach(() => vi.unstubAllGlobals())

describe("api", () => {
  it("returns the API's JSON", async () => {
    answer(200, { firm_name: "Santos Bookkeeping" })
    await expect(api("GET", "/api/me")).resolves.toEqual({ firm_name: "Santos Bookkeeping" })
  })

  it("sends a body as JSON from the same origin", async () => {
    answer(200, {})
    await api("POST", "/api/login", { email: "ana@santos.test" })
    expect(fetch).toHaveBeenCalledWith("/api/login", {
      method: "POST",
      credentials: "same-origin",
      headers: { "Content-Type": "application/json" },
      body: '{"email":"ana@santos.test"}',
    })
  })

  it("sends a file form as is, letting the browser set its type", async () => {
    answer(200, {})
    const form = new FormData()
    await api("POST", "/api/import", form)
    expect(fetch).toHaveBeenCalledWith("/api/import", {
      method: "POST",
      credentials: "same-origin",
      headers: undefined,
      body: form,
    })
  })

  it("accepts a success with no body, such as 201 Created", async () => {
    vi.stubGlobal("fetch", vi.fn().mockResolvedValue(new Response(null, { status: 201 })))
    await expect(
      api("POST", "/api/clients/1/payors", { name: "Jollibee" })
    ).resolves.toBeUndefined()
  })

  it("turns a refusal into an ApiError with the API's words", async () => {
    answer(422, { message: "Not saved.", problems: ["Email is missing."] })
    const error = await api("POST", "/api/clients", {}).catch((e: unknown) => e)
    expect(error).toBeInstanceOf(ApiError)
    expect(error).toMatchObject({
      status: 422,
      message: "Not saved.",
      problems: ["Email is missing."],
    })
  })

  it("explains an answer that isn't from the API", async () => {
    vi.stubGlobal("fetch", vi.fn().mockResolvedValue(new Response("Bad gateway", { status: 502 })))
    const error = (await api("GET", "/api/me").catch((e: unknown) => e)) as ApiError
    expect(error.message).toMatch(/couldn't be reached/)
  })
})

describe("apiFile", () => {
  it("returns the file the API sends", async () => {
    vi.stubGlobal("fetch", vi.fn().mockResolvedValue(new Response("%PDF-1.3", { status: 200 })))
    const file = await apiFile("POST", "/api/files/1")
    await expect(file.text()).resolves.toBe("%PDF-1.3")
  })

  it("turns a refusal into an ApiError in the API's words, not a file", async () => {
    answer(409, { message: "That file isn't ready.", problems: [] })
    await expect(apiFile("POST", "/api/files/1")).rejects.toThrow("That file isn't ready.")
  })
})
