<template>
  <div class="plot-frame">
    <svg
      class="embedding-plot"
      viewBox="0 0 720 500"
      role="img"
      aria-label="Mandrake two-dimensional embedding"
    >
      <line class="plot-grid" x1="60" y1="40" x2="60" y2="455" />
      <line class="plot-grid" x1="60" y1="455" x2="690" y2="455" />
      <line class="plot-grid plot-grid-faint" x1="60" y1="247" x2="690" y2="247" />
      <line class="plot-grid plot-grid-faint" x1="375" y1="40" x2="375" y2="455" />
      <text class="plot-axis-label" x="375" y="490" text-anchor="middle">SCE dimension 1</text>
      <text class="plot-axis-label" x="17" y="248" text-anchor="middle" transform="rotate(-90 17 248)">SCE dimension 2</text>
      <circle
        v-for="point in points"
        :key="point.index"
        class="plot-point"
        :cx="point.x"
        :cy="point.y"
        r="5"
      >
        <title>{{ point.name }}</title>
      </circle>
    </svg>
  </div>
</template>

<script setup lang="ts">
import { computed } from "vue";

const props = defineProps<{
  embedding: Float64Array;
  names: string[];
}>();

const points = computed(() => {
  const values = Array.from(props.embedding);
  const xs = values.filter((_, index) => index % 2 === 0 && Number.isFinite(values[index]));
  const ys = values.filter((_, index) => index % 2 === 1 && Number.isFinite(values[index]));
  if (!xs.length || xs.length !== ys.length) return [];

  const minX = xs.reduce((minimum, value) => Math.min(minimum, value), Number.POSITIVE_INFINITY);
  const maxX = xs.reduce((maximum, value) => Math.max(maximum, value), Number.NEGATIVE_INFINITY);
  const minY = ys.reduce((minimum, value) => Math.min(minimum, value), Number.POSITIVE_INFINITY);
  const maxY = ys.reduce((maximum, value) => Math.max(maximum, value), Number.NEGATIVE_INFINITY);
  const scale = (value: number, minimum: number, maximum: number, start: number, end: number) => {
    if (maximum === minimum) return (start + end) / 2;
    return start + ((value - minimum) / (maximum - minimum)) * (end - start);
  };

  return xs.map((x, index) => ({
    index,
    name: props.names[index] ?? `Sample ${index + 1}`,
    x: scale(x, minX, maxX, 70, 680),
    y: scale(ys[index], minY, maxY, 445, 50),
  }));
});
</script>
