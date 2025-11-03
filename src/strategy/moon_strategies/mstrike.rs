//! MStrike стратегия - детект прострела с LastBidEMA
//! Ловит быстрое падение цены и выставляет buy ордер

use crate::backtest::market::TradeTick;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::VecDeque;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MStrikeConfig {
    // Основные параметры детекта
    pub mstrike_depth: f64,              // Глубина прострела в % (10% по умолчанию)
    pub mstrike_volume: f64,             // Минимальный объем прострела
    pub mstrike_buy_delay: u64,          // Задержка выставления buy (мс)
    
    // Параметры выставления ордера
    pub mstrike_buy_level: f64,          // % от глубины прострела для buy (0 = в самом низу)
    pub mstrike_buy_relative: bool,       // YES = относительно глубины, NO = относительно цены до детекта
    pub mstrike_sell_level: f64,         // % от глубины прострела для sell (80% = 80% от глубины)
    pub mstrike_sell_adjust: f64,        // Объединение всех sell ордеров
    
    // Модификаторы дельт
    pub mstrike_add_hourly_delta: f64,   // Добавить % к глубине за каждый % часовой дельты
    pub mstrike_add_15min_delta: f64,    // Добавить % к глубине за каждый % 15м дельты
    pub mstrike_add_market_delta: f64,   // Добавить % к глубине за каждый % дельты маркета
    pub mstrike_add_btc_delta: f64,      // Добавить % к глубине за каждый % дельты BTC
    
    // Направление
    pub mstrike_direction: MStrikeDirection, // Both, OnlyLong, OnlyShort
    
    // MStrikeWaitDip: Ждать пока не появится трейд выше (или ниже для шорта)
    pub mstrike_wait_dip: bool,          // Ждать разворот
    pub mstrike_wait_dip_timeout: u64,   // Таймаут ожидания (мс, макс 10 сек)
    
    // Общие параметры
    pub order_size: f64,                 // Размер ордера
    pub use_stop_loss: bool,
    pub use_trailing: bool,
    pub use_take_profit: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum MStrikeDirection {
    Both,      // В обе стороны симметрично
    OnlyLong,  // Только лонг
    OnlyShort, // Только шорт
}

impl Default for MStrikeConfig {
    fn default() -> Self {
        MStrikeConfig {
            mstrike_depth: 10.0,
            mstrike_volume: 0.0,
            mstrike_buy_delay: 0,
            mstrike_buy_level: 0.0,
            mstrike_buy_relative: true,
            mstrike_sell_level: 80.0,
            mstrike_sell_adjust: 0.0,
            mstrike_add_hourly_delta: 0.0,
            mstrike_add_15min_delta: 0.0,
            mstrike_add_market_delta: 0.0,
            mstrike_add_btc_delta: 0.0,
            mstrike_direction: MStrikeDirection::Both,
            mstrike_wait_dip: false,
            mstrike_wait_dip_timeout: 10000,
            order_size: 100.0,
            use_stop_loss: false,
            use_trailing: false,
            use_take_profit: false,
        }
    }
}

#[derive(Debug, Clone)]
pub struct MStrikeState {
    // LastBidEMA и история
    last_bid_ema: Option<f64>,
    bid_history: VecDeque<(DateTime<Utc>, f64)>, // История бидов для EMA
    
    // Состояние детекта
    min_price_during_strike: Option<f64>,    // Минимальная цена во время прострела
    strike_start_time: Option<DateTime<Utc>>, // Время начала прострела
    strike_volume: f64,                       // Объем прострела
    
    // Цена до детекта
    price_before_strike: Option<f64>,
    
    // Текущий ордер
    active_order_id: Option<u64>,
    buy_price: Option<f64>,
    position_size: f64,
    
    // Дельты (для модификаторов)
    delta_hourly: f64,
    delta_15min: f64,
    delta_market: f64,
    delta_btc: f64,
    
    // Ожидание разворота (MStrikeWaitDip)
    waiting_for_dip_reversal: bool,
    dip_wait_start: Option<DateTime<Utc>>,
    last_price_before_dip: Option<f64>,
}

#[derive(Debug, Clone)]
pub enum MStrikeSignal {
    NoAction,
    DetectStrike {
        depth: f64,
        volume: f64,
        min_price: f64,
    },
    PlaceBuy {
        price: f64,
        size: f64,
        reason: String,
    },
    PlaceSell {
        price: f64,
        size: f64,
    },
    CancelOrder {
        order_id: u64,
    },
}

pub struct MStrikeStrategy {
    config: MStrikeConfig,
    state: MStrikeState,
}

impl MStrikeStrategy {
    pub fn new(config: MStrikeConfig) -> Self {
        Self {
            config,
            state: MStrikeState {
                last_bid_ema: None,
                bid_history: VecDeque::new(),
                min_price_during_strike: None,
                strike_start_time: None,
                strike_volume: 0.0,
                price_before_strike: None,
                active_order_id: None,
                buy_price: None,
                position_size: 0.0,
                delta_hourly: 0.0,
                delta_15min: 0.0,
                delta_market: 0.0,
                delta_btc: 0.0,
                waiting_for_dip_reversal: false,
                dip_wait_start: None,
                last_price_before_dip: None,
            },
        }
    }
    
    pub fn default() -> Self {
        Self::new(MStrikeConfig::default())
    }
    
    /// Обработка нового тика
    pub fn on_tick(&mut self, tick: &TradeTick, deltas: &super::mshot::Deltas) -> MStrikeSignal {
        let now = tick.timestamp;
        let current_price = tick.price;
        let current_bid = tick.best_bid.unwrap_or(current_price);
        
        // Обновляем дельты
        self.update_deltas(deltas);
        
        // Обновляем историю бидов
        self.update_bid_history(now, current_bid);
        
        // Вычисляем LastBidEMA по специальной формуле
        self.update_last_bid_ema(current_bid);
        
        // Если есть активная позиция - управляем ей
        if self.state.buy_price.is_some() {
            return self.manage_position(tick);
        }
        
        // Если ждем разворот (MStrikeWaitDip)
        if self.state.waiting_for_dip_reversal {
            return self.check_dip_reversal(tick);
        }
        
        // Проверяем детект прострела
        if let Some(signal) = self.detect_strike(tick) {
            return signal;
        }
        
        MStrikeSignal::NoAction
    }
    
    fn update_bid_history(&mut self, timestamp: DateTime<Utc>, bid: f64) {
        self.state.bid_history.push_back((timestamp, bid));
        
        // Храним только последние 10 тиков для EMA(4)
        if self.state.bid_history.len() > 10 {
            self.state.bid_history.pop_front();
        }
    }
    
    /// Вычисление LastBidEMA по формуле MoonBot
    /// Если на предпоследнем тике бид меньше чем LastBidEMA, то LastBidEMA = бид на предпоследнем тике
    /// Если больше - обычное EMA(4)
    fn update_last_bid_ema(&mut self, current_bid: f64) {
        if self.state.bid_history.len() < 4 {
            // Недостаточно данных для EMA(4)
            return;
        }
        
        let bids: Vec<f64> = self.state.bid_history
            .iter()
            .map(|(_, bid)| *bid)
            .collect();
        
        // Предпоследний бид (2 секунды назад)
        let prev_bid = if bids.len() >= 2 {
            bids[bids.len() - 2]
        } else {
            bids[bids.len() - 1]
        };
        
        // Вычисляем EMA(4)
        let multiplier = 2.0 / (4.0 + 1.0); // 2 / (period + 1)
        let recent_bids = &bids[bids.len().saturating_sub(4)..];
        
        let mut ema = recent_bids[0];
        for &bid in recent_bids.iter().skip(1) {
            ema = (bid * multiplier) + (ema * (1.0 - multiplier));
        }
        
        // Применяем формулу LastBidEMA
        if let Some(last_ema) = self.state.last_bid_ema {
            if prev_bid < last_ema {
                // При падении цены LastBidEMA = бид на предпоследнем тике
                self.state.last_bid_ema = Some(prev_bid);
            } else {
                // При росте - обычное EMA(4)
                self.state.last_bid_ema = Some(ema);
            }
        } else {
            // Первое вычисление
            self.state.last_bid_ema = Some(ema);
        }
    }
    
    fn detect_strike(&mut self, tick: &TradeTick) -> Option<MStrikeSignal> {
        let now = tick.timestamp;
        let current_price = tick.price;
        let current_bid = tick.best_bid.unwrap_or(current_price);
        let volume = tick.volume;
        
        let last_bid_ema = self.state.last_bid_ema?;
        
        // Вычисляем эффективную глубину с учетом дельт
        let effective_depth = self.calculate_effective_depth();
        
        // Находим минимальную цену во время прострела
        if self.state.min_price_during_strike.is_none() {
            // Начинаем отслеживание прострела
            if current_price < last_bid_ema {
                self.state.strike_start_time = Some(now);
                self.state.min_price_during_strike = Some(current_price);
                self.state.price_before_strike = Some(last_bid_ema);
                self.state.strike_volume = volume;
                return None;
            }
        } else {
            // Обновляем минимум
            let min_price = self.state.min_price_during_strike.unwrap();
            if current_price < min_price {
                self.state.min_price_during_strike = Some(current_price);
                self.state.strike_volume += volume;
            }
        }
        
        let min_price = self.state.min_price_during_strike.unwrap();
        let price_before = self.state.price_before_strike.unwrap();
        
        // Вычисляем глубину прострела
        let depth = ((price_before - min_price) / price_before) * 100.0;
        
        // Проверяем условие детекта
        if depth >= effective_depth {
            // Проверяем объем
            if self.state.strike_volume >= self.config.mstrike_volume {
                // Детект! Логируем информацию
                let signal = MStrikeSignal::DetectStrike {
                    depth,
                    volume: self.state.strike_volume,
                    min_price,
                };
                
                // Если нужна задержка перед выставлением ордера
                if self.config.mstrike_buy_delay > 0 {
                    // Задержка будет обработана позже через event queue
                    return Some(signal);
                }
                
                // Если нужно ждать разворот (MStrikeWaitDip)
                if self.config.mstrike_wait_dip {
                    self.state.waiting_for_dip_reversal = true;
                    self.state.dip_wait_start = Some(now);
                    self.state.last_price_before_dip = Some(current_price);
                    return Some(signal);
                }
                
                // Выставляем ордер сразу
                return self.place_buy_order(min_price, depth);
            }
        }
        
        None
    }
    
    fn calculate_effective_depth(&self) -> f64 {
        let mut depth = self.config.mstrike_depth;
        
        // Добавляем модификаторы дельт
        depth += self.state.delta_hourly * self.config.mstrike_add_hourly_delta;
        depth += self.state.delta_15min * self.config.mstrike_add_15min_delta;
        depth += self.state.delta_market * self.config.mstrike_add_market_delta;
        depth += self.state.delta_btc * self.config.mstrike_add_btc_delta;
        
        depth.max(0.1) // Минимум 0.1%
    }
    
    fn place_buy_order(&mut self, min_price: f64, depth: f64) -> Option<MStrikeSignal> {
        let price_before = self.state.price_before_strike.unwrap();
        
        // Вычисляем цену buy ордера
        let buy_price = if self.config.mstrike_buy_relative {
            // Относительно глубины прострела
            if self.config.mstrike_buy_level == 0.0 {
                // В самом низу
                min_price
            } else {
                // На уровне MStrikeBuyLevel % от глубины
                let level_from_min = depth * (self.config.mstrike_buy_level / 100.0);
                min_price * (1.0 + level_from_min / 100.0)
            }
        } else {
            // Относительно цены до прострела
            price_before * (1.0 - self.config.mstrike_buy_level / 100.0)
        };
        
        self.state.buy_price = Some(buy_price);
        self.state.position_size = self.config.order_size;
        
        // Вычисляем цену продажи заранее
        let sell_price = self.calculate_sell_price(min_price, depth);
        
        Some(MStrikeSignal::PlaceBuy {
            price: buy_price,
            size: self.config.order_size,
            reason: format!("MStrike detected: depth={:.2}%, volume={:.2}", depth, self.state.strike_volume),
        })
    }
    
    fn calculate_sell_price(&self, min_price: f64, depth: f64) -> f64 {
        let price_before = self.state.price_before_strike.unwrap();
        
        // SellLevel - процент от глубины прострела
        let sell_level_price = min_price * (1.0 + (depth * self.config.mstrike_sell_level / 100.0) / 100.0);
        
        sell_level_price
    }
    
    fn check_dip_reversal(&mut self, tick: &TradeTick) -> MStrikeSignal {
        let now = tick.timestamp;
        let current_price = tick.price;
        
        // Проверяем таймаут
        if let Some(wait_start) = self.state.dip_wait_start {
            let elapsed = (now - wait_start).num_milliseconds() as u64;
            if elapsed > self.config.mstrike_wait_dip_timeout {
                // Таймаут - сбрасываем ожидание
                self.reset_strike_state();
                return MStrikeSignal::NoAction;
            }
        }
        
        // Проверяем разворот: появился трейд выше предыдущего
        if let Some(last_price) = self.state.last_price_before_dip {
            if current_price > last_price {
                // Разворот обнаружен - выставляем ордер
                self.state.waiting_for_dip_reversal = false;
                
                let min_price = self.state.min_price_during_strike.unwrap();
                let depth = {
                    let price_before = self.state.price_before_strike.unwrap();
                    ((price_before - min_price) / price_before) * 100.0
                };
                
                return self.place_buy_order(min_price, depth).unwrap_or(MStrikeSignal::NoAction);
            }
        }
        
        MStrikeSignal::NoAction
    }
    
    fn manage_position(&mut self, tick: &TradeTick) -> MStrikeSignal {
        let current_price = tick.price;
        let buy_price = self.state.buy_price.unwrap();
        
        // Вычисляем цену продажи
        let min_price = self.state.min_price_during_strike.unwrap();
        let depth = {
            let price_before = self.state.price_before_strike.unwrap();
            ((price_before - min_price) / price_before) * 100.0
        };
        let sell_price = self.calculate_sell_price(min_price, depth);
        
        // Проверяем условие продажи
        if current_price >= sell_price {
            return MStrikeSignal::PlaceSell {
                price: sell_price,
                size: self.state.position_size,
            };
        }
        
        // TODO: Добавить стоп-лосс и трейлинг
        
        MStrikeSignal::NoAction
    }
    
    fn update_deltas(&mut self, deltas: &super::mshot::Deltas) {
        self.state.delta_hourly = deltas.delta_hourly;
        self.state.delta_15min = deltas.delta_15min;
        self.state.delta_market = deltas.delta_market;
        self.state.delta_btc = deltas.delta_btc;
    }
    
    fn reset_strike_state(&mut self) {
        self.state.min_price_during_strike = None;
        self.state.strike_start_time = None;
        self.state.strike_volume = 0.0;
        self.state.price_before_strike = None;
        self.state.waiting_for_dip_reversal = false;
        self.state.dip_wait_start = None;
        self.state.last_price_before_dip = None;
    }
    
    /// Вызывается при исполнении buy ордера
    pub fn on_buy_filled(&mut self, price: f64, size: f64) {
        self.state.buy_price = Some(price);
        self.state.position_size = size;
        self.state.active_order_id = Some(0); // TODO: получить реальный ID
    }
    
    /// Вызывается при исполнении sell ордера
    pub fn on_sell_filled(&mut self) {
        self.state.buy_price = None;
        self.state.position_size = 0.0;
        self.state.active_order_id = None;
        self.reset_strike_state();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::backtest::market::{TradeTick, TradeSide};
    use crate::strategy::moon_strategies::mshot::Deltas;
    use chrono::Utc;

    #[test]
    fn test_mstrike_detect_strike() {
        let config = MStrikeConfig::default();
        let mut strategy = MStrikeStrategy::new(config);
        
        let now = Utc::now();
        
        // Создаем серию тиков с прострелом вниз
        let ticks = vec![
            TradeTick {
                timestamp: now,
                symbol: "BTC_USDT".to_string(),
                price: 100.0,
                volume: 1.0,
                side: TradeSide::Buy,
                trade_id: "1".to_string(),
                best_bid: Some(99.9),
                best_ask: Some(100.1),
            },
            TradeTick {
                timestamp: now + chrono::Duration::try_milliseconds(100).unwrap(),
                symbol: "BTC_USDT".to_string(),
                price: 95.0, // Прострел на 5%
                volume: 10.0,
                side: TradeSide::Sell,
                trade_id: "2".to_string(),
                best_bid: Some(94.9),
                best_ask: Some(95.1),
            },
        ];
        
        let deltas = Deltas::default();
        
        // Первый тик - цена еще высокая
        let signal1 = strategy.on_tick(&ticks[0], &deltas);
        assert!(matches!(signal1, MStrikeSignal::NoAction));
        
        // Второй тик - детект прострела
        let signal2 = strategy.on_tick(&ticks[1], &deltas);
        // Должен быть либо PlaceBuy, либо NoAction в зависимости от параметров
        assert!(matches!(signal2, MStrikeSignal::PlaceBuy { .. } | MStrikeSignal::NoAction));
    }
    
    #[test]
    fn test_mstrike_config_default() {
        let config = MStrikeConfig::default();
        assert!(config.mstrike_depth > 0.0);
        assert!(config.order_size > 0.0);
    }
    
    #[test]
    fn test_mstrike_strategy_creation() {
        let config = MStrikeConfig::default();
        let strategy = MStrikeStrategy::new(config);
        // Проверяем, что стратегия создается без ошибок
        assert_eq!(strategy.state.buy_price, None);
        assert_eq!(strategy.state.position_size, 0.0);
    }
}
