/* Copyright (C) 2025 me@webbeef.org
 * This program is free software: you can redistribute it and/or modify
 * it under the terms of the GNU Affero General Public License as
 * published by the Free Software Foundation, version 3.
 * This program is distributed in the hope that it will be useful,
 * but WITHOUT ANY WARRANTY; without even the implied warranty of
 * MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE. See the GNU
 * Affero General Public License for more details.
 * You should have received a copy of the GNU Affero General Public License
 * along with this program. If not, see <https://www.gnu.org/licenses/>. */

use std::rc::Rc;

use constellation_traits::{AtProtoRequest, AtProtoResult, ScriptToConstellationMessage};
use dom_struct::dom_struct;
use js::jsval::UndefinedValue;
use js::rust::HandleObject;
use script_bindings::error::Error;
use script_bindings::interfaces::PincoyaHelpers;
use script_bindings::script_runtime::JSContext;
use script_bindings::str::USVString;

use crate::dom::bindings::codegen::Bindings::AtProtoBinding::{AtProtoMethods, AtProtoSession};
use crate::dom::bindings::reflector::{DomGlobal, Reflector, reflect_dom_object};
use crate::dom::bindings::root::DomRoot;
use crate::dom::globalscope::GlobalScope;
use crate::dom::promise::Promise;
use crate::realms::{AlreadyInRealm, InRealm};
use crate::routed_promise::{RoutedPromiseListener, route_promise};
use crate::script_runtime::CanGc;

#[dom_struct]
pub(crate) struct AtProto {
    reflector_: Reflector,
}

impl AtProto {
    pub fn new_inherited() -> AtProto {
        AtProto {
            reflector_: Reflector::new(),
        }
    }

    pub(crate) fn new(global: &GlobalScope, can_gc: CanGc) -> DomRoot<AtProto> {
        reflect_dom_object(Box::new(AtProto::new_inherited()), global, can_gc)
    }
}

impl AtProto {
    fn request(&self, request: AtProtoRequest, comp: InRealm, can_gc: CanGc) -> Rc<Promise> {
        let global = &self.global();
        let promise = Promise::new_in_current_realm(comp, can_gc);
        let task_source = global.task_manager().dom_manipulation_task_source();
        let sender = route_promise(&promise, self, task_source);

        let script_to_constellation_chan = global.script_to_constellation_chan();
        if script_to_constellation_chan
            .send(ScriptToConstellationMessage::AtProto(request, sender))
            .is_err()
        {
            promise.reject_error(
                Error::Operation(Some("Constellation is dead".to_owned())),
                can_gc,
            );
        }
        promise
    }
}

impl AtProtoMethods<crate::DomTypeHolder> for AtProto {
    /// <https://webbeef.org/atproto>
    fn Login(
        &self,
        handle: USVString,
        password: USVString,
        comp: InRealm,
        can_gc: CanGc,
    ) -> Rc<Promise> {
        self.request(
            AtProtoRequest::Login(handle.to_string(), password.to_string()),
            comp,
            can_gc,
        )
    }

    /// <https://webbeef.org/atproto>
    fn Logout(&self, comp: InRealm, can_gc: CanGc) -> Rc<Promise> {
        self.request(AtProtoRequest::Logout, comp, can_gc)
    }

    /// <https://webbeef.org/atproto>
    fn Current(&self, comp: InRealm, can_gc: CanGc) -> Rc<Promise> {
        self.request(AtProtoRequest::Current, comp, can_gc)
    }
}

impl RoutedPromiseListener<AtProtoResult> for AtProto {
    fn handle_response(&self, response: AtProtoResult, promise: &Rc<Promise>, can_gc: CanGc) {
        match response {
            AtProtoResult::NewSession(session, _) => {
                println!("New session is {session:?}");
                let dom_session = AtProtoSession {
                    did: session.did.into(),
                    handle: session.handle.into(),
                };
                promise.resolve_native(&dom_session, can_gc);
            },
            AtProtoResult::CurrentSession(session) => {
                println!("Current session is {session:?}");
                let dom_session = AtProtoSession {
                    did: session.did.into(),
                    handle: session.handle.into(),
                };
                promise.resolve_native(&dom_session, can_gc);
            },
            AtProtoResult::Logout => promise.resolve_native(&UndefinedValue(), can_gc),
            AtProtoResult::Error => {
                error!("ATProto error :(");
                promise.reject_error(Error::Operation(Some("ATProto error".to_owned())), can_gc)
            },
            AtProtoResult::RefreshRequired => {
                error!("ATProto refresh required :(");
                promise.reject_error(
                    Error::Operation(Some("ATProto refresh required".to_owned())),
                    can_gc,
                )
            },
        }
    }
}

impl PincoyaHelpers for AtProto {
    #[allow(unsafe_code)]
    fn is_pincoya_api(cx: JSContext, _global: HandleObject) -> bool {
        unsafe {
            let in_realm_proof = AlreadyInRealm::assert_for_cx(cx);
            let global_scope = GlobalScope::from_context(*cx, InRealm::Already(&in_realm_proof));
            let url = global_scope.get_url();
            url.scheme() == "pincoya"
        }
    }
}
