//! Matriz de Proyeccion Topologica 264x31
//!
//! 264 vectores estrategicos (filas) x 31 operadores matematicos (columnas).
//! Cada celda almacena el valor escalar proyectado de un operador sobre un estado.

pub mod topology_map;

pub use topology_map::TopologyMap;
