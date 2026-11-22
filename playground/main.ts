import { createApp } from 'vue';
import App from './App.vue';

import '@xterm/xterm/css/xterm.css';

import './main.scss';

// Initialize Vue application.
const app = createApp(App);

app.mount('#app');
