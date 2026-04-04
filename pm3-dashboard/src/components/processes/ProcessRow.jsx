import { useState } from 'react';
import { useNavigate } from 'react-router-dom';
import { Play, Square, RefreshCw, Trash2, FileText } from 'lucide-react';
import StatusBadge from '../common/StatusBadge';
import ConfirmDialog from '../common/ConfirmDialog';

export default function ProcessRow({ process, onStart, onStop, onRestart, onDelete, currentMetrics }) {
  const navigate = useNavigate();
  const [showConfirm, setShowConfirm] = useState(false);
  const isTransitioning = ['starting', 'stopping', 'restarting'].includes(process.status);

  const cpu = currentMetrics?.cpu ?? process.cpu;
  const ram = currentMetrics?.ram ?? process.ram;

  const ActionButton = ({ icon: Icon, label, onClick, variant = 'default' }) => (
    <button
      onClick={onClick}
      disabled={isTransitioning}
      title={label}
      className={`p-1.5 rounded text-xs transition-colors disabled:opacity-30 disabled:cursor-not-allowed ${
        variant === 'danger'
          ? 'text-red-400 hover:bg-red-500/10 hover:text-red-300'
          : 'text-pm3-muted hover:bg-pm3-hover hover:text-pm3-text'
      }`}
    >
      <Icon className="w-3.5 h-3.5" />
    </button>
  );

  return (
    <>
      <tr className="border-b border-pm3-border/50 hover:bg-pm3-hover/20">
        <td className="py-2.5 px-2 text-pm3-muted text-xs">{process.id}</td>
        <td className="py-2.5 px-2 text-pm3-text font-mono text-xs">{process.name}</td>
        <td className="py-2.5 px-2"><StatusBadge status={process.status} /></td>
        <td className="py-2.5 px-2 text-pm3-text font-mono text-xs">{process.pid || '—'}</td>
        <td className="py-2.5 px-2 text-right text-pm3-text font-mono text-xs">
          {process.status === 'running' ? cpu.toFixed(1) + '%' : '—'}
        </td>
        <td className="py-2.5 px-2 text-right text-pm3-text font-mono text-xs">
          {process.status === 'running' ? ram.toFixed(0) + ' MB' : '—'}
        </td>
        <td className="py-2.5 px-2 text-pm3-muted text-xs">{process.uptime || '—'}</td>
        <td className="py-2.5 px-2 text-center text-pm3-muted text-xs">{process.restarts}</td>
        <td className="py-2.5 px-2">
          <div className="flex items-center gap-1 justify-end">
            {process.status === 'running' && (
              <>
                <ActionButton icon={Square} label="Stop" onClick={() => onStop(process.id)} />
                <ActionButton icon={RefreshCw} label="Restart" onClick={() => onRestart(process.id)} />
                <ActionButton icon={FileText} label="View Logs" onClick={() => navigate(`/logs?process=${process.name}`)} />
              </>
            )}
            {process.status === 'stopped' && (
              <>
                <ActionButton icon={Play} label="Start" onClick={() => onStart(process.id)} />
                <ActionButton icon={Trash2} label="Delete" onClick={() => setShowConfirm(true)} variant="danger" />
              </>
            )}
            {process.status === 'error' && (
              <>
                <ActionButton icon={RefreshCw} label="Restart" onClick={() => onRestart(process.id)} />
                <ActionButton icon={FileText} label="View Logs" onClick={() => navigate(`/logs?process=${process.name}`)} />
                <ActionButton icon={Trash2} label="Delete" onClick={() => setShowConfirm(true)} variant="danger" />
              </>
            )}
          </div>
        </td>
      </tr>

      <ConfirmDialog
        isOpen={showConfirm}
        title="Delete Process"
        message={`Are you sure you want to delete "${process.name}"? This cannot be undone.`}
        confirmLabel="Delete"
        confirmVariant="danger"
        onConfirm={() => { onDelete(process.id); setShowConfirm(false); }}
        onCancel={() => setShowConfirm(false)}
      />
    </>
  );
}
