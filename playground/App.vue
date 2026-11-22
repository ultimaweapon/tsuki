<template>
  <!-- navigation bar -->
  <nav class="navbar navbar-expand bg-body-tertiary">
    <div class="container-fluid">
      <RouterLink class="navbar-brand" to="/" active-class="" exact-active-class="">
        Tsuki
      </RouterLink>
      <div class="collapse navbar-collapse">
        <!-- left group -->
        <ul class="navbar-nav me-auto"></ul>
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
    </div>
  </nav>
  <!-- content -->
  <div>
    <RouterView></RouterView>
  </div>
</template>

<script setup lang="ts">
import { RouterLink, RouterView } from 'vue-router';
</script>
