import { useMemo } from 'react';
import { Link } from 'react-router-dom';
import { Server, HardDrive, Cpu, FileText } from 'lucide-react';
import { LineChart, Line, XAxis, YAxis, Tooltip, ResponsiveContainer } from 'recharts';
import { useApp } from '../context/AppContext';
import MetricCard from '../components/common/MetricCard';
import StatusBadge from '../components/common/StatusBadge';

const TOOLTIP_STYLE = { background: '#1E293B', border: '1px solid #334155', borderRadius: '8px' };
const TICK_STYLE = { fill: '#94A3B8', fontSize: 11 };

export default function Overview() {
  const { processes, logs, metrics, metricsHistory } = useApp();

  const runningProcesses = processes.filter(p => p.status === 'running');
  const activeCount = runningProcesses.length;
  const totalMemory = runningProcesses.reduce((sum, p) => sum + (metrics[p.id]?.ram || p.ram), 0);
  const avgCpu = runningProcesses.length
    ? runningProcesses.reduce((sum, p) => sum + (metrics[p.id]?.cpu || p.cpu), 0) / runningProcesses.length
    : 0;

  const aggregateHistory = useMemo(() => {
    const allIds = Object.keys(metricsHistory);
    if (allIds.length === 0) return [];
    const maxLen = Math.max(...allIds.map(id => metricsHistory[id].length));
    const result = [];
    for (let i = 0; i < maxLen; i++) {
      let cpu = 0, ram = 0, time = '';
      let count = 0;
      for (const id of allIds) {
        const entry = metricsHistory[id][i];
        if (entry) {
          cpu += entry.cpu;
          ram += entry.ram;
          time = entry.time;
          count++;
        }
      }
      if (count > 0) {
        result.push({ time, cpu: cpu / count, ram });
      }
    }
    return result;
  }, [metricsHistory]);

  const recentLogs = logs.slice(-5);

  const levelColor = (level) => {
    if (level === 'WARN') return 'text-pm3-warn';
    if (level === 'ERROR') return 'text-pm3-error';
    return 'text-pm3-muted';
  };

  return (
    <div className="space-y-6">
      {/* Stats Row */}
      <div className="grid grid-cols-1 md:grid-cols-2 lg:grid-cols-4 gap-4">
        <MetricCard title="Active Processes" value={activeCount} subtitle="currently running" accentColor="#10B981" icon={Server} />
        <MetricCard title="Total Memory" value={`${totalMemory.toFixed(0)} MB`} subtitle="across all processes" accentColor="#3B82F6" icon={HardDrive} />
        <MetricCard title="CPU Load" value={`${avgCpu.toFixed(1)}%`} subtitle="average across running" accentColor="#F59E0B" icon={Cpu} />
        <MetricCard title="Log Entries" value={logs.length} subtitle="this session" accentColor="#8B5CF6" icon={FileText} />
      </div>

      {/* Mini Process Table */}
      <div className="bg-pm3-surface rounded-lg border border-pm3-border p-4">
        <div className="flex justify-between items-center mb-3">
          <h2 className="text-lg font-semibold text-pm3-text">Process Status</h2>
          <Link to="/processes" className="text-pm3-orange text-sm hover:underline">View All</Link>
        </div>
        <div className="overflow-x-auto">
          <table className="w-full text-sm">
            <thead>
              <tr className="text-pm3-muted text-xs border-b border-pm3-border">
                <th className="text-left py-2 font-medium">Name</th>
                <th className="text-left py-2 font-medium">Status</th>
                <th className="text-right py-2 font-medium">CPU%</th>
                <th className="text-right py-2 font-medium">RAM</th>
                <th className="text-right py-2 font-medium">Uptime</th>
              </tr>
            </thead>
            <tbody>
              {processes.map(p => (
                <tr key={p.id} className="border-b border-pm3-border/50 hover:bg-pm3-hover/20">
                  <td className="py-2 text-pm3-text font-mono text-xs">{p.name}</td>
                  <td className="py-2"><StatusBadge status={p.status} /></td>
                  <td className="py-2 text-right text-pm3-text font-mono text-xs">
                    {p.status === 'running' ? (metrics[p.id]?.cpu || p.cpu).toFixed(1) + '%' : '—'}
                  </td>
                  <td className="py-2 text-right text-pm3-text font-mono text-xs">
                    {p.status === 'running' ? (metrics[p.id]?.ram || p.ram).toFixed(0) + ' MB' : '—'}
                  </td>
                  <td className="py-2 text-right text-pm3-muted text-xs">{p.uptime || '—'}</td>
                </tr>
              ))}
            </tbody>
          </table>
        </div>
      </div>

      {/* Charts Row */}
      <div className="grid grid-cols-1 lg:grid-cols-2 gap-4">
        <div className="bg-pm3-surface rounded-lg border border-pm3-border p-4">
          <h3 className="text-sm font-semibold text-pm3-text mb-3">System CPU</h3>
          <ResponsiveContainer width="100%" height={180}>
            <LineChart data={aggregateHistory}>
              <XAxis dataKey="time" tick={TICK_STYLE} axisLine={false} tickLine={false} />
              <YAxis tick={TICK_STYLE} axisLine={false} tickLine={false} domain={[0, 'auto']} unit="%" />
              <Tooltip contentStyle={TOOLTIP_STYLE} />
              <Line type="monotone" dataKey="cpu" stroke="#CE412B" dot={false} strokeWidth={2} name="Avg CPU %" />
            </LineChart>
          </ResponsiveContainer>
        </div>
        <div className="bg-pm3-surface rounded-lg border border-pm3-border p-4">
          <h3 className="text-sm font-semibold text-pm3-text mb-3">System Memory</h3>
          <ResponsiveContainer width="100%" height={180}>
            <LineChart data={aggregateHistory}>
              <XAxis dataKey="time" tick={TICK_STYLE} axisLine={false} tickLine={false} />
              <YAxis tick={TICK_STYLE} axisLine={false} tickLine={false} domain={[0, 'auto']} unit=" MB" />
              <Tooltip contentStyle={TOOLTIP_STYLE} />
              <Line type="monotone" dataKey="ram" stroke="#3B82F6" dot={false} strokeWidth={2} name="Total RAM (MB)" />
            </LineChart>
          </ResponsiveContainer>
        </div>
      </div>

      {/* Recent Logs */}
      <div className="bg-pm3-surface rounded-lg border border-pm3-border p-4">
        <div className="flex justify-between items-center mb-3">
          <h2 className="text-lg font-semibold text-pm3-text">Recent Logs</h2>
          <Link to="/logs" className="text-pm3-orange text-sm hover:underline">View All Logs</Link>
        </div>
        <div className="space-y-1">
          {recentLogs.length === 0 ? (
            <p className="text-pm3-muted text-sm">No log entries yet...</p>
          ) : (
            recentLogs.map(log => (
              <div key={log.id} className={`font-mono text-xs py-1 px-2 rounded ${log.level === 'ERROR' ? 'bg-red-500/5' : ''} ${levelColor(log.level)}`}>
                <span className="text-pm3-muted">[{log.timestamp}]</span>
                {' '}
                <span className="text-pm3-info">[{log.process}]</span>
                {' '}
                <span className={levelColor(log.level)}>[{log.level}]</span>
                {' '}
                <span className="text-pm3-text">{log.message}</span>
              </div>
            ))
          )}
        </div>
      </div>
    </div>
  );
}
