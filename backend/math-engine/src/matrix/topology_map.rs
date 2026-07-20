//! TopologyMap — Matriz de Proyeccion 264x31
//!
//! Mapea 264 vectores estrategicos contra 31 operadores topologicos.
//! La matriz se alimenta desde OperatorRegistry y MarketState.

use crate::operators::{MarketState, OperatorRegistry};
use nalgebra::{DMatrix, DVector};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Numero de vectores estrategicos (filas)
pub const ROWS: usize = 264;
/// Numero de operadores topologicos (columnas)
pub const COLS: usize = 31;

/// Matriz de proyeccion topologica: 264 estados estrategicos x 31 operadores.
///
/// Cada fila representa un vector estrategico (ej: par de tokens + manifold de liquidez).
/// Cada columna representa un operador matematico (op_01 .. op_31).
/// El valor de la celda (i, j) es el scalar_value normalizado del operador j
/// evaluado sobre el estado estrategico i.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TopologyMap {
    /// Matriz densa 264x31 de proyecciones escalares.
    /// Filas: vectores estrategicos. Columnas: operadores.
    pub projections: DMatrix<f64>,
    /// Mapeo de indice de fila a identificador del vector estrategico.
    pub row_labels: Vec<String>,
    /// Mapeo de indice de columna a identificador del operador.
    pub col_labels: Vec<String>,
    /// Cache de metadatos por celda (opcional, para diagnostico).
    #[serde(skip)]
    pub metadata: HashMap<(usize, usize), HashMap<String, f64>>,
}

impl TopologyMap {
    /// Crea una matriz vacia con etiquetas por defecto.
    pub fn new() -> Self {
        let row_labels = (0..ROWS)
            .map(|i| format!("vector_{:03}", i))
            .collect();
        let col_labels = (1..=COLS)
            .map(|j| format!("op_{:02}", j))
            .collect();

        Self {
            projections: DMatrix::zeros(ROWS, COLS),
            row_labels,
            col_labels,
            metadata: HashMap::new(),
        }
    }

    /// Crea una matriz con etiquetas personalizadas.
    pub fn with_labels(row_labels: Vec<String>, col_labels: Vec<String>) -> Self {
        assert_eq!(row_labels.len(), ROWS, "row_labels debe tener {} elementos", ROWS);
        assert_eq!(col_labels.len(), COLS, "col_labels debe tener {} elementos", COLS);

        Self {
            projections: DMatrix::zeros(ROWS, COLS),
            row_labels,
            col_labels,
            metadata: HashMap::new(),
        }
    }

    /// Proyecta un unico estado de mercado contra todos los operadores registrados.
    ///
    /// Devuelve un vector de 31 escalares (uno por operador).
    /// Si un operador no esta disponible o devuelve None, la celda es 0.0.
    pub fn project_state(
        &mut self,
        registry: &OperatorRegistry,
        state: &MarketState,
        row_index: usize,
    ) -> DVector<f64> {
        assert!(row_index < ROWS, "row_index {} fuera de rango [0, {})", row_index, ROWS);

        let mut result = DVector::zeros(COLS);
        let available = registry.available();

        for op in available {
            let col = (op.id() as usize).saturating_sub(1);
            if col >= COLS {
                continue;
            }
            let output = op.evaluate(state);
            let scalar = output.scalar_value.unwrap_or(0.0);
            result[col] = scalar;
            self.projections[(row_index, col)] = scalar;
            self.metadata
                .insert((row_index, col), output.metadata);
        }

        result
    }

    /// Proyecta un lote de estados de mercado (batch) contra todos los operadores.
    ///
    /// `states` debe tener longitud <= ROWS. Cada estado se proyecta en su fila correspondiente.
    /// Devuelve la sub-matriz resultante (n_states x COLS).
    pub fn batch_project(
        &mut self,
        registry: &OperatorRegistry,
        states: &[MarketState],
    ) -> DMatrix<f64> {
        let n = states.len().min(ROWS);
        let mut batch = DMatrix::zeros(n, COLS);

        for (i, state) in states.iter().enumerate().take(n) {
            let row = self.project_state(registry, state, i);
            batch.set_row(i, &row.transpose());
        }

        batch
    }

    /// Obtiene el valor de una celda (fila, columna).
    pub fn get(&self, row: usize, col: usize) -> Option<f64> {
        self.projections.get((row, col)).copied()
    }

    /// Establece el valor de una celda (fila, columna).
    pub fn set(&mut self, row: usize, col: usize, value: f64) {
        assert!(row < ROWS, "row {} fuera de rango [0, {})", row, ROWS);
        assert!(col < COLS, "col {} fuera de rango [0, {})", col, COLS);
        self.projections[(row, col)] = value;
    }

    /// Obtiene una fila completa (vector estrategico proyectado).
    pub fn row(&self, index: usize) -> Option<DVector<f64>> {
        if index >= ROWS {
            return None;
        }
        Some(self.projections.row(index).transpose().into_owned())
    }

    /// Obtiene una columna completa (resultado de un operador sobre todos los vectores).
    pub fn col(&self, index: usize) -> Option<DVector<f64>> {
        if index >= COLS {
            return None;
        }
        Some(self.projections.column(index).into_owned())
    }

    /// Normaliza la matriz por columnas (z-score sobre cada operador).
    ///
    /// Util para comparar operadores con escalas distintas.
    pub fn normalize_columns(&mut self) {
        for j in 0..COLS {
            let col = self.projections.column(j);
            let mean = col.mean();
            let std = col.variance().sqrt();
            if std > 0.0 {
                for i in 0..ROWS {
                    self.projections[(i, j)] = (self.projections[(i, j)] - mean) / std;
                }
            }
        }
    }

    /// Normaliza la matriz por filas (z-score sobre cada vector estrategico).
    pub fn normalize_rows(&mut self) {
        for i in 0..ROWS {
            let row = self.projections.row(i);
            let mean = row.mean();
            let std = row.variance().sqrt();
            if std > 0.0 {
                for j in 0..COLS {
                    self.projections[(i, j)] = (self.projections[(i, j)] - mean) / std;
                }
            }
        }
    }

    /// Calcula la descomposicion en valores singulares (SVD) de la matriz de proyecciones.
    ///
    /// Devuelve (U, S, Vt) donde:
    /// - U: matriz ortogonal 264x264
    /// - S: valores singulares (vector de tamano min(264, 31) = 31)
    /// - Vt: matriz ortogonal 31x31
    pub fn svd(&self) -> Option<(DMatrix<f64>, DVector<f64>, DMatrix<f64>)> {
        let svd = self.projections.clone().try_svd(true, true, f64::EPSILON, 0)?;
        let u = svd.u?;
        let v_t = svd.v_t?;
        let s = DVector::from_iterator(svd.singular_values.len(), svd.singular_values.iter().copied());
        Some((u, s, v_t))
    }

    /// Reduce dimensionalidad via SVD truncado a `k` componentes.
    ///
    /// Devuelve la matriz reconstruida de tamano 264x31.
    pub fn truncate_svd(&self, k: usize) -> Option<DMatrix<f64>> {
        let (u, s, v_t) = self.svd()?;
        let k = k.min(s.len()).min(COLS);
        let u_k = u.columns(0, k);
        let s_k = DMatrix::from_diagonal(&s.rows(0, k));
        let v_t_k = v_t.rows(0, k);
        Some(u_k * s_k * v_t_k)
    }

    /// Serializa la matriz a JSON (incluye projections, row_labels, col_labels).
    pub fn to_json(&self) -> Result<String, serde_json::Error> {
        serde_json::to_string_pretty(self)
    }

    /// Deserializa la matriz desde JSON.
    pub fn from_json(json: &str) -> Result<Self, serde_json::Error> {
        let mut map: Self = serde_json::from_str(json)?;
        map.metadata = HashMap::new();
        Ok(map)
    }

    /// Obtiene estadisticas descriptivas por columna (operador).
    pub fn column_stats(&self) -> Vec<ColumnStats> {
        let mut stats = Vec::with_capacity(COLS);
        for j in 0..COLS {
            let col = self.projections.column(j);
            let min = col.min();
            let max = col.max();
            let mean = col.mean();
            let var = col.variance();
            stats.push(ColumnStats {
                col_index: j,
                col_label: self.col_labels.get(j).cloned().unwrap_or_default(),
                min,
                max,
                mean,
                std: var.sqrt(),
            });
        }
        stats
    }

    /// Devuelve la fila con mayor energia (norma euclidea) en el espacio de operadores.
    pub fn dominant_row(&self) -> Option<(usize, f64)> {
        let mut max_idx = 0;
        let mut max_norm = 0.0;
        for i in 0..ROWS {
            let norm = self.projections.row(i).norm();
            if norm > max_norm {
                max_norm = norm;
                max_idx = i;
            }
        }
        Some((max_idx, max_norm))
    }

    /// Devuelve la columna con mayor varianza (operador mas discriminativo).
    pub fn dominant_col(&self) -> Option<(usize, f64)> {
        let mut max_idx = 0;
        let mut max_var = 0.0;
        for j in 0..COLS {
            let var = self.projections.column(j).variance();
            if var > max_var {
                max_var = var;
                max_idx = j;
            }
        }
        Some((max_idx, max_var))
    }
}

impl Default for TopologyMap {
    fn default() -> Self {
        Self::new()
    }
}

/// Estadisticas descriptivas de una columna (operador).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ColumnStats {
    pub col_index: usize,
    pub col_label: String,
    pub min: f64,
    pub max: f64,
    pub mean: f64,
    pub std: f64,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::operators::MarketState;

    #[test]
    fn test_topology_map_dimensions() {
        let map = TopologyMap::new();
        assert_eq!(map.projections.nrows(), ROWS);
        assert_eq!(map.projections.ncols(), COLS);
    }

    #[test]
    fn test_set_and_get() {
        let mut map = TopologyMap::new();
        map.set(0, 0, 3.14);
        assert_eq!(map.get(0, 0), Some(3.14));
    }

    #[test]
    fn test_project_state() {
        let mut map = TopologyMap::new();
        let registry = OperatorRegistry::new();
        let state = MarketState {
            price_matrix: vec![vec![1.0, 2.0]],
            liquidity_reserves: vec![(1000.0, 2000.0)],
            gas_price_gwei: 20.0,
            block_timestamp: 1234567890,
            block_number: 100,
            features: HashMap::new(),
        };
        let result = map.project_state(&registry, &state, 0);
        assert_eq!(result.len(), COLS);
    }

    #[test]
    fn test_batch_project() {
        let mut map = TopologyMap::new();
        let registry = OperatorRegistry::new();
        let states: Vec<MarketState> = (0..10)
            .map(|i| MarketState {
                price_matrix: vec![vec![i as f64, (i + 1) as f64]],
                liquidity_reserves: vec![(1000.0, 2000.0)],
                gas_price_gwei: 20.0,
                block_timestamp: 1234567890 + i as u64,
                block_number: 100 + i as u64,
                features: HashMap::new(),
            })
            .collect();
        let batch = map.batch_project(&registry, &states);
        assert_eq!(batch.nrows(), 10);
        assert_eq!(batch.ncols(), COLS);
    }

    #[test]
    fn test_svd() {
        let mut map = TopologyMap::new();
        for i in 0..ROWS {
            for j in 0..COLS {
                map.set(i, j, (i + j) as f64);
            }
        }
        let svd = map.svd();
        assert!(svd.is_some());
    }

    #[test]
    fn test_serialize_deserialize() {
        let mut map = TopologyMap::new();
        map.set(0, 0, 1.0);
        map.set(1, 2, 2.0);
        let json = map.to_json().unwrap();
        let restored = TopologyMap::from_json(&json).unwrap();
        assert_eq!(restored.get(0, 0), Some(1.0));
        assert_eq!(restored.get(1, 2), Some(2.0));
    }
}
