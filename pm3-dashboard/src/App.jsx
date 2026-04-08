import { Routes, Route } from 'react-router-dom';
import { AppProvider } from './context/AppContext';
import AppShell from './components/layout/AppShell';
import Toast from './components/common/Toast';
import Overview from './pages/Overview';
import Processes from './pages/Processes';
import Logs from './pages/Logs';
import Monitoring from './pages/Monitoring';
import Settings from './pages/Settings';

function AppContent() {
  return (
    <>
      <Toast />
      <Routes>
        <Route path="/" element={<AppShell><Overview /></AppShell>} />
        <Route path="/processes" element={<AppShell><Processes /></AppShell>} />
        <Route path="/logs" element={<AppShell><Logs /></AppShell>} />
        <Route path="/monitoring" element={<AppShell><Monitoring /></AppShell>} />
        <Route path="/settings" element={<AppShell><Settings /></AppShell>} />
      </Routes>
    </>
  );
}

function App() {
  return (
    <AppProvider>
      <AppContent />
    </AppProvider>
  );
}

export default App;
