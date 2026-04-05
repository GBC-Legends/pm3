import { BarChart, Bar, XAxis, YAxis, Tooltip, Legend, ResponsiveContainer } from 'recharts';
import { useApp } from '../context/AppContext';
import CpuChart from '../components/monitoring/CpuChart';
import RamChart from '../components/monitoring/RamChart';

const TOOLTIP_STYLE = { background: '#1E293B', border: '1px solid #334155', borderRadius: '8px' };
const TICK_STYLE = { fill: '#94A3B8', fontSize: 11 };

export default function Monitoring() {
  const { processes, metrics } = useApp();
  const running = processes.filter(p => p.status === 'running');

  const barData = running.map(p => ({
    name: p.name,
    cpu: parseFloat((metrics[p.id]?.cpu ?? p.cpu).toFixed(1)),
    ram: parseFloat((metrics[p.id]?.ram ?? p.ram).toFixed(0)),
  }));

  return (
    <div className="space-y-6">
      <h1 className="text-2xl font-bold text-pm3-text">Monitoring</h1>

      <CpuChart />
      <RamChart />

      {/* Per-Process Bar Chart */}
      <div className="bg-pm3-surface rounded-lg border border-pm3-border p-4">
        <h3 className="text-pm3-text font-semibold mb-4">Current Per-Process Usage</h3>
        <ResponsiveContainer width="100%" height={250}>
          <BarChart data={barData} layout="vertical">
            <XAxis type="number" tick={TICK_STYLE} axisLine={false} tickLine={false} />
            <YAxis type="category" dataKey="name" tick={TICK_STYLE} axisLine={false} tickLine={false} width={120} />
            <Tooltip contentStyle={TOOLTIP_STYLE} />
            <Legend />
            <Bar dataKey="cpu" fill="#CE412B" name="CPU %" radius={[0, 4, 4, 0]} />
            <Bar dataKey="ram" fill="#3B82F6" name="RAM (MB)" radius={[0, 4, 4, 0]} />
          </BarChart>
        </ResponsiveContainer>
      </div>
    </div>
  );
}
