import { state, formatSpeed } from './state.js';
import { renderDashboardGrid, updateGlobalCounters, renderLogsHistory } from './ui.js';

export function setupIPCListeners() {
  window.electronAPI.onStatusChanged((data) => {
    const { profileId, status } = data;
    const profile = state.appData.profiles.find(p => p.id === profileId);
    if (profile) {
      profile.status = status;
      if (status === 'Disconnected') {
        profile.telemetry = null;
      }
    }
    renderDashboardGrid();
    updateGlobalCounters();
  });

  window.electronAPI.onTelemetryUpdate((data) => {
    const { profileId, telemetry, duration } = data;
    const profile = state.appData.profiles.find(p => p.id === profileId);
    if (profile) {
      profile.telemetry = telemetry;
    }

    const timerElem = document.getElementById(`timer-${profileId}`);
    if (timerElem) timerElem.innerText = duration;

    const downElem = document.getElementById(`speed-down-${profileId}`);
    if (downElem) downElem.innerText = formatSpeed(telemetry.downloadSpeed);

    const upElem = document.getElementById(`speed-up-${profileId}`);
    if (upElem) upElem.innerText = formatSpeed(telemetry.uploadSpeed);

    updateGlobalCounters();
  });

  const logTerminal = document.getElementById('log-terminal');
  window.electronAPI.onLogOutput((data) => {
    const { profileName, message, timestamp } = data;
    if (!logTerminal) return;
    
    const timeStr = new Date(timestamp).toLocaleTimeString();
    const line = document.createElement('div');
    
    let isError = message.toLowerCase().includes('failed') || message.toLowerCase().includes('error');
    let isSystem = profileName === 'System';
    let isSuccess = message.toLowerCase().includes('completed') || message.toLowerCase().includes('active');

    if (isError) {
      line.className = 'terminal-line error-msg';
    } else if (isSystem) {
      line.className = 'terminal-line system-msg';
    } else if (isSuccess) {
      line.className = 'terminal-line info-msg';
    } else {
      line.className = 'terminal-line';
    }

    line.innerText = `[${timeStr}] [${profileName}] ${message}`;
    logTerminal.appendChild(line);
    logTerminal.scrollTop = logTerminal.scrollHeight;
  });

  window.electronAPI.onLogsUpdated((logs) => {
    state.appData.logs = logs;
    renderLogsHistory();
  });
}
