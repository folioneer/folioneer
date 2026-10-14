//! Global event bus for cross-context communication.

mod bus;
mod event;

pub use bus::{EventReceiver, SideEffectEventBus};
pub use event::{Event, PriceMovementReport, PriceMovementRow, UnpricedAsset};
