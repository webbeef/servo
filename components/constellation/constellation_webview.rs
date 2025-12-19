/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

use base::id::{BrowsingContextId, PipelineId, WebViewId};
use embedder_traits::user_contents::UserContentManagerId;
use embedder_traits::{InputEvent, MouseLeftViewportEvent, Theme};
use euclid::Point2D;
use log::warn;
use rustc_hash::FxHashMap;
use script_traits::{ConstellationInputEvent, ScriptThreadMessage};
use style_traits::CSSPixel;

use crate::browsingcontext::BrowsingContext;
use crate::pipeline::Pipeline;
use crate::session_history::JointSessionHistory;

/// The `Constellation`'s view of a `WebView` in the embedding layer. This tracks all of the
/// `Constellation` state for this `WebView`.
pub(crate) struct ConstellationWebView {
    /// The [`WebViewId`] of this [`ConstellationWebView`].
    webview_id: WebViewId,

    /// The currently focused browsing context in this webview for key events.
    /// The focused pipeline is the current entry of the focused browsing
    /// context.
    pub focused_browsing_context_id: BrowsingContextId,

    /// The [`BrowsingContextId`] of the currently hovered browsing context, to use for
    /// knowing which frame is currently receiving cursor events.
    pub hovered_browsing_context_id: Option<BrowsingContextId>,

    /// The last mouse move point in the coordinate space of the Pipeline that it
    /// happened int.
    pub last_mouse_move_point: Point2D<f32, CSSPixel>,

    /// The joint session history for this webview.
    pub session_history: JointSessionHistory,

    /// The [`UserContentManagerId`] for all pipelines in this `WebView`. This is `Some`
    /// if the embedder has set a `UserContentManager` using the WebViewBuilder API and
    /// it is `None` otherwise.
    pub user_content_manager_id: Option<UserContentManagerId>,

    /// The [`Theme`] that this [`ConstellationWebView`] uses. This is communicated to all
    /// `ScriptThread`s so that they know how to render the contents of a particular `WebView.
    theme: Theme,

    /// Whether this webview should never receive focus (hidefocus attribute on the iframe).
    /// When true, focus-related events are processed but focus is not transferred to
    /// elements in this webview's documents.
    pub hide_focus: bool,
}

impl ConstellationWebView {
    pub(crate) fn new(
        webview_id: WebViewId,
        focused_browsing_context_id: BrowsingContextId,
        user_content_manager_id: Option<UserContentManagerId>,
    ) -> Self {
        Self::new_with_hide_focus(
            webview_id,
            focused_browsing_context_id,
            user_content_manager_id,
            false,
        )
    }

    pub(crate) fn new_with_hide_focus(
        webview_id: WebViewId,
        focused_browsing_context_id: BrowsingContextId,
        user_content_manager_id: Option<UserContentManagerId>,
        hide_focus: bool,
    ) -> Self {
        Self {
            webview_id,
            user_content_manager_id,
            focused_browsing_context_id,
            hovered_browsing_context_id: None,
            last_mouse_move_point: Default::default(),
            session_history: JointSessionHistory::new(),
            theme: Theme::Light,
            hide_focus,
        }
    }

    /// Set the [`Theme`] on this [`ConstellationWebView`] returning true if the theme changed.
    pub(crate) fn set_theme(&mut self, new_theme: Theme) -> bool {
        let old_theme = std::mem::replace(&mut self.theme, new_theme);
        old_theme != self.theme
    }

    /// Get the [`Theme`] of this [`ConstellationWebView`].
    pub(crate) fn theme(&self) -> Theme {
        self.theme
    }

    fn target_pipeline_id_for_input_event(
        &self,
        event: &ConstellationInputEvent,
        browsing_contexts: &FxHashMap<BrowsingContextId, BrowsingContext>,
    ) -> Option<PipelineId> {
        // For pointer events with coordinates (mouse, touch, wheel), we always route to
        // the ROOT browsing context of this webview, not the focused one.
        // This is because:
        // 1. WebRender hit testing doesn't respect CSS z-index/stacking contexts
        // 2. When an embedded webview is focused, we still want pointer events to go through
        //    the parent's DOM hit testing first to respect z-index of overlays
        // 3. The parent's DOM hit testing will forward events to embedded webviews if the
        //    hit target is an embedded iframe element
        //
        // The root browsing context ID is the same as the webview ID (WebViewId wraps
        // TopLevelBrowsingContextId which wraps BrowsingContextId).
        let has_pointer_coordinates = matches!(
            event.event.event,
            InputEvent::MouseButton(_) |
                InputEvent::MouseMove(_) |
                InputEvent::Touch(_) |
                InputEvent::Wheel(_)
        );

        if has_pointer_coordinates {
            // Route to the root/parent browsing context of this webview
            let root_browsing_context_id: BrowsingContextId = self.webview_id.into();
            return Some(
                browsing_contexts
                    .get(&root_browsing_context_id)?
                    .pipeline_id,
            );
        }

        // For non-pointer events (keyboard, etc.), use hit test result if available
        if let Some(hit_test_result) = &event.hit_test_result {
            return Some(hit_test_result.pipeline_id);
        }

        // Otherwise, send to either the hovered or focused browsing context
        let browsing_context_id = if matches!(event.event.event, InputEvent::MouseLeftViewport(_)) {
            self.hovered_browsing_context_id
                .unwrap_or(self.focused_browsing_context_id)
        } else {
            self.focused_browsing_context_id
        };

        Some(browsing_contexts.get(&browsing_context_id)?.pipeline_id)
    }

    /// Forward the [`InputEvent`] to this [`ConstellationWebView`]. Returns false if
    /// the event could not be forwarded or true otherwise.
    pub(crate) fn forward_input_event(
        &mut self,
        event: ConstellationInputEvent,
        pipelines: &FxHashMap<PipelineId, Pipeline>,
        browsing_contexts: &FxHashMap<BrowsingContextId, BrowsingContext>,
    ) -> bool {
        let Some(pipeline_id) = self.target_pipeline_id_for_input_event(&event, browsing_contexts)
        else {
            warn!("Unknown pipeline for input event. Ignoring.");
            return false;
        };
        let Some(pipeline) = pipelines.get(&pipeline_id) else {
            warn!("Unknown pipeline id {pipeline_id:?} for input event. Ignoring.");
            return false;
        };

        let mut update_hovered_browsing_context =
            |newly_hovered_browsing_context_id, focus_moving_to_another_iframe: bool| {
                let old_hovered_context_id = std::mem::replace(
                    &mut self.hovered_browsing_context_id,
                    newly_hovered_browsing_context_id,
                );
                if old_hovered_context_id == newly_hovered_browsing_context_id {
                    return;
                }
                let Some(old_hovered_context_id) = old_hovered_context_id else {
                    return;
                };
                let Some(pipeline) = browsing_contexts
                    .get(&old_hovered_context_id)
                    .and_then(|browsing_context| pipelines.get(&browsing_context.pipeline_id))
                else {
                    return;
                };

                let mut synthetic_mouse_leave_event = event.clone();
                synthetic_mouse_leave_event.event.event =
                    InputEvent::MouseLeftViewport(MouseLeftViewportEvent {
                        focus_moving_to_another_iframe,
                    });

                let _ = pipeline
                    .event_loop
                    .send(ScriptThreadMessage::SendInputEvent(
                        self.webview_id,
                        pipeline.id,
                        synthetic_mouse_leave_event,
                    ));
            };

        if let InputEvent::MouseLeftViewport(_) = &event.event.event {
            update_hovered_browsing_context(None, false);
            return true;
        }

        if let InputEvent::MouseMove(_) = &event.event.event {
            update_hovered_browsing_context(Some(pipeline.browsing_context_id), true);
            if let Some(ref hit_test_result) = event.hit_test_result {
                self.last_mouse_move_point = hit_test_result.point_in_viewport;
            }
        }

        let _ = pipeline
            .event_loop
            .send(ScriptThreadMessage::SendInputEvent(
                self.webview_id,
                pipeline.id,
                event,
            ));
        true
    }
}
