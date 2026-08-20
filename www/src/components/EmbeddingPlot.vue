<template>
  <div ref="plotElement" class="plot-frame" role="img" aria-label="Mandrake two-dimensional embedding" />
</template>

<script setup lang="ts">
import { onBeforeUnmount, onMounted, ref, watch } from "vue";

// plotly.js-dist does not ship usable TypeScript declarations for this build.
// @ts-ignore
import Plotly from "plotly.js-dist";

const props = defineProps<{
  embedding: Float64Array;
  names: string[];
  labels?: string[];
  runKey?: number;
}>();

const plotElement = ref<HTMLDivElement | null>(null);
const palette = [
  "#2563eb",
  "#db2777",
  "#059669",
  "#d97706",
  "#7c3aed",
  "#0891b2",
  "#dc2626",
  "#4f46e5",
  "#65a30d",
  "#c026d3",
];

function render(): void {
  const element = plotElement.value;
  if (!element || props.embedding.length < 2) return;

  const groups = new Map<string, { x: number[]; y: number[]; names: string[] }>();
  const pointCount = Math.floor(props.embedding.length / 2);
  const hasLabels = Boolean(props.labels && props.labels.length === pointCount);
  for (let index = 0; index < pointCount; index += 1) {
    const x = props.embedding[index * 2];
    const y = props.embedding[index * 2 + 1];
    if (!Number.isFinite(x) || !Number.isFinite(y)) continue;
    const label = hasLabels ? (props.labels?.[index] ?? "") : "Samples";
    const group = groups.get(label) ?? { x: [], y: [], names: [] };
    group.x.push(x);
    group.y.push(y);
    group.names.push(props.names[index] ?? `Sample ${index + 1}`);
    groups.set(label, group);
  }
  if (!groups.size) return;

  const labels = Array.from(groups.keys()).sort((left, right) => left.localeCompare(right));
  const traces = labels.map((label, index) => {
    const group = groups.get(label)!;
    return {
      type: "scattergl",
      mode: "markers",
      name: label || "(empty label)",
      x: [...group.x],
      y: [...group.y],
      text: [...group.names],
      hovertemplate: "<b>%{text}</b><br>%{fullData.name}<extra></extra>",
      marker: {
        size: 9,
        color: palette[index % palette.length],
        line: { color: "#ffffff", width: 1.5 },
      },
    };
  });

  const axis = {
    zeroline: false,
    showgrid: true,
    gridcolor: "#d9e2ec",
    linecolor: "#9aa5b1",
    linewidth: 1,
    tickfont: { size: 10, color: "#52606d" },
    title: { font: { size: 11, color: "#52606d" } },
  };
  const layout = {
    height: 520,
    margin: { l: 58, r: hasLabels ? 18 : 22, t: 12, b: 52 },
    paper_bgcolor: "#ffffff",
    plot_bgcolor: "#ffffff",
    font: { family: "DM Sans, sans-serif", size: 11, color: "#18212b" },
    hoverlabel: { font: { size: 11 } },
    hovermode: "closest",
    showlegend: hasLabels,
    legend: { font: { size: 10 }, orientation: "h", y: -0.14 },
    uirevision: props.runKey ?? 0,
    xaxis: { ...axis, title: { ...axis.title, text: "SCE dimension 1" } },
    yaxis: {
      ...axis,
      title: { ...axis.title, text: "SCE dimension 2" },
      scaleanchor: "x",
      scaleratio: 1,
    },
  };
  const config = {
    displaylogo: false,
    responsive: true,
    modeBarButtonsToRemove: ["select2d", "lasso2d", "autoScale2d"],
  };
  Plotly.react(element, traces, layout, config);
}

function resize(): void {
  if (plotElement.value) Plotly.Plots.resize(plotElement.value);
}

onMounted(() => {
  render();
  window.addEventListener("resize", resize);
});

watch(
  () => [props.embedding, props.names, props.labels, props.runKey],
  render,
  { deep: false },
);

onBeforeUnmount(() => {
  window.removeEventListener("resize", resize);
  if (plotElement.value) Plotly.purge(plotElement.value);
});
</script>
