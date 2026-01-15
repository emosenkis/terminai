/**
 * Fetch polyfill that uses Rust ops
 * 
 * This provides a minimal fetch implementation that works in deno_core
 * by calling our op_fetch Rust operation.
 */

// Response class implementation
class FetchResponse implements Response {
  readonly status: number;
  readonly statusText: string;
  readonly ok: boolean;
  readonly headers: Headers;
  readonly url: string;
  readonly redirected: boolean;
  readonly type: ResponseType;
  private _body: string;
  private _bodyUsed: boolean = false;

  constructor(status: number, body: string, url: string = "") {
    this.status = status;
    this.statusText = status >= 200 && status < 300 ? "OK" : "Error";
    this.ok = status >= 200 && status < 300;
    this.headers = new Headers();
    this.headers.set("content-type", "application/json");
    this.url = url;
    this.redirected = false;
    this.type = "basic";
    this._body = body;
  }

  get body(): ReadableStream<Uint8Array> | null {
    return null; // Not implementing streaming body
  }

  get bodyUsed(): boolean {
    return this._bodyUsed;
  }

  async arrayBuffer(): Promise<ArrayBuffer> {
    this._bodyUsed = true;
    const encoder = new TextEncoder();
    return encoder.encode(this._body).buffer;
  }

  async blob(): Promise<Blob> {
    this._bodyUsed = true;
    return new Blob([this._body], { type: "application/json" });
  }

  async formData(): Promise<FormData> {
    throw new Error("formData() not implemented");
  }

  async json(): Promise<unknown> {
    this._bodyUsed = true;
    return JSON.parse(this._body);
  }

  async text(): Promise<string> {
    this._bodyUsed = true;
    return this._body;
  }

  clone(): Response {
    return new FetchResponse(this.status, this._body, this.url);
  }

  async bytes(): Promise<Uint8Array> {
    this._bodyUsed = true;
    const encoder = new TextEncoder();
    return encoder.encode(this._body);
  }
}

// Headers polyfill if not available
if (typeof globalThis.Headers === "undefined") {
  globalThis.Headers = class HeadersPolyfill {
    private _headers: Map<string, string> = new Map();

    constructor(init?: HeadersInit) {
      if (init) {
        if (init instanceof HeadersPolyfill) {
          init._headers.forEach((value, key) => {
            this._headers.set(key.toLowerCase(), value);
          });
        } else if (Array.isArray(init)) {
          for (const [key, value] of init) {
            this._headers.set(key.toLowerCase(), value);
          }
        } else if (typeof init === "object") {
          for (const [key, value] of Object.entries(init)) {
            this._headers.set(key.toLowerCase(), value);
          }
        }
      }
    }

    append(name: string, value: string): void {
      const existing = this._headers.get(name.toLowerCase());
      this._headers.set(name.toLowerCase(), existing ? `${existing}, ${value}` : value);
    }

    delete(name: string): void {
      this._headers.delete(name.toLowerCase());
    }

    get(name: string): string | null {
      return this._headers.get(name.toLowerCase()) ?? null;
    }

    has(name: string): boolean {
      return this._headers.has(name.toLowerCase());
    }

    set(name: string, value: string): void {
      this._headers.set(name.toLowerCase(), value);
    }

    forEach(callback: (value: string, key: string, parent: Headers) => void): void {
      this._headers.forEach((value, key) => {
        callback(value, key, this as unknown as Headers);
      });
    }

    entries(): IterableIterator<[string, string]> {
      return this._headers.entries();
    }

    keys(): IterableIterator<string> {
      return this._headers.keys();
    }

    values(): IterableIterator<string> {
      return this._headers.values();
    }

    [Symbol.iterator](): IterableIterator<[string, string]> {
      return this._headers.entries();
    }

    getSetCookie(): string[] {
      const cookie = this._headers.get("set-cookie");
      return cookie ? [cookie] : [];
    }
  } as unknown as typeof Headers;
}

// Our fetch implementation using Rust ops
async function fetchViaOps(input: RequestInfo | URL, init?: RequestInit): Promise<Response> {
  const url = typeof input === "string" ? input : input instanceof URL ? input.toString() : input.url;
  
  const method = init?.method ?? "GET";
  const headers: Record<string, string> = {};
  
  if (init?.headers) {
    if (init.headers instanceof Headers) {
      init.headers.forEach((value, key) => {
        headers[key] = value;
      });
    } else if (Array.isArray(init.headers)) {
      for (const [key, value] of init.headers) {
        headers[key] = value;
      }
    } else {
      Object.assign(headers, init.headers);
    }
  }
  
  const body = init?.body ? String(init.body) : undefined;
  
  try {
    // Call our Rust op_fetch
    const response = await globalThis.Deno.core.ops.op_fetch(url, {
      method,
      headers,
      body,
    });
    
    return new FetchResponse(response.status, response.body, url);
  } catch (error) {
    throw new TypeError(`Network request failed: ${(error as Error).message}`);
  }
}

// Install fetch globally if not present
if (typeof globalThis.fetch === "undefined") {
  (globalThis as unknown as { fetch: typeof fetch }).fetch = fetchViaOps as typeof fetch;
}

// Also install Response if not present
if (typeof globalThis.Response === "undefined") {
  (globalThis as unknown as { Response: typeof Response }).Response = FetchResponse as unknown as typeof Response;
}

// Install Request if not present
if (typeof globalThis.Request === "undefined") {
  (globalThis as unknown as { Request: typeof Request }).Request = class RequestPolyfill {
    readonly url: string;
    readonly method: string;
    readonly headers: Headers;
    readonly body: ReadableStream<Uint8Array> | null = null;
    readonly bodyUsed: boolean = false;
    readonly cache: RequestCache = "default";
    readonly credentials: RequestCredentials = "same-origin";
    readonly destination: RequestDestination = "";
    readonly integrity: string = "";
    readonly keepalive: boolean = false;
    readonly mode: RequestMode = "cors";
    readonly redirect: RequestRedirect = "follow";
    readonly referrer: string = "";
    readonly referrerPolicy: ReferrerPolicy = "";
    readonly signal: AbortSignal = new AbortController().signal;
    readonly attribute: unknown = undefined;
    readonly targetAddressSpace: unknown = undefined;

    constructor(input: RequestInfo | URL, init?: RequestInit) {
      this.url = typeof input === "string" ? input : input instanceof URL ? input.toString() : input.url;
      this.method = init?.method ?? "GET";
      this.headers = new Headers(init?.headers);
    }

    clone(): Request {
      return new RequestPolyfill(this.url, {
        method: this.method,
        headers: this.headers,
      }) as unknown as Request;
    }

    async arrayBuffer(): Promise<ArrayBuffer> {
      return new ArrayBuffer(0);
    }

    async blob(): Promise<Blob> {
      return new Blob();
    }

    async formData(): Promise<FormData> {
      throw new Error("formData() not implemented");
    }

    async json(): Promise<unknown> {
      return {};
    }

    async text(): Promise<string> {
      return "";
    }

    async bytes(): Promise<Uint8Array> {
      return new Uint8Array();
    }
  } as unknown as typeof Request;
}

export { fetchViaOps };
