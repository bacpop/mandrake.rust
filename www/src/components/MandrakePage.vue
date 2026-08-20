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
        <label class="control-label" for="source">Distance source</label>
        <select id="source" v-model="source" class="text-input" :disabled="isRunning">
          <option value="alignment">Alignment (FASTA/FASTQ)</option>
          <option value="accessory">Accessory table (Rtab/TSV)</option>
        </select>

        <label class="control-label" for="input-file">
          <span>Input file</span>
          <span class="info-dot" :title="sourceHelp">i</span>
        </label>
        <input
          id="input-file"
          class="file-input"
          type="file"
          :accept="source === 'alignment' ? '.fa,.fasta,.fas,.fq,.fastq,.txt' : '.Rtab,.rtab,.tsv,.txt'"
          :disabled="isRunning"
          @change="onFileChange"
        >
        <p v-if="selectedFile" class="selected-file">{{ selectedFile.name }}</p>

        <div class="section-rule" />

        <label class="control-label" for="sparsification">Sparsification</label>
        <select id="sparsification" v-model="mode" class="text-input" :disabled="isRunning">
          <option value="knn">k-nearest neighbours</option>
          <option value="threshold">Distance threshold</option>
        </select>

        <label class="control-label" for="sparsification-value">
          <span>{{ mode === "knn" ? "Neighbours per sample" : "Distance threshold" }}</span>
          <span class="info-dot" :title="mode === 'knn' ? 'Zero keeps every non-self neighbour.' : 'Edges at or above this normalised distance are discarded.'">i</span>
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
          <span class="info-dot" title="Target entropy for conditional probabilities.">i</span>
        </label>
        <input id="perplexity" v-model.number="perplexity" class="text-input" type="number" min="5" max="100" step="1" :disabled="isRunning">

        <label class="control-label" for="max-updates">
          <span>Maximum updates</span>
          <span class="info-dot" title="Total stochastic optimisation update attempts.">i</span>
        </label>
        <input id="max-updates" v-model.number="maxUpdates" class="text-input" type="number" min="1" step="100" :disabled="isRunning">

        <label class="control-label" for="repulsion-samples">
          <span>Repulsion samples</span>
          <span class="info-dot" title="Randomly sampled repulsion pairs per update.">i</span>
        </label>
        <input id="repulsion-samples" v-model.number="repulsionSamples" class="text-input" type="number" min="1" step="1" :disabled="isRunning">

        <label class="control-label" for="learning-rate">
          <span>Learning rate</span>
          <span class="info-dot" title="Initial optimisation learning rate.">i</span>
        </label>
        <input id="learning-rate" v-model.number="learningRate" class="text-input" type="number" min="0.0001" step="0.1" :disabled="isRunning">

        <label class="check-row" for="initial-exaggeration">
          <input id="initial-exaggeration" v-model="initialExaggeration" type="checkbox" :disabled="isRunning">
          <span>Use initial exaggeration</span>
          <span class="info-dot" title="Strengthen attraction during the first tenth of optimisation.">i</span>
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
import { MandrakeRunner, type MandrakeProgress, type MandrakeResult, type MandrakeSettings } from "../workers/Mandrake";

const source = ref<"alignment" | "accessory">("alignment");
const selectedFile = ref<File | null>(null);
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

const sourceHelp = computed(() => source.value === "alignment"
  ? "Plain FASTA or FASTQ records with equal-length sequences."
  : "A tab-separated Roary-style table beginning with Gene.");

const progressPercent = computed(() => {
  if (!progress.value.maximum) return 0;
  return Math.min(100, Math.round((progress.value.completed / progress.value.maximum) * 100));
});

watch(source, () => {
  selectedFile.value = null;
  result.value = null;
  progress.value = { completed: 0, maximum: 0, eq: Number.NaN, complete: false };
  errorMessage.value = "";
});

watch(mode, (nextMode) => {
  sparsificationValue.value = nextMode === "knn" ? 15 : 0.5;
});

function onFileChange(event: Event): void {
  const input = event.target as HTMLInputElement;
  selectedFile.value = input.files?.[0] ?? null;
  result.value = null;
  errorMessage.value = "";
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
  if (!selectedFile.value) return;
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
