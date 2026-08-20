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
            accept=".fa,.fasta,.fas,.fna,.fq,.fnq,.fastq,.rtab,.tsv"
            :disabled="isRunning"
            @click.stop
            @change="onFileChange"
          >
          <span class="drop-zone-icon" aria-hidden="true">↑</span>
          <strong v-if="isDragActive">Drop input here</strong>
          <strong v-else>Drop or click to upload an input</strong>
          <span class="drop-zone-help">FASTA/FASTQ alignment or Roary Rtab/TSV</span>
        </div>
        <p v-if="selectedFile && source" class="selected-file">
          <span>{{ selectedFile.name }}</span>
          <span class="detected-type">{{ sourceLabel }}</span>
        </p>
        <p v-if="inputError" class="input-error" role="alert">{{ inputError }}</p>

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
        <input id="max-updates" v-model.number="maxUpdates" class="text-input" type="number" min="1" step="100" :disabled="isRunning">

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
        Sketch databases, intermediate frames, labels, and HDBSCAN are planned
        for a later browser phase.
      </p>
    </section>

    <section class="results-column">
      <div v-if="errorMessage" class="message message-error" role="alert">
        {{ errorMessage }}
      </div>

      <div v-if="isRunning || progress.maximum > 0" class="progress-card">
        <div class="progress-heading">
          <span>{{ isRunning ? "Calculating embedding" : "Embedding complete" }}</span>
          <span>{{ progressPercent }}%</span>
        </div>
        <div class="progress-track" aria-hidden="true">
          <div class="progress-fill" :style="{ width: `${progressPercent}%` }" />
        </div>
        <p class="progress-detail">
          {{ progress.completed.toLocaleString() }} / {{ progress.maximum.toLocaleString() }} updates
          <span v-if="Number.isFinite(progress.eq)"> · Eq {{ progress.eq.toFixed(4) }}</span>
        </p>
      </div>

      <div v-if="result" class="result-card">
        <div class="result-header">
          <div>
            <h2>Final embedding</h2>
            <p>{{ result.names.length.toLocaleString() }} samples · two dimensions</p>
          </div>
          <div class="download-row">
            <button class="secondary-button" @click="downloadEmbedding">Download embedding</button>
            <button class="secondary-button" @click="downloadNames">Download names</button>
          </div>
        </div>
        <EmbeddingPlot :embedding="result.embedding" :names="result.names" />
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
import { MandrakeRunner, type MandrakeProgress, type MandrakeResult, type MandrakeSettings } from "../workers/Mandrake";

type InputSource = "alignment" | "accessory";

const fileInput = ref<HTMLInputElement | null>(null);
const source = ref<InputSource | null>(null);
const selectedFile = ref<File | null>(null);
const isDragActive = ref(false);
const inputError = ref("");
const mode = ref<"knn" | "threshold">("knn");
const sparsificationValue = ref(15);
const perplexity = ref(30);
const maxUpdates = ref(10_000);
const repulsionSamples = ref(5);
const learningRate = ref(1);
const initialExaggeration = ref(false);
const isRunning = ref(false);
const errorMessage = ref("");
const result = ref<MandrakeResult | null>(null);
const progress = ref<MandrakeProgress>({ completed: 0, maximum: 0, eq: Number.NaN, complete: false });
const runner = new MandrakeRunner();

const sourceLabel = computed(() => source.value === "alignment" ? "Alignment" : "Accessory table");

const progressPercent = computed(() => {
  if (!progress.value.maximum) return 0;
  return Math.min(100, Math.round((progress.value.completed / progress.value.maximum) * 100));
});

watch(mode, (nextMode) => {
  sparsificationValue.value = nextMode === "knn" ? 15 : 0.5;
});

function detectSource(filename: string): InputSource | null {
  const suffix = filename.slice(filename.lastIndexOf(".")).toLowerCase();
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
  progress.value = { completed: 0, maximum: 0, eq: Number.NaN, complete: false };
  errorMessage.value = "";
  inputError.value = "";
  if (!detectedSource) {
    inputError.value = "Unsupported input suffix. Use FASTA/FASTQ (.fa, .fasta, .fas, .fna, .fq, .fnq, .fastq) or Rtab/TSV (.rtab, .tsv).";
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
  result.value = null;
  progress.value = { completed: 0, maximum: Number(maxUpdates.value), eq: Number.NaN, complete: false };
  try {
    const bytes = new Uint8Array(await selectedFile.value.arrayBuffer());
    result.value = await runner.run(source.value, bytes, settings(), (next) => {
      progress.value = next;
    });
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
