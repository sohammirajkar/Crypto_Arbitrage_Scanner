// arbitrage/engine.rs - Core arbitrage detection engine in Rust
use std::collections::{HashMap, VecDeque};
use std::sync::{Arc, Mutex, RwLock};
use std::time::{Duration, Instant};
use crossbeam::channel::{self, Receiver, Sender, TryRecvError};
use tokio::{task, time};
use tracing::{debug, info, warn, error};
use serde::{Deserialize, Serialize};

use super::types::{ArbitrageOpportunity, MarketTick, PerformanceStats};

pub type OpportunityCallback = Box<dyn Fn(ArbitrageOpportunity) + Send + Sync>;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    pub exchanges: Vec<String>,
    pub symbols: Vec<String>,
    pub min_profit_threshold: f64,
    pub max_position_size: f64,
    pub dashboard_port: u16,
    pub websocket_timeout: Duration,
    pub reconnect_interval: Duration,
    pub max_reconnect_attempts: u32,
    pub enable_triangle_arbitrage: bool,
    pub enable_cross_exchange: bool,
    pub thread_pool_size: usize,
    pub enable_thread_pinning: bool,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            exchanges: vec!["binance".to_string(), "coinbase".to_string()],
            symbols: vec!["BTC/USDT".to_string(), "ETH/USDT".to_string()],
            min_profit_threshold: 0.001,
            max_position_size: 1000.0,
            dashboard_port: 8080,
            websocket_timeout: Duration::from_secs(30),
            reconnect_interval: Duration::from_secs(5),
            max_reconnect_attempts: 10,
            enable_triangle_arbitrage: true,
            enable_cross_exchange: true,
            thread_pool_size: num_cpus::get(),
            enable_thread_pinning: true,
        }
    }
}

pub struct ArbitrageEngine {
    config: Config,
    
    // High-performance data structures
    price_graph: Arc<RwLock<Vec<Vec<f64>>>>,  // Adjacency matrix for currencies
    currency_map: Arc<RwLock<HashMap<String, usize>>>,  // Currency -> index mapping
    
    // Lock-free communication channels
    tick_sender: Sender<MarketTick>,
    tick_receiver: Arc<Mutex<Receiver<MarketTick>>>,
    
    // Opportunity storage and callbacks
    opportunities: Arc<Mutex<VecDeque<ArbitrageOpportunity>>>,
    callbacks: Arc<Mutex<Vec<OpportunityCallback>>>,
    
    // Performance monitoring
    stats: Arc<Mutex<PerformanceStats>>,
    
    // Control
    is_running: Arc<std::sync::atomic::AtomicBool>,
    task_handles: Arc<Mutex<Vec<task::JoinHandle<()>>>>,
}

impl ArbitrageEngine {
    pub fn new(config: Config) -> Self {
        let (tx, rx) = channel::unbounded();
        let max_currencies = 100; // Support up to 100 currencies
        
        Self {
            config,
            price_graph: Arc::new(RwLock::new(vec![vec![f64::INFINITY; max_currencies]; max_currencies])),
            currency_map: Arc::new(RwLock::new(HashMap::new())),
            tick_sender: tx,
            tick_receiver: Arc::new(Mutex::new(rx)),
            opportunities: Arc::new(Mutex::new(VecDeque::new())),
            callbacks: Arc::new(Mutex::new(Vec::new())),
            stats: Arc::new(Mutex::new(PerformanceStats::default())),
            is_running: Arc::new(std::sync::atomic::AtomicBool::new(false)),
            task_handles: Arc::new(Mutex::new(Vec::new())),
        }
    }
    
    pub async fn start(&self) {
        if self.is_running.swap(true, std::sync::atomic::Ordering::SeqCst) {
            return; // Already running
        }
        
        info!("Starting arbitrage engine with {} threads", self.config.thread_pool_size);
        
        // Initialize price graph diagonal
        {
            let mut graph = self.price_graph.write().unwrap();
            for i in 0..graph.len() {
                graph[i][i] = 0.0;
            }
        }
        
        let mut handles = self.task_handles.lock().unwrap();
        
        // Start market data processing task
        handles.push(self.spawn_market_data_processor());
        
        // Start arbitrage detection task
        handles.push(self.spawn_arbitrage_detector());
        
        // Start performance monitoring task
        handles.push(self.spawn_performance_monitor());
        
        info!("Arbitrage engine started successfully");
    }
    
    pub async fn stop(&self) {
        self.is_running.store(false, std::sync::atomic::Ordering::SeqCst);
        
        // Wait for all tasks to complete
        let mut handles = self.task_handles.lock().unwrap();
        for handle in handles.drain(..) {
            if let Err(e) = handle.await {
                error!("Error stopping task: {:?}", e);
            }
        }
        
        info!("Arbitrage engine stopped");
    }
    
    pub async fn is_running(&self) -> bool {
        self.is_running.load(std::sync::atomic::Ordering::SeqCst)
    }
    
    /// Update price for a trading pair - core hot path function
    pub async fn update_price(
        &self,
        exchange: &str,
        symbol: &str,
        bid: f64,
        ask: f64,
        volume: f64,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let start_time = Instant::now();
        
        let tick = MarketTick {
            exchange: exchange.to_string(),
            symbol: symbol.to_string(),
            bid,
            ask,
            last_price: (bid + ask) / 2.0,
            volume,
            timestamp: start_time,
            sequence: self.get_next_sequence(),
        };
        
        // Send to processing thread via lock-free channel
        self.tick_sender.send(tick).map_err(|e| {
            error!("Failed to send market tick: {}", e);
            e
        })?;
        
        // Update performance stats
        let processing_time = start_time.elapsed();
        if let Ok(mut stats) = self.stats.lock() {
            stats.messages_processed += 1;
            stats.update_avg_latency(processing_time.as_micros() as f64);
        }
        
        Ok(())
    }
    
    fn spawn_market_data_processor(&self) -> task::JoinHandle<()> {
        let receiver = Arc::clone(&self.tick_receiver);
        let price_graph = Arc::clone(&self.price_graph);
        let currency_map = Arc::clone(&self.currency_map);
        let is_running = Arc::clone(&self.is_running);
        
        task::spawn(async move {
            info!("Market data processor started");
            
            while is_running.load(std::sync::atomic::Ordering::SeqCst) {
                let tick = match receiver.lock().unwrap().try_recv() {
                    Ok(tick) => tick,
                    Err(TryRecvError::Empty) => {
                        // No data available, brief sleep to prevent busy waiting
                        tokio::time::sleep(Duration::from_micros(100)).await;
                        continue;
                    }
                    Err(TryRecvError::Disconnected) => break,
                };
                
                Self::process_market_tick(tick, &price_graph, &currency_map);
            }
            
            info!("Market data processor stopped");
        })
    }
    
    fn process_market_tick(
        tick: MarketTick,
        price_graph: &Arc<RwLock<Vec<Vec<f64>>>>,
        currency_map: &Arc<RwLock<HashMap<String, usize>>>,
    ) {
        // Parse symbol (e.g., "BTC/USDT" -> ("BTC", "USDT"))
        let (base, quote) = match Self::parse_symbol(&tick.symbol) {
            Some(pair) => pair,
            None => {
                warn!("Invalid symbol format: {}", tick.symbol);
                return;
            }
        };
        
        // Create unique currency identifiers for each exchange
        let base_key = format!("{}_{}", base, tick.exchange);
        let quote_key = format!("{}_{}", quote, tick.exchange);
        
        // Get or create currency indices
        let (base_idx, quote_idx) = {
            let mut map = currency_map.write().unwrap();
            let base_idx = Self::get_or_create_currency_index(&mut map, base_key);
            let quote_idx = Self::get_or_create_currency_index(&mut map, quote_key);
            (base_idx, quote_idx)
        };
        
        // Update price graph with log-transformed prices for Bellman-Ford
        {
            let mut graph = price_graph.write().unwrap();
            if base_idx < graph.len() && quote_idx < graph.len() {
                // Forward edge: base -> quote (selling base for quote)
                if tick.bid > 0.0 {
                    graph[base_idx][quote_idx] = -tick.bid.ln();
                }
                
                // Reverse edge: quote -> base (buying base with quote)
                if tick.ask > 0.0 {
                    graph[quote_idx][base_idx] = -(1.0 / tick.ask).ln();
                }
            }
        }
        
        debug!(
            "Updated price graph: {} -> {} = {:.6}, {} -> {} = {:.6}",
            base, quote, tick.bid, quote, base, 1.0 / tick.ask
        );
    }
    
    fn spawn_arbitrage_detector(&self) -> task::JoinHandle<()> {
        let price_graph = Arc::clone(&self.price_graph);
        let currency_map = Arc::clone(&self.currency_map);
        let opportunities = Arc::clone(&self.opportunities);
        let callbacks = Arc::clone(&self.callbacks);
        let stats = Arc::clone(&self.stats);
        let is_running = Arc::clone(&self.is_running);
        let config = self.config.clone();
        
        task::spawn(async move {
            info!("Arbitrage detector started");
            let mut detection_interval = time::interval(Duration::from_millis(10)); // 100Hz
            
            while is_running.load(std::sync::atomic::Ordering::SeqCst) {
                detection_interval.tick().await;
                
                let start_time = Instant::now();
                
                // Find arbitrage opportunities using Bellman-Ford
                let found_opportunities = Self::detect_arbitrage_opportunities(
                    &price_graph,
                    &currency_map,
                    &config,
                );
                
                let detection_time = start_time.elapsed();
                
                // Process opportunities
                for opp in found_opportunities {
                    if opp.profit_percentage > config.min_profit_threshold {
                        // Store opportunity
                        {
                            let mut opps = opportunities.lock().unwrap();
                            opps.push_back(opp.clone());
                            
                            // Keep only recent opportunities (last 1000)
                            while opps.len() > 1000 {
                                opps.pop_front();
                            }
                        }
                        
                        // Notify callbacks
                        {
                            let callbacks_guard = callbacks.lock().unwrap();
                            for callback in callbacks_guard.iter() {
                                callback(opp.clone());
                            }
                        }
                        
                        // Update stats
                        if let Ok(mut stats) = stats.lock() {
                            stats.opportunities_found += 1;
                        }
                        
                        info!(
                            "Arbitrage opportunity: {} - {:.4}% profit",
                            opp.path, opp.profit_percentage * 100.0
                        );
                    }
                }
                
                // Update detection latency stats
                if let Ok(mut stats) = stats.lock() {
                    stats.detection_latency_us = detection_time.as_micros() as f64;
                }
            }
            
            info!("Arbitrage detector stopped");
        })
    }
    
    fn detect_arbitrage_opportunities(
        price_graph: &Arc<RwLock<Vec<Vec<f64>>>>,
        currency_map: &Arc<RwLock<HashMap<String, usize>>>,
        config: &Config,
    ) -> Vec<ArbitrageOpportunity> {
        let graph = price_graph.read().unwrap();
        let currencies = currency_map.read().unwrap();
        let n = currencies.len().min(graph.len());
        
        if n < 3 {
            return Vec::new(); // Need at least 3 currencies for arbitrage
        }
        
        let mut opportunities = Vec::new();
        
        // Bellman-Ford algorithm to detect negative cycles
        for source in 0..n {
            if let Some(cycle) = Self::bellman_ford_negative_cycle(&graph, source, n) {
                if let Some(opp) = Self::cycle_to_opportunity(cycle, &currencies, &graph) {
                    if opp.profit_percentage > config.min_profit_threshold {
                        opportunities.push(opp);
                    }
                }
            }
        }
        
        opportunities
    }
    
    fn bellman_ford_negative_cycle(
        graph: &[Vec<f64>],
        source: usize,
        n: usize,
    ) -> Option<Vec<usize>> {
        let mut dist = vec![f64::INFINITY; n];
        let mut parent = vec![None; n];
        
        dist[source] = 0.0;
        
        // Relax edges V-1 times
        for _ in 0..n - 1 {
            let mut updated = false;
            for u in 0..n {
                if dist[u] != f64::INFINITY {
                    for v in 0..n {
                        if graph[u][v] != f64::INFINITY {
                            let new_dist = dist[u] + graph[u][v];
                            if new_dist < dist[v] {
                                dist[v] = new_dist;
                                parent[v] = Some(u);
                                updated = true;
                            }
                        }
                    }
                }
            }
            if !updated {
                break; // Early termination
            }
        }
        
        // Check for negative cycles
        for u in 0..n {
            if dist[u] != f64::INFINITY {
                for v in 0..n {
                    if graph[u][v] != f64::INFINITY {
                        if dist[u] + graph[u][v] < dist[v] {
                            // Found negative cycle, extract it
                            return Self::extract_cycle(parent, v);
                        }
                    }
                }
            }
        }
        
        None
    }
    
    fn extract_cycle(parent: Vec<Option<usize>>, mut node: usize) -> Option<Vec<usize>> {
        let mut cycle = Vec::new();
        let mut visited = std::collections::HashSet::new();
        
        // Find the cycle
        while !visited.contains(&node) {
            visited.insert(node);
            cycle.push(node);
            
            node = parent[node]?;
        }
        
        // Find where the cycle actually starts
        let cycle_start_pos = cycle.iter().position(|&x| x == node)?;
        cycle.drain(0..cycle_start_pos);
        
        if cycle.len() >= 3 {
            Some(cycle)
        } else {
            None
        }
    }
    
    fn cycle_to_opportunity(
        cycle: Vec<usize>,
        currencies: &HashMap<String, usize>,
        graph: &[Vec<f64>],
    ) -> Option<ArbitrageOpportunity> {
        if cycle.len() < 3 {
            return None;
        }
        
        // Calculate total profit
        let mut total_log_return = 0.0;
        for i in 0..cycle.len() {
            let u = cycle[i];
            let v = cycle[(i + 1) % cycle.len()];
            total_log_return += graph[u][v];
        }
        
        let profit_multiplier = (-total_log_return).exp();
        let profit_percentage = profit_multiplier - 1.0;
        
        if profit_percentage <= 0.0 {
            return None;
        }
        
        // Build currency path string
        let reverse_map: HashMap<usize, String> = currencies
            .iter()
            .map(|(k, &v)| (v, k.clone()))
            .collect();
        
        let path = cycle
            .iter()
            .filter_map(|&idx| reverse_map.get(&idx))
            .cloned()
            .collect::<Vec<_>>()
            .join(" -> ");
        
        Some(ArbitrageOpportunity {
            path,
            profit_percentage,
            max_volume: 100.0, // Simplified volume estimation
            confidence: Self::calculate_confidence(profit_percentage, cycle.len()),
            detected_at: Instant::now(),
            exchanges: cycle
                .iter()
                .filter_map(|&idx| {
                    reverse_map.get(&idx)?
                        .split('_')
                        .last()
                        .map(|s| s.to_string())
                })
                .collect(),
        })
    }
    
    fn calculate_confidence(profit: f64, path_length: usize) -> u32 {
        // Simple confidence calculation
        let profit_score = (profit * 1000.0).min(50.0);
        let path_score = (50.0 - path_length as f64 * 5.0).max(0.0);
        (profit_score + path_score) as u32
    }
    
    fn spawn_performance_monitor(&self) -> task::JoinHandle<()> {
        let stats = Arc::clone(&self.stats);
        let is_running = Arc::clone(&self.is_running);
        
        task::spawn(async move {
            let mut interval = time::interval(Duration::from_secs(10));
            
            while is_running.load(std::sync::atomic::Ordering::SeqCst) {
                interval.tick().await;
                
                if let Ok(stats) = stats.lock() {
                    info!(
                        "Performance: {} msgs/s, {} opps found, {:.2}μs avg latency",
                        stats.messages_processed / 10,
                        stats.opportunities_found,
                        stats.avg_latency_us
                    );
                }
            }
        })
    }
    
    // Utility methods
    
    fn get_next_sequence(&self) -> u64 {
        static SEQUENCE: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
        SEQUENCE.fetch_add(1, std::sync::atomic::Ordering::SeqCst)
    }
    
    fn parse_symbol(symbol: &str) -> Option<(String, String)> {
        let parts: Vec<&str> = symbol.split('/').collect();
        if parts.len() == 2 {
            Some((parts[0].to_string(), parts[1].to_string()))
        } else {
            None
        }
    }
    
    fn get_or_create_currency_index(
        map: &mut HashMap<String, usize>,
        currency: String,
    ) -> usize {
        if let Some(&index) = map.get(&currency) {
            index
        } else {
            let index = map.len();
            map.insert(currency, index);
            index
        }
    }
    
    // Public API methods
    
    pub fn register_callback(&self, callback: OpportunityCallback) {
        let mut callbacks = self.callbacks.lock().unwrap();
        callbacks.push(callback);
    }
    
    pub async fn get_recent_opportunities(&self, limit: usize) -> Vec<ArbitrageOpportunity> {
        let opportunities = self.opportunities.lock().unwrap();
        let start_idx = if opportunities.len() > limit {
            opportunities.len() - limit
        } else {
            0
        };
        
        opportunities.range(start_idx..).cloned().collect()
    }
    
    pub async fn get_performance_stats(&self) -> PerformanceStats {
        self.stats.lock().unwrap().clone()
    }
}

// Thread-safe Drop implementation
impl Drop for ArbitrageEngine {
    fn drop(&mut self) {
        self.is_running.store(false, std::sync::atomic::Ordering::SeqCst);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[tokio::test]
    async fn test_engine_lifecycle() {
        let config = Config::default();
        let engine = ArbitrageEngine::new(config);
        
        assert!(!engine.is_running().await);
        
        engine.start().await;
        assert!(engine.is_running().await);
        
        engine.stop().await;
        assert!(!engine.is_running().await);
    }
    
    #[tokio::test]
    async fn test_price_update_latency() {
        let config = Config::default();
        let engine = ArbitrageEngine::new(config);
        
        engine.start().await;
        
        let start = Instant::now();
        engine.update_price("binance", "BTC/USDT", 50000.0, 50001.0, 100.0)
            .await
            .unwrap();
        let latency = start.elapsed();
        
        // Should process in under 1ms
        assert!(latency.as_millis() < 1);
        
        engine.stop().await;
    }
    
    #[test]
    fn test_symbol_parsing() {
        assert_eq!(
            ArbitrageEngine::parse_symbol("BTC/USDT"),
            Some(("BTC".to_string(), "USDT".to_string()))
        );
        
        assert_eq!(ArbitrageEngine::parse_symbol("INVALID"), None);
    }
}