import { useApp } from '../../context/AppContext';
import { CheckCircle, AlertCircle, Info } from 'lucide-react';

const ICONS = {
  success: CheckCircle,
  error: AlertCircle,
  info: Info,
};

const BORDER_COLORS = {
  success: 'border-pm3-success',
  error: 'border-pm3-error',
  info: 'border-pm3-info',
};

const ICON_COLORS = {
  success: 'text-pm3-success',
  error: 'text-pm3-error',
  info: 'text-pm3-info',
};

export default function Toast() {
  const { toasts } = useApp();

  if (toasts.length === 0) return null;

  return (
    <div className="fixed top-4 right-4 z-50 flex flex-col gap-2">
      {toasts.map(toast => {
        const Icon = ICONS[toast.type] || Info;
        return (
          <div
            key={toast.id}
            className={`bg-pm3-surface border-l-4 ${BORDER_COLORS[toast.type] || BORDER_COLORS.info} rounded-md px-4 py-3 text-pm3-text text-sm shadow-lg animate-slide-in flex items-center gap-2`}
          >
            <Icon className={`w-4 h-4 flex-shrink-0 ${ICON_COLORS[toast.type] || ICON_COLORS.info}`} />
            {toast.message}
          </div>
        );
      })}
    </div>
  );
}
