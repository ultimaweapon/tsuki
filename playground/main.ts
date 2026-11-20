import { createApp } from 'vue';
import { createRouter, createWebHistory } from 'vue-router';
import App from './App.vue';

import './main.scss';

// Initialize Vue application.
const app = createApp(App);
const router = createRouter({
  history: createWebHistory(import.meta.env.BASE_URL),
  routes: [
    {
      path: '/',
      name: 'home',
      component: () => import('@/pages/playground'),
    },
  ],
});

app.use(router);

app.mount('#app');
