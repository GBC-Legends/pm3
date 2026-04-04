import { useState } from 'react';
import { useNavigate } from 'react-router-dom';
import { Eye, EyeOff, Shield } from 'lucide-react';
import { auth } from '../services/auth';

export default function Login() {
  const navigate = useNavigate();
  const [password, setPassword] = useState('');
  const [showPassword, setShowPassword] = useState(false);
  const [error, setError] = useState(false);
  const [shaking, setShaking] = useState(false);

  const handleSubmit = (e) => {
    e.preventDefault();
    if (auth.login(password)) {
      navigate('/');
    } else {
      setError(true);
      setShaking(true);
      setTimeout(() => setShaking(false), 400);
    }
  };

  const handleReset = () => {
    auth.resetSetup();
    navigate('/setup');
  };

  return (
    <div className="min-h-screen bg-pm3-bg flex items-center justify-center p-4">
      <div className="bg-pm3-surface rounded-lg border border-pm3-border p-8 w-full max-w-md">
        <div className="text-center mb-8">
          <div className="flex items-center justify-center gap-2 mb-4">
            <Shield className="w-8 h-8 text-pm3-orange" />
            <span className="text-3xl font-bold text-pm3-orange">PM3</span>
          </div>
          <h1 className="text-xl font-semibold text-pm3-text mb-2">PM3 Dashboard</h1>
          <p className="text-pm3-muted text-sm mb-4">Enter your dashboard password</p>
          <div className="flex items-center justify-center gap-2 text-xs text-pm3-muted">
            <span className="inline-block w-2 h-2 rounded-full bg-pm3-success animate-pulse" />
            Daemon connected · {import.meta.env.VITE_API_URL ? 'Remote' : 'Mock Mode'}
          </div>
        </div>

        <form onSubmit={handleSubmit} className="space-y-4">
          <div className={shaking ? 'animate-shake' : ''}>
            <label className="block text-pm3-muted text-sm mb-1.5">Password</label>
            <div className="relative">
              <input
                type={showPassword ? 'text' : 'password'}
                value={password}
                onChange={(e) => { setPassword(e.target.value); setError(false); }}
                placeholder="Enter password"
                className={`w-full bg-slate-900 border rounded-md px-3 py-2 text-pm3-text focus:outline-none pr-10 ${
                  error ? 'border-pm3-error focus:border-pm3-error' : 'border-pm3-border focus:border-pm3-orange'
                }`}
                autoFocus
              />
              <button
                type="button"
                onClick={() => setShowPassword(!showPassword)}
                className="absolute right-2 top-1/2 -translate-y-1/2 text-pm3-muted hover:text-pm3-text"
              >
                {showPassword ? <EyeOff className="w-4 h-4" /> : <Eye className="w-4 h-4" />}
              </button>
            </div>
            {error && (
              <p className="text-pm3-error text-sm mt-1.5">Incorrect password</p>
            )}
          </div>

          <button
            type="submit"
            className="w-full bg-pm3-orange hover:bg-pm3-orange-hover text-white rounded-md px-4 py-2.5 font-medium transition-colors"
          >
            Connect
          </button>
        </form>

        <p className="text-center mt-6">
          <button
            onClick={handleReset}
            className="text-pm3-muted text-xs hover:text-pm3-text underline cursor-pointer"
          >
            Reset setup
          </button>
        </p>
      </div>
    </div>
  );
}
