import { useMemo } from 'react';
import { LineChart, Line, XAxis, YAxis, Tooltip, Legend, ResponsiveContainer } from 'recharts';
import { useApp } from '../../context/AppContext';

const PROCESS_COLORS = ['#CE412B', '#3B82F6', '#10B981', '#F59E0B', '#8B5CF6', '#EC4899', '#06B6D4', '#84CC16'];
const TOOLTIP_STYLE = { background: '#1E293B', border: '1px solid #334155', borderRadius: '8px' };
const TICK_STYLE = { fill: '#94A3B8', fontSize: 11 };

export default function RamChart() {
  const { processes, metricsHistory } = useApp();
  const runningProcesses = processes.filter(p => p.status === 'running');

  const chartData = useMemo(() => {
    if (runningProcesses.length === 0) return [];
    const firstId = runningProcesses[0].id;
    const history = metricsHistory[firstId] || [];
    return history.map((entry, i) => {
      const point = { time: entry.time };
      runningProcesses.forEach(p => {
        const h = metricsHistory[p.id];
        point[p.name] = h?.[i]?.ram ?? 0;
      });
      return point;
    });
  }, [runningProcesses, metricsHistory]);

  return (
    <div className="bg-pm3-surface rounded-lg border border-pm3-border p-4">
      <h3 className="text-pm3-text font-semibold mb-4">Memory Usage Over Time</h3>
      <ResponsiveContainer width="100%" height={250}>
        <LineChart data={chartData}>
          <XAxis dataKey="time" tick={TICK_STYLE} axisLine={false} tickLine={false} />
          <YAxis domain={[0, 500]} tick={TICK_STYLE} axisLine={false} tickLine={false} unit=" MB" />
          <Tooltip contentStyle={TOOLTIP_STYLE} />
          <Legend />
          {runningProcesses.map((proc, i) => (
            <Line
              key={proc.id}
              type="monotone"
              dataKey={proc.name}
              stroke={PROCESS_COLORS[i % PROCESS_COLORS.length]}
              dot={false}
              strokeWidth={2}
            />
          ))}
        </LineChart>
      </ResponsiveContainer>
    </div>
  );
}
