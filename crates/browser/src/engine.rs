//! Runs generations off the UI thread and reports back.

use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use tokio::task::AbortHandle;

use crate::browser::{Generation, RequestId, TabId};
use crate::llm::{self, GenerateError};
use crate::settings::Settings;

#[derive(Debug)]
pub enum EngineEvent {
    Generated {
        tab: TabId,
        request: RequestId,
        result: Result<String, GenerateError>,
    },
}

/// Called from a worker thread; the shell must hop to its main thread.
pub type EventSink = Arc<dyn Fn(EngineEvent) + Send + Sync>;

pub struct Engine {
    runtime: tokio::runtime::Runtime,
    client: reqwest::Client,
    pending: Arc<Mutex<HashMap<RequestId, AbortHandle>>>,
    mock: bool,
    sink: EventSink,
}

impl Engine {
    pub fn new(mock: bool, sink: EventSink) -> Engine {
        let runtime = tokio::runtime::Builder::new_multi_thread()
            .worker_threads(2)
            .enable_all()
            .thread_name("llmouser-engine")
            .build()
            .expect("tokio runtime");
        let client = reqwest::Client::builder()
            .user_agent(format!("{}/{}", crate::APP_NAME, crate::APP_VERSION))
            .build()
            .expect("http client");
        Engine {
            runtime,
            client,
            pending: Arc::default(),
            mock,
            sink,
        }
    }

    /// Whether the deterministic offline provider is selected by the environment.
    pub fn mock_from_env() -> bool {
        std::env::var(crate::env::MOCK)
            .map(|v| v == "1")
            .unwrap_or(false)
    }

    pub fn is_mock(&self) -> bool {
        self.mock
    }

    /// Start a generation; the sink receives the result unless it is cancelled.
    pub fn start(&self, generation: Generation, settings: Settings) {
        let Generation { tab, request, site } = generation;
        let client = self.client.clone();
        let mock = self.mock;
        let sink = self.sink.clone();
        let pending = self.pending.clone();
        // The task must not start generating until its abort handle is in
        // `pending`. Otherwise a fast generation (e.g. the mock) can finish and
        // remove `request` before `start` inserts it, leaving a stale handle.
        let (registered_tx, registered_rx) = tokio::sync::oneshot::channel();
        let handle = self.runtime.spawn(async move {
            let _ = registered_rx.await;
            let result = llm::generate(&client, &settings, &site, mock).await;
            pending.lock().expect("pending map").remove(&request);
            sink(EngineEvent::Generated {
                tab,
                request,
                result,
            });
        });
        self.pending
            .lock()
            .expect("pending map")
            .insert(request, handle.abort_handle());
        // Registration is done; let the task run (a dropped receiver, e.g. after
        // an immediate abort, simply makes the await return and the task end).
        let _ = registered_tx.send(());
    }

    /// Abort an in-flight generation; no event is delivered for it.
    pub fn cancel(&self, request: RequestId) {
        if let Some(handle) = self.pending.lock().expect("pending map").remove(&request) {
            handle.abort();
        }
    }

    /// Run a blocking job (e.g. writing a file) off the UI thread.
    pub fn run_blocking(&self, job: impl FnOnce() + Send + 'static) {
        self.runtime.spawn_blocking(job);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::i18n::Language;
    use crate::prompt::SiteRequest;
    use std::sync::mpsc;
    use std::time::Duration;

    fn engine() -> (Engine, mpsc::Receiver<EngineEvent>) {
        let (tx, rx) = mpsc::channel();
        let tx = Mutex::new(tx);
        let engine = Engine::new(
            true,
            Arc::new(move |e| {
                let _ = tx.lock().unwrap().send(e);
            }),
        );
        (engine, rx)
    }

    fn generation(request: RequestId, url: &str) -> Generation {
        Generation {
            tab: 1,
            request,
            site: SiteRequest {
                url: url.into(),
                referer: None,
                history: vec![],
            },
        }
    }

    #[test]
    fn delivers_results_and_cancels() {
        let (engine, rx) = engine();
        assert!(engine.is_mock());
        engine.start(
            generation(7, "https://a.com"),
            Settings::defaults_for(Language::En),
        );
        match rx.recv_timeout(Duration::from_secs(5)).unwrap() {
            EngineEvent::Generated {
                tab: 1,
                request: 7,
                result: Ok(html),
            } => assert!(html.contains("Mock page")),
            other => panic!("unexpected {other:?}"),
        }

        engine.start(
            generation(8, "https://slow.com"),
            Settings::defaults_for(Language::En),
        );
        engine.cancel(8);
        engine.cancel(999);
        assert!(rx.recv_timeout(Duration::from_millis(500)).is_err());
        assert!(engine.pending.lock().unwrap().is_empty());

        let (tx, done) = mpsc::channel();
        engine.run_blocking(move || tx.send(42).unwrap());
        assert_eq!(done.recv_timeout(Duration::from_secs(5)).unwrap(), 42);
    }
}
