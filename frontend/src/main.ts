import { installAgentControl } from './automation/bridge.js';
import { createBrowserAdapter } from './adapters/scripted-adapter.js';
import { App } from './app/controller.js';
const root = document.getElementById('app');
const dialogs = document.getElementById('dialog-root');
const toasts = document.getElementById('toast-root');
if (!root || !dialogs || !toasts)
    throw new Error('The application mounting elements are missing.');
const app = new App(createBrowserAdapter(), root, dialogs, toasts);
const disposeAgentControl = installAgentControl(app);
window.addEventListener('pagehide', () => { disposeAgentControl(); app.dispose(); }, { once: true });
