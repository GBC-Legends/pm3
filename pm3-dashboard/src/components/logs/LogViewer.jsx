import { useState, useEffect, useRef } from 'react';
import { Search, Trash2 } from 'lucide-react';
import { useApp } from '../../context/AppContext';
import ConfirmDialog from '../common/ConfirmDialog';

const LEVEL_TABS = ['all', 'INFO', 'WARN', 'ERROR'];

export default function LogViewer({ initialProcessFilter }) {
  const { logs, processes, setLogs, addToast } = useApp();
  const [processFilter, setProcessFilter] = useState(initialProcessFilter || 'all');
  const [levelFilter, setLevelFilter] = useState('all');
  const [search, setSearch] = useState('');
  const [autoScroll, setAutoScroll] = useState(true);
  const [showClearConfirm, setShowClearConfirm] = useState(false);
  const logEndRef = useRef(null);

  useEffect(() => {
    if (autoScroll && logEndRef.current) {
      logEndRef.current.scrollIntoView({ behavior: 'smooth' });
    }
  }, [logs, autoScroll]);

  const filtered = logs.filter(log => {
    if (processFilter !== 'all' && log.process !== processFilter) return false;
    if (levelFilter !== 'all' && log.level !== levelFilter) return false;
    if (search && !log.message.toLowerCase().includes(search.toLowerCase())) return false;
    return true;
  });

  const levelColor = (level) => {
    if (level === 'WARN') return 'text-pm3-warn';
    if (level === 'ERROR') return 'text-pm3-error';
    return '';
  };

  const uniqueProcessNames = [...new Set(processes.map(p => p.name))];

  return (
    <div className="flex flex-col h-full">
      {/* Controls */}
      <div className="flex flex-wrap items-center gap-3 mb-4">
        <select
          value={processFilter}
          onChange={(e) => setProcessFilter(e.target.value)}
          className="bg-slate-900 border border-pm3-border rounded-md px-3 py-2 text-pm3-text text-sm focus:outline-none focus:border-pm3-orange"
        >
          <option value="all">All Processes</option>
          {uniqueProcessNames.map(name => (
            <option key={name} value={name}>{name}</option>
          ))}
        </select>

        <div className="flex gap-1 bg-pm3-surface rounded-lg p-1">
          {LEVEL_TABS.map(level => (
            <button
              key={level}
              onClick={() => setLevelFilter(level)}
              className={`px-3 py-1 rounded text-xs transition-colors ${
                levelFilter === level
                  ? 'bg-pm3-border text-pm3-text'
                  : 'text-pm3-muted hover:text-pm3-text'
              }`}
            >
              {level === 'all' ? 'All' : level}
            </button>
          ))}
        </div>

        <div className="relative flex-1 min-w-[200px]">
          <Search className="absolute left-3 top-1/2 -translate-y-1/2 w-4 h-4 text-pm3-muted" />
          <input
            type="text"
            placeholder="Search logs..."
            value={search}
            onChange={(e) => setSearch(e.target.value)}
            className="w-full bg-slate-900 border border-pm3-border rounded-md pl-9 pr-3 py-2 text-pm3-text text-sm focus:outline-none focus:border-pm3-orange"
          />
        </div>

        <button
          onClick={() => setAutoScroll(!autoScroll)}
          className={`px-3 py-2 rounded-md text-xs transition-colors ${
            autoScroll
              ? 'bg-pm3-success/20 text-pm3-success'
              : 'bg-slate-700 text-pm3-muted'
          }`}
        >
          Auto-scroll: {autoScroll ? 'On' : 'Off'}
        </button>

        <button
          onClick={() => setShowClearConfirm(true)}
          className="flex items-center gap-1.5 px-3 py-2 rounded-md text-xs bg-red-600/20 text-red-400 hover:bg-red-600/30 transition-colors"
        >
          <Trash2 className="w-3.5 h-3.5" />
          Clear
        </button>
      </div>

      {/* Log List */}
      <div className="flex-1 overflow-y-auto bg-slate-900 rounded-lg border border-pm3-border p-3 font-mono text-xs min-h-0">
        {filtered.length === 0 ? (
          <p className="text-pm3-muted text-center py-8">No log entries match your filters</p>
        ) : (
          filtered.map(log => (
            <div
              key={log.id}
              className={`py-0.5 px-1 rounded ${log.level === 'ERROR' ? 'bg-red-500/5' : ''}`}
            >
              <span className="text-pm3-muted">[{log.timestamp}]</span>
              {' '}
              <span className="text-pm3-info">[{log.process}]</span>
              {' '}
              <span className={levelColor(log.level)}>[{log.level.padEnd(5)}]</span>
              {' '}
              <span className={log.level === 'ERROR' ? 'text-pm3-error' : log.level === 'WARN' ? 'text-pm3-warn' : 'text-pm3-text'}>
                {log.message}
              </span>
            </div>
          ))
        )}
        <div ref={logEndRef} />
      </div>

      <ConfirmDialog
        isOpen={showClearConfirm}
        title="Clear All Logs"
        message="Are you sure? This will remove all log entries."
        confirmLabel="Clear Logs"
        confirmVariant="danger"
        onConfirm={() => { setLogs([]); setShowClearConfirm(false); addToast('Logs cleared', 'info'); }}
        onCancel={() => setShowClearConfirm(false)}
      />
    </div>
  );
}
