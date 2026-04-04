# PM3 Dashboard

Web dashboard for PM3, a Rust-based process manager. Shows live process metrics, logs, and management UI.

## Quick Start

```bash
npm install
npm run dev        # http://localhost:5173
```

## Environment Setup

Copy the example env file and configure:

```bash
cp .env.example .env
```

Set `VITE_API_URL` to your PM3 daemon's API endpoint:

```
VITE_API_URL=http://your-daemon-ip:8096/api/v1/
```

**Without `VITE_API_URL`**, the app runs in mock mode with simulated data — useful for development and demos.

## Deploying on a VPS

```bash
cd ~/pm3-dashboard
npm install
cp .env.example .env        # edit VITE_API_URL as needed
npm run build               # outputs to dist/
```

Serve the `dist/` folder with nginx or any static file server.

For development on a VPS:
```bash
npm run dev
# From your local machine, tunnel in:
ssh -L 5173:localhost:5173 user@YOUR_SERVER_IP -N
# Then open http://localhost:5173
```

## First Time Setup

1. Open the dashboard — you'll see "Initialize PM3 Dashboard"
2. Create a password
3. Login with your password

Go to Settings → "Reset Dashboard Setup" to clear everything and start fresh.
