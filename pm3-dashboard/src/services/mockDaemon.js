const INITIAL_PROCESSES = [
  { id: 1, name: 'web-server',   status: 'running', pid: 12453, cpu: 3.2,  ram: 45,  uptime: '2d 14h', restarts: 0,  script: './server.js' },
  { id: 2, name: 'api-service',  status: 'running', pid: 12891, cpu: 7.8,  ram: 89,  uptime: '2d 14h', restarts: 2,  script: './api/index.js' },
  { id: 3, name: 'worker-queue', status: 'running', pid: 13204, cpu: 1.1,  ram: 32,  uptime: '1d 6h',  restarts: 0,  script: './workers/queue.js' },
  { id: 4, name: 'scheduler',    status: 'running', pid: 13501, cpu: 0.3,  ram: 18,  uptime: '1d 6h',  restarts: 0,  script: './cron.js' },
  { id: 5, name: 'legacy-cron',  status: 'stopped', pid: null,  cpu: 0,    ram: 0,   uptime: null,     restarts: 5,  script: './old/cron.sh' },
  { id: 6, name: 'test-server',  status: 'error',   pid: null,  cpu: 0,    ram: 0,   uptime: null,     restarts: 12, script: './test/server.js' },
];

const BASE_METRICS = {
  1: { cpu: 3.2, ram: 45 },
  2: { cpu: 7.8, ram: 89 },
  3: { cpu: 1.1, ram: 32 },
  4: { cpu: 0.3, ram: 18 },
};

const LOG_TEMPLATES = {
  'web-server': {
    INFO: [
      'GET /api/health 200 3ms',
      'GET /dashboard 304 1ms',
      'GET /api/users 200 12ms',
      'POST /api/login 200 45ms',
      'GET /static/bundle.js 304 0ms',
      'GET /api/metrics 200 8ms',
    ],
    WARN: [
      'Response time above threshold: 320ms',
      'Connection pool nearing capacity: 85%',
      'Rate limit approaching for IP 192.168.1.50',
    ],
    ERROR: [
      'Failed to serve static asset: ENOENT',
      'TLS handshake timeout with upstream',
    ],
  },
  'api-service': {
    INFO: [
      'Processing job #' + Math.floor(Math.random() * 9000 + 1000),
      'Reconnected successfully',
      'Cache hit ratio: 94.2%',
      'Request processed in 23ms',
      'Batch insert completed: 150 rows',
    ],
    WARN: [
      'DB connection timeout, retrying...',
      'Response time above threshold: 450ms',
      'Memory usage above 80%',
      'Slow query detected: 890ms',
    ],
    ERROR: [
      'DB connection timeout, retrying...',
      'Unhandled promise rejection in /api/orders',
      'Redis connection lost, reconnecting...',
    ],
  },
  'worker-queue': {
    INFO: [
      'Job completed: email-send #' + Math.floor(Math.random() * 9000 + 1000),
      'Queue depth: ' + Math.floor(Math.random() * 30),
      'Processing batch: 12 items',
      'Job completed: report-generate #' + Math.floor(Math.random() * 500),
      'Worker heartbeat OK',
    ],
    WARN: [
      'Queue depth exceeding threshold: 45',
      'Job retry attempt 2/3: pdf-export #331',
    ],
    ERROR: [
      'Job failed permanently: image-resize #892',
      'Dead letter queue overflow',
    ],
  },
  'scheduler': {
    INFO: [
      'Running scheduled task: cleanup',
      'Task completed in 120ms',
      'Running scheduled task: db-backup',
      'Cron tick: next run in 300s',
      'Running scheduled task: cache-invalidation',
    ],
    WARN: [
      'Task overdue by 15s: log-rotation',
      'Skipping duplicate task execution',
    ],
    ERROR: [
      'Scheduled task failed: cleanup — EPERM',
    ],
  },
};

let processes = JSON.parse(JSON.stringify(INITIAL_PROCESSES));
let nextId = 7;
let logId = 1;
let currentMetrics = {};

// Initialize metrics for running processes
processes.forEach(p => {
  if (p.status === 'running' && BASE_METRICS[p.id]) {
    currentMetrics[p.id] = { cpu: p.cpu, ram: p.ram };
  }
});

function nudge(current, base, variance = 3) {
  return Math.max(0.1, Math.min(base * 4, current + (Math.random() - 0.5) * variance));
}

function randomPid() {
  return Math.floor(10000 + Math.random() * 50000);
}

function pickLevel() {
  const r = Math.random();
  if (r < 0.7) return 'INFO';
  if (r < 0.9) return 'WARN';
  return 'ERROR';
}

function generateLogEntry() {
  const runningProcesses = processes.filter(p => p.status === 'running');
  if (runningProcesses.length === 0) {
    return {
      id: logId++,
      timestamp: new Date().toLocaleTimeString('en-US', { hour12: false }),
      process: 'system',
      level: 'WARN',
      message: 'No running processes',
    };
  }

  const proc = runningProcesses[Math.floor(Math.random() * runningProcesses.length)];
  const level = pickLevel();
  const templates = LOG_TEMPLATES[proc.name]?.[level] || ['Activity logged'];
  const message = templates[Math.floor(Math.random() * templates.length)];

  // Regenerate dynamic values
  let finalMessage = message;
  if (message.includes('#')) {
    finalMessage = message.replace(/#\d+/, '#' + Math.floor(Math.random() * 9000 + 1000));
  }
  if (message.includes('Queue depth:')) {
    finalMessage = 'Queue depth: ' + Math.floor(Math.random() * 50);
  }

  return {
    id: logId++,
    timestamp: new Date().toLocaleTimeString('en-US', { hour12: false }),
    process: proc.name,
    level,
    message: finalMessage,
  };
}

export const mockDaemon = {
  getProcesses() {
    return [...processes];
  },

  getMetricsHistory(sinceSec = 300) {
    const history = {};
    const now = Date.now();
    const intervalMs = 5000;
    const points = Math.floor(sinceSec / (intervalMs / 1000));

    processes.forEach(p => {
      if (p.status === 'running' && BASE_METRICS[p.id]) {
        const base = BASE_METRICS[p.id];
        let cpu = base.cpu;
        let ram = base.ram;
        const entries = [];

        for (let i = 0; i < points; i++) {
          cpu = nudge(cpu, base.cpu, 3);
          ram = nudge(ram, base.ram, 8);
          const t = new Date(now - (points - i) * intervalMs);
          entries.push({
            time: t.toLocaleTimeString('en-US', { hour12: false }),
            cpu: Math.round(cpu * 10) / 10,
            ram: Math.round(ram * 10) / 10,
          });
        }

        history[p.id] = entries;
      }
    });

    return history;
  },

  startProcess(id) {
    const proc = processes.find(p => p.id === id);
    if (!proc) return Promise.reject(new Error('Process not found'));
    proc.status = 'starting';
    return new Promise(resolve => {
      setTimeout(() => {
        proc.status = 'running';
        proc.pid = randomPid();
        proc.uptime = '0s';
        proc.cpu = BASE_METRICS[id]?.cpu || 1.0;
        proc.ram = BASE_METRICS[id]?.ram || 20;
        currentMetrics[id] = { cpu: proc.cpu, ram: proc.ram };
        resolve(proc);
      }, 1500);
    });
  },

  stopProcess(id) {
    const proc = processes.find(p => p.id === id);
    if (!proc) return Promise.reject(new Error('Process not found'));
    proc.status = 'stopping';
    return new Promise(resolve => {
      setTimeout(() => {
        proc.status = 'stopped';
        proc.pid = null;
        proc.cpu = 0;
        proc.ram = 0;
        proc.uptime = null;
        delete currentMetrics[id];
        resolve(proc);
      }, 1000);
    });
  },

  restartProcess(id) {
    const proc = processes.find(p => p.id === id);
    if (!proc) return Promise.reject(new Error('Process not found'));
    proc.status = 'restarting';
    return new Promise(resolve => {
      setTimeout(() => {
        proc.status = 'running';
        proc.pid = randomPid();
        proc.restarts += 1;
        proc.uptime = '0s';
        proc.cpu = BASE_METRICS[id]?.cpu || 1.0;
        proc.ram = BASE_METRICS[id]?.ram || 20;
        currentMetrics[id] = { cpu: proc.cpu, ram: proc.ram };
        resolve(proc);
      }, 1500);
    });
  },

  deleteProcess(id) {
    processes = processes.filter(p => p.id !== id);
    delete currentMetrics[id];
  },

  addProcess(config) {
    const newProc = {
      id: nextId++,
      name: config.name,
      status: 'stopped',
      pid: null,
      cpu: 0,
      ram: 0,
      uptime: null,
      restarts: 0,
      script: config.script || './' + config.name + '.js',
    };
    processes.push(newProc);
    BASE_METRICS[newProc.id] = { cpu: 1.0 + Math.random() * 3, ram: 15 + Math.random() * 40 };
    return this.startProcess(newProc.id);
  },

  getMetrics() {
    processes.forEach(p => {
      if (p.status === 'running') {
        const base = BASE_METRICS[p.id] || { cpu: 2, ram: 30 };
        const prev = currentMetrics[p.id] || { cpu: base.cpu, ram: base.ram };
        currentMetrics[p.id] = {
          cpu: nudge(prev.cpu, base.cpu, 3),
          ram: nudge(prev.ram, base.ram, 8),
        };
      }
    });
    return { ...currentMetrics };
  },

  subscribeToMetrics(callback, intervalMs = 5000) {
    const id = setInterval(() => {
      callback(this.getMetrics());
    }, intervalMs);
    return () => clearInterval(id);
  },

  subscribeToLogs(callback) {
    let timeoutId;
    const tick = () => {
      const entry = generateLogEntry();
      callback(entry);
      const delay = 3000 + Math.random() * 2000;
      timeoutId = setTimeout(tick, delay);
    };
    const delay = 1000 + Math.random() * 2000;
    timeoutId = setTimeout(tick, delay);
    return () => clearTimeout(timeoutId);
  },
};
