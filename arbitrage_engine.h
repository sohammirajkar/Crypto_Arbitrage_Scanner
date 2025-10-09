// arbitrage_engine.h - Core arbitrage detection engine interface
#pragma once

#include "cpp-types-header.h"
#include <functional>
#include <array>
#include <unordered_map>
#include <thread>
#include <mutex>
#include <optional>
#include <pthread.h>

// Forward declaration for the third-party library
namespace moodycamel {
    template<typename T>
    class ReaderWriterQueue;
}

namespace arbitrage {

// Forward declarations
class ArbitrageEngine;

// Callback type for arbitrage opportunities
typedef std::function<void(const ArbitrageOpportunity&)> OpportunityCallback;

// Constants
const size_t MAX_EXCHANGES = 16;
const size_t MAX_SYMBOLS = 256;

class ArbitrageEngine {
private:
    Config config_;
    std::atomic<bool> running_;
    PerformanceStats stats_;
    
    // Price graph for Bellman-Ford algorithm
    std::array<std::array<double, MAX_EXCHANGES * MAX_SYMBOLS>, 
               MAX_EXCHANGES * MAX_SYMBOLS> price_graph_;
    
    // Currency indexing
    std::unordered_map<std::string, size_t> currency_indices_;
    std::unordered_map<size_t, std::string> currency_names_;
    
    // Threading
    std::thread processing_thread_;
    std::thread detection_thread_;
    std::atomic<Timestamp> last_update_time_;
    std::atomic<uint64_t> sequence_counter_;
    
    // Lock-free queue for market data
    moodycamel::ReaderWriterQueue<MarketTick>* tick_queue_;
    
    // Opportunity storage and callbacks
    mutable std::mutex opportunities_mutex_;
    std::vector<ArbitrageOpportunity> detected_opportunities_;
    mutable std::mutex callback_mutex_;
    std::vector<OpportunityCallback> callbacks_;
    
    // Helper methods
    void process_market_data();
    void update_price_graph(const MarketTick& tick);
    void detect_arbitrage_opportunities();
    std::vector<ArbitrageOpportunity> find_negative_cycles();
    std::optional<ArbitrageOpportunity> extract_arbitrage_cycle(
        int cycle_node, 
        const std::vector<int>& parent, 
        const std::vector<double>& dist);
    uint32_t calculate_confidence(const std::vector<int>& path, double log_return) const;
    Volume estimate_max_volume(const std::vector<int>& path) const;
    std::optional<std::pair<std::string, std::string> > parse_symbol(const std::string& symbol);
    size_t get_currency_index(const std::string& currency, uint8_t exchange);
    std::string get_currency_name(size_t index) const;
    void on_opportunity_detected(const ArbitrageOpportunity& opportunity);

public:
    explicit ArbitrageEngine(const Config& config);
    ~ArbitrageEngine();
    
    // Control methods
    void start();
    void stop();
    
    // Market data interface
    bool update_price(Exchange exchange, const std::string& symbol,
                     Price bid, Price ask, Volume volume);
    
    // Callback registration
    void register_opportunity_callback(OpportunityCallback callback);
    
    // Query methods
    std::vector<ArbitrageOpportunity> get_recent_opportunities(size_t limit) const;
    PerformanceStats get_performance_stats() const;
};

} // namespace arbitrage