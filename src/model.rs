//! Recorded model answers (I2). A live run with the owner's key saves each
//! answer under a fingerprint of the model and the exact request; tests and
//! CI replay them with no key and no network. A request nobody recorded
//! fails rather than reaching a model.

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::engine::llm::{LanguageModel, LlmError, LlmReply, LlmRequest};

/// Where the project's recorded answers are committed.
pub const DIR: &str = "evalset/recorded";

#[derive(Serialize, Deserialize)]
struct Saved {
    model: String,
    text: String,
    request_id: Option<String>,
    input_tokens: u32,
    output_tokens: u32,
}

/// The fingerprint of `request` sent to `model`.
pub fn fingerprint(model: &str, request: &LlmRequest) -> String {
    let mut hash = Sha256::new();
    for part in [
        model,
        &request.system,
        &request.user,
        &request.schema.to_string(),
        &request.max_tokens.to_string(),
    ] {
        hash.update(part.as_bytes());
        hash.update([0]);
    }
    hex::encode(hash.finalize())
}

fn path(dir: &Path, model: &str, request: &LlmRequest) -> PathBuf {
    dir.join(format!("{}.json", fingerprint(model, request)))
}

/// Answers from the recordings in a folder.
pub struct Recorded {
    model: String,
    dir: PathBuf,
}

impl Recorded {
    /// The recordings of `model` in `dir`.
    pub fn new(model: &str, dir: &Path) -> Self {
        Self {
            model: model.to_owned(),
            dir: dir.to_owned(),
        }
    }
}

impl LanguageModel for Recorded {
    fn model(&self) -> &str {
        &self.model
    }

    async fn complete(&self, request: &LlmRequest) -> Result<LlmReply, LlmError> {
        let file = path(&self.dir, &self.model, request);
        let saved = std::fs::read_to_string(&file).map_err(|_| {
            LlmError::Unavailable(format!(
                "no recorded answer for this request ({}); record it with the owner's key",
                file.display()
            ))
        })?;
        let saved: Saved = serde_json::from_str(&saved).map_err(|err| LlmError::Malformed(err.to_string()))?;
        Ok(LlmReply {
            text: saved.text,
            model: saved.model,
            request_id: saved.request_id,
            input_tokens: saved.input_tokens,
            output_tokens: saved.output_tokens,
        })
    }
}

/// A live model whose every answer is also saved to a folder.
pub struct Recording<M> {
    inner: M,
    dir: PathBuf,
}

impl<M: LanguageModel> Recording<M> {
    /// Records `inner`'s answers in `dir`.
    pub fn new(inner: M, dir: &Path) -> Self {
        Self {
            inner,
            dir: dir.to_owned(),
        }
    }

    /// The live model.
    pub fn inner(&self) -> &M {
        &self.inner
    }
}

impl<M: LanguageModel> LanguageModel for Recording<M> {
    fn model(&self) -> &str {
        self.inner.model()
    }

    async fn complete(&self, request: &LlmRequest) -> Result<LlmReply, LlmError> {
        let reply = self.inner.complete(request).await?;
        let saved = Saved {
            model: reply.model.clone(),
            text: reply.text.clone(),
            request_id: reply.request_id.clone(),
            input_tokens: reply.input_tokens,
            output_tokens: reply.output_tokens,
        };
        // The answer was paid for: a failed save is reported, never allowed
        // to throw it away.
        let saved = serde_json::to_string_pretty(&saved)
            .map_err(std::io::Error::other)
            .and_then(|json| {
                std::fs::create_dir_all(&self.dir)?;
                std::fs::write(path(&self.dir, self.inner.model(), request), format!("{json}\n"))
            });
        if let Err(err) = saved {
            eprintln!("couldn't save a recorded answer in {}: {err}", self.dir.display());
        }
        Ok(reply)
    }
}

/// Replays recorded answers and asks the live model only for requests not
/// yet recorded, recording those; it remembers every request it saw, so
/// recordings no request uses any more can be removed.
pub struct RecordMissing<M> {
    recorded: Recorded,
    live: Recording<M>,
    used: std::sync::Mutex<std::collections::HashSet<String>>,
    live_calls: std::sync::atomic::AtomicUsize,
}

impl<M: LanguageModel> RecordMissing<M> {
    /// `live`'s recordings in `dir`.
    pub fn new(live: M, dir: &Path) -> Self {
        Self {
            recorded: Recorded::new(live.model(), dir),
            live: Recording::new(live, dir),
            used: std::sync::Mutex::new(std::collections::HashSet::new()),
            live_calls: std::sync::atomic::AtomicUsize::new(0),
        }
    }

    /// The fingerprints of every request seen.
    pub fn used(&self) -> Vec<String> {
        self.used
            .lock()
            .map(|used| used.iter().cloned().collect())
            .unwrap_or_default()
    }

    /// How many requests went to the live model.
    pub fn live_calls(&self) -> usize {
        self.live_calls.load(std::sync::atomic::Ordering::SeqCst)
    }

    /// Deletes every recording in the folder that no request seen here used;
    /// returns how many.
    pub fn remove_unused(&self) -> std::io::Result<usize> {
        remove_unused(&self.live.dir, &self.used().into_iter().collect())
    }
}

impl<M: LanguageModel> LanguageModel for RecordMissing<M> {
    fn model(&self) -> &str {
        self.live.model()
    }

    async fn complete(&self, request: &LlmRequest) -> Result<LlmReply, LlmError> {
        if let Ok(mut used) = self.used.lock() {
            used.insert(fingerprint(self.model(), request));
        }
        match self.recorded.complete(request).await {
            Ok(reply) => Ok(reply),
            Err(_) => {
                self.live_calls.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
                self.live.complete(request).await
            }
        }
    }
}

/// Deletes every recording in `dir` whose fingerprint isn't in `used`;
/// returns how many.
pub fn remove_unused(dir: &Path, used: &std::collections::HashSet<String>) -> std::io::Result<usize> {
    let mut removed = 0;
    for entry in std::fs::read_dir(dir)? {
        let path = entry?.path();
        let stem = path.file_stem().and_then(|stem| stem.to_str()).unwrap_or_default();
        if path.extension().is_some_and(|ext| ext == "json") && !used.contains(stem) {
            std::fs::remove_file(&path)?;
            removed += 1;
        }
    }
    Ok(removed)
}
