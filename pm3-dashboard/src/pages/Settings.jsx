import { useState, useEffect } from 'react';
import { useApp } from '../context/AppContext';
import { daemon, daemonCapabilities } from '../services';
import ConfirmDialog from '../components/common/ConfirmDialog';

export default function Settings() {
  const { addToast, setLogs, refreshInterval, setRefreshInterval } = useApp();
  const [showClearConfirm, setShowClearConfirm] = useState(false);
  const [daemonStatus, setDaemonStatus] = useState('checking');

  const apiUrl = import.meta.env.VITE_API_URL;
  const isRealDaemon = !!apiUrl;

  useEffect(() => {
    if (!isRealDaemon) {
      setDaemonStatus('mock');
      return;
    }
    let cancelled = false;
    const check = async () => {
      const ok = daemon.healthCheck ? await daemon.healthCheck() : false;
      if (!cancelled) setDaemonStatus(ok ? 'connected' : 'disconnected');
    };
    check();
    const id = setInterval(check, 15000);
    return () => { cancelled = true; clearInterval(id); };
  }, [isRealDaemon]);

  const handleClearLogs = () => {
    setLogs([]);
    setShowClearConfirm(false);
    addToast('All logs cleared', 'info');
  };

  return (
    <div className="space-y-6 max-w-2xl">
      <h1 className="text-2xl font-bold text-pm3-text">Settings</h1>

      {/* Connection Settings */}
      <div className="bg-pm3-surface rounded-lg border border-pm3-border p-4 space-y-3">
        <h2 className="text-lg font-semibold text-pm3-text">Connection</h2>
        <div className="grid grid-cols-2 gap-4 text-sm">
          <div>
            <span className="text-pm3-muted">Mode: </span>
            <span className="text-pm3-text font-mono">{isRealDaemon ? 'Real Daemon' : 'Mock (offline)'}</span>
          </div>
          <div>
            <span className="text-pm3-muted">API URL: </span>
            <span className="text-pm3-text font-mono text-xs">{isRealDaemon ? apiUrl : 'N/A'}</span>
          </div>
          <div>
            <span className="text-pm3-muted">Process Mgmt: </span>
            <span className="text-pm3-text">{daemonCapabilities.canManageProcesses ? 'Enabled' : 'Read-only'}</span>
          </div>
          <div className="flex items-center gap-2">
            <span className="text-pm3-muted">Status: </span>
            <span className="inline-flex items-center gap-1.5 text-pm3-text">
              {daemonStatus === 'connected' && (
                <><span className="w-2 h-2 rounded-full bg-pm3-success animate-pulse" />Connected</>
              )}
              {daemonStatus === 'disconnected' && (
                <><span className="w-2 h-2 rounded-full bg-pm3-error" />Disconnected</>
              )}
              {daemonStatus === 'checking' && (
                <><span className="w-2 h-2 rounded-full bg-pm3-warn animate-pulse" />Checking...</>
              )}
              {daemonStatus === 'mock' && (
                <><span className="w-2 h-2 rounded-full bg-pm3-info animate-pulse" />Mock Mode</>
              )}
            </span>
          </div>
        </div>
      </div>

      {/* Dashboard Preferences */}
      <div className="bg-pm3-surface rounded-lg border border-pm3-border p-4 space-y-4">
        <h2 className="text-lg font-semibold text-pm3-text">Preferences</h2>

        <div className="flex items-center justify-between">
          <div>
            <p className="text-pm3-text text-sm">Refresh Interval</p>
            <p className="text-pm3-muted text-xs">How often metrics update</p>
          </div>
          <select
            value={refreshInterval}
            onChange={(e) => setRefreshInterval(Number(e.target.value))}
            className="bg-slate-900 border border-pm3-border rounded-md px-3 py-1.5 text-pm3-text text-sm focus:outline-none focus:border-pm3-orange"
          >
            <option value={1000}>1 second</option>
            <option value={5000}>5 seconds</option>
            <option value={10000}>10 seconds</option>
            <option value={30000}>30 seconds</option>
          </select>
        </div>

        <div className="flex items-center justify-between">
          <div>
            <p className="text-pm3-text text-sm">Theme</p>
            <p className="text-pm3-muted text-xs">Dashboard appearance</p>
          </div>
          <span className="text-pm3-muted text-sm">Dark (only)</span>
        </div>
      </div>

      {/* Danger Zone */}
      <div className="bg-pm3-surface rounded-lg border border-red-500/30 p-4 space-y-4">
        <h2 className="text-lg font-semibold text-red-400">Danger Zone</h2>

        <div className="flex items-center justify-between">
          <div>
            <p className="text-pm3-text text-sm">Clear All Logs</p>
            <p className="text-pm3-muted text-xs">Remove all log entries from memory</p>
          </div>
          <button
            onClick={() => setShowClearConfirm(true)}
            className="bg-red-600 hover:bg-red-700 text-white rounded-md px-4 py-2 text-sm transition-colors"
          >
            Clear
          </button>
        </div>
      </div>

      <ConfirmDialog
        isOpen={showClearConfirm}
        title="Clear All Logs"
        message="Are you sure? This will remove all log entries from memory."
        confirmLabel="Clear Logs"
        confirmVariant="danger"
        onConfirm={handleClearLogs}
        onCancel={() => setShowClearConfirm(false)}
      />
    </div>
  );
}
