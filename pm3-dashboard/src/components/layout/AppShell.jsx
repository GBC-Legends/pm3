import Sidebar from './Sidebar';
import TopBar from './TopBar';

export default function AppShell({ children }) {
  return (
    <div className="flex h-screen bg-pm3-bg overflow-hidden">
      <Sidebar />
      <div className="flex-1 flex flex-col overflow-hidden">
        <TopBar />
        <main className="flex-1 overflow-y-auto p-6">
          {children}
        </main>
      </div>
    </div>
  );
}
