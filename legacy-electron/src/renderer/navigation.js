import { showToast } from './toast.js';

export function setupNavigation() {
  // Titlebar custom window controls
  const btnMinimize = document.getElementById('btn-minimize');
  if (btnMinimize) {
    btnMinimize.addEventListener('click', () => {
      window.electronAPI.minimize();
    });
  }

  const btnMaximize = document.getElementById('btn-maximize');
  if (btnMaximize) {
    btnMaximize.addEventListener('click', () => {
      window.electronAPI.maximize();
    });
  }

  const btnClose = document.getElementById('btn-close');
  if (btnClose) {
    btnClose.addEventListener('click', () => {
      window.electronAPI.close();
    });
  }

  const menuItems = document.querySelectorAll('.menu-item');
  const screens = document.querySelectorAll('.screen');

  menuItems.forEach(item => {
    item.addEventListener('click', (e) => {
      e.preventDefault();
      const targetId = item.getAttribute('data-target');
      
      menuItems.forEach(i => i.classList.remove('active'));
      item.classList.add('active');

      screens.forEach(s => {
        s.classList.remove('active');
        if (s.id === targetId) {
          s.classList.add('active');
        }
      });
    });
  });

  const tabBtns = document.querySelectorAll('.tab-btn');
  const tabContents = document.querySelectorAll('.tab-content');

  tabBtns.forEach(btn => {
    btn.addEventListener('click', () => {
      const targetTab = btn.getAttribute('data-tab');

      tabBtns.forEach(b => b.classList.remove('active'));
      btn.classList.add('active');

      tabContents.forEach(content => {
        content.classList.remove('active');
        if (content.id === targetTab) {
          content.classList.add('active');
        }
      });
    });
  });

  const btnCreateFirst = document.getElementById('btn-create-first-profile');
  if (btnCreateFirst) {
    btnCreateFirst.addEventListener('click', () => {
      document.querySelector('.menu-item[data-target="screen-settings"]').click();
      document.querySelector('.tab-btn[data-tab="tab-profiles"]').click();
      document.getElementById('profile-name').focus();
    });
  }
}

export function setupAdminSwitch() {
  const adminSwitch = document.getElementById('admin-privilege-switch');
  if (adminSwitch) {
    adminSwitch.addEventListener('change', async () => {
      if (adminSwitch.checked) {
        adminSwitch.disabled = true;
        showToast("Requesting administrator privilege...", "warning");
        try {
          await window.electronAPI.requestAdminPrivileges();
        } catch (err) {
          showToast(err.message || "Failed to request administrator privileges.", "error");
          adminSwitch.checked = false;
          adminSwitch.disabled = false;
        }
      }
    });
  }
}
