import type { ErrorBody } from "@/api/generated/ErrorBody"

/** An answer from the API other than success, in the API's own words. */
export class ApiError extends Error {
  readonly status: number
  readonly problems: string[]

  constructor(status: number, body: ErrorBody) {
    super(body.message)
    this.name = "ApiError"
    this.status = status
    this.problems = body.problems
  }

  /** The session has ended; they must log in again. */
  get isNotLoggedIn() {
    return this.status === 401
  }
}

type Method = "GET" | "POST" | "PUT" | "DELETE"

/**
 * Calls the Rust API. Every request in the app goes through here: it sends
 * the session cookie and JSON (or a file form as is), and turns any failure
 * into an ApiError.
 */
export async function api<T>(method: Method, path: string, body?: unknown): Promise<T> {
  const asJson = body !== undefined && !(body instanceof FormData)
  const response = await fetch(path, {
    method,
    credentials: "same-origin",
    headers: asJson ? { "Content-Type": "application/json" } : undefined,
    body: asJson ? JSON.stringify(body) : (body as FormData | undefined),
  })
  if (!response.ok) throw new ApiError(response.status, await errorBody(response))
  // 201, 202 and 204 answers may have no body at all.
  const text = await response.text()
  return (text === "" ? undefined : JSON.parse(text)) as T
}

/** Asks the Rust API for a file, such as a PDF: the file, or an ApiError
 *  in the API's words when it refuses. */
export async function apiFile(method: Method, path: string): Promise<Blob> {
  const response = await fetch(path, { method, credentials: "same-origin" })
  if (!response.ok) throw new ApiError(response.status, await errorBody(response))
  return response.blob()
}

async function errorBody(response: Response): Promise<ErrorBody> {
  try {
    const body = (await response.json()) as Partial<ErrorBody>
    if (typeof body.message === "string") {
      return { message: body.message, problems: body.problems ?? [] }
    }
  } catch {
    // Not JSON: a proxy or the network answered, not the API.
  }
  return {
    message: "The app couldn't be reached. Check your connection and try again.",
    problems: [],
  }
}
