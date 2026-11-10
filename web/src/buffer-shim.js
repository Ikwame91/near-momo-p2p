// The NEAR wallet libraries expect Node's global `Buffer`; browsers don't have one.
// This must be the FIRST import in main.js so it runs before those libraries load.
import { Buffer } from "buffer";

if (!globalThis.Buffer) globalThis.Buffer = Buffer;
