// Application State Cache
export const state = {
  appData: {
    providers: [],
    profiles: [],
    settings: {},
    logs: [],
    isAdmin: false
  },
  lastImportedConfigContent: ''
};

// Formatting helpers
export function formatBytes(bytes, decimals = 2) {
  if (bytes === null || bytes === undefined || isNaN(bytes) || bytes < 0) return '--';
  if (bytes === 0) return '0.00 B';
  const k = 1024;
  const dm = decimals < 0 ? 0 : decimals;
  const sizes = ['B', 'KB', 'MB', 'GB', 'TB'];
  const i = Math.floor(Math.log(bytes) / Math.log(k));
  return parseFloat((bytes / Math.pow(k, i)).toFixed(dm)) + ' ' + sizes[i];
}

export function formatSpeed(bytesPerSec) {
  if (bytesPerSec === null || bytesPerSec === undefined || isNaN(bytesPerSec) || bytesPerSec < 0) return '--';
  if (bytesPerSec === 0 || bytesPerSec === 0.0) return '0.0 Kbps';
  const bitsPerSec = bytesPerSec * 8;
  if (bitsPerSec >= 1000000) {
    const mbps = bitsPerSec / 1000000;
    return mbps.toFixed(1) + ' Mbps';
  } else {
    const kbps = bitsPerSec / 1000;
    return kbps.toFixed(1) + ' Kbps';
  }
}
