# Hyperliquid Trading Bot Frontend & Desktop App

A real-time dashboard for the Hyperliquid Trading Bot, built with React, TypeScript, Vite, and Electron.

## 🌟 Features

-   **Multi-Bot Support**: Connect to and manage multiple trading bots simultaneously via tabs.
-   **Persistent Connections**: Bot configurations are saved locally and restored on launch.
-   **Real-time Data**: WebSocket integration for live updates of orders, positions, and market data.
-   **Desktop Experience**: Packaged as a standalone Electron application.

---

## 🚀 Quick Start (Web Mode)

For quick development in a browser environment:

```bash
npm install
npm run dev
```
Open `http://localhost:5173`.

---

## 🖥️ Desktop App (Electron)

### Development
To run the application in Electron mode during development (with hot reload):

```bash
npm run electron:dev
```
This spawns the Vite dev server and opens an Electron window pointing to it.

### Release (Build)
To package the application for distribution (creates an AppImage on Linux):

```bash
npm run electron:build
```
The output artifacts (e.g., `.AppImage`) will be located in the `dist` folder.

---


## 🔌 Connection Management

The app supports combining multiple bot instances (e.g., Hyperliquid on port 9000, Lighter on port 9001) into a single dashboard.

### Default Behavior
On the very first launch, the app automatically creates a default connection to:
`ws://localhost:9000`

### Adding a New Bot
1.  Click the **"Manage Bots"** button in the top right corner.
2.  Enter a **Name** for your bot (e.g., "Lighter Bot").
3.  Enter the **WebSocket URL** (e.g., `ws://localhost:9001`).
4.  Click **"Add Connection"**.

### Persistence
Your configured connections are saved automatically.
*   **Web**: Saved in browser `localStorage`.
*   **Desktop**: Saved in the application's user data directory (`~/.config/Hyperliquid Dashboard/Local Storage/` on Linux).

