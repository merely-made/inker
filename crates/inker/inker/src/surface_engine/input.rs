//! Input vocabulary: pointer, mouse, keyboard, drag and focus.

use super::*;

// ── Input vocabulary ───────────────────────────────────────────────────────

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct PhysicalPosition {
    pub x: f32,
    pub y: f32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum MouseButton {
    Left,
    Middle,
    Right,
    Back,
    Forward,
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub enum MouseEventKind {
    Moved,
    Pressed,
    Released,
    ScrollPixels { delta_x: f32, delta_y: f32 },
    ScrollLines { delta_x: f32, delta_y: f32 },
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct MouseEvent {
    pub position: PhysicalPosition,
    pub button: Option<MouseButton>,
    pub kind: MouseEventKind,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct KeyboardModifiers {
    pub shift: bool,
    pub ctrl: bool,
    pub alt: bool,
    pub meta: bool,
}

/// A wheel gesture with its actual modifier state. Deltas and position are physical pixels.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct SurfaceWheelEvent {
    pub position: PhysicalPosition,
    pub delta: (f32, f32),
    pub modifiers: KeyboardModifiers,
}

/// The Pointer Events `pointerType` values. `Unknown` represents the empty
/// string or a device kind the host cannot identify; it must not be guessed.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum PointerType {
    Mouse,
    Pen,
    Touch,
    Unknown,
}

/// Native contact phases from which the engine fires DOM Pointer Events.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum PointerPhase {
    Down,
    Move,
    Up,
    Cancel,
}

/// Pointer Events `buttons` bitmask. Values follow the DOM allocation rather
/// than a backend's native flag values.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct PointerButtons(pub u16);

impl PointerButtons {
    pub const NONE: Self = Self(0);
    pub const PRIMARY: Self = Self(1);
    pub const SECONDARY: Self = Self(2);
    pub const AUXILIARY: Self = Self(4);
    pub const BACK: Self = Self(8);
    pub const FORWARD: Self = Self(16);

    pub fn contains(self, other: Self) -> bool {
        self.0 & other.0 == other.0
    }
}

impl std::ops::BitOr for PointerButtons {
    type Output = Self;

    fn bitor(self, rhs: Self) -> Self::Output {
        Self(self.0 | rhs.0)
    }
}

/// Hardware-neutral input following the W3C Pointer Events fields. Optional
/// sensor values distinguish "not reported by the host" from a real zero;
/// the engine applies the specification's DOM defaults when it dispatches.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct PointerEvent {
    pub pointer_id: i32,
    pub pointer_type: PointerType,
    pub is_primary: bool,
    pub phase: PointerPhase,
    pub position: PhysicalPosition,
    /// The button whose state changed for Down/Up, if any.
    pub button: Option<MouseButton>,
    /// All buttons held after this native event.
    pub buttons: PointerButtons,
    /// Contact geometry in CSS-pixel shape, expressed here in physical pixels
    /// at the host boundary. Engines perform their normal device-scale mapping.
    pub width: f32,
    pub height: f32,
    pub pressure: Option<f32>,
    pub tangential_pressure: Option<f32>,
    /// Degrees in the Pointer Events `-90..=90` convention.
    pub tilt_x: Option<f32>,
    pub tilt_y: Option<f32>,
    /// Clockwise degrees in `0..=359`.
    pub twist: Option<f32>,
    /// Radians in the Pointer Events conventions.
    pub altitude_angle: Option<f32>,
    pub azimuth_angle: Option<f32>,
    pub modifiers: KeyboardModifiers,
}

/// Standard drag effects shared by HTML `DataTransfer.effectAllowed` and the
/// host toolkit. Backend-only effects do not enter this mask.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct DragOperationSet(pub u8);

impl DragOperationSet {
    pub const NONE: Self = Self(0);
    pub const COPY: Self = Self(1);
    pub const LINK: Self = Self(2);
    pub const MOVE: Self = Self(4);

    pub fn contains(self, other: Self) -> bool {
        self.0 & other.0 == other.0
    }
}

impl std::ops::BitOr for DragOperationSet {
    type Output = Self;

    fn bitor(self, rhs: Self) -> Self::Output {
        Self(self.0 | rhs.0)
    }
}

/// One HTML `DataTransferItem`. The enum makes the standard `kind` value
/// (`string` or `file`) structural, while `mime_type` carries its lowercase
/// type string. A file path is host-private transport data, not page-visible
/// identity.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum DataTransferItem {
    String {
        mime_type: String,
        data: String,
    },
    File {
        mime_type: String,
        path: std::path::PathBuf,
        display_name: Option<String>,
    },
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct DataTransfer {
    pub items: Vec<DataTransferItem>,
    pub allowed_operations: DragOperationSet,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum DragPhase {
    Enter,
    Over,
    Leave,
    Drop,
}

/// One host-to-page HTML drag lifecycle event. HTML exposes one `DataTransfer`
/// throughout the drag; engines may only materialize its native payload on
/// Enter, but the allowed effects remain available for every phase.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct DragEvent {
    pub phase: DragPhase,
    pub position: PhysicalPosition,
    pub modifiers: KeyboardModifiers,
    /// DOM `MouseEvent.buttons` bitfield inherited by `DragEvent`.
    pub buttons: PointerButtons,
    pub data_transfer: DataTransfer,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct KeyboardEvent {
    /// Host-framework virtual key code (gpui key code on the main path).
    pub key_code: u32,
    /// Hardware scan code; zero when absent.
    pub scan_code: u32,
    pub modifiers: KeyboardModifiers,
    pub pressed: bool,
    /// Composed text for printable keys; `None` for non-printable and key-up.
    pub text: Option<String>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum FocusReason {
    Mouse,
    Tab,
    ShiftTab,
    Programmatic,
}
