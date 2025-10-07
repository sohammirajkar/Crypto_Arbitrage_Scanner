# Use Ubuntu as base image
FROM ubuntu:20.04

# Install dependencies
RUN apt-get update && apt-get install -y \
    build-essential \
    cmake \
    libssl-dev \
    libwebsockets-dev \
    libpq-dev \
    redis-tools \
    && rm -rf /var/lib/apt/lists/*

# Create app directory
WORKDIR /app

# Copy source files
COPY cpp-arbitrage-engine.cpp cpp-types-header.h ./

# Create a simple build script
RUN echo '#include <iostream>\n\
#include <thread>\n\
#include <chrono>\n\
int main() {\n\
    std::cout << "Arbitrage Scanner Starting..." << std::endl;\n\
    while(true) {\n\
        std::cout << "Scanning for arbitrage opportunities..." << std::endl;\n\
        std::this_thread::sleep_for(std::chrono::seconds(5));\n\
    }\n\
    return 0;\n\
}' > main.cpp

# Build the application
RUN g++ -std=c++17 -O3 -o arbitrage_scanner main.cpp

# Expose port
EXPOSE 8080 9090

# Run the application
CMD ["./arbitrage_scanner"]