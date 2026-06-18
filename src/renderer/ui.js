import { state, formatBytes, formatSpeed } from './state.js';
import { showToast } from './toast.js';
import { editProfile, deleteProfile, editProvider, deleteProvider } from './forms.js';

export function applyTheme(theme) {
  if (theme === 'light') {
    document.body.classList.remove('dark-mode');
    document.body.classList.add('light-mode');
  } else {
    document.body.classList.remove('light-mode');
    document.body.classList.add('dark-mode');
  }
}

export async function refreshData() {
  try {
    state.appData = await window.electronAPI.loadAppData();
    
    // Apply current theme settings on load
    applyTheme(state.appData.settings.theme || 'dark');
    
    // Toggle Admin rights warning banner
    const banner = document.getElementById('admin-warning-banner');
    if (banner) {
      if (state.appData.isAdmin === false) {
        banner.style.display = 'flex';
      } else {
        banner.style.display = 'none';
      }
    }

    // Sync Admin switch
    const adminSwitch = document.getElementById('admin-privilege-switch');
    const adminLabel = document.querySelector('.admin-request-label');
    if (adminSwitch) {
      if (state.appData.isAdmin) {
        adminSwitch.checked = true;
        adminSwitch.disabled = true;
        if (adminLabel) {
          adminLabel.innerHTML = 'Admin Mode <span class="admin-status-dot"></span> <span class="admin-status-text">Active</span>';
        }
      } else {
        adminSwitch.checked = false;
        adminSwitch.disabled = false;
        if (adminLabel) {
          adminLabel.innerText = 'Admin Privilege';
        }
      }
    }
    
    renderDashboardGrid();
    renderProvidersTable();
    renderProfilesTable();
    populateProviderSelect();
    renderLogsHistory();
    syncGeneralSettingsForm();
    updateGlobalCounters();
    
  } catch (err) {
    console.error("Error loading application configurations:", err);
    showToast("Failed to retrieve system settings from storage.", "error");
  }
}

export function updateGlobalCounters() {
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
  if (globalBytesInElem) globalBytesInElem.innerText = formatBytes(totalBytesIn);
  const globalBytesOutElem = document.getElementById('global-bytes-out');
  if (globalBytesOutElem) globalBytesOutElem.innerText = formatBytes(totalBytesOut);
  
  const combinedDownElem = document.getElementById('combined-down');
  if (combinedDownElem) combinedDownElem.innerText = formatSpeed(totalSpeedDown);
  const combinedUpElem = document.getElementById('combined-up');
  if (combinedUpElem) combinedUpElem.innerText = formatSpeed(totalSpeedUp);

  const dot = document.getElementById('global-status-dot');
  const text = document.getElementById('global-status-text');

  if (dot && text) {
    if (connectedProfiles.length > 0) {
      dot.className = "status-indicator-dot active";
      text.innerText = `${connectedProfiles.length} Tunnel(s) Connected`;
    } else if (state.appData.profiles.some(p => p.status === 'Connecting' || p.status === 'Disconnecting')) {
      dot.className = "status-indicator-dot connecting";
      text.innerText = `Establishing tunnels...`;
    } else {
      dot.className = "status-indicator-dot";
      text.innerText = "All Tunnels Offline";
    }
  }
}

export function renderDashboardGrid() {
  const vpnList = document.getElementById('vpn-list');
  const emptyState = document.getElementById('empty-profiles');
  if (!vpnList || !emptyState) return;

  // Clear previous cards, keeping empty state
  Array.from(vpnList.children).forEach(child => {
    if (child.id !== 'empty-profiles') child.remove();
  });

  if (!state.appData.profiles || state.appData.profiles.length === 0) {
    emptyState.style.display = 'block';
    return;
  }
  
  emptyState.style.display = 'none';

  state.appData.profiles.forEach(profile => {
    const provider = state.appData.providers.find(p => p.id === profile.providerId);
    const providerName = provider ? provider.name : 'Unknown Provider';
    const serverAddress = provider ? `${provider.server}:${provider.port}` : 'n/a';
    const protoType = provider ? provider.protocol : 'n/a';
    const isConnected = profile.status === 'Connected';
    const isConnecting = profile.status === 'Connecting';
    const isDisconnecting = profile.status === 'Disconnecting';
    
    let statusClass = 'badge-disconnected';
    if (isConnected) statusClass = 'badge-connected';
    if (isConnecting) statusClass = 'badge-connecting';
    if (isDisconnecting) statusClass = 'badge-disconnecting';

    const card = document.createElement('div');
    card.className = `vpn-card ${isConnected ? 'connected' : ''}`;
    card.id = `card-prof-${profile.id}`;

    let telemetryHtml = '';
    if (isConnected && profile.telemetry) {
      telemetryHtml = `
        <div class="vpn-telemetry-row">
          <div class="vpn-telemetry-item">
            <span class="title">Speed Down</span>
            <span class="val text-cyan" id="speed-down-${profile.id}">${formatSpeed(profile.telemetry.downloadSpeed)}</span>
          </div>
          <div class="vpn-telemetry-item">
            <span class="title">Speed Up</span>
            <span class="val text-green" id="speed-up-${profile.id}">${formatSpeed(profile.telemetry.uploadSpeed)}</span>
          </div>
        </div>
      `;
    }

    card.innerHTML = `
      <div class="vpn-card-header">
        <div>
          <h3 class="vpn-card-title">${profile.name}</h3>
          <span class="vpn-card-provider">${providerName}</span>
        </div>
        <span class="badge ${statusClass}" id="badge-status-${profile.id}">${profile.status}</span>
      </div>

      <div class="vpn-card-details">
        <div class="vpn-detail-row">
          <span class="label">Server Host:</span>
          <span class="value">${serverAddress}</span>
        </div>
        <div class="vpn-detail-row">
          <span class="label">Protocol / Port:</span>
          <span class="value">${protoType}</span>
        </div>
        <div class="vpn-detail-row">
          <span class="label">Username:</span>
          <span class="value connected-user">${profile.username}</span>
        </div>
        ${profile.interfaceName ? `
        <div class="vpn-detail-row">
          <span class="label">Interface:</span>
          <span class="value" style="font-family: var(--font-mono); font-size: 0.8rem;">${profile.interfaceName}</span>
        </div>
        ` : ''}
        <div class="vpn-detail-row">
          <span class="label">Session duration:</span>
          <span class="value" id="timer-${profile.id}">00:00:00</span>
        </div>
      </div>

      ${telemetryHtml}

      <div class="vpn-card-actions">
        <div class="card-action-group">
          <button class="card-icon-btn edit" data-id="${profile.id}" title="Edit Profile">
            <svg viewBox="0 0 24 24" width="16" height="16"><path fill="currentColor" d="M3 17.25V21h3.75L17.81 9.94l-3.75-3.75L3 17.25zM20.71 7.04a9.96 9.96 0 0 0 0-1.41l-2.34-2.34a9.96 9.96 0 0 0-1.41 0l-1.83 1.83 3.75 3.75 1.83-1.83z"/></svg>
          </button>
          <button class="card-icon-btn delete" data-id="${profile.id}" title="Delete Profile">
            <svg viewBox="0 0 24 24" width="16" height="16"><path fill="currentColor" d="M6 19c0 1.1.9 2 2 2h8c1.1 0 2-.9 2-2V7H6v12zM19 4h-3.5l-1-1h-5l-1 1H5v2h14V4z"/></svg>
          </button>
        </div>

        <label class="toggle-switch">
          <input type="checkbox" class="vpn-connect-toggle" data-id="${profile.id}" 
            ${isConnected ? 'checked' : ''} 
            ${isConnecting || isDisconnecting ? 'disabled' : ''}>
          <span class="toggle-slider"></span>
        </label>
      </div>
    `;

    vpnList.appendChild(card);
  });

  setupCardButtons();
}

export function setupCardButtons() {
  const toggles = document.querySelectorAll('.vpn-connect-toggle');
  toggles.forEach(toggle => {
    toggle.addEventListener('change', async (e) => {
      const profileId = toggle.getAttribute('data-id');
      const isChecked = toggle.checked;

      toggle.disabled = true;
      try {
        if (isChecked) {
          await window.electronAPI.connectVPN(profileId);
          showToast(`Connecting to VPN profile...`, 'warning');
        } else {
          await window.electronAPI.disconnectVPN(profileId);
          showToast(`Disconnecting VPN tunnel...`, 'warning');
          
          const profile = state.appData.profiles.find(p => p.id === profileId);
          if (profile) {
            profile.status = 'Disconnected';
            profile.telemetry = null;
          }
          renderDashboardGrid();
          updateGlobalCounters();
        }
      } catch (err) {
        console.error(err);
        showToast(err.message || "Failed to trigger tunnel state change.", "error");
        toggle.checked = !isChecked;
        toggle.disabled = false;
      }
    });
  });

  document.querySelectorAll('.vpn-card .edit').forEach(btn => {
    btn.addEventListener('click', () => {
      editProfile(btn.getAttribute('data-id'));
    });
  });

  document.querySelectorAll('.vpn-card .delete').forEach(btn => {
    btn.addEventListener('click', () => {
      deleteProfile(btn.getAttribute('data-id'));
    });
  });
}

export function populateProviderSelect() {
  const select = document.getElementById('profile-provider-select');
  if (!select) return;
  select.innerHTML = '<option value="" disabled selected>Select a Provider...</option>';
  
  state.appData.providers.forEach(p => {
    const opt = document.createElement('option');
    opt.value = p.id;
    opt.innerText = `${p.name} (${p.protocol})`;
    select.appendChild(opt);
  });
}

export function renderProvidersTable() {
  const tbody = document.getElementById('providers-table-body');
  if (!tbody) return;
  tbody.innerHTML = '';

  if (!state.appData.providers || state.appData.providers.length === 0) {
    tbody.innerHTML = '<tr><td colspan="5" style="text-align: center; color: var(--text-muted);">No VPN providers configured.</td></tr>';
    return;
  }

  state.appData.providers.forEach(p => {
    const tr = document.createElement('tr');
    tr.innerHTML = `
      <td><strong>${p.name}</strong></td>
      <td>${p.server}</td>
      <td><span class="badge badge-disconnected">${p.protocol}</span> port ${p.port}</td>
      <td>${p.authMethod}</td>
      <td>
        <div class="card-action-group">
          <button class="card-icon-btn edit-prov" data-id="${p.id}" title="Edit Provider">
            <svg viewBox="0 0 24 24" width="14" height="14"><path fill="currentColor" d="M3 17.25V21h3.75L17.81 9.94l-3.75-3.75L3 17.25z"/></svg>
          </button>
          <button class="card-icon-btn delete delete-prov" data-id="${p.id}" title="Delete Provider">
            <svg viewBox="0 0 24 24" width="14" height="14"><path fill="currentColor" d="M6 19c0 1.1.9 2 2 2h8c1.1 0 2-.9 2-2V7H6v12z"/></svg>
          </button>
        </div>
      </td>
    `;
    tbody.appendChild(tr);
  });

  document.querySelectorAll('.edit-prov').forEach(btn => {
    btn.addEventListener('click', () => editProvider(btn.getAttribute('data-id')));
  });

  document.querySelectorAll('.delete-prov').forEach(btn => {
    btn.addEventListener('click', () => deleteProvider(btn.getAttribute('data-id')));
  });
}

export function renderProfilesTable() {
  const tbody = document.getElementById('profiles-table-body');
  if (!tbody) return;
  tbody.innerHTML = '';

  if (!state.appData.profiles || state.appData.profiles.length === 0) {
    tbody.innerHTML = '<tr><td colspan="5" style="text-align: center; color: var(--text-muted);">No VPN profiles configured.</td></tr>';
    return;
  }

  state.appData.profiles.forEach(p => {
    const provider = state.appData.providers.find(prov => prov.id === p.providerId);
    const providerName = provider ? provider.name : 'Unknown';
    
    const tr = document.createElement('tr');
    tr.innerHTML = `
      <td><strong>${p.name}</strong></td>
      <td>${providerName}</td>
      <td style="font-family: var(--font-mono); font-size: 0.8rem; color: var(--primary);">${p.username}</td>
      <td>${p.autoConnect ? 'Yes' : 'No'}</td>
      <td>
        <div class="card-action-group">
          <button class="card-icon-btn edit-prof" data-id="${p.id}" title="Edit Profile">
            <svg viewBox="0 0 24 24" width="14" height="14"><path fill="currentColor" d="M3 17.25V21h3.75L17.81 9.94l-3.75-3.75L3 17.25z"/></svg>
          </button>
          <button class="card-icon-btn delete delete-prof" data-id="${p.id}" title="Delete Profile">
            <svg viewBox="0 0 24 24" width="14" height="14"><path fill="currentColor" d="M6 19c0 1.1.9 2 2 2h8c1.1 0 2-.9 2-2V7H6v12z"/></svg>
          </button>
        </div>
      </td>
    `;
    tbody.appendChild(tr);
  });

  document.querySelectorAll('.edit-prof').forEach(btn => {
    btn.addEventListener('click', () => editProfile(btn.getAttribute('data-id')));
  });

  document.querySelectorAll('.delete-prof').forEach(btn => {
    btn.addEventListener('click', () => deleteProfile(btn.getAttribute('data-id')));
  });
}

export function renderLogsHistory() {
  const tbody = document.getElementById('logs-table-body');
  if (!tbody) return;
  tbody.innerHTML = '';

  if (!state.appData.logs || state.appData.logs.length === 0) {
    tbody.innerHTML = '<tr><td colspan="7" style="text-align: center; color: var(--text-muted);">No connection logs recorded.</td></tr>';
    return;
  }

  state.appData.logs.forEach(l => {
    const isSuccess = l.message.toLowerCase().includes('success') || l.message.toLowerCase().includes('closed gracefully');
    const statusText = isSuccess ? 'Success' : 'Error';
    const statusClass = isSuccess ? 'badge-connected' : 'badge-disconnecting';

    const tr = document.createElement('tr');
    tr.innerHTML = `
      <td><strong>${l.profileName}</strong></td>
      <td>${l.providerName}</td>
      <td style="font-family: var(--font-mono); font-size: 0.75rem;">${l.username}</td>
      <td style="font-size: 0.78rem;">${new Date(l.startTime).toLocaleString()}</td>
      <td style="font-size: 0.78rem;">${l.endTime ? new Date(l.endTime).toLocaleString() : 'Active'}</td>
      <td style="font-family: var(--font-mono); font-size: 0.8rem;">${l.duration || '00:00:00'}</td>
      <td><span class="badge ${statusClass}">${statusText}</span></td>
    `;
    
    tr.setAttribute('title', l.message);
    tbody.appendChild(tr);
  });
}

export function syncGeneralSettingsForm() {
  const startupSetting = document.getElementById('setting-startup');
  if (startupSetting) startupSetting.checked = state.appData.settings.autoConnectOnStartup || false;

  const traySetting = document.getElementById('setting-tray');
  if (traySetting) traySetting.checked = state.appData.settings.minimizeToTray !== false;
  
  const themeSwitch = document.getElementById('setting-theme');
  if (themeSwitch) {
    themeSwitch.checked = (state.appData.settings.theme !== 'light');
  }
  
  const openconnectPathInput = document.getElementById('setting-openconnect-path');
  if (openconnectPathInput) {
    openconnectPathInput.value = state.appData.settings.openconnectPath || 'openconnect.exe';
  }
  const openvpnPathInput = document.getElementById('setting-openvpn-path');
  if (openvpnPathInput) {
    openvpnPathInput.value = state.appData.settings.openvpnPath || 'openvpn.exe';
  }
}
