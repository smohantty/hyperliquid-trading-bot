# Hyperliquid Trading Bot Frontend

A real-time dashboard for the Hyperliquid Trading Bot, built with React, TypeScript, and Vite.

## 🚀 Quick Start

### 1. Same Machine (Default)
If you are running the bot locally on port 9000 (default), just run:

```bash
npm install
npm run dev
```
Open `http://localhost:5173`. It will connect to `ws://localhost:9000`.

### 2. Remote Machine (Accessing from Laptop)
If the bot is running on a remote server (e.g., `192.168.0.25`) and you are running the frontend locally:

1. Create a `.env.local` file in this directory.
2. Add your remote bot URL:
   ```ini
   VITE_WS_URL=ws://192.168.0.25:9000
   ```
3. Run `npm run dev`.

### 3. Custom Port (Same Machine)
If you changed the bot to run on a different port (e.g., 9001):

1. Create a `.env.local` file.
2. Add the port override:
   ```ini
   VITE_WS_PORT=9001
   ```
3. Run `npm run dev`.

## 📦 Building for Production

To build the static files for hosting (e.g., to serve them directly from the remote machine):

```bash
npm run build
```

The output will be in the `dist` folder. You can test it locally with:
```bash
npx serve -s dist
```

## 🔧 Configuration Options

| Environment Variable | Description | Example |
|---------------------|-------------|---------|
| `VITE_WS_URL` | Overrides the entire WebSocket connection URL. Use this for connecting to a different machine. | `ws://192.168.1.100:9000` |
| `VITE_WS_PORT` | Overrides only the port. Keeps the hostname dynamic (matches the browser URL). | `9001` |
