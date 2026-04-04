import ProcessRow from './ProcessRow';

export default function ProcessTable({ processes, filter, metrics, onStart, onStop, onRestart, onDelete }) {
  const filtered = filter === 'all' ? processes : processes.filter(p => p.status === filter);

  return (
    <div className="bg-pm3-surface rounded-lg border border-pm3-border overflow-x-auto">
      <table className="w-full text-sm">
        <thead>
          <tr className="text-pm3-muted text-xs border-b border-pm3-border">
            <th className="text-left py-3 px-2 font-medium">#</th>
            <th className="text-left py-3 px-2 font-medium">Name</th>
            <th className="text-left py-3 px-2 font-medium">Status</th>
            <th className="text-left py-3 px-2 font-medium">PID</th>
            <th className="text-right py-3 px-2 font-medium">CPU%</th>
            <th className="text-right py-3 px-2 font-medium">RAM (MB)</th>
            <th className="text-left py-3 px-2 font-medium">Uptime</th>
            <th className="text-center py-3 px-2 font-medium">Restarts</th>
            <th className="text-right py-3 px-2 font-medium">Actions</th>
          </tr>
        </thead>
        <tbody>
          {filtered.length === 0 ? (
            <tr>
              <td colSpan={9} className="py-8 text-center text-pm3-muted">
                No processes found
              </td>
            </tr>
          ) : (
            filtered.map(p => (
              <ProcessRow
                key={p.id}
                process={p}
                currentMetrics={metrics[p.id]}
                onStart={onStart}
                onStop={onStop}
                onRestart={onRestart}
                onDelete={onDelete}
              />
            ))
          )}
        </tbody>
      </table>
    </div>
  );
}
