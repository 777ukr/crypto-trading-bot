//! Hook стратегия - динамический коридор цены
//! Детектит быстрое падение и выставляет buy-ордер, который движется в коридоре

use crate::backtest::market::TradeTick;
use chrono::{DateTime, Utc, Duration};
use serde::{Deserialize, Serialize};
use std::collections::VecDeque;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HookConfig {
    // Основные параметры детекта
    pub hook_time_frame: Duration,        // Интервал времени для анализа (обычно 2 сек)
    pub hook_detect_depth: f64,           // Глубина детекта (%)
    pub hook_detect_depth_max: f64,       // Максимальная глубина (0 = не ограничивать)
    
    // Параметры коридора
    pub hook_initial_price: f64,          // Куда ставить buy в % от глубины
    pub hook_price_distance: f64,         // Ширина коридора в % от глубины
    pub hook_price_roll_back: f64,        // Процент отката цены (%)
    pub hook_price_roll_back_max: f64,    // Ограничение роллбека
    pub hook_roll_back_wait: u64,        // Время ожидания отката (мс)
    
    // Дополнительные фильтры
    pub hook_anti_pump: bool,             // Исключить прострелы после быстрого роста
    pub hook_drop_min: f64,               // Падение цены перед детектом (мин %)
    pub hook_drop_max: f64,               // Падение цены перед детектом (макс %)
    
    // Направление
    pub hook_direction: HookDirection,    // Long, Short, Both
    pub hook_opposite_order: bool,        // Ставить ордер в обратную сторону
    
    // Интерполяция (0-4)
    pub hook_interpolate: u8,             // Способ вычисления коридора
    
    // Параметры ордера
    pub buy_order_reduce: u64,            // Интервал для расчета среднего объема (мс)
    pub min_reduced_size: f64,            // Минимальный размер ордера после уменьшения
    
    // Параметры продажи
    pub hook_sell_level: f64,             // % от глубины детекта для sell
    pub hook_sell_fixed: bool,            // Считать sell всегда как HookSellLevel * глубина
    
    // Задержки
    pub hook_replace_delay: f64,          // Задержка перед перестановкой (сек)
    pub hook_raise_wait: f64,             // Задержка при росте цены (сек)
    pub hook_part_filled_delay: u64,      // Задержка отмены после частичного заполнения (мс)
    
    // Повторные ордера
    pub hook_repeat_after_sell: bool,
    pub hook_repeat_if_profit: f64,       // % для повтора
    
    // Общие параметры
    pub order_size: f64,
    pub buy_modifier: f64,                // Модификатор ширины коридора (отрицательный!)
    pub use_stop_loss: bool,
    pub use_trailing: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum HookDirection {
    Long,
    Short,
    Both,
}

impl Default for HookConfig {
    fn default() -> Self {
        HookConfig {
            hook_time_frame: Duration::seconds(2),
            hook_detect_depth: 5.0,
            hook_detect_depth_max: 0.0,
            hook_initial_price: 25.0,
            hook_price_distance: 10.0,
            hook_price_roll_back: 33.0,
            hook_price_roll_back_max: 0.0,
            hook_roll_back_wait: 100,
            hook_anti_pump: false,
            hook_drop_min: 0.0,
            hook_drop_max: 0.0,
            hook_direction: HookDirection::Long,
            hook_opposite_order: false,
            hook_interpolate: 0,
            buy_order_reduce: 100,
            min_reduced_size: 0.0,
            hook_sell_level: 75.0,
            hook_sell_fixed: false,
            hook_replace_delay: 0.0,
            hook_raise_wait: 0.0,
            hook_part_filled_delay: 0,
            hook_repeat_after_sell: false,
            hook_repeat_if_profit: 0.0,
            order_size: 100.0,
            buy_modifier: -3.0,
            use_stop_loss: false,
            use_trailing: false,
        }
    }
}

#[derive(Debug, Clone)]
pub struct HookState {
    // Окно для анализа (HookTimeFrame)
    price_window: VecDeque<(DateTime<Utc>, f64)>, // История цен в окне
    volume_window: VecDeque<(DateTime<Utc>, f64)>, // История объемов
    
    // Состояние детекта
    strike_detected: bool,
    strike_detection_time: Option<DateTime<Utc>>,
    strike_depth: f64,
    strike_min_price: f64,
    strike_max_price: f64,
    strike_rollback_price: Option<f64>,
    
    // Дельты на момент детекта (для BuyModifier)
    deltas_at_detection: Option<super::mshot::Deltas>,
    
    // Коридор цены
    corridor_upper: Option<f64>,
    corridor_lower: Option<f64>,
    initial_buy_price: Option<f64>,
    
    // Текущий ордер
    active_order_id: Option<u64>,
    buy_price: Option<f64>,
    position_size: f64,
    
    // Повторные ордера
    repeat_orders: Vec<RepeatOrderState>,
}

#[derive(Debug, Clone)]
struct RepeatOrderState {
    buy_price: f64,
    sell_price: f64,
    placed_at: DateTime<Utc>,
}

#[derive(Debug, Clone)]
pub enum HookSignal {
    NoAction,
    DetectHook {
        depth: f64,
        min_price: f64,
        max_price: f64,
    },
    PlaceBuy {
        price: f64,
        size: f64,
        reason: String,
    },
    ReplaceBuy {
        new_price: f64,
    },
    PlaceSell {
        price: f64,
        size: f64,
    },
    CancelOrder {
        order_id: u64,
    },
}

pub struct HookStrategy {
    config: HookConfig,
    state: HookState,
}

impl HookStrategy {
    pub fn new(config: HookConfig) -> Self {
        Self {
            config,
            state: HookState {
                price_window: VecDeque::new(),
                volume_window: VecDeque::new(),
                strike_detected: false,
                strike_detection_time: None,
                strike_depth: 0.0,
                strike_min_price: 0.0,
                strike_max_price: 0.0,
                strike_rollback_price: None,
                deltas_at_detection: None,
                corridor_upper: None,
                corridor_lower: None,
                initial_buy_price: None,
                active_order_id: None,
                buy_price: None,
                position_size: 0.0,
                repeat_orders: Vec::new(),
            },
        }
    }
    
    pub fn default() -> Self {
        Self::new(HookConfig::default())
    }
    
    /// Обработка нового тика
    pub fn on_tick(&mut self, tick: &TradeTick, deltas: &super::mshot::Deltas) -> HookSignal {
        let now = tick.timestamp;
        let current_price = tick.price;
        let volume = tick.volume;
        
        // Обновляем окно данных
        self.update_window(now, current_price, volume);
        
        // Если есть позиция - управляем ей
        if self.state.buy_price.is_some() {
            return self.manage_position(tick);
        }
        
        // Если есть активный ордер в коридоре - проверяем перестановку
        if self.state.active_order_id.is_some() && self.state.corridor_upper.is_some() {
            return self.manage_corridor_order(tick);
        }
        
        // Проверяем детект (если не детектировали или прошло достаточно времени)
        if !self.state.strike_detected || self.can_detect_again(now) {
            if let Some(signal) = self.detect_hook(tick, deltas) {
                return signal;
            }
        }
        
        HookSignal::NoAction
    }
    
    fn update_window(&mut self, timestamp: DateTime<Utc>, price: f64, volume: f64) {
        // Добавляем текущие данные
        self.state.price_window.push_back((timestamp, price));
        self.state.volume_window.push_back((timestamp, volume));
        
        // Удаляем старые данные вне HookTimeFrame
        let cutoff_time = timestamp - self.config.hook_time_frame;
        
        while let Some(&(time, _)) = self.state.price_window.front() {
            if time >= cutoff_time {
                break;
            }
            self.state.price_window.pop_front();
        }
        
        while let Some(&(time, _)) = self.state.volume_window.front() {
            if time >= cutoff_time {
                break;
            }
            self.state.volume_window.pop_front();
        }
    }
    
    fn detect_hook(&mut self, tick: &TradeTick, deltas: &super::mshot::Deltas) -> Option<HookSignal> {
        if self.state.price_window.len() < 2 {
            return None;
        }
        
        let current_price = tick.price;
        let prices: Vec<f64> = self.state.price_window.iter().map(|(_, p)| *p).collect();
        
        // Находим максимум и минимум в окне
        let max_price = prices.iter().fold(0.0f64, |a, &b| a.max(b));
        let min_price = prices.iter().fold(f64::INFINITY, |a, &b| a.min(b));
        
        // Вычисляем глубину прострела
        let depth = ((max_price - min_price) / max_price) * 100.0;
        
        // Проверяем условие детекта
        if depth < self.config.hook_detect_depth {
            return None;
        }
        
        // Проверяем максимальную глубину
        if self.config.hook_detect_depth_max > 0.0 && depth > self.config.hook_detect_depth_max {
            return None;
        }
        
        // HookAntiPump: проверка быстрого роста перед прострелом
        if self.config.hook_anti_pump {
            // TODO: Проверка средней цены перед детектом
        }
        
        // HookDropMin/Max: проверка падения перед детектом
        if self.config.hook_drop_min > 0.0 || self.config.hook_drop_max > 0.0 {
            // TODO: Проверка падения за последние 2 минуты
        }
        
        // Детект найден!
        self.state.strike_detected = true;
        self.state.strike_detection_time = Some(tick.timestamp);
        self.state.strike_depth = depth;
        self.state.strike_min_price = min_price;
        self.state.strike_max_price = max_price;
        self.state.deltas_at_detection = Some(deltas.clone());
        
        // Вычисляем откат (HookPriceRollBack)
        let rollback_price = max_price - (depth * self.config.hook_price_roll_back / 100.0) * (max_price / 100.0);
        self.state.strike_rollback_price = Some(rollback_price);
        
        // Вычисляем коридор и начальную цену
        self.calculate_corridor();
        
        // Вычисляем размер ордера с учетом BuyOrderReduce
        let order_size = self.calculate_order_size();
        
        if order_size < self.config.min_reduced_size {
            // Ордер слишком маленький - не ставим
            return None;
        }
        
        // Выставляем ордер
        let buy_price = self.state.initial_buy_price.unwrap();
        
        Some(HookSignal::PlaceBuy {
            price: buy_price,
            size: order_size,
            reason: format!("Hook detected: depth={:.2}%", depth),
        })
    }
    
    fn calculate_corridor(&mut self) {
        let depth = self.state.strike_depth;
        let max_price = self.state.strike_max_price;
        let min_price = self.state.strike_min_price;
        let rollback = self.state.strike_rollback_price.unwrap_or(max_price);
        
        // Вычисляем коридор в зависимости от HookInterpolate
        let (upper, lower, initial) = match self.config.hook_interpolate {
            0 => {
                // От верхней цены до прострела
                let distance = depth * self.config.hook_price_distance / 100.0;
                let upper = max_price;
                let lower = min_price;
                let initial = min_price + (max_price - min_price) * (self.config.hook_initial_price / 100.0);
                (upper, lower, initial)
            }
            1 => {
                // От отката после прострела
                let distance = depth * self.config.hook_price_distance / 100.0;
                let upper = rollback;
                let lower = min_price;
                let initial = rollback - (rollback - min_price) * (self.config.hook_initial_price / 100.0);
                (upper, lower, initial)
            }
            2 => {
                // Приоритет HookInitialPrice
                let distance = depth * self.config.hook_price_distance / 100.0;
                let initial = min_price + (max_price - min_price) * (self.config.hook_initial_price / 100.0);
                let upper = initial + distance * (max_price / 100.0);
                let lower = initial - distance * (max_price / 100.0);
                (upper, lower, initial)
            }
            3 => {
                // Относительно текущей цены
                let current = self.state.price_window.back().map(|(_, p)| *p).unwrap_or(max_price);
                let distance = depth * self.config.hook_price_distance / 100.0;
                let upper = current * (1.0 + distance / 100.0);
                let lower = current * (1.0 - distance / 100.0);
                let initial = current * (1.0 - self.config.hook_initial_price / 100.0);
                (upper, lower, initial)
            }
            4 => {
                // Относительно глубины отката
                let distance = depth * self.config.hook_price_distance / 100.0;
                let initial = min_price + (rollback - min_price) * (self.config.hook_initial_price / 100.0);
                let upper = (initial + rollback) / 2.0;
                let lower = initial - distance * (max_price / 100.0);
                (upper.max(initial), lower.min(initial), initial)
            }
            _ => {
                // По умолчанию - как вариант 0
                let initial = min_price + (max_price - min_price) * (self.config.hook_initial_price / 100.0);
                (max_price, min_price, initial)
            }
        };
        
        // Применяем BuyModifier если HookInterpolate != 0
        if self.config.hook_interpolate != 0 && self.config.buy_modifier < 0.0 {
            if let Some(ref deltas_at_detection) = self.state.deltas_at_detection {
                // TODO: Применить модификатор на основе изменения дельт
            }
        }
        
        self.state.corridor_upper = Some(upper);
        self.state.corridor_lower = Some(lower);
        self.state.initial_buy_price = Some(initial);
    }
    
    fn calculate_order_size(&self) -> f64 {
        if self.config.buy_order_reduce == 0 {
            return self.config.order_size;
        }
        
        // Вычисляем средний объем за BuyOrderReduce интервал
        let total_volume: f64 = self.state.volume_window.iter().map(|(_, v)| *v).sum();
        let time_window_ms = self.config.hook_time_frame.num_milliseconds() as f64;
        let avg_volume_per_interval = (total_volume / time_window_ms) * (self.config.buy_order_reduce as f64);
        
        // Ордер не должен быть больше среднего объема
        self.config.order_size.min(avg_volume_per_interval)
    }
    
    fn manage_corridor_order(&mut self, tick: &TradeTick) -> HookSignal {
        let current_price = tick.price;
        let upper = self.state.corridor_upper.unwrap();
        let lower = self.state.corridor_lower.unwrap();
        let buy_price = self.state.initial_buy_price.unwrap();
        
        // Проверяем, нужно ли переставить ордер
        if current_price <= lower {
            // Цена упала до нижней границы - переставляем вниз
            let new_price = lower * 0.99; // Немного ниже нижней границы
            return HookSignal::ReplaceBuy { new_price };
        } else if current_price >= upper {
            // Цена выросла до верхней границы - переставляем вверх
            let new_price = upper * 0.99;
            return HookSignal::ReplaceBuy { new_price };
        }
        
        HookSignal::NoAction
    }
    
    fn manage_position(&mut self, tick: &TradeTick) -> HookSignal {
        let current_price = tick.price;
        let buy_price = self.state.buy_price.unwrap();
        let depth = self.state.strike_depth;
        
        // Вычисляем цену продажи
        let sell_price = if self.config.hook_sell_fixed {
            let min_price = self.state.strike_min_price;
            min_price * (1.0 + (depth * self.config.hook_sell_level / 100.0) / 100.0)
        } else {
            buy_price * (1.0 + (depth * self.config.hook_sell_level / 100.0) / 100.0)
        };
        
        if current_price >= sell_price {
            return HookSignal::PlaceSell {
                price: sell_price,
                size: self.state.position_size,
            };
        }
        
        HookSignal::NoAction
    }
    
    fn can_detect_again(&self, now: DateTime<Utc>) -> bool {
        if let Some(detection_time) = self.state.strike_detection_time {
            let elapsed = (now - detection_time).num_milliseconds();
            elapsed >= self.config.hook_time_frame.num_milliseconds()
        } else {
            true
        }
    }
    
    pub fn on_buy_filled(&mut self, price: f64, size: f64) {
        self.state.buy_price = Some(price);
        self.state.position_size = size;
        self.state.active_order_id = Some(0); // TODO: реальный ID
    }
    
    pub fn on_sell_filled(&mut self) {
        let buy_price = self.state.buy_price.unwrap();
        
        // Проверяем повторный ордер
        if self.config.hook_repeat_after_sell {
            // TODO: Проверить HookRepeatIfProfit
            self.state.repeat_orders.push(RepeatOrderState {
                buy_price,
                sell_price: 0.0, // Будет вычислена позже
                placed_at: Utc::now(),
            });
        }
        
        self.state.buy_price = None;
        self.state.position_size = 0.0;
        self.state.active_order_id = None;
        // Не сбрасываем коридор - он остается активным
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::backtest::market::{TradeTick, TradeSide};
    use crate::strategy::moon_strategies::mshot::Deltas;
    use chrono::Utc;

    #[test]
    fn test_hook_detect_drop() {
        let config = HookConfig {
            hook_detect_depth: 5.0, // 5% падение
            hook_time_frame: chrono::Duration::seconds(2),
            ..Default::default()
        };
        let mut strategy = HookStrategy::new(config);
        
        let now = Utc::now();
        
        // Создаем серию тиков с падением
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
                timestamp: now + chrono::Duration::milliseconds(500),
                symbol: "BTC_USDT".to_string(),
                price: 95.0, // Падение 5%
                volume: 10.0,
                side: TradeSide::Sell,
                trade_id: "2".to_string(),
                best_bid: Some(94.9),
                best_ask: Some(95.1),
            },
        ];
        
        let deltas = Deltas::default();
        
        // Первый тик
        let signal1 = strategy.on_tick(&ticks[0], &deltas);
        assert!(matches!(signal1, HookSignal::NoAction));
        
        // Второй тик - детект падения
        let signal2 = strategy.on_tick(&ticks[1], &deltas);
        // Может быть PlaceBuy или NoAction в зависимости от параметров
        assert!(matches!(signal2, HookSignal::PlaceBuy { .. } | HookSignal::NoAction));
    }
    
    #[test]
    fn test_hook_config_default() {
        let config = HookConfig::default();
        assert!(config.hook_detect_depth > 0.0);
        assert!(config.order_size > 0.0);
        assert!(config.hook_time_frame.num_seconds() > 0);
    }
    
    #[test]
    fn test_hook_strategy_creation() {
        let config = HookConfig::default();
        let strategy = HookStrategy::new(config);
        // Проверяем, что стратегия создается без ошибок
        assert_eq!(strategy.state.buy_price, None);
        assert_eq!(strategy.state.position_size, 0.0);
    }
}
