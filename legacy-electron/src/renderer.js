import { setupNavigation, setupAdminSwitch } from './renderer/navigation.js';
import { setupConfigImporter } from './renderer/importer.js';
import { setupForms } from './renderer/forms.js';
import { setupIPCListeners } from './renderer/ipc.js';
import { refreshData } from './renderer/ui.js';

document.addEventListener('DOMContentLoaded', async () => {
  setupNavigation();
  setupConfigImporter();
  setupForms();
  setupIPCListeners();
  setupAdminSwitch();
  
  await refreshData();
});
