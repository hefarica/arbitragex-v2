use rand::Rng;

/// Modelo Corregido: Retry como retraso puro del dado de absorción
struct MonteCarloValidator {
    max_retries: u64,
    direct_success_prob: f64,
    retry_success_prob: f64,
}

impl MonteCarloValidator {
    /// Loop que implementa la topología correcta
    fn simulate_trajectory(&self) -> (bool, u64) {
        let mut deliveries = 0u64;

        loop {
            deliveries += 1;

            // 1. DLQ FORZADO
            if deliveries > self.max_retries {
                return (false, deliveries);
            }

            let roll = rand::thread_rng().gen::<f64>();

            // 2. DADO DE ABSORCIÓN (Probabilidad constante p_s + p_d = 1 - p_r)
            if roll < self.direct_success_prob {
                return (true, deliveries);
            } else if roll < (self.direct_success_prob + self.retry_success_prob) {
                continue; // Retry: solo retrasar el lanzamiento del dado
            } else {
                return (false, deliveries);
            }
        }
    }

    /// Cálculo Teórico Exacto para Validación TCL
    fn expected_value(&self) -> f64 {
        let p_r = self.retry_success_prob;
        let K = self.max_retries as i32;

        (1.0 - p_r.powi(K + 1)) / (1.0 - p_r)
    }

    /// Validación con TCL Calcado de la Muestra (No hay fórmulas mágicas)
    fn validate(&self, trials: u64) -> (f64, f64, f64, f64) {
        let mut results: Vec<u64> = Vec::with_capacity(trials as usize);

        for _ in 0..trials {
            let (success, deliveries) = self.simulate_trajectory();
            results.push(deliveries);
        }

        let mean: f64 = results.iter().sum::<u64>() as f64 / trials as f64;
        let expected = self.expected_value();

        // Desviación estándar de la MUESTRA (Empírica)
        let variance = results
            .iter()
            .map(|&x| (x as f64 - mean).powi(2))
            .sum::<f64>()
            / trials as f64;
        let std_dev = variance.sqrt();

        // TCL: Media Empírica debe estar dentro de 3σ/√N
        let tcl_threshold = 3.0 * std_dev / (trials as f64).sqrt();
        let error = (mean - expected).abs();

        (mean, expected, error, tcl_threshold)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_convergence_final() {
        let op = MonteCarloValidator {
            max_retries: 3,
            direct_success_prob: 0.4,
            retry_success_prob: 0.4,
        };

        let (mean, exp, err, tcl) = op.validate(10_000);

        println!("Media: {:.4}", mean);
        println!("Teórico: {:.4}", exp);
        println!("Error: {:.4}", err);
        println!("TCL: {:.4}", tcl);

        assert!(err < tcl, "Error {:.4} > TCL {:.4}", err, tcl);
    }
}
