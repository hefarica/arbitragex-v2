pub trait FeedbackLoop {
    fn record_outcome(&self, outcome_hash: &str) -> Result<(), String>;
}
