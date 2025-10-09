// arbitrage_engine.cpp - Core arbitrage detection algorithms
#include "arbitrage_engine.h"
#include <algorithm>
#include <cmath>
#include <execution>
#include <limits>

namespace arbitrage {

ArbitrageEngine::ArbitrageEngine(const Config &config)
    : config_(config), running_(false), stats_{},
      price_graph_(MAX_EXCHANGES * MAX_SYMBOLS),
      last_update_time_(std::chrono::high_resolution_clock::now()),
      sequence_counter_(0), tick_queue_(nullptr) {
  // Initialize exchange rate graph with infinity
  for (auto &row : price_graph_) {
    std::fill(row.begin(), row.end(), std::numeric_limits<double>::infinity());
  }

  // Set diagonal to zero (no cost to convert currency to itself)
  for (size_t i = 0; i < price_graph_.size(); ++i) {
    price_graph_[i][i] = 0.0;
  }

  // Pre-allocate opportunity storage
  detected_opportunities_.reserve(1000);
}

ArbitrageEngine::~ArbitrageEngine() { stop(); }

void ArbitrageEngine::start() {
  if (running_.exchange(true)) {
    return; // Already running
  }

  // Start processing thread with core affinity
  processing_thread_ = std::thread([this] { process_market_data(); });

  // Start opportunity detection thread
  detection_thread_ = std::thread([this] { detect_arbitrage_opportunities(); });
}

void ArbitrageEngine::stop() {
  running_.store(false);

  if (processing_thread_.joinable()) {
    processing_thread_.join();
  }

  if (detection_thread_.joinable()) {
    detection_thread_.join();
  }
}

bool ArbitrageEngine::update_price(Exchange exchange, const std::string &symbol,
                                   Price bid, Price ask, Volume volume) {
  auto start_time = std::chrono::high_resolution_clock::now();

  // Create market tick
  MarketTick tick;
  tick.exchange = exchange;
  std::strncpy(tick.symbol, symbol.c_str(), sizeof(tick.symbol) - 1);
  tick.bid = bid;
  tick.ask = ask;
  tick.last_price = (bid + ask) / 2.0;
  tick.volume = volume;
  tick.timestamp = start_time;
  tick.sequence = sequence_counter_.fetch_add(1);

  // Try to enqueue - if queue is full, drop oldest messages (backpressure)
  if (tick_queue_ != nullptr) {
    bool success = tick_queue_->try_enqueue(tick);

    if (success) {
      stats_.messages_processed.fetch_add(1);

      // Update latency statistics
      auto end_time = std::chrono::high_resolution_clock::now();
      auto latency_us = std::chrono::duration_cast<std::chrono::microseconds>(
                            end_time - start_time)
                            .count();
      stats_.update_latency(static_cast<double>(latency_us));
    }
    
    return success;
  }
  
  return false;
}

void ArbitrageEngine::process_market_data() {
  MarketTick tick;
  const auto wait_time =
      std::chrono::microseconds(100); // 100μs polling interval

  while (running_.load()) {
    if (tick_queue_.try_dequeue(tick)) {
      update_price_graph(tick);
      last_update_time_.store(tick.timestamp);
    } else {
      // No data available, brief sleep to prevent busy waiting
      std::this_thread::sleep_for(wait_time);
    }
  }
}

void ArbitrageEngine::update_price_graph(const MarketTick &tick) {
  // Update the price graph with new market data
  // This implements a simplified currency graph where each trading pair
  // represents an edge in the graph

  auto symbol_pair = parse_symbol(tick.symbol);
  if (!symbol_pair.has_value())
    return;

  auto [base, quote] = symbol_pair.value();

  // Get indices for base and quote currencies
  size_t base_idx =
      get_currency_index(base, static_cast<uint8_t>(tick.exchange));
  size_t quote_idx =
      get_currency_index(quote, static_cast<uint8_t>(tick.exchange));

  if (base_idx >= price_graph_.size() || quote_idx >= price_graph_.size()) {
    return;
  }

  // Update graph edges
  // Forward edge: base -> quote (selling base for quote)
  if (tick.bid > 0) {
    price_graph_[base_idx][quote_idx] = -std::log(tick.bid);
  }

  // Reverse edge: quote -> base (buying base with quote)
  if (tick.ask > 0) {
    price_graph_[quote_idx][base_idx] = -std::log(1.0 / tick.ask);
  }
}

void ArbitrageEngine::detect_arbitrage_opportunities() {
  const auto detection_interval =
      std::chrono::milliseconds(10); // 100Hz detection

  while (running_.load()) {
    auto start_time = std::chrono::high_resolution_clock::now();

    // Run Bellman-Ford algorithm to detect negative cycles (arbitrage)
    auto opportunities = find_negative_cycles();

    // Process found opportunities
    for (const auto &opp : opportunities) {
      if (opp.is_profitable(config_.arbitrage.min_profit_threshold)) {
        on_opportunity_detected(opp);
        stats_.opportunities_found.fetch_add(1);
      }
    }

    auto end_time = std::chrono::high_resolution_clock::now();
    auto detection_latency =
        std::chrono::duration_cast<std::chrono::microseconds>(end_time -
                                                              start_time);

    // Maintain detection frequency
    if (detection_latency < detection_interval) {
      std::this_thread::sleep_for(detection_interval - detection_latency);
    }
  }
}

std::vector<ArbitrageOpportunity> ArbitrageEngine::find_negative_cycles() {
  std::vector<ArbitrageOpportunity> opportunities;
  const size_t V = price_graph_.size();

  // Bellman-Ford algorithm for shortest paths
  std::vector<double> dist(V, std::numeric_limits<double>::infinity());
  std::vector<int> parent(V, -1);

  // Try each vertex as source to find all negative cycles
  for (size_t src = 0; src < V; ++src) {
    if (price_graph_[src][src] != 0.0)
      continue; // Skip inactive currencies

    std::fill(dist.begin(), dist.end(),
              std::numeric_limits<double>::infinity());
    std::fill(parent.begin(), parent.end(), -1);
    dist[src] = 0.0;

    // Relax edges V-1 times
    for (size_t i = 0; i < V - 1; ++i) {
      bool updated = false;
      for (size_t u = 0; u < V; ++u) {
        if (dist[u] == std::numeric_limits<double>::infinity())
          continue;

        for (size_t v = 0; v < V; ++v) {
          if (price_graph_[u][v] != std::numeric_limits<double>::infinity()) {
            double new_dist = dist[u] + price_graph_[u][v];
            if (new_dist < dist[v]) {
              dist[v] = new_dist;
              parent[v] = static_cast<int>(u);
              updated = true;
            }
          }
        }
      }
      if (!updated)
        break; // Early termination if no updates
    }

    // Check for negative cycles
    for (size_t u = 0; u < V; ++u) {
      if (dist[u] == std::numeric_limits<double>::infinity())
        continue;

      for (size_t v = 0; v < V; ++v) {
        if (price_graph_[u][v] != std::numeric_limits<double>::infinity()) {
          if (dist[u] + price_graph_[u][v] < dist[v]) {
            // Found negative cycle - extract the opportunity
            auto opp =
                extract_arbitrage_cycle(static_cast<int>(v), parent, dist);
            if (opp.has_value()) {
              opportunities.push_back(opp.value());
            }
          }
        }
      }
    }
  }

  return opportunities;
}

std::optional<ArbitrageOpportunity>
ArbitrageEngine::extract_arbitrage_cycle(int cycle_node,
                                         const std::vector<int> &parent,
                                         const std::vector<double> &dist) {

  // Trace back to find the cycle
  std::vector<int> cycle;
  std::unordered_set<int> visited;

  int current = cycle_node;
  while (current != -1 && visited.find(current) == visited.end()) {
    visited.insert(current);
    cycle.push_back(current);
    current = parent[current];
  }

  if (current == -1 || cycle.size() < 3) {
    return std::nullopt; // Invalid cycle
  }

  // Find where cycle starts
  auto cycle_start = std::find(cycle.begin(), cycle.end(), current);
  if (cycle_start == cycle.end()) {
    return std::nullopt;
  }

  // Extract the actual cycle
  std::vector<int> arbitrage_path(cycle_start, cycle.end());
  std::reverse(arbitrage_path.begin(), arbitrage_path.end());

  // Calculate profit
  double total_log_return = 0.0;
  for (size_t i = 0; i < arbitrage_path.size(); ++i) {
    size_t next_i = (i + 1) % arbitrage_path.size();
    total_log_return += price_graph_[arbitrage_path[i]][arbitrage_path[next_i]];
  }

  double profit_multiplier = std::exp(-total_log_return);
  double profit_percentage = profit_multiplier - 1.0;

  if (profit_percentage <= 0) {
    return std::nullopt; // No profit
  }

  // Build opportunity description
  ArbitrageOpportunity opp;
  opp.profit_percentage = profit_percentage;
  opp.detected_at = std::chrono::high_resolution_clock::now();
  opp.confidence = calculate_confidence(arbitrage_path, total_log_return);
  opp.max_volume = estimate_max_volume(arbitrage_path);

  // Build path string
  std::string path_str;
  for (size_t i = 0; i < arbitrage_path.size(); ++i) {
    if (i > 0)
      path_str += " -> ";
    path_str += get_currency_name(arbitrage_path[i]);
  }
  opp.path = path_str;

  return opp;
}

uint32_t ArbitrageEngine::calculate_confidence(const std::vector<int> &path,
                                               double log_return) const {
  // Confidence based on:
  // 1. Magnitude of profit (higher = more confident)
  // 2. Path length (shorter = more confident)
  // 3. Data freshness (newer = more confident)

  double profit_confidence = std::min(std::abs(log_return) * 100.0, 50.0);
  double path_confidence = std::max(0.0, 50.0 - path.size() * 10.0);

  auto now = std::chrono::high_resolution_clock::now();
  auto data_age = std::chrono::duration_cast<std::chrono::milliseconds>(
                      now - last_update_time_.load())
                      .count();
  double freshness_confidence = std::max(0.0, 50.0 - data_age / 100.0);

  return static_cast<uint32_t>(profit_confidence + path_confidence +
                               freshness_confidence);
}

Volume
ArbitrageEngine::estimate_max_volume(const std::vector<int> &path) const {
  // Simplified volume estimation - in practice this would consider
  // order book depth and liquidity at each step
  return config_.arbitrage.max_position_size / path.size();
}

std::optional<std::pair<std::string, std::string>>
ArbitrageEngine::parse_symbol(const std::string &symbol) {
  // Parse trading pairs like "BTC/USDT", "ETH/BTC", etc.
  auto pos = symbol.find('/');
  if (pos == std::string::npos || pos == 0 || pos == symbol.length() - 1) {
    return std::nullopt;
  }

  return std::make_pair(symbol.substr(0, pos), symbol.substr(pos + 1));
}

size_t ArbitrageEngine::get_currency_index(const std::string &currency,
                                           uint8_t exchange) {
  // Create unique index for each currency on each exchange
  // In practice, this would use a hash table for O(1) lookup
  std::string key = currency + "_" + std::to_string(exchange);

  auto it = currency_indices_.find(key);
  if (it != currency_indices_.end()) {
    return it->second;
  }

  // Assign new index
  size_t index = currency_indices_.size();
  currency_indices_[key] = index;
  currency_names_[index] = key;

  return index;
}

std::string ArbitrageEngine::get_currency_name(size_t index) const {
  auto it = currency_names_.find(index);
  return (it != currency_names_.end()) ? it->second : "UNKNOWN";
}

void ArbitrageEngine::register_opportunity_callback(
    OpportunityCallback callback) {
  std::lock_guard<std::mutex> lock(callback_mutex_);
  callbacks_.push_back(std::move(callback));
}

void ArbitrageEngine::on_opportunity_detected(
    const ArbitrageOpportunity &opportunity) {
  // Rate limiting
  auto now = std::chrono::high_resolution_clock::now();
  static auto last_alert = now;
  static uint32_t alerts_this_second = 0;

  if (std::chrono::duration_cast<std::chrono::seconds>(now - last_alert)
          .count() >= 1) {
    alerts_this_second = 0;
    last_alert = now;
  }

  if (alerts_this_second >= config_.arbitrage.max_opportunities_per_second) {
    return; // Rate limited
  }

  alerts_this_second++;

  // Store opportunity
  {
    std::lock_guard<std::mutex> lock(opportunities_mutex_);
    detected_opportunities_.push_back(opportunity);

    // Keep only recent opportunities (last 1000)
    if (detected_opportunities_.size() > 1000) {
      detected_opportunities_.erase(detected_opportunities_.begin());
    }
  }

  // Notify callbacks
  {
    std::lock_guard<std::mutex> lock(callback_mutex_);
    for (const auto &callback : callbacks_) {
      try {
        callback(opportunity);
      } catch (const std::exception &e) {
        // Log error but continue with other callbacks
        std::cerr << "Error in opportunity callback: " << e.what() << std::endl;
      }
    }
  }
}

std::vector<ArbitrageOpportunity>
ArbitrageEngine::get_recent_opportunities(size_t limit) const {
  std::lock_guard<std::mutex> lock(opportunities_mutex_);

  size_t start_idx = detected_opportunities_.size() > limit
                         ? detected_opportunities_.size() - limit
                         : 0;

  return std::vector<ArbitrageOpportunity>(detected_opportunities_.begin() +
                                               start_idx,
                                           detected_opportunities_.end());
}

PerformanceStats ArbitrageEngine::get_performance_stats() const {
  return stats_;
}

} // namespace arbitrage