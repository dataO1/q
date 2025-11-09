use ai_agent_orchestrator::OrchestratorSystem;
use axum::{extract::Path, response::sse::{Sse, Event}, Extension};
use futures::stream::Stream;
use std::{convert::Infallible, sync::Arc};
use tokio::sync::broadcast::{Receiver, error::RecvError};

pub async fn stream_status(
    Extension(orchestrator): Extension<Arc<OrchestratorSystem>>,
    Path(task_id): Path<String>,
) -> Sse<impl Stream<Item = Result<Event, Infallible>>> {
    let mut receiver: Receiver<_> = orchestrator.subscribe_to_status(task_id).await;
    let stream = async_stream::stream! {
        loop {
            match receiver.recv().await {
                Ok(event) => {
                    let data = serde_json::to_string(&event).unwrap();
                    yield Ok(Event::default().data(data));
                }
                Err(RecvError::Lagged(_)) => continue,
                Err(_) => break,
            }
        }
    };
    Sse::new(stream)
}
