import assert from "node:assert/strict";
import fs from "node:fs";
import path from "node:path";
import { fileURLToPath, pathToFileURL } from "node:url";

const repositoryRoot = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..");
const packageArgument = process.argv[2] ?? path.join(repositoryRoot, "www/src/pkg");
const packagePath = path.resolve(packageArgument);
const packageEntry = packagePath.endsWith(".js")
  ? packagePath
  : fs.existsSync(path.join(packagePath, "index.js"))
    ? path.join(packagePath, "index.js")
    : path.join(packagePath, "mandrake.js");
const wasm = await import(pathToFileURL(packageEntry));
const metadata = fs.readFileSync(path.join(repositoryRoot, "tests/fixtures/sketches.skm"));
const data = fs.readFileSync(path.join(repositoryRoot, "tests/fixtures/sketches.skd"));

assert.deepEqual(Array.from(wasm.sketchKmerLengthsBytes(metadata)), [21]);

const operation = wasm.MandrakeOperation.fromSketch(
  metadata,
  data,
  "knn",
  2,
  30,
  8,
  5,
  1,
  false,
  "jaccard",
  21,
);
const distanceProgress = operation.advanceDistances(0);
assert.equal(distanceProgress.complete, true);
assert.equal(distanceProgress.completed, 616);
assert.equal(operation.names().split("\n").length, 616);

operation.beginEmbedding();
let progress;
do {
  progress = operation.advance(8);
} while (!progress.complete);
assert.equal(progress.complete, true);
assert.equal(operation.embedding().length, 616 * 2);
assert.ok(Array.from(operation.embedding()).every(Number.isFinite));
operation.free();

assert.throws(
  () => wasm.MandrakeOperation.fromSketch(
    metadata,
    data,
    "knn",
    2,
    30,
    8,
    5,
    1,
    false,
    "core",
    21,
  ),
  /at least two k-mer lengths/,
);

console.log("sketch wasm smoke passed");
