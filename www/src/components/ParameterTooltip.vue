<template>
  <span class="tooltip-wrap">
    <button
      :id="triggerId"
      class="info-dot"
      type="button"
      :aria-label="`More information: ${props.text}`"
      :aria-describedby="tooltipId"
      @click.stop.prevent
    >
      i
    </button>
    <span :id="tooltipId" class="tooltip-content" role="tooltip">{{ props.text }}</span>
  </span>
</template>

<script setup lang="ts">
const props = defineProps<{
  text: string;
}>();

const tooltipId = `mandrake-tooltip-${Math.random().toString(36).slice(2)}`;
const triggerId = `${tooltipId}-trigger`;
</script>
