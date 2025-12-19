/* SPDX Id: AGPL-3.0-or-later */

use dom_struct::dom_struct;
use script_bindings::domstring::DOMString;

use crate::dom::bindings::codegen::Bindings::PairingBinding::PeerMethods;
use crate::dom::eventtarget::EventTarget;

#[dom_struct]
pub(crate) struct Peer {
    eventtarget: EventTarget,
}

impl Peer {
    fn new_inherited() -> Peer {
        Peer {
            eventtarget: EventTarget::new_inherited(),
        }
    }
}

impl PeerMethods<crate::DomTypeHolder> for Peer {
    fn DisplayName(&self) -> DOMString {
        todo!()
    }

    fn SetDisplayName(&self, _name: DOMString) {
        todo!()
    }

    fn Id(&self) -> DOMString {
        todo!()
    }

    event_handler!(peerleft, GetOnpeerleft, SetOnpeerleft);
}
