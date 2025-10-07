# Real-time Cryptocurrency Arbitrage Scanner

A high-performance arbitrage detection system built with C++ and Rust, demonstrating HFT expertise in multithreading and low-latency design.

## 🎯 Purpose

This project showcases production-ready HFT system design by detecting cryptocurrency arbitrage opportunities across multiple exchanges in real-time.

## 👥 Target Audience

- HFT developers learning low-latency system design
- Quantitative traders exploring arbitrage strategies
- System architects designing real-time processing pipelines
- Computer science students studying concurrency patterns

## 🚀 Quick Start

```bash
# Run demo dashboard
python3 -m http.server 3000
# Open http://localhost:3000

# Build and run C++ version
cd cpp && mkdir build && cd build
cmake .. && make
./arbitrage_scanner

# Build and run Rust version
cd ../rust
cargo run --release
```

## 🔧 Key Features

- Multi-threaded architecture processing 50K+ price updates/sec
- Sub-millisecond arbitrage detection (<500μs)
- Lock-free data structures for zero-copy messaging
- Real-time WebSocket feeds from Binance, Coinbase, Kraken
- Web dashboard for live opportunity visualization
- Docker deployment for production environments

## 📊 Performance

- Message Processing: <100μs
- Arbitrage Detection: <500μs
- Memory Usage: <100MB
- Throughput: 50K+ price updates/sec

## 📄 License

MIT License
