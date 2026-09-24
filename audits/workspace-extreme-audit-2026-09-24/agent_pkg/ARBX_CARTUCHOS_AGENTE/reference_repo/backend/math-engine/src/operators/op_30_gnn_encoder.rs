//! FUSILE: candle-core@v0.4 (crates.io) -- Graph Neural Network Encoder
//! Arquitectura: GraphSAGE simplificado (2 layers) + mean aggregation
//! Categoria: ml

use super::{MarketState, OperatorOutput, TopologicalOperator};
use nalgebra::DMatrix;
use std::collections::HashMap;

pub struct GnnEncoderOperator {
    hidden_dim: usize,
    n_layers: usize,
}

impl GnnEncoderOperator {
    pub fn new() -> Self {
        Self {
            hidden_dim: 16,
            n_layers: 2,
        }
    }

    pub fn encode(
        &self,
        node_features: &DMatrix<f64>,
        adjacency: &HashMap<usize, Vec<usize>>,
    ) -> DMatrix<f64> {
        let n_nodes = node_features.nrows();
        let mut h = node_features.clone();

        for _layer in 0..self.n_layers {
            let mut h_next = DMatrix::zeros(n_nodes, self.hidden_dim);
            for v in 0..n_nodes {
                let h_v = h.row(v).transpose();
                let mut neighbor_sum = nalgebra::DVector::zeros(h.ncols());
                let mut neighbor_count = 0usize;

                if let Some(neighbors) = adjacency.get(&v) {
                    for &u in neighbors {
                        if u < n_nodes {
                            neighbor_sum += h.row(u).transpose();
                            neighbor_count += 1;
                        }
                    }
                }

                let neighbor_mean = if neighbor_count > 0 {
                    neighbor_sum / neighbor_count as f64
                } else {
                    neighbor_sum
                };
                let combined = &h_v + &neighbor_mean;
                let mut projected = nalgebra::DVector::zeros(self.hidden_dim);
                for i in 0..self.hidden_dim.min(combined.nrows()) {
                    projected[i] = combined[i % combined.nrows()];
                }
                for i in 0..self.hidden_dim {
                    h_next[(v, i)] = projected[i].max(0.0);
                }
            }
            h = h_next;
        }
        h
    }
}

impl Default for GnnEncoderOperator {
    fn default() -> Self {
        Self::new()
    }
}

impl TopologicalOperator for GnnEncoderOperator {
    fn id(&self) -> u8 {
        30
    }
    fn name(&self) -> &'static str {
        "GNN Encoder"
    }
    fn category(&self) -> &'static str {
        "ml"
    }

    fn evaluate(&self, state: &MarketState) -> OperatorOutput {
        let n = state.price_matrix.len();
        if n == 0 {
            return OperatorOutput {
                operator_id: self.id(),
                operator_name: self.name().to_string(),
                scalar_value: None,
                vector_result: None,
                matrix_result: None,
                metadata: HashMap::new(),
            };
        }
        let m = state.price_matrix[0].len();
        let mut node_data = Vec::with_capacity(n * m);
        for row in &state.price_matrix {
            for &val in row {
                node_data.push(val);
            }
        }
        let node_features = DMatrix::from_row_slice(n, m, &node_data);

        let mut adjacency = HashMap::new();
        for v in 0..n {
            adjacency.insert(v, vec![(v + n - 1) % n, (v + 1) % n]);
        }

        let embeddings = self.encode(&node_features, &adjacency);

        let mut avg_embedding = nalgebra::DVector::zeros(self.hidden_dim);
        for v in 0..n {
            for i in 0..self.hidden_dim {
                avg_embedding[i] += embeddings[(v, i)];
            }
        }
        avg_embedding /= n as f64;
        let embedding_norm: f64 = avg_embedding.iter().map(|&x| x * x).sum::<f64>().sqrt();

        let mut metadata = HashMap::new();
        metadata.insert("n_nodes".to_string(), n as f64);
        metadata.insert("embedding_norm".to_string(), embedding_norm);

        OperatorOutput {
            operator_id: self.id(),
            operator_name: self.name().to_string(),
            scalar_value: Some(embedding_norm),
            vector_result: Some(avg_embedding.iter().cloned().collect()),
            matrix_result: None,
            metadata,
        }
    }
}
