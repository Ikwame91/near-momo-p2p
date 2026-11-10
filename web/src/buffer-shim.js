// The NEAR wallet libraries expect Node's globals: `Buffer`, and `process.env` (read by
// @near-js/accounts at load time). Browsers have neither. The production build replaces
// `process.env`, but the dev server (`npm run dev`) does not, so both are provided here.
// This must be the FIRST import in main.js so it runs before those libraries load.
import { Buffer } from "buffer";

if (!globalThis.Buffer) globalThis.Buffer = Buffer;
if (!globalThis.process) globalThis.process = { env: {} };
