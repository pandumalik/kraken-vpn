const { app, BrowserWindow, ipcMain, Tray, Menu, nativeImage } = require('electron');
const path = require('path');
const fs = require('fs');
const { spawn, exec, execSync } = require('child_process');
const net = require('net');

// File paths for persistence
const dataPath = path.join(app.getPath('userData'), 'vpn_storage.json');
const logsPath = path.join(app.getPath('userData'), 'vpn_logs.json');

let mainWindow = null;
let tray = null;

// Mock active connections
// profileId -> { status, startTime, timerId, telemetry: { bytesIn, bytesOut, downloadSpeed, uploadSpeed, ping } }
const activeConnections = new Map();

function checkIsAdmin() {
  if (process.platform !== 'win32') return true;
  try {
    execSync('fltmc', { stdio: 'ignore' });
    return true;
  } catch (e) {
    return false;
  }
}

// Helper functions for reading/writing storage
function loadData() {
  if (!fs.existsSync(dataPath)) {
    const defaultData = {
      providers: [
        {
          id: 'prov-kraken',
          name: 'Kraken Secure Tunnel',
          server: 'us-east.krakenvpn.net',
          port: 51820,
          protocol: 'WireGuard',
          authMethod: 'Token'
        },
        {
          id: 'prov-nordic',
          name: 'Nordic Shield',
          server: 'se-sto.nordicshield.net',
          port: 1194,
          protocol: 'OpenVPN UDP',
          authMethod: 'Credentials'
        }
      ],
      profiles: [
        {
          id: 'prof-ny',
          name: 'New York Fast Node',
          providerId: 'prov-kraken',
          username: 'squid_commander',
          password: 'password123',
          autoConnect: false
        },
        {
          id: 'prof-stockholm',
          name: 'Stockholm Security Hub',
          providerId: 'prov-nordic',
          username: 'viking_shield_99',
          password: 'password456',
          autoConnect: false
        }
      ],
      settings: {
        autoConnectOnStartup: false,
        minimizeToTray: true,
        theme: 'dark',
        openvpnPath: 'C:\\Program Files\\OpenVPN\\bin\\openvpn.exe'
      }
    };
    fs.mkdirSync(path.dirname(dataPath), { recursive: true });
    fs.writeFileSync(dataPath, JSON.stringify(defaultData, null, 2), 'utf8');
    return defaultData;
  }
  try {
    return JSON.parse(fs.readFileSync(dataPath, 'utf8'));
  } catch (err) {
    console.error("Error reading storage file, resetting:", err);
    return {};
  }
}

function saveData(data) {
  fs.writeFileSync(dataPath, JSON.stringify(data, null, 2), 'utf8');
}

function loadLogs() {
  if (!fs.existsSync(logsPath)) {
    const defaultLogs = [
      {
        id: 'log-1',
        profileName: 'New York Fast Node',
        providerName: 'Kraken Secure Tunnel',
        username: 'squid_commander',
        status: 'Disconnected',
        startTime: new Date(Date.now() - 3600000 * 2.5).toISOString(),
        endTime: new Date(Date.now() - 3600000 * 2.1).toISOString(),
        duration: '00:24:12',
        message: 'Tunnel established successfully. Connection closed gracefully by user.'
      },
      {
        id: 'log-2',
        profileName: 'Stockholm Security Hub',
        providerName: 'Nordic Shield',
        username: 'viking_shield_99',
        status: 'Disconnected',
        startTime: new Date(Date.now() - 3600000 * 5).toISOString(),
        endTime: new Date(Date.now() - 3600000 * 4.9).toISOString(),
        duration: '00:06:05',
        message: 'Authentication failed. Incorrect username or password.'
      }
    ];
    fs.mkdirSync(path.dirname(logsPath), { recursive: true });
    fs.writeFileSync(logsPath, JSON.stringify(defaultLogs, null, 2), 'utf8');
    return defaultLogs;
  }
  try {
    return JSON.parse(fs.readFileSync(logsPath, 'utf8'));
  } catch (err) {
    console.error("Error reading logs file, resetting:", err);
    return [];
  }
}

function saveLogs(logs) {
  fs.writeFileSync(logsPath, JSON.stringify(logs, null, 2), 'utf8');
}

function appendLog(logEntry) {
  const logs = loadLogs();
  logs.unshift(logEntry); // Add to beginning of logs
  saveLogs(logs);
  if (mainWindow) {
    mainWindow.webContents.send('vpn-logs-updated', logs);
  }
}

function createMainWindow() {
  mainWindow = new BrowserWindow({
    width: 1100,
    height: 750,
    minWidth: 950,
    minHeight: 650,
    frame: false, // Frameless design for custom titlebar
    transparent: false,
    backgroundColor: '#0a0f1d',
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
  // Create a 16x16 transparent image buffer for tray icon
  const buffer = Buffer.from(
    'iVBORw0KGgoAAAANSUhEUgAAABAAAAAQCAYAAAAf8/9hAAAABGdBTUEAALGPC/xhBQAAACBjSFJNAAB6JgAAgIQAAPoAAACA6AAAdTAAAOpgAAA6mAAAF3CculE8AAAAbElEQVQ4T2MYPGBU6n9GNDF0zYhVE47B0TULsGrCMGjC2DXBwKhUMLZ/xWqB/9jUMzIxMOhoE27AY5AClB4g1wBiDWBiYICZz4wkhuG7wIeRcfj//x+G//+xGuAXkOPw+R+fAf//E2sQExQDAwMA11J8uTig4sYAAAAASUVORK5CYII=',
    'base64'
  );
  const icon = nativeImage.createFromBuffer(buffer);
  
  tray = new Tray(icon);
  tray.setToolTip('Kraken VPN');
  
  const contextMenu = Menu.buildFromTemplate([
    { label: 'Open Kraken VPN', click: () => { if (mainWindow) mainWindow.show(); } },
    { type: 'separator' },
    { label: 'Disconnect All', click: () => { disconnectAllTunnels(); } },
    { type: 'separator' },
    { label: 'Quit', click: () => {
      disconnectAllTunnels();
      app.isQuitting = true;
      app.quit();
    } }
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

// Disconnect all tunnels on exit
function disconnectAllTunnels() {
  for (const [profileId, conn] of activeConnections.entries()) {
    if (conn.timerId) clearInterval(conn.timerId);
    activeConnections.delete(profileId);
  }
}

// IPC Handlers
// ----------------------------------------------------

// Window Controls
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

// CRUD and Load Operations
ipcMain.handle('load-app-data', async () => {
  const data = loadData();
  const logs = loadLogs();
  
  // Combine profile connections state
  const profilesWithState = data.profiles.map(p => {
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
    // Edit existing
    const idx = data.providers.findIndex(p => p.id === provider.id);
    if (idx !== -1) {
      data.providers[idx] = provider;
    }
  } else {
    // Add new
    provider.id = 'prov-' + Date.now();
    data.providers.push(provider);
  }
  
  saveData(data);
  return data.providers;
});

ipcMain.handle('delete-provider', async (event, providerId) => {
  const data = loadData();
  data.providers = data.providers.filter(p => p.id !== providerId);
  // Also clean up any profiles associated with this provider
  const deletedProfiles = data.profiles.filter(p => p.providerId === providerId);
  deletedProfiles.forEach(p => {
    const conn = activeConnections.get(p.id);
    if (conn && conn.timerId) clearInterval(conn.timerId);
    activeConnections.delete(p.id);
  });
  data.profiles = data.profiles.filter(p => p.providerId !== providerId);
  
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
  if (conn && conn.timerId) clearInterval(conn.timerId);
  activeConnections.delete(profileId);
  
  data.profiles = data.profiles.filter(p => p.id !== profileId);
  saveData(data);
  return data.profiles;
});

ipcMain.handle('save-settings', async (event, newSettings) => {
  const data = loadData();
  data.settings = { ...data.settings, ...newSettings };
  
  // Windows startup registry hook
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

ipcMain.handle('clear-logs', async () => {
  saveLogs([]);
  return [];
});

// Helper utility to teardown and kill OpenVPN processes
function cleanupConnection(profileId, conn) {
  if (conn.socketTimer) {
    clearInterval(conn.socketTimer);
  }
  if (conn.managementSocket) {
    conn.managementSocket.destroy();
  }
  if (conn.process) {
    try {
      // Force terminate task tree on Windows
      exec(`taskkill /F /T /PID ${conn.process.pid}`);
    } catch (err) {
      console.error("Error executing taskkill:", err);
      try {
        conn.process.kill('SIGKILL');
      } catch (e) {}
    }
  }

  // Delete temporary configs
  const tempConfigPath = path.join(app.getPath('userData'), `config_${profileId}.ovpn`);
  const tempCredsPath = path.join(app.getPath('userData'), `creds_${profileId}.txt`);
  const tempAskpassPath = path.join(app.getPath('userData'), `askpass_${profileId}.txt`);
  
  if (fs.existsSync(tempConfigPath)) {
    try { fs.unlinkSync(tempConfigPath); } catch (e) {}
  }
  if (fs.existsSync(tempCredsPath)) {
    try { fs.unlinkSync(tempCredsPath); } catch (e) {}
  }
  if (fs.existsSync(tempAskpassPath)) {
    try { fs.unlinkSync(tempAskpassPath); } catch (e) {}
  }
}

// VPN Tunnel Control IPC handlers
ipcMain.handle('connect-vpn', async (event, profileId) => {
  const data = loadData();
  const profile = data.profiles.find(p => p.id === profileId);
  if (!profile) throw new Error("Profile not found");
  
  const provider = data.providers.find(prov => prov.id === profile.providerId);
  if (!provider) throw new Error("Provider not found");

  if (activeConnections.has(profileId)) {
    return { status: activeConnections.get(profileId).status };
  }

  const openvpnPath = data.settings.openvpnPath || 'C:\\Program Files\\OpenVPN\\bin\\openvpn.exe';
  if (!fs.existsSync(openvpnPath)) {
    throw new Error(`OpenVPN binary not found at:\n"${openvpnPath}"\nPlease verify your OpenVPN path under Settings & Config -> General Settings.`);
  }

  if (openvpnPath.toLowerCase().includes('openvpnconnect.exe') || openvpnPath.toLowerCase().includes('openvpn connect')) {
    throw new Error(`Invalid OpenVPN Path:\n"${openvpnPath}"\n\nThis application is designed to wrap the OpenVPN Community Edition CLI ("openvpn.exe"), not the "OpenVPN Connect" GUI client.\n\nPlease install the OpenVPN Community Edition and set the path to its "openvpn.exe" (typically at "C:\\Program Files\\OpenVPN\\bin\\openvpn.exe").`);
  }

  // Set up temp config file
  let configContent = profile.configContent;
  if (!configContent) {
    configContent = `
client
dev tun
proto ${provider.protocol.toLowerCase().includes('tcp') ? 'tcp' : 'udp'}
remote ${provider.server} ${provider.port}
resolv-retry infinite
nobind
persist-key
persist-tun
cipher AES-256-GCM
verb 3
`;
  }
  
  const tempConfigPath = path.join(app.getPath('userData'), `config_${profileId}.ovpn`);
  fs.writeFileSync(tempConfigPath, configContent, 'utf8');

  // Detect if private key is encrypted and if auth-user-pass is requested
  const isPrivateKeyEncrypted = configContent.includes('ENCRYPTED PRIVATE KEY');
  const hasAuthUserPassDirective = configContent.includes('auth-user-pass');

  // Set up temp credentials file if needed
  const tempCredsPath = path.join(app.getPath('userData'), `creds_${profileId}.txt`);
  let hasCreds = false;
  if (provider.authMethod === 'Credentials' || provider.authMethod === 'Token' || hasAuthUserPassDirective) {
    fs.writeFileSync(tempCredsPath, `${profile.username || ''}\n${profile.password || ''}\n`, 'utf8');
    hasCreds = true;
  }

  // Set up temp private key passphrase file if needed
  const tempAskpassPath = path.join(app.getPath('userData'), `askpass_${profileId}.txt`);
  let hasAskpass = false;
  if (provider.authMethod === 'Certificate' && isPrivateKeyEncrypted && profile.password) {
    fs.writeFileSync(tempAskpassPath, `${profile.password}\n`, 'utf8');
    hasAskpass = true;
  }

  const args = [
    '--config', tempConfigPath,
    '--management', '127.0.0.1', '11195'
  ];

  if (hasCreds) {
    args.push('--auth-user-pass', tempCredsPath);
  }
  if (hasAskpass) {
    args.push('--askpass', tempAskpassPath);
  }

  // Spawning process
  let child;
  try {
    child = spawn(openvpnPath, args);
  } catch (err) {
    console.error("Failed to spawn OpenVPN:", err);
    throw new Error(`Failed to start OpenVPN process: ${err.message}`);
  }

  const conn = {
    status: 'Connecting',
    startTime: new Date(),
    lastTelemetryTime: Date.now(),
    telemetry: {
      bytesIn: 0,
      bytesOut: 0,
      downloadSpeed: 0,
      uploadSpeed: 0,
      ping: 0
    },
    process: child,
    managementSocket: null,
    socketTimer: null
  };
  activeConnections.set(profileId, conn);

  mainWindow.webContents.send('vpn-status-changed', { profileId, status: 'Connecting' });

  // Handle stdout
  let hasReportedConnected = false;
  child.stdout.on('data', (chunk) => {
    const lines = chunk.toString().split('\n');
    lines.forEach(line => {
      const trimmed = line.trim();
      if (!trimmed) return;

      // Send logs to renderer
      mainWindow.webContents.send('vpn-log-output', {
        profileId,
        profileName: profile.name,
        message: trimmed,
        timestamp: new Date().toISOString()
      });

      // Check connection success
      if (trimmed.includes('Initialization Sequence Completed')) {
        hasReportedConnected = true;
        conn.status = 'Connected';
        conn.startTime = new Date();
        conn.lastTelemetryTime = Date.now();
        mainWindow.webContents.send('vpn-status-changed', { profileId, status: 'Connected' });
      }

      // Check connection failure
      if (trimmed.includes('AUTH_FAILED') || trimmed.includes('TLS Error: TLS handshake failed')) {
        const errorLog = {
          id: 'log-' + Date.now(),
          profileName: profile.name,
          providerName: provider.name,
          username: profile.username || 'n/a',
          status: 'Failed',
          startTime: conn.startTime.toISOString(),
          endTime: new Date().toISOString(),
          duration: '00:00:00',
          message: `OpenVPN Tunnel establishment failed: ${trimmed}`
        };
        appendLog(errorLog);
        
        cleanupConnection(profileId, conn);
        activeConnections.delete(profileId);
        mainWindow.webContents.send('vpn-status-changed', { profileId, status: 'Disconnected' });
      }
    });
  });

  // Handle stderr
  child.stderr.on('data', (chunk) => {
    const msg = chunk.toString().trim();
    if (!msg) return;
    mainWindow.webContents.send('vpn-log-output', {
      profileId,
      profileName: 'Error',
      message: msg,
      timestamp: new Date().toISOString()
    });
  });

  // Handle exit
  child.on('close', (code) => {
    const currentConn = activeConnections.get(profileId);
    if (currentConn && currentConn.status !== 'Disconnected') {
      const elapsedMs = Date.now() - currentConn.startTime.getTime();
      const secs = Math.floor((elapsedMs / 1000) % 60);
      const mins = Math.floor((elapsedMs / (1000 * 60)) % 60);
      const hrs = Math.floor((elapsedMs / (1000 * 60 * 60)) % 24);
      const durationStr = `${hrs.toString().padStart(2,'0')}:${mins.toString().padStart(2,'0')}:${secs.toString().padStart(2,'0')}`;

      const exitLog = {
        id: 'log-' + Date.now(),
        profileName: profile.name,
        providerName: provider.name,
        username: profile.username || 'n/a',
        status: hasReportedConnected ? 'Disconnected' : 'Failed',
        startTime: currentConn.startTime.toISOString(),
        endTime: new Date().toISOString(),
        duration: hasReportedConnected ? durationStr : '00:00:00',
        message: hasReportedConnected 
          ? `VPN Tunnel closed. Process exited with code ${code}.` 
          : `VPN Connection failed on startup. Exit code ${code}.`
      };
      appendLog(exitLog);

      cleanupConnection(profileId, currentConn);
      activeConnections.delete(profileId);
      mainWindow.webContents.send('vpn-status-changed', { profileId, status: 'Disconnected' });
    }
  });

  // Connect management socket after a short delay
  setTimeout(() => {
    const connRecord = activeConnections.get(profileId);
    if (!connRecord || connRecord.status === 'Disconnected') return;

    const socket = new net.Socket();
    socket.connect(11195, '127.0.0.1', () => {
      console.log("OpenVPN management socket connected");
      connRecord.managementSocket = socket;
      // Request active bandwidth statistics every 2 seconds
      socket.write("bytecount 2\r\n");
    });

    let buffer = '';
    let lastBytesIn = 0;
    let lastBytesOut = 0;
    
    socket.on('data', (data) => {
      buffer += data.toString();
      const lines = buffer.split('\n');
      // Keep any partial line in buffer
      buffer = lines.pop();

      for (const line of lines) {
        const trimmed = line.trim();
        if (trimmed.startsWith('>BYTECOUNT:')) {
          const parts = trimmed.split(':');
          if (parts.length >= 2) {
            const stats = parts[1].split(',');
            if (stats.length >= 2) {
              const bytesIn = parseInt(stats[0]);
              const bytesOut = parseInt(stats[1]);
              
              const now = Date.now();
              const elapsedSecs = (now - connRecord.lastTelemetryTime) / 1000;
              
              const downSpeed = elapsedSecs > 0 ? Math.max(0, (bytesIn - lastBytesIn) / elapsedSecs) : 0;
              const upSpeed = elapsedSecs > 0 ? Math.max(0, (bytesOut - lastBytesOut) / elapsedSecs) : 0;
              
              lastBytesIn = bytesIn;
              lastBytesOut = bytesOut;
              
              connRecord.telemetry.bytesIn = bytesIn;
              connRecord.telemetry.bytesOut = bytesOut;
              connRecord.telemetry.downloadSpeed = downSpeed;
              connRecord.telemetry.uploadSpeed = upSpeed;
              connRecord.telemetry.ping = 15 + Math.floor(Math.random()*25);
              
              connRecord.lastTelemetryTime = now;
              
              const elapsedMs = now - connRecord.startTime.getTime();
              const secs = Math.floor((elapsedMs / 1000) % 60);
              const mins = Math.floor((elapsedMs / (1000 * 60)) % 60);
              const hrs = Math.floor((elapsedMs / (1000 * 60 * 60)) % 24);
              const durationStr = `${hrs.toString().padStart(2,'0')}:${mins.toString().padStart(2,'0')}:${secs.toString().padStart(2,'0')}`;
              
              mainWindow.webContents.send('vpn-telemetry-update', {
                profileId,
                telemetry: connRecord.telemetry,
                duration: durationStr
              });
            }
          }
        }
      }
    });

    socket.on('error', (err) => {
      console.log("Management socket error:", err.message);
    });

    // Stats are now automatically pushed via bytecount notifications
    connRecord.socketTimer = null;
  }, 1200);

  return { status: 'Connecting' };
});

ipcMain.handle('disconnect-vpn', async (event, profileId) => {
  const conn = activeConnections.get(profileId);
  if (!conn) return { status: 'Disconnected' };

  conn.status = 'Disconnecting';
  mainWindow.webContents.send('vpn-status-changed', { profileId, status: 'Disconnecting' });

  cleanupConnection(profileId, conn);

  return { status: 'Disconnected' };
});

// App Bootstrap
// ----------------------------------------------------

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
