declare module "@/pkg/index" {
  export class MandrakeProgress {
    readonly completed: number;
    readonly maximum: number;
    readonly eq: number;
    readonly complete: boolean;
  }

  export class MandrakeDistanceProgress {
    readonly completed: number;
    readonly maximum: number;
    readonly complete: boolean;
  }

  export class MandrakeOperation {
    static fromAlignment(
      bytes: Uint8Array,
      mode: string,
      value: number,
      perplexity: number,
      maxUpdates: number,
      repulsionSamples: number,
      learningRate: number,
      initialExaggeration: boolean,
    ): MandrakeOperation;
    static fromAccessory(
      bytes: Uint8Array,
      mode: string,
      value: number,
      perplexity: number,
      maxUpdates: number,
      repulsionSamples: number,
      learningRate: number,
      initialExaggeration: boolean,
    ): MandrakeOperation;
    advanceDistances(rowBudget: number): MandrakeDistanceProgress;
    beginEmbedding(): void;
    advance(roundBudget: number): MandrakeProgress;
    embedding(): Float64Array;
    names(): string;
    sample_count(): number;
    is_complete(): boolean;
  }
}
