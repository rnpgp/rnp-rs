//! [`Recipient`] — public-key recipient extracted from a [`super::VerifyResult`].

use crate::error::Result;
use crate::ffi;
use crate::ffi_safe::call_for_string;

/// Public-key recipient extracted from a [`super::VerifyResult`].
pub struct Recipient {
    pub(crate) handle: ffi::rnp_recipient_handle_t,
}

impl Recipient {
    pub(crate) fn new(handle: ffi::rnp_recipient_handle_t) -> Self {
        Recipient { handle }
    }

    /// Encryption algorithm used for this recipient's session key.
    pub fn alg(&self) -> Result<String> {
        call_for_string(|raw| unsafe { ffi::rnp_recipient_get_alg(self.handle, raw) })
    }

    /// Hex keyid of the recipient's key.
    pub fn keyid(&self) -> Result<String> {
        call_for_string(|raw| unsafe { ffi::rnp_recipient_get_keyid(self.handle, raw) })
    }
}
