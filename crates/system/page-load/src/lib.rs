// Copyright 2026 Mark Alan Boykin
// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
// SPDX-License-Identifier: MPL-2.0

//! Mere's fetch vocabulary and the sans-IO page load.
//!
//! The vocabulary (request identity, fetched bytes, typed failures, streamed
//! fragments) is what the `mere-fetch` actor speaks; `mere-fetch` re-exports
//! it unchanged. It lives here, without a transport, so a host-neutral
//! controller such as `pelt-core` can speak it without linking netfetcher or a
//! runtime.
//!
//! [`PageLoad`] is one document's load: which request is current, what phase
//! the transfer is in, the exact bytes received so far, and the retained
//! response. It performs no I/O and reads no clock. A host begins a request,
//! sends the matching command to whatever transport it owns, and feeds the
//! answers back; the load gates every answer on its exact request, so a
//! superseded or stopped transfer can never surface. It was lifted from
//! Turnstone's per-node content state (SC, 2026-10-07).

use std::sync::atomic::{AtomicU64, Ordering};

/// Process-local correlation for one page request. The id crosses the actor
/// boundary unchanged so hosts can cancel one node's load without affecting a
/// second node fetching the same address.
pub type FetchRequestId = u64;

static NEXT_FETCH_REQUEST: AtomicU64 = AtomicU64::new(1);

pub fn next_fetch_request_id() -> FetchRequestId {
    NEXT_FETCH_REQUEST.fetch_add(1, Ordering::Relaxed)
}

/// Successfully fetched content. Rendering uses the decoded text while hosts
/// that retain or download a response use the original bytes.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Fetched {
    pub content_type: Option<String>,
    pub content_disposition: Option<String>,
    pub bytes: Vec<u8>,
    pub body: String,
}

impl Fetched {
    /// Build a text fixture while keeping the byte and decoded views coherent.
    pub fn text(content_type: Option<String>, body: impl Into<String>) -> Self {
        let body = body.into();
        let bytes = body.as_bytes().to_vec();
        Self {
            content_type,
            content_disposition: None,
            bytes,
            body,
        }
    }
}

/// A page request that needs host participation rather than being reducible
/// to a terminal error string. The fetch actor preserves these arms so a UI
/// host can continue the protocol conversation without parsing prose.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum FetchFailure {
    /// The host explicitly stopped this request.
    Cancelled,
    /// A Gemini-style input response. `url` is the final request address after
    /// redirects and is therefore the address the submitted query belongs to.
    InputRequired {
        url: String,
        prompt: String,
        sensitive: bool,
    },
    /// The server requires a client certificate. Identity selection remains
    /// a host decision; carrying the target keeps that later conversation
    /// typed instead of collapsing it into an ordinary transport failure.
    ClientCertificateRequired {
        url: String,
        prompt: String,
        code: Option<u8>,
    },
    /// A Gemini capsule presented a certificate that differs from its durable
    /// pin. The request was not sent; the host must ask a human before replacing
    /// `pinned` with `seen` and retrying `url`.
    CertificateChanged {
        url: String,
        target: String,
        pinned: String,
        seen: String,
    },
    /// A terminal transport, protocol, HTTP, or size-limit failure.
    Failed(String),
}

impl std::fmt::Display for FetchFailure {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Cancelled => f.write_str("cancelled"),
            Self::InputRequired { prompt, .. } => write!(f, "input required: {prompt}"),
            Self::ClientCertificateRequired { .. } => f.write_str("client certificate required"),
            Self::CertificateChanged { target, .. } => {
                write!(f, "certificate for {target} changed")
            },
            Self::Failed(error) => f.write_str(error),
        }
    }
}

/// The result of one fetch, tagged with the requested URL so the host routes it
/// back to the right node's content slot.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FetchOutcome {
    pub request: FetchRequestId,
    pub url: String,
    pub result: Result<Fetched, FetchFailure>,
}

/// One exact page-body fragment observed before the terminal fetch outcome.
/// Only Gemini currently exposes transport reads incrementally; the final
/// [`FetchOutcome`] remains authoritative and retains the complete bytes.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PageProgress {
    /// Exact actor request this fragment belongs to.
    pub request: FetchRequestId,
    /// Original address used to correlate the actor request.
    pub url: String,
    /// Address of the successful response after any redirects.
    pub response_url: String,
    pub content_type: Option<String>,
    pub bytes: Vec<u8>,
}

/// Explicit attachments and response types a document engine cannot render
/// become downloads. A missing media type keeps the render attempt.
pub fn is_download_response(fetched: &Fetched) -> bool {
    let attachment = fetched.content_disposition.as_deref().is_some_and(|value| {
        value
            .split(';')
            .next()
            .is_some_and(|kind| kind.trim().eq_ignore_ascii_case("attachment"))
    });
    attachment
        || fetched.content_type.as_deref().is_some_and(|value| {
            let media = media_type(value);
            !(media.starts_with("text/")
                || media == "application/xml"
                || media == "application/xhtml+xml"
                || media.ends_with("+xml")
                || media == "application/gopher-menu")
        })
}

/// Whether a streamed prefix may be shown before the transfer settles. Only
/// text renders from a prefix; a missing media type is Gemini's default.
pub fn is_streamable(content_type: Option<&str>) -> bool {
    media_type(content_type.unwrap_or("text/gemini")).starts_with("text/")
}

fn media_type(value: &str) -> String {
    value
        .split(';')
        .next()
        .unwrap_or_default()
        .trim()
        .to_ascii_lowercase()
}

/// One page body the host's transport fetched, ready to hand to a document
/// engine. A live session never issues a second network request for bytes the
/// host already owns.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LoadedDocument {
    /// Exact acquired response bytes. The decoded body is only for rendering;
    /// source capture always deposits these bytes.
    pub bytes: Vec<u8>,
    pub content_type: Option<String>,
    pub body: String,
    /// The response address the transport reported, after redirects. Absent
    /// when only the request address is known.
    pub effective_url: Option<String>,
    /// Milliseconds since the Unix epoch when the host admitted these bytes.
    pub acquired_at_ms: u64,
}

/// Network progress, kept even after the first prefix has become a live
/// document session.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum LoadPhase {
    Requested,
    Streaming {
        response_url: String,
        content_type: Option<String>,
        received_bytes: usize,
    },
    Settled {
        received_bytes: usize,
    },
    /// A hosted surface reports normalized load progress rather than bytes.
    Loading {
        progress_millis: Option<u16>,
    },
    /// The human stopped the active transfer before it settled.
    Stopped {
        received_bytes: usize,
    },
}

impl LoadPhase {
    fn received_bytes(&self) -> usize {
        match self {
            Self::Streaming { received_bytes, .. }
            | Self::Settled { received_bytes }
            | Self::Stopped { received_bytes } => *received_bytes,
            Self::Requested | Self::Loading { .. } => 0,
        }
    }
}

/// What one terminal answer did to a [`PageLoad`].
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum LoadAnswer {
    /// The answer belonged to a superseded or stopped request and changed
    /// nothing.
    Stale,
    /// The response is retained as the load's document for its address.
    Document,
    /// The response is a download. It is handed back, not retained.
    Download(Fetched),
    /// The request failed. Conversation arms (input, identity, certificate
    /// change) stay typed for the host.
    Failed(FetchFailure),
}

/// One document's load. See the crate documentation.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct PageLoad {
    active: Option<FetchRequestId>,
    phase: Option<LoadPhase>,
    document: Option<(String, LoadedDocument)>,
    stream: Option<(String, Vec<u8>)>,
}

impl PageLoad {
    /// Begin one transport request and return the exact older request it
    /// supersedes, which the host cancels.
    pub fn begin(&mut self, request: FetchRequestId) -> Option<FetchRequestId> {
        self.stream = None;
        self.phase = Some(LoadPhase::Requested);
        self.active.replace(request)
    }

    pub fn active_request(&self) -> Option<FetchRequestId> {
        self.active
    }

    pub fn is_active(&self, request: FetchRequestId) -> bool {
        self.active == Some(request)
    }

    /// Retire `request` if it is the current one. Every answer passes this
    /// gate, so a stale answer reports `false` and changes nothing.
    pub fn finish(&mut self, request: FetchRequestId) -> bool {
        if !self.is_active(request) {
            return false;
        }
        self.active = None;
        true
    }

    /// Retire `request` and mark the transfer settled at the bytes received.
    pub fn settle(&mut self, request: FetchRequestId) -> bool {
        if !self.finish(request) {
            return false;
        }
        let received_bytes = self.received_bytes();
        self.phase = Some(LoadPhase::Settled { received_bytes });
        true
    }

    /// Retire `request` because the human stopped it. A streamed prefix
    /// already retained as the document stays; the partial stream does not.
    pub fn stop(&mut self, request: FetchRequestId) -> bool {
        if !self.finish(request) {
            return false;
        }
        let received_bytes = self.received_bytes();
        self.stream = None;
        self.phase = Some(LoadPhase::Stopped { received_bytes });
        true
    }

    /// Stop whatever request is current and return it for the host to cancel.
    pub fn stop_active(&mut self) -> Option<FetchRequestId> {
        let request = self.active?;
        self.stop(request).then_some(request)
    }

    pub fn surface_started(&mut self) {
        self.phase = Some(LoadPhase::Loading {
            progress_millis: None,
        });
    }

    pub fn surface_progress(&mut self, value: f32) {
        let progress_millis = (value.clamp(0.0, 1.0) * 1000.0).round() as u16;
        self.phase = Some(LoadPhase::Loading {
            progress_millis: Some(progress_millis),
        });
    }

    pub fn surface_settled(&mut self) {
        self.phase = Some(LoadPhase::Settled { received_bytes: 0 });
    }

    pub fn surface_stopped(&mut self) {
        self.phase = Some(LoadPhase::Stopped { received_bytes: 0 });
    }

    /// Retain a response under its request address. The address guard makes a
    /// late or superseded body unusable for a later navigation.
    pub fn note_fetched(&mut self, url: String, document: LoadedDocument, received_bytes: usize) {
        self.document = Some((url, document));
        self.stream = None;
        self.phase = Some(LoadPhase::Settled { received_bytes });
    }

    /// Append one exact transport fragment and refresh the decoded document
    /// from the whole prefix, so split UTF-8 code points cannot corrupt later
    /// replacement frames. Returns the bytes received so far.
    pub fn note_streamed(
        &mut self,
        url: String,
        response_url: String,
        content_type: Option<String>,
        chunk: &[u8],
        now_ms: u64,
    ) -> usize {
        let stream = self.stream.get_or_insert_with(|| (url.clone(), Vec::new()));
        if stream.0 != url {
            *stream = (url.clone(), Vec::new());
        }
        stream.1.extend_from_slice(chunk);
        let bytes = stream.1.clone();
        let received_bytes = bytes.len();
        let acquired_at_ms = self.acquired_at(&url).unwrap_or(now_ms);
        self.document = Some((
            url,
            LoadedDocument {
                body: String::from_utf8_lossy(&bytes).into_owned(),
                bytes,
                content_type: content_type.clone(),
                effective_url: Some(response_url.clone()),
                acquired_at_ms,
            },
        ));
        self.phase = Some(LoadPhase::Streaming {
            response_url,
            content_type,
            received_bytes,
        });
        received_bytes
    }

    /// Take one streamed fragment if it belongs to the current request and
    /// can render from a prefix. Returns the bytes received so far.
    pub fn accept_progress(&mut self, progress: PageProgress, now_ms: u64) -> Option<usize> {
        if !self.is_active(progress.request) || !is_streamable(progress.content_type.as_deref()) {
            return None;
        }
        Some(self.note_streamed(
            progress.url,
            progress.response_url,
            progress.content_type,
            &progress.bytes,
            now_ms,
        ))
    }

    /// Take one terminal answer. A response keeps what streaming already
    /// learned about its address (the effective URL, the media type, the time
    /// it was first admitted) and retains the complete bytes.
    pub fn accept_outcome(&mut self, outcome: FetchOutcome, now_ms: u64) -> LoadAnswer {
        if !self.finish(outcome.request) {
            return LoadAnswer::Stale;
        }
        match outcome.result {
            Ok(fetched) if is_download_response(&fetched) => {
                self.stream = None;
                self.phase = Some(LoadPhase::Settled {
                    received_bytes: fetched.bytes.len(),
                });
                LoadAnswer::Download(fetched)
            },
            Ok(fetched) => {
                let observed = self.fetched(&outcome.url);
                let effective_url = observed.and_then(|previous| previous.effective_url.clone());
                let content_type = fetched
                    .content_type
                    .or_else(|| observed.and_then(|previous| previous.content_type.clone()));
                let acquired_at_ms = self.acquired_at(&outcome.url).unwrap_or(now_ms);
                let received_bytes = fetched.bytes.len();
                self.note_fetched(
                    outcome.url,
                    LoadedDocument {
                        bytes: fetched.bytes,
                        content_type,
                        body: fetched.body,
                        effective_url,
                        acquired_at_ms,
                    },
                    received_bytes,
                );
                LoadAnswer::Document
            },
            Err(failure) => {
                self.fail();
                LoadAnswer::Failed(failure)
            },
        }
    }

    pub fn phase(&self) -> Option<&LoadPhase> {
        self.phase.as_ref()
    }

    pub fn in_progress(&self) -> bool {
        self.active.is_some() || matches!(self.phase, Some(LoadPhase::Loading { .. }))
    }

    /// The retained response for exactly `url`.
    pub fn fetched(&self, url: &str) -> Option<&LoadedDocument> {
        self.document
            .as_ref()
            .and_then(|(owner, document)| (owner == url).then_some(document))
    }

    /// The retained response and the address it belongs to.
    pub fn document(&self) -> Option<(&str, &LoadedDocument)> {
        self.document
            .as_ref()
            .map(|(owner, document)| (owner.as_str(), document))
    }

    /// Drop the retained response and transfer state, keeping any current
    /// request so its answer still lands (a reload refetches the same page).
    pub fn forget_fetched(&mut self) {
        self.document = None;
        self.stream = None;
        self.phase = None;
    }

    /// A failed load keeps its last document but no transfer.
    pub fn fail(&mut self) {
        self.stream = None;
        self.phase = None;
        self.active = None;
    }

    /// Forget everything, including the current request.
    pub fn clear(&mut self) {
        *self = Self::default();
    }

    fn received_bytes(&self) -> usize {
        self.phase.as_ref().map_or(0, LoadPhase::received_bytes)
    }

    fn acquired_at(&self, url: &str) -> Option<u64> {
        self.fetched(url)
            .map(|document| document.acquired_at_ms)
            .filter(|observed_at| *observed_at != 0)
    }
}

#[cfg(test)]
mod tests;
