const SALT = 'pm3_dashboard_salt';

export const auth = {
  isSetupComplete: () => !!localStorage.getItem('pm3_setup_complete'),

  setupPassword: (password) => {
    localStorage.setItem('pm3_password_hash', btoa(password + SALT));
    localStorage.setItem('pm3_setup_complete', 'true');
  },

  login: (password) => {
    const hash = btoa(password + SALT);
    const stored = localStorage.getItem('pm3_password_hash');
    if (hash === stored) {
      sessionStorage.setItem('pm3_session', 'true');
      return true;
    }
    return false;
  },

  logout: () => sessionStorage.removeItem('pm3_session'),

  isAuthenticated: () => !!sessionStorage.getItem('pm3_session'),

  resetSetup: () => {
    localStorage.removeItem('pm3_setup_complete');
    localStorage.removeItem('pm3_password_hash');
    sessionStorage.removeItem('pm3_session');
  }
};
