/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

//! Implementation of embedded webview functionality for HTMLIFrameElement.

use std::rc::Rc;

use base::Epoch;
use base::generic_channel::GenericSender;
use base::id::{Index, PipelineId, PipelineNamespaceId};
use constellation_traits::{
    BlobImpl, EmbeddedWebViewEventType, ScriptToConstellationMessage, TraversalDirection,
};
use embedder_traits::{
    AlertResponse, AllowOrDeny, ConfirmResponse, ContextMenuAction, ContextMenuItem,
    EmbeddedWebViewScreenshotError, EmbeddedWebViewScreenshotRequest,
    EmbeddedWebViewScreenshotResult, EmbedderControlId, EmbedderControlRequest,
    EmbedderControlResponse, InputMethodType, LoadStatus, PermissionFeature, PromptResponse,
    RgbColor, ScreenshotImageType, SimpleDialogRequest,
};
use js::jsval::UndefinedValue;
use script_bindings::codegen::GenericBindings::CustomEventBinding::CustomEventMethods;
use script_bindings::conversions::SafeToJSValConvertible;
use stylo_atoms::Atom;

use crate::dom::bindings::codegen::Bindings::EmbeddedWebViewBinding::{
    EmbedderColorParameters, EmbedderContextMenuItem, EmbedderContextMenuParameters,
    EmbedderControlHideEventDetail, EmbedderControlPosition, EmbedderControlShowEventDetail,
    EmbedderDialogShowEventDetail, EmbedderFileParameters, EmbedderInputMethodParameters,
    EmbedderNotificationShowEventDetail, EmbedderPermissionParameters, EmbedderSelectOption,
    EmbedderSelectParameters, ScreenshotOptions,
};
use crate::dom::bindings::error::{Error, Fallible};
use crate::dom::bindings::inheritance::Castable;
use crate::dom::bindings::num::Finite;
use crate::dom::bindings::str::{DOMString, USVString};
use crate::dom::blob::Blob;
use crate::dom::customevent::CustomEvent;
use crate::dom::event::Event;
use crate::dom::eventtarget::EventTarget;
use crate::dom::globalscope::GlobalScope;
use crate::dom::html::htmliframeelement::HTMLIFrameElement;
use crate::dom::node::NodeTraits;
use crate::dom::promise::Promise;
use crate::routed_promise::{RoutedPromiseListener, callback_promise};
use crate::script_runtime::CanGc;

/// Storage for pending dialog response senders.
/// When an embedded webview shows a dialog, the IPC sender is stored here
/// until the parent shell responds via respondToAlert/Confirm/Prompt.
pub(crate) enum PendingDialogSender {
    Alert(GenericSender<AlertResponse>),
    Confirm(GenericSender<ConfirmResponse>),
    Prompt(GenericSender<PromptResponse>),
}

/// Embedded webview methods for HTMLIFrameElement.
/// These methods are separated into this module for easier maintenance.
impl HTMLIFrameElement {
    /// Dispatch an event from an embedded webview to this iframe element.
    /// This is called when the embedded webview's document reports changes
    /// (title, URL, load status) that should be exposed to the parent document.
    pub(crate) fn dispatch_embedded_webview_event(
        &self,
        event: EmbeddedWebViewEventType,
        can_gc: CanGc,
    ) {
        // Only dispatch events if this is actually an embedded webview iframe
        if !self.is_embedded_webview() {
            return;
        }

        // Update history tracking when constellation sends history change
        if let EmbeddedWebViewEventType::HistoryChanged(ref entries, index) = event {
            self.set_embedded_history(entries.clone(), index);
        }

        let event_type = match &event {
            EmbeddedWebViewEventType::UrlChanged(_) => Atom::from("embedurlchange"),
            EmbeddedWebViewEventType::TitleChanged(_) => Atom::from("embedtitlechange"),
            EmbeddedWebViewEventType::LoadStatusChanged(_) => Atom::from("embedloadstatuschange"),
            EmbeddedWebViewEventType::FaviconChanged { .. } => Atom::from("embedfaviconchange"),
            EmbeddedWebViewEventType::Closed => Atom::from("embedclosed"),
            EmbeddedWebViewEventType::HistoryChanged(_, _) => Atom::from("embedhistorychange"),
            EmbeddedWebViewEventType::HistoryTraversalComplete(_) => {
                Atom::from("embedhistorytraversalcomplete")
            },
            EmbeddedWebViewEventType::ThemeColorChanged(_) => Atom::from("embedthemecolorchange"),
            EmbeddedWebViewEventType::InputReceived => Atom::from("embedinputreceived"),
            EmbeddedWebViewEventType::EmbedderControlShow { .. } => Atom::from("embedcontrolshow"),
            EmbeddedWebViewEventType::EmbedderControlHide { .. } => Atom::from("embedcontrolhide"),
            EmbeddedWebViewEventType::SimpleDialogShow(_) => Atom::from("embeddialogshow"),
            EmbeddedWebViewEventType::NotificationShow(_) => Atom::from("embednotificationshow"),
        };

        let cx = GlobalScope::get_cx();
        rooted!(in(*cx) let mut detail = UndefinedValue());

        // Convert event data to JS value for CustomEvent detail
        match &event {
            EmbeddedWebViewEventType::UrlChanged(url) => {
                url.as_str().safe_to_jsval(cx, detail.handle_mut(), can_gc);
            },
            EmbeddedWebViewEventType::TitleChanged(title) => {
                // Option<String> needs special handling - convert to null or string
                match title {
                    Some(t) => t.as_str().safe_to_jsval(cx, detail.handle_mut(), can_gc),
                    None => {
                        // detail stays as undefined for null/None title
                    },
                }
            },
            EmbeddedWebViewEventType::LoadStatusChanged(status) => {
                let status_str = match status {
                    LoadStatus::Started => "started",
                    LoadStatus::HeadParsed => "headparsed",
                    LoadStatus::Complete => "complete",
                };
                status_str.safe_to_jsval(cx, detail.handle_mut(), can_gc);
            },
            EmbeddedWebViewEventType::FaviconChanged {
                bytes,
                width: _,
                height: _,
            } => {
                // Create a Blob from the favicon image bytes
                let global = self.owner_global();
                let blob_impl = BlobImpl::new_from_bytes(bytes.to_vec(), "image/png".to_string());
                let blob = Blob::new(&global, blob_impl, can_gc);
                blob.safe_to_jsval(cx, detail.handle_mut(), can_gc);
            },
            EmbeddedWebViewEventType::Closed => {
                // detail stays as undefined for closed event
            },
            EmbeddedWebViewEventType::HistoryChanged(_, index) => {
                // For history change, expose the current index
                (*index as u32).safe_to_jsval(cx, detail.handle_mut(), can_gc);
            },
            EmbeddedWebViewEventType::HistoryTraversalComplete(traversal_id) => {
                // Expose the traversal ID so scripts can match with goBack()/goForward() return values
                traversal_id
                    .to_string()
                    .safe_to_jsval(cx, detail.handle_mut(), can_gc);
            },
            EmbeddedWebViewEventType::ThemeColorChanged(content) => {
                content.safe_to_jsval(cx, detail.handle_mut(), can_gc);
            },
            EmbeddedWebViewEventType::InputReceived => {
                // detail stays as undefined for input received event
            },
            EmbeddedWebViewEventType::EmbedderControlShow { id, rect, request } => {
                // Create a dictionary-based detail object with control information
                let control_type = match &request {
                    EmbedderControlRequest::SelectElement(..) => "select",
                    EmbedderControlRequest::ColorPicker(..) => "color",
                    EmbedderControlRequest::FilePicker(..) => "file",
                    EmbedderControlRequest::InputMethod(..) => "inputmethod",
                    EmbedderControlRequest::ContextMenu(..) => "contextmenu",
                    EmbedderControlRequest::PermissionPrompt(..) => "permission",
                };

                // Serialize the control ID for response routing
                let control_id = format!("{:?}", id);

                // Create position dictionary
                let position = EmbedderControlPosition {
                    x: Finite::wrap(rect.min.x as f64),
                    y: Finite::wrap(rect.min.y as f64),
                    width: Finite::wrap(rect.width() as f64),
                    height: Finite::wrap(rect.height() as f64),
                };

                // Build control-specific parameters
                let mut select_parameters = None;
                let mut color_parameters = None;
                let mut file_parameters = None;
                let mut input_method_parameters = None;
                let mut context_menu_parameters = None;
                let mut permission_parameters = None;

                match &request {
                    EmbedderControlRequest::SelectElement(options, selected) => {
                        let select_options: Vec<EmbedderSelectOption> = options
                            .iter()
                            .flat_map(|opt_or_group| match opt_or_group {
                                embedder_traits::SelectElementOptionOrOptgroup::Option(opt) => {
                                    vec![EmbedderSelectOption {
                                        id: opt.id as u32,
                                        label: DOMString::from(opt.label.clone()),
                                        disabled: opt.is_disabled,
                                        group: None,
                                    }]
                                },
                                embedder_traits::SelectElementOptionOrOptgroup::Optgroup {
                                    label,
                                    options,
                                } => options
                                    .iter()
                                    .map(|opt| EmbedderSelectOption {
                                        id: opt.id as u32,
                                        label: DOMString::from(opt.label.clone()),
                                        disabled: opt.is_disabled,
                                        group: Some(DOMString::from(label.clone())),
                                    })
                                    .collect(),
                            })
                            .collect();
                        select_parameters = Some(EmbedderSelectParameters {
                            options: Some(select_options),
                            selectedIndex: selected.map(|i| i as i32).unwrap_or(-1),
                        });
                    },
                    EmbedderControlRequest::ColorPicker(color) => {
                        let current_color =
                            format!("#{:02x}{:02x}{:02x}", color.red, color.green, color.blue);
                        color_parameters = Some(EmbedderColorParameters {
                            currentColor: DOMString::from(current_color),
                        });
                    },
                    EmbedderControlRequest::FilePicker(file_request) => {
                        let accept_types: Vec<DOMString> = file_request
                            .filter_patterns
                            .iter()
                            .map(|p| DOMString::from(format!(".{}", p.0)))
                            .collect();
                        file_parameters = Some(EmbedderFileParameters {
                            multiple: file_request.allow_select_multiple,
                            acceptTypes: Some(accept_types),
                            directory: None,
                        });
                    },
                    EmbedderControlRequest::InputMethod(im_request) => {
                        let input_type = match im_request.input_method_type {
                            InputMethodType::Color => "color",
                            InputMethodType::Date => "date",
                            InputMethodType::DatetimeLocal => "datetime-local",
                            InputMethodType::Email => "email",
                            InputMethodType::Month => "month",
                            InputMethodType::Number => "number",
                            InputMethodType::Password => "password",
                            InputMethodType::Search => "search",
                            InputMethodType::Tel => "tel",
                            InputMethodType::Text => "text",
                            InputMethodType::Time => "time",
                            InputMethodType::Url => "url",
                            InputMethodType::Week => "week",
                        };
                        input_method_parameters = Some(EmbedderInputMethodParameters {
                            inputType: DOMString::from(input_type),
                            currentValue: Some(DOMString::from(im_request.text.clone())),
                            placeholder: None,
                        });
                    },
                    EmbedderControlRequest::ContextMenu(menu_request) => {
                        // Check history state to properly enable/disable GoBack and GoForward
                        let (can_go_back, can_go_forward) = self.embedded_history_state();

                        let items: Vec<EmbedderContextMenuItem> = menu_request
                            .items
                            .iter()
                            .filter_map(|item| match item {
                                ContextMenuItem::Item {
                                    label,
                                    action,
                                    enabled,
                                } => {
                                    // Override enabled status for history actions based on actual state
                                    let is_enabled = match action {
                                        ContextMenuAction::GoBack => can_go_back,
                                        ContextMenuAction::GoForward => can_go_forward,
                                        _ => *enabled,
                                    };
                                    Some(EmbedderContextMenuItem {
                                        id: DOMString::from(format!("{:?}", action)),
                                        label: DOMString::from(label.clone()),
                                        disabled: !is_enabled,
                                        checked: false,
                                        icon: None,
                                    })
                                },
                                ContextMenuItem::Separator => None,
                            })
                            .collect();
                        context_menu_parameters =
                            Some(EmbedderContextMenuParameters { items: Some(items) });
                    },
                    EmbedderControlRequest::PermissionPrompt(permission_request) => {
                        // Convert PermissionFeature to string representations
                        let feature = match permission_request.feature {
                            PermissionFeature::Geolocation => "geolocation",
                            PermissionFeature::Notifications => "notifications",
                            PermissionFeature::Push => "push",
                            PermissionFeature::Midi => "midi",
                            PermissionFeature::Camera => "camera",
                            PermissionFeature::Microphone => "microphone",
                            PermissionFeature::Speaker => "speaker",
                            PermissionFeature::DeviceInfo => "device-info",
                            PermissionFeature::BackgroundSync => "background-sync",
                            PermissionFeature::Bluetooth => "bluetooth",
                            PermissionFeature::PersistentStorage => "persistent-storage",
                        };

                        let feature_name = match permission_request.feature {
                            PermissionFeature::Geolocation => "Location",
                            PermissionFeature::Notifications => "Notifications",
                            PermissionFeature::Push => "Push",
                            PermissionFeature::Midi => "MIDI",
                            PermissionFeature::Camera => "Camera",
                            PermissionFeature::Microphone => "Microphone",
                            PermissionFeature::Speaker => "Speaker",
                            PermissionFeature::DeviceInfo => "Device Info",
                            PermissionFeature::BackgroundSync => "Background Sync",
                            PermissionFeature::Bluetooth => "Bluetooth",
                            PermissionFeature::PersistentStorage => "Persistent Storage",
                        };

                        // Store the response sender for later use by respondToPermissionPrompt
                        self.set_pending_permission_sender(Some(
                            permission_request.response_sender.clone(),
                        ));

                        permission_parameters = Some(EmbedderPermissionParameters {
                            feature: DOMString::from(feature),
                            featureName: DOMString::from(feature_name),
                        });
                    },
                }

                // Build the event detail dictionary
                let event_detail = EmbedderControlShowEventDetail {
                    controlType: DOMString::from(control_type),
                    controlId: DOMString::from(control_id),
                    position: Some(position),
                    selectParameters: select_parameters,
                    colorParameters: color_parameters,
                    fileParameters: file_parameters,
                    inputMethodParameters: input_method_parameters,
                    contextMenuParameters: context_menu_parameters,
                    permissionParameters: permission_parameters,
                };

                event_detail.safe_to_jsval(cx, detail.handle_mut(), can_gc);
            },
            EmbeddedWebViewEventType::EmbedderControlHide { id } => {
                // Create a dictionary for hide events
                let event_detail = EmbedderControlHideEventDetail {
                    controlId: DOMString::from(format!("{:?}", id)),
                };
                event_detail.safe_to_jsval(cx, detail.handle_mut(), can_gc);
            },
            EmbeddedWebViewEventType::SimpleDialogShow(request) => {
                // Clone the request to take ownership since we're matching on a reference
                let request = request.clone();

                // Extract the request details and store the sender for response routing
                let (id, dialog_type_str, message, default_value, sender) = match request {
                    SimpleDialogRequest::Alert {
                        id,
                        message,
                        response_sender,
                    } => (
                        id,
                        "alert",
                        message,
                        None,
                        PendingDialogSender::Alert(response_sender),
                    ),
                    SimpleDialogRequest::Confirm {
                        id,
                        message,
                        response_sender,
                    } => (
                        id,
                        "confirm",
                        message,
                        None,
                        PendingDialogSender::Confirm(response_sender),
                    ),
                    SimpleDialogRequest::Prompt {
                        id,
                        message,
                        default,
                        response_sender,
                    } => (
                        id,
                        "prompt",
                        message,
                        Some(default),
                        PendingDialogSender::Prompt(response_sender),
                    ),
                };

                // Store the sender for response routing
                self.set_pending_dialog(Some(sender));

                let event_detail = EmbedderDialogShowEventDetail {
                    dialogType: DOMString::from(dialog_type_str),
                    controlId: DOMString::from(format!("{:?}", id)),
                    message: DOMString::from(message),
                    defaultValue: Some(default_value.map(DOMString::from)),
                };
                event_detail.safe_to_jsval(cx, detail.handle_mut(), can_gc);
            },
            EmbeddedWebViewEventType::NotificationShow(notification) => {
                let event_detail = EmbedderNotificationShowEventDetail {
                    title: DOMString::from(notification.title.clone()),
                    body: DOMString::from(notification.body.clone()),
                    tag: DOMString::from(notification.tag.clone()),
                    iconUrl: notification
                        .icon_url
                        .as_ref()
                        .map(|url| DOMString::from(url.as_str())),
                };
                event_detail.safe_to_jsval(cx, detail.handle_mut(), can_gc);
            },
        }

        let global = self.owner_global();
        let custom_event = CustomEvent::new_uninitialized(&global, can_gc);
        custom_event.InitCustomEvent(
            cx,
            DOMString::from(event_type.as_ref()),
            false, // bubbles
            false, // cancelable
            detail.handle(),
        );

        custom_event
            .upcast::<Event>()
            .fire(self.upcast::<EventTarget>(), can_gc);
    }

    /// Parse a control ID string back into an EmbedderControlId.
    /// The format is the Debug output: "EmbedderControlId { webview_id: ..., pipeline_id: ..., index: ... }"
    fn parse_control_id(&self, control_id: &DOMString) -> Fallible<EmbedderControlId> {
        if !self.is_embedded_webview() {
            return Err(Error::InvalidState(Some(
                "This iframe is not an embedded webview".to_string(),
            )));
        }

        let s = control_id.str();

        // Parse the control ID from Debug format:
        // "EmbedderControlId { webview_id: WebViewId(PainterId(N), (A,B)), pipeline_id: (X,Y), index: Epoch(E) }"

        // Extract pipeline_id: (X,Y)
        let pipeline_id = if let Some(idx) = s.find("pipeline_id: (") {
            let start = idx + 14; // length of "pipeline_id: ("
            let end = s[start..].find(')').map(|e| start + e).unwrap_or(s.len());
            let parts: Vec<&str> = s[start..end].split(',').collect();
            if parts.len() == 2 {
                let namespace = parts[0].trim().parse::<u32>().ok();
                let index = parts[1].trim().parse::<u32>().ok();
                match (namespace, index) {
                    (Some(ns), Some(idx)) => Index::new(idx).ok().map(|index| PipelineId {
                        namespace_id: PipelineNamespaceId(ns),
                        index,
                    }),
                    _ => None,
                }
            } else {
                None
            }
        } else {
            None
        };

        let pipeline_id = pipeline_id.ok_or_else(|| {
            Error::InvalidState(Some(format!(
                "Could not parse pipeline_id from control ID: {:?}",
                s
            )))
        })?;

        // Extract webview_id from the embedded webview or parse from string
        // The webview_id format is: WebViewId(PainterId(N), (A,B))
        // For simplicity, use the stored embedded_webview_id since it should match
        let webview_id = self
            .embedded_webview_id()
            .ok_or_else(|| Error::InvalidState(Some("No embedded webview ID".to_string())))?;

        // Extract the epoch from the control ID string
        // Look for "index: Epoch(" and extract the number
        let epoch = if let Some(idx) = s.find("Epoch(") {
            let start = idx + 6;
            let end = s[start..].find(')').map(|e| start + e).unwrap_or(s.len());
            s[start..end].parse::<u32>().unwrap_or(0)
        } else {
            0
        };

        Ok(EmbedderControlId {
            webview_id,
            pipeline_id,
            index: Epoch(epoch),
        })
    }

    /// Send a control response to the embedded webview.
    fn send_control_response(
        &self,
        id: EmbedderControlId,
        response: EmbedderControlResponse,
    ) -> Fallible<()> {
        if !self.is_embedded_webview() {
            return Err(Error::InvalidState(Some(
                "This iframe is not an embedded webview".to_string(),
            )));
        }

        let window = self.owner_window();
        window
            .as_global_scope()
            .script_to_constellation_chan()
            .send(ScriptToConstellationMessage::EmbeddedWebViewControlResponse(id, response))
            .unwrap();

        Ok(())
    }

    // ========== Embedded WebView WebIDL helper methods ==========
    // These are called from the HTMLIFrameElementMethods impl in htmliframeelement.rs

    /// Load a URL in the embedded webview.
    pub(crate) fn embedded_load(&self, url: USVString) -> Fallible<()> {
        let Some(webview_id) = self.embedded_webview_id() else {
            return Err(crate::dom::bindings::error::Error::InvalidState(None));
        };

        // Parse and validate URL
        let base_url = self.owner_document().base_url();
        let Ok(url) = base_url.join(&url.0) else {
            return Ok(());
        };

        let window = self.owner_window();
        window
            .as_global_scope()
            .script_to_constellation_chan()
            .send(ScriptToConstellationMessage::EmbeddedWebViewLoad(
                webview_id, url,
            ))
            .unwrap();
        Ok(())
    }

    /// Get the current page zoom level for the embedded webview.
    pub(crate) fn embedded_page_zoom(&self) -> Fallible<f64> {
        if !self.is_embedded_webview() {
            return Err(crate::dom::bindings::error::Error::InvalidState(None));
        }
        Ok(self.get_page_zoom())
    }

    /// Set the page zoom level for the embedded webview.
    pub(crate) fn embedded_set_page_zoom(&self, zoom: f64) -> Fallible<()> {
        let Some(webview_id) = self.embedded_webview_id() else {
            return Err(crate::dom::bindings::error::Error::InvalidState(None));
        };

        // Clamp to valid range (0.1 to 10.0)
        let zoom = zoom.clamp(0.1, 10.0);
        self.set_page_zoom(zoom);

        let window = self.owner_window();
        window
            .as_global_scope()
            .script_to_constellation_chan()
            .send(ScriptToConstellationMessage::EmbeddedWebViewSetPageZoom(
                webview_id,
                zoom as f32,
            ))
            .unwrap();
        Ok(())
    }

    /// Reload the embedded webview.
    pub(crate) fn embedded_reload(&self) -> Fallible<()> {
        let Some(webview_id) = self.embedded_webview_id() else {
            return Err(crate::dom::bindings::error::Error::InvalidState(None));
        };

        let window = self.owner_window();
        window
            .as_global_scope()
            .script_to_constellation_chan()
            .send(ScriptToConstellationMessage::EmbeddedWebViewReload(
                webview_id,
            ))
            .unwrap();
        Ok(())
    }

    /// Returns true if the embedded webview can navigate back in history.
    pub(crate) fn embedded_can_go_back(&self) -> Fallible<bool> {
        if !self.is_embedded_webview() {
            return Err(crate::dom::bindings::error::Error::InvalidState(None));
        }
        let (can_go_back, _) = self.embedded_history_state();
        Ok(can_go_back)
    }

    /// Navigate back in the embedded webview's history.
    pub(crate) fn embedded_go_back(&self) -> Fallible<DOMString> {
        let Some(webview_id) = self.embedded_webview_id() else {
            return Err(crate::dom::bindings::error::Error::InvalidState(None));
        };
        let (can_go_back, _) = self.embedded_history_state();
        if !can_go_back {
            return Ok(DOMString::new()); // Can't go back
        }

        let traversal_id = embedder_traits::TraversalId::new();
        let window = self.owner_window();
        window
            .as_global_scope()
            .script_to_constellation_chan()
            .send(
                ScriptToConstellationMessage::EmbeddedWebViewTraverseHistory(
                    webview_id,
                    TraversalDirection::Back(1),
                    traversal_id.clone(),
                ),
            )
            .unwrap();
        Ok(DOMString::from(traversal_id.to_string()))
    }

    /// Returns true if the embedded webview can navigate forward in history.
    pub(crate) fn embedded_can_go_forward(&self) -> Fallible<bool> {
        if !self.is_embedded_webview() {
            return Err(crate::dom::bindings::error::Error::InvalidState(None));
        }
        let (_, can_go_forward) = self.embedded_history_state();
        Ok(can_go_forward)
    }

    /// Navigate forward in the embedded webview's history.
    pub(crate) fn embedded_go_forward(&self) -> Fallible<DOMString> {
        let Some(webview_id) = self.embedded_webview_id() else {
            return Err(crate::dom::bindings::error::Error::InvalidState(None));
        };
        let (_, can_go_forward) = self.embedded_history_state();
        if !can_go_forward {
            return Ok(DOMString::new()); // Can't go forward
        }

        let traversal_id = embedder_traits::TraversalId::new();
        let window = self.owner_window();
        window
            .as_global_scope()
            .script_to_constellation_chan()
            .send(
                ScriptToConstellationMessage::EmbeddedWebViewTraverseHistory(
                    webview_id,
                    TraversalDirection::Forward(1),
                    traversal_id.clone(),
                ),
            )
            .unwrap();
        Ok(DOMString::from(traversal_id.to_string()))
    }

    /// Take a screenshot of the embedded webview.
    pub(crate) fn embedded_take_screenshot(
        &self,
        options: &ScreenshotOptions,
    ) -> Fallible<Rc<Promise>> {
        let can_gc = CanGc::note();

        // Must be an embedded webview
        let Some(webview_id) = self.embedded_webview_id() else {
            return Err(Error::InvalidState(Some(
                "This iframe is not an embedded webview".to_string(),
            )));
        };

        let global = self.owner_global();
        let promise = Promise::new(&global, can_gc);

        // Parse image type from options
        let image_type = match &*options.type_.str() {
            "image/jpeg" => ScreenshotImageType::Jpeg,
            "image/webp" => ScreenshotImageType::Webp,
            _ => ScreenshotImageType::Png, // Default to PNG for unknown types
        };

        // Clamp quality to valid range (0.0 to 1.0)
        let quality = options.quality.clamp(0.0, 1.0);

        let request = EmbeddedWebViewScreenshotRequest {
            image_type,
            quality,
        };

        let callback = callback_promise(
            &promise,
            self,
            global.task_manager().dom_manipulation_task_source(),
        );

        // Send the screenshot request to the constellation
        global
            .script_to_constellation_chan()
            .send(ScriptToConstellationMessage::EmbeddedWebViewTakeScreenshot(
                webview_id, request, callback,
            ))
            .unwrap();

        Ok(promise)
    }

    /// Respond to a select control request.
    pub(crate) fn embedded_respond_to_select_control(
        &self,
        control_id: DOMString,
        selected_index: i32,
    ) -> Fallible<()> {
        let id = self.parse_control_id(&control_id)?;
        let response = if selected_index < 0 {
            EmbedderControlResponse::SelectElement(None)
        } else {
            EmbedderControlResponse::SelectElement(Some(selected_index as usize))
        };
        self.send_control_response(id, response)
    }

    /// Respond to a color picker request.
    pub(crate) fn embedded_respond_to_color_picker(
        &self,
        control_id: DOMString,
        color: Option<DOMString>,
    ) -> Fallible<()> {
        let id = self.parse_control_id(&control_id)?;
        let response = match color {
            Some(ref c) => EmbedderControlResponse::ColorPicker(parse_color(c)),
            None => EmbedderControlResponse::ColorPicker(None),
        };
        self.send_control_response(id, response)
    }

    /// Respond to a context menu request with the selected action.
    pub(crate) fn embedded_respond_to_context_menu(
        &self,
        control_id: DOMString,
        action_id: Option<DOMString>,
    ) -> Fallible<()> {
        let id = self.parse_control_id(&control_id)?;

        // Parse the action ID string back to a ContextMenuAction
        let action = action_id.and_then(|action_str| {
            let s = action_str.str();
            if s == "GoBack" {
                Some(ContextMenuAction::GoBack)
            } else if s == "GoForward" {
                Some(ContextMenuAction::GoForward)
            } else if s == "Reload" {
                Some(ContextMenuAction::Reload)
            } else if s == "Cut" {
                Some(ContextMenuAction::Cut)
            } else if s == "Copy" {
                Some(ContextMenuAction::Copy)
            } else if s == "Paste" {
                Some(ContextMenuAction::Paste)
            } else if s == "SelectAll" {
                Some(ContextMenuAction::SelectAll)
            } else if s == "CopyLink" {
                Some(ContextMenuAction::CopyLink)
            } else if s == "OpenLinkInNewWebView" {
                Some(ContextMenuAction::OpenLinkInNewWebView)
            } else if s == "CopyImageLink" {
                Some(ContextMenuAction::CopyImageLink)
            } else if s == "OpenImageInNewView" {
                Some(ContextMenuAction::OpenImageInNewView)
            } else {
                None
            }
        });

        let response = EmbedderControlResponse::ContextMenu(action);
        self.send_control_response(id, response)
    }

    /// Cancel an embedder control request.
    pub(crate) fn embedded_cancel_embedder_control(&self, control_id: DOMString) -> Fallible<()> {
        let id = self.parse_control_id(&control_id)?;
        let response = EmbedderControlResponse::SelectElement(None);
        self.send_control_response(id, response)
    }

    /// Respond to an alert dialog request.
    pub(crate) fn embedded_respond_to_alert(&self, _control_id: DOMString) -> Fallible<()> {
        if !self.is_embedded_webview() {
            return Err(Error::InvalidState(Some(
                "This iframe is not an embedded webview".to_string(),
            )));
        }

        let Some(sender) = self.take_pending_dialog() else {
            return Err(Error::InvalidState(Some(
                "No pending dialog to respond to".to_string(),
            )));
        };

        match sender {
            PendingDialogSender::Alert(sender) => {
                let _ = sender.send(AlertResponse::Ok);
                Ok(())
            },
            _ => Err(Error::InvalidState(Some(
                "Pending dialog is not an alert".to_string(),
            ))),
        }
    }

    /// Respond to a confirm dialog request.
    pub(crate) fn embedded_respond_to_confirm(
        &self,
        _control_id: DOMString,
        confirmed: bool,
    ) -> Fallible<()> {
        if !self.is_embedded_webview() {
            return Err(Error::InvalidState(Some(
                "This iframe is not an embedded webview".to_string(),
            )));
        }

        let Some(sender) = self.take_pending_dialog() else {
            return Err(Error::InvalidState(Some(
                "No pending dialog to respond to".to_string(),
            )));
        };

        match sender {
            PendingDialogSender::Confirm(sender) => {
                let response = if confirmed {
                    ConfirmResponse::Ok
                } else {
                    ConfirmResponse::Cancel
                };
                let _ = sender.send(response);
                Ok(())
            },
            _ => Err(Error::InvalidState(Some(
                "Pending dialog is not a confirm".to_string(),
            ))),
        }
    }

    /// Respond to a prompt dialog request.
    pub(crate) fn embedded_respond_to_prompt(
        &self,
        _control_id: DOMString,
        value: Option<DOMString>,
    ) -> Fallible<()> {
        if !self.is_embedded_webview() {
            return Err(Error::InvalidState(Some(
                "This iframe is not an embedded webview".to_string(),
            )));
        }

        let Some(sender) = self.take_pending_dialog() else {
            return Err(Error::InvalidState(Some(
                "No pending dialog to respond to".to_string(),
            )));
        };

        match sender {
            PendingDialogSender::Prompt(sender) => {
                let response = match value {
                    Some(s) => PromptResponse::Ok(s.to_string()),
                    None => PromptResponse::Cancel,
                };
                let _ = sender.send(response);
                Ok(())
            },
            _ => Err(Error::InvalidState(Some(
                "Pending dialog is not a prompt".to_string(),
            ))),
        }
    }

    /// Respond to a permission prompt request.
    pub(crate) fn embedded_respond_to_permission_prompt(
        &self,
        _control_id: DOMString,
        allowed: bool,
    ) -> Fallible<()> {
        if !self.is_embedded_webview() {
            return Err(Error::InvalidState(Some(
                "This iframe is not an embedded webview".to_string(),
            )));
        }

        let Some(sender) = self.take_pending_permission_sender() else {
            return Err(Error::InvalidState(Some(
                "No pending permission prompt to respond to".to_string(),
            )));
        };

        let response = if allowed {
            AllowOrDeny::Allow
        } else {
            AllowOrDeny::Deny
        };
        let _ = sender.send(response);
        Ok(())
    }
}

/// Handle screenshot responses from the compositor via IPC.
impl RoutedPromiseListener<Result<EmbeddedWebViewScreenshotResult, EmbeddedWebViewScreenshotError>>
    for HTMLIFrameElement
{
    fn handle_response(
        &self,
        response: Result<EmbeddedWebViewScreenshotResult, EmbeddedWebViewScreenshotError>,
        promise: &Rc<Promise>,
        can_gc: CanGc,
    ) {
        match response {
            Ok(result) => {
                // Create a Blob from the encoded bytes
                let global = self.owner_global();
                let blob_impl = BlobImpl::new_from_bytes(result.bytes, result.mime_type);
                let blob = Blob::new(&global, blob_impl, can_gc);
                promise.resolve_native(&blob, can_gc);
            },
            Err(error) => {
                let error_msg = match error {
                    EmbeddedWebViewScreenshotError::NotEmbeddedWebView => {
                        "This iframe is not an embedded webview"
                    },
                    EmbeddedWebViewScreenshotError::WebViewDoesNotExist => {
                        "The webview does not exist"
                    },
                    EmbeddedWebViewScreenshotError::CaptureFailed => "Failed to capture screenshot",
                    EmbeddedWebViewScreenshotError::EncodingFailed => "Failed to encode screenshot",
                };
                promise.reject_error(Error::InvalidState(Some(error_msg.to_string())), can_gc);
            },
        }
    }
}

/// Parse a CSS color string (e.g., "#ff0000") into an RgbColor.
fn parse_color(color: &DOMString) -> Option<RgbColor> {
    let color_str = color.str();
    let s = color_str.trim();

    // Parse hex format: #RRGGBB
    if let Some(hex) = s.strip_prefix('#') {
        if hex.len() == 6 {
            let red = u8::from_str_radix(&hex[0..2], 16).ok()?;
            let green = u8::from_str_radix(&hex[2..4], 16).ok()?;
            let blue = u8::from_str_radix(&hex[4..6], 16).ok()?;
            return Some(RgbColor { red, green, blue });
        }
    }

    None
}
