/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

use std::cell::Cell;

use base::id::{BrowsingContextId, PipelineId, TopLevelBrowsingContextId};
use dom_struct::dom_struct;
use html5ever::{local_name, namespace_url, ns, LocalName, Prefix};
use js::rust::HandleObject;
use profile_traits::ipc as ProfiledIpc;
use script_traits::IFrameSandboxState::IFrameUnsandboxed;
use script_traits::{
    HistoryEntryReplacement, IFrameLoadInfo, IFrameLoadInfoWithData, JsEvalResult, LoadData,
    LoadOrigin, NewLayoutInfo, ScriptMsg, UpdatePipelineIdReason, WindowSizeData,
};
use servo_url::ServoUrl;

use crate::document_loader::{LoadBlocker, LoadType};
use crate::dom::attr::Attr;
use crate::dom::bindings::cell::DomRefCell;
use crate::dom::bindings::codegen::Bindings::HTMLWebViewElementBinding::HTMLWebViewElementMethods;
use crate::dom::bindings::codegen::Bindings::WindowBinding::Window_Binding::WindowMethods;
use crate::dom::bindings::inheritance::Castable;
use crate::dom::bindings::refcounted::Trusted;
use crate::dom::bindings::reflector::DomObject;
use crate::dom::bindings::root::{DomRoot, LayoutDom};
use crate::dom::bindings::str::{DOMString, USVString};
use crate::dom::document::Document;
use crate::dom::element::{AttributeMutation, Element};
use crate::dom::globalscope::GlobalScope;
use crate::dom::htmlelement::HTMLElement;
use crate::dom::node::{
    document_from_node, window_from_node, BindContext, Node, NodeDamage, UnbindContext,
};
use crate::dom::virtualmethods::VirtualMethods;
use crate::dom::windowproxy::WindowProxy;
use crate::script_runtime::CanGc;
use crate::script_thread::ScriptThread;

#[derive(PartialEq)]
enum PipelineType {
    InitialAboutBlank,
    Navigation,
}

#[derive(PartialEq)]
enum ProcessingMode {
    FirstTime,
    NotFirstTime,
}

#[dom_struct]
pub struct HTMLWebViewElement {
    htmlelement: HTMLElement,
    #[no_trace]
    top_level_browsing_context_id: Cell<Option<TopLevelBrowsingContextId>>,
    #[no_trace]
    browsing_context_id: Cell<Option<BrowsingContextId>>,
    #[no_trace]
    pipeline_id: Cell<Option<PipelineId>>,
    #[no_trace]
    pending_pipeline_id: Cell<Option<PipelineId>>,
    #[no_trace]
    about_blank_pipeline_id: Cell<Option<PipelineId>>,
    load_blocker: DomRefCell<Option<LoadBlocker>>,
    throttled: Cell<bool>,
}

impl HTMLWebViewElement {
    /// <https://html.spec.whatwg.org/multipage/#otherwise-steps-for-iframe-or-frame-elements>,
    /// step 1.
    fn get_url(&self) -> ServoUrl {
        println!("get_url");
        let element = self.upcast::<Element>();
        element
            .get_attribute(&ns!(), &local_name!("src"))
            .and_then(|src| {
                let url = src.value();
                if url.is_empty() {
                    None
                } else {
                    document_from_node(self).base_url().join(&url).ok()
                }
            })
            .unwrap_or_else(|| ServoUrl::parse("about:blank").unwrap())
    }

    pub fn navigate_or_reload_child_browsing_context(
        &self,
        load_data: LoadData,
        replace: HistoryEntryReplacement,
        can_gc: CanGc,
    ) {
        println!("navigate_or_reload_child_browsing_context");
        self.start_new_pipeline(load_data, PipelineType::Navigation, replace, can_gc);
    }

    fn start_new_pipeline(
        &self,
        mut load_data: LoadData,
        pipeline_type: PipelineType,
        replace: HistoryEntryReplacement,
        can_gc: CanGc,
    ) {
        println!("start_new_pipeline");

        let browsing_context_id = match self.browsing_context_id() {
            None => return warn!("Attempted to start a new pipeline on an unattached iframe."),
            Some(id) => id,
        };

        let top_level_browsing_context_id = match self.top_level_browsing_context_id() {
            None => return warn!("Attempted to start a new pipeline on an unattached iframe."),
            Some(id) => id,
        };

        let document = document_from_node(self);

        {
            let mut load_blocker = self.load_blocker.borrow_mut();
            // Any oustanding load is finished from the point of view of the blocked
            // document; the new navigation will continue blocking it.
            LoadBlocker::terminate(&mut load_blocker, can_gc);
        }

        if load_data.url.scheme() == "javascript" {
            let window_proxy = self.GetContentWindow();
            if let Some(window_proxy) = window_proxy {
                // Important re security. See https://github.com/servo/servo/issues/23373
                // TODO: check according to https://w3c.github.io/webappsec-csp/#should-block-navigation-request
                if ScriptThread::check_load_origin(&load_data.load_origin, &document.url().origin())
                {
                    ScriptThread::eval_js_url(&window_proxy.global(), &mut load_data);
                }
            }
        }

        match load_data.js_eval_result {
            Some(JsEvalResult::NoContent) => (),
            _ => {
                let mut load_blocker = self.load_blocker.borrow_mut();
                *load_blocker = Some(LoadBlocker::new(
                    &document,
                    LoadType::PageSource(load_data.url.clone()),
                ));
            },
        };

        let window = window_from_node(self);
        let old_pipeline_id = self.pipeline_id();
        let new_pipeline_id = PipelineId::new();
        self.pending_pipeline_id.set(Some(new_pipeline_id));

        let global_scope = window.upcast::<GlobalScope>();
        let load_info = IFrameLoadInfo {
            parent_pipeline_id: global_scope.pipeline_id(),
            browsing_context_id,
            top_level_browsing_context_id,
            new_pipeline_id,
            is_private: false, // FIXME
            inherited_secure_context: load_data.inherited_secure_context,
            replace,
        };

        let window_size = WindowSizeData {
            initial_viewport: window
                .inner_window_dimensions_query(browsing_context_id)
                .unwrap_or_default(),
            device_pixel_ratio: window.device_pixel_ratio(),
        };

        match pipeline_type {
            PipelineType::InitialAboutBlank => {
                self.about_blank_pipeline_id.set(Some(new_pipeline_id));

                let load_info = IFrameLoadInfoWithData {
                    info: load_info,
                    load_data: load_data.clone(),
                    old_pipeline_id,
                    sandbox: IFrameUnsandboxed,
                    window_size,
                };
                global_scope
                    .script_to_constellation_chan()
                    .send(ScriptMsg::ScriptNewIFrame(load_info))
                    .unwrap();

                let new_layout_info = NewLayoutInfo {
                    parent_info: Some(global_scope.pipeline_id()),
                    new_pipeline_id,
                    browsing_context_id,
                    top_level_browsing_context_id,
                    opener: None,
                    load_data,
                    window_size,
                };

                self.pipeline_id.set(Some(new_pipeline_id));
                ScriptThread::process_attach_layout(new_layout_info, document.origin().clone());
            },
            PipelineType::Navigation => {
                let load_info = IFrameLoadInfoWithData {
                    info: load_info,
                    load_data,
                    old_pipeline_id,
                    sandbox: IFrameUnsandboxed,
                    window_size,
                };
                global_scope
                    .script_to_constellation_chan()
                    .send(ScriptMsg::ScriptLoadedURLInIFrame(load_info))
                    .unwrap();
            },
        }
    }

    /// <https://html.spec.whatwg.org/multipage/#process-the-iframe-attributes>
    fn process_the_iframe_attributes(&self, mode: ProcessingMode, can_gc: CanGc) {
        println!("process_the_iframe_attribute");
        let window = window_from_node(self);

        // https://html.spec.whatwg.org/multipage/#attr-iframe-name
        // Note: the spec says to set the name 'when the nested browsing context is created'.
        // The current implementation sets the name on the window,
        // when the iframe attributes are first processed.
        if mode == ProcessingMode::FirstTime {
            if let Some(window) = self.GetContentWindow() {
                window.set_name(
                    self.upcast::<Element>()
                        .get_name()
                        .map_or(DOMString::from(""), |n| DOMString::from(&*n)),
                );
            }
        }

        if mode == ProcessingMode::FirstTime &&
            !self.upcast::<Element>().has_attribute(&local_name!("src"))
        {
            return;
        }

        // > 2. Otherwise, if `element` has a `src` attribute specified, or
        // >    `initialInsertion` is false, then run the shared attribute
        // >    processing steps for `iframe` and `frame` elements given
        // >    `element`.
        let url = self.get_url();

        let creator_pipeline_id = if url.as_str() == "about:blank" {
            Some(window.upcast::<GlobalScope>().pipeline_id())
        } else {
            None
        };

        let document = document_from_node(self);
        let load_data = LoadData::new(
            LoadOrigin::Script(document.origin().immutable().clone()),
            url,
            creator_pipeline_id,
            window.upcast::<GlobalScope>().get_referrer(),
            document.get_referrer_policy(),
            Some(window.upcast::<GlobalScope>().is_secure_context()),
        );

        let pipeline_id = self.pipeline_id();
        // If the initial `about:blank` page is the current page, load with replacement enabled,
        // see https://html.spec.whatwg.org/multipage/#the-iframe-element:about:blank-3
        let is_about_blank =
            pipeline_id.is_some() && pipeline_id == self.about_blank_pipeline_id.get();
        let replace = if is_about_blank {
            HistoryEntryReplacement::Enabled
        } else {
            HistoryEntryReplacement::Disabled
        };
        self.navigate_or_reload_child_browsing_context(load_data, replace, can_gc);
    }

    fn create_nested_browsing_context(&self, can_gc: CanGc) {
        println!("create_nested_browsing_context");
        // Synchronously create a new browsing context, which will present
        // `about:blank`. (This is not a navigation.)
        //
        // The pipeline started here will synchronously "completely finish
        // loading", which will then asynchronously call
        // `iframe_load_event_steps`.
        //
        // The precise event timing differs between implementations and
        // remains controversial:
        //
        //  - [Unclear "iframe load event steps" for initial load of about:blank
        //    in an iframe #490](https://github.com/whatwg/html/issues/490)
        //  - [load event handling for iframes with no src may not be web
        //    compatible #4965](https://github.com/whatwg/html/issues/4965)
        //
        let url = ServoUrl::parse("about:blank").unwrap();
        let document = document_from_node(self);
        let window = window_from_node(self);
        let pipeline_id = Some(window.upcast::<GlobalScope>().pipeline_id());
        let load_data = LoadData::new(
            LoadOrigin::Script(document.origin().immutable().clone()),
            url,
            pipeline_id,
            window.upcast::<GlobalScope>().get_referrer(),
            document.get_referrer_policy(),
            Some(window.upcast::<GlobalScope>().is_secure_context()),
        );

        //  let top_level_browsing_context_id = TopLevelBrowsingContextId::new();
        //  let browsing_context_id = top_level_browsing_context_id.into();

        let browsing_context_id = BrowsingContextId::new();
        let top_level_browsing_context_id = window.window_proxy().top_level_browsing_context_id();

        self.pipeline_id.set(None);
        self.pending_pipeline_id.set(None);
        self.top_level_browsing_context_id
            .set(Some(top_level_browsing_context_id));
        self.browsing_context_id.set(Some(browsing_context_id));
        self.start_new_pipeline(
            load_data,
            PipelineType::InitialAboutBlank,
            HistoryEntryReplacement::Disabled,
            can_gc,
        );
    }

    pub(crate) fn destroy_nested_browsing_context(&self) {
        println!("destroy_nested_browsing_context");
        self.pipeline_id.set(None);
        self.pending_pipeline_id.set(None);
        self.about_blank_pipeline_id.set(None);
        self.top_level_browsing_context_id.set(None);
        self.browsing_context_id.set(None);
    }

    pub fn update_pipeline_id(
        &self,
        new_pipeline_id: PipelineId,
        reason: UpdatePipelineIdReason,
        can_gc: CanGc,
    ) {
        println!("update_pipeline_id");
        if self.pending_pipeline_id.get() != Some(new_pipeline_id) &&
            reason == UpdatePipelineIdReason::Navigation
        {
            return;
        }

        self.pipeline_id.set(Some(new_pipeline_id));

        // Only terminate the load blocker if the pipeline id was updated due to a traversal.
        // The load blocker will be terminated for a navigation in iframe_load_event_steps.
        if reason == UpdatePipelineIdReason::Traversal {
            let mut blocker = self.load_blocker.borrow_mut();
            LoadBlocker::terminate(&mut blocker, can_gc);
        }

        self.upcast::<Node>().dirty(NodeDamage::OtherNodeDamage);
        let window = window_from_node(self);
        window.add_pending_reflow();
    }

    fn new_inherited(
        local_name: LocalName,
        prefix: Option<Prefix>,
        document: &Document,
    ) -> HTMLWebViewElement {
        println!("new_inherited");
        HTMLWebViewElement {
            htmlelement: HTMLElement::new_inherited(local_name, prefix, document),
            browsing_context_id: Cell::new(None),
            top_level_browsing_context_id: Cell::new(None),
            pipeline_id: Cell::new(None),
            pending_pipeline_id: Cell::new(None),
            about_blank_pipeline_id: Cell::new(None),
            load_blocker: DomRefCell::new(None),
            throttled: Cell::new(false),
        }
    }

    #[allow(crown::unrooted_must_root)]
    pub fn new(
        local_name: LocalName,
        prefix: Option<Prefix>,
        document: &Document,
        proto: Option<HandleObject>,
    ) -> DomRoot<HTMLWebViewElement> {
        println!("webview: new");
        Node::reflect_node_with_proto(
            Box::new(HTMLWebViewElement::new_inherited(
                local_name, prefix, document,
            )),
            document,
            proto,
        )
    }

    #[inline]
    pub fn pipeline_id(&self) -> Option<PipelineId> {
        println!("pipeline_id");
        self.pipeline_id.get()
    }

    #[inline]
    pub fn browsing_context_id(&self) -> Option<BrowsingContextId> {
        println!("browsing_context_id");
        self.browsing_context_id.get()
    }

    #[inline]
    pub fn top_level_browsing_context_id(&self) -> Option<TopLevelBrowsingContextId> {
        println!("top_level_browsing_context_id");
        self.top_level_browsing_context_id.get()
    }

    pub fn set_throttled(&self, throttled: bool) {
        println!("set_throttled");
        if self.throttled.get() != throttled {
            self.throttled.set(throttled);
        }
    }
}

pub trait HTMLWebViewElementLayoutMethods {
    fn pipeline_id(self) -> Option<PipelineId>;
    fn browsing_context_id(self) -> Option<BrowsingContextId>;
}

impl HTMLWebViewElementLayoutMethods for LayoutDom<'_, HTMLWebViewElement> {
    #[inline]
    fn pipeline_id(self) -> Option<PipelineId> {
        println!("layout: pipeline_id");
        (self.unsafe_get()).pipeline_id.get()
    }

    #[inline]
    fn browsing_context_id(self) -> Option<BrowsingContextId> {
        println!("layout: browsing_context_id");
        (self.unsafe_get()).browsing_context_id.get()
    }
}

impl HTMLWebViewElementMethods for HTMLWebViewElement {
    // https://html.spec.whatwg.org/multipage/#dom-iframe-src
    make_url_getter!(Src, "src");

    // https://html.spec.whatwg.org/multipage/#dom-iframe-src
    make_url_setter!(SetSrc, "src");

    // https://html.spec.whatwg.org/multipage/#dom-iframe-contentwindow
    fn GetContentWindow(&self) -> Option<DomRoot<WindowProxy>> {
        println!("method: GetContentWindow");
        self.browsing_context_id
            .get()
            .and_then(ScriptThread::find_window_proxy)
    }

    // https://html.spec.whatwg.org/multipage/#dom-iframe-contentdocument
    // https://html.spec.whatwg.org/multipage/#concept-bcc-content-document
    fn GetContentDocument(&self) -> Option<DomRoot<Document>> {
        println!("method: GetContentDocument");
        // Step 1.
        let pipeline_id = self.pipeline_id.get()?;

        // Step 2-3.
        // Note that this lookup will fail if the document is dissimilar-origin,
        // so we should return None in that case.
        let document = ScriptThread::find_document(pipeline_id)?;

        // Step 4.
        let current = GlobalScope::current()
            .expect("No current global object")
            .as_window()
            .Document();
        if !current.origin().same_origin_domain(document.origin()) {
            return None;
        }
        // Step 5.
        Some(document)
    }
}

impl VirtualMethods for HTMLWebViewElement {
    fn super_type(&self) -> Option<&dyn VirtualMethods> {
        println!("virtualmethod: super_type");
        Some(self.upcast::<HTMLElement>() as &dyn VirtualMethods)
    }

    fn attribute_mutated(&self, attr: &Attr, mutation: AttributeMutation) {
        println!("virtualmethod: attribute_mutated");
        self.super_type().unwrap().attribute_mutated(attr, mutation);
        match *attr.local_name() {
            local_name!("src") => {
                // https://html.spec.whatwg.org/multipage/#the-iframe-element
                // "Similarly, whenever an iframe element with a non-null nested browsing context
                // but with no srcdoc attribute specified has its src attribute set, changed, or removed,
                // the user agent must process the iframe attributes,"
                // but we can't check that directly, since the child browsing context
                // may be in a different script thread. Instead, we check to see if the parent
                // is in a document tree and has a browsing context, which is what causes
                // the child browsing context to be created.
                if self.upcast::<Node>().is_connected_with_browsing_context() {
                    debug!("iframe src set while in browsing context.");
                    self.process_the_iframe_attributes(ProcessingMode::NotFirstTime, CanGc::note());
                }
            },
            _ => {},
        }
    }

    fn bind_to_tree(&self, context: &BindContext) {
        println!("virtualmethod: bind_to_tree");
        if let Some(s) = self.super_type() {
            s.bind_to_tree(context);
        }

        let tree_connected = context.tree_connected;
        let iframe = Trusted::new(self);
        document_from_node(self).add_delayed_task(task!(IFrameDelayedInitialize: move || {
            let this = iframe.root();
            // https://html.spec.whatwg.org/multipage/#the-iframe-element
            // "When an iframe element is inserted into a document that has
            // a browsing context, the user agent must create a new
            // browsing context, set the element's nested browsing context
            // to the newly-created browsing context, and then process the
            // iframe attributes for the "first time"."
            if this.upcast::<Node>().is_connected_with_browsing_context() {
                debug!("iframe bound to browsing context.");
                debug_assert!(tree_connected, "is_connected_with_bc, but not tree_connected");
                this.create_nested_browsing_context(CanGc::note());
                this.process_the_iframe_attributes(ProcessingMode::FirstTime, CanGc::note());
            }
        }));
    }

    fn unbind_from_tree(&self, context: &UnbindContext) {
        println!("virtualmethod: unbind_from_tree");
        self.super_type().unwrap().unbind_from_tree(context);

        let mut blocker = self.load_blocker.borrow_mut();
        LoadBlocker::terminate(&mut blocker, CanGc::note());

        // https://html.spec.whatwg.org/multipage/#a-browsing-context-is-discarded
        let window = window_from_node(self);
        let (sender, receiver) =
            ProfiledIpc::channel(self.global().time_profiler_chan().clone()).unwrap();

        // Ask the constellation to remove the iframe, and tell us the
        // pipeline ids of the closed pipelines.
        let browsing_context_id = match self.browsing_context_id() {
            None => return warn!("Unbinding already unbound iframe."),
            Some(id) => id,
        };
        debug!("Unbinding frame {}.", browsing_context_id);

        let msg = ScriptMsg::RemoveIFrame(browsing_context_id, sender);
        window
            .upcast::<GlobalScope>()
            .script_to_constellation_chan()
            .send(msg)
            .unwrap();
        let _exited_pipeline_ids = receiver.recv().unwrap();

        // The spec for discarding is synchronous,
        // so we need to discard the browsing contexts now, rather than
        // when the `PipelineExit` message arrives.
        // for exited_pipeline_id in exited_pipeline_ids {
        //     // https://html.spec.whatwg.org/multipage/#a-browsing-context-is-discarded
        //     if let Some(exited_document) = ScriptThread::find_document(exited_pipeline_id) {
        //         debug!(
        //             "Discarding browsing context for pipeline {}",
        //             exited_pipeline_id
        //         );
        //         let exited_window = exited_document.window();
        //         exited_window.discard_browsing_context();
        //         for exited_iframe in exited_document.iter_iframes() {
        //             debug!("Discarding nested browsing context");
        //             exited_iframe.destroy_nested_browsing_context();
        //         }
        //     }
        // }

        // Resetting the pipeline_id to None is required here so that
        // if this iframe is subsequently re-added to the document
        // the load doesn't think that it's a navigation, but instead
        // a new iframe. Without this, the constellation gets very
        // confused.
        self.destroy_nested_browsing_context();
    }
}
