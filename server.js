const http = require('http');
const fs = require('fs');
const path = require('path');
const WebSocket = require('ws');

// Exchange data storage
let exchangeData = {
    binance: {},
    coinbase: {},
    kraken: {}
};

// Detected arbitrage opportunities
let arbitrageOpportunities = [];

// Statistics
let stats = {
    messages_processed: 0,
    opportunities_found: 0,
    avg_latency_us: 0,
    last_update: Date.now()
};

// Active WebSocket connections
let connections = {};

// Connect to Binance WebSocket
function connectToBinance() {
    try {
        const ws = new WebSocket('wss://stream.binance.com:9443/ws');

        ws.on('open', function open() {
            console.log('Connected to Binance WebSocket');
            connections.binance = ws;

            // Subscribe to ticker streams for major trading pairs
            const subscribeMsg = {
                method: "SUBSCRIBE",
                params: [
                    "btcusdt@ticker",
                    "ethusdt@ticker",
                    "bnbusdt@ticker",
                    "adausdt@ticker"
                ],
                id: 1
            };

            ws.send(JSON.stringify(subscribeMsg));
        });

        ws.on('message', function incoming(data) {
            try {
                const ticker = JSON.parse(data);

                // Process ticker data
                if (ticker.stream && ticker.data) {
                    const symbol = ticker.data.s.toLowerCase();
                    const price = parseFloat(ticker.data.c);

                    // Store the latest price
                    exchangeData.binance[symbol] = {
                        price: price,
                        timestamp: Date.now()
                    };

                    stats.messages_processed++;

                    // Detect arbitrage opportunities
                    detectArbitrageOpportunities();
                }
            } catch (error) {
                console.error('Error processing Binance message:', error);
            }
        });

        ws.on('error', function error(error) {
            console.error('Binance WebSocket error:', error);
        });

        ws.on('close', function close() {
            console.log('Binance WebSocket disconnected');
            delete connections.binance;

            // Reconnect after 5 seconds
            setTimeout(connectToBinance, 5000);
        });
    } catch (error) {
        console.error('Error connecting to Binance:', error);
        setTimeout(connectToBinance, 5000);
    }
}

// Simple arbitrage detection algorithm
function detectArbitrageOpportunities() {
    // This is a simplified version of the Bellman-Ford algorithm
    // In a production system, this would be much more complex

    const binance = exchangeData.binance;

    // Check for triangular arbitrage opportunities
    // Example: BTC/USDT -> ETH/BTC -> ETH/USDT
    if (binance.btcusdt && binance.ethbtc && binance.ethusdt) {
        const btcUsdtPrice = binance.btcusdt.price;
        const ethBtcPrice = binance.ethbtc.price;
        const ethUsdtPrice = binance.ethusdt.price;

        // Calculate the implied price
        const impliedEthUsdt = btcUsdtPrice * ethBtcPrice;

        // Calculate profit percentage
        const profitPercentage = (ethUsdtPrice / impliedEthUsdt) - 1;

        // If profit is positive and above threshold (0.1%)
        if (profitPercentage > 0.001) {
            const opportunity = {
                path: "BTC/USDT -> ETH/BTC -> ETH/USDT",
                profit_percentage: profitPercentage,
                max_volume: 1000, // Simplified
                confidence: 95,
                detected_at: Date.now(),
                exchanges: ["binance"]
            };

            // Add to opportunities list
            arbitrageOpportunities.unshift(opportunity);

            // Keep only the last 50 opportunities
            if (arbitrageOpportunities.length > 50) {
                arbitrageOpportunities.pop();
            }

            stats.opportunities_found++;
            console.log(`Arbitrage opportunity found: ${profitPercentage * 100}%`);
        }
    }

    // Check for cross-exchange opportunities (simplified)
    // In a real implementation, we would compare prices across exchanges
}

// MIME types
const mimeTypes = {
    '.html': 'text/html',
    '.js': 'text/javascript',
    '.css': 'text/css',
    '.json': 'application/json',
    '.png': 'image/png',
    '.jpg': 'image/jpg',
    '.gif': 'image/gif',
    '.svg': 'image/svg+xml',
    '.wav': 'audio/wav',
    '.mp4': 'video/mp4',
    '.woff': 'application/font-woff',
    '.ttf': 'application/font-ttf',
    '.eot': 'application/vnd.ms-fontobject',
    '.otf': 'application/font-otf',
    '.wasm': 'application/wasm'
};

const server = http.createServer((req, res) => {
    console.log(`${req.method} ${req.url}`);

    // Handle API endpoints
    if (req.url.startsWith('/api/')) {
        res.setHeader('Access-Control-Allow-Origin', '*');
        res.setHeader('Content-Type', 'application/json');

        if (req.url === '/api/opportunities' && req.method === 'GET') {
            // Return real arbitrage opportunities
            res.writeHead(200);
            res.end(JSON.stringify(arbitrageOpportunities));
            return;
        }

        if (req.url === '/api/stats' && req.method === 'GET') {
            // Update stats
            stats.last_update = Date.now();

            res.writeHead(200);
            res.end(JSON.stringify(stats));
            return;
        }

        if (req.url === '/health' && req.method === 'GET') {
            res.writeHead(200);
            res.end(JSON.stringify({ status: 'healthy', timestamp: Date.now() }));
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

const PORT = 3000;
server.listen(PORT, () => {
    console.log(`🚀 Server running on http://localhost:${PORT}`);
    console.log('Available endpoints:');
    console.log('  GET /api/opportunities - Get recent arbitrage opportunities');
    console.log('  GET /api/stats - Get performance statistics');
    console.log('  GET /health - Health check');
    console.log('  GET / - Web dashboard');

    // Connect to exchanges
    connectToBinance();
});