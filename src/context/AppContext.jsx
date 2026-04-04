import { createContext, useContext, useState, useEffect, useCallback, useRef } from 'react';
import { daemon } from '../services';

const AppContext = createContext(null);

export function AppProvider({ children }) {
  const [processes, setProcesses] = useState([]);
  const [logs, setLogs] = useState([]);
  const [metrics, setMetrics] = useState({});
  const [metricsHistory, setMetricsHistory] = useState({});
  const [sidebarCollapsed, setSidebarCollapsed] = useState(false);
  const [toasts, setToasts] = useState([]);
  const [refreshInterval, setRefreshInterval] = useState(5000);
  const toastIdRef = useRef(0);

  // Load processes on mount
  useEffect(() => {
    Promise.resolve(daemon.getProcesses()).then(setProcesses);
  }, []);

  // Subscribe to metrics
  useEffect(() => {
    const unsub = daemon.subscribeToMetrics((newMetrics) => {
      setMetrics(newMetrics);
      setMetricsHistory(prev => {
        const next = { ...prev };
        const time = new Date().toLocaleTimeString('en-US', { hour12: false });
        for (const [id, data] of Object.entries(newMetrics)) {
          const history = next[id] || [];
          next[id] = [...history, { time, cpu: data.cpu, ram: data.ram }].slice(-24);
        }
        return next;
      });
    }, refreshInterval);
    return unsub;
  }, [refreshInterval]);

  // Subscribe to logs
  useEffect(() => {
    const unsub = daemon.subscribeToLogs((entry) => {
      setLogs(prev => [...prev, entry].slice(-200));
    });
    return unsub;
  }, []);

  const refreshProcesses = useCallback(async () => {
    const procs = await Promise.resolve(daemon.getProcesses());
    setProcesses(procs);
  }, []);

  const addToast = useCallback((message, type = 'success') => {
    const id = ++toastIdRef.current;
    setToasts(prev => [...prev, { id, message, type }]);
    setTimeout(() => {
      setToasts(prev => prev.filter(t => t.id !== id));
    }, 3000);
  }, []);

  const value = {
    processes, refreshProcesses,
    logs, setLogs,
    metrics, metricsHistory,
    sidebarCollapsed, setSidebarCollapsed,
    toasts, addToast,
    refreshInterval, setRefreshInterval,
  };

  return <AppContext.Provider value={value}>{children}</AppContext.Provider>;
}

export function useApp() {
  const ctx = useContext(AppContext);
  if (!ctx) throw new Error('useApp must be used within AppProvider');
  return ctx;
}
