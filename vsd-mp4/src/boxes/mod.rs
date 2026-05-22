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

#[cfg(feature = "sub-vtt")]
mod mdhd;

#[cfg(feature = "sub-vtt")]
#[cfg_attr(docsrs, doc(cfg(feature = "sub-vtt")))]
pub use mdhd::MdhdBox;

#[cfg(feature = "sub-vtt")]
mod tfdt;

#[cfg(feature = "sub-vtt")]
#[cfg_attr(docsrs, doc(cfg(feature = "sub-vtt")))]
pub use tfdt::TfdtBox;

#[cfg(any(feature = "decrypt-cenc", feature = "sub-vtt"))]
mod tfhd;

#[cfg(any(feature = "decrypt-cenc", feature = "sub-vtt"))]
#[cfg_attr(docsrs, doc(cfg(any(feature = "decrypt-cenc", feature = "sub-vtt"))))]
pub use tfhd::TfhdBox;

#[cfg(any(feature = "decrypt-cenc", feature = "sub-vtt"))]
mod trun;

#[cfg(any(feature = "decrypt-cenc", feature = "sub-vtt"))]
#[cfg_attr(docsrs, doc(cfg(any(feature = "decrypt-cenc", feature = "sub-vtt"))))]
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
