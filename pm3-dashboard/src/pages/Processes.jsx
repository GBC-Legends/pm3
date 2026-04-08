import { useState } from 'react';
import { useApp } from '../context/AppContext';
import { useProcesses } from '../hooks/useProcesses';
import ProcessTable from '../components/processes/ProcessTable';

const FILTER_TABS = ['all', 'running', 'stopped', 'error'];

export default function Processes() {
  const { metrics } = useApp();
  const { processes, startProcess, stopProcess, restartProcess, deleteProcess } = useProcesses();
  const [filter, setFilter] = useState('all');

  return (
    <div className="space-y-4">
      {/* Header */}
      <div className="flex items-center justify-between">
        <h1 className="text-2xl font-bold text-pm3-text">Processes</h1>
      </div>

      {/* Filter Tabs */}
      <div className="flex gap-1 bg-pm3-surface rounded-lg p-1 w-fit">
        {FILTER_TABS.map(tab => (
          <button
            key={tab}
            onClick={() => setFilter(tab)}
            className={`px-4 py-1.5 rounded text-sm transition-colors ${
              filter === tab
                ? 'bg-pm3-border text-pm3-text'
                : 'text-pm3-muted hover:text-pm3-text'
            }`}
          >
            {tab.charAt(0).toUpperCase() + tab.slice(1)}
          </button>
        ))}
      </div>

      {/* Process Table */}
      <ProcessTable
        processes={processes}
        filter={filter}
        metrics={metrics}
        onStart={startProcess}
        onStop={stopProcess}
        onRestart={restartProcess}
        onDelete={deleteProcess}
      />
    </div>
  );
}
