//! Frame vocabulary: native texture handles, ownership and sync.

use super::*;

// ── Frame vocabulary ───────────────────────────────────────────────────────

/// Platform-specific texture handle emitted by [`SurfaceProducer::acquire_frame`].
#[non_exhaustive]
pub enum NativeTextureHandle {
    /// Windows: D3D12 shared texture HANDLE cast to u64.
    ///
    /// The host must obey [`FrameHandleOwnership`]. A `Transferred` handle is
    /// closed by the host after `OpenSharedHandle`; a `Borrowed` handle remains
    /// the producer's property and must not be closed by the host. A zero handle
    /// means "reuse the resource already imported for this `resource_epoch`";
    /// it is invalid for a new epoch. Cross-API shared textures use D3D12's
    /// simultaneous-access rules. The host imports them from
    /// `D3D12_RESOURCE_STATE_COMMON` and must finish sampling and return them to
    /// COMMON before it asks the producer for another frame.
    D3d12Shared {
        handle: u64,
        ownership: FrameHandleOwnership,
    },
    /// macOS: IOSurface ref (opaque u64; downcast on the host side).
    IoSurface(u64),
    /// Linux: DMA-BUF fd (negative means absent/invalid).
    DmaBuf(i64),
    /// An owned native frame whose concrete import boundary belongs to the
    /// selected engine and host binding.
    ///
    /// This keeps RAII custody intact across Inker's type-erased boundary.
    /// Use it when import may consume descriptors or platform retains. The
    /// host consumes the payload through [`Self::into_owned_payload`] and
    /// downcasts it to the engine's documented payload type before importing.
    OwnedPayload(Box<dyn OwnedSurfaceFrame>),
}

impl fmt::Debug for NativeTextureHandle {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::D3d12Shared { handle, ownership } => f
                .debug_struct("D3d12Shared")
                .field("handle", handle)
                .field("ownership", ownership)
                .finish(),
            Self::IoSurface(handle) => f.debug_tuple("IoSurface").field(handle).finish(),
            Self::DmaBuf(fd) => f.debug_tuple("DmaBuf").field(fd).finish(),
            Self::OwnedPayload(payload) => f.debug_tuple("OwnedPayload").field(payload).finish(),
        }
    }
}

impl NativeTextureHandle {
    /// Consume an opaque owned frame payload.
    ///
    /// The caller must select the payload type from the surface engine it
    /// instantiated. A mismatch returns the still-owned payload so it can be
    /// dropped safely or handed to the correct importer.
    pub fn into_owned_payload<T: Any>(self) -> Result<T, Self> {
        let Self::OwnedPayload(payload) = self else {
            return Err(self);
        };
        match payload.into_any().downcast::<T>() {
            Ok(payload) => Ok(*payload),
            Err(payload) => Err(Self::OwnedPayload(Box::new(OwnedAnySurfaceFrame(payload)))),
        }
    }
}

/// An owned native-frame payload crossing Inker's wgpu-free surface boundary.
///
/// The payload is deliberately consumed rather than borrowed: importing an
/// external texture can consume descriptors or platform retains. Concrete
/// engine crates expose their payload type and importer pairing.
pub trait OwnedSurfaceFrame: fmt::Debug + 'static {
    /// Stable producer-owned vocabulary for diagnostics before downcasting.
    fn payload_kind(&self) -> &'static str;

    /// Move this payload to its concrete importer without losing custody.
    fn into_any(self: Box<Self>) -> Box<dyn Any>;
}

pub(super) struct OwnedAnySurfaceFrame(Box<dyn Any>);

impl fmt::Debug for OwnedAnySurfaceFrame {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("OwnedAnySurfaceFrame(..)")
    }
}

impl OwnedSurfaceFrame for OwnedAnySurfaceFrame {
    fn payload_kind(&self) -> &'static str {
        "unknown"
    }

    fn into_any(self: Box<Self>) -> Box<dyn Any> {
        self.0
    }
}

/// Who closes a native handle carried in a [`SurfaceFrame`].
///
/// Frame handles cannot all use the same rule: a system-webview producer may
/// retain and reuse its shared handle across paints, while Weld deliberately
/// copies CEF's callback-scoped texture into a fresh application-owned handle
/// and transfers that handle to the host. This is explicit so a generic host
/// cannot either leak the latter or close the former underneath its producer.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum FrameHandleOwnership {
    /// The producer owns the handle for the lifetime of this resource epoch.
    Borrowed,
    /// The host owns this one-shot handle and closes it after importing.
    Transferred,
}

/// Pixel format for a native surface frame.
///
/// Inker deliberately models the small cross-platform texture vocabulary
/// rather than depending on wgpu. Hosts map the known variants to their GPU
/// API and must reject `Other` until they add an explicit import mapping.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum SurfaceTextureFormat {
    Rgba8Unorm,
    Rgba8UnormSrgb,
    Bgra8Unorm,
    Bgra8UnormSrgb,
    Other(String),
}

/// Synchronization handle accompanying a [`SurfaceFrame`].
#[non_exhaustive]
#[derive(Debug)]
pub enum SurfaceSyncHandle {
    /// Windows: D3D12 fence + signal value. The fence HANDLE is borrowed; the
    /// producer must keep it valid until the producer is dropped. A host may
    /// open and retain its own COM reference instead of reopening it per frame.
    D3d12Fence { handle: u64, value: u64 },
    /// Synchronization already complete before the handle was emitted.
    None,
}

/// A composited frame from a surface producer.
///
/// `texture` is a raw platform handle, not a `wgpu::Texture` — the host imports it
/// on its own device, which is what keeps inker wgpu-free. Because importing a
/// shared handle every frame is wasteful (and some producers, e.g. WebView2, reuse
/// one allocation and overwrite it in place), [`resource_epoch`](Self::resource_epoch)
/// lets the host import once and re-sample.
#[derive(Debug)]
pub struct SurfaceFrame {
    pub texture: NativeTextureHandle,
    pub sync: SurfaceSyncHandle,
    pub width: u32,
    pub height: u32,
    /// Exact pixel format of `texture`. The host must use this value when it
    /// creates or imports the GPU resource; assuming BGRA silently corrupts
    /// RGBA CEF frames.
    pub format: SurfaceTextureFormat,
    /// Monotonic generation of the underlying GPU allocation. Bumps when the
    /// producer (re)allocates the shared resource (first frame, resize, realloc);
    /// stays constant while it overwrites the same allocation in place. The host's
    /// import cache keys on this: re-import (releasing the previous handle) when it
    /// changes, re-sample the already-imported texture when it doesn't. This is the
    /// import-once signal a type-erased producer would otherwise lose (the reason
    /// scrying once held its concrete producer outside the registry).
    pub resource_epoch: u64,
}
