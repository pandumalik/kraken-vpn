const { app, BrowserWindow, ipcMain, Tray, Menu, nativeImage, dialog } = require('electron');
const path = require('path');
const { exec } = require('child_process');

// Import modular helper sub-modules
const {
  loadData,
  saveData,
  loadLogs,
  saveLogs,
  appendLog
} = require('./src/main/store');

const { checkIsAdmin } = require('./src/main/utils');

const {
  activeConnections,
  cleanupConnection,
  disconnectAllTunnels
} = require('./src/main/vpn/manager');

const { connectOpenVPN } = require('./src/main/vpn/openvpn');
const { connectOpenConnect } = require('./src/main/vpn/openconnect');
const { testOpenConnect } = require('./src/main/vpn/test');

let mainWindow = null;
let tray = null;

function createMainWindow() {
  mainWindow = new BrowserWindow({
    width: 1100,
    height: 750,
    minWidth: 950,
    minHeight: 650,
    frame: false, // Frameless design for custom titlebar
    transparent: false,
    backgroundColor: '#0a0f1d',
    icon: path.join(__dirname, 'icon.png'),
    webPreferences: {
      preload: path.join(__dirname, 'preload.js'),
      contextIsolation: true,
      nodeIntegration: false,
      sandbox: true
    },
    show: false
  });

  mainWindow.loadFile(path.join(__dirname, 'src', 'index.html'));

  mainWindow.once('ready-to-show', () => {
    mainWindow.show();
  });

  mainWindow.on('closed', () => {
    mainWindow = null;
  });
}

function createTray() {
  const iconPath = path.join(__dirname, 'icon.png');
  let icon = nativeImage.createFromPath(iconPath);
  icon = icon.resize({ width: 16, height: 16 });

  tray = new Tray(icon);
  tray.setToolTip('Kraken VPN');

  const contextMenu = Menu.buildFromTemplate([
    { label: 'Open Kraken VPN', click: () => { if (mainWindow) mainWindow.show(); } },
    { type: 'separator' },
    { label: 'Disconnect All', click: () => { disconnectAllTunnels(); } },
    { type: 'separator' },
    {
      label: 'Quit', click: () => {
        disconnectAllTunnels();
        app.isQuitting = true;
        app.quit();
      }
    }
  ]);

  tray.setContextMenu(contextMenu);
  tray.on('double-click', () => {
    if (mainWindow) {
      if (mainWindow.isVisible()) {
        mainWindow.focus();
      } else {
        mainWindow.show();
      }
    }
  });
}

// Window Controls Action Listeners
ipcMain.on('window-minimize', () => {
  if (mainWindow) mainWindow.minimize();
});

ipcMain.on('window-maximize', () => {
  if (mainWindow) {
    if (mainWindow.isMaximized()) {
      mainWindow.unmaximize();
    } else {
      mainWindow.maximize();
    }
  }
});

ipcMain.on('window-close', () => {
  const data = loadData();
  const minimizeToTray = data.settings ? data.settings.minimizeToTray : true;
  if (minimizeToTray && mainWindow) {
    mainWindow.hide();
  } else {
    disconnectAllTunnels();
    app.quit();
  }
});

// Register Electron IPC Handlers
ipcMain.handle('load-app-data', async () => {
  const data = loadData();
  const logs = loadLogs();

  const profilesWithState = (data.profiles || []).map(p => {
    const conn = activeConnections.get(p.id);
    return {
      ...p,
      status: conn ? conn.status : 'Disconnected',
      telemetry: conn ? conn.telemetry : null
    };
  });

  return {
    providers: data.providers || [],
    profiles: profilesWithState || [],
    settings: data.settings || {},
    logs: logs || [],
    isAdmin: checkIsAdmin()
  };
});

ipcMain.handle('save-provider', async (event, provider) => {
  const data = loadData();
  if (!data.providers) data.providers = [];

  if (provider.id) {
    const idx = data.providers.findIndex(p => p.id === provider.id);
    if (idx !== -1) {
      data.providers[idx] = provider;
    }
  } else {
    provider.id = 'prov-' + Date.now();
    data.providers.push(provider);
  }

  saveData(data);
  return data.providers;
});

ipcMain.handle('delete-provider', async (event, providerId) => {
  const data = loadData();
  data.providers = (data.providers || []).filter(p => p.id !== providerId);

  const deletedProfiles = (data.profiles || []).filter(p => p.providerId === providerId);
  deletedProfiles.forEach(p => {
    const conn = activeConnections.get(p.id);
    if (conn) {
      cleanupConnection(p.id, conn);
      activeConnections.delete(p.id);
    }
  });
  data.profiles = (data.profiles || []).filter(p => p.providerId !== providerId);

  saveData(data);
  return { providers: data.providers, profiles: data.profiles };
});

ipcMain.handle('save-profile', async (event, profile) => {
  const data = loadData();
  if (!data.profiles) data.profiles = [];

  if (profile.id) {
    const idx = data.profiles.findIndex(p => p.id === profile.id);
    if (idx !== -1) {
      data.profiles[idx] = { ...data.profiles[idx], ...profile };
    }
  } else {
    profile.id = 'prof-' + Date.now();
    profile.autoConnect = profile.autoConnect || false;
    data.profiles.push(profile);
  }

  saveData(data);
  return data.profiles;
});

ipcMain.handle('delete-profile', async (event, profileId) => {
  const data = loadData();
  const conn = activeConnections.get(profileId);
  if (conn) {
    cleanupConnection(profileId, conn);
    activeConnections.delete(profileId);
  }

  data.profiles = (data.profiles || []).filter(p => p.id !== profileId);
  saveData(data);
  return data.profiles;
});

ipcMain.handle('save-settings', async (event, newSettings) => {
  const data = loadData();
  data.settings = { ...data.settings, ...newSettings };

  if (process.platform === 'win32') {
    app.setLoginItemSettings({
      openAtLogin: data.settings.autoConnectOnStartup,
      path: process.execPath,
      args: ['--hidden']
    });
  }

  saveData(data);
  return data.settings;
});

ipcMain.handle('select-openconnect-path', async () => {
  if (!mainWindow) return null;
  const result = await dialog.showOpenDialog(mainWindow, {
    title: 'Select OpenConnect Executable',
    properties: ['openFile'],
    filters: [
      { name: 'Executables', extensions: ['exe'] },
      { name: 'All Files', extensions: ['*'] }
    ]
  });
  if (result.canceled || result.filePaths.length === 0) {
    return null;
  }
  return result.filePaths[0];
});

ipcMain.handle('select-openvpn-path', async () => {
  if (!mainWindow) return null;
  const result = await dialog.showOpenDialog(mainWindow, {
    title: 'Select OpenVPN Executable',
    properties: ['openFile'],
    filters: [
      { name: 'Executables', extensions: ['exe'] },
      { name: 'All Files', extensions: ['*'] }
    ]
  });
  if (result.canceled || result.filePaths.length === 0) {
    return null;
  }
  return result.filePaths[0];
});

ipcMain.handle('test-vpn-connection', async (event, config) => {
  const settings = loadData().settings || {};
  return testOpenConnect(config, settings);
});

ipcMain.handle('clear-logs', async () => {
  saveLogs([]);
  return [];
});

ipcMain.handle('clear-connection-telemetry', async () => {
  for (const conn of activeConnections.values()) {
    if (conn.telemetry) {
      conn.telemetry.bytesIn = 0;
      conn.telemetry.bytesOut = 0;
      conn.telemetry.downloadSpeed = 0;
      conn.telemetry.uploadSpeed = 0;
      if (conn.lastRawBytesIn !== undefined) {
        conn.bytesInOffset = conn.lastRawBytesIn;
      }
      if (conn.lastRawBytesOut !== undefined) {
        conn.bytesOutOffset = conn.lastRawBytesOut;
      }
    }
  }
  return { success: true };
});

ipcMain.handle('request-admin-privileges', async () => {
  if (process.platform !== 'win32') return true;
  if (checkIsAdmin()) return true;

  return new Promise((resolve, reject) => {
    const escapedExecPath = process.execPath.replace(/'/g, "''");
    const escapedArgs = process.argv.slice(1).map(arg => `'${arg.replace(/'/g, "''")}'`).join(', ');
    const cmd = `Start-Process -FilePath '${escapedExecPath}' -ArgumentList ${escapedArgs || "''"} -Verb RunAs`;

    exec(`powershell -Command "${cmd}"`, (error) => {
      if (error) {
        console.error("Failed to relaunch as admin:", error);
        reject(new Error("Administrator privilege request was cancelled or failed."));
      } else {
        app.quit();
        resolve(true);
      }
    });
  });
});

ipcMain.handle('connect-vpn', async (event, profileId) => {
  const data = loadData();
  const profile = data.profiles.find(p => p.id === profileId);
  if (!profile) throw new Error("Profile not found");

  const provider = data.providers.find(prov => prov.id === profile.providerId);
  if (!provider) throw new Error("Provider not found");

  if (activeConnections.has(profileId)) {
    return { status: activeConnections.get(profileId).status };
  }

  const lowerProto = provider.protocol.toLowerCase();
  const isOpenVPN = lowerProto.includes('openvpn');

  if (isOpenVPN) {
    return connectOpenVPN(profile, provider, data.settings || {}, mainWindow);
  } else {
    return connectOpenConnect(profile, provider, data.settings || {}, mainWindow);
  }
});

ipcMain.handle('disconnect-vpn', async (event, profileId) => {
  const conn = activeConnections.get(profileId);
  if (!conn) return { status: 'Disconnected' };

  conn.status = 'Disconnecting';
  if (mainWindow) {
    mainWindow.webContents.send('vpn-status-changed', { profileId, status: 'Disconnecting' });
  }

  cleanupConnection(profileId, conn);
  activeConnections.delete(profileId);

  return { status: 'Disconnected' };
});

// App Bootstrap
app.whenReady().then(() => {
  createMainWindow();
  createTray();

  app.on('activate', () => {
    if (BrowserWindow.getAllWindows().length === 0) {
      createMainWindow();
    }
  });
});

app.on('window-all-closed', () => {
  if (process.platform !== 'darwin') {
    disconnectAllTunnels();
    app.quit();
  }
});
