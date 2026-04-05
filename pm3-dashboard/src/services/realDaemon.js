import { decompress } from "fzstd";

const API_BASE = import.meta.env.VITE_API_URL || "/api/v1/";

let processCache = [];
let logId = 1;

function parseListResponse(text) {
  return text
    .split("\n")
    .map((line) => line.trim())
    .filter(Boolean)
    .map((line) => {
      const match = line.match(/^(\d+):\s*(.+)$/);
      if (!match) return null;
      return {
        id: parseInt(match[1], 10),
        name: match[2].trim(),
        status: "running",
        pid: null,
        cpu: 0,
        ram: 0,
        uptime: null,
        restarts: 0,
        script: null,
      };
    })
    .filter(Boolean);
}

function parseCompressedMetrics(buffer) {
  const compressed = new Uint8Array(buffer);
  let decompressed;
  try {
    decompressed = decompress(compressed);
  } catch {
    return null;
  }

  // Header: 4 bytes magic "PM3M" + 1 byte version + 1 byte record size
  if (decompressed.length < 6) return null;

  const magic = String.fromCharCode(
    decompressed[0],
    decompressed[1],
    decompressed[2],
    decompressed[3],
  );
  if (magic !== "PM3M") return null;

  const recordSize = decompressed[5];
  if (recordSize < 14) return null;

  const dataStart = 6;
  const dataLen = decompressed.length - dataStart;
  const recordCount = Math.floor(dataLen / recordSize);

  if (recordCount === 0) return null;

  // Read the last record for current values
  const lastOffset = dataStart + (recordCount - 1) * recordSize;
  const view = new DataView(
    decompressed.buffer,
    decompressed.byteOffset + lastOffset,
    recordSize,
  );

  const timestamp = view.getUint32(0, true);
  const cpuPermille = view.getUint16(8, true);
  const ramKB = view.getUint16(10, true);

  return {
    timestamp,
    cpu: cpuPermille / 10, // permille -> percentage
    ram: ramKB / 1024, // KB -> MB
  };
}

function parseCompressedMetricsHistory(buffer) {
  const compressed = new Uint8Array(buffer);
  let decompressed;
  try {
    decompressed = decompress(compressed);
  } catch {
    return [];
  }

  if (decompressed.length < 6) return [];

  const magic = String.fromCharCode(
    decompressed[0],
    decompressed[1],
    decompressed[2],
    decompressed[3],
  );
  if (magic !== "PM3M") return [];

  const recordSize = decompressed[5];
  if (recordSize < 14) return [];

  const dataStart = 6;
  const dataLen = decompressed.length - dataStart;
  const recordCount = Math.floor(dataLen / recordSize);

  const records = [];
  for (let i = 0; i < recordCount; i++) {
    const offset = dataStart + i * recordSize;
    const view = new DataView(
      decompressed.buffer,
      decompressed.byteOffset + offset,
      recordSize,
    );
    records.push({
      timestamp: view.getUint32(0, true),
      cpu: view.getUint16(8, true) / 10,
      ram: view.getUint16(10, true) / 1024,
    });
  }

  return records;
}

export const realDaemon = {
  async healthCheck() {
    try {
      const res = await fetch(`${API_BASE}healthz`);
      const text = await res.text();
      return text.trim() === "ok";
    } catch {
      return false;
    }
  },

  async getProcesses() {
    try {
      const res = await fetch(`${API_BASE}list`);
      if (!res.ok) throw new Error(`HTTP ${res.status}`);
      const text = await res.text();
      processCache = parseListResponse(text);
      return [...processCache];
    } catch (err) {
      console.error("Failed to fetch process list:", err);
      return [...processCache]; // return cached if available
    }
  },

  async getMetrics() {
    if (processCache.length === 0) {
      await this.getProcesses();
    }

    const since = Math.floor(Date.now() / 1000) - 10;
    const results = {};

    await Promise.all(
      processCache.map(async (proc) => {
        try {
          const res = await fetch(
            `${API_BASE}get_metrics_compressed/${proc.id}?since=${since}`,
          );
          if (!res.ok) return;
          const buffer = await res.arrayBuffer();
          const parsed = parseCompressedMetrics(buffer);
          if (parsed) {
            results[proc.id] = { cpu: parsed.cpu, ram: parsed.ram };
          }
        } catch {
          // skip this process's metrics
        }
      }),
    );

    return results;
  },

  subscribeToMetrics(callback, intervalMs = 5000) {
    let active = true;

    const tick = async () => {
      if (!active) return;
      try {
        const metrics = await this.getMetrics();
        if (active) callback(metrics);
      } catch {
        // silently retry next interval
      }
    };

    // Initial fetch
    tick();
    const id = setInterval(tick, intervalMs);

    return () => {
      active = false;
      clearInterval(id);
    };
  },

  subscribeToLogs(callback) {
    const controller = new AbortController();
    let active = true;

    const startStream = async () => {
      // Wait for process cache to be populated
      if (processCache.length === 0) {
        await this.getProcesses();
      }

      const params = processCache.map((p) => `p=${p.id}`).join("&");
      const url = `${API_BASE}subscribe_logs?${params}&lines=50`;
      try {
        const res = await fetch(url, { signal: controller.signal });
        if (!res.ok || !res.body) return;

        const reader = res.body.getReader();
        const decoder = new TextDecoder();
        let buffer = "";

        while (active) {
          const { done, value } = await reader.read();
          if (done) break;

          buffer += decoder.decode(value, { stream: true });
          const lines = buffer.split("\n");
          buffer = lines.pop() || ""; // keep incomplete last line

          for (const line of lines) {
            if (!line.trim()) continue;
            const entry = parseLogLine(line);
            if (entry) callback(entry);
          }
        }
      } catch (err) {
        if (err.name !== "AbortError") {
          console.error("Log stream error:", err);
          // Reconnect after delay if still active
          if (active) {
            setTimeout(startStream, 3000);
          }
        }
      }
    };

    function parseLogLine(line) {
      // Format: "{processId} {stdout|stderr} {message}"
      // TODO: daemon should add a timestamp field per line
      const match = line.match(/^(\d+)\s+(stdout|stderr)\s+(.*)$/);
      if (!match) return null;

      const processId = parseInt(match[1], 10);
      const stream = match[2];
      const message = match[3];

      const proc = processCache.find((p) => p.id === processId);

      return {
        id: logId++,
        timestamp: new Date().toLocaleTimeString("en-US", { hour12: false }),
        process: proc ? proc.name : `process-${processId}`,
        level: stream === "stderr" ? "ERROR" : "INFO",
        message,
      };
    }

    startStream();

    return () => {
      active = false;
      controller.abort();
    };
  },

  // Process management — not supported by daemon yet
  startProcess() {
    return Promise.reject(new Error("Not supported by daemon"));
  },
  stopProcess() {
    return Promise.reject(new Error("Not supported by daemon"));
  },
  restartProcess() {
    return Promise.reject(new Error("Not supported by daemon"));
  },
  deleteProcess() {
    return Promise.reject(new Error("Not supported by daemon"));
  },
  addProcess() {
    return Promise.reject(new Error("Not supported by daemon"));
  },
};
