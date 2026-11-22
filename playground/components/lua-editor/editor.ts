import * as monaco from 'monaco-editor';
import { defineComponent, h, useTemplateRef } from 'vue';

const isDark = window.matchMedia('(prefers-color-scheme: dark)');

export default defineComponent({
  setup() {
    const root = useTemplateRef('root');
    const editor = null as monaco.editor.IStandaloneCodeEditor | null;
    const onColorSchemeChange = null as ((ev: MediaQueryListEvent) => void) | null;

    return { root, editor, onColorSchemeChange };
  },
  mounted() {
    const editor = monaco.editor.create(this.root as HTMLElement, {
      language: 'lua',
      autoIndent: 'full', // TODO: This does not currently working.
      tabSize: 2,
      theme: isDark.matches ? 'vs-dark' : 'vs',
      automaticLayout: true,
      minimap: {
        enabled: false,
      },
    });

    this.editor = editor;
    this.onColorSchemeChange = (e) => monaco.editor.setTheme(e.matches ? 'vs-dark' : 'vs');

    isDark.addEventListener('change', this.onColorSchemeChange);
  },
  unmounted() {
    if (this.onColorSchemeChange) {
      isDark.removeEventListener('change', this.onColorSchemeChange);
    }

    this.editor?.dispose();
  },
  render() {
    return h('div', { ref: 'root' });
  },
});
