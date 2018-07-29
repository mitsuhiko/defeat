#[cfg(feature = "backtrace")]
extern crate backtrace as backtrace_support;

mod backtrace;
mod traits;

pub use backtrace::{AddrHint, Backtrace, CapturePurpose, Frame, FrameIter, Symbol, SymbolName};
pub use traits::Error;
