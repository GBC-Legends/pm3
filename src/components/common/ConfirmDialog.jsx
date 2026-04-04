export default function ConfirmDialog({ isOpen, title, message, confirmLabel = 'Confirm', confirmVariant = 'danger', onConfirm, onCancel }) {
  if (!isOpen) return null;

  return (
    <div className="fixed inset-0 z-50 flex items-center justify-center bg-black/50">
      <div className="bg-pm3-surface rounded-lg border border-pm3-border p-6 w-full max-w-sm animate-modal-in">
        <h3 className="text-lg font-semibold text-pm3-text">{title}</h3>
        <p className="text-pm3-muted text-sm mt-2">{message}</p>
        <div className="flex justify-end gap-3 mt-6">
          <button
            onClick={onCancel}
            className="bg-slate-700 hover:bg-slate-600 text-slate-200 rounded-md px-4 py-2 text-sm transition-colors"
          >
            Cancel
          </button>
          <button
            onClick={onConfirm}
            className={`rounded-md px-4 py-2 text-sm text-white transition-colors ${
              confirmVariant === 'danger'
                ? 'bg-red-600 hover:bg-red-700'
                : 'bg-pm3-orange hover:bg-pm3-orange-hover'
            }`}
          >
            {confirmLabel}
          </button>
        </div>
      </div>
    </div>
  );
}
