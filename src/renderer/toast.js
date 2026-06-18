// Toast Notifications System
export function showToast(message, type = 'success') {
  const container = document.getElementById('toast-container');
  if (!container) return;
  
  const toast = document.createElement('div');
  toast.className = `toast ${type}`;
  
  let typeColor = '#00d2ff';
  if (type === 'success') typeColor = '#00e676';
  if (type === 'warning') typeColor = '#ff9100';
  if (type === 'error') typeColor = '#ff3d00';

  toast.innerHTML = `
    <span>${message}</span>
    <button class="toast-close">
      <svg viewBox="0 0 24 24" width="16" height="16"><path fill="${typeColor}" d="M19 6.41L17.59 5L12 10.59L6.41 5L5 6.41L10.59 12L5 17.59L6.41 19L12 13.41L17.59 19L19 17.59L13.41 12L19 6.41z"/></svg>
    </button>
  `;
  
  container.appendChild(toast);
  
  const closeBtn = toast.querySelector('.toast-close');
  closeBtn.addEventListener('click', () => {
    toast.style.animation = 'slide-out 0.2s ease forwards';
    toast.addEventListener('animationend', () => toast.remove());
  });

  // Auto remove after 4.5 seconds
  setTimeout(() => {
    if (toast.parentNode) {
      toast.style.animation = 'slide-out 0.2s ease forwards';
      toast.addEventListener('animationend', () => toast.remove());
    }
  }, 4500);
}
