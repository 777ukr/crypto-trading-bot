//! MoonBot стратегии: MShot, MStrike, Hook, Spread
//! Полная реализация всех параметров для бэктестинга и ИИ оптимизации

pub mod mshot;
pub mod mstrike;
pub mod hook;
pub mod spread;
pub mod ema_filter;
pub mod triggers;
pub mod sessions;

pub use mshot::{MShotStrategy, MShotConfig, MShotSignal};
pub use mstrike::{MStrikeStrategy, MStrikeConfig, MStrikeSignal, MStrikeDirection};
pub use hook::{HookStrategy, HookConfig, HookSignal, HookDirection};
pub use spread::{SpreadStrategy, SpreadConfig, SpreadSignal};
pub use ema_filter::{EmaFilter, EmaFilterCondition};
pub use triggers::{TriggerManager, TriggerKey};
pub use sessions::{SessionManager, SessionState};

