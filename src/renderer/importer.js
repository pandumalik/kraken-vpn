import { state } from './state.js';
import { showToast } from './toast.js';
import { refreshData } from './ui.js';
import { resetProfileForm } from './forms.js';

export function parseOVPNFile(text, filename) {
  const lines = text.split('\n');
  let server = '';
  let port = '1194';
  let proto = 'OpenVPN UDP';
  let authMethod = 'Certificate';
  let hasAuthUserPass = false;
  let hasToken = false;

  for (let line of lines) {
    line = line.trim();
    if (!line || line.startsWith('#') || line.startsWith(';')) continue;

    const remoteMatch = line.match(/^remote\s+([^\s]+)(?:\s+(\d+))?/i);
    if (remoteMatch) {
      server = remoteMatch[1];
      if (remoteMatch[2]) {
        port = remoteMatch[2];
      }
    }

    const portMatch = line.match(/^port\s+(\d+)/i);
    if (portMatch) {
      port = portMatch[1];
    }

    const protoMatch = line.match(/^proto\s+(\w+)/i);
    if (protoMatch) {
      const pName = protoMatch[1].toLowerCase();
      if (pName.includes('tcp')) {
        proto = 'OpenVPN TCP';
      } else {
        proto = 'OpenVPN UDP';
      }
    }

    if (line.match(/^auth-user-pass/i)) {
      hasAuthUserPass = true;
    }
    if (line.match(/^secret\s+/i) || line.match(/^static-key\s+/i)) {
      hasToken = true;
    }
  }

  if (text.includes('<secret>') || text.includes('</secret>')) {
    hasToken = true;
  }

  if (hasAuthUserPass) {
    authMethod = 'Credentials';
  } else if (hasToken) {
    authMethod = 'Token';
  } else {
    authMethod = 'Certificate';
  }

  const profileName = filename.replace(/\.ovpn$/i, '')
                              .replace(/[-_]/g, ' ')
                              .replace(/\b\w/g, c => c.toUpperCase());

  return { profileName, server, port, proto, authMethod };
}

export function setupConfigImporter() {
  const btnImport = document.getElementById('btn-import-config');
  const fileInput = document.getElementById('file-input');

  if (btnImport && fileInput) {
    btnImport.addEventListener('click', () => {
      fileInput.click();
    });

    fileInput.addEventListener('change', (e) => {
      if (e.target.files.length > 0) {
        handleImportedFile(e.target.files[0]);
      }
    });
  }
}

export async function handleImportedFile(file) {
  if (!file.name.toLowerCase().endsWith('.ovpn')) {
    showToast("Invalid file format. Please select an .ovpn configuration file.", "error");
    return;
  }

  const reader = new FileReader();
  reader.onload = async (e) => {
    const text = e.target.result;
    const parsed = parseOVPNFile(text, file.name);

    if (!parsed.server) {
      showToast("Could not find a valid remote server address in the configuration file.", "error");
      return;
    }

    document.querySelector('.menu-item[data-target="screen-settings"]').click();
    document.querySelector('.tab-btn[data-tab="tab-profiles"]').click();

    let provider = state.appData.providers.find(p => p.server.toLowerCase() === parsed.server.toLowerCase() && p.port === parseInt(parsed.port));
    
    if (!provider) {
      try {
        const providers = await window.electronAPI.saveProvider({
          name: parsed.profileName + ' Provider',
          server: parsed.server,
          port: parseInt(parsed.port),
          protocol: parsed.proto,
          authMethod: parsed.authMethod
        });
        
        showToast("Provider configuration imported successfully!", "success");
        await refreshData();
        
        provider = state.appData.providers.find(p => p.server.toLowerCase() === parsed.server.toLowerCase() && p.port === parseInt(parsed.port));
      } catch (err) {
        console.error(err);
        showToast("Failed to auto-create provider details.", "error");
        return;
      }
    }

    resetProfileForm();
    state.lastImportedConfigContent = text;
    
    document.getElementById('profile-name').value = parsed.profileName;
    document.getElementById('profile-provider-select').value = provider.id;
    document.getElementById('profile-username').focus();

    showToast("Configuration parsed! Please verify settings and enter credentials.", "warning");
  };
  reader.readAsText(file);
}
