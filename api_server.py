#!/usr/bin/env python3
"""
Simple API server to serve market anomalies and statistics
for the web dashboard.
"""

from http.server import HTTPServer, BaseHTTPRequestHandler
import json
import time
import threading
import random

# Mock data for market anomalies
mock_opportunities = [
    {
        "path": "SPY Statistical Arbitrage Detected",
        "profit_percentage": 0.0045,
        "max_volume": 12500.0,
        "confidence": 94,
        "detected_at": time.time() - 2,
        "exchanges": ["NYSE", "NASDAQ"]
    },
    {
        "path": "QQQ Index Constituent Mispricing Detected",
        "profit_percentage": 0.0032,
        "max_volume": 8500.0,
        "confidence": 87,
        "detected_at": time.time() - 5,
        "exchanges": ["NASDAQ"]
    },
    {
        "path": "DIA Cross-Index Discrepancy Detected",
        "profit_percentage": 0.0028,
        "max_volume": 20000.0,
        "confidence": 91,
        "detected_at": time.time() - 1,
        "exchanges": ["NYSE", "DOW"]
    }
]

# Mock statistics
mock_stats = {
    "messages_processed": 45623,
    "anomalies_found": 127,
    "avg_latency_us": 145.7,
    "last_update": time.time()
}

class APIServer(BaseHTTPRequestHandler):
    def do_GET(self):
        if self.path == '/api/opportunities':
            self.send_response(200)
            self.send_header('Content-type', 'application/json')
            self.send_header('Access-Control-Allow-Origin', '*')
            self.end_headers()
            
            # Update mock data with current timestamps
            updated_opportunities = []
            for opp in mock_opportunities:
                updated_opp = opp.copy()
                updated_opp["detected_at"] = time.time() - random.randint(0, 10)
                updated_opportunities.append(updated_opp)
            
            self.wfile.write(json.dumps(updated_opportunities).encode())
            
        elif self.path == '/api/stats':
            self.send_response(200)
            self.send_header('Content-type', 'application/json')
            self.send_header('Access-Control-Allow-Origin', '*')
            self.end_headers()
            
            # Update mock stats
            updated_stats = mock_stats.copy()
            updated_stats["last_update"] = time.time()
            updated_stats["messages_processed"] += random.randint(10, 100)
            updated_stats["anomalies_found"] += random.randint(0, 3)
            
            response = json.dumps(updated_stats)
            self.wfile.write(response.encode())
            
        elif self.path == '/health':
            self.send_response(200)
            self.send_header('Content-type', 'application/json')
            self.send_header('Access-Control-Allow-Origin', '*')
            self.end_headers()
            
            health_data = {
                "status": "healthy",
                "timestamp": time.time()
            }
            self.wfile.write(json.dumps(health_data).encode())
            
        else:
            self.send_response(404)
            self.end_headers()
            self.wfile.write(b'Not Found')

def run_server():
    server_address = ('localhost', 8001)
    httpd = HTTPServer(server_address, APIServer)
    print("🚀 API Server running on port 8001")
    print("Available endpoints:")
    print("  GET /api/opportunities - Get recent market anomalies")
    print("  GET /api/stats - Get performance statistics")
    print("  GET /health - Health check")
    httpd.serve_forever()

if __name__ == '__main__':
    run_server()