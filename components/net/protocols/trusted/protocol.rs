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

use std::future::{self, Future};
use std::pin::Pin;

use http::StatusCode;
use net_traits::fetch::utils::{SimpleFetchTarget, http_response, rewrite_response_url};
use net_traits::request::{Referrer, Request, RequestBuilder};
use net_traits::response::{Response, ResponseBody};
use serde::de::DeserializeOwned;
use servo_url::ServoUrl;

use super::zone_description::{TrustedZoneUrl, ZoneFile};
use crate::fetch::methods::{DoneChannel, FetchContext, fetch};
use crate::protocols::ProtocolHandler;

async fn fetch_json<T: DeserializeOwned>(
    url: ServoUrl,
    context: &FetchContext,
) -> Result<T, StatusCode> {
    let request = RequestBuilder::new(None, url.clone(), Referrer::NoReferrer)
        .origin(url.origin())
        .build();
    let mut target = SimpleFetchTarget::default();
    let response = fetch(request, &mut target, context).await;
    if response.is_network_error() {
        return Err(response.status.code());
    }
    let ResponseBody::Done(_) = *response.body.lock() else {
        return Err(response.status.code());
    };
    serde_json::from_slice::<T>(&target.body).map_err(|_| StatusCode::UNPROCESSABLE_ENTITY)
}

#[derive(Default)]
pub struct TrustedProtocolHandler {}

impl ProtocolHandler for TrustedProtocolHandler {
    fn load(
        &self,
        request: &mut Request,
        _done_chan: &mut DoneChannel,
        context: &FetchContext,
    ) -> Pin<Box<dyn Future<Output = Response> + Send>> {
        let url = request.current_url();

        // Reject paths that could lead to url rewriting attacks.
        let path = url.path();
        if path.contains("..") {
            return Box::pin(future::ready(http_response(
                url,
                StatusCode::BAD_REQUEST,
                "Invalid path",
            )));
        }

        // url looks like: trusted://demo.trustedweb.org:8000/port8800/index.html

        let Some((zone_name, user_id)) = url.zone_and_user() else {
            return Box::pin(future::ready(http_response(
                url,
                StatusCode::BAD_REQUEST,
                "Invalid trusted:// url",
            )));
        };

        let port = match url.port() {
            Some(value) => &format!(":{value}"),
            None => "",
        };
        let Ok(zone_base_url) = ServoUrl::parse(&format!("http://{user_id}{port}")) else {
            return Box::pin(future::ready(http_response(
                url,
                StatusCode::BAD_REQUEST,
                "Invalid trusted user_id",
            )));
        };

        let Ok(zone_url) = zone_base_url.join(".well-known/trusted-zones.json") else {
            return Box::pin(future::ready(http_response(
                url,
                StatusCode::BAD_REQUEST,
                "Invalid zone url",
            )));
        };

        let context2 = context.clone();
        Box::pin(async move {
            // Fetch the zone file.
            let zone_file = match fetch_json::<ZoneFile>(zone_url, &context2).await {
                Ok(zone_file) => zone_file,
                Err(status) => return http_response(url, status, "Failed to fetch zone file!"),
            };

            // Look for the zone by name.
            let Some(zone) = zone_file.find_zone(&zone_name) else {
                return http_response(url, StatusCode::BAD_REQUEST, "Unkwnon zone name");
            };

            let Some(mut segments) = url.path_segments() else {
                return http_response(url, StatusCode::BAD_REQUEST, "Invalid path segments");
            };

            let Some(mapping_path) = segments.next() else {
                return http_response(url, StatusCode::BAD_REQUEST, "Failed to find first segment");
            };

            let Some(mapping) = zone.find_mapping(mapping_path) else {
                return http_response(url, StatusCode::BAD_REQUEST, "Invalid Mapping Name");
            };

            // Build the final url by using the mapping source as the base url and the remaining path segments as the path.
            let Ok(base_url) = ServoUrl::parse(&mapping.source) else {
                return http_response(url, StatusCode::BAD_REQUEST, "Invalid source url");
            };

            let Ok(inner_url) =
                base_url.join(&segments.fold("".to_owned(), |mut current, item| {
                    current.push_str(item);
                    current
                }))
            else {
                return http_response(url, StatusCode::BAD_REQUEST, "Failed to build final url");
            };

            let request = RequestBuilder::new(None, inner_url.clone(), Referrer::NoReferrer)
                .origin(inner_url.origin())
                .build();
            let mut target = SimpleFetchTarget::default();

            let fetched = fetch(request, &mut target, &context2).await;
            rewrite_response_url(url, inner_url, fetched)
        })
    }

    fn is_fetchable(&self) -> bool {
        true
    }

    fn is_secure(&self) -> bool {
        true
    }
}
