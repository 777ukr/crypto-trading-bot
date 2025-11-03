//! BTC Trading Strategy with Stop-Loss
//! 
//! Использует индикаторы для генерации торговых сигналов и управляет позицией со стоп-лоссом

#![allow(dead_code)]

use std::collections::VecDeque;
use std::time::Instant;

use crate::base_classes::types::Side;
use crate::execution::{
    ClientOrderId, ExecutionReport, OrderStatus, QuoteIntent, TimeInForce, Venue,
};
use crate::models::Position;
use crate::strategy::stop_loss::{check_stop_loss, check_take_profit};

/// Конфигурация BTC стратегии
#[derive(Debug, Clone)]
pub struct BtcStrategyConfig {
    pub symbol: String,
    pub entry_size: f64,
    pub max_position_size: f64,
    pub stop_loss_percent: f64,      // Например, 2.0 = 2% стоп-лосс
    pub take_profit_percent: f64,     // Например, 4.0 = 4% тейк-профит
    pub rsi_period: usize,
    pub rsi_oversold: f64,            // Уровень перепроданности (например, 30)
    pub rsi_overbought: f64,         // Уровень перекупленности (например, 70)
    pub min_tick: f64,
}

impl Default for BtcStrategyConfig {
    fn default() -> Self {
        Self {
            symbol: "BTC_USDT".to_string(),
            entry_size: 0.001,  // 0.001 BTC
            max_position_size: 0.01,  // Максимум 0.01 BTC
            stop_loss_percent: 2.0,
            take_profit_percent: 4.0,
            rsi_period: 14,
            rsi_oversold: 30.0,
            rsi_overbought: 70.0,
            min_tick: 1e-2,
        }
    }
}

/// Состояние стратегии
pub struct BtcTradingStrategy {
    config: BtcStrategyConfig,
    current_position: Option<Position>,
    price_history: VecDeque<f64>,  // История цен для индикаторов
    latest_price: Option<f64>,
    next_order_id: u64,
}

impl BtcTradingStrategy {
    pub fn new(config: BtcStrategyConfig) -> Self {
        Self {
            config,
            current_position: None,
            price_history: VecDeque::with_capacity(100),
            latest_price: None,
            next_order_id: 0,
        }
    }

    /// Обновление рыночных данных
    pub fn update_market_data(&mut self, price: f64, _timestamp: Instant) {
        if !price.is_finite() || price <= 0.0 {
            return;
        }

        self.latest_price = Some(price);
        
        // Сохраняем историю цен для индикаторов
        self.price_history.push_back(price);
        if self.price_history.len() > 100 {
            self.price_history.pop_front();
        }
    }

    /// Проверка стоп-лосса и тейк-профита
    pub fn check_position_limits(&mut self, current_price: f64) -> Option<QuoteIntent> {
        if let Some(position) = &self.current_position {
            // Проверяем стоп-лосс
            if check_stop_loss(position, current_price) {
                return Some(self.create_close_order(current_price, position.clone()));
            }
            
            // Проверяем тейк-профит
            if check_take_profit(position, current_price) {
                return Some(self.create_close_order(current_price, position.clone()));
            }
        }
        None
    }

    /// Генерация торговых сигналов на основе индикаторов
    pub fn generate_signal(&mut self) -> Option<QuoteIntent> {
        let price = self.latest_price?;
        
        // Проверяем стоп-лосс/тейк-профит сначала
        if let Some(close_order) = self.check_position_limits(price) {
            return Some(close_order);
        }

        // Если уже есть позиция, не открываем новую
        if self.current_position.is_some() {
            return None;
        }

        // Вычисляем RSI (упрощенная версия)
        let rsi = self.calculate_simple_rsi()?;
        
        // Сигнал на покупку (oversold)
        if rsi < self.config.rsi_oversold {
            let entry_price = price * 0.999;  // Немного ниже рынка для лимитного ордера
            return Some(self.create_entry_order(Side::Bid, entry_price));
        }

        // Сигнал на продажу (overbought) - для шорта
        // if rsi > self.config.rsi_overbought {
        //     let entry_price = price * 1.001;
        //     return Some(self.create_entry_order(Side::Ask, entry_price));
        // }

        None
    }

    /// Упрощенный расчет RSI
    fn calculate_simple_rsi(&self) -> Option<f64> {
        if self.price_history.len() < self.config.rsi_period + 1 {
            return None;
        }

        let prices: Vec<f64> = self.price_history.iter().copied().collect();
        let mut gains = 0.0;
        let mut losses = 0.0;

        for i in 1..prices.len() {
            let change = prices[i] - prices[i - 1];
            if change > 0.0 {
                gains += change;
            } else {
                losses += change.abs();
            }
        }

        if losses == 0.0 {
            return Some(100.0);
        }

        let rs = gains / losses;
        let rsi = 100.0 - (100.0 / (1.0 + rs));
        Some(rsi)
    }

    /// Создание ордера на вход
    fn create_entry_order(&mut self, side: Side, price: f64) -> QuoteIntent {
        self.next_order_id += 1;
        let order_id = format!("btc-{:?}-{}", side, self.next_order_id);

        // Округляем цену до min_tick
        let rounded_price = (price / self.config.min_tick).round() * self.config.min_tick;

        QuoteIntent::new(
            Venue::Gate,
            self.config.symbol.clone(),
            side,
            rounded_price,
            self.config.entry_size,
            TimeInForce::Gtc,
            ClientOrderId::new(order_id),
        )
    }

    /// Создание ордера на закрытие позиции
    fn create_close_order(&mut self, current_price: f64, position: Position) -> QuoteIntent {
        self.next_order_id += 1;
        let order_id = format!("btc-close-{}", self.next_order_id);

        let side = match position.side.as_str() {
            "long" => Side::Ask,
            "short" => Side::Bid,
            _ => Side::Ask,
        };

        // Рыночный ордер для закрытия (используем текущую цену)
        let rounded_price = (current_price / self.config.min_tick).round() * self.config.min_tick;

        QuoteIntent::new(
            Venue::Gate,
            self.config.symbol.clone(),
            side,
            rounded_price,
            position.amount,
            TimeInForce::Ioc,  // Immediate or Cancel для быстрого закрытия
            ClientOrderId::new(order_id),
        )
    }

    /// Обработка исполнения ордера
    pub fn handle_execution(&mut self, report: &ExecutionReport) {
        match report.status {
            OrderStatus::Filled | OrderStatus::PartiallyFilled => {
                if let Some(avg_price) = report.avg_fill_price {
                    // Проверяем, это вход или выход
                    let id_str = report.client_order_id.0.as_str();
                    
                    if id_str.contains("close") {
                        // Закрываем позицию
                        self.current_position = None;
                    } else {
                        // Открываем новую позицию
                        let side = if id_str.contains("Bid") { "long" } else { "short" };
                        let stop_loss = if side == "long" {
                            avg_price * (1.0 - self.config.stop_loss_percent / 100.0)
                        } else {
                            avg_price * (1.0 + self.config.stop_loss_percent / 100.0)
                        };
                        let take_profit = if side == "long" {
                            avg_price * (1.0 + self.config.take_profit_percent / 100.0)
                        } else {
                            avg_price * (1.0 - self.config.take_profit_percent / 100.0)
                        };

                        self.current_position = Some(Position {
                            symbol: self.config.symbol.clone(),
                            side: side.to_string(),
                            entry_price: avg_price,
                            amount: report.filled_qty,
                            stop_loss,
                            take_profit,
                            entry_time: std::time::SystemTime::now()
                                .duration_since(std::time::UNIX_EPOCH)
                                .unwrap()
                                .as_secs(),
                        });
                    }
                }
            }
            _ => {}
        }
    }

    /// Получить текущую позицию
    pub fn current_position(&self) -> Option<&Position> {
        self.current_position.as_ref()
    }

    /// Получить текущую цену
    pub fn latest_price(&self) -> Option<f64> {
        self.latest_price
    }
}

