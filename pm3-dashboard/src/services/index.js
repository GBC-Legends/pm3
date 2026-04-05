import { mockDaemon } from './mockDaemon.js';
import { realDaemon } from './realDaemon.js';

const apiUrl = import.meta.env.VITE_API_URL;

export const daemon = apiUrl ? realDaemon : mockDaemon;
export const daemonCapabilities = { canManageProcesses: !apiUrl };
