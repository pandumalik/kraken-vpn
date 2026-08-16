const path = require('path');
const fs = require('fs');
const { spawn, exec } = require('child_process');

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

function terminateProcess(child) {
  if (!child) return;
  try {
    child.kill('SIGKILL');
  } catch (e) {}

  try {
    exec(`taskkill /F /T /PID ${child.pid}`, { stdio: 'ignore' }, () => {});
  } catch (err) {}
}

function testOpenConnect(config, settings) {
  const openconnectPath = settings.openconnectPath || 'openconnect.exe';
  let resolvedPath = openconnectPath;
  if (!path.isAbsolute(resolvedPath)) {
    const localPath = path.join(process.cwd(), resolvedPath);
    if (fs.existsSync(localPath)) {
      resolvedPath = localPath;
    }
  }

  // Parse inner protocol
  let openconnectProtocol = 'fortinet';
  const lowerProto = (config.protocol || '').toLowerCase();
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

  const args = [
    `--protocol=${openconnectProtocol}`,
    `${config.server}:${config.port}`,
    '--no-dtls'
  ];

  if (config.servercert) {
    args.push('--servercert', config.servercert);
  }

  if (config.username) {
    args.push('--user', config.username);
  }

  if (config.interfaceName) {
    args.push(`--interface=${config.interfaceName}`);
  }

  return new Promise((resolve) => {
    let child;
    try {
      child = spawn(resolvedPath, args, { stdio: ['pipe', 'pipe', 'pipe'] });
    } catch (err) {
      resolve({ success: false, message: `Failed to start openconnect: ${err.message}` });
      return;
    }

    let output = '';
    let success = false;
    let message = 'Test connection timed out.';
    let detectedPin = null;

    // Timeout after 8 seconds
    const timeoutTimer = setTimeout(() => {
      terminateProcess(child);
      resolve({ success, message });
    }, 8000);

    let passwordSent = false;
    let lastCertSentTime = 0;

    const handleOutput = (chunk) => {
      const text = chunk.toString();
      output += text;
      const lowerOutput = output.toLowerCase();

      // Check certificate failure fingerprint
      const pinMatch = text.match(/pin-sha256:([a-zA-Z0-9+/=]+)/i);
      if (pinMatch) {
        detectedPin = 'pin-sha256:' + pinMatch[1];
      }

      // Check connection success indicators
      if (lowerOutput.includes("connected as") || 
          lowerOutput.includes("configured as") || 
          lowerOutput.includes("with ssl connected") || 
          lowerOutput.includes("using tap-windows device") || 
          lowerOutput.includes("tunnel successfully opened") || 
          lowerOutput.includes("established dtls connection") || 
          lowerOutput.includes("session tunneling through https") ||
          lowerOutput.includes("exiting https mainloop") ||
          lowerOutput.includes("exiting tcp mainloop") ||
          lowerOutput.includes("ssl tunnel connected") ||
          lowerOutput.includes("dtls tunnel connected") ||
          lowerOutput.includes("esp tunnel connected") ||
          lowerOutput.includes("tunnel connected")) {
        success = true;
        message = 'Connection successful!';
        clearTimeout(timeoutTimer);
        terminateProcess(child);
        resolve({ success, message });
      }

      // Check password prompt - if we see password prompt and this is a Provider-only test (no password provided), then endpoint is valid!
      if (!config.password && (lowerOutput.includes("password:") || lowerOutput.includes("enter password"))) {
        success = true;
        message = 'Server reached successfully (login prompt received).';
        if (detectedPin) {
          message += `\nDetected Certificate Pin:\n${detectedPin}`;
        }
        clearTimeout(timeoutTimer);
        terminateProcess(child);
        resolve({ success, message, detectedPin });
      }

      // If password prompt is received and we have a password, submit it!
      if (config.password && (lowerOutput.includes("password:") || lowerOutput.includes("enter password"))) {
        if (!passwordSent) {
          passwordSent = true;
          child.stdin.write(config.password + '\n');
        }
      }

      // Check validation failure prompt without retry
      if (isCertPrompt(text) || isCertPrompt(output)) {
        if (Date.now() - lastCertSentTime > 500) {
          lastCertSentTime = Date.now();
          child.stdin.write('yes\n');
        }
      }

      // Check authentication failure
      if (lowerOutput.includes("authentication failed") || lowerOutput.includes("failed to connect") || lowerOutput.includes("unknown host") || lowerOutput.includes("tls handshake failed")) {
        success = false;
        if (lowerOutput.includes("authentication failed")) {
          message = 'Authentication failed. Please verify your credentials.';
        } else if (lowerOutput.includes("unknown host")) {
          message = 'Server host unreachable or unknown.';
        } else {
          message = `Connection failed: ${text.trim().split('\n')[0]}`;
        }
        clearTimeout(timeoutTimer);
        terminateProcess(child);
        resolve({ success, message, detectedPin });
      }
    };

    child.stdout.on('data', handleOutput);
    child.stderr.on('data', handleOutput);

    child.on('close', (code) => {
      clearTimeout(timeoutTimer);
      if (success) {
        resolve({ success, message });
      } else {
        let failureReason = `Connection failed on exit (code ${code}).`;
        if (output.toLowerCase().includes("signer not found")) {
          failureReason = 'Server certificate verification failed (signer not found).';
          if (detectedPin) {
            failureReason += `\n\nAuto-detected fingerprint:\n${detectedPin}`;
          }
        }
        resolve({ success: false, message: failureReason, detectedPin });
      }
    });
  });
}

module.exports = {
  testOpenConnect
};
