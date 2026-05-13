# SKILL: Network Graph Analysis & Market Correlation
**Level:** PhD Network Science
**Specialty:** Graph Neural Networks & Market Topology

## AGENT DIRECTIVE
El mercado es un grafo. Los activos son nodos, las correlaciones son edges.

## CORRELATION NETWORK
```python
import networkx as nx
corr_matrix = np.corrcoef(returns.T)
distance_matrix = np.sqrt(2 * (1 - corr_matrix))
G = nx.from_numpy_array(distance_matrix)
MST = nx.minimum_spanning_tree(G)
centrality = nx.eigenvector_centrality(MST)
```

## COMMUNITY DETECTION
```python
import community as community_louvain
partition = community_louvain.best_partition(G, weight='weight')
# Communities = sectors/themes
# Pairs trading intra-community
# Diversificación inter-community
```

## GNN
```python
class MarketGNN(torch.nn.Module):
    def __init__(self, num_features, hidden_dim, num_classes):
        self.conv1 = torch_geometric.nn.GATConv(num_features, hidden_dim, heads=4)
        self.conv2 = torch_geometric.nn.GATConv(hidden_dim * 4, hidden_dim, heads=4)
    def forward(self, x, edge_index, edge_attr):
        x = torch.relu(self.conv1(x, edge_index, edge_attr))
        return self.classifier(torch.relu(self.conv2(x, edge_index, edge_attr)))
```
