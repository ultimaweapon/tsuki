<template>
  <div ref="root"></div>
</template>

<script setup lang="ts">
import { FitAddon } from '@xterm/addon-fit';
import { Terminal } from '@xterm/xterm';
import { onMounted, onUnmounted, useTemplateRef } from 'vue';

// Create read-only xterm.js.
const root = useTemplateRef('root');
const term = new Terminal({
  fontSize: 12,
  disableStdin: true,
  cursorInactiveStyle: 'none',
  cursorStyle: 'underline',
});

term.attachCustomKeyEventHandler(() => false);

// Set up @xterm/addon-fit.
const fit = new FitAddon();
const observer = new ResizeObserver(() => fit.fit());

term.loadAddon(fit);

// Public methods.
const clear = () => term.clear();
const write = (data: string | Uint8Array) => term.write(data);
const writeln = (data: string | Uint8Array) => term.writeln(data);

defineExpose({ clear, write, writeln });

// Lifecycle hooks.
onMounted(() => {
  term.open(root.value as HTMLElement);

  fit.fit();

  observer.observe(root.value as Element);
});

onUnmounted(() => {
  observer.disconnect();
  term.dispose();
});
</script>
