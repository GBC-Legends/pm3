import { useState, useCallback } from 'react';
import { auth } from '../services/auth';

export function useAuth() {
  const [isAuthenticated, setIsAuthenticated] = useState(auth.isAuthenticated());

  const login = useCallback((password) => {
    const success = auth.login(password);
    setIsAuthenticated(success);
    return success;
  }, []);

  const logout = useCallback(() => {
    auth.logout();
    setIsAuthenticated(false);
  }, []);

  const setupPassword = useCallback((password) => {
    auth.setupPassword(password);
  }, []);

  const resetSetup = useCallback(() => {
    auth.resetSetup();
    setIsAuthenticated(false);
  }, []);

  return {
    isAuthenticated,
    isSetupComplete: auth.isSetupComplete(),
    login,
    logout,
    setupPassword,
    resetSetup,
  };
}
