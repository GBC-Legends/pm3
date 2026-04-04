export default function MetricCard({ title, value, subtitle, accentColor, icon: Icon }) {
  return (
    <div className="bg-pm3-surface rounded-lg border border-pm3-border p-4">
      <div className="flex items-center justify-between mb-2">
        <span className="text-pm3-muted text-sm">{title}</span>
        {Icon && <Icon className="w-4 h-4" style={{ color: accentColor }} />}
      </div>
      <div className="text-2xl font-bold text-pm3-text">{value}</div>
      {subtitle && <div className="text-pm3-muted text-xs mt-1">{subtitle}</div>}
    </div>
  );
}
