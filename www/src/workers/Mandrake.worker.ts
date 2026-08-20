import { MandrakeOperation } from "@/pkg/mandrake";

interface Settings {
  mode: "knn" | "threshold";
  value: number;
  perplexity: number;
  maxUpdates: number;
  repulsionSamples: number;
  learningRate: number;
  initialExaggeration: boolean;
}

interface StartMessage {
  type: "start";
  source: "alignment" | "accessory";
  bytes: ArrayBuffer;
  settings: Settings;
}

interface AdvanceMessage {
  type: "advance";
  roundBudget: number;
}

interface ResetMessage {
  type: "reset";
}

type WorkerMessage = StartMessage | AdvanceMessage | ResetMessage;

let operation: MandrakeOperation | null = null;
let queue = Promise.resolve();

function reportError(error: unknown): void {
  const message = error instanceof Error ? error.message : String(error);
  self.postMessage({ type: "error", message });
}

async function handle(message: WorkerMessage): Promise<void> {
  if (message.type === "reset") {
    operation = null;
    self.postMessage({ type: "reset" });
    return;
  }

  if (message.type === "start") {
    const bytes = new Uint8Array(message.bytes);
    const settings = message.settings;
    operation = message.source === "alignment"
      ? MandrakeOperation.fromAlignment(
          bytes,
          settings.mode,
          settings.value,
          settings.perplexity,
          settings.maxUpdates,
          settings.repulsionSamples,
          settings.learningRate,
          settings.initialExaggeration,
        )
      : MandrakeOperation.fromAccessory(
          bytes,
          settings.mode,
          settings.value,
          settings.perplexity,
          settings.maxUpdates,
          settings.repulsionSamples,
          settings.learningRate,
          settings.initialExaggeration,
        );
    const progress = operation.advance(0);
    self.postMessage({
      type: "progress",
      completed: progress.completed,
      maximum: progress.maximum,
      eq: progress.eq,
      complete: progress.complete,
    });
    return;
  }

  if (!operation) {
    throw new Error("no Mandrake operation is active");
  }

  const progress = operation.advance(message.roundBudget);
  if (progress.complete) {
    const embedding = operation.embedding();
    self.postMessage({
      type: "complete",
      completed: progress.completed,
      maximum: progress.maximum,
      eq: progress.eq,
      embedding,
      names: operation.names(),
    }, { transfer: [embedding.buffer] });
  } else {
    self.postMessage({
      type: "progress",
      completed: progress.completed,
      maximum: progress.maximum,
      eq: progress.eq,
      complete: progress.complete,
    });
  }
}

self.onmessage = (event: MessageEvent<WorkerMessage>) => {
  queue = queue.then(() => handle(event.data)).catch(reportError);
};
