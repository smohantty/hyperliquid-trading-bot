# Project Memory & Context Log

**Last Updated**: 2026-01-10
**Status**: Active Development

## 1. Current Focus
Core Engine and Strategy implementation for Hyperliquid Spot & Perp grid trading. The primary architecture follows the "Engine-Strategy-Broadcaster" separation pattern.

## 2. Recent Key Decisions
- **Engine-Strategy Separation**: Strategies are pure logic that return order requests. The Engine handles all API/networking.
- **Broadcaster Pattern**: Real-time WebSocket server pushes state to external UIs.
- **Telegram Integration**: `TelegramReporter` provides `/status` command and trade notifications.
- **Documentation Policy**: Docs in `docs/` must be updated alongside code changes.

## 3. Known Technical Debt / TODOs

## 4. Work in Progress
None.
