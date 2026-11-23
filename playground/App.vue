<template>
  <!-- navigation bar -->
  <nav class="navbar navbar-expand bg-body-tertiary sticky-top">
    <div class="container-fluid">
      <!-- left group -->
      <form class="d-flex">
        <button class="btn btn-primary" type="button" @click="run">Run</button>
      </form>
      <!-- right group -->
      <ul class="navbar-nav">
        <li class="nav-item">
          <a class="nav-link" href="https://crates.io/crates/tsuki" target="_blank">
            <img src="./cargo.svg" height="20" />
          </a>
        </li>
        <li class="nav-item">
          <a class="nav-link" href="https://github.com/ultimaweapon/tsuki" target="_blank">
            <i class="bi bi-github"></i>
          </a>
        </li>
      </ul>
    </div>
  </nav>
  <!-- content -->
  <LuaEditor class="flex-grow-1" ref="editor" :init="code"></LuaEditor>
  <OutputViewer style="height: 200px" ref="output"></OutputViewer>
</template>

<script setup lang="ts">
import { onBeforeUnmount, useTemplateRef } from 'vue';
import LuaEditor from './components/lua-editor';
import OutputViewer from './components/output-viewer';
import { Context } from './context';
import * as rust from './rust';

const editor = useTemplateRef('editor');
const output = useTemplateRef('output');
const code = localStorage.getItem('editor.code') ?? undefined;

// Install visibilitychange handler to save Lua code.
const save = () => {
  if (document.hidden) {
    const code = editor.value?.retrieve();

    if (!code) {
      return;
    }

    localStorage.setItem('editor.code', code);
  }
};

document.addEventListener('visibilitychange', save);

// Event handlers.
const run = () => {
  const code = editor.value?.retrieve();

  if (!code || !output.value) {
    return;
  }

  output.value.clear();

  // Run code.
  const c = new Context(output.value);
  let e;

  try {
    e = rust.run(c, code);
  } catch (e) {
    alert(e);
    return;
  }

  if (e) {
    output.value.writeln(e);
  }
};

// Lifecycle hooks.
onBeforeUnmount(() => document.removeEventListener('visibilitychange', save));
</script>
