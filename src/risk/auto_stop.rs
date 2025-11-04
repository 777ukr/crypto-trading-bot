//! Auto Stop on Errors/Ping - автоматическая остановка при критических условиях
//!
//! Функции:
//! - Auto Stop if errors level >= N
//! - Auto Stop if Ping > N ms
//! - Panic Sell опция при остановке
//! - Restart in N minutes после остановки

use chrono::{DateTime, Utc, Duration};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StopReason {
    None,
    ErrorLevelExceeded,
    PingTooHigh,
    Manual,
}

#[derive(Debug, Clone)]
pub struct AutoStopManager {
    pub max_error_level: u32,
    pub current_error_level: u32,
    pub max_ping_ms: u64,
    pub panic_sell_on_stop: bool,
    pub restart_after_minutes: Option<u32>,
    
    pub stopped_at: Option<DateTime<Utc>>,
    pub stop_reason: StopReason,
    pub error_history: Vec<DateTime<Utc>>, // История ошибок для decay
}

impl Default for AutoStopManager {
    fn default() -> Self {
        Self {
            max_error_level: 3,
            current_error_level: 0,
            max_ping_ms: 1000,
            panic_sell_on_stop: false,
            restart_after_minutes: Some(5),
            stopped_at: None,
            stop_reason: StopReason::None,
            error_history: Vec::new(),
        }
    }
}

impl AutoStopManager {
    pub fn new(
        max_error_level: u32,
        max_ping_ms: u64,
        panic_sell_on_stop: bool,
        restart_after_minutes: Option<u32>,
    ) -> Self {
        Self {
            max_error_level,
            current_error_level: 0,
            max_ping_ms,
            panic_sell_on_stop,
            restart_after_minutes,
            stopped_at: None,
            stop_reason: StopReason::None,
            error_history: Vec::new(),
        }
    }

    /// Записывает ошибку и увеличивает уровень ошибок
    pub fn record_error(&mut self) {
        let now = Utc::now();
        self.error_history.push(now);
        self.current_error_level += 1;
        
        // Удаляем старые ошибки (старше 1 часа) для decay
        self.error_history.retain(|&ts| {
            now.signed_duration_since(ts) < Duration::hours(1)
        });
        
        // Пересчитываем уровень ошибок на основе истории
        // Ошибки старше 1 часа не учитываются
        self.current_error_level = self.error_history.len() as u32;
    }

    /// Проверяет пинг и возвращает true если нужно остановиться
    pub fn check_ping(&mut self, ping_ms: u64) -> bool {
        if ping_ms > self.max_ping_ms {
            self.stop(StopReason::PingTooHigh);
            return true;
        }
        false
    }

    /// Проверяет уровень ошибок и возвращает true если нужно остановиться
    pub fn check_errors(&mut self) -> bool {
        if self.current_error_level >= self.max_error_level {
            self.stop(StopReason::ErrorLevelExceeded);
            return true;
        }
        false
    }

    /// Останавливает торговлю
    pub fn stop(&mut self, reason: StopReason) {
        if self.stopped_at.is_none() {
            self.stopped_at = Some(Utc::now());
            self.stop_reason = reason;
        }
    }

    /// Проверяет, нужно ли перезапуститься
    pub fn should_restart(&self) -> bool {
        if let (Some(stopped_at), Some(minutes)) = (self.stopped_at, self.restart_after_minutes) {
            let elapsed = Utc::now() - stopped_at;
            return elapsed >= Duration::minutes(minutes as i64);
        }
        false
    }

    /// Перезапускает менеджер (сбрасывает состояние)
    pub fn restart(&mut self) {
        self.stopped_at = None;
        self.stop_reason = StopReason::None;
        self.current_error_level = 0;
        self.error_history.clear();
    }

    /// Проверяет, остановлена ли торговля
    pub fn is_stopped(&self) -> bool {
        self.stopped_at.is_some()
    }

    /// Получить причину остановки
    pub fn stop_reason(&self) -> StopReason {
        self.stop_reason
    }

    /// Получить время остановки
    pub fn stopped_at(&self) -> Option<DateTime<Utc>> {
        self.stopped_at
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_error_recording() {
        let mut manager = AutoStopManager::default();
        assert_eq!(manager.current_error_level, 0);
        
        manager.record_error();
        assert_eq!(manager.current_error_level, 1);
        
        manager.record_error();
        assert_eq!(manager.current_error_level, 2);
    }

    #[test]
    fn test_stop_on_error_level() {
        let mut manager = AutoStopManager::new(3, 1000, false, None);
        
        manager.record_error();
        assert!(!manager.check_errors());
        
        manager.record_error();
        assert!(!manager.check_errors());
        
        manager.record_error();
        assert!(manager.check_errors());
        assert!(manager.is_stopped());
        assert_eq!(manager.stop_reason(), StopReason::ErrorLevelExceeded);
    }

    #[test]
    fn test_stop_on_ping() {
        let mut manager = AutoStopManager::new(3, 1000, false, None);
        
        assert!(!manager.check_ping(500));
        assert!(!manager.is_stopped());
        
        assert!(manager.check_ping(1500));
        assert!(manager.is_stopped());
        assert_eq!(manager.stop_reason(), StopReason::PingTooHigh);
    }

    #[test]
    fn test_restart() {
        let mut manager = AutoStopManager::new(3, 1000, false, Some(1));
        
        manager.stop(StopReason::Manual);
        assert!(manager.is_stopped());
        
        // Симулируем прошествие времени (в реальности нужно ждать)
        // Для теста просто проверяем логику
        manager.restart();
        assert!(!manager.is_stopped());
        assert_eq!(manager.current_error_level, 0);
    }
}

