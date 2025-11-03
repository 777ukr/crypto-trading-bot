//! Investor Portal - Полнофункциональный веб-портал для инвестора
//! - Выбор стратегий
//! - Выбор плеча (3x, 5x, 10x, 21x, 40x, 50x, 80x, 100x, 125x)
//! - Запуск бэктестов с WebSocket стримингом прогресса
//! - Визуализация результатов (equity curve, таблица сделок)
//! - Сохранение в PostgreSQL
//! - Выбор лучших стратегий для live торговли

#![cfg(feature = "dashboard")]

use axum::{
    extract::{State, Query, ws::{WebSocket, WebSocketUpgrade}},
    http::StatusCode,
    response::{Html, Json, Response},
    routing::{get, post},
    Router,
};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::{Mutex, broadcast};
use chrono::{Utc, Duration};

#[cfg(feature = "database")]
use rust_test::database::{DatabaseRepository, BacktestResult as DbBacktestResult};
#[cfg(feature = "database")]
use rust_test::backtest::{BacktestEngine, BacktestSettings, ExecutionMode, TradeStream};
#[cfg(feature = "database")]
use rust_test::backtest::market::{TradeTick, TradeSide};
#[cfg(feature = "database")]
use rust_test::backtest::replay::ReplayEngine;
#[cfg(feature = "database")]
use rust_test::backtest::metrics::BacktestResult;
#[cfg(feature = "database")]
use rust_test::backtest::strategy_adapter::{MShotAdapter, MStrikeAdapter, HookAdapter};
#[cfg(feature = "database")]
use rust_test::strategy::moon_strategies::{mshot::MShotConfig, mstrike::MStrikeConfig, hook::HookConfig};

#[derive(Debug, Clone, Serialize, Deserialize)]
struct BacktestRequest {
    strategies: Vec<String>,
    symbols: Vec<String>,
    leverage: f64,
    initial_balance: f64,
    use_rebate: bool,
}

#[derive(Debug, Serialize)]
struct BacktestResponse {
    success: bool,
    message: String,
    backtest_id: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(tag = "type")]
enum ProgressMessage {
    #[serde(rename = "progress")]
    Progress {
        backtest_id: String,
        progress: f64,
        current_tick: usize,
        total_ticks: usize,
        current_pnl: f64,
        trades: usize,
    },
    #[serde(rename = "complete")]
    Complete {
        backtest_id: String,
        result: StrategyResult,
    },
    #[serde(rename = "error")]
    Error {
        backtest_id: String,
        error: String,
    },
}

#[derive(Debug, Clone, Serialize)]
struct StrategyRating {
    profitability_score: f64,
    stability_score: f64,
    risk_score: f64,
    fill_rate_score: f64,
    overall_rating: f64,
    stars: u32,
}

#[derive(Debug, Clone, Serialize)]
struct TradeRecord {
    timestamp: i64,
    entry_price: f64,
    exit_price: f64,
    side: String,
    pnl: f64,
    pnl_percent: f64,
    size: f64,
}

#[derive(Debug, Clone, Serialize)]
struct EquityPoint {
    timestamp: i64,
    equity: f64,
    pnl: f64,
}

#[derive(Debug, Clone, Serialize)]
struct StrategyResult {
    strategy_name: String,
    symbol: String,
    initial_balance: f64,
    final_balance: f64,
    total_pnl: f64,
    total_fees: f64,
    fees_after_rebate: f64,
    trades: usize,
    wins: usize,
    losses: usize,
    win_rate: f64,
    roi: f64,
    profit_factor: f64,
    max_drawdown: f64,
    leverage: f64,
    profitable: bool,
    rating: Option<StrategyRating>,
    trades_list: Vec<TradeRecord>,
    equity_curve: Vec<EquityPoint>,
}

#[derive(Debug, Clone)]
enum BacktestJob {
    Pending,
    Running { progress_sender: broadcast::Sender<ProgressMessage> },
    Completed { result: StrategyResult },
    Failed { error: String },
}

#[derive(Clone)]
struct AppState {
    results: Arc<Mutex<Vec<StrategyResult>>>,
    jobs: Arc<Mutex<HashMap<String, BacktestJob>>>,
    #[cfg(feature = "database")]
    db_repo: Option<Arc<DatabaseRepository>>,
}

#[tokio::main]
async fn main() {
    println!("🚀 Investor Portal запущен!");
    println!("📊 Откройте в браузере: http://localhost:8080");

    #[cfg(feature = "database")]
    let db_repo = if let Ok(database_url) = std::env::var("DATABASE_URL") {
        match DatabaseRepository::create_pool(&database_url).await {
            Ok(pool) => {
                println!("✅ Подключено к PostgreSQL");
                Some(Arc::new(DatabaseRepository::new(pool)))
            }
            Err(e) => {
                eprintln!("⚠️  Не удалось подключиться к БД: {}", e);
                eprintln!("   Продолжаем без БД (результаты только в памяти)");
                None
            }
        }
    } else {
        println!("⚠️  DATABASE_URL не установлен, работаем без БД");
        None
    };

    let state = AppState {
        results: Arc::new(Mutex::new(Vec::new())),
        jobs: Arc::new(Mutex::new(HashMap::new())),
        #[cfg(feature = "database")]
        db_repo,
    };

    let app = Router::new()
        .route("/", get(index))
        .route("/api/strategies", get(get_available_strategies))
        .route("/api/leverages", get(get_available_leverages))
        .route("/api/symbols", get(get_available_symbols))
        .route("/api/backtest", post(run_backtest))
        .route("/api/backtest/:id/stream", get(stream_backtest_progress))
        .route("/api/results", get(get_results))
        .route("/api/results/latest", get(get_latest_results))
        .route("/api/trades/:backtest_id", get(get_trades))
        .route("/api/equity/:backtest_id", get(get_equity_curve))
        .with_state(state);

    let listener = tokio::net::TcpListener::bind("0.0.0.0:8080").await.unwrap();
    println!("🌐 Server listening on http://0.0.0.0:8080");
    axum::serve(listener, app).await.unwrap();
}

async fn index() -> Html<&'static str> {
    Html(include_str!("../../templates/investor_portal.html"))
}

async fn get_available_strategies() -> Json<Vec<HashMap<&'static str, &'static str>>> {
    let mut strategies = vec![
        {
            let mut s = HashMap::new();
            s.insert("id", "channel_split");
            s.insert("name", "Channel Split");
            s.insert("description", "Канальная стратегия с дроблением ордеров");
            s.insert("type", "long");
            s
        },
        {
            let mut s = HashMap::new();
            s.insert("id", "market_making");
            s.insert("name", "Market Making");
            s.insert("description", "Маркет-мейкинг с контролем спреда");
            s.insert("type", "both");
            s
        },
        {
            let mut s = HashMap::new();
            s.insert("id", "hft");
            s.insert("name", "High-Frequency Trading");
            s.insert("description", "Высокочастотная торговля");
            s.insert("type", "both");
            s
        },
        {
            let mut s = HashMap::new();
            s.insert("id", "long_trailing");
            s.insert("name", "Long Trailing Stop");
            s.insert("description", "Лонговая позиция с трейлинг стопом");
            s.insert("type", "long");
            s
        },
        {
            let mut s = HashMap::new();
            s.insert("id", "short_trailing");
            s.insert("name", "Short Trailing Stop");
            s.insert("description", "Шортовая позиция с трейлинг стопом");
            s.insert("type", "short");
            s
        },
    ];
    
    // Добавляем Moon стратегии
    #[cfg(feature = "gate_exec")]
    {
        strategies.push({
            let mut s = HashMap::new();
            s.insert("id", "mshot");
            s.insert("name", "MShot");
            s.insert("description", "MoonShot - ловля прострелов с переставлением ордеров");
            s.insert("type", "long");
            s
        });
        strategies.push({
            let mut s = HashMap::new();
            s.insert("id", "mstrike");
            s.insert("name", "MStrike");
            s.insert("description", "MoonStrike - детект прострела через LastBidEMA");
            s.insert("type", "long");
            s
        });
        strategies.push({
            let mut s = HashMap::new();
            s.insert("id", "hook");
            s.insert("name", "Hook");
            s.insert("description", "Hook - динамический коридор с интерполяцией");
            s.insert("type", "long");
            s
        });
    }
    
    Json(strategies)
}

async fn get_available_leverages() -> Json<Vec<f64>> {
    Json(vec![3.0, 5.0, 10.0, 21.0, 40.0, 50.0, 80.0, 100.0, 125.0])
}

async fn get_available_symbols() -> Json<Vec<&'static str>> {
    Json(vec!["SOL_USDT", "ETH_USDT", "BTC_USDT"])
}

async fn run_backtest(
    State(state): State<AppState>,
    Json(request): Json<BacktestRequest>,
) -> Result<Json<BacktestResponse>, StatusCode> {
    use std::time::{SystemTime, UNIX_EPOCH};
    let backtest_id = format!("bt_{}", 
        SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs());
    
    println!("📊 Запуск бэктеста {}: стратегии={:?}, символы={:?}, плечо={}x", 
             backtest_id, request.strategies, request.symbols, request.leverage);

    // Создаем канал для прогресса
    let (tx, _rx) = broadcast::channel::<ProgressMessage>(100);
    
    // Добавляем задачу в очередь
    {
        let mut jobs = state.jobs.lock().await;
        jobs.insert(backtest_id.clone(), BacktestJob::Running { 
            progress_sender: tx.clone() 
        });
    }

    // Запускаем фоновую задачу
    let state_clone = state.clone();
    let backtest_id_clone = backtest_id.clone();
    tokio::spawn(async move {
        run_backtest_task(state_clone, backtest_id_clone, request, tx).await;
    });

    Ok(Json(BacktestResponse {
        success: true,
        message: "Бэктест запущен".to_string(),
        backtest_id,
    }))
}

#[cfg(feature = "database")]
async fn run_backtest_task(
    state: AppState,
    backtest_id: String,
    request: BacktestRequest,
    progress_tx: broadcast::Sender<ProgressMessage>,
) {
    let mut results = Vec::new();
    
    for strategy_name in &request.strategies {
        for symbol in &request.symbols {
            // Загружаем исторические данные
            match load_trade_data(symbol).await {
                Ok(streams) => {
                    // Создаем движок бэктеста
                    let settings = BacktestSettings {
                        tick_interval_ms: 2,
                        latency_ms_range: (10, 20),
                        execution_delay_ms_range: (10, 20),
                        reposition_delay_ms_range: (10, 20),
                        recalculation_interval_ms: 50,
                        missed_trade_probability: 0.0,
                        mode: ExecutionMode::Emulator,
                        enforce_emulator_mode: true,
                        slippage_satoshi: 0,
                        random_seed: None,
                    };
                    
                    let mut engine = BacktestEngine::new(settings);
                    
                    // Добавляем потоки данных
                    for stream in streams {
                        engine.add_stream(stream);
                    }
                    
                    // Добавляем стратегии
                    match strategy_name.as_str() {
                        "mshot" => {
                            let config = MShotConfig::default();
                            engine.add_strategy_adapter(MShotAdapter::new(config));
                        }
                        "mstrike" => {
                            let config = MStrikeConfig::default();
                            engine.add_strategy_adapter(MStrikeAdapter::new(config));
                        }
                        "hook" => {
                            let config = HookConfig::default();
                            engine.add_strategy_adapter(HookAdapter::new(config));
                        }
                        _ => {
                            // Другие стратегии пока не интегрированы
                            eprintln!("⚠️  Стратегия {} пока не поддерживается", strategy_name);
                            continue;
                        }
                    }
                    
                    // Запускаем бэктест
                    match engine.run() {
                        Ok(backtest_result) => {
                            // Конвертируем результат
                            let result = convert_to_strategy_result(
                                strategy_name.clone(),
                                symbol.clone(),
                                &backtest_result,
                                request.initial_balance,
                                request.leverage,
                                request.use_rebate,
                            );
                            
                            // Отправляем прогресс о завершении
                            let complete_msg = ProgressMessage::Complete {
                                backtest_id: backtest_id.clone(),
                                result: result.clone(),
                            };
                            let _ = progress_tx.send(complete_msg);
                            
                            // Сохраняем в БД если доступно
                            if let Some(ref repo) = state.db_repo {
                                let db_result = convert_to_db_result(&result, &backtest_result);
                                if let Err(e) = repo.insert_backtest_result(&db_result).await {
                                    eprintln!("⚠️  Ошибка сохранения в БД: {}", e);
                                }
                            }
                            
                            results.push(result);
                        }
                        Err(e) => {
                            let _ = progress_tx.send(ProgressMessage::Error {
                                backtest_id: backtest_id.clone(),
                                error: format!("Ошибка бэктеста: {}", e),
                            });
                        }
                    }
                }
                Err(e) => {
                    let _ = progress_tx.send(ProgressMessage::Error {
                        backtest_id: backtest_id.clone(),
                        error: format!("Ошибка загрузки данных: {}", e),
                    });
                }
            }
        }
    }
    
    // Сохраняем результаты
    {
        let mut stored = state.results.lock().await;
        stored.extend(results);
    }
    
    // Обновляем статус задачи
    {
        let mut jobs = state.jobs.lock().await;
        if let Some(BacktestJob::Running { .. }) = jobs.get(&backtest_id) {
            // Статус уже обновлен через Complete сообщение
        }
    }
}

#[cfg(not(feature = "database"))]
async fn run_backtest_task(
    state: AppState,
    backtest_id: String,
    request: BacktestRequest,
    progress_tx: broadcast::Sender<ProgressMessage>,
) {
    // Без database фичи - возвращаем заглушку
    let _ = progress_tx.send(ProgressMessage::Error {
        backtest_id,
        error: "Database feature not enabled".to_string(),
    });
}

#[cfg(feature = "database")]
async fn load_trade_data(symbol: &str) -> anyhow::Result<Vec<TradeStream>> {
    // Пытаемся загрузить из БД
    if let Ok(database_url) = std::env::var("DATABASE_URL") {
        if let Ok(pool) = DatabaseRepository::create_pool(&database_url).await {
            let repo = DatabaseRepository::new(pool);
            let end_time = Utc::now();
            let start_time = end_time - Duration::days(180);
            
            let ticks = repo.query_ticks(&rust_test::database::TickQuery {
                symbol: symbol.to_string(),
                start_time: Some(start_time),
                end_time: Some(end_time),
                limit: Some(1_000_000),
                exchange: None,
            }).await?;
            
            if !ticks.is_empty() {
                let trade_ticks: Vec<TradeTick> = ticks.into_iter().map(|t| TradeTick {
                    timestamp: t.timestamp,
                    symbol: t.symbol,
                    price: f64::try_from(t.price).unwrap_or(0.0),
                    volume: f64::try_from(t.quantity).unwrap_or(0.0),
                    side: if t.side == "BUY" { TradeSide::Buy } else { TradeSide::Sell },
                    trade_id: t.trade_id,
                    best_bid: None,
                    best_ask: None,
                }).collect();
                
                return Ok(vec![TradeStream::new(symbol.to_string(), trade_ticks)]);
            }
        }
    }
    
    // Пытаемся загрузить из .bin файла
    let bin_path = format!("data/{}_trades.bin", symbol.replace("_", "").to_lowercase());
    if std::path::Path::new(&bin_path).exists() {
        let mut replay = ReplayEngine::new(rust_test::backtest::replay::ReplaySettings {
            speed_multiplier: 1.0,
            start_time: Some(Utc::now() - Duration::days(180)),
            end_time: Some(Utc::now()),
        });
        
        if replay.load_bin_file(&bin_path).is_ok() {
            return Ok(replay.take_streams());
        }
    }
    
    // Если данных нет - генерируем синтетические для демо
    anyhow::bail!("Нет исторических данных для {}", symbol);
}

#[cfg(feature = "database")]
fn convert_to_strategy_result(
    strategy_name: String,
    symbol: String,
    backtest_result: &BacktestResult,
    initial_balance: f64,
    leverage: f64,
    use_rebate: bool,
) -> StrategyResult {
    let final_balance = initial_balance + backtest_result.total_pnl;
    
    // Вычисляем ROI и fees (их нет в BacktestResult напрямую)
    let roi = (backtest_result.total_pnl / initial_balance) * 100.0;
    let estimated_fees = initial_balance * 0.0005 * backtest_result.total_trades as f64; // 0.05% на сделку
    let fees_after_rebate = if use_rebate {
        estimated_fees * 0.4 // 60% возврат
    } else {
        estimated_fees
    };
    
    // Рассчитываем рейтинг
    let rating = calculate_rating(backtest_result);
    
    StrategyResult {
        strategy_name,
        symbol,
        initial_balance,
        final_balance,
        total_pnl: backtest_result.total_pnl,
        total_fees: estimated_fees,
        fees_after_rebate,
        trades: backtest_result.total_trades,
        wins: backtest_result.winning_trades,
        losses: backtest_result.losing_trades,
        win_rate: backtest_result.win_rate,
        roi,
        profit_factor: backtest_result.profit_factor,
        max_drawdown: backtest_result.max_drawdown,
        leverage,
        profitable: roi > 0.0,
        rating: Some(rating),
        trades_list: vec![], // TODO: извлечь из backtest_result
        equity_curve: vec![], // TODO: извлечь из backtest_result
    }
}

#[cfg(feature = "database")]
fn calculate_rating(result: &BacktestResult) -> StrategyRating {
    // Используем рейтинг из BacktestResult напрямую
    let rating = &result.rating;
    
    StrategyRating {
        profitability_score: rating.profitability_score,
        stability_score: rating.stability_score,
        risk_score: rating.risk_score,
        fill_rate_score: rating.fill_rate_score,
        overall_rating: rating.overall_rating,
        stars: rating.stars as u32,
    }
}

#[cfg(feature = "database")]
fn convert_to_db_result(
    result: &StrategyResult,
    backtest_result: &BacktestResult,
) -> DbBacktestResult {
    use rust_decimal::Decimal;
    use rust_test::database::types::BacktestResult;
    
    BacktestResult {
        strategy_name: result.strategy_name.clone(),
        symbol: result.symbol.clone(),
        initial_balance: Decimal::try_from(result.initial_balance).unwrap_or_default(),
        leverage: result.leverage as i32,
        final_balance: Decimal::try_from(result.final_balance).unwrap_or_default(),
        total_pnl: Decimal::try_from(result.total_pnl).unwrap_or_default(),
        total_fees: Decimal::try_from(result.total_fees).unwrap_or_default(),
        total_trades: result.trades as i32,
        winning_trades: result.wins as i32,
        losing_trades: result.losses as i32,
        win_rate: Decimal::try_from(result.win_rate).unwrap_or_default(),
        roi: Decimal::try_from(result.roi).unwrap_or_default(),
        profit_factor: backtest_result.profit_factor.map(|pf| Decimal::try_from(pf).unwrap_or_default()),
        max_drawdown: backtest_result.max_drawdown.map(|dd| Decimal::try_from(dd).unwrap_or_default()),
        sharpe_ratio: backtest_result.sharpe_ratio.map(|sr| Decimal::try_from(sr).unwrap_or_default()),
        start_time: Some(Utc::now() - Duration::days(180)),
        end_time: Some(Utc::now()),
        config: None,
        notes: None,
    }
}

async fn stream_backtest_progress(
    ws: WebSocketUpgrade,
    State(state): State<AppState>,
    axum::extract::Path(backtest_id): axum::extract::Path<String>,
) -> Response {
    ws.on_upgrade(move |socket| handle_websocket(socket, state, backtest_id))
}

async fn handle_websocket(socket: WebSocket, state: AppState, backtest_id: String) {
    use futures_util::{SinkExt, StreamExt};
    let (mut sender, mut _receiver) = socket.split();
    let mut rx = {
        let jobs = state.jobs.lock().await;
        if let Some(BacktestJob::Running { progress_sender }) = jobs.get(&backtest_id) {
            progress_sender.subscribe()
        } else {
            return; // Задача не найдена
        }
    };
    
    // Отправляем сообщения прогресса
    while let Ok(msg) = rx.recv().await {
        let json = serde_json::to_string(&msg).unwrap_or_default();
        if sender.send(axum::extract::ws::Message::Text(json)).await.is_err() {
            break;
        }
    }
}

async fn get_results(
    State(state): State<AppState>,
    Query(params): Query<HashMap<String, String>>,
) -> Json<Vec<StrategyResult>> {
    let results = state.results.lock().await;
    let mut filtered: Vec<StrategyResult> = results.clone();
    
    // Фильтр только прибыльных
    if params.get("only_profitable") == Some(&"true".to_string()) {
        filtered.retain(|r| r.profitable);
    }
    
    // Сортировка
    if let Some(sort_by) = params.get("sort_by") {
        match sort_by.as_str() {
            "roi" => filtered.sort_by(|a, b| b.roi.partial_cmp(&a.roi).unwrap_or(std::cmp::Ordering::Equal)),
            "profit_factor" => filtered.sort_by(|a, b| b.profit_factor.partial_cmp(&a.profit_factor).unwrap_or(std::cmp::Ordering::Equal)),
            _ => {}
        }
    }
    
    Json(filtered)
}

async fn get_latest_results(State(state): State<AppState>) -> Json<Vec<StrategyResult>> {
    let results = state.results.lock().await;
    let latest: Vec<StrategyResult> = results
        .iter()
               .filter(|r| r.profitable)
        .cloned()
        .collect();
    Json(latest)
}

async fn get_trades(
    State(state): State<AppState>,
    axum::extract::Path(backtest_id): axum::extract::Path<String>,
) -> Json<Vec<TradeRecord>> {
    // TODO: Извлечь trades из результатов
    Json(vec![])
}

async fn get_equity_curve(
    State(state): State<AppState>,
    axum::extract::Path(backtest_id): axum::extract::Path<String>,
) -> Json<Vec<EquityPoint>> {
    // TODO: Извлечь equity curve из результатов
    Json(vec![])
}
