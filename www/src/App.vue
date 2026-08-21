<template>
  <div class="app-shell">
    <aside class="sidebar">
      <img class="brand-mark" :src="mandrakeLogo" alt="" aria-hidden="true">
      <div>
        <p class="brand-name">Mandrake</p>
        <p class="brand-caption">stochastic cluster embedding</p>
      </div>
      <nav class="sidebar-nav" aria-label="Main navigation">
        <span class="sidebar-item sidebar-item-active">Embedding</span>
      </nav>
      <p class="sidebar-footer">Runs locally in your browser.</p>
    </aside>

    <main class="main-panel">
      <MandrakePage />
    </main>
  </div>
</template>

<script setup lang="ts">
import MandrakePage from "./components/MandrakePage.vue";
import mandrakeLogo from "./assets/mandrake_logo.png";
</script>
