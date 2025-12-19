/* SPDX Id: AGPL-3.0-or-later */

//! The `Embedder` interface provides communication between web content and the embedder.
//! This is a Servo-specific, non-standard API.

use std::ptr;

use base::id::ScriptEventLoopId;
use constellation_traits::ScriptToConstellationMessage;
use dom_struct::dom_struct;
use embedder_traits::{EmbedderMsg, NewOSWindowParams, ServoErrorType};
use js::jsapi::{JS_NewObject, JSPROP_ENUMERATE};
use js::jsval::{BooleanValue, Int32Value, ObjectValue, UndefinedValue};
use js::rust::HandleObject;
use js::rust::wrappers::JS_DefineProperty;
use script_bindings::conversions::SafeToJSValConvertible;
use script_bindings::interfaces::EmbedderHelpers;
use script_bindings::script_runtime::JSContext;
use script_bindings::str::USVString;
use servo_config::pref_util::PrefValue;
use servo_url::ServoUrl;

use crate::dom::bindings::codegen::Bindings::CustomEventBinding::CustomEventMethods;
use crate::dom::bindings::codegen::Bindings::EmbedderBinding::EmbedderMethods;
use crate::dom::bindings::inheritance::Castable;
use crate::dom::bindings::reflector::{DomGlobal, reflect_dom_object};
use crate::dom::bindings::root::{Dom, DomRoot, Root};
use crate::dom::bindings::str::DOMString;
use crate::dom::customevent::CustomEvent;
use crate::dom::event::Event;
use crate::dom::eventtarget::EventTarget;
use crate::dom::globalscope::GlobalScope;
use crate::realms::{AlreadyInRealm, InRealm};
use crate::script_runtime::CanGc;

#[dom_struct]
pub(crate) struct Embedder {
    eventtarget: EventTarget,
}

impl Embedder {
    fn new_inherited() -> Embedder {
        Embedder {
            eventtarget: EventTarget::new_inherited(),
        }
    }

    pub(crate) fn new(global: &GlobalScope, can_gc: CanGc) -> DomRoot<Embedder> {
        // Register this script thread as having an embedder error listener.
        // The Constellation tracks which script threads have listeners to avoid
        // broadcasting error events to threads that don't have any navigator.embedder.
        if let Some(event_loop_id) = ScriptEventLoopId::installed() {
            let _ = global.script_to_constellation_chan().send(
                ScriptToConstellationMessage::RegisterEmbedderErrorListener(event_loop_id),
            );
        }

        reflect_dom_object(Box::new(Embedder::new_inherited()), global, can_gc)
    }

    /// Dispatch a servoerror event with the given error type and message.
    #[expect(unsafe_code)]
    pub(crate) fn dispatch_servo_error(
        &self,
        error_type: &ServoErrorType,
        message: &str,
        can_gc: CanGc,
    ) {
        let cx = GlobalScope::get_cx();

        // Create the detail object: { errorType: string, message: string }
        rooted!(in(*cx) let mut detail = UndefinedValue());

        unsafe {
            rooted!(in(*cx) let detail_obj = JS_NewObject(*cx, ptr::null()));
            if !detail_obj.get().is_null() {
                // Set errorType property
                rooted!(in(*cx) let mut error_type_val = UndefinedValue());
                error_type
                    .as_str()
                    .safe_to_jsval(cx, error_type_val.handle_mut(), can_gc);
                JS_DefineProperty(
                    *cx,
                    detail_obj.handle(),
                    c"errorType".as_ptr(),
                    error_type_val.handle(),
                    JSPROP_ENUMERATE as u32,
                );

                // Set message property
                rooted!(in(*cx) let mut message_val = UndefinedValue());
                message.safe_to_jsval(cx, message_val.handle_mut(), can_gc);
                JS_DefineProperty(
                    *cx,
                    detail_obj.handle(),
                    c"message".as_ptr(),
                    message_val.handle(),
                    JSPROP_ENUMERATE as u32,
                );

                detail.set(ObjectValue(detail_obj.get()));
            }
        }

        let global = self.global();
        let custom_event = CustomEvent::new_uninitialized(&global, can_gc);
        custom_event.InitCustomEvent(
            cx,
            DOMString::from("servoerror"),
            false, // bubbles
            false, // cancelable
            detail.handle(),
        );

        custom_event
            .upcast::<Event>()
            .fire(self.upcast::<EventTarget>(), can_gc);
    }

    /// Dispatch a preferencechanged event with the given name and value.
    #[expect(unsafe_code)]
    pub(crate) fn dispatch_preference_changed(&self, name: &str, value: &PrefValue, can_gc: CanGc) {
        let cx = GlobalScope::get_cx();

        // Create the detail object: { name: string, value: any }
        rooted!(in(*cx) let mut detail = UndefinedValue());

        unsafe {
            rooted!(in(*cx) let detail_obj = JS_NewObject(*cx, ptr::null()));
            if !detail_obj.get().is_null() {
                // Set name property
                rooted!(in(*cx) let mut name_val = UndefinedValue());
                name.safe_to_jsval(cx, name_val.handle_mut(), can_gc);
                JS_DefineProperty(
                    *cx,
                    detail_obj.handle(),
                    c"name".as_ptr(),
                    name_val.handle(),
                    JSPROP_ENUMERATE as u32,
                );

                // Set value property (convert PrefValue to JSVal)
                rooted!(in(*cx) let mut value_val = UndefinedValue());
                match value {
                    PrefValue::Bool(b) => value_val.set(BooleanValue(*b)),
                    PrefValue::Int(i) => value_val.set(Int32Value(*i as i32)),
                    PrefValue::Str(s) => s.safe_to_jsval(cx, value_val.handle_mut(), can_gc),
                    _ => {
                        // For other types, convert to string representation
                        format!("{:?}", value).safe_to_jsval(cx, value_val.handle_mut(), can_gc);
                    },
                }
                JS_DefineProperty(
                    *cx,
                    detail_obj.handle(),
                    c"value".as_ptr(),
                    value_val.handle(),
                    JSPROP_ENUMERATE as u32,
                );

                detail.set(ObjectValue(detail_obj.get()));
            }
        }

        let global = self.global();
        let custom_event = CustomEvent::new_uninitialized(&global, can_gc);
        custom_event.InitCustomEvent(
            cx,
            DOMString::from("preferencechanged"),
            false, // bubbles
            false, // cancelable
            detail.handle(),
        );

        custom_event
            .upcast::<Event>()
            .fire(self.upcast::<EventTarget>(), can_gc);
    }

    pub(crate) fn is_allowed_to_embed_for_url(url: &ServoUrl) -> bool {
        // TODO: better permission mechanism with finer granularity
        url.domain() == Some("system.localhost") ||
            url.domain() == Some("settings.localhost") ||
            url.domain() == Some("keyboard.localhost") ||
            url.domain() == Some("homescreen.localhost")
    }
}

impl EmbedderHelpers for Embedder {
    #[expect(unsafe_code)]
    fn is_allowed_to_embed(cx: JSContext, _global: HandleObject) -> bool {
        unsafe {
            let in_realm_proof = AlreadyInRealm::assert_for_cx(cx);
            let global_scope = GlobalScope::from_context(*cx, InRealm::Already(&in_realm_proof));
            Self::is_allowed_to_embed_for_url(&global_scope.get_url())
        }
    }
}

impl EmbedderMethods<crate::DomTypeHolder> for Embedder {
    /// Request the embedder to open a new OS-level window with the given URL.
    fn OpenNewOSWindow(&self, url: USVString, features: USVString) {
        let global = self.global();

        // Parse the URL relative to the document's base URL
        let base_url = global.api_base_url();
        let Ok(parsed_url) = ServoUrl::parse_with_base(Some(&base_url), &url) else {
            // Silently ignore invalid URLs
            return;
        };

        let _ = global
            .script_to_embedder_chan()
            .send(EmbedderMsg::OpenNewOSWindow(NewOSWindowParams {
                url: parsed_url,
                features: features.into(),
            }));
    }

    /// Request the embedder to close the currently focused OS window.
    fn CloseCurrentOSWindow(&self) {
        let _ = self
            .global()
            .script_to_embedder_chan()
            .send(EmbedderMsg::CloseCurrentOSWindow);
    }

    /// Request the embedder to exit the application.
    fn Exit(&self) {
        let _ = self
            .global()
            .script_to_embedder_chan()
            .send(EmbedderMsg::ExitApplication);
    }

    /// Request the embedder to start a window drag operation.
    fn StartWindowDrag(&self) {
        let global = self.global();
        let Some(webview_id) = global.webview_id() else {
            return;
        };
        let _ = global
            .script_to_embedder_chan()
            .send(EmbedderMsg::StartWindowDrag(webview_id));
    }

    /// Request the embedder to start a window resize operation.
    fn StartWindowResize(&self) {
        let global = self.global();
        let Some(webview_id) = global.webview_id() else {
            return;
        };
        let _ = global
            .script_to_embedder_chan()
            .send(EmbedderMsg::StartWindowResize(webview_id));
    }

    fn Pairing(&self) -> Root<Dom<<crate::DomTypeHolder as crate::DomTypes>::Pairing>> {
        todo!()
    }

    // Event handler for servo error events
    event_handler!(servoerror, GetOnservoerror, SetOnservoerror);

    // Event handler for preference changed events
    event_handler!(
        preferencechanged,
        GetOnpreferencechanged,
        SetOnpreferencechanged
    );
}
