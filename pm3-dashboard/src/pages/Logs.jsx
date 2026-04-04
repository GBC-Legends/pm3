import { useSearchParams } from 'react-router-dom';
import LogViewer from '../components/logs/LogViewer';

export default function Logs() {
  const [searchParams] = useSearchParams();
  const processFilter = searchParams.get('process') || undefined;

  return (
    <div className="h-full flex flex-col">
      <h1 className="text-2xl font-bold text-pm3-text mb-4">Logs</h1>
      <div className="flex-1 min-h-0">
        <LogViewer initialProcessFilter={processFilter} />
      </div>
    </div>
  );
}
