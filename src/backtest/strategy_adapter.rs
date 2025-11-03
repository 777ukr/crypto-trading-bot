//! Адаптер для интеграции стратегий с бэктестером

#![cfg(feature = "gate_exec")]

use crate::backtest::market::TradeTick;
use crate::strategy::moon_strategies::{
    MShotStrategy, MShotConfig, MShotSignal,
    MStrikeStrategy, MStrikeConfig, MStrikeSignal,
    HookStrategy, HookConfig, HookSignal,
    mshot::Deltas,
};

/// Трейт для унификации работы со стратегиями в бэктестере
pub trait StrategyAdapter {
    fn on_tick(&mut self, tick: &TradeTick, deltas: &Deltas) -> StrategyAction;
    fn get_name(&self) -> &str;
    fn reset(&mut self);
}

#[derive(Debug, Clone)]
pub enum StrategyAction {
    NoAction,
    PlaceBuy { price: f64, size: f64 },
    PlaceSell { price: f64, size: f64 },
    ReplaceBuy { new_price: f64 },
    CancelOrder { order_id: u64 },
    DetectSignal { message: String },
}

/// Адаптер для MShot стратегии
pub struct MShotAdapter {
    strategy: MShotStrategy,
}

impl MShotAdapter {
    pub fn new(config: MShotConfig) -> Self {
        Self {
            strategy: MShotStrategy::new(config),
        }
    }
    
    pub fn default() -> Self {
        Self {
            strategy: MShotStrategy::new(MShotConfig::default()),
        }
    }
}

impl StrategyAdapter for MShotAdapter {
    fn on_tick(&mut self, tick: &TradeTick, deltas: &Deltas) -> StrategyAction {
        match self.strategy.on_tick(tick, deltas) {
            MShotSignal::NoAction => StrategyAction::NoAction,
            MShotSignal::PlaceBuy { price, size } => StrategyAction::PlaceBuy { price, size },
            MShotSignal::ReplaceBuy { new_price } => StrategyAction::ReplaceBuy { new_price },
            MShotSignal::RepeatShot { price, size } => StrategyAction::PlaceBuy { price, size },
            MShotSignal::CancelBuy => StrategyAction::CancelOrder { order_id: 0 },
            MShotSignal::PlaceSell { price, size } => StrategyAction::PlaceSell { price, size },
        }
    }
    
    fn get_name(&self) -> &str {
        "MShot"
    }
    
    fn reset(&mut self) {
        // TODO: Реализовать reset для MShotStrategy
    }
}

/// Адаптер для MStrike стратегии
pub struct MStrikeAdapter {
    strategy: MStrikeStrategy,
}

impl MStrikeAdapter {
    pub fn new(config: MStrikeConfig) -> Self {
        Self {
            strategy: MStrikeStrategy::new(config),
        }
    }
    
    pub fn default() -> Self {
        Self {
            strategy: MStrikeStrategy::new(MStrikeConfig::default()),
        }
    }
}

impl StrategyAdapter for MStrikeAdapter {
    fn on_tick(&mut self, tick: &TradeTick, deltas: &Deltas) -> StrategyAction {
        match self.strategy.on_tick(tick, deltas) {
            MStrikeSignal::NoAction => StrategyAction::NoAction,
            MStrikeSignal::DetectStrike { depth, volume, min_price } => {
                StrategyAction::DetectSignal {
                    message: format!("MStrike: depth={:.2}%, volume={:.2}, min={:.8}", depth, volume, min_price),
                }
            }
            MStrikeSignal::PlaceBuy { price, size, reason: _ } => {
                StrategyAction::PlaceBuy { price, size }
            }
            MStrikeSignal::PlaceSell { price, size } => {
                StrategyAction::PlaceSell { price, size }
            }
            MStrikeSignal::CancelOrder { order_id } => {
                StrategyAction::CancelOrder { order_id }
            }
        }
    }
    
    fn get_name(&self) -> &str {
        "MStrike"
    }
    
    fn reset(&mut self) {
        // TODO: Реализовать reset для MStrikeStrategy
    }
}

/// Адаптер для Hook стратегии
pub struct HookAdapter {
    strategy: HookStrategy,
}

impl HookAdapter {
    pub fn new(config: HookConfig) -> Self {
        Self {
            strategy: HookStrategy::new(config),
        }
    }
    
    pub fn default() -> Self {
        Self {
            strategy: HookStrategy::new(HookConfig::default()),
        }
    }
}

impl StrategyAdapter for HookAdapter {
    fn on_tick(&mut self, tick: &TradeTick, deltas: &Deltas) -> StrategyAction {
        match self.strategy.on_tick(tick, deltas) {
            HookSignal::NoAction => StrategyAction::NoAction,
            HookSignal::DetectHook { depth, min_price, max_price } => {
                StrategyAction::DetectSignal {
                    message: format!("Hook: depth={:.2}%, min={:.8}, max={:.8}", depth, min_price, max_price),
                }
            }
            HookSignal::PlaceBuy { price, size, reason: _ } => {
                StrategyAction::PlaceBuy { price, size }
            }
            HookSignal::ReplaceBuy { new_price } => {
                StrategyAction::ReplaceBuy { new_price }
            }
            HookSignal::PlaceSell { price, size } => {
                StrategyAction::PlaceSell { price, size }
            }
            HookSignal::CancelOrder { order_id } => {
                StrategyAction::CancelOrder { order_id }
            }
        }
    }
    
    fn get_name(&self) -> &str {
        "Hook"
    }
    
    fn reset(&mut self) {
        // TODO: Реализовать reset для HookStrategy
    }
}

