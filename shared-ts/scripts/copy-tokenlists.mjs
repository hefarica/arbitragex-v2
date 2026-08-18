/**
 * Build step — copy the curated tokenlist snapshots into dist/.
 *
 * The tokenlists module reads the JSONs via fs at boot (a 2 MB `resolveJsonModule`
 * import would explode tsc's literal-type inference), so tsc alone does not move
 * them. Consumers (api-server registry, selector-api safety gate) reach the files
 * through dist/tokenlists/data/ relative to the compiled module.
 */
import { cpSync, mkdirSync } from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const root = path.dirname(path.dirname(fileURLToPath(import.meta.url)));
const src = path.join(root, "src", "tokenlists", "data");
const dest = path.join(root, "dist", "tokenlists", "data");

mkdirSync(dest, { recursive: true });
cpSync(src, dest, { recursive: true });
console.log("tokenlists: copied snapshots to", dest);
