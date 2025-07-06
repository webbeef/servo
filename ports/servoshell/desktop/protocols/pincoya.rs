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

//! Loads resources using a mapping from well-known shortcuts to resource: urls.
//! Recognized shortcuts:
//! - pincoya:atproto/browser

use std::future::Future;
use std::pin::Pin;

use servo::protocol_handler::{DoneChannel, FetchContext, ProtocolHandler, Request, Response};

use crate::desktop::protocols::resource::ResourceProtocolHandler;

#[derive(Default)]
pub struct PincoyaProtocolHandler {}

impl ProtocolHandler for PincoyaProtocolHandler {
    fn load(
        &self,
        request: &mut Request,
        done_chan: &mut DoneChannel,
        context: &FetchContext,
    ) -> Pin<Box<dyn Future<Output = Response> + Send>> {
        let url = request.current_url();

        match url.path() {
            "atproto/browser" => ResourceProtocolHandler::response_for_path(
                request,
                done_chan,
                context,
                "/pincoya/atproto/browser.html",
            ),
            "atproto/account" => ResourceProtocolHandler::response_for_path(
                request,
                done_chan,
                context,
                "/pincoya/atproto/account.html",
            ),
            "" => ResourceProtocolHandler::response_for_path(
                request,
                done_chan,
                context,
                "/pincoya/about.html",
            ),
            _ => Box::pin(std::future::ready(Response::network_internal_error(
                "Invalid pincoya: url",
            ))),
        }
    }
}
