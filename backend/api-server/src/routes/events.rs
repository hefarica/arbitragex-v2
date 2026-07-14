use axum::{
    extract::Query,
    response::sse::{Event, Sse},
    response::IntoResponse,
};
use std::convert::Infallible;
use std::time::Duration;

#[derive(Debug, serde::Deserialize)]
pub struct EventQuery {
    pub chain_id: u64,
}

pub async fn sse_events(Query(query): Query<EventQuery>) -> impl IntoResponse {
    let stream = tokio_stream::wrappers::IntervalStream::new(
        tokio::time::interval(Duration::from_secs(1))
    )
    .map(move |_| {
        Ok::<_, Infallible>(Event::default().data(format!(
            r#"{{"event_type":"heartbeat","chain_id":{}}}"#,
            query.chain_id
        )))
    });

    Sse::new(stream).keep_alive(axum::response::sse::KeepAlive::default())
}

pub async fn get_history(Query(query): Query<EventQuery>) -> impl IntoResponse {
    axum::Json(serde_json::json!({
        "chain_id": query.chain_id,
        "events": [],
    }))
}
