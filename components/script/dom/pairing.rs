/* SPDX Id: AGPL-3.0-or-later */

use std::rc::Rc;

use dom_struct::dom_struct;

use crate::dom::bindings::codegen::Bindings::PairingBinding::PairingMethods;
use crate::dom::eventtarget::EventTarget;
use crate::dom::peer::Peer;
use crate::dom::promise::Promise;

#[dom_struct]
pub(crate) struct Pairing {
    eventtarget: EventTarget,
}

impl Pairing {
    fn new_inherited() -> Pairing {
        Pairing {
            eventtarget: EventTarget::new_inherited(),
        }
    }
}

impl PairingMethods<crate::DomTypeHolder> for Pairing {
    fn Local(&self) -> Rc<Promise> {
        todo!()
    }

    fn Peers(&self) -> Rc<Promise> {
        todo!()
    }

    fn RequestPairing(&self, _peer: &Peer) -> Rc<Promise> {
        todo!()
    }

    event_handler!(peerdiscovered, GetOnpeerdiscovered, SetOnpeerdiscovered);

    event_handler!(peerjoined, GetOnpeerjoined, SetOnpeerjoined);

    event_handler!(peerleft, GetOnpeerleft, SetOnpeerleft);

    event_handler!(pairingrequest, GetOnpairingrequest, SetOnpairingrequest);
}
