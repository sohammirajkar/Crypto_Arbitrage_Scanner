import plotly.graph_objects as go
import plotly.express as px

# Create a system architecture diagram using Plotly
fig = go.Figure()

# Define component positions and types
components = {
    # Data Sources (top row)
    'Binance WS': {'x': 1, 'y': 5, 'type': 'data_source', 'details': ['WebSocket', 'Real-time', 'High Freq']},
    'Coinbase WS': {'x': 2, 'y': 5, 'type': 'data_source', 'details': ['WebSocket', 'Real-time', 'Pro Feed']},
    'Kraken WS': {'x': 3, 'y': 5, 'type': 'data_source', 'details': ['WebSocket', 'Real-time', 'Low Latency']},
    
    # Network Layer
    'Network Layer': {'x': 2, 'y': 4, 'type': 'processing', 'details': ['Connection Mgmt', 'Reconnect Logic', 'Rate Limiting']},
    
    # Processing Layers
    'Threading': {'x': 2, 'y': 3, 'type': 'processing', 'details': ['Thread Pool', 'Parser Threads', 'Queue Mgmt']},
    'Normalization': {'x': 2, 'y': 2.5, 'type': 'processing', 'details': ['Data Standard', 'Symbol Map', 'Timestamp Sync']},
    
    # Core Engine
    'Arbitrage Engine': {'x': 2, 'y': 1.5, 'type': 'core', 'details': ['Bellman-Ford', 'Triangle Detect', 'Profit Calc']},
    
    # Output Systems
    'Alert System': {'x': 0.5, 'y': 0.5, 'type': 'output', 'details': ['Real-time Alert', 'Notifications', 'Thresholds']},
    'Web Dashboard': {'x': 2, 'y': 0.5, 'type': 'output', 'details': ['Live Prices', 'Visualizations', 'Metrics']},
    'Storage': {'x': 3.5, 'y': 0.5, 'type': 'storage', 'details': ['Time-series DB', 'Historical', 'Config']}
}

# Color mapping for component types
colors = {
    'data_source': '#1FB8CD',  # Strong cyan
    'processing': '#DB4545',   # Bright red
    'core': '#2E8B57',         # Sea green
    'output': '#5D878F',       # Cyan
    'storage': '#D2BA4C'       # Moderate yellow
}

# Add component boxes
for name, info in components.items():
    color = colors[info['type']]
    
    # Create hover text with details
    hover_text = f"<b>{name}</b><br>" + "<br>".join(info['details'])
    
    # Add rectangle for component
    fig.add_shape(
        type="rect",
        x0=info['x']-0.35, y0=info['y']-0.15,
        x1=info['x']+0.35, y1=info['y']+0.15,
        fillcolor=color,
        line=dict(color="black", width=2),
        opacity=0.8
    )
    
    # Add text label
    fig.add_trace(go.Scatter(
        x=[info['x']], y=[info['y']],
        text=[name],
        mode='text',
        textfont=dict(size=11, color='white', family='Arial Black'),
        hovertemplate=hover_text + "<extra></extra>",
        showlegend=False
    ))

# Add connection arrows
connections = [
    ('Binance WS', 'Network Layer'),
    ('Coinbase WS', 'Network Layer'),
    ('Kraken WS', 'Network Layer'),
    ('Network Layer', 'Threading'),
    ('Threading', 'Normalization'),
    ('Normalization', 'Arbitrage Engine'),
    ('Arbitrage Engine', 'Alert System'),
    ('Arbitrage Engine', 'Web Dashboard'),
    ('Arbitrage Engine', 'Storage')
]

for start, end in connections:
    start_pos = components[start]
    end_pos = components[end]
    
    # Calculate arrow positions
    if start_pos['y'] > end_pos['y']:  # Downward arrow
        start_y = start_pos['y'] - 0.15
        end_y = end_pos['y'] + 0.15
    else:  # Upward or sideways arrow
        start_y = start_pos['y']
        end_y = end_pos['y']
    
    # Add arrow
    fig.add_annotation(
        x=end_pos['x'], y=end_y,
        ax=start_pos['x'], ay=start_y,
        xref='x', yref='y',
        axref='x', ayref='y',
        showarrow=True,
        arrowhead=2,
        arrowsize=1.5,
        arrowwidth=2,
        arrowcolor='#333333'
    )

# Create legend
legend_items = [
    ('Data Sources', colors['data_source']),
    ('Processing', colors['processing']),
    ('Core Engine', colors['core']),
    ('Output Systems', colors['output']),
    ('Storage', colors['storage'])
]

for i, (label, color) in enumerate(legend_items):
    fig.add_trace(go.Scatter(
        x=[None], y=[None],
        mode='markers',
        marker=dict(size=15, color=color, symbol='square'),
        name=label,
        showlegend=True
    ))

# Update layout
fig.update_layout(
    title="Crypto Arbitrage Scanner Architecture",
    xaxis=dict(range=[-0.5, 4], showgrid=False, showticklabels=False, zeroline=False),
    yaxis=dict(range=[0, 5.5], showgrid=False, showticklabels=False, zeroline=False),
    plot_bgcolor='white',
    legend=dict(
        orientation='h',
        yanchor='bottom',
        y=1.05,
        xanchor='center',
        x=0.5
    ),
    showlegend=True,
    hovermode='closest'
)

# Save the chart
fig.write_image("arbitrage_architecture.png")
fig.write_image("arbitrage_architecture.svg", format="svg")

print("System architecture diagram created successfully!")