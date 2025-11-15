# Rust Trading System

A comprehensive cryptocurrency trading system built with Rust, featuring backtesting, paper trading, and live trading capabilities.

## Features

- 🔄 **Multiple Exchange Support**: Binance and Gate.io integration
- 📊 **Backtesting Engine**: Test strategies on historical data
- 📈 **Paper Trading**: Simulate trading without real money
- 🌐 **Web Interface**: Next.js-based frontend with real-time visualization
- 🖥️ **Desktop App**: Tauri-based desktop application
- 📡 **HTTP API**: RESTful API for web frontend integration
- 💾 **PostgreSQL Database**: Persistent data storage
- ⚡ **High Performance**: Built with Rust for speed and reliability

## Strategies

- **SMA Strategy**: Simple Moving Average crossover
- **RSI Strategy**: Relative Strength Index based trading
- **Dip Buy Strategy**: Low-frequency strategy for buying dips
- **EMA BTC Week Strategy**: Jesse-based strategy for weekly patterns

## Quick Start

### Prerequisites

- Rust 1.70+
- PostgreSQL
- Node.js 18+
- Redis (optional, for caching)

### Setup

1. **Clone the repository**
   ```bash
   git clone https://github.com/777ukr/rust-trade.git
   cd rust-trade
   ```

2. **Setup database**
   ```bash
   # Create database and user
   sudo -u postgres psql
   CREATE DATABASE trading_core;
   CREATE USER cryptotrader WITH PASSWORD 'cryptotrader';
   GRANT ALL PRIVILEGES ON DATABASE trading_core TO cryptotrader;
   ```

3. **Run migrations**
   ```bash
   psql -U cryptotrader -d trading_core -f config/schema.sql
   ```

4. **Configure**
   ```bash
   # Edit config/development.toml
   # Set your exchange API keys if needed
   ```

5. **Run API server**
   ```bash
   cd trading-core
   export DATABASE_URL="postgresql://cryptotrader:cryptotrader@localhost/trading_core"
   cargo run api
   ```

6. **Run web interface**
   ```bash
   cd frontend
   npm install
   npm run dev
   ```

7. **Open in browser**
   ```
   http://localhost:3000
   ```

## Documentation

- [API Server Guide](API_SERVER_GUIDE.md)
- [Quick Start API](QUICK_START_API.md)
- [Database Setup](DATABASE_SETUP.md)
- [EMA BTC Week Strategy](EMA_BTC_WEEK_STRATEGY.md)
- [Dip Buy Strategy](DIP_BUY_STRATEGY.md)

## Project Structure

```
rust-trade/
├── trading-core/      # Rust backend
│   ├── src/
│   │   ├── api/       # HTTP API endpoints
│   │   ├── backtest/  # Backtesting engine
│   │   ├── data/      # Data repository and cache
│   │   ├── exchange/  # Exchange integrations
│   │   └── live_trading/ # Paper trading
│   └── Cargo.toml
├── frontend/          # Next.js frontend
│   ├── src/
│   │   └── app/       # React components
│   └── package.json
├── src-tauri/         # Tauri desktop app
├── config/            # Configuration files
└── scripts/           # Data import scripts
```

## License

MIT License

Copyright (c) 2025 777ukr
