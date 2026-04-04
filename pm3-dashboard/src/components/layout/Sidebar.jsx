import { useLocation, useNavigate } from 'react-router-dom';
import { LayoutDashboard, Server, FileText, Activity, Settings, ChevronLeft, ChevronRight } from 'lucide-react';
import { useApp } from '../../context/AppContext';

const NAV_ITEMS = [
  { path: '/', label: 'Overview', icon: LayoutDashboard },
  { path: '/processes', label: 'Processes', icon: Server },
  { path: '/logs', label: 'Logs', icon: FileText },
  { path: '/monitoring', label: 'Monitoring', icon: Activity },
  { path: '/settings', label: 'Settings', icon: Settings },
];

export default function Sidebar() {
  const { sidebarCollapsed, setSidebarCollapsed } = useApp();
  const location = useLocation();
  const navigate = useNavigate();

  return (
    <div
      className={`h-screen bg-slate-900 border-r border-pm3-border flex flex-col transition-all duration-250 ease-in-out ${
        sidebarCollapsed ? 'w-16' : 'w-60'
      }`}
    >
      {/* Header */}
      <div className="flex items-center justify-between p-4 border-b border-pm3-border">
        {!sidebarCollapsed && (
          <span className="text-xl font-bold text-pm3-orange">PM3</span>
        )}
        <button
          onClick={() => setSidebarCollapsed(!sidebarCollapsed)}
          className="text-pm3-muted hover:text-pm3-text p-1 rounded hover:bg-pm3-hover transition-colors"
        >
          {sidebarCollapsed ? <ChevronRight className="w-4 h-4" /> : <ChevronLeft className="w-4 h-4" />}
        </button>
      </div>

      {/* Navigation */}
      <nav className="flex-1 py-2">
        {NAV_ITEMS.map(item => {
          const isActive = location.pathname === item.path;
          return (
            <button
              key={item.path}
              onClick={() => navigate(item.path)}
              className={`w-full flex items-center gap-3 px-4 py-2.5 text-sm transition-colors ${
                isActive
                  ? 'border-l-2 border-pm3-orange bg-slate-800/50 text-pm3-orange'
                  : 'border-l-2 border-transparent text-pm3-muted hover:bg-slate-800 hover:text-pm3-text'
              }`}
              title={sidebarCollapsed ? item.label : undefined}
            >
              <item.icon className="w-5 h-5 flex-shrink-0" />
              {!sidebarCollapsed && <span>{item.label}</span>}
            </button>
          );
        })}
      </nav>

      {/* Version */}
      {!sidebarCollapsed && (
        <div className="p-4 border-t border-pm3-border">
          <span className="text-pm3-muted text-xs">PM3 v0.1.0-alpha</span>
        </div>
      )}
    </div>
  );
}
