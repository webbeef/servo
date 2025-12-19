/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

use std::rc::Rc;

use constellation_traits::ScriptToConstellationMessage;
use dom_struct::dom_struct;
use js::rust::HandleObject;
use profile_traits::mem::MemoryReportResult;
use script_bindings::error::{Error, Fallible};
use script_bindings::interfaces::{EmbedderHelpers, ServoInternalsHelpers};
use script_bindings::script_runtime::JSContext;
use script_bindings::str::USVString;
use servo_config::embedder_prefs;
use servo_config::prefs::{self, PrefValue};

use crate::dom::bindings::codegen::Bindings::ServoInternalsBinding::ServoInternalsMethods;
use crate::dom::bindings::reflector::{DomGlobal, Reflector, reflect_dom_object};
use crate::dom::bindings::root::DomRoot;
use crate::dom::embedder::Embedder;
use crate::dom::globalscope::GlobalScope;
use crate::dom::promise::Promise;
use crate::realms::{AlreadyInRealm, InRealm};
use crate::routed_promise::{RoutedPromiseListener, callback_promise};
use crate::script_runtime::CanGc;
use crate::script_thread::ScriptThread;

#[dom_struct]
pub(crate) struct ServoInternals {
    reflector_: Reflector,
}

impl ServoInternals {
    pub fn new_inherited() -> ServoInternals {
        ServoInternals {
            reflector_: Reflector::new(),
        }
    }

    pub(crate) fn new(global: &GlobalScope, can_gc: CanGc) -> DomRoot<ServoInternals> {
        reflect_dom_object(Box::new(ServoInternals::new_inherited()), global, can_gc)
    }
}

impl ServoInternalsMethods<crate::DomTypeHolder> for ServoInternals {
    /// <https://servo.org/internal-no-spec>
    fn ReportMemory(&self, comp: InRealm, can_gc: CanGc) -> Rc<Promise> {
        let global = &self.global();
        let promise = Promise::new_in_current_realm(comp, can_gc);
        let task_source = global.task_manager().dom_manipulation_task_source();
        let callback = callback_promise(&promise, self, task_source);

        let script_to_constellation_chan = global.script_to_constellation_chan();
        if script_to_constellation_chan
            .send(ScriptToConstellationMessage::ReportMemory(callback))
            .is_err()
        {
            promise.reject_error(Error::Operation(None), can_gc);
        }
        promise
    }

    /// <https://servo.org/internal-no-spec>
    fn GetBoolPreference(&self, name: USVString) -> Fallible<bool> {
        // Check if this is an embedder preference (contains a namespace separator)
        if name.contains('.') {
            // Look up in embedder preferences registry
            if let Some(PrefValue::Bool(b)) = prefs::get_embedder_pref(&name) {
                return Ok(b);
            }
        } else {
            // Core Servo preference
            if let PrefValue::Bool(b) = prefs::get().get_value(&name) {
                return Ok(b);
            }
        }
        Err(Error::TypeMismatch(None))
    }

    /// <https://servo.org/internal-no-spec>
    fn GetIntPreference(&self, name: USVString) -> Fallible<i64> {
        // Check if this is an embedder preference (contains a namespace separator)
        if name.contains('.') {
            // Look up in embedder preferences registry
            if let Some(PrefValue::Int(i)) = prefs::get_embedder_pref(&name) {
                return Ok(i);
            }
        } else {
            // Core Servo preference
            if let PrefValue::Int(i) = prefs::get().get_value(&name) {
                return Ok(i);
            }
        }
        Err(Error::TypeMismatch(None))
    }

    /// <https://servo.org/internal-no-spec>
    fn GetStringPreference(&self, name: USVString) -> Fallible<USVString> {
        // Check if this is an embedder preference (contains a namespace separator)
        if name.contains('.') {
            // Look up in embedder preferences registry
            if let Some(PrefValue::Str(s)) = prefs::get_embedder_pref(&name) {
                return Ok(s.into());
            }
        } else {
            // Core Servo preference
            if let PrefValue::Str(s) = prefs::get().get_value(&name) {
                return Ok(s.into());
            }
        }
        Err(Error::TypeMismatch(None))
    }

    /// <https://servo.org/internal-no-spec>
    fn SetBoolPreference(&self, name: USVString, value: bool) {
        let pref_value: PrefValue = value.into();
        // Check if this is an embedder preference (contains a namespace separator)
        if name.contains('.') {
            // Use the embedder prefs setter which invokes callbacks and notifies observers
            embedder_prefs::set_embedder_pref_from_script(&name, pref_value.clone());
        } else {
            // Core Servo preference
            let mut current_prefs = prefs::get().clone();
            current_prefs.set_value(&name, pref_value.clone());
            prefs::set(current_prefs);
        }
        // Broadcast preference change to all script threads via the Constellation
        let _ = self.global().script_to_constellation_chan().send(
            ScriptToConstellationMessage::BroadcastPreferenceChange(name.to_string(), pref_value),
        );
    }

    /// <https://servo.org/internal-no-spec>
    fn SetIntPreference(&self, name: USVString, value: i64) {
        let pref_value: PrefValue = value.into();
        // Check if this is an embedder preference (contains a namespace separator)
        if name.contains('.') {
            // Use the embedder prefs setter which invokes callbacks and notifies observers
            embedder_prefs::set_embedder_pref_from_script(&name, pref_value.clone());
        } else {
            // Core Servo preference
            let mut current_prefs = prefs::get().clone();
            current_prefs.set_value(&name, pref_value.clone());
            prefs::set(current_prefs);
        }
        // Broadcast preference change to all script threads via the Constellation
        let _ = self.global().script_to_constellation_chan().send(
            ScriptToConstellationMessage::BroadcastPreferenceChange(name.to_string(), pref_value),
        );
    }

    /// <https://servo.org/internal-no-spec>
    fn SetStringPreference(&self, name: USVString, value: USVString) {
        let pref_value: PrefValue = value.0.into();
        // Check if this is an embedder preference (contains a namespace separator)
        if name.contains('.') {
            // Use the embedder prefs setter which invokes callbacks and notifies observers
            embedder_prefs::set_embedder_pref_from_script(&name, pref_value.clone());
        } else {
            // Core Servo preference
            let mut current_prefs = prefs::get().clone();
            current_prefs.set_value(&name, pref_value.clone());
            prefs::set(current_prefs);
        }
        // Broadcast preference change to all script threads via the Constellation
        let _ = self.global().script_to_constellation_chan().send(
            ScriptToConstellationMessage::BroadcastPreferenceChange(name.to_string(), pref_value),
        );
    }
}

impl RoutedPromiseListener<MemoryReportResult> for ServoInternals {
    fn handle_response(&self, response: MemoryReportResult, promise: &Rc<Promise>, can_gc: CanGc) {
        let stringified = serde_json::to_string(&response.results)
            .unwrap_or_else(|_| "{ error: \"failed to create memory report\"}".to_owned());
        promise.resolve_native(&stringified, can_gc);
    }
}

impl ServoInternalsHelpers for ServoInternals {
    /// The navigator.servo api is exposed to about: pages except about:blank, as
    /// well as any URLs provided by embedders that register new protocol handlers.
    #[expect(unsafe_code)]
    fn is_servo_internal(cx: JSContext, global: HandleObject) -> bool {
        if Embedder::is_allowed_to_embed(cx, global) {
            return true;
        }
        unsafe {
            let in_realm_proof = AlreadyInRealm::assert_for_cx(cx);
            let global_scope = GlobalScope::from_context(*cx, InRealm::Already(&in_realm_proof));
            let url = global_scope.get_url();
            (url.scheme() == "about" && url.as_str() != "about:blank") ||
                ScriptThread::is_servo_privileged(url)
        }
    }
}
