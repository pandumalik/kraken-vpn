const { app, BrowserWindow } = require('electron');
const path = require('path');
const fs = require('fs');

const dataPath = path.join(app.getPath('userData'), 'vpn_storage.json');
const logsPath = path.join(app.getPath('userData'), 'vpn_logs.json');

function loadData() {
  if (!fs.existsSync(dataPath)) {
    const defaultData = {
      providers: [
        {
          id: 'prov-victoria',
          name: 'VictoriaBank VPN',
          server: 'vpn3.victoriabank.co.id',
          port: 8888,
          protocol: 'Fortinet SSL',
          authMethod: 'Credentials'
        }
      ],
      profiles: [
        {
          id: 'prof-victoria',
          name: 'VictoriaBank Malik',
          providerId: 'prov-victoria',
          username: 'malik.ist',
          password: 'password123',
          autoConnect: false
        }
      ],
      settings: {
        autoConnectOnStartup: false,
        minimizeToTray: true,
        theme: 'dark',
        openconnectPath: 'openconnect.exe',
        openvpnPath: 'C:\\Program Files\\OpenVPN\\bin\\openvpn.exe'
      }
    };
    fs.mkdirSync(path.dirname(dataPath), { recursive: true });
    fs.writeFileSync(dataPath, JSON.stringify(defaultData, null, 2), 'utf8');
    return defaultData;
  }
  try {
    const data = JSON.parse(fs.readFileSync(dataPath, 'utf8'));
    if (data.settings) {
      let migrated = false;
      if (data.settings.fortinetPath) {
        delete data.settings.fortinetPath;
        migrated = true;
      }
      if (!data.settings.openconnectPath) {
        data.settings.openconnectPath = 'openconnect.exe';
        migrated = true;
      }
      if (!data.settings.openvpnPath) {
        data.settings.openvpnPath = 'C:\\Program Files\\OpenVPN\\bin\\openvpn.exe';
        migrated = true;
      }
      if (migrated) {
        fs.writeFileSync(dataPath, JSON.stringify(data, null, 2), 'utf8');
      }
    }
    return data;
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
  logs.unshift(logEntry);
  saveLogs(logs);
  const windows = BrowserWindow.getAllWindows();
  if (windows.length > 0) {
    windows[0].webContents.send('vpn-logs-updated', logs);
  }
}

module.exports = {
  loadData,
  saveData,
  loadLogs,
  saveLogs,
  appendLog
};
