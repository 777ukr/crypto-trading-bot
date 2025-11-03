//! Метрики производительности торговли

// use crate::analytics::trade_analyzer::TradeRecord;

#[derive(Debug, Clone)]
pub struct PerformanceMetrics {
    pub total_trades: usize,
    pub winning_trades: usize,
    pub losing_trades: usize,
    pub total_pnl: f64,
    pub total_pnl_percent: f64,
    pub win_rate: f64,
    pub avg_win: f64,
    pub avg_loss: f64,
    pub profit_factor: f64,
    pub max_drawdown: f64,
    pub sharpe_ratio: f64,
    pub max_consecutive_wins: usize,
    pub max_consecutive_losses: usize,
}

impl PerformanceMetrics {
    pub fn calculate(trades: &[crate::analytics::trade_analyzer::TradeRecord]) -> Self {
        if trades.is_empty() {
            return Self::default();
        }

        let winning: Vec<_> = trades.iter().filter(|t| t.pnl.unwrap_or(0.0) > 0.0).collect();
        let losing: Vec<_> = trades.iter().filter(|t| t.pnl.unwrap_or(0.0) < 0.0).collect();
        
        let total_pnl = trades.iter()
            .map(|t| t.pnl.unwrap_or(0.0))
            .sum::<f64>();
        
        let avg_win = if !winning.is_empty() {
            winning.iter().map(|t| t.pnl.unwrap_or(0.0)).sum::<f64>() / winning.len() as f64
        } else {
            0.0
        };
        
        let avg_loss = if !losing.is_empty() {
            losing.iter().map(|t| t.pnl.unwrap_or(0.0)).sum::<f64>() / losing.len() as f64
        } else {
            0.0
        };
        
        let profit_factor = if avg_loss.abs() > 0.0 {
            (avg_win * winning.len() as f64) / (avg_loss.abs() * losing.len() as f64)
        } else if !winning.is_empty() {
            f64::INFINITY
        } else {
            0.0
        };

        let win_rate = trades.len() as f64 / trades.len() as f64 * 100.0;
        
        // Простой расчет drawdown
        let mut max_drawdown = 0.0;
        let mut peak = 0.0;
        let mut cumulative = 0.0;
        for trade in trades {
            cumulative += trade.pnl.unwrap_or(0.0);
            if cumulative > peak {
                peak = cumulative;
            }
            let drawdown = (peak - cumulative) / peak.max(1.0) * 100.0;
            if drawdown > max_drawdown {
                max_drawdown = drawdown;
            }
        }

        // Consecutive wins/losses
        let mut max_wins = 0;
        let mut max_losses = 0;
        let mut current_wins = 0;
        let mut current_losses = 0;
        
        for trade in trades {
            if trade.pnl.unwrap_or(0.0) > 0.0 {
                current_wins += 1;
                current_losses = 0;
                max_wins = max_wins.max(current_wins);
            } else {
                current_losses += 1;
                current_wins = 0;
                max_losses = max_losses.max(current_losses);
            }
        }

        // Простой Sharpe ratio (упрощенный)
        let returns: Vec<f64> = trades.iter()
            .map(|t| t.pnl.unwrap_or(0.0))
            .collect();
        let avg_return = if !returns.is_empty() {
            returns.iter().sum::<f64>() / returns.len() as f64
        } else {
            0.0
        };
        let variance = if returns.len() > 1 {
            returns.iter()
                .map(|r| (r - avg_return).powi(2))
                .sum::<f64>() / (returns.len() - 1) as f64
        } else {
            0.0
        };
        let std_dev = variance.sqrt();
        let sharpe = if std_dev > 0.0 { avg_return / std_dev } else { 0.0 };

        Self {
            total_trades: trades.len(),
            winning_trades: winning.len(),
            losing_trades: losing.len(),
            total_pnl,
            total_pnl_percent: if trades.is_empty() { 0.0 } else { total_pnl / trades[0].entry_price * 100.0 },
            win_rate: (winning.len() as f64 / trades.len() as f64) * 100.0,
            avg_win,
            avg_loss,
            profit_factor,
            max_drawdown,
            sharpe_ratio: sharpe,
            max_consecutive_wins: max_wins,
            max_consecutive_losses: max_losses,
        }
    }

    pub fn print(&self) {
        println!("\n📊 Performance Metrics:");
        println!("  Total trades: {}", self.total_trades);
        println!("  Wins: {} | Losses: {}", self.winning_trades, self.losing_trades);
        println!("  Win rate: {:.2}%", self.win_rate);
        println!("  Total P&L: ${:.2} ({:.2}%)", self.total_pnl, self.total_pnl_percent);
        println!("  Avg win: ${:.2} | Avg loss: ${:.2}", self.avg_win, self.avg_loss);
        println!("  Profit factor: {:.2}", self.profit_factor);
        println!("  Max drawdown: {:.2}%", self.max_drawdown);
        println!("  Sharpe ratio: {:.2}", self.sharpe_ratio);
        println!("  Max consecutive: {} wins, {} losses", 
                 self.max_consecutive_wins, self.max_consecutive_losses);
    }
}

impl Default for PerformanceMetrics {
    fn default() -> Self {
        Self {
            total_trades: 0,
            winning_trades: 0,
            losing_trades: 0,
            total_pnl: 0.0,
            total_pnl_percent: 0.0,
            win_rate: 0.0,
            avg_win: 0.0,
            avg_loss: 0.0,
            profit_factor: 0.0,
            max_drawdown: 0.0,
            sharpe_ratio: 0.0,
            max_consecutive_wins: 0,
            max_consecutive_losses: 0,
        }
    }
}

// TradeRecord определен в crate::analytics::trade_analyzer

