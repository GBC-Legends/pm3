import { useState } from 'react';
import { useNavigate } from 'react-router-dom';
import { Eye, EyeOff, Shield } from 'lucide-react';
import { auth } from '../services/auth';
import { useApp } from '../context/AppContext';

export default function Setup() {
  const { addToast } = useApp();
  const navigate = useNavigate();
  const [password, setPassword] = useState('');
  const [confirmPassword, setConfirmPassword] = useState('');
  const [showPassword, setShowPassword] = useState(false);
  const [showConfirm, setShowConfirm] = useState(false);
  const [error, setError] = useState('');

  const isValid = password.length >= 8 && password === confirmPassword;

  const getError = () => {
    if (error) return error;
    if (password.length > 0 && password.length < 8) return 'Password must be at least 8 characters';
    if (confirmPassword.length > 0 && password !== confirmPassword) return 'Passwords do not match';
    return '';
  };

  const handleSubmit = (e) => {
    e.preventDefault();
    if (!isValid) return;

    auth.setupPassword(password);
    addToast('Dashboard initialized!', 'success');
    setTimeout(() => navigate('/login'), 500);
  };

  return (
    <div className="min-h-screen bg-pm3-bg flex items-center justify-center p-4">
      <div className="bg-pm3-surface rounded-lg border border-pm3-border p-8 w-full max-w-md">
        <div className="text-center mb-8">
          <div className="flex items-center justify-center gap-2 mb-4">
            <Shield className="w-8 h-8 text-pm3-orange" />
            <span className="text-3xl font-bold text-pm3-orange">PM3</span>
          </div>
          <h1 className="text-xl font-semibold text-pm3-text mb-2">Initialize PM3 Dashboard</h1>
          <p className="text-pm3-muted text-sm">Create a password to secure your local dashboard</p>
        </div>

        <form onSubmit={handleSubmit} className="space-y-4">
          <div>
            <label className="block text-pm3-muted text-sm mb-1.5">Password</label>
            <div className="relative">
              <input
                type={showPassword ? 'text' : 'password'}
                value={password}
                onChange={(e) => { setPassword(e.target.value); setError(''); }}
                placeholder="Min 8 characters"
                className="w-full bg-slate-900 border border-pm3-border rounded-md px-3 py-2 text-pm3-text focus:outline-none focus:border-pm3-orange pr-10"
              />
              <button
                type="button"
                onClick={() => setShowPassword(!showPassword)}
                className="absolute right-2 top-1/2 -translate-y-1/2 text-pm3-muted hover:text-pm3-text"
              >
                {showPassword ? <EyeOff className="w-4 h-4" /> : <Eye className="w-4 h-4" />}
              </button>
            </div>
          </div>

          <div>
            <label className="block text-pm3-muted text-sm mb-1.5">Confirm Password</label>
            <div className="relative">
              <input
                type={showConfirm ? 'text' : 'password'}
                value={confirmPassword}
                onChange={(e) => { setConfirmPassword(e.target.value); setError(''); }}
                placeholder="Re-enter password"
                className="w-full bg-slate-900 border border-pm3-border rounded-md px-3 py-2 text-pm3-text focus:outline-none focus:border-pm3-orange pr-10"
              />
              <button
                type="button"
                onClick={() => setShowConfirm(!showConfirm)}
                className="absolute right-2 top-1/2 -translate-y-1/2 text-pm3-muted hover:text-pm3-text"
              >
                {showConfirm ? <EyeOff className="w-4 h-4" /> : <Eye className="w-4 h-4" />}
              </button>
            </div>
          </div>

          {getError() && (
            <p className="text-pm3-error text-sm">{getError()}</p>
          )}

          <button
            type="submit"
            disabled={!isValid}
            className="w-full bg-pm3-orange hover:bg-pm3-orange-hover text-white rounded-md px-4 py-2.5 font-medium transition-colors disabled:opacity-50 disabled:cursor-not-allowed"
          >
            Initialize Dashboard
          </button>
        </form>

        <p className="text-pm3-muted text-xs text-center mt-6">
          Your password is stored locally. This dashboard is only accessible on this machine.
        </p>
      </div>
    </div>
  );
}
