//! Risk Management модуль
//! Глобальное управление рисками, сессиями, паник-селлами

pub mod global;
pub mod session;
pub mod panic_sell;
pub mod auto_stop;
pub mod liquidation;

pub use global::{GlobalRiskManager, RiskAction};
pub use session::{SessionManager, SessionAction};
pub use panic_sell::{PanicSellManager};
pub use auto_stop::{AutoStopManager, StopReason};
pub use liquidation::{LiquidationControl, LiquidationWarning};

