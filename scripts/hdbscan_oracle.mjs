#!/usr/bin/env node

import assert from "node:assert/strict";
import { createRequire } from "node:module";

const packageDirectory = process.argv[2];
if (!packageDirectory) {
  console.error("usage: hdbscan_oracle.mjs <wasm-pack-node-package>");
  process.exit(2);
}

const require = createRequire(import.meta.url);
const { clusterEmbedding } = require(packageDirectory);
const fixture = new Float64Array([
  1.5, 2.2, 1.0, 1.1, 1.2, 1.4, 0.8, 1.0, 1.1, 1.0,
  3.7, 4.0, 3.9, 3.9, 3.6, 4.1, 3.8, 3.9, 4.0, 4.1,
  10.0, 10.0,
]);
const expected = [0, 0, 0, 0, 0, 1, 1, 1, 1, 1, -1];

const first = Array.from(clusterEmbedding(fixture));
const second = Array.from(clusterEmbedding(fixture));
assert.deepEqual(first, expected);
assert.deepEqual(second, expected);
console.log(`HDBSCAN wasm oracle passed (${first.length} labels)`);
