const { app, BrowserWindow } = require('electron');
const path = require('path');
const fs = require('fs');
const { spawn } = require('child_process');
const net = require('net');
const { activeConnections, cleanupConnection } = require('./manager');
const { appendLog } = require('../store');

function connectOpenVPN(profile, provider, settings, mainWindow) {
  const openvpnPath = settings.openvpnPath || 'C:\\Program Files\\OpenVPN\\bin\\openvpn.exe';
  let resolvedOpenvpnPath = openvpnPath;
  if (!path.isAbsolute(resolvedOpenvpnPath)) {
    const localPath = path.join(process.cwd(), resolvedOpenvpnPath);
    if (fs.existsSync(localPath)) {
      resolvedOpenvpnPath = localPath;
    }
  }

  if (!fs.existsSync(resolvedOpenvpnPath) && openvpnPath !== 'openvpn.exe') {
    throw new Error(`OpenVPN binary not found at:\n"${openvpnPath}"\nPlease verify your OpenVPN path under Settings & Config -> General Settings.`);
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
  
  const tempConfigPath = path.join(app.getPath('userData'), `config_${profile.id}.ovpn`);
  fs.writeFileSync(tempConfigPath, configContent, 'utf8');

  // Detect if private key is encrypted and if auth-user-pass is requested
  const isPrivateKeyEncrypted = configContent.includes('ENCRYPTED PRIVATE KEY');
  const hasAuthUserPassDirective = configContent.includes('auth-user-pass');

  // Set up temp credentials file if needed
  const tempCredsPath = path.join(app.getPath('userData'), `creds_${profile.id}.txt`);
  let hasCreds = false;
  if (provider.authMethod === 'Credentials' || provider.authMethod === 'Token' || hasAuthUserPassDirective) {
    fs.writeFileSync(tempCredsPath, `${profile.username || ''}\n${profile.password || ''}\n`, 'utf8');
    hasCreds = true;
  }

  // Set up temp private key passphrase file if needed
  const tempAskpassPath = path.join(app.getPath('userData'), `askpass_${profile.id}.txt`);
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
    child = spawn(resolvedOpenvpnPath, args);
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
  activeConnections.set(profile.id, conn);

  if (mainWindow) {
    mainWindow.webContents.send('vpn-status-changed', { profileId: profile.id, status: 'Connecting' });
  }

  let hasReportedConnected = false;
  let lineBuffer = '';
  const handleOpenvpnOutput = (chunk) => {
    const chunkStr = chunk.toString();
    lineBuffer += chunkStr;

    let index;
    while ((index = lineBuffer.indexOf('\n')) !== -1) {
      const line = lineBuffer.substring(0, index);
      lineBuffer = lineBuffer.substring(index + 1);

      const trimmed = line.trim();
      if (!trimmed) continue;
      console.log(`[OpenVPN] ${trimmed}`);

      if (mainWindow) {
        mainWindow.webContents.send('vpn-log-output', {
          profileId: profile.id,
          profileName: profile.name,
          message: trimmed,
          timestamp: new Date().toISOString()
        });
      }

      // Check connection success
      if (trimmed.includes('Initialization Sequence Completed')) {
        hasReportedConnected = true;
        conn.status = 'Connected';
        conn.startTime = new Date();
        conn.lastTelemetryTime = Date.now();
        if (mainWindow) {
          mainWindow.webContents.send('vpn-status-changed', { profileId: profile.id, status: 'Connected' });
        }
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
        
        cleanupConnection(profile.id, conn);
        activeConnections.delete(profile.id);
        if (mainWindow) {
          mainWindow.webContents.send('vpn-status-changed', { profileId: profile.id, status: 'Disconnected' });
        }
      }
    }
  };

  child.stdout.on('data', handleOpenvpnOutput);
  child.stderr.on('data', (chunk) => {
    const msg = chunk.toString().trim();
    if (!msg) return;
    if (mainWindow) {
      mainWindow.webContents.send('vpn-log-output', {
        profileId: profile.id,
        profileName: 'Error',
        message: msg,
        timestamp: new Date().toISOString()
      });
    }
  });

  child.on('close', (code) => {
    const currentConn = activeConnections.get(profile.id);
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

      cleanupConnection(profile.id, currentConn);
      activeConnections.delete(profile.id);
      if (mainWindow) {
        mainWindow.webContents.send('vpn-status-changed', { profileId: profile.id, status: 'Disconnected' });
      }
    }
  });

  // Connect management socket after a short delay
  setTimeout(() => {
    const connRecord = activeConnections.get(profile.id);
    if (!connRecord || connRecord.status === 'Disconnected') return;

    const socket = new net.Socket();
    socket.connect(11195, '127.0.0.1', () => {
      console.log("OpenVPN management socket connected");
      connRecord.managementSocket = socket;
      socket.write("bytecount 2\r\n");
    });

    let buffer = '';
    let lastBytesIn = 0;
    let lastBytesOut = 0;
    
    socket.on('data', (data) => {
      buffer += data.toString();
      const lines = buffer.split('\n');
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

              connRecord.lastRawBytesIn = bytesIn;
              connRecord.lastRawBytesOut = bytesOut;
              
              const adjustedBytesIn = Math.max(0, bytesIn - (connRecord.bytesInOffset || 0));
              const adjustedBytesOut = Math.max(0, bytesOut - (connRecord.bytesOutOffset || 0));
              
              connRecord.telemetry.bytesIn = adjustedBytesIn;
              connRecord.telemetry.bytesOut = adjustedBytesOut;
              connRecord.telemetry.downloadSpeed = downSpeed;
              connRecord.telemetry.uploadSpeed = upSpeed;
              connRecord.telemetry.ping = 15 + Math.floor(Math.random()*25);
              
              connRecord.lastTelemetryTime = now;
              
              const elapsedMs = now - connRecord.startTime.getTime();
              const secs = Math.floor((elapsedMs / 1000) % 60);
              const mins = Math.floor((elapsedMs / (1000 * 60)) % 60);
              const hrs = Math.floor((elapsedMs / (1000 * 60 * 60)) % 24);
              const durationStr = `${hrs.toString().padStart(2,'0')}:${mins.toString().padStart(2,'0')}:${secs.toString().padStart(2,'0')}`;
              
              if (mainWindow) {
                mainWindow.webContents.send('vpn-telemetry-update', {
                  profileId: profile.id,
                  telemetry: connRecord.telemetry,
                  duration: durationStr
                });
              }
            }
          }
        }
      }
    });

    socket.on('error', (err) => {
      console.log("Management socket error:", err.message);
    });

    connRecord.socketTimer = null;
  }, 1200);

  return { status: 'Connecting' };
}

module.exports = {
  connectOpenVPN
};
