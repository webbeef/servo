/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

use std::cell::Cell;

use cssparser::{Parser, ParserInput, StyleSheetParser};
use dom_struct::dom_struct;
use js::rust::HandleObject;
use servo_arc::Arc;
use servo_url::ServoUrl;
use style::context::QuirksMode;
use style::error_reporting::{ContextualParseError, ParseErrorReporter};
use style::media_queries::MediaList as StyleMediaList;
use style::parser::ParserContext;
use style::shared_lock::SharedRwLock;
use style::stylesheets::{
    AllowImportRules, CssRule, CssRuleType, CssRuleTypes, Origin, State,
    Stylesheet as StyleStyleSheet, StylesheetLoader as StyleStylesheetLoader, TopLevelRuleParser,
    UrlExtraData,
};
use style_traits::ParsingMode;

use crate::dom::bindings::codegen::Bindings::CSSStyleSheetBinding::{
    CSSStyleSheetInit, CSSStyleSheetMethods,
};
use crate::dom::bindings::codegen::Bindings::DocumentBinding::Document_Binding::DocumentMethods;
use crate::dom::bindings::codegen::Bindings::WindowBinding::Window_Binding::WindowMethods;
use crate::dom::bindings::error::{Error, ErrorResult, Fallible};
use crate::dom::bindings::inheritance::Castable;
use crate::dom::bindings::reflector::{
    reflect_dom_object, reflect_dom_object_with_proto, DomObject,
};
use crate::dom::bindings::root::{DomRoot, MutNullableDom};
use crate::dom::bindings::str::{DOMString, USVString};
use crate::dom::cssrulelist::{CSSRuleList, RulesSource};
use crate::dom::element::Element;
use crate::dom::globalscope::GlobalScope;
use crate::dom::htmlelement::HTMLElement;
use crate::dom::medialist::MediaList;
use crate::dom::node::{stylesheets_owner_from_node, Node};
use crate::dom::stylesheet::StyleSheet;
use crate::dom::window::Window;
use crate::script_runtime::CanGc;
use crate::stylesheet_loader::StylesheetLoader;

#[dom_struct]
pub struct CSSStyleSheet {
    stylesheet: StyleSheet,
    owner: MutNullableDom<Element>,
    rulelist: MutNullableDom<CSSRuleList>,
    #[ignore_malloc_size_of = "Arc"]
    #[no_trace]
    style_stylesheet: Arc<StyleStyleSheet>,
    origin_clean: Cell<bool>,
    constructed: Cell<bool>,
    #[no_trace]
    medialist: StyleMediaList,
}

impl CSSStyleSheet {
    fn new_inherited(
        owner: &Element,
        type_: DOMString,
        href: Option<DOMString>,
        title: Option<DOMString>,
        stylesheet: Arc<StyleStyleSheet>,
    ) -> CSSStyleSheet {
        CSSStyleSheet {
            stylesheet: StyleSheet::new_inherited(type_, href, title),
            owner: MutNullableDom::new(Some(owner)),
            rulelist: MutNullableDom::new(None),
            style_stylesheet: stylesheet,
            origin_clean: Cell::new(true),
            constructed: Cell::new(false),
            medialist: StyleMediaList::empty(),
        }
    }

    #[allow(crown::unrooted_must_root)]
    pub fn new(
        window: &Window,
        owner: &Element,
        type_: DOMString,
        href: Option<DOMString>,
        title: Option<DOMString>,
        stylesheet: Arc<StyleStyleSheet>,
    ) -> DomRoot<CSSStyleSheet> {
        reflect_dom_object(
            Box::new(CSSStyleSheet::new_inherited(
                owner, type_, href, title, stylesheet,
            )),
            window,
        )
    }

    fn create_media_list(window: &Window, value: &USVString) -> StyleMediaList {
        if value.is_empty() {
            return StyleMediaList::empty();
        }

        let mut input = ParserInput::new(&value);
        let mut parser = Parser::new(&mut input);
        let url_data = UrlExtraData(window.get_url().get_arc());
        let context = ParserContext::new(
            Origin::Author,
            &url_data,
            Some(CssRuleType::Media),
            ParsingMode::DEFAULT,
            window.Document().quirks_mode(),
            /* namespaces = */ Default::default(),
            window.css_error_reporter(),
            None,
        );
        StyleMediaList::parse(&context, &mut parser)
    }

    #[allow(non_snake_case)]
    pub fn Constructor(
        window: &Window,
        proto: Option<HandleObject>,
        can_gc: CanGc,
        init: &CSSStyleSheetInit,
    ) -> Result<DomRoot<CSSStyleSheet>, Error> {
        let stylesheet = StyleSheet::new_inherited(DOMString::default(), None, None);

        let lock = SharedRwLock::new();

        // Use init.baseURL to set the sheet stylesheet base URL
        let url = if let Some(base_url) = &init.baseURL {
            ServoUrl::parse(&base_url).map_err(|_| Error::NotAllowed)?
        } else {
            window.get_url()
        };

        let medialist = Self::create_media_list(window, &init.media);

        let owner = window
            .Document()
            .GetDocumentElement()
            .and_then(DomRoot::downcast::<HTMLElement>);
        let loader = owner
            .as_ref()
            .map(|element| StylesheetLoader::for_element(element))
            .unwrap();

        let style_stylesheet = StyleStyleSheet::from_bytes(
            &[],
            UrlExtraData(url.get_arc()),
            None, /* protocol_encoding_label */
            None, /* environment_encoding */
            Origin::Author,
            medialist.clone(),
            lock,
            Some(&loader),
            window.css_error_reporter(),
            window.Document().quirks_mode(),
        );

        let css_stylesheet = CSSStyleSheet {
            stylesheet,
            owner: MutNullableDom::new(None),
            rulelist: MutNullableDom::new(None),
            style_stylesheet: servo_arc::Arc::new(style_stylesheet),
            origin_clean: Cell::new(true),
            constructed: Cell::new(true),
            medialist,
        };
        css_stylesheet.set_disabled(init.disabled);

        Ok(reflect_dom_object_with_proto(
            Box::new(css_stylesheet),
            window.upcast::<GlobalScope>(),
            proto,
            can_gc,
        ))
    }

    fn rulelist(&self) -> DomRoot<CSSRuleList> {
        self.rulelist.or_init(|| {
            let rules = self.style_stylesheet.contents.rules.clone();
            CSSRuleList::new(self.global().as_window(), self, RulesSource::Rules(rules))
        })
    }

    pub fn disabled(&self) -> bool {
        self.style_stylesheet.disabled()
    }

    pub fn get_owner(&self) -> Option<DomRoot<Element>> {
        self.owner.get()
    }

    pub fn set_disabled(&self, disabled: bool) {
        if self.style_stylesheet.set_disabled(disabled) && self.get_owner().is_some() {
            stylesheets_owner_from_node(self.get_owner().unwrap().upcast::<Node>())
                .invalidate_stylesheets();
        }
    }

    pub fn set_owner(&self, value: Option<&Element>) {
        self.owner.set(value);
    }

    pub fn shared_lock(&self) -> &SharedRwLock {
        &self.style_stylesheet.shared_lock
    }

    pub fn style_stylesheet(&self) -> &StyleStyleSheet {
        &self.style_stylesheet
    }

    pub fn set_origin_clean(&self, origin_clean: bool) {
        self.origin_clean.set(origin_clean);
    }

    pub fn medialist(&self) -> DomRoot<MediaList> {
        MediaList::new(
            self.global().as_window(),
            self,
            self.style_stylesheet().media.clone(),
        )
    }
}

/// Simplified version of StyleStylesheet::parse_rules which is not public.
fn parse_rules(
    css: &str,
    url_data: &UrlExtraData,
    origin: Origin,
    shared_lock: &SharedRwLock,
    stylesheet_loader: Option<&dyn StyleStylesheetLoader>,
    error_reporter: Option<&dyn ParseErrorReporter>,
    quirks_mode: QuirksMode,
    allow_import_rules: AllowImportRules,
) -> Vec<CssRule> {
    let mut input = ParserInput::new(css);
    let mut input = Parser::new(&mut input);

    let context = ParserContext::new(
        origin,
        url_data,
        None,
        ParsingMode::DEFAULT,
        quirks_mode,
        /* namespaces = */ Default::default(),
        error_reporter,
        /* use_counters */ None,
    );

    let mut rule_parser = TopLevelRuleParser {
        shared_lock,
        loader: stylesheet_loader,
        context,
        state: State::Start,
        dom_error: None,
        insert_rule_context: None,
        allow_import_rules,
        declaration_parser_state: Default::default(),
        error_reporting_state: Default::default(),
        rules: Vec::new(),
    };

    {
        let mut iter = StyleSheetParser::new(&mut input, &mut rule_parser);
        while let Some(result) = iter.next() {
            if let Err((error, slice)) = result {
                let location = error.location;
                let error = ContextualParseError::InvalidRule(slice, error);
                iter.parser.context.log_css_error(location, error);
            }
        }
    }

    rule_parser.rules
}

impl CSSStyleSheetMethods for CSSStyleSheet {
    // https://drafts.csswg.org/cssom/#dom-cssstylesheet-cssrules
    fn GetCssRules(&self) -> Fallible<DomRoot<CSSRuleList>> {
        if !self.origin_clean.get() {
            return Err(Error::Security);
        }
        Ok(self.rulelist())
    }

    // https://drafts.csswg.org/cssom/#dom-cssstylesheet-insertrule
    fn InsertRule(&self, rule: DOMString, index: u32) -> Fallible<u32> {
        if !self.origin_clean.get() {
            return Err(Error::Security);
        }
        self.rulelist()
            .insert_rule(&rule, index, CssRuleTypes::default(), None)
    }

    // https://drafts.csswg.org/cssom/#dom-cssstylesheet-deleterule
    fn DeleteRule(&self, index: u32) -> ErrorResult {
        if !self.origin_clean.get() {
            return Err(Error::Security);
        }
        self.rulelist().remove_rule(index)
    }

    // https://drafts.csswg.org/cssom/#synchronously-replace-the-rules-of-a-cssstylesheet
    fn ReplaceSync(&self, text: USVString) -> Result<(), Error> {
        // Step 1.
        if !self.constructed.get() {
            return Err(Error::NotAllowed);
        }

        // Step 2.
        let global = self.global();
        let window = global.as_window();

        // let sheet_contents = StylesheetContents::from_str(
        let new_rules = parse_rules(
            &text,
            &UrlExtraData(window.get_url().get_arc()),
            Origin::Author,
            &self.shared_lock(),
            None,
            None,
            window.Document().quirks_mode(),
            AllowImportRules::No, // Step 3.
        );

        // Step 4.
        let rules = self.rulelist();
        let _ = rules.remove_all_rules();

        rules.add_css_rules(&new_rules);

        Ok(())
    }
}
