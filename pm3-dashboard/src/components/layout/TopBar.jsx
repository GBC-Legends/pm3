import { useState, useEffect } from 'react';
import { daemon } from '../../services';

export default function TopBar() {
  const [time, setTime] = useState(new Date().toLocaleTimeString('en-US', { hour12: false }));
  const [connected, setConnected] = useState(true);

  const apiUrl = import.meta.env.VITE_API_URL;
  const isRealDaemon = !!apiUrl;

  useEffect(() => {
    const interval = setInterval(() => {
      setTime(new Date().toLocaleTimeString('en-US', { hour12: false }));
    }, 1000);
    return () => clearInterval(interval);
  }, []);

  useEffect(() => {
    if (!isRealDaemon) { setConnected(true); return; }
    let cancelled = false;
    const check = async () => {
      const ok = daemon.healthCheck ? await daemon.healthCheck() : false;
      if (!cancelled) setConnected(ok);
    };
    check();
    const id = setInterval(check, 15000);
    return () => { cancelled = true; clearInterval(id); };
  }, [isRealDaemon]);

  let daemonHost;
  try { daemonHost = new URL(apiUrl, window.location.origin).host; } catch { daemonHost = apiUrl; }

  const statusLabel = isRealDaemon
    ? (connected ? `Connected · ${daemonHost}` : `Disconnected · ${daemonHost}`)
    : 'Connected · Mock Mode';

  return (
    <div className="h-14 bg-pm3-surface border-b border-pm3-border flex items-center justify-between px-4 flex-shrink-0">
      <span className="text-lg font-bold text-pm3-orange">PM3</span>

      <div className="flex items-center gap-2 text-sm text-pm3-muted">
        <span className={`inline-block w-2 h-2 rounded-full ${connected ? 'bg-pm3-success animate-pulse' : 'bg-pm3-error'}`} />
        {statusLabel}
      </div>

      <span className="text-sm text-pm3-muted font-mono">{time}</span>
    </div>
  );
}
