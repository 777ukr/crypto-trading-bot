//! MShot стратегия - переставление buy ордеров в коридоре цен
//! Ловит прострелы и автоматически переставляет ордер при движении цены

use crate::backtest::market::TradeTick;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::VecDeque;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MShotConfig {
    // Основные параметры цены
    pub mshot_price: f64,              // % от текущей цены для buy ордера
    pub mshot_price_min: f64,          // Мин. % когда переставлять ордер
    pub mshot_minus_satoshi: bool,      // Отступ от ASK на 2 сатоши
    
    // Модификаторы дельт
    pub mshot_add_3h_delta: f64,       // Коэффициент 3ч дельты
    pub mshot_add_hourly_delta: f64,   // Коэффициент часовой дельты
    pub mshot_add_15min_delta: f64,    // Коэффициент 15м дельты
    pub mshot_add_market_delta: f64,   // Коэффициент дельты маркета
    pub mshot_add_btc_delta: f64,      // Коэффициент дельты BTC
    pub mshot_add_btc_5m_delta: f64,   // Коэффициент 5м дельты BTC
    pub mshot_add_distance: f64,       // Расширение дальней границы
    pub mshot_add_price_bug: f64,      // Модификатор от PriceBug (лаги биржи)
    
    // Параметры продажи
    pub mshot_sell_at_last_price: bool, // Продавать по последней цене ASK
    pub mshot_sell_price_adjust: f64,   // Поправка к ASK цене (%)
    
    // Задержки переставления
    pub mshot_replace_delay: f64,      // Задержка перед переставлением (сек, до мс)
    pub mshot_raise_wait: f64,         // Задержка при росте цены (сек)
    
    // Сортировка монет
    pub mshot_sort_by: String,         // LastNhDelta, DVolToHVolAsc, DailyVol, etc.
    pub mshot_sort_desc: bool,         // Сортировка по убыванию
    
    // Источник цены для расчета
    pub mshot_use_price: String,       // BID, ASK, Trade
    
    // Повторные сработки (Repeat)
    pub mshot_repeat_after_buy: bool,  // Ставить повторный шот после покупки
    pub mshot_repeat_if_profit: f64,   // % прибыли для повторного шота
    pub mshot_repeat_wait: f64,        // Время ожидания для повторного шота (сек)
    pub mshot_repeat_delay: f64,       // Задержка перед повторным шотом (сек)
    
    // Общие параметры (из базовой стратегии)
    pub order_size: f64,               // Размер ордера
    pub sell_price: f64,               // Цена продажи (%)
    pub use_stop_loss: bool,           // Использовать стоп-лосс
    pub use_trailing: bool,            // Использовать трейлинг
    pub use_take_profit: bool,         // Использовать тейк-профит
}

impl Default for MShotConfig {
    fn default() -> Self {
        MShotConfig {
            mshot_price: 10.0,
            mshot_price_min: 7.0,
            mshot_minus_satoshi: false,
            mshot_add_3h_delta: 0.0,
            mshot_add_hourly_delta: 0.0,
            mshot_add_15min_delta: 0.0,
            mshot_add_market_delta: 0.0,
            mshot_add_btc_delta: 0.0,
            mshot_add_btc_5m_delta: 0.0,
            mshot_add_distance: 0.0,
            mshot_add_price_bug: 0.2,
            mshot_sell_at_last_price: false,
            mshot_sell_price_adjust: 1.0,
            mshot_replace_delay: 0.0,
            mshot_raise_wait: 0.0,
            mshot_sort_by: "LastNhDelta".to_string(),
            mshot_sort_desc: true,
            mshot_use_price: "ASK".to_string(),
            mshot_repeat_after_buy: false,
            mshot_repeat_if_profit: 0.0,
            mshot_repeat_wait: 5.0,
            mshot_repeat_delay: 0.0,
            order_size: 100.0,
            sell_price: 1.35,
            use_stop_loss: false,
            use_trailing: false,
            use_take_profit: false,
        }
    }
}

#[derive(Debug, Clone)]
pub struct MShotState {
    active_buy_order: Option<BuyOrderState>,
    last_ask_price: Option<f64>,
    last_ask_time: Option<DateTime<Utc>>,
    repeat_shots: Vec<RepeatShotState>,
    
    // Текущие дельты (для модификаторов)
    delta_3h: f64,
    delta_hourly: f64,
    delta_15min: f64,
    delta_market: f64,
    delta_btc: f64,
    delta_btc_5m: f64,
    
    // История цен для расчета
    price_history: VecDeque<(DateTime<Utc>, f64)>,
}

#[derive(Debug, Clone)]
struct BuyOrderState {
    price: f64,
    size: f64,
    placed_at: DateTime<Utc>,
    original_price: f64, // Цена до применения модификаторов
}

#[derive(Debug, Clone)]
struct RepeatShotState {
    buy_price: f64,
    buy_time: DateTime<Utc>,
    active: bool,
}

#[derive(Debug, Clone)]
pub enum MShotSignal {
    PlaceBuy { price: f64, size: f64 },
    ReplaceBuy { new_price: f64 },
    CancelBuy,
    PlaceSell { price: f64, size: f64 },
    RepeatShot { price: f64, size: f64 },
    NoAction,
}

pub struct MShotStrategy {
    config: MShotConfig,
    state: MShotState,
}

impl MShotStrategy {
    pub fn new(config: MShotConfig) -> Self {
        Self {
            config,
            state: MShotState {
                active_buy_order: None,
                last_ask_price: None,
                last_ask_time: None,
                repeat_shots: Vec::new(),
                delta_3h: 0.0,
                delta_hourly: 0.0,
                delta_15min: 0.0,
                delta_market: 0.0,
                delta_btc: 0.0,
                delta_btc_5m: 0.0,
                price_history: VecDeque::new(),
            },
        }
    }
    
    /// Обработка нового тика
    pub fn on_tick(&mut self, tick: &TradeTick, deltas: &Deltas) -> MShotSignal {
        let now = tick.timestamp;
        let current_price = tick.price;
        
        // Обновляем дельты
        self.update_deltas(deltas);
        
        // Обновляем историю цен
        self.state.price_history.push_back((now, current_price));
        if self.state.price_history.len() > 1000 {
            self.state.price_history.pop_front();
        }
        
        // Определяем базовую цену (BID/ASK/Trade)
        let base_price = match self.config.mshot_use_price.as_str() {
            "BID" => tick.best_bid.unwrap_or(current_price),
            "ASK" => tick.best_ask.unwrap_or(current_price),
            "Trade" => current_price,
            _ => current_price,
        };
        
        if let Some(ask) = tick.best_ask {
            self.state.last_ask_price = Some(ask);
            self.state.last_ask_time = Some(now);
        }
        
        // Вычисляем модифицированные параметры с учетом дельт
        let (effective_price, effective_price_min) = self.calculate_effective_prices(base_price);
        
        // Обработка активного buy ордера
        if let Some(ref buy_order) = self.state.active_buy_order {
            // Клонируем необходимые данные для избежания множественных заимствований
            let order_price = buy_order.price;
            let order_placed_at = buy_order.placed_at;
            let order_original_price = buy_order.original_price;
            
            // Вычисляем сигнал (без мутабельного заимствования)
            let signal = self.compute_order_signal(
                order_price,
                order_placed_at,
                order_original_price,
                base_price,
                effective_price,
                effective_price_min,
                now,
            );
            
            // Применяем изменения к ордеру если нужно
            if let MShotSignal::ReplaceBuy { new_price } = signal {
                if let Some(ref mut buy_order_mut) = self.state.active_buy_order {
                    buy_order_mut.price = new_price;
                    buy_order_mut.placed_at = now;
                }
                return MShotSignal::ReplaceBuy { new_price };
            }
            
            return signal;
        }
        
        // Проверка повторных шотов
        if !self.state.repeat_shots.is_empty() {
            if let Some(signal) = self.check_repeat_shots(current_price, base_price, effective_price, now) {
                return signal;
            }
        }
        
        // Выставление нового ордера (если нет активного)
        if self.should_place_order(base_price, effective_price, effective_price_min) {
            let buy_price = self.calculate_buy_price(base_price, effective_price);
            let order_size = self.config.order_size;
            
            self.state.active_buy_order = Some(BuyOrderState {
                price: buy_price,
                size: order_size,
                placed_at: now,
                original_price: effective_price,
            });
            
            return MShotSignal::PlaceBuy {
                price: buy_price,
                size: order_size,
            };
        }
        
        MShotSignal::NoAction
    }
    
    fn update_deltas(&mut self, deltas: &Deltas) {
        self.state.delta_3h = deltas.delta_3h;
        self.state.delta_hourly = deltas.delta_hourly;
        self.state.delta_15min = deltas.delta_15min;
        self.state.delta_market = deltas.delta_market;
        self.state.delta_btc = deltas.delta_btc;
        self.state.delta_btc_5m = deltas.delta_btc_5m;
    }
    
    fn calculate_effective_prices(&self, base_price: f64) -> (f64, f64) {
        // Базовые значения
        let mut price = self.config.mshot_price;
        let mut price_min = self.config.mshot_price_min;
        
        // Применяем модификаторы дельт
        let delta_adjustment = 
            self.state.delta_3h * self.config.mshot_add_3h_delta +
            self.state.delta_hourly * self.config.mshot_add_hourly_delta +
            self.state.delta_15min * self.config.mshot_add_15min_delta +
            self.state.delta_market * self.config.mshot_add_market_delta +
            self.state.delta_btc * self.config.mshot_add_btc_delta +
            self.state.delta_btc_5m * self.config.mshot_add_btc_5m_delta;
        
        price += delta_adjustment;
        price_min += delta_adjustment;
        
        // Расширение дальней границы
        if self.config.mshot_add_distance > 0.0 {
            let distance_mult = 1.0 + self.config.mshot_add_distance / 100.0;
            price = price_min + (price - price_min) * distance_mult;
        }
        
        (price, price_min)
    }
    
    fn calculate_buy_price(&self, base_price: f64, effective_price: f64) -> f64 {
        let mut buy_price = base_price * (1.0 - effective_price / 100.0);
        
        // MShotMinusSatoshi: отступ от ASK на 2 сатоши
        if self.config.mshot_minus_satoshi {
            if let Some(ask) = self.state.last_ask_price {
                let min_price = ask * 0.99998; // 2 сатоши ≈ 0.002%
                if buy_price > min_price {
                    buy_price = min_price;
                }
            }
        }
        
        buy_price
    }
    
    fn should_place_order(&self, base_price: f64, effective_price: f64, effective_price_min: f64) -> bool {
        // Логика определения момента выставления ордера
        // Здесь можно добавить дополнительные фильтры
        true
    }
    
    fn compute_order_signal(
        &self,
        order_price: f64,
        order_placed_at: DateTime<Utc>,
        _order_original_price: f64,
        base_price: f64,
        effective_price: f64,
        effective_price_min: f64,
        now: DateTime<Utc>,
    ) -> MShotSignal {
        // Проверяем, нужно ли переставить ордер
        let distance_from_base = (base_price - order_price) / base_price * 100.0;
        
        // Проверка MShotPriceMin: если цена подошла слишком близко
        if distance_from_base <= effective_price_min {
            // Проверяем задержку ReplaceDelay
            let time_since_last_move = (now - order_placed_at).num_milliseconds() as f64 / 1000.0;
            if time_since_last_move >= self.config.mshot_replace_delay {
                let new_price = base_price * (1.0 - effective_price / 100.0);
                return MShotSignal::ReplaceBuy { new_price };
            }
        }
        
        // Проверка RaiseWait: задержка при росте цены
        if distance_from_base > effective_price {
            let time_since_last_move = (now - order_placed_at).num_milliseconds() as f64 / 1000.0;
            if time_since_last_move >= self.config.mshot_raise_wait {
                let new_price = base_price * (1.0 - effective_price / 100.0);
                return MShotSignal::ReplaceBuy { new_price };
            }
        }
        
        MShotSignal::NoAction
    }
    
    fn check_repeat_shots(
        &mut self,
        current_price: f64,
        base_price: f64,
        effective_price: f64,
        now: DateTime<Utc>,
    ) -> Option<MShotSignal> {
        // Проверяем условия для повторных шотов
        for repeat in &mut self.state.repeat_shots {
            if !repeat.active {
                continue;
            }
            
            let profit_pct = (current_price - repeat.buy_price) / repeat.buy_price * 100.0;
            let time_since_buy = (now - repeat.buy_time).num_seconds() as f64;
            
            if profit_pct >= self.config.mshot_repeat_if_profit &&
               time_since_buy <= self.config.mshot_repeat_wait {
                // Выставляем повторный шот
                let new_buy_price = base_price * (1.0 - effective_price / 100.0);
                
                return Some(MShotSignal::RepeatShot {
                    price: new_buy_price,
                    size: self.config.order_size,
                });
            }
        }
        
        None
    }
    
    /// Вызывается при исполнении buy ордера
    pub fn on_buy_filled(&mut self, price: f64, size: f64) {
        // Запускаем повторный шот если настроено
        if self.config.mshot_repeat_after_buy {
            self.state.repeat_shots.push(RepeatShotState {
                buy_price: price,
                buy_time: Utc::now(),
                active: true,
            });
        }
        
        self.state.active_buy_order = None;
    }
    
    /// Вычисление цены продажи
    pub fn calculate_sell_price(&self, buy_price: f64, current_ask: Option<f64>) -> f64 {
        if self.config.mshot_sell_at_last_price {
            // Используем последнюю ASK цену с поправкой
            if let Some(ask) = current_ask.or(self.state.last_ask_price) {
                let adjusted_ask = ask * (1.0 - self.config.mshot_sell_price_adjust / 100.0);
                let sell_by_config = buy_price * (1.0 + self.config.sell_price / 100.0);
                return adjusted_ask.max(sell_by_config);
            }
        }
        
        // Стандартная цена продажи
        buy_price * (1.0 + self.config.sell_price / 100.0)
    }
}

#[derive(Debug, Clone, Default)]
pub struct Deltas {
    pub delta_3h: f64,
    pub delta_hourly: f64,
    pub delta_15min: f64,
    pub delta_market: f64,
    pub delta_btc: f64,
    pub delta_btc_5m: f64,
}

