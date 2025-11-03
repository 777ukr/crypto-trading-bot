//! Backtest Engine - Полноценный бэктестер с эмуляцией случайностей
//! Реализует философию MoonBot: статистический эмулятор, а не детерминированный симулятор

pub mod engine;
pub mod emulator;
pub mod market;
pub mod replay;
pub mod metrics;
pub mod bin_format;
pub mod orderbook;
pub mod filters;
#[cfg(feature = "gate_exec")]
pub mod strategy_adapter;

pub use engine::{BacktestEngine, BacktestSettings, ExecutionMode};
pub use emulator::{MarketEmulator, EmulatorSettings};
pub use market::{MarketState, TradeStream, TradeTick};
pub use replay::{ReplayEngine, ReplaySettings};
pub use metrics::{BacktestMetrics, BacktestResult};
pub use bin_format::{BinFileReader, BinFileWriter, TradeRecord};
pub use orderbook::{OrderBook, OrderLevel, FillModel};
pub use filters::{MarketFilters, MarketSelector, SortCriterion};

