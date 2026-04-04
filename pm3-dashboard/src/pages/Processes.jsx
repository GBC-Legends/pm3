import { useState } from 'react';
import { Plus } from 'lucide-react';
import { useApp } from '../context/AppContext';
import { useProcesses } from '../hooks/useProcesses';
import ProcessTable from '../components/processes/ProcessTable';
import StartProcessModal from '../components/processes/StartProcessModal';

const FILTER_TABS = ['all', 'running', 'stopped', 'error'];

export default function Processes() {
  const { metrics } = useApp();
  const { processes, startProcess, stopProcess, restartProcess, deleteProcess, addProcess } = useProcesses();
  const [filter, setFilter] = useState('all');
  const [showStartModal, setShowStartModal] = useState(false);

  return (
    <div className="space-y-4">
      {/* Header */}
      <div className="flex items-center justify-between">
        <h1 className="text-2xl font-bold text-pm3-text">Processes</h1>
        <button
          onClick={() => setShowStartModal(true)}
          className="flex items-center gap-2 bg-pm3-orange hover:bg-pm3-orange-hover text-white rounded-md px-4 py-2 text-sm font-medium transition-colors"
        >
          <Plus className="w-4 h-4" />
          Start New Process
        </button>
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

      {/* Start Process Modal */}
      {showStartModal && (
        <StartProcessModal
          onSubmit={addProcess}
          onClose={() => setShowStartModal(false)}
        />
      )}
    </div>
  );
}
