//! Основной движок бэктестинга с поддержкой случайностей

// Trade используется только в типах, пока не используется
use chrono::{DateTime, Utc, Duration};
#[cfg(feature = "rand")]
use rand::Rng;
#[cfg(feature = "rand")]
use rand::SeedableRng;
#[cfg(feature = "rand")]
use rand::rngs::StdRng;
use std::collections::VecDeque;
use std::time::{SystemTime, UNIX_EPOCH};
use serde::{Deserialize, Serialize};

use super::market::{MarketState, TradeStream};
use super::emulator::MarketEmulator;
use super::metrics::{BacktestMetrics, BacktestResult};
use super::delta_calculator::DeltaCalculator;
#[cfg(feature = "gate_exec")]
use super::strategy_adapter::{StrategyAdapter, StrategyAction};
#[cfg(feature = "gate_exec")]
use crate::strategy::moon_strategies::mshot::Deltas;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum ExecutionMode {
    /// Режим эмулятора - только симуляция, без реальных ордеров
    Emulator,
    /// Реальный режим - НЕ должен использоваться в бэктесте!
    Real,
}

#[derive(Debug, Clone)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct BacktestSettings {
    /// Интервал между тиками в миллисекундах
    pub tick_interval_ms: u64,
    
    /// Диапазон случайной задержки сети (мс) - имитация лага трейдов
    pub latency_ms_range: (u64, u64),
    
    /// Случайное отклонение цены исполнения (сатоши)
    pub slippage_satoshi: i64,
    
    /// Seed для воспроизводимого рандома (None = случайный каждый раз)
    pub random_seed: Option<u64>,
    
    /// Задержка на исполнение ордера (мс)
    pub execution_delay_ms_range: (u64, u64),
    
    /// Задержка на перестановку ордера (мс) - для Sell ордеров
    pub reposition_delay_ms_range: (u64, u64),
    
    /// Дискретность пересчета - стратегии пересчитываются не каждый тик
    pub recalculation_interval_ms: u64,
    
    /// Вероятность "пропуска" трейда (0.0 - никогда, 1.0 - всегда)
    pub missed_trade_probability: f64,
    
    /// Режим исполнения
    pub mode: ExecutionMode,
    
    /// Защита от реальных ордеров в режиме эмулятора
    pub enforce_emulator_mode: bool,
}

impl Default for BacktestSettings {
    fn default() -> Self {
        BacktestSettings {
            tick_interval_ms: 2, // 2 мс как в MoonBot (1 тик = 2 сек)
            latency_ms_range: (10, 20), // Случайная задержка 10-20 мс
            slippage_satoshi: 0,
            random_seed: None,
            execution_delay_ms_range: (10, 20),
            reposition_delay_ms_range: (10, 20),
            recalculation_interval_ms: 50, // Пересчет раз в 50 мс
            missed_trade_probability: 0.0,
            mode: ExecutionMode::Emulator,
            enforce_emulator_mode: true,
        }
    }
}

pub struct BacktestEngine {
    settings: BacktestSettings,
    #[cfg(feature = "rand")]
    rng: StdRng,
    
    /// Потоки данных по инструментам
    streams: Vec<TradeStream>,
    
    /// Состояние рынка
    market_state: MarketState,
    
    /// Эмулятор рынка
    emulator: MarketEmulator,
    
    /// Текущее время симуляции
    current_time: DateTime<Utc>,
    
    /// Время последнего пересчета стратегий
    last_recalculation_time: DateTime<Utc>,
    
    /// Метрики бэктеста
    metrics: BacktestMetrics,
    
    /// Очередь событий с задержками
    event_queue: VecDeque<DelayedEvent>,
    
    /// Флаг остановки
    stopped: bool,

    /// Подключенные стратегии (адаптеры)
    #[cfg(feature = "gate_exec")]
    strategies: Vec<Box<dyn StrategyAdapter + Send>>,
    
    /// Калькулятор дельт для стратегий
    delta_calculator: DeltaCalculator, 
}

#[derive(Debug, Clone)]
enum DelayedEvent {
    OrderExecution {
        order_id: u64,
        execute_at: DateTime<Utc>,
    },
    OrderReposition {
        order_id: u64,
        new_price: f64,
        execute_at: DateTime<Utc>,
    },
    StrategyRecalculation {
        execute_at: DateTime<Utc>,
    },
}

impl BacktestEngine {
    pub fn new(settings: BacktestSettings) -> Self {
        #[cfg(feature = "rand")]
        let seed = settings.random_seed.unwrap_or_else(|| {
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos() as u64
        });
        
        #[cfg(feature = "rand")]
        let rng = StdRng::seed_from_u64(seed);
        
        // Защита: в режиме бэктеста принудительно включаем эмулятор
        let mode = if settings.enforce_emulator_mode {
            ExecutionMode::Emulator
        } else {
            settings.mode
        };
        
        let mut final_settings = settings;
        final_settings.mode = mode;
        
        Self {
            settings: final_settings,
            #[cfg(feature = "rand")]
            rng,
            streams: Vec::new(),
            market_state: MarketState::new(),
            emulator: MarketEmulator::new(),
            current_time: Utc::now(),
            last_recalculation_time: Utc::now(),
            metrics: BacktestMetrics::new(),
            event_queue: VecDeque::new(),
            stopped: false,
            #[cfg(feature = "gate_exec")]
            strategies: Vec::new(),
            delta_calculator: DeltaCalculator::new(),
        }
    }
    
    /// Добавить поток данных
    pub fn add_stream(&mut self, stream: TradeStream) {
        self.streams.push(stream);
    }

    /// Добавить стратегию (адаптер)
    #[cfg(feature = "gate_exec")]
    pub fn add_strategy_adapter<A: StrategyAdapter + Send + 'static>(&mut self, adapter: A) {
        self.strategies.push(Box::new(adapter));
    }
    
    /// Запуск бэктеста
    pub fn run(&mut self) -> anyhow::Result<BacktestResult> {
        if self.streams.is_empty() {
            return Err(anyhow::anyhow!("No trade streams loaded"));
        }
        
        // Проверка режима эмулятора
        if self.settings.mode != ExecutionMode::Emulator {
            return Err(anyhow::anyhow!(
                "Backtest must run in Emulator mode! Real trading disabled."
            ));
        }
        
        println!("🚀 Starting backtest with seed: {:?}", self.settings.random_seed);
        println!("📊 Streams: {}", self.streams.len());
        
        // Инициализация времени
        self.current_time = self.get_earliest_timestamp();
        self.last_recalculation_time = self.current_time;
        
        // Основной цикл симуляции
        let mut tick_count = 0;
        while !self.stopped && self.has_more_data() {
            // Получаем следующий тик с учетом случайных задержек
            if let Some(next_tick) = self.get_next_tick_with_lag() {
                // Применяем случайную задержку сети
                #[cfg(feature = "rand")]
                let network_lag_ms = {
                    use rand::Rng;
                    self.rng.gen_range(self.settings.latency_ms_range.0..=self.settings.latency_ms_range.1)
                };
                #[cfg(not(feature = "rand"))]
                let network_lag_ms = self.settings.latency_ms_range.0;
                let adjusted_time = self.current_time + Duration::milliseconds(network_lag_ms as i64);
                
                // Обновляем время симуляции
                self.current_time = next_tick.timestamp;
                
                // Проверяем, не пропустили ли мы этот трейд (случайность)
                if self.should_miss_trade() {
                    continue; // Пропускаем этот трейд
                }
                
                // Обрабатываем задержанные события из очереди
                self.process_delayed_events(adjusted_time);
                
                // Дискретный пересчет стратегий (не каждый тик!)
                let time_since_recalc = (adjusted_time - self.last_recalculation_time)
                    .num_milliseconds() as u64;
                
                if time_since_recalc >= self.settings.recalculation_interval_ms {
                    self.recalculate_strategies(&next_tick, adjusted_time);
                    self.last_recalculation_time = adjusted_time;
                }
                
                // Обновляем состояние рынка
                self.market_state.update_from_tick(&next_tick);
                
                // Обновляем калькулятор дельт
                self.delta_calculator.update(&next_tick, adjusted_time);
                
                // Эмулируем исполнение ордеров
                #[cfg(feature = "rand")]
                {
                    use rand::Rng;
                    self.emulator.process_tick(&next_tick, &mut self.metrics, &mut self.rng);
                }
                #[cfg(not(feature = "rand"))]
                {
                    // Без рандома просто обрабатываем тик
                    // В реальной реализации здесь будет другой способ передачи RNG
                }
                
                tick_count += 1;
                
                // Прогресс каждые 10000 тиков
                if tick_count % 10000 == 0 {
                    println!("⏳ Progress: {} ticks processed, P&L: {:.2}", 
                        tick_count, self.metrics.total_pnl);
                }
            } else {
                break;
            }
        }
        
        println!("✅ Backtest completed: {} ticks", tick_count);
        
        Ok(self.metrics.to_result())
    }
    
    fn get_earliest_timestamp(&self) -> DateTime<Utc> {
        self.streams
            .iter()
            .filter_map(|s| s.trades.first().map(|t| t.timestamp))
            .min()
            .unwrap_or_else(Utc::now)
    }
    
    fn has_more_data(&self) -> bool {
        self.streams.iter().any(|s| s.has_more())
    }
    
    fn get_next_tick_with_lag(&mut self) -> Option<super::market::TradeTick> {
        // Находим самый ранний тик из всех потоков
        let mut earliest: Option<(usize, usize)> = None;
        let mut earliest_time = None;
        
        for (stream_idx, stream) in self.streams.iter().enumerate() {
            if let Some(trade_idx) = stream.current_index {
                if trade_idx < stream.trades.len() {
                    let tick = &stream.trades[trade_idx];
                    if earliest_time.is_none() || tick.timestamp < earliest_time.unwrap() {
                        earliest = Some((stream_idx, trade_idx));
                        earliest_time = Some(tick.timestamp);
                    }
                }
            }
        }
        
        if let Some((stream_idx, trade_idx)) = earliest {
            // Увеличиваем индекс для этого потока
            let stream = &mut self.streams[stream_idx];
            stream.current_index = Some(trade_idx + 1);
            
            return Some(stream.trades[trade_idx].clone());
        }
        
        None
    }
    
    fn should_miss_trade(&mut self) -> bool {
        if self.settings.missed_trade_probability <= 0.0 {
            return false;
        }
        
        #[cfg(feature = "rand")]
        {
            use rand::Rng;
            self.rng.gen_range(0.0f64..1.0f64) < self.settings.missed_trade_probability
        }
        #[cfg(not(feature = "rand"))]
        {
            false
        }
    }
    
    fn process_delayed_events(&mut self, current_time: DateTime<Utc>) {
        // Обрабатываем события, время которых пришло
        while let Some(event) = self.event_queue.front() {
            let execute_at = match event {
                DelayedEvent::OrderExecution { execute_at, .. } => *execute_at,
                DelayedEvent::OrderReposition { execute_at, .. } => *execute_at,
                DelayedEvent::StrategyRecalculation { execute_at } => *execute_at,
            };
            
            if execute_at > current_time {
                break; // Еще не время
            }
            
            let event = self.event_queue.pop_front().unwrap();
            
            match event {
                DelayedEvent::OrderExecution { order_id, .. } => {
                    // Исполняем ордер с задержкой
                    // Note: execute_order требует изменяемого заимствования metrics
                    // Это временное решение - в реальной реализации нужна другая архитектура
                }
                DelayedEvent::OrderReposition { order_id, new_price, .. } => {
                    // Переставляем ордер с задержкой
                    self.emulator.reposition_order(order_id, new_price, current_time);
                }
                DelayedEvent::StrategyRecalculation { .. } => {
                    // Пересчет стратегий
                }
            }
        }
    }
    
    fn recalculate_strategies(
        &mut self,
        tick: &super::market::TradeTick,
        adjusted_time: DateTime<Utc>,
    ) {
        // Вызываем стратегии через адаптеры (если подключены)
        #[cfg(feature = "gate_exec")]
        {
            // Вычисляем реальные дельты из истории
            let deltas = self.delta_calculator.calculate_deltas(tick.price, adjusted_time);
            for adapter in &mut self.strategies {
                match adapter.on_tick(tick, &deltas) {
                    StrategyAction::NoAction => {}
                    StrategyAction::PlaceBuy { price, size } => {
                        let _id = self.emulator.place_limit_order(&tick.symbol, price, size, true, adjusted_time);
                        // Можно сохранить id при необходимости
                    }
                    StrategyAction::PlaceSell { price, size } => {
                        let _id = self.emulator.place_limit_order(&tick.symbol, price, size, false, adjusted_time);
                    }
                    StrategyAction::ReplaceBuy { new_price } => {
                        // Переставление: выберем любой активный ордер по символу (упрощенно)
                        if let Some((&order_id, _)) = self.emulator.get_active_orders().iter().find(|(_, o)| o.symbol == tick.symbol) {
                            self.emulator.reposition_order(order_id, new_price, adjusted_time);
                        }
                    }
                    StrategyAction::CancelOrder { order_id } => {
                        let _ = self.emulator.cancel_order(order_id);
                    }
                    StrategyAction::DetectSignal { .. } => {}
                }
            }
        }
        
        // Эмулируем случайную задержку на перестановку Sell ордеров
        #[cfg(feature = "rand")]
        {
            use rand::Rng;
            if self.rng.gen_bool(0.1) { // 10% вероятность перестановки
                let delay_ms = self.rng.gen_range(
                    self.settings.reposition_delay_ms_range.0..=self.settings.reposition_delay_ms_range.1
                );
                
                self.event_queue.push_back(DelayedEvent::StrategyRecalculation {
                    execute_at: adjusted_time + Duration::milliseconds(delay_ms as i64),
                });
            }
        }
    }
    
    /// Остановка бэктеста
    pub fn stop(&mut self) {
        self.stopped = true;
    }
    
    /// Запуск Монте-Карло симуляции (многократные прогоны)
    pub fn run_monte_carlo(
        &mut self,
        num_runs: usize,
    ) -> anyhow::Result<Vec<BacktestResult>> {
        let mut results = Vec::new();
        
        println!("🎲 Starting Monte Carlo simulation: {} runs", num_runs);
        
        for run in 0..num_runs {
            println!("📊 Run {}/{}", run + 1, num_runs);
            
            // Новый seed для каждого прогона
            let seed = self.settings.random_seed.map(|s| s + run as u64);
            let mut run_settings = self.settings.clone();
            run_settings.random_seed = seed;
            
            // Создаем новый движок для этого прогона
            let mut engine = BacktestEngine::new(run_settings);
            
            // Копируем потоки данных
            for stream in &self.streams {
                engine.add_stream(stream.clone());
            }
            
            // Запускаем прогон
            match engine.run() {
                Ok(result) => {
                    let pnl = result.total_pnl;
                    let trades = result.total_trades;
                    results.push(result);
                    println!("  ✅ Run {}: P&L={:.2}, Trades={}", run + 1, pnl, trades);
                }
                Err(e) => {
                    eprintln!("  ❌ Run {} failed: {}", run + 1, e);
                }
            }
        }
        
        println!("🎯 Monte Carlo completed: {} successful runs", results.len());
        
        Ok(results)
    }
}

