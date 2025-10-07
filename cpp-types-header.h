// types.h - Core data types and structures
#pragma once

#include <string>
#include <chrono>
#include <atomic>
#include <memory>
#include <vector>

namespace arbitrage {

using Price = double;
using Volume = double;
using Timestamp = std::chrono::high_resolution_clock::time_point;

// Exchange identifiers
enum class Exchange : uint8_t {
    BINANCE = 0,
    COINBASE = 1,
    KRAKEN = 2,
    UNKNOWN = 255
};

// Market data structure optimized for cache efficiency
struct alignas(64) MarketTick {
    Exchange exchange;
    char symbol[16];     // e.g., "BTC/USDT"
    Price bid;
    Price ask;
    Price last_price;
    Volume volume;
    Timestamp timestamp;
    uint64_t sequence;   // For ordering
    
    MarketTick() : exchange(Exchange::UNKNOWN), bid(0), ask(0), 
                   last_price(0), volume(0), sequence(0) {
        std::memset(symbol, 0, sizeof(symbol));
        timestamp = std::chrono::high_resolution_clock::now();
    }
};

// Arbitrage opportunity structure
struct ArbitrageOpportunity {
    Exchange exchange1, exchange2, exchange3;
    std::string path;           // e.g., "BTC/USDT -> ETH/BTC -> ETH/USDT"
    Price entry_price;
    Price exit_price;
    double profit_percentage;
    Volume max_volume;
    Timestamp detected_at;
    uint32_t confidence;        // 0-100 reliability score
    
    bool is_profitable(double min_profit = 0.001) const {
        return profit_percentage > min_profit;
    }
};

// Lock-free queue node for inter-thread communication
template<typename T>
struct alignas(64) QueueNode {
    std::atomic<QueueNode*> next{nullptr};
    T data;
    
    QueueNode() = default;
    QueueNode(const T& item) : data(item) {}
};

// Thread-safe statistics for performance monitoring
struct alignas(64) PerformanceStats {
    std::atomic<uint64_t> messages_processed{0};
    std::atomic<uint64_t> opportunities_found{0};
    std::atomic<uint64_t> false_positives{0};
    std::atomic<double> avg_latency_us{0.0};
    std::atomic<Timestamp> last_update;
    
    void update_latency(double latency_us) {
        // Exponential moving average
        double current = avg_latency_us.load();
        avg_latency_us.store(0.9 * current + 0.1 * latency_us);
    }
};

// Configuration for different deployment scenarios  
struct Config {
    struct Network {
        uint16_t dashboard_port = 8080;
        uint32_t websocket_timeout_ms = 30000;
        uint32_t reconnect_interval_ms = 5000;
        uint8_t max_reconnect_attempts = 10;
        bool enable_compression = true;
    } network;
    
    struct Threading {
        uint8_t num_exchange_threads = 3;
        uint8_t num_processor_threads = 4;
        bool pin_threads = true;
        uint32_t queue_capacity = 65536;
        std::vector<uint8_t> cpu_affinity = {0, 1, 2, 3};
    } threading;
    
    struct Arbitrage {
        double min_profit_threshold = 0.001;  // 0.1%
        double max_position_size = 1000.0;
        uint32_t max_opportunities_per_second = 100;
        bool enable_triangle_arbitrage = true;
        bool enable_cross_exchange = true;
        std::vector<std::string> monitored_symbols = {
            "BTC/USDT", "ETH/USDT", "BNB/USDT", "ADA/USDT"
        };
    } arbitrage;
    
    struct Logging {
        std::string level = "INFO";
        std::string file_path = "/var/log/arbitrage.log";
        bool enable_metrics = true;
        uint32_t metrics_interval_ms = 1000;
    } logging;
};

// Memory pool for zero-allocation message handling
template<typename T, size_t PoolSize = 1024>
class MemoryPool {
private:
    alignas(64) std::array<T, PoolSize> pool_;
    std::atomic<size_t> next_free_{0};
    std::array<std::atomic<bool>, PoolSize> in_use_;
    
public:
    MemoryPool() {
        for (auto& flag : in_use_) {
            flag.store(false);
        }
    }
    
    T* acquire() {
        for (size_t i = 0; i < PoolSize; ++i) {
            size_t idx = (next_free_.load() + i) % PoolSize;
            bool expected = false;
            if (in_use_[idx].compare_exchange_weak(expected, true)) {
                next_free_.store((idx + 1) % PoolSize);
                return &pool_[idx];
            }
        }
        return nullptr; // Pool exhausted - fallback to heap allocation
    }
    
    void release(T* ptr) {
        if (ptr >= pool_.data() && ptr < pool_.data() + PoolSize) {
            size_t idx = ptr - pool_.data();
            in_use_[idx].store(false);
        }
    }
};

} // namespace arbitrage