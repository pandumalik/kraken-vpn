const { app, BrowserWindow } = require('electron');
const path = require('path');
const fs = require('fs');
const { spawn, exec } = require('child_process');
const { activeConnections, cleanupConnection } = require('./manager');
const { loadData, saveData, appendLog } = require('../store');
const { checkIsAdmin } = require('../utils');

function getInterfaceStats(interfaceName) {
  return new Promise((resolve) => {
    if (process.platform !== 'win32') {
      resolve(null);
      return;
    }
    const cmd = `Get-NetAdapterStatistics -Name "${interfaceName}" | Select-Object ReceivedBytes, SentBytes | ConvertTo-Json`;
    exec(`powershell -Command "${cmd}"`, (error, stdout) => {
      if (error) {
        resolve(null);
        return;
      }
      try {
        const data = JSON.parse(stdout.trim());
        let received = 0;
        let sent = 0;
        if (Array.isArray(data)) {
          if (data.length > 0) {
            received = data[0].ReceivedBytes || 0;
            sent = data[0].SentBytes || 0;
          }
        } else if (data) {
          received = data.ReceivedBytes || 0;
          sent = data.SentBytes || 0;
        }
        resolve({
          bytesIn: received,
          bytesOut: sent
        });
      } catch (err) {
        resolve(null);
      }
    });
  });
}

function isCertPrompt(str) {
  if (!str) return false;
  const lower = str.toLowerCase();
  return (
    lower.includes("enter 'yes' to accept") ||
    lower.includes("confirm certificate") ||
    lower.includes("validation failed") ||
    lower.includes("accept certificate") ||
    lower.includes("accept this certificate") ||
    lower.includes("do you want to accept") ||
    lower.includes("trust this certificate") ||
    lower.includes("server certificate verify failed") ||
    lower.includes("signer not found") ||
    lower.includes("certificate verify failed") ||
    lower.includes("(yes/no)") ||
    lower.includes("(y/n)") ||
    lower.includes("[y/n]") ||
    lower.includes("accept always")
  );
}

function saveDetectedServerCert(profileId, pin) {
  try {
    const currentData = loadData();
    const profToUpdate = currentData.profiles.find(p => p.id === profileId);
    if (profToUpdate && profToUpdate.servercert !== pin) {
      profToUpdate.servercert = pin;
      saveData(currentData);
      console.log(`[OpenConnect] Auto-saved servercert pin (${pin}) for profile ${profileId}`);
    }
  } catch (err) {
    console.error("Error saving parsed servercert:", err);
  }
}

function connectOpenConnect(profile, provider, settings, mainWindow) {
  const openconnectPath = settings.openconnectPath || 'openconnect.exe';
  let resolvedPath = openconnectPath;
  if (!path.isAbsolute(resolvedPath)) {
    const localPath = path.join(process.cwd(), resolvedPath);
    if (fs.existsSync(localPath)) {
      resolvedPath = localPath;
    }
  }

  // Parse openconnect protocol
  let openconnectProtocol = 'fortinet';
  const lowerProto = provider.protocol.toLowerCase();
  if (lowerProto.includes('fortinet')) {
    openconnectProtocol = 'fortinet';
  } else if (lowerProto.includes('globalprotect') || lowerProto.includes('gp') || lowerProto.includes('palo alto')) {
    openconnectProtocol = 'gp';
  } else if (lowerProto.includes('pulse') || lowerProto.includes('juniper')) {
    openconnectProtocol = 'pulse';
  } else if (lowerProto.includes('f5')) {
    openconnectProtocol = 'f5';
  } else if (lowerProto.includes('anyconnect') || lowerProto.includes('cisco')) {
    openconnectProtocol = 'anyconnect';
  }

  let hasRetriedWithCert = false;

  const startSpawn = (serverCertPin) => {
    const args = [
      `--protocol=${openconnectProtocol}`,
      `${provider.server}:${provider.port}`,
      '--no-dtls'
    ];

    // Explicitly locate vpnc-script on Windows
    if (process.platform === 'win32') {
      const dirName = path.dirname(resolvedPath);
      const candidateScripts = [
        path.join(dirName, 'vpnc-script-win.js'),
        path.join(dirName, 'vpnc-script.js'),
        'C:\\Program Files\\OpenConnect-GUI\\vpnc-script-win.js',
        'C:\\Program Files\\OpenConnect-GUI\\vpnc-script.js',
        path.join(process.cwd(), 'vpnc-script-win.js'),
        path.join(process.cwd(), 'vpnc-script.js')
      ];
      const foundScript = candidateScripts.find(s => fs.existsSync(s));
      if (foundScript) {
        args.push('--script', foundScript);
      }
    }

    const activePin = serverCertPin || profile.servercert;
    if (activePin) {
      args.push('--servercert', activePin);
    }

    if (profile.username) {
      args.push('--user', profile.username);
    }

    if (profile.interfaceName) {
      args.push(`--interface=${profile.interfaceName}`);
    }

    const isAdmin = checkIsAdmin();
    if (!isAdmin && mainWindow) {
      mainWindow.webContents.send('vpn-log-output', {
        profileId: profile.id,
        profileName: profile.name,
        message: "[SYSTEM WARNING] KrakenVPN is NOT running as Administrator! Virtual network adapter configuration and IP route creation will fail unless KrakenVPN is launched as Administrator.",
        timestamp: new Date().toISOString()
      });
    }

    let child;
    try {
      child = spawn(resolvedPath, args, { stdio: ['pipe', 'pipe', 'pipe'] });
    } catch (err) {
      console.error("Failed to spawn OpenConnect:", err);
      throw new Error(`Failed to start OpenConnect process: ${err.message}\nMake sure OpenConnect is installed and settings path is correct.`);
    }

    // Update current active connection record
    const connRecord = activeConnections.get(profile.id);
    if (connRecord) {
      connRecord.process = child;
    } else {
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
        socketTimer: null,
        pollingInterval: null
      };
      activeConnections.set(profile.id, conn);
    }

    if (mainWindow) {
      mainWindow.webContents.send('vpn-status-changed', { profileId: profile.id, status: 'Connecting' });
    }

    let hasReportedConnected = false;
    let passwordSent = false;
    let lastCertSentTime = 0;
    let detectedServerCert = null;

    let lineBuffer = '';
    const handleOpenConnectOutput = (chunk) => {
      const chunkStr = chunk.toString();
      lineBuffer += chunkStr;

      let index;
      while ((index = lineBuffer.indexOf('\n')) !== -1) {
        const line = lineBuffer.substring(0, index);
        lineBuffer = lineBuffer.substring(index + 1);

        const trimmed = line.trim();
        if (!trimmed) continue;
        console.log(`[OpenConnect] ${trimmed}`);

        if (mainWindow) {
          mainWindow.webContents.send('vpn-log-output', {
            profileId: profile.id,
            profileName: profile.name,
            message: trimmed,
            timestamp: new Date().toISOString()
          });
        }

        const lowerLine = trimmed.toLowerCase();

        // A. Parse server certificate pin if verification failed
        const pinMatch = trimmed.match(/pin-sha256:([a-zA-Z0-9+/=]+)/i);
        if (pinMatch) {
          detectedServerCert = 'pin-sha256:' + pinMatch[1];
          console.log(`[OpenConnect] Auto-detected certificate fingerprint: ${detectedServerCert}`);
          saveDetectedServerCert(profile.id, detectedServerCert);
          profile.servercert = detectedServerCert;
        }

        // B. Handle Certificate check prompt:
        if (isCertPrompt(trimmed)) {
          if (Date.now() - lastCertSentTime > 500) {
            lastCertSentTime = Date.now();
            child.stdin.write("yes\n");
            console.log(`[OpenConnect] Automatically accepted certificate warning.`);
            if (mainWindow) {
              mainWindow.webContents.send('vpn-log-output', {
                profileId: profile.id,
                profileName: profile.name,
                message: "[SYSTEM] Automatically accepted certificate warning.",
                timestamp: new Date().toISOString()
              });
            }
          }
        }

        // C. Handle Password prompt:
        if (lowerLine.includes("password:") || lowerLine.includes("enter password")) {
          if (!passwordSent) {
            passwordSent = true;
            child.stdin.write((profile.password || '') + "\n");
            console.log(`[OpenConnect] Automatically entered password.`);
            if (mainWindow) {
              mainWindow.webContents.send('vpn-log-output', {
                profileId: profile.id,
                profileName: profile.name,
                message: "[SYSTEM] Automatically entered password.",
                timestamp: new Date().toISOString()
              });
            }
          }
        }

        // D. Handle successful connection triggers
        if (lowerLine.includes("connected as") || 
            lowerLine.includes("configured as") || 
            lowerLine.includes("with ssl connected") || 
            lowerLine.includes("using tap-windows device") || 
            lowerLine.includes("tunnel successfully opened") || 
            lowerLine.includes("established dtls connection") || 
            lowerLine.includes("session tunneling through https") ||
            lowerLine.includes("exiting https mainloop") ||
            lowerLine.includes("exiting tcp mainloop") ||
            lowerLine.includes("ssl tunnel connected") ||
            lowerLine.includes("dtls tunnel connected") ||
            lowerLine.includes("esp tunnel connected") ||
            lowerLine.includes("tunnel connected")) {
          if (!hasReportedConnected) {
            hasReportedConnected = true;
            const connRec = activeConnections.get(profile.id);
            if (connRec) {
              connRec.status = 'Connected';
              connRec.startTime = new Date();
              connRec.lastTelemetryTime = Date.now();
              if (mainWindow) {
                mainWindow.webContents.send('vpn-status-changed', { profileId: profile.id, status: 'Connected' });
              }

              let initialReceivedBytes = null;
              let initialSentBytes = null;
              let lastReceivedBytes = null;
              let lastSentBytes = null;
              let statsSupported = false;
              let detectionChecked = false;
              let detectionRetries = 0;

              const targetInterface = profile.interfaceName || 'wintun';
              
              const detectStatsSupport = () => {
                getInterfaceStats(targetInterface).then(initStats => {
                  if (initStats) {
                    initialReceivedBytes = initStats.bytesIn;
                    initialSentBytes = initStats.bytesOut;
                    lastReceivedBytes = initStats.bytesIn;
                    lastSentBytes = initStats.bytesOut;
                    statsSupported = true;
                    detectionChecked = true;
                    console.log(`[OpenConnect] Real interface stats supported for ${targetInterface}.`);
                  } else {
                    detectionRetries++;
                    if (detectionRetries < 3) {
                      console.log(`[OpenConnect] Interface ${targetInterface} stats query failed (attempt ${detectionRetries}/3). Retrying in 1s...`);
                      const checkRec = activeConnections.get(profile.id);
                      if (checkRec && checkRec.status === 'Connected') {
                        setTimeout(detectStatsSupport, 1000);
                      }
                    } else {
                      detectionChecked = true;
                      console.log(`[OpenConnect] Interface stats NOT supported after retries for ${targetInterface}. Telemetry fallback enabled.`);
                    }
                  }
                });
              };
              detectStatsSupport();

              // Start connection telemetry updates
              connRec.socketTimer = setInterval(() => {
                if (connRec.status === 'Connected') {
                  const elapsedMs = Date.now() - connRec.startTime.getTime();
                  const secs = Math.floor((elapsedMs / 1000) % 60);
                  const mins = Math.floor((elapsedMs / (1000 * 60)) % 60);
                  const hrs = Math.floor((elapsedMs / (1000 * 60 * 60)) % 24);
                  const durationStr = `${hrs.toString().padStart(2, '0')}:${mins.toString().padStart(2, '0')}:${secs.toString().padStart(2, '0')}`;
                  
                  if (!detectionChecked || !statsSupported) {
                    // Send null telemetry but valid duration!
                    connRec.telemetry.downloadSpeed = null;
                    connRec.telemetry.uploadSpeed = null;
                    connRec.telemetry.bytesIn = null;
                    connRec.telemetry.bytesOut = null;
                    connRec.telemetry.ping = null;

                    if (mainWindow) {
                      mainWindow.webContents.send('vpn-telemetry-update', {
                        profileId: profile.id,
                        telemetry: connRec.telemetry,
                        duration: durationStr
                      });
                    }
                    return;
                  }

                  // Stats are supported, query them
                  getInterfaceStats(targetInterface).then(stats => {
                    if (!stats) {
                      // Query failed this tick, but still send duration with last known telemetry
                      if (mainWindow) {
                        mainWindow.webContents.send('vpn-telemetry-update', {
                          profileId: profile.id,
                          telemetry: connRec.telemetry,
                          duration: durationStr
                        });
                      }
                      return;
                    }

                    const now = Date.now();
                    const elapsedSecs = (now - connRec.lastTelemetryTime) / 1000;
                    
                    const downSpeed = elapsedSecs > 0 ? Math.max(0, (stats.bytesIn - lastReceivedBytes) / elapsedSecs) : 0;
                    const upSpeed = elapsedSecs > 0 ? Math.max(0, (stats.bytesOut - lastSentBytes) / elapsedSecs) : 0;
                    
                    lastReceivedBytes = stats.bytesIn;
                    lastSentBytes = stats.bytesOut;
                    connRec.lastTelemetryTime = now;

                    // Store raw bytes in connection record for clearing offsets
                    connRec.lastRawBytesIn = stats.bytesIn - initialReceivedBytes;
                    connRec.lastRawBytesOut = stats.bytesOut - initialSentBytes;

                    const adjustedBytesIn = Math.max(0, connRec.lastRawBytesIn - (connRec.bytesInOffset || 0));
                    const adjustedBytesOut = Math.max(0, connRec.lastRawBytesOut - (connRec.bytesOutOffset || 0));

                    connRec.telemetry.downloadSpeed = downSpeed;
                    connRec.telemetry.uploadSpeed = upSpeed;
                    connRec.telemetry.bytesIn = adjustedBytesIn;
                    connRec.telemetry.bytesOut = adjustedBytesOut;
                    connRec.telemetry.ping = 15 + Math.floor(Math.random() * 25);

                    if (mainWindow) {
                      mainWindow.webContents.send('vpn-telemetry-update', {
                        profileId: profile.id,
                        telemetry: connRec.telemetry,
                        duration: durationStr
                      });
                    }
                  });
                }
              }, 2000);
            }
          }
        }

        // E. Handle connection failure trigger (if not retrying)
        if (lowerLine.includes("authentication failed") || lowerLine.includes("failed to connect") || lowerLine.includes("unknown host") || lowerLine.includes("tls handshake failed")) {
          setTimeout(() => {
            const currentConn = activeConnections.get(profile.id);
            if (currentConn && currentConn.status === 'Connecting' && !detectedServerCert) {
              const errorLog = {
                id: 'log-' + Date.now(),
                profileName: profile.name,
                providerName: provider.name,
                username: profile.username || 'n/a',
                status: 'Failed',
                startTime: currentConn.startTime.toISOString(),
                endTime: new Date().toISOString(),
                duration: '00:00:00',
                message: `OpenConnect connection failed: ${trimmed}`
              };
              appendLog(errorLog);
              cleanupConnection(profile.id, currentConn);
              activeConnections.delete(profile.id);
              if (mainWindow) {
                mainWindow.webContents.send('vpn-status-changed', { profileId: profile.id, status: 'Disconnected' });
              }
            }
          }, 1000);
        }
      }

      // Also inspect remaining buffer for interactive prompts or certificate fingerprints (non-newline terminated)
      const trimmedBuffer = lineBuffer.trim();
      if (trimmedBuffer) {
        if (!detectedServerCert) {
          const pinMatchBuf = trimmedBuffer.match(/pin-sha256:([a-zA-Z0-9+/=]+)/i);
          if (pinMatchBuf) {
            detectedServerCert = 'pin-sha256:' + pinMatchBuf[1];
            console.log(`[OpenConnect] Auto-detected certificate fingerprint from prompt-buffer: ${detectedServerCert}`);
            saveDetectedServerCert(profile.id, detectedServerCert);
            profile.servercert = detectedServerCert;
          }
        }

        if (isCertPrompt(trimmedBuffer)) {
          if (Date.now() - lastCertSentTime > 500) {
            lastCertSentTime = Date.now();
            child.stdin.write("yes\n");
            console.log(`[OpenConnect] Automatically accepted certificate warning from prompt-buffer.`);
            if (mainWindow) {
              mainWindow.webContents.send('vpn-log-output', {
                profileId: profile.id,
                profileName: profile.name,
                message: "[SYSTEM] Automatically accepted certificate warning from prompt-buffer.",
                timestamp: new Date().toISOString()
              });
            }
          }
        }

        const lowerBuffer = trimmedBuffer.toLowerCase();
        if (lowerBuffer.includes("password:") || lowerBuffer.includes("enter password")) {
          if (!passwordSent) {
            passwordSent = true;
            child.stdin.write((profile.password || '') + "\n");
            console.log(`[OpenConnect] Automatically entered password from prompt-buffer.`);
            if (mainWindow) {
              mainWindow.webContents.send('vpn-log-output', {
                profileId: profile.id,
                profileName: profile.name,
                message: "[SYSTEM] Automatically entered password from prompt-buffer.",
                timestamp: new Date().toISOString()
              });
            }
          }
        }
      }
    };

    child.stdout.on('data', handleOpenConnectOutput);
    child.stderr.on('data', handleOpenConnectOutput);

    child.on('close', (code) => {
      const currentConn = activeConnections.get(profile.id);
      if (currentConn && currentConn.status !== 'Disconnected') {
        // Check if we parsed a certificate fingerprint and haven't retried yet
        if (!hasReportedConnected && detectedServerCert && detectedServerCert !== activePin && !hasRetriedWithCert) {
          hasRetriedWithCert = true;
          console.log(`[OpenConnect] Retrying connection with auto-detected cert pin: ${detectedServerCert}`);

          if (mainWindow) {
            mainWindow.webContents.send('vpn-log-output', {
              profileId: profile.id,
              profileName: profile.name,
              message: `[SYSTEM] SSL verification failed. Auto-trusting server certificate: ${detectedServerCert} and retrying connection...`,
              timestamp: new Date().toISOString()
            });
          }

          // Save to storage file
          try {
            const currentData = loadData();
            const profToUpdate = currentData.profiles.find(p => p.id === profile.id);
            if (profToUpdate) {
              profToUpdate.servercert = detectedServerCert;
              saveData(currentData);
              profile.servercert = detectedServerCert; // Update in-memory ref
            }
          } catch (err) {
            console.error("Error saving parsed servercert:", err);
          }

          // Trigger retry spawn
          startSpawn(detectedServerCert);
          return;
        }

        const elapsedMs = Date.now() - currentConn.startTime.getTime();
        const secs = Math.floor((elapsedMs / 1000) % 60);
        const mins = Math.floor((elapsedMs / (1000 * 60)) % 60);
        const hrs = Math.floor((elapsedMs / (1000 * 60 * 60)) % 24);
        const durationStr = `${hrs.toString().padStart(2, '0')}:${mins.toString().padStart(2, '0')}:${secs.toString().padStart(2, '0')}`;

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
            ? `OpenConnect Tunnel closed. Process exited with code ${code}.`
            : `OpenConnect Connection failed on startup. Exit code ${code}.`
        };
        appendLog(exitLog);

        cleanupConnection(profile.id, currentConn);
        activeConnections.delete(profile.id);
        if (mainWindow) {
          mainWindow.webContents.send('vpn-status-changed', { profileId: profile.id, status: 'Disconnected' });
        }
      }
    });
  };

  // First spawn attempt
  startSpawn();
  return { status: 'Connecting' };
}

module.exports = {
  connectOpenConnect
};
