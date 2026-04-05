import { useState } from 'react';
import { X } from 'lucide-react';

export default function StartProcessModal({ onSubmit, onClose }) {
  const [name, setName] = useState('');
  const [script, setScript] = useState('');
  const [autoRestart, setAutoRestart] = useState(true);

  const handleSubmit = (e) => {
    e.preventDefault();
    if (!name.trim()) return;
    onSubmit({ name: name.trim(), script: script.trim() || `./${name.trim()}.js`, autoRestart });
    onClose();
  };

  return (
    <div className="fixed inset-0 z-50 flex items-center justify-center bg-black/50">
      <div className="bg-pm3-surface rounded-lg border border-pm3-border p-6 w-full max-w-md animate-modal-in">
        <div className="flex items-center justify-between mb-4">
          <h3 className="text-lg font-semibold text-pm3-text">Start New Process</h3>
          <button onClick={onClose} className="text-pm3-muted hover:text-pm3-text p-1">
            <X className="w-4 h-4" />
          </button>
        </div>

        <form onSubmit={handleSubmit} className="space-y-4">
          <div>
            <label className="block text-pm3-muted text-sm mb-1.5">Process Name</label>
            <input
              type="text"
              value={name}
              onChange={(e) => setName(e.target.value)}
              placeholder="e.g. my-service"
              className="w-full bg-slate-900 border border-pm3-border rounded-md px-3 py-2 text-pm3-text text-sm focus:outline-none focus:border-pm3-orange"
              autoFocus
            />
          </div>

          <div>
            <label className="block text-pm3-muted text-sm mb-1.5">Script Path</label>
            <input
              type="text"
              value={script}
              onChange={(e) => setScript(e.target.value)}
              placeholder="e.g. ./server.js"
              className="w-full bg-slate-900 border border-pm3-border rounded-md px-3 py-2 text-pm3-text text-sm focus:outline-none focus:border-pm3-orange font-mono"
            />
          </div>

          <div className="flex items-center justify-between">
            <label className="text-pm3-muted text-sm">Auto-restart</label>
            <button
              type="button"
              onClick={() => setAutoRestart(!autoRestart)}
              className={`relative w-10 h-5 rounded-full transition-colors ${autoRestart ? 'bg-pm3-success' : 'bg-slate-600'}`}
            >
              <span className={`absolute top-0.5 w-4 h-4 rounded-full bg-white transition-transform ${autoRestart ? 'left-5.5' : 'left-0.5'}`} />
            </button>
          </div>

          <div className="flex gap-3 pt-2">
            <button
              type="button"
              onClick={onClose}
              className="flex-1 bg-slate-700 hover:bg-slate-600 text-slate-200 rounded-md px-4 py-2 text-sm transition-colors"
            >
              Cancel
            </button>
            <button
              type="submit"
              disabled={!name.trim()}
              className="flex-1 bg-pm3-orange hover:bg-pm3-orange-hover text-white rounded-md px-4 py-2 text-sm font-medium transition-colors disabled:opacity-50 disabled:cursor-not-allowed"
            >
              Start Process
            </button>
          </div>
        </form>
      </div>
    </div>
  );
}
