const { app, BrowserWindow } = require('electron');
const path = require('path');
const fs = require('fs');
const { exec } = require('child_process');

const activeConnections = new Map();

function cleanupConnection(profileId, conn) {
  if (conn.socketTimer) {
    clearInterval(conn.socketTimer);
  }
  if (conn.managementSocket) {
    try { conn.managementSocket.destroy(); } catch (e) {}
  }

  if (conn.process) {
    try {
      conn.process.kill('SIGKILL');
    } catch (e) {}

    try {
      const pid = conn.process.pid;
      exec(`taskkill /F /T /PID ${pid}`, { stdio: 'ignore' }, (err) => {
        // Ignore errors
      });
    } catch (err) {
      console.error("Error executing taskkill:", err);
    }
  }

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

function disconnectAllTunnels() {
  const windows = BrowserWindow.getAllWindows();
  for (const [profileId, conn] of activeConnections.entries()) {
    if (conn.timerId) clearInterval(conn.timerId);
    cleanupConnection(profileId, conn);
    activeConnections.delete(profileId);
    
    windows.forEach(win => {
      try {
        win.webContents.send('vpn-status-changed', { profileId, status: 'Disconnected' });
      } catch (e) {}
    });
  }
}

module.exports = {
  activeConnections,
  cleanupConnection,
  disconnectAllTunnels
};
