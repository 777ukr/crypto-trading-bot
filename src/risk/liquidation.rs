//! Liquidation Control - контроль ликвидации для высоких плеч
//!
//! Функции:
//! - Расчет риска ликвидации
//! - Предупреждения о близости к ликвидации
//! - Автоматическое уменьшение позиции при риске

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum LiquidationWarning {
    None,
    Low,      // 20-30% до ликвидации
    Medium,   // 10-20% до ликвидации
    High,     // 5-10% до ликвидации
    Critical, // < 5% до ликвидации
}

#[derive(Debug, Clone)]
pub struct LiquidationControl {
    pub enabled: bool,
    pub max_leverage: u32,
    pub maintenance_margin_rate: f64, // Процент маржи для поддержания позиции (обычно 0.5-1%)
    pub liquidation_price_threshold: f64, // % до ликвидации для предупреждения (например, 20%)
}

impl Default for LiquidationControl {
    fn default() -> Self {
        Self {
            enabled: true,
            max_leverage: 125,
            maintenance_margin_rate: 0.01, // 1%
            liquidation_price_threshold: 20.0, // Предупреждение при 20% до ликвидации
        }
    }
}

impl LiquidationControl {
    pub fn new(
        enabled: bool,
        max_leverage: u32,
        maintenance_margin_rate: f64,
        liquidation_price_threshold: f64,
    ) -> Self {
        Self {
            enabled,
            max_leverage,
            maintenance_margin_rate,
            liquidation_price_threshold,
        }
    }

    /// Проверяет риск ликвидации для позиции
    ///
    /// # Arguments
    /// * `position_size` - размер позиции (положительное для long, отрицательное для short)
    /// * `entry_price` - цена входа
    /// * `mark_price` - текущая марк-цена
    /// * `balance` - текущий баланс
    /// * `leverage` - используемое плечо
    ///
    /// # Returns
    /// `LiquidationWarning` - уровень предупреждения
    pub fn check_liquidation_risk(
        &self,
        position_size: f64,
        entry_price: f64,
        mark_price: f64,
        _balance: f64,
        leverage: f64,
    ) -> LiquidationWarning {
        if !self.enabled || position_size == 0.0 || entry_price <= 0.0 || mark_price <= 0.0 {
            return LiquidationWarning::None;
        }

        // Вычисляем цену ликвидации
        let liquidation_price = self.calculate_liquidation_price(
            position_size,
            entry_price,
            _balance,
            leverage,
        );

        // Вычисляем процент до ликвидации
        let is_long = position_size > 0.0;
        let price_distance = if is_long {
            // Для long: цена ликвидации ниже mark_price
            (mark_price - liquidation_price) / mark_price * 100.0
        } else {
            // Для short: цена ликвидации выше mark_price
            (liquidation_price - mark_price) / mark_price * 100.0
        };

        // Определяем уровень предупреждения
        if price_distance < 5.0 {
            LiquidationWarning::Critical
        } else if price_distance < 10.0 {
            LiquidationWarning::High
        } else if price_distance < 20.0 {
            LiquidationWarning::Medium
        } else if price_distance < 30.0 {
            LiquidationWarning::Low
        } else {
            LiquidationWarning::None
        }
    }

    /// Вычисляет цену ликвидации
    ///
    /// Формула для long:
    /// liquidation_price = entry_price * (1 - (1 - maintenance_margin_rate) / leverage)
    ///
    /// Формула для short:
    /// liquidation_price = entry_price * (1 + (1 - maintenance_margin_rate) / leverage)
    fn calculate_liquidation_price(
        &self,
        position_size: f64,
        entry_price: f64,
        balance: f64,
        leverage: f64,
    ) -> f64 {
        let is_long = position_size > 0.0;
        let margin_factor = (1.0 - self.maintenance_margin_rate) / leverage;

        if is_long {
            entry_price * (1.0 - margin_factor)
        } else {
            entry_price * (1.0 + margin_factor)
        }
    }

    /// Рекомендует размер позиции для уменьшения риска ликвидации
    ///
    /// # Returns
    /// `Some(new_size)` если нужно уменьшить позицию, `None` если все в порядке
    pub fn should_reduce_position(
        &self,
        warning: LiquidationWarning,
        current_size: f64,
    ) -> Option<f64> {
        if !self.enabled {
            return None;
        }

        match warning {
            LiquidationWarning::Critical => Some(current_size * 0.5), // Уменьшить на 50%
            LiquidationWarning::High => Some(current_size * 0.7),     // Уменьшить на 30%
            LiquidationWarning::Medium => Some(current_size * 0.85),   // Уменьшить на 15%
            LiquidationWarning::Low | LiquidationWarning::None => None,
        }
    }

    /// Проверяет, можно ли открыть новую позицию с учетом риска ликвидации
    pub fn can_open_position(
        &self,
        proposed_size: f64,
        entry_price: f64,
        balance: f64,
        leverage: f64,
        current_positions_size: f64,
    ) -> bool {
        if !self.enabled {
            return true;
        }

        // Проверяем, что общий размер позиций не превышает лимит
        let total_size = (current_positions_size.abs() + proposed_size.abs()) * entry_price;
        let max_position_value = balance * leverage;

        total_size <= max_position_value
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_liquidation_price_long() {
        let control = LiquidationControl::default();
        let liquidation_price = control.calculate_liquidation_price(
            1.0,  // long position
            100.0, // entry price
            1000.0, // balance
            10.0,  // leverage
        );

        // Для long: liquidation_price должна быть ниже entry_price
        assert!(liquidation_price < 100.0);
    }

    #[test]
    fn test_liquidation_price_short() {
        let control = LiquidationControl::default();
        let liquidation_price = control.calculate_liquidation_price(
            -1.0, // short position
            100.0,
            1000.0,
            10.0,
        );

        // Для short: liquidation_price должна быть выше entry_price
        assert!(liquidation_price > 100.0);
    }

    #[test]
    fn test_liquidation_warning() {
        let control = LiquidationControl::default();
        
        // Тест с высоким риском (цена близка к ликвидации)
        let warning = control.check_liquidation_risk(
            1.0,
            100.0,
            95.0, // mark_price близко к liquidation_price
            1000.0,
            10.0,
        );

        assert!(matches!(warning, LiquidationWarning::High | LiquidationWarning::Critical));
    }

    #[test]
    fn test_reduce_position() {
        let control = LiquidationControl::default();
        
        assert_eq!(
            control.should_reduce_position(LiquidationWarning::Critical, 100.0),
            Some(50.0)
        );
        
        assert_eq!(
            control.should_reduce_position(LiquidationWarning::None, 100.0),
            None
        );
    }
}

