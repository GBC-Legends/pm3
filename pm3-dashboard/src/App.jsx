import { Routes, Route } from 'react-router-dom';
import { AppProvider } from './context/AppContext';
import ProtectedRoute from './components/layout/ProtectedRoute';
import AppShell from './components/layout/AppShell';
import Toast from './components/common/Toast';
import Setup from './pages/Setup';
import Login from './pages/Login';
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
        <Route path="/setup" element={<Setup />} />
        <Route path="/login" element={<Login />} />
        <Route path="/" element={
          <ProtectedRoute><AppShell><Overview /></AppShell></ProtectedRoute>
        } />
        <Route path="/processes" element={
          <ProtectedRoute><AppShell><Processes /></AppShell></ProtectedRoute>
        } />
        <Route path="/logs" element={
          <ProtectedRoute><AppShell><Logs /></AppShell></ProtectedRoute>
        } />
        <Route path="/monitoring" element={
          <ProtectedRoute><AppShell><Monitoring /></AppShell></ProtectedRoute>
        } />
        <Route path="/settings" element={
          <ProtectedRoute><AppShell><Settings /></AppShell></ProtectedRoute>
        } />
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
