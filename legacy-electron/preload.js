const { contextBridge, ipcRenderer } = require('electron');

contextBridge.exposeInMainWorld('electronAPI', {
  // Window management
  minimize: () => ipcRenderer.send('window-minimize'),
  maximize: () => ipcRenderer.send('window-maximize'),
  close: () => ipcRenderer.send('window-close'),

  // CRUD & Settings API
  loadAppData: () => ipcRenderer.invoke('load-app-data'),
  saveProvider: (provider) => ipcRenderer.invoke('save-provider', provider),
  deleteProvider: (providerId) => ipcRenderer.invoke('delete-provider', providerId),
  saveProfile: (profile) => ipcRenderer.invoke('save-profile', profile),
  deleteProfile: (profileId) => ipcRenderer.invoke('delete-profile', profileId),
  saveSettings: (settings) => ipcRenderer.invoke('save-settings', settings),
  clearConnectionTelemetry: () => ipcRenderer.invoke('clear-connection-telemetry'),
  clearLogs: () => ipcRenderer.invoke('clear-logs'),
  requestAdminPrivileges: () => ipcRenderer.invoke('request-admin-privileges'),
  selectOpenConnectPath: () => ipcRenderer.invoke('select-openconnect-path'),
  selectOpenVPNPath: () => ipcRenderer.invoke('select-openvpn-path'),
  testVPNConnection: (config) => ipcRenderer.invoke('test-vpn-connection', config),

  // VPN connection controllers
  connectVPN: (profileId) => ipcRenderer.invoke('connect-vpn', profileId),
  disconnectVPN: (profileId) => ipcRenderer.invoke('disconnect-vpn', profileId),

  // Events listeners from Main process
  onStatusChanged: (callback) => {
    const listener = (event, data) => callback(data);
    ipcRenderer.on('vpn-status-changed', listener);
    return () => ipcRenderer.removeListener('vpn-status-changed', listener);
  },
  onTelemetryUpdate: (callback) => {
    const listener = (event, data) => callback(data);
    ipcRenderer.on('vpn-telemetry-update', listener);
    return () => ipcRenderer.removeListener('vpn-telemetry-update', listener);
  },
  onLogOutput: (callback) => {
    const listener = (event, data) => callback(data);
    ipcRenderer.on('vpn-log-output', listener);
    return () => ipcRenderer.removeListener('vpn-log-output', listener);
  },
  onLogsUpdated: (callback) => {
    const listener = (event, data) => callback(data);
    ipcRenderer.on('vpn-logs-updated', listener);
    return () => ipcRenderer.removeListener('vpn-logs-updated', listener);
  }
});
