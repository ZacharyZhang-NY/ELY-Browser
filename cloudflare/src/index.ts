import type { Env } from "./bindings.js";

export default {
  fetch(_request: Request, _env: Env): Response {
    return new Response("Not Found", { status: 404 });
  },
};
