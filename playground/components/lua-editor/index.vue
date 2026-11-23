<template>
  <div ref="root"></div>
</template>

<script lang="ts">
const isDark = window.matchMedia('(prefers-color-scheme: dark)');
</script>

<script setup lang="ts">
import * as monaco from 'monaco-editor';
import { onMounted, onUnmounted, shallowRef, useTemplateRef } from 'vue';
import type { Props } from './props';

const props = defineProps<Props>();
const root = useTemplateRef('root');
const editor = shallowRef(null as monaco.editor.IStandaloneCodeEditor | null);

// Public methods.
const retrieve = () => editor.value?.getValue() ?? '';

defineExpose({ retrieve });

// Lifecycle hooks.
let onColorSchemeChange = null as ((ev: MediaQueryListEvent) => void) | null;

onMounted(() => {
  editor.value = monaco.editor.create(root.value as HTMLElement, {
    value: props.init,
    language: 'lua',
    autoIndent: 'full', // TODO: This does not currently working.
    tabSize: 2,
    theme: isDark.matches ? 'vs-dark' : 'vs',
    automaticLayout: true,
    minimap: {
      enabled: false,
    },
  });

  onColorSchemeChange = (e) => monaco.editor.setTheme(e.matches ? 'vs-dark' : 'vs');

  isDark.addEventListener('change', onColorSchemeChange);
});

onUnmounted(() => {
  if (onColorSchemeChange) {
    isDark.removeEventListener('change', onColorSchemeChange);
  }

  if (editor.value) {
    editor.value.dispose();
    editor.value = null;
  }
});
</script>
