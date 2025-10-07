// main.rs - Entry point for Rust arbitrage scanner
use std::sync::Arc;
use tokio::{signal, time::Duration};
use tracing::{info, error, Level};
use tracing_subscriber;

mod exchange;
mod arbitrage;
mod networking;
mod alert;

use exchange::ExchangeManager;
use arbitrage::{ArbitrageEngine, Config};
use alert::AlertSystem;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize logging
    tracing_subscriber::fmt()
        .with_max_level(Level::INFO)
        .with_target(false)
        .with_thread_ids(true)
        .init();

    info!("Starting Rust Arbitrage Scanner...");

    // Load configuration
    let config = load_config().await?;
    
    // Initialize core components
    let arbitrage_engine = Arc::new(ArbitrageEngine::new(config.clone()));
    let alert_system = Arc::new(AlertSystem::new(config.clone()));
    let exchange_manager = ExchangeManager::new(config.clone(), arbitrage_engine.clone());

    // Setup opportunity alerting
    let alert_system_clone = alert_system.clone();
    arbitrage_engine.register_callback(Box::new(move |opportunity| {
        let alert_system = alert_system_clone.clone();
        let opp = opportunity.clone();
        tokio::spawn(async move {
            if let Err(e) = alert_system.send_alert(opp).await {
                error!("Failed to send alert: {}", e);
            }
        });
    }));

    // Start all systems
    info!("Starting exchange connections...");
    exchange_manager.start().await?;

    info!("Starting arbitrage engine...");
    arbitrage_engine.start().await;

    info!("Starting alert system...");
    alert_system.start().await?;

    // Start web dashboard
    let dashboard_handle = tokio::spawn(start_web_dashboard(
        config.dashboard_port, 
        arbitrage_engine.clone()
    ));

    info!("Arbitrage scanner running on all cores...");
    info!("Web dashboard available at http://localhost:{}", config.dashboard_port);
    info!("Press Ctrl+C to shutdown");

    // Wait for shutdown signal
    tokio::select! {
        _ = signal::ctrl_c() => {
            info!("Received shutdown signal");
        }
        result = dashboard_handle => {
            error!("Web dashboard crashed: {:?}", result);
        }
    }

    // Graceful shutdown
    info!("Shutting down...");
    exchange_manager.stop().await;
    arbitrage_engine.stop().await;
    alert_system.stop().await;
    
    info!("Shutdown complete");
    Ok(())
}

async fn load_config() -> Result<Config, Box<dyn std::error::Error>> {
    Ok(Config {
        exchanges: vec!["binance", "coinbase", "kraken"]
            .into_iter()
            .map(|s| s.to_string())
            .collect(),
        symbols: vec!["BTC/USDT", "ETH/USDT", "BNB/USDT", "ADA/USDT"]
            .into_iter()
            .map(|s| s.to_string())
            .collect(),
        min_profit_threshold: 0.001, // 0.1%
        max_position_size: 1000.0,
        dashboard_port: 8080,
        websocket_timeout: Duration::from_secs(30),
        reconnect_interval: Duration::from_secs(5),
        max_reconnect_attempts: 10,
        enable_triangle_arbitrage: true,
        enable_cross_exchange: true,
        thread_pool_size: num_cpus::get(),
        enable_thread_pinning: true,
    })
}

async fn start_web_dashboard(
    port: u16,
    engine: Arc<ArbitrageEngine>,
) -> Result<(), Box<dyn std::error::Error>> {
    use warp::Filter;

    // CORS headers
    let cors = warp::cors()
        .allow_any_origin()
        .allow_headers(vec!["content-type"])
        .allow_methods(vec!["GET", "POST", "OPTIONS"]);

    // API routes
    let api = warp::path("api");

    // Get recent opportunities
    let opportunities = api
        .and(warp::path("opportunities"))
        .and(warp::get())
        .and(with_engine(engine.clone()))
        .and_then(get_opportunities);

    // Get performance stats  
    let stats = api
        .and(warp::path("stats"))
        .and(warp::get())
        .and(with_engine(engine.clone()))
        .and_then(get_stats);

    // Serve static files
    let static_files = warp::fs::dir("../web-dashboard/");

    let routes = opportunities
        .or(stats)
        .or(static_files)
        .with(cors);

    info!("Starting web server on port {}", port);
    warp::serve(routes)
        .run(([0, 0, 0, 0], port))
        .await;

    Ok(())
}

fn with_engine(
    engine: Arc<ArbitrageEngine>,
) -> impl Filter<Extract = (Arc<ArbitrageEngine>,), Error = std::convert::Infallible> + Clone {
    warp::any().map(move || engine.clone())
}

async fn get_opportunities(
    engine: Arc<ArbitrageEngine>,
) -> Result<impl warp::Reply, warp::Rejection> {
    let opportunities = engine.get_recent_opportunities(100).await;
    Ok(warp::reply::json(&opportunities))
}

async fn get_stats(
    engine: Arc<ArbitrageEngine>,
) -> Result<impl warp::Reply, warp::Rejection> {
    let stats = engine.get_performance_stats().await;
    Ok(warp::reply::json(&stats))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Instant;

    #[tokio::test]
    async fn test_arbitrage_engine_creation() {
        let config = Config::default();
        let engine = ArbitrageEngine::new(config);
        
        assert!(!engine.is_running().await);
    }

    #[tokio::test]
    async fn test_performance_latency() {
        let config = Config::default();
        let engine = Arc::new(ArbitrageEngine::new(config));
        
        engine.start().await;
        
        // Simulate price update and measure latency
        let start = Instant::now();
        engine.update_price(
            "binance", 
            "BTC/USDT", 
            50000.0, 
            50001.0, 
            100.0
        ).await.unwrap();
        
        let latency = start.elapsed();
        
        // Should process in under 100 microseconds
        assert!(latency.as_micros() < 100);
        
        engine.stop().await;
    }

    #[test]
    fn test_bellman_ford_negative_cycle() {
        // Test the mathematical correctness of arbitrage detection
        // Using a simple 3-currency cycle: USD -> EUR -> GBP -> USD
        
        // Prices: USD/EUR = 0.85, EUR/GBP = 0.90, GBP/USD = 1.35
        // Expected profit: 1 * 0.85 * 0.90 * 1.35 = 1.03275 (3.275% profit)
        
        let graph = vec![
            vec![0.0, -f64::ln(0.85), f64::INFINITY],
            vec![f64::INFINITY, 0.0, -f64::ln(0.90)],
            vec![-f64::ln(1.35), f64::INFINITY, 0.0],
        ];
        
        let cycle = detect_negative_cycle(&graph);
        assert!(cycle.is_some());
        
        if let Some(cycle) = cycle {
            let profit = calculate_cycle_profit(&graph, &cycle);
            assert!(profit > 0.03); // Should be profitable
        }
    }
}

// Test helper functions
fn detect_negative_cycle(graph: &Vec<Vec<f64>>) -> Option<Vec<usize>> {
    // Simplified Bellman-Ford for testing
    let n = graph.len();
    let mut dist = vec![f64::INFINITY; n];
    let mut parent = vec![None; n];
    
    dist[0] = 0.0;
    
    // Relax edges n-1 times
    for _ in 0..n-1 {
        for u in 0..n {
            if dist[u] != f64::INFINITY {
                for v in 0..n {
                    if graph[u][v] != f64::INFINITY {
                        let new_dist = dist[u] + graph[u][v];
                        if new_dist < dist[v] {
                            dist[v] = new_dist;
                            parent[v] = Some(u);
                        }
                    }
                }
            }
        }
    }
    
    // Check for negative cycle
    for u in 0..n {
        if dist[u] != f64::INFINITY {
            for v in 0..n {
                if graph[u][v] != f64::INFINITY {
                    if dist[u] + graph[u][v] < dist[v] {
                        // Found negative cycle, reconstruct it
                        return reconstruct_cycle(parent, v);
                    }
                }
            }
        }
    }
    
    None
}

fn reconstruct_cycle(parent: Vec<Option<usize>>, mut node: usize) -> Option<Vec<usize>> {
    let mut cycle = Vec::new();
    let mut visited = std::collections::HashSet::new();
    
    while !visited.contains(&node) {
        visited.insert(node);
        cycle.push(node);
        
        if let Some(p) = parent[node] {
            node = p;
        } else {
            return None;
        }
    }
    
    // Find cycle start
    let cycle_start = cycle.iter().position(|&x| x == node)?;
    cycle.truncate(cycle.len() - cycle_start);
    
    Some(cycle)
}

fn calculate_cycle_profit(graph: &Vec<Vec<f64>>, cycle: &Vec<usize>) -> f64 {
    let mut total_weight = 0.0;
    
    for i in 0..cycle.len() {
        let u = cycle[i];
        let v = cycle[(i + 1) % cycle.len()];
        total_weight += graph[u][v];
    }
    
    (-total_weight).exp() - 1.0
}