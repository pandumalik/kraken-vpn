import { state } from './state.js';
import { showToast } from './toast.js';
import { refreshData } from './ui.js';

export function setupForms() {
  const btnTogglePassword = document.getElementById('btn-toggle-password');
  const passwordInput = document.getElementById('profile-password');
  const eyeIcon = document.getElementById('eye-icon');
  
  if (btnTogglePassword && passwordInput && eyeIcon) {
    btnTogglePassword.addEventListener('click', () => {
      const isPassword = passwordInput.getAttribute('type') === 'password';
      passwordInput.setAttribute('type', isPassword ? 'text' : 'password');
      
      if (isPassword) {
        eyeIcon.innerHTML = `<path fill="currentColor" d="M12 7c2.76 0 5 2.24 5 5 0 .65-.13 1.26-.36 1.82l2.92 2.92c1.51-1.2 2.7-2.78 3.44-4.74-1.73-4.39-6-7.5-11-7.5-1.4 0-2.74.25-3.98.7l2.16 2.16C10.74 7.13 11.35 7 12 7zM2 4.27l2.28 2.28.46.46C3.08 8.3 1.78 10.02 1 12c1.73 4.39 6 7.5 11 7.5 1.55 0 3.03-.3 4.38-.84l.42.42L19.73 22 21 20.73 3.27 3 2 4.27zM7.53 9.8l1.55 1.55c-.05.21-.08.43-.08.65 0 1.66 1.34 3 3 3 .22 0 .44-.03.65-.08l1.55 1.55c-.67.33-1.41.53-2.2.53-2.76 0-5-2.24-5-5 0-.79.2-1.53.53-2.2zm4.72-4.72l1.63 1.63c.27.08.53.21.77.38l-1.2 1.2c-.37-.32-.86-.53-1.4-.53-1.66 0-3 1.34-3 3 0 .54.21 1.03.53 1.4l-1.2 1.2c-.17-.24-.3-.5-.38-.77L7.6 8.3c.7-.7 1.55-1.27 2.5-1.63L11 6c.32-.43.65-.87 1.25-1.25V5c.01-.27.14-.5.37-.65z"/>`;
      } else {
        eyeIcon.innerHTML = `<path fill="currentColor" d="M12 4.5C7 4.5 2.73 7.61 1 12c1.73 4.39 6 7.5 11 7.5s9.27-3.11 11-7.5c-1.73-4.39-6-7.5-11-7.5zM12 17c-2.76 0-5-2.24-5-5s2.24-5 5-5 5 2.24 5 5-2.24 5-5 5zm0-8c-1.66 0-3 1.34-3 3s1.34 3 3 3 3-1.34 3-3-1.34-3-3-3z"/>`;
      }
    });
  }

  const profileProviderSelect = document.getElementById('profile-provider-select');
  if (profileProviderSelect) {
    profileProviderSelect.addEventListener('change', () => {
      const selectedProvId = profileProviderSelect.value;
      const provider = state.appData.providers.find(p => p.id === selectedProvId);
      
      const usernameInput = document.getElementById('profile-username');
      const passwordInput = document.getElementById('profile-password');
      const usernameLabel = document.querySelector('label[for="profile-username"]');
      const passwordLabel = document.querySelector('label[for="profile-password"]');
      
      if (usernameInput && passwordInput && usernameLabel && passwordLabel) {
        if (provider) {
          if (provider.authMethod === 'Certificate') {
            usernameLabel.innerText = 'Username (Optional)';
            usernameInput.placeholder = 'Optional username (if required by server)';
            usernameInput.required = false;
            
            passwordLabel.innerText = 'Private Key Passphrase (Optional)';
            passwordInput.placeholder = 'Enter passphrase if private key is encrypted';
            passwordInput.required = false;
          } else if (provider.authMethod === 'Token') {
            usernameLabel.innerText = 'Token ID (Required)';
            usernameInput.placeholder = 'Enter API Token ID';
            usernameInput.required = true;
            
            passwordLabel.innerText = 'Token Secret Key (Required)';
            passwordInput.placeholder = 'Enter token secret key';
            passwordInput.required = true;
          } else { // Credentials
            usernameLabel.innerText = 'Username (Required)';
            usernameInput.placeholder = 'Enter username';
            usernameInput.required = true;
            
            passwordLabel.innerText = 'Password (Required)';
            passwordInput.placeholder = 'Enter password';
            passwordInput.required = true;
          }
        } else {
          usernameLabel.innerText = 'Username (Required)';
          usernameInput.placeholder = 'Enter username';
          usernameInput.required = true;
          
          passwordLabel.innerText = 'Password (Required)';
          passwordInput.placeholder = 'Enter password';
          passwordInput.required = true;
        }
      }
    });
  }

  const providerForm = document.getElementById('provider-form');
  if (providerForm) {
    providerForm.addEventListener('submit', async (e) => {
      e.preventDefault();
      
      const providerId = document.getElementById('provider-id').value;
      const name = document.getElementById('provider-name').value;
      const server = document.getElementById('provider-server').value;
      const protocol = document.getElementById('provider-protocol').value;
      const port = document.getElementById('provider-port').value;
      const authMethod = document.getElementById('provider-auth').value;

      try {
        await window.electronAPI.saveProvider({
          id: providerId || undefined,
          name,
          server,
          protocol,
          port: parseInt(port),
          authMethod
        });

        showToast(providerId ? "Provider updated successfully!" : "Provider created successfully!", "success");
        resetProviderForm();
        await refreshData();
      } catch (err) {
        console.error(err);
        showToast("Failed to save provider.", "error");
      }
    });
  }

  const cancelProvBtn = document.getElementById('btn-cancel-provider');
  if (cancelProvBtn) {
    cancelProvBtn.addEventListener('click', () => {
      resetProviderForm();
    });
  }

  const profileForm = document.getElementById('profile-form');
  if (profileForm) {
    profileForm.addEventListener('submit', async (e) => {
      e.preventDefault();

      const profileId = document.getElementById('profile-id').value;
      const name = document.getElementById('profile-name').value;
      const providerId = document.getElementById('profile-provider-select').value;
      const username = document.getElementById('profile-username').value;
      const password = document.getElementById('profile-password').value;
      const servercert = document.getElementById('profile-servercert').value;
      const interfaceName = document.getElementById('profile-interface').value;
      const autoConnect = document.getElementById('profile-auto-connect').checked;

      try {
        let configContent = state.lastImportedConfigContent;
        if (!configContent && profileId) {
          const existingProf = state.appData.profiles.find(p => p.id === profileId);
          if (existingProf) {
            configContent = existingProf.configContent;
          }
        }

        await window.electronAPI.saveProfile({
          id: profileId || undefined,
          name,
          providerId,
          username,
          password,
          servercert,
          interfaceName,
          autoConnect,
          configContent
        });

        showToast(profileId ? "Profile updated successfully!" : "Profile created successfully!", "success");
        resetProfileForm();
        await refreshData();
      } catch (err) {
        console.error(err);
        showToast("Failed to save credentials profile.", "error");
      }
    });
  }

  const cancelProfBtn = document.getElementById('btn-cancel-profile');
  if (cancelProfBtn) {
    cancelProfBtn.addEventListener('click', () => {
      resetProfileForm();
    });
  }

  const testProvBtn = document.getElementById('btn-test-provider');
  if (testProvBtn) {
    testProvBtn.addEventListener('click', async () => {
      const server = document.getElementById('provider-server').value;
      const protocol = document.getElementById('provider-protocol').value;
      const port = document.getElementById('provider-port').value;
      if (!server || !port || !protocol) {
        showToast("Please fill in Server, Protocol, and Port before testing.", "warning");
        return;
      }

      const originalText = testProvBtn.innerText;
      testProvBtn.disabled = true;
      testProvBtn.innerText = "Testing...";

      try {
        const res = await window.electronAPI.testVPNConnection({
          server,
          protocol,
          port: parseInt(port)
        });

        if (res.success) {
          showToast(res.message || "Connection test successful!", "success");
        } else {
          showToast(res.message || "Connection test failed.", "error");
          if (res.detectedPin) {
            try {
              await navigator.clipboard.writeText(res.detectedPin);
              showToast("Certificate pin copied to clipboard!", "success");
            } catch (clipErr) {
              console.error("Clipboard copy failed:", clipErr);
            }
          }
        }
      } catch (err) {
        console.error(err);
        showToast("Error during test: " + err.message, "error");
      } finally {
        testProvBtn.disabled = false;
        testProvBtn.innerText = originalText;
      }
    });
  }

  const testProfBtn = document.getElementById('btn-test-profile');
  if (testProfBtn) {
    testProfBtn.addEventListener('click', async () => {
      const providerId = document.getElementById('profile-provider-select').value;
      const provider = state.appData.providers.find(p => p.id === providerId);
      if (!provider) {
        showToast("Please select a VPN Provider first.", "warning");
        return;
      }
      const username = document.getElementById('profile-username').value;
      const password = document.getElementById('profile-password').value;
      const servercert = document.getElementById('profile-servercert').value;
      const interfaceName = document.getElementById('profile-interface').value;

      const originalText = testProfBtn.innerText;
      testProfBtn.disabled = true;
      testProfBtn.innerText = "Testing...";

      try {
        const res = await window.electronAPI.testVPNConnection({
          server: provider.server,
          protocol: provider.protocol,
          port: provider.port,
          username,
          password,
          servercert,
          interfaceName
        });

        if (res.success) {
          showToast(res.message || "Connection test successful!", "success");
        } else {
          showToast(res.message || "Connection test failed.", "error");
          if (res.detectedPin) {
            try {
              await navigator.clipboard.writeText(res.detectedPin);
              showToast("Certificate pin copied to clipboard! Paste it in the SHA field.", "success");
            } catch (clipErr) {
              console.error("Clipboard copy failed:", clipErr);
            }
          }
        }
      } catch (err) {
        console.error(err);
        showToast("Error during test: " + err.message, "error");
      } finally {
        testProfBtn.disabled = false;
        testProfBtn.innerText = originalText;
      }
    });
  }

  const startupSetting = document.getElementById('setting-startup');
  if (startupSetting) {
    startupSetting.addEventListener('change', async (e) => {
      try {
        await window.electronAPI.saveSettings({ autoConnectOnStartup: e.target.checked });
        showToast("Startup configuration updated.", "success");
      } catch (err) {
        console.error(err);
        showToast("Failed to update startup registry setting.", "error");
      }
    });
  }

  const traySetting = document.getElementById('setting-tray');
  if (traySetting) {
    traySetting.addEventListener('change', async (e) => {
      try {
        await window.electronAPI.saveSettings({ minimizeToTray: e.target.checked });
        showToast("Minimize to tray configuration updated.", "success");
      } catch (err) {
        console.error(err);
        showToast("Failed to update window close setting.", "error");
      }
    });
  }

  const openconnectPathInput = document.getElementById('setting-openconnect-path');
  if (openconnectPathInput) {
    openconnectPathInput.addEventListener('change', async (e) => {
      try {
        await window.electronAPI.saveSettings({ openconnectPath: e.target.value });
        showToast("OpenConnect CLI path updated.", "success");
      } catch (err) {
        console.error(err);
        showToast("Failed to save OpenConnect path setting.", "error");
      }
    });
  }

  const browseOpenConnectBtn = document.getElementById('btn-browse-openconnect');
  if (browseOpenConnectBtn) {
    browseOpenConnectBtn.addEventListener('click', async () => {
      try {
        const selectedPath = await window.electronAPI.selectOpenConnectPath();
        if (selectedPath) {
          document.getElementById('setting-openconnect-path').value = selectedPath;
          await window.electronAPI.saveSettings({ openconnectPath: selectedPath });
          showToast("OpenConnect CLI path updated.", "success");
        }
      } catch (err) {
        console.error(err);
        showToast("Failed to open path selector.", "error");
      }
    });
  }

  const openvpnPathInput = document.getElementById('setting-openvpn-path');
  if (openvpnPathInput) {
    openvpnPathInput.addEventListener('change', async (e) => {
      try {
        await window.electronAPI.saveSettings({ openvpnPath: e.target.value });
        showToast("OpenVPN CLI path updated.", "success");
      } catch (err) {
        console.error(err);
        showToast("Failed to save OpenVPN path setting.", "error");
      }
    });
  }

  const browseOpenVPNBtn = document.getElementById('btn-browse-openvpn');
  if (browseOpenVPNBtn) {
    browseOpenVPNBtn.addEventListener('click', async () => {
      try {
        const selectedPath = await window.electronAPI.selectOpenVPNPath();
        if (selectedPath) {
          document.getElementById('setting-openvpn-path').value = selectedPath;
          await window.electronAPI.saveSettings({ openvpnPath: selectedPath });
          showToast("OpenVPN CLI path updated.", "success");
        }
      } catch (err) {
        console.error(err);
        showToast("Failed to open path selector.", "error");
      }
    });
  }

  const clearLogsBtn = document.getElementById('btn-clear-logs');
  if (clearLogsBtn) {
    clearLogsBtn.addEventListener('click', async () => {
      if (confirm("Are you sure you want to clear all historical connection logs?")) {
        try {
          await window.electronAPI.clearLogs();
          
          const logTerminal = document.getElementById('log-terminal');
          if (logTerminal) {
            logTerminal.innerHTML = '<div class="terminal-line system-msg">[SYSTEM] Diagnostic console streaming started. Waiting for tunnel triggers...</div>';
          }
          
          showToast("Logs cleared successfully.", "success");
          await refreshData();
        } catch (err) {
          console.error(err);
          showToast("Failed to clear historical database logs.", "error");
        }
      }
    });
  }

  const themeSwitch = document.getElementById('setting-theme');
  if (themeSwitch) {
    themeSwitch.addEventListener('change', async (e) => {
      const selectedTheme = e.target.checked ? 'dark' : 'light';
      applyTheme(selectedTheme);
      try {
        await window.electronAPI.saveSettings({ theme: selectedTheme });
        showToast(`${selectedTheme === 'dark' ? 'Dark' : 'Light'} theme applied.`, "success");
      } catch (err) {
        console.error(err);
        showToast("Failed to save theme setting.", "error");
      }
    });
  }

  const btnClearStats = document.getElementById('btn-clear-stats');
  if (btnClearStats) {
    btnClearStats.addEventListener('click', async () => {
      try {
        await window.electronAPI.clearConnectionTelemetry();
        state.appData.profiles.forEach(p => {
          if (p.telemetry) {
            p.telemetry.bytesIn = 0;
            p.telemetry.bytesOut = 0;
            p.telemetry.downloadSpeed = 0;
            p.telemetry.uploadSpeed = 0;
          }
        });
        updateGlobalCounters();
        showToast("Data count cleared.", "success");
      } catch (err) {
        console.error(err);
        showToast("Failed to clear stats.", "error");
      }
    });
  }
}

export function resetProviderForm() {
  const form = document.getElementById('provider-form');
  if (form) form.reset();
  const idElem = document.getElementById('provider-id');
  if (idElem) idElem.value = '';
  const titleElem = document.getElementById('provider-form-title');
  if (titleElem) titleElem.innerText = 'Add VPN Provider';
}

export function resetProfileForm() {
  const form = document.getElementById('profile-form');
  if (form) form.reset();
  const idElem = document.getElementById('profile-id');
  if (idElem) idElem.value = '';
  const certElem = document.getElementById('profile-servercert');
  if (certElem) certElem.value = '';
  const intfElem = document.getElementById('profile-interface');
  if (intfElem) intfElem.value = '';
  const titleElem = document.getElementById('profile-form-title');
  if (titleElem) titleElem.innerText = 'Add VPN Profile';
  
  const passwordInput = document.getElementById('profile-password');
  const eyeIcon = document.getElementById('eye-icon');
  if (passwordInput && eyeIcon) {
    passwordInput.setAttribute('type', 'password');
    eyeIcon.innerHTML = `<path fill="currentColor" d="M12 4.5C7 4.5 2.73 7.61 1 12c1.73 4.39 6 7.5 11 7.5s9.27-3.11 11-7.5c-1.73-4.39-6-7.5-11-7.5zM12 17c-2.76 0-5-2.24-5-5s2.24-5 5-5 5 2.24 5 5-2.24 5-5 5zm0-8c-1.66 0-3 1.34-3 3s1.34 3 3 3 3-1.34 3-3-1.34-3-3-3z"/>`;
  }

  state.lastImportedConfigContent = '';
  
  const profileProviderSelect = document.getElementById('profile-provider-select');
  if (profileProviderSelect) {
    profileProviderSelect.dispatchEvent(new Event('change'));
  }
}

export function editProvider(id) {
  const p = state.appData.providers.find(prov => prov.id === id);
  if (!p) return;

  document.getElementById('provider-id').value = p.id;
  document.getElementById('provider-name').value = p.name;
  document.getElementById('provider-server').value = p.server;
  document.getElementById('provider-protocol').value = p.protocol;
  document.getElementById('provider-port').value = p.port;
  document.getElementById('provider-auth').value = p.authMethod;

  document.getElementById('provider-form-title').innerText = 'Edit VPN Provider';
  
  document.querySelector('.tab-btn[data-tab="tab-providers"]').click();
  document.getElementById('provider-name').focus();
}

export async function deleteProvider(id) {
  if (confirm("Delete this provider? WARNING: This will also remove any VPN profiles associated with it!")) {
    try {
      await window.electronAPI.deleteProvider(id);
      showToast("Provider and linked profiles removed.", "success");
      await refreshData();
    } catch (err) {
      console.error(err);
      showToast("Failed to remove provider.", "error");
    }
  }
}

export function editProfile(id) {
  const p = state.appData.profiles.find(prof => prof.id === id);
  if (!p) return;

  document.getElementById('profile-id').value = p.id;
  document.getElementById('profile-name').value = p.name;
  document.getElementById('profile-provider-select').value = p.providerId;
  document.getElementById('profile-username').value = p.username;
  document.getElementById('profile-password').value = p.password;
  document.getElementById('profile-servercert').value = p.servercert || '';
  document.getElementById('profile-interface').value = p.interfaceName || '';
  document.getElementById('profile-auto-connect').checked = p.autoConnect;
  
  state.lastImportedConfigContent = p.configContent || '';

  const profileProviderSelect = document.getElementById('profile-provider-select');
  if (profileProviderSelect) {
    profileProviderSelect.dispatchEvent(new Event('change'));
  }

  document.getElementById('profile-form-title').innerText = 'Edit VPN Profile';
  
  document.querySelector('.menu-item[data-target="screen-settings"]').click();
  document.querySelector('.tab-btn[data-tab="tab-profiles"]').click();
  document.getElementById('profile-name').focus();
}

export async function deleteProfile(id) {
  if (confirm("Are you sure you want to delete this VPN credentials profile?")) {
    try {
      await window.electronAPI.deleteProfile(id);
      showToast("Profile credentials deleted.", "success");
      await refreshData();
    } catch (err) {
      console.error(err);
      showToast("Failed to delete profile.", "error");
    }
  }
}

// Inline fallback themes handler to prevent modular circular dependency import issues
function applyTheme(theme) {
  if (theme === 'light') {
    document.body.classList.remove('dark-mode');
    document.body.classList.add('light-mode');
  } else {
    document.body.classList.remove('light-mode');
    document.body.classList.add('dark-mode');
  }
}

function updateGlobalCounters() {
  const connectedProfiles = state.appData.profiles.filter(p => p.status === 'Connected');
  const activeCountElem = document.getElementById('active-count');
  if (activeCountElem) {
    activeCountElem.innerText = connectedProfiles.length;
  }

  let totalBytesIn = 0;
  let totalBytesOut = 0;
  let totalSpeedDown = 0;
  let totalSpeedUp = 0;

  state.appData.profiles.forEach(p => {
    if (p.telemetry) {
      totalBytesIn += p.telemetry.bytesIn;
      totalBytesOut += p.telemetry.bytesOut;
      totalSpeedDown += p.telemetry.downloadSpeed;
      totalSpeedUp += p.telemetry.uploadSpeed;
    }
  });

  const globalBytesInElem = document.getElementById('global-bytes-in');
  if (globalBytesInElem) {
    // Dynamically retrieve formatting helpers from state module
    import('./state.js').then(({ formatBytes, formatSpeed }) => {
      globalBytesInElem.innerText = formatBytes(totalBytesIn);
      const globalBytesOutElem = document.getElementById('global-bytes-out');
      if (globalBytesOutElem) globalBytesOutElem.innerText = formatBytes(totalBytesOut);
      
      const combinedDownElem = document.getElementById('combined-down');
      if (combinedDownElem) combinedDownElem.innerText = formatSpeed(totalSpeedDown);
      const combinedUpElem = document.getElementById('combined-up');
      if (combinedUpElem) combinedUpElem.innerText = formatSpeed(totalSpeedUp);
    });
  }
}
