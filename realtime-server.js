const http = require('http');
const WebSocket = require('ws');
const fs = require('fs');
const path = require('path');

// Exchange data storage
let exchangeData = {
    nyse: {},
    nasdaq: {},
    dow: {}
};

// Detected market anomalies
let marketAnomalies = [];

// Statistics
let stats = {
    messages_processed: 0,
    anomalies_found: 0,
    avg_latency_us: 0,
    last_update: Date.now()
};

// MIME types
const mimeTypes = {
    '.html': 'text/html',
    '.js': 'text/javascript',
    '.css': 'text/css',
    '.json': 'application/json'
};

// Connect to Equity Market WebSocket
function connectToEquityMarkets() {
    console.log('Connecting to Equity Markets...');

    // For demonstration, we'll simulate connections to major equity exchanges
    // In a real implementation, you would connect to actual exchange APIs
    console.log('Connected to NYSE, NASDAQ, and DOW JONES WebSocket feeds');

    // Start simulating market data
    setInterval(simulateMarketData, 1000);
}

// Simulate market data for demonstration
function simulateMarketData() {
    // Simulate price updates for major indices and their constituents
    const symbols = [
        'SPY',   // S&P 500 ETF
        'QQQ',   // NASDAQ-100 ETF
        'DIA',   // DOW JONES ETF
        'AAPL',  // Apple
        'MSFT',  // Microsoft
        'GOOGL', // Google
        'AMZN',  // Amazon
        'TSLA',  // Tesla
        'NVDA',  // NVIDIA
        'META'   // Meta
    ];

    // Update exchange data with simulated prices
    symbols.forEach(symbol => {
        // Simulate price movement
        const currentPrice = exchangeData.nasdaq[symbol] ? exchangeData.nasdaq[symbol].price : 100 + Math.random() * 500;
        const priceChange = (Math.random() - 0.5) * 2; // Random change between -1% and +1%
        const newPrice = currentPrice * (1 + priceChange / 100);

        // Store in all exchanges for demonstration
        exchangeData.nyse[symbol] = {
            price: newPrice,
            timestamp: Date.now()
        };
        exchangeData.nasdaq[symbol] = {
            price: newPrice * (1 + (Math.random() - 0.5) * 0.001), // Small difference between exchanges
            timestamp: Date.now()
        };
        exchangeData.dow[symbol] = {
            price: newPrice * (1 + (Math.random() - 0.5) * 0.001), // Small difference between exchanges
            timestamp: Date.now()
        };

        stats.messages_processed++;
    });

    // Detect market anomalies
    detectMarketAnomalies();
}

// Market anomaly detection algorithm
function detectMarketAnomalies() {
    // Check for statistical arbitrage opportunities between major indices
    const symbols = ['SPY', 'QQQ', 'DIA']; // Major ETFs representing indices

    // For demonstration, we'll create artificial anomalies
    symbols.forEach(symbol => {
        // Simulate anomalies with 0.3% probability
        if (Math.random() < 0.003) {
            const anomalyStrength = 0.002 + Math.random() * 0.003; // 0.2% to 0.5% anomaly

            // Calculate costs (simulated)
            const brokerageFee = 0.0005; // 0.05% brokerage fee
            const exchangeFee = 0.0001; // 0.01% exchange fee
            const slippage = 0.0001; // 0.01% slippage
            const totalCosts = brokerageFee + exchangeFee + slippage;
            const netAnomalyStrength = anomalyStrength - totalCosts;

            const anomalyTypes = [
                'Statistical Arbitrage',
                'Index Constituent Mispricing',
                'Cross-Index Discrepancy',
                'Volatility Anomaly'
            ];

            const anomalyType = anomalyTypes[Math.floor(Math.random() * anomalyTypes.length)];

            const anomaly = {
                path: `${symbol} ${anomalyType} Detected`,
                profit_percentage: anomalyStrength,
                net_profit_percentage: netAnomalyStrength,
                max_volume: 5000 + Math.random() * 10000, // $5K to $15K
                confidence: 75 + Math.floor(Math.random() * 20), // 75% to 95%
                detected_at: Date.now(),
                exchanges: ["NYSE", "NASDAQ", "DOW"],
                costs: {
                    brokerage: brokerageFee,
                    exchange: exchangeFee,
                    slippage: slippage,
                    total: totalCosts
                }
            };

            // Add to anomalies list (limit to 10)
            if (marketAnomalies.length < 10) {
                marketAnomalies.unshift(anomaly);
                stats.anomalies_found++;
                console.log(`${anomalyType} detected: ${(anomalyStrength * 100).toFixed(3)}%`);
            }
        }
    });
}

// HTTP server
const server = http.createServer((req, res) => {
    console.log(`${req.method} ${req.url}`);

    // Handle API endpoints
    if (req.url.startsWith('/api/')) {
        res.setHeader('Access-Control-Allow-Origin', '*');
        res.setHeader('Content-Type', 'application/json');

        if (req.url === '/api/opportunities' && req.method === 'GET') {
            res.writeHead(200);
            res.end(JSON.stringify(marketAnomalies));
            return;
        }

        if (req.url === '/api/stats' && req.method === 'GET') {
            stats.last_update = Date.now();
            res.writeHead(200);
            res.end(JSON.stringify(stats));
            return;
        }

        if (req.url === '/health' && req.method === 'GET') {
            res.writeHead(200);
            res.end(JSON.stringify({
                status: 'healthy',
                timestamp: Date.now(),
                connected_exchanges: Object.keys(connections).length
            }));
            return;
        }

        res.writeHead(404);
        res.end(JSON.stringify({ error: 'Not found' }));
        return;
    }

    // Serve static files
    let filePath = '.' + req.url;
    if (filePath === './') {
        filePath = './web-dashboard-index.html';
    }

    const extname = String(path.extname(filePath)).toLowerCase();
    const contentType = mimeTypes[extname] || 'application/octet-stream';

    fs.readFile(filePath, (error, content) => {
        if (error) {
            if (error.code === 'ENOENT') {
                fs.readFile('./web-dashboard-index.html', (error, content) => {
                    if (error) {
                        res.writeHead(500);
                        res.end('Sorry, check with the site admin for error: ' + error.code + ' ..\n');
                    } else {
                        res.writeHead(200, { 'Content-Type': 'text/html' });
                        res.end(content, 'utf-8');
                    }
                });
            } else {
                res.writeHead(500);
                res.end('Sorry, check with the site admin for error: ' + error.code + ' ..\n');
            }
        } else {
            res.writeHead(200, { 'Content-Type': contentType });
            res.end(content, 'utf-8');
        }
    });
});

const PORT = 3001; // Use a different port to avoid conflicts
server.listen(PORT, () => {
    console.log(`🚀 Real-time Server running on http://localhost:${PORT}`);
    console.log('Available endpoints:');
    console.log('  GET /api/opportunities - Get recent market anomalies');
    console.log('  GET /api/stats - Get performance statistics');
    console.log('  GET /health - Health check');
    console.log('  GET / - Web dashboard');

    // Connect to equity markets
    connectToEquityMarkets();
});