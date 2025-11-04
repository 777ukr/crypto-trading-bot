//! Panic Sell System - автоматическая паник-продажа при критических условиях
//! 
//! Функции:
//! - Panic Sell drop price to [actual buy] +X%
//! - Panic Sell Spread: X%
//! - Auto Panic Sell If price drops < [actual buy] -X%
//! - Panic Sell If BIDs at [buy] +X% drops

// DateTime и Utc не используются напрямую, но могут понадобиться в будущем

#[derive(Debug, Clone)]
pub struct PanicSellManager {
    pub enabled: bool,
    pub drop_to_percent: f64, // % от цены покупки (например, +2% = 1.02)
    pub spread_percent: f64,  // Spread для паник-продажи (например, 1% = 0.01)
    pub auto_panic_if_drop: Option<f64>, // Автоматический паник при падении < X% (отрицательное значение)
    pub panic_if_bids_drop: Option<f64>, // Паник если BID упали на X% от цены покупки
}

impl Default for PanicSellManager {
    fn default() -> Self {
        Self {
            enabled: false,
            drop_to_percent: 1.02, // По умолчанию +2%
            spread_percent: 0.01,  // 1% spread
            auto_panic_if_drop: None,
            panic_if_bids_drop: None,
        }
    }
}

impl PanicSellManager {
    pub fn new(
        enabled: bool,
        drop_to_percent: f64,
        spread_percent: f64,
        auto_panic_if_drop: Option<f64>,
        panic_if_bids_drop: Option<f64>,
    ) -> Self {
        Self {
            enabled,
            drop_to_percent,
            spread_percent,
            auto_panic_if_drop,
            panic_if_bids_drop,
        }
    }

    /// Проверяет, нужно ли делать паник-продажу
    /// 
    /// Возвращает Some(panic_price) если нужно продавать, None если нет
    pub fn should_panic_sell(
        &self,
        buy_price: f64,
        current_price: f64,
        best_bid: Option<f64>,
    ) -> Option<f64> {
        if !self.enabled || buy_price <= 0.0 {
            return None;
        }

        // 1. Auto Panic Sell If price drops < [actual buy] -X%
        if let Some(threshold) = self.auto_panic_if_drop {
            let drop_threshold = buy_price * (1.0 - threshold.abs() / 100.0);
            if current_price < drop_threshold {
                return Some(self.calculate_panic_price(buy_price));
            }
        }

        // 2. Panic Sell If BIDs at [buy] +X% drops
        if let (Some(threshold), Some(bid)) = (self.panic_if_bids_drop, best_bid) {
            let bid_threshold = buy_price * (1.0 + threshold / 100.0);
            if bid < bid_threshold {
                return Some(self.calculate_panic_price(buy_price));
            }
        }

        None
    }

    /// Вычисляет цену паник-продажи
    /// 
    /// Формула: buy_price * drop_to_percent - spread
    pub fn calculate_panic_price(&self, buy_price: f64) -> f64 {
        let target_price = buy_price * self.drop_to_percent;
        target_price * (1.0 - self.spread_percent)
    }

    /// Принудительная паник-продажа (вызывается вручную или глобальным риск-менеджером)
    pub fn force_panic_sell(&self, buy_price: f64) -> f64 {
        self.calculate_panic_price(buy_price)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_panic_sell_calculation() {
        let manager = PanicSellManager::new(
            true,
            1.02,  // +2%
            0.01,  // 1% spread
            None,
            None,
        );

        let buy_price = 100.0;
        let panic_price = manager.calculate_panic_price(buy_price);
        
        // 100 * 1.02 * 0.99 = 100.98
        assert!((panic_price - 100.98).abs() < 0.01);
    }

    #[test]
    fn test_auto_panic_on_drop() {
        let manager = PanicSellManager::new(
            true,
            1.02,
            0.01,
            Some(5.0), // Автоматический паник при падении > 5%
            None,
        );

        let buy_price = 100.0;
        let current_price = 94.0; // Падение на 6%
        
        assert!(manager.should_panic_sell(buy_price, current_price, None).is_some());
    }

    #[test]
    fn test_panic_on_bid_drop() {
        let manager = PanicSellManager::new(
            true,
            1.02,
            0.01,
            None,
            Some(3.0), // Паник если BID упали на 3% от buy + 3%
        );

        let buy_price = 100.0;
        let current_price = 100.0;
        let best_bid = 102.0; // BID ниже порога (buy_price * 1.03 = 103)
        
        assert!(manager.should_panic_sell(buy_price, current_price, Some(best_bid)).is_some());
    }

    #[test]
    fn test_panic_disabled() {
        let manager = PanicSellManager::new(
            false,
            1.02,
            0.01,
            Some(5.0),
            None,
        );

        let buy_price = 100.0;
        let current_price = 90.0; // Падение на 10%
        
        assert!(manager.should_panic_sell(buy_price, current_price, None).is_none());
    }
}


