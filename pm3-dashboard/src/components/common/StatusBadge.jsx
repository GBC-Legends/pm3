import { Loader2 } from 'lucide-react';

const STATUS_STYLES = {
  running:    'bg-green-500/20 text-green-400',
  stopped:    'bg-slate-500/20 text-slate-400',
  error:      'bg-red-500/20 text-red-400',
  starting:   'bg-yellow-500/20 text-yellow-400',
  stopping:   'bg-yellow-500/20 text-yellow-400',
  restarting: 'bg-yellow-500/20 text-yellow-400',
};

const STATUS_LABELS = {
  running: 'Running',
  stopped: 'Stopped',
  error: 'Error',
  starting: 'Starting...',
  stopping: 'Stopping...',
  restarting: 'Restarting...',
};

export default function StatusBadge({ status }) {
  const isTransitioning = ['starting', 'stopping', 'restarting'].includes(status);

  return (
    <span className={`inline-flex items-center gap-1.5 px-2.5 py-0.5 rounded-full text-xs font-medium ${STATUS_STYLES[status] || ''}`}>
      {status === 'running' && (
        <span className="w-1.5 h-1.5 rounded-full bg-green-400 animate-pulse" />
      )}
      {isTransitioning && (
        <Loader2 className="w-3 h-3 animate-spin" />
      )}
      {STATUS_LABELS[status] || status}
    </span>
  );
}
