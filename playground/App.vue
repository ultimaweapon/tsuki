<template>
  <!-- navigation bar -->
  <nav class="navbar navbar-expand bg-body-tertiary sticky-top">
    <div class="container-fluid">
      <!-- left group -->
      <form class="d-flex">
        <button class="btn btn-primary" type="button">Run</button>
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
  <LuaEditor class="flex-grow-1"></LuaEditor>
</template>

<script setup lang="ts">
import LuaEditor from './components/lua-editor';
</script>
