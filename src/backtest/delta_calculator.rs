//! Калькулятор дельт цены для различных временных окон
//! Используется стратегиями MShot, MStrike, Hook для модификации параметров

use crate::strategy::moon_strategies::mshot::Deltas;
use crate::backtest::market::TradeTick;
use chrono::{DateTime, Utc, Duration};
use std::collections::VecDeque;

#[derive(Debug, Clone)]
struct PricePoint {
    timestamp: DateTime<Utc>,
    price: f64,
}

/// Калькулятор дельт на основе истории тиков
pub struct DeltaCalculator {
    /// История цен для текущего символа
    price_history: VecDeque<PricePoint>,
    
    /// История цен для BTC (для delta_btc)
    btc_price_history: VecDeque<PricePoint>,
    
    /// История цен для маркета (для delta_market)
    market_price_history: VecDeque<PricePoint>,
    
    /// Максимальное время хранения истории (для очистки)
    max_history_duration: Duration,
}

impl DeltaCalculator {
    pub fn new() -> Self {
        Self {
            price_history: VecDeque::new(),
            btc_price_history: VecDeque::new(),
            market_price_history: VecDeque::new(),
            max_history_duration: Duration::hours(24), // Храним 24 часа
        }
    }
    
    /// Обновить историю цен новым тиком
    pub fn update(&mut self, tick: &TradeTick, current_time: DateTime<Utc>) {
        // Обновляем историю для текущего символа
        self.price_history.push_back(PricePoint {
            timestamp: tick.timestamp,
            price: tick.price,
        });
        
        // Обновляем BTC историю если это BTC пара
        if tick.symbol.contains("BTC") || tick.symbol == "BTC_USDT" {
            self.btc_price_history.push_back(PricePoint {
                timestamp: tick.timestamp,
                price: tick.price,
            });
        }
        
        // Обновляем маркет историю (упрощенно - для всех символов)
        self.market_price_history.push_back(PricePoint {
            timestamp: tick.timestamp,
            price: tick.price,
        });
        
        // Очищаем старую историю
        self.cleanup(current_time);
    }
    
    /// Вычислить дельты для текущего символа
    pub fn calculate_deltas(&self, current_price: f64, current_time: DateTime<Utc>) -> Deltas {
        let delta_15min = self.calculate_delta_percent(
            &self.price_history,
            current_price,
            current_time,
            Duration::minutes(15),
        );
        
        let delta_hourly = self.calculate_delta_percent(
            &self.price_history,
            current_price,
            current_time,
            Duration::hours(1),
        );
        
        let delta_3h = self.calculate_delta_percent(
            &self.price_history,
            current_price,
            current_time,
            Duration::hours(3),
        );
        
        // BTC дельты
        let delta_btc = if !self.btc_price_history.is_empty() {
            self.calculate_delta_percent(
                &self.btc_price_history,
                self.btc_price_history.back().map(|p| p.price).unwrap_or(current_price),
                current_time,
                Duration::hours(1),
            )
        } else {
            0.0
        };
        
        let delta_btc_5m = if !self.btc_price_history.is_empty() {
            // Для 5м дельты BTC берем минимум/максимум за последние 5 минут
            let cutoff = current_time - Duration::minutes(5);
            let prices_5m: Vec<f64> = self.btc_price_history
                .iter()
                .filter(|p| p.timestamp >= cutoff)
                .map(|p| p.price)
                .collect();
            
            if !prices_5m.is_empty() {
                let min = prices_5m.iter().fold(f64::INFINITY, |a, &b| a.min(b));
                let max = prices_5m.iter().fold(f64::NEG_INFINITY, |a, &b| a.max(b));
                ((max - min) / min) * 100.0
            } else {
                0.0
            }
        } else {
            0.0
        };
        
        // Маркет дельта (упрощенно - используем текущий символ)
        let delta_market = delta_hourly; // Можно заменить на реальный расчет маркета
        
        Deltas {
            delta_3h,
            delta_hourly,
            delta_15min,
            delta_market,
            delta_btc,
            delta_btc_5m,
        }
    }
    
    /// Вычислить процентное изменение цены за период
    fn calculate_delta_percent(
        &self,
        history: &VecDeque<PricePoint>,
        current_price: f64,
        current_time: DateTime<Utc>,
        window: Duration,
    ) -> f64 {
        let cutoff = current_time - window;
        
        // Находим цену в начале окна
        let start_price = history
            .iter()
            .find(|p| p.timestamp >= cutoff)
            .map(|p| p.price)
            .unwrap_or_else(|| {
                // Если не нашли в окне, берем самую старую цену
                history.front().map(|p| p.price).unwrap_or(current_price)
            });
        
        if start_price > 0.0 {
            ((current_price - start_price) / start_price) * 100.0
        } else {
            0.0
        }
    }
    
    /// Очистить старую историю
    fn cleanup(&mut self, current_time: DateTime<Utc>) {
        let cutoff = current_time - self.max_history_duration;
        
        while let Some(front) = self.price_history.front() {
            if front.timestamp < cutoff {
                self.price_history.pop_front();
            } else {
                break;
            }
        }
        
        while let Some(front) = self.btc_price_history.front() {
            if front.timestamp < cutoff {
                self.btc_price_history.pop_front();
            } else {
                break;
            }
        }
        
        while let Some(front) = self.market_price_history.front() {
            if front.timestamp < cutoff {
                self.market_price_history.pop_front();
            } else {
                break;
            }
        }
    }
}

impl Default for DeltaCalculator {
    fn default() -> Self {
        Self::new()
    }
}

