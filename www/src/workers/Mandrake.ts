import WorkerMandrake from "@/workers/Mandrake.worker";

export interface MandrakeSettings {
  mode: "knn" | "threshold";
  value: number;
  perplexity: number;
  maxUpdates: number;
  repulsionSamples: number;
  learningRate: number;
  initialExaggeration: boolean;
}

export interface MandrakeProgress {
  completed: number;
  maximum: number;
  eq: number;
  complete: boolean;
}

export interface MandrakeResult {
  embedding: Float64Array;
  names: string[];
  completed: number;
  maximum: number;
  eq: number;
}

type ProgressHandler = (progress: MandrakeProgress) => void;

interface ProgressMessage extends MandrakeProgress {
  type: "progress";
}

interface CompleteMessage {
  type: "complete";
  embedding: Float64Array;
  names: string;
  completed: number;
  maximum: number;
  eq: number;
}

interface ErrorMessage {
  type: "error";
  message: string;
}

export class MandrakeRunner {
  private worker: Worker | null = null;
  private progressHandler: ProgressHandler | null = null;
  private resolve: ((result: MandrakeResult) => void) | null = null;
  private reject: ((error: Error) => void) | null = null;

  run(
    source: "alignment" | "accessory",
    bytes: Uint8Array,
    settings: MandrakeSettings,
    onProgress: ProgressHandler,
  ): Promise<MandrakeResult> {
    this.cancel();
    const worker = new WorkerMandrake();
    this.worker = worker;
    this.progressHandler = onProgress;

    const result = new Promise<MandrakeResult>((resolve, reject) => {
      this.resolve = resolve;
      this.reject = reject;
    });
    worker.onmessage = (event: MessageEvent<ProgressMessage | CompleteMessage | ErrorMessage>) => {
      this.handleMessage(event.data);
    };
    worker.onerror = (event: ErrorEvent) => {
      this.fail(new Error(event.message || "Mandrake worker failed"));
    };

    const transferable = bytes.buffer.slice(
      bytes.byteOffset,
      bytes.byteOffset + bytes.byteLength,
    );
    worker.postMessage(
      { type: "start", source, bytes: transferable, settings },
      [transferable],
    );
    return result;
  }

  cancel(): void {
    if (this.worker) {
      this.worker.terminate();
      this.worker = null;
    }
    if (this.reject) {
      this.reject(new Error("Mandrake operation cancelled"));
    }
    this.resolve = null;
    this.reject = null;
    this.progressHandler = null;
  }

  private handleMessage(message: ProgressMessage | CompleteMessage | ErrorMessage): void {
    if (message.type === "error") {
      this.fail(new Error(message.message));
      return;
    }

    if (message.type === "progress") {
      this.progressHandler?.(message);
      if (!message.complete) {
        this.worker?.postMessage({ type: "advance", roundBudget: 64 });
      }
      return;
    }

    this.progressHandler?.({
      completed: message.completed,
      maximum: message.maximum,
      eq: message.eq,
      complete: true,
    });
    const result: MandrakeResult = {
      embedding: message.embedding,
      names: message.names ? message.names.split("\n") : [],
      completed: message.completed,
      maximum: message.maximum,
      eq: message.eq,
    };
    this.resolve?.(result);
    this.cleanupWorker();
  }

  private fail(error: Error): void {
    this.reject?.(error);
    this.cleanupWorker();
  }

  private cleanupWorker(): void {
    this.worker?.terminate();
    this.worker = null;
    this.resolve = null;
    this.reject = null;
    this.progressHandler = null;
  }
}
