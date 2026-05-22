//! MP4 box structures and parsers.

#[cfg(feature = "decrypt-cenc")]
mod schm;

#[cfg(feature = "decrypt-cenc")]
#[cfg_attr(docsrs, doc(cfg(feature = "decrypt-cenc")))]
pub use schm::SchmBox;

#[cfg(feature = "decrypt-cenc")]
mod senc;

#[cfg(feature = "decrypt-cenc")]
#[cfg_attr(docsrs, doc(cfg(feature = "decrypt-cenc")))]
pub use senc::{SencBox, SencSample, SencSubsample};

#[cfg(feature = "decrypt-cenc")]
mod tenc;

#[cfg(feature = "decrypt-cenc")]
#[cfg_attr(docsrs, doc(cfg(feature = "decrypt-cenc")))]
pub use tenc::TencBox;

#[cfg(feature = "sidx")]
mod sidx;

#[cfg(feature = "sidx")]
#[cfg_attr(docsrs, doc(cfg(feature = "sidx")))]
pub use sidx::{SidxBox, SidxRange};

#[cfg(feature = "text-vtt")]
mod mdhd;

#[cfg(feature = "text-vtt")]
#[cfg_attr(docsrs, doc(cfg(feature = "text-vtt")))]
pub use mdhd::MdhdBox;

#[cfg(feature = "text-vtt")]
mod tfdt;

#[cfg(feature = "text-vtt")]
#[cfg_attr(docsrs, doc(cfg(feature = "text-vtt")))]
pub use tfdt::TfdtBox;

#[cfg(any(feature = "decrypt-cenc", feature = "text-vtt"))]
mod tfhd;

#[cfg(any(feature = "decrypt-cenc", feature = "text-vtt"))]
#[cfg_attr(docsrs, doc(cfg(any(feature = "decrypt-cenc", feature = "text-vtt"))))]
pub use tfhd::TfhdBox;

#[cfg(any(feature = "decrypt-cenc", feature = "text-vtt"))]
mod trun;

#[cfg(any(feature = "decrypt-cenc", feature = "text-vtt"))]
#[cfg_attr(docsrs, doc(cfg(any(feature = "decrypt-cenc", feature = "text-vtt"))))]
pub use trun::{TrunBox, TrunSample};

/// Helper macro to create a reference-counted, interior-mutable `Option` cell.
///
/// This macro is widely used in custom MP4 parsing chains to collect parsed boxes in parser closures.
#[macro_export]
macro_rules! data {
    () => {
        std::rc::Rc::new(std::cell::RefCell::new(None))
    };
    ($val:expr) => {
        std::rc::Rc::new(std::cell::RefCell::new($val))
    };
}
