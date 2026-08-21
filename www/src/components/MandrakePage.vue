<template>
  <div class="page-layout">
    <section class="parameter-rail">
      <div class="page-heading">
        <div class="heading-icon" aria-hidden="true">M</div>
        <div>
          <h1>Mandrake</h1>
          <p>Stochastic cluster embedding</p>
        </div>
      </div>

      <p class="help-copy">
        Upload a genomic alignment or a Roary-style accessory table. Mandrake
        calculates sparse distances and embeds the samples entirely in this page.
      </p>

      <div class="control-stack">
        <div
          class="drop-zone"
          :class="{ 'drop-zone-active': isDragActive }"
          role="button"
          tabindex="0"
          :aria-disabled="isRunning"
          @click="openFilePicker"
          @keydown.enter.prevent="openFilePicker"
          @keydown.space.prevent="openFilePicker"
          @dragenter.prevent="isDragActive = true"
          @dragover.prevent="isDragActive = true"
          @dragleave.prevent="isDragActive = false"
          @drop.prevent="onDrop"
        >
          <input
            ref="fileInput"
            class="hidden-file-input"
            type="file"
            accept=".fa,.fasta,.fas,.fna,.fq,.fnq,.fastq,.rtab,.tsv,.fa.gz,.fasta.gz,.fas.gz,.fna.gz,.fq.gz,.fnq.gz,.fastq.gz,.rtab.gz,.tsv.gz"
            :disabled="isRunning"
            @click.stop
            @change="onFileChange"
          >
          <span class="drop-zone-icon" aria-hidden="true">↑</span>
          <strong v-if="isDragActive">Drop input here</strong>
          <strong v-else>Drop or click to upload an input</strong>
          <span class="drop-zone-help">FASTA/FASTQ or Roary Rtab/TSV, plain or gzip-compressed</span>
        </div>
        <p v-if="selectedFile && source" class="selected-file">
          <span>{{ selectedFile.name }}</span>
          <span class="detected-type">{{ sourceLabel }}</span>
        </p>
        <p v-if="inputError" class="input-error" role="alert">{{ inputError }}</p>

        <label class="control-label" for="labels-file">
          <span>Optional sample labels</span>
          <ParameterTooltip text="Unheadered sample-name<TAB>label rows. Every input sample must appear exactly once." />
        </label>
        <input
          id="labels-file"
          ref="labelsInput"
          class="text-input"
          type="file"
          accept=".tsv,.txt"
          :disabled="isRunning"
          @change="onLabelsChange"
        >
        <p v-if="selectedLabelsFile" class="selected-file">
          <span>{{ selectedLabelsFile.name }}</span>
          <span class="detected-type">Labels</span>
        </p>
        <p v-if="labelError" class="input-error" role="alert">{{ labelError }}</p>

        <div class="section-rule" />

        <label class="control-label" for="sparsification">
          <span>Sparsification</span>
          <ParameterTooltip text="Choose k-nearest-neighbour or strict normalized distance threshold sparsification." />
        </label>
        <select id="sparsification" v-model="mode" class="text-input" :disabled="isRunning">
          <option value="knn">k-nearest neighbours</option>
          <option value="threshold">Distance threshold</option>
        </select>

        <label class="control-label" for="sparsification-value">
          <span>{{ mode === "knn" ? "Neighbours per sample" : "Distance threshold" }}</span>
          <ParameterTooltip
            :text="mode === 'knn'
              ? 'Number of neighbours to retain per sample.'
              : 'Strict normalized distance threshold (in the range (0, 1]).'"
          />
        </label>
        <input
          id="sparsification-value"
          v-model.number="sparsificationValue"
          class="text-input"
          type="number"
          :min="mode === 'knn' ? 0 : 0.001"
          :max="mode === 'knn' ? 10000 : 1"
          :step="mode === 'knn' ? 1 : 0.01"
          :disabled="isRunning"
        >

        <label class="control-label" for="perplexity">
          <span>Perplexity</span>
          <ParameterTooltip text="Conditional-probability perplexity in the inclusive range 5.0..=100.0." />
        </label>
        <input id="perplexity" v-model.number="perplexity" class="text-input" type="number" min="5" max="100" step="1" :disabled="isRunning">

        <label class="control-label" for="max-updates">
          <span>Maximum updates</span>
          <ParameterTooltip text="Target number of stochastic update attempts." />
        </label>
        <input id="max-updates" v-model.number="maxUpdates" class="text-input" type="number" min="1" step="1000000" :disabled="isRunning">

        <label class="control-label" for="repulsion-samples">
          <span>Repulsion samples</span>
          <ParameterTooltip text="Repulsion samples per update attempt." />
        </label>
        <input id="repulsion-samples" v-model.number="repulsionSamples" class="text-input" type="number" min="1" step="1" :disabled="isRunning">

        <label class="control-label" for="learning-rate">
          <span>Learning rate</span>
          <ParameterTooltip text="Initial learning rate." />
        </label>
        <input id="learning-rate" v-model.number="learningRate" class="text-input" type="number" min="0.0001" step="0.1" :disabled="isRunning">

        <label class="check-row" for="initial-exaggeration">
          <input id="initial-exaggeration" v-model="initialExaggeration" type="checkbox" :disabled="isRunning">
          <span>Use initial exaggeration</span>
          <ParameterTooltip text="Apply initial attraction exaggeration." />
        </label>
      </div>

      <div class="action-row">
        <button class="primary-button" :disabled="!selectedFile || isRunning" @click="runEmbedding">
          {{ isRunning ? "Embedding…" : "Run Mandrake" }}
        </button>
        <button v-if="isRunning" class="secondary-button" @click="cancel">Cancel</button>
      </div>

      <p class="sidebar-note">
        The plot updates live as Mandrake computes. Zoom and pan with the
        Plotly controls, or provide an optional named TSV to colour samples.
      </p>
    </section>

    <section class="results-column">
      <div v-if="errorMessage" class="message message-error" role="alert">
        {{ errorMessage }}
      </div>

      <div v-if="isRunning || distanceProgress.maximum > 0" class="progress-card">
        <div class="progress-heading">
          <span>Distance phase</span>
          <span>{{ distancePercent }}%</span>
        </div>
        <div class="progress-track" aria-hidden="true">
          <div class="progress-fill" :style="{ width: `${distancePercent}%` }" />
        </div>
        <p class="progress-detail">
          {{ distanceProgress.completed.toLocaleString() }} / {{ distanceProgress.maximum.toLocaleString() }} rows
        </p>
      </div>

      <div v-if="isRunning || embeddingProgress.maximum > 0" class="progress-card">
        <div class="progress-heading">
          <span>{{ isRunning ? "Embedding phase" : "Embedding complete" }}</span>
          <span>{{ embeddingPercent }}%</span>
        </div>
        <div class="progress-track" aria-hidden="true">
          <div class="progress-fill" :style="{ width: `${embeddingPercent}%` }" />
        </div>
        <p class="progress-detail">
          {{ embeddingProgress.completed.toLocaleString() }} / {{ embeddingProgress.maximum.toLocaleString() }} updates
          <span v-if="Number.isFinite(embeddingProgress.eq)"> · Eq {{ embeddingProgress.eq.toFixed(4) }}</span>
        </p>
      </div>

      <div v-if="liveEmbedding" class="result-card">
        <div class="result-header">
          <div>
            <h2>{{ isRunning ? "Embedding in progress" : "Final embedding" }}</h2>
            <p>{{ sampleNames.length.toLocaleString() }} samples · two dimensions</p>
          </div>
          <div v-if="result" class="download-row">
            <button class="secondary-button" @click="downloadEmbedding">Download embedding</button>
            <button class="secondary-button" @click="downloadNames">Download names</button>
          </div>
        </div>
        <EmbeddingPlot
          :embedding="liveEmbedding"
          :names="sampleNames"
          :labels="labels ?? undefined"
          :run-key="runKey"
        />
      </div>

      <div v-else-if="!isRunning && !errorMessage" class="empty-state">
        <div class="empty-icon" aria-hidden="true">M</div>
        <h2>Your embedding will appear here</h2>
        <p>Choose an input file and parameters, then run Mandrake.</p>
      </div>
    </section>
  </div>
</template>

<script setup lang="ts">
import { computed, ref, watch } from "vue";
import EmbeddingPlot from "./EmbeddingPlot.vue";
import ParameterTooltip from "./ParameterTooltip.vue";
import {
  MandrakeRunner,
  type MandrakeDistanceProgress,
  type MandrakeProgress,
  type MandrakeResult,
  type MandrakeSettings,
  type MandrakeUpdate,
} from "../workers/Mandrake";

type InputSource = "alignment" | "accessory";

const fileInput = ref<HTMLInputElement | null>(null);
const labelsInput = ref<HTMLInputElement | null>(null);
const source = ref<InputSource | null>(null);
const selectedFile = ref<File | null>(null);
const selectedLabelsFile = ref<File | null>(null);
const isDragActive = ref(false);
const inputError = ref("");
const labelError = ref("");
const mode = ref<"knn" | "threshold">("knn");
const sparsificationValue = ref(15);
const perplexity = ref(30);
const maxUpdates = ref(1_000_000);
const repulsionSamples = ref(5);
const learningRate = ref(1);
const initialExaggeration = ref(false);
const isRunning = ref(false);
const errorMessage = ref("");
const result = ref<MandrakeResult | null>(null);
const liveEmbedding = ref<Float64Array | null>(null);
const sampleNames = ref<string[]>([]);
const labels = ref<string[] | null>(null);
const labelContents = ref<string | null>(null);
const distanceProgress = ref<MandrakeDistanceProgress>({ completed: 0, maximum: 0, complete: false });
const embeddingProgress = ref<MandrakeProgress>({ completed: 0, maximum: 0, eq: Number.NaN, complete: false });
const runKey = ref(0);
const runner = new MandrakeRunner();

const sourceLabel = computed(() => source.value === "alignment" ? "Alignment" : "Accessory table");

const distancePercent = computed(() => {
  if (!distanceProgress.value.maximum) return 0;
  return Math.min(100, Math.round((distanceProgress.value.completed / distanceProgress.value.maximum) * 100));
});

const embeddingPercent = computed(() => {
  if (!embeddingProgress.value.maximum) return 0;
  return Math.min(100, Math.round((embeddingProgress.value.completed / embeddingProgress.value.maximum) * 100));
});

watch(mode, (nextMode) => {
  sparsificationValue.value = nextMode === "knn" ? 15 : 0.5;
});

function detectSource(filename: string): InputSource | null {
  const lowerFilename = filename.toLowerCase();
  const sourceFilename = lowerFilename.endsWith(".gz")
    ? lowerFilename.slice(0, -3)
    : lowerFilename;
  const suffix = sourceFilename.slice(sourceFilename.lastIndexOf("."));
  if ([".fa", ".fasta", ".fas", ".fna", ".fq", ".fnq", ".fastq"].includes(suffix)) {
    return "alignment";
  }
  if ([".rtab", ".tsv"].includes(suffix)) {
    return "accessory";
  }
  return null;
}

function chooseFile(file: File | undefined): void {
  if (!file) return;
  const detectedSource = detectSource(file.name);
  selectedFile.value = null;
  source.value = null;
  result.value = null;
  liveEmbedding.value = null;
  sampleNames.value = [];
  labels.value = null;
  labelContents.value = null;
  distanceProgress.value = { completed: 0, maximum: 0, complete: false };
  embeddingProgress.value = { completed: 0, maximum: 0, eq: Number.NaN, complete: false };
  errorMessage.value = "";
  inputError.value = "";
  labelError.value = "";
  if (!detectedSource) {
    inputError.value = "Unsupported input suffix. Use FASTA/FASTQ (.fa, .fasta, .fas, .fna, .fq, .fnq, .fastq) or Rtab/TSV (.rtab, .tsv), optionally followed by .gz.";
    return;
  }
  selectedFile.value = file;
  source.value = detectedSource;
}

function openFilePicker(): void {
  if (!isRunning.value) fileInput.value?.click();
}

function onDrop(event: DragEvent): void {
  isDragActive.value = false;
  if (!isRunning.value) chooseFile(event.dataTransfer?.files[0]);
}

function onFileChange(event: Event): void {
  const input = event.target as HTMLInputElement;
  chooseFile(input.files?.[0]);
  input.value = "";
}

function onLabelsChange(event: Event): void {
  const input = event.target as HTMLInputElement;
  selectedLabelsFile.value = input.files?.[0] ?? null;
  labels.value = null;
  labelContents.value = null;
  labelError.value = "";
  input.value = "";
}

function parseLabels(contents: string, names: string[]): string[] {
  const lines = contents.replace(/\r\n/g, "\n").split("\n");
  if (lines.at(-1) === "") lines.pop();
  const labelsByName = new Map<string, string>();
  lines.forEach((line, index) => {
    const fields = line.split("\t");
    if (fields.length !== 2) {
      throw new Error(`label file row ${index + 1} must contain exactly two tab-separated fields`);
    }
    const [name, label] = fields;
    if (!name) throw new Error(`label file row ${index + 1} has an empty sample name`);
    if (labelsByName.has(name)) throw new Error(`label file contains duplicate sample name: ${name}`);
    labelsByName.set(name, label);
  });

  const nameSet = new Set(names);
  const missing = names.filter((name) => !labelsByName.has(name));
  const extra = Array.from(labelsByName.keys()).filter((name) => !nameSet.has(name));
  if (missing.length || extra.length) {
    const details = [];
    if (missing.length) details.push(`missing names: ${missing.join(", ")}`);
    if (extra.length) details.push(`unknown names: ${extra.join(", ")}`);
    throw new Error(`label/name mismatch (${details.join("; ")})`);
  }
  return names.map((name) => labelsByName.get(name)!);
}

function handleUpdate(update: MandrakeUpdate): void {
  if (update.phase === "distance") {
    distanceProgress.value = update.progress;
    if (update.names?.length) {
      sampleNames.value = update.names;
      if (labelContents.value !== null && labels.value === null) {
        try {
          labels.value = parseLabels(labelContents.value, update.names);
        } catch (error) {
          labelError.value = error instanceof Error ? error.message : String(error);
          errorMessage.value = labelError.value;
          runner.cancel();
        }
      }
    }
    return;
  }
  if (update.phase === "embedding") {
    embeddingProgress.value = update.progress;
    return;
  }
  liveEmbedding.value = update.embedding;
}

function settings(): MandrakeSettings {
  return {
    mode: mode.value,
    value: Number(sparsificationValue.value),
    perplexity: Number(perplexity.value),
    maxUpdates: Number(maxUpdates.value),
    repulsionSamples: Number(repulsionSamples.value),
    learningRate: Number(learningRate.value),
    initialExaggeration: initialExaggeration.value,
  };
}

async function runEmbedding(): Promise<void> {
  if (!selectedFile.value || !source.value) return;
  isRunning.value = true;
  errorMessage.value = "";
  labelError.value = "";
  result.value = null;
  liveEmbedding.value = null;
  sampleNames.value = [];
  labels.value = null;
  labelContents.value = null;
  distanceProgress.value = { completed: 0, maximum: 0, complete: false };
  embeddingProgress.value = { completed: 0, maximum: Number(maxUpdates.value), eq: Number.NaN, complete: false };
  runKey.value += 1;
  try {
    labelContents.value = selectedLabelsFile.value
      ? await selectedLabelsFile.value.text()
      : null;
    result.value = await runner.run(source.value, selectedFile.value, settings(), handleUpdate);
    sampleNames.value = result.value.names;
    liveEmbedding.value = result.value.embedding;
  } catch (error) {
    if (error instanceof Error && error.message !== "Mandrake operation cancelled") {
      errorMessage.value = error.message;
    }
  } finally {
    isRunning.value = false;
  }
}

function cancel(): void {
  runner.cancel();
  isRunning.value = false;
}

function downloadText(filename: string, contents: string): void {
  const blob = new Blob([contents], { type: "text/plain;charset=utf-8" });
  const url = URL.createObjectURL(blob);
  const anchor = document.createElement("a");
  anchor.href = url;
  anchor.download = filename;
  document.body.appendChild(anchor);
  anchor.click();
  anchor.remove();
  URL.revokeObjectURL(url);
}

function outputPrefix(): string {
  const filename = selectedFile.value?.name ?? "mandrake";
  return filename.replace(/\.[^.]+$/, "");
}

function downloadEmbedding(): void {
  if (!result.value) return;
  const rows: string[] = [];
  for (let index = 0; index < result.value.embedding.length; index += 2) {
    rows.push(`${result.value.embedding[index].toExponential(17)}\t${result.value.embedding[index + 1].toExponential(17)}`);
  }
  downloadText(`${outputPrefix()}.embedding.txt`, `${rows.join("\n")}\n`);
}

function downloadNames(): void {
  if (!result.value) return;
  downloadText(`${outputPrefix()}.names.txt`, `${result.value.names.join("\n")}\n`);
}
</script>
