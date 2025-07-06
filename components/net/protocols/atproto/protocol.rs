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
use std::num::NonZeroUsize;
use std::pin::Pin;
use std::sync::Arc;

use atproto_identity::resolve::HickoryDnsResolver;
use atproto_identity::storage_lru::LruDidDocumentStorage;
use content_security_policy::percent_encoding::percent_decode_str;
use http::header::CONTENT_TYPE;
use http::{Method, StatusCode};
use log::info;
use net_traits::fetch::utils::http_response;
use net_traits::request::Request;
use net_traits::response::Response;
use servo_url::ServoUrl;
use sync_wrapper::SyncWrapper;

use crate::atproto::pds::get_endpoint_for_subject;
use crate::atproto::xrpc::XrpcClient;
use crate::fetch::methods::{DoneChannel, FetchContext};
use crate::protocols::ProtocolHandler;

pub struct AtProtocolHandler {
    document_storage: LruDidDocumentStorage,
    dns_resolver: Arc<HickoryDnsResolver>,
}

impl Default for AtProtocolHandler {
    fn default() -> Self {
        Self {
            document_storage: LruDidDocumentStorage::new(NonZeroUsize::new(1000).unwrap()),
            dns_resolver: Arc::new(HickoryDnsResolver::create_resolver(Default::default())),
        }
    }
}

fn maybe_bad_request(url: &ServoUrl, reason: &str, response: Result<Response, ()>) -> Response {
    response.unwrap_or_else(|_| http_response(url.clone(), StatusCode::BAD_REQUEST, reason))
}

/// Implementation of the at:// protocol handler.
/// at://did:plc:44ybard66vv44zksje25o7dz/app.bsky.feed.post/3jwdwj2ctlk26
/// at://bnewbold.bsky.team/app.bsky.feed.post/3jwdwj2ctlk26
impl ProtocolHandler for AtProtocolHandler {
    fn load(
        &self,
        request: &mut Request,
        _done_chan: &mut DoneChannel,
        context: &FetchContext,
    ) -> Pin<Box<dyn Future<Output = Response> + Send>> {
        let url = request.current_url();

        let method = request.method.clone();
        if method != Method::GET && method != Method::POST && method != Method::DELETE {
            return Box::pin(future::ready(http_response(
                url,
                StatusCode::BAD_REQUEST,
                "Invalid method",
            )));
        }

        let context2 = context.clone();
        let document_storage = self.document_storage.clone();
        let dns_resolver = Arc::clone(&self.dns_resolver);
        let mut request = SyncWrapper::new(request.clone());
        Box::pin(async move {
            // Get the host and run percent decoding.
            let Some(host) = url.host_str() else {
                return http_response(url, StatusCode::BAD_REQUEST, "No host");
            };

            let Ok(subject) = percent_decode_str(host).decode_utf8() else {
                return http_response(url, StatusCode::BAD_REQUEST, "Host decoding error");
            };

            let Ok((endpoint_url, document)) =
                get_endpoint_for_subject(&subject, Some(document_storage), Some(dns_resolver))
                    .await
            else {
                return http_response(
                    url,
                    StatusCode::BAD_REQUEST,
                    "Failed to resolve endpoint for subject",
                );
            };

            let (collection, rkey) = if let Some(mut segments) = url.path_segments() {
                let collection = segments.next();
                let rkey = segments.next();
                (collection, rkey)
            } else {
                (None, None)
            };

            let client = XrpcClient::new(endpoint_url, url.clone(), &document.id, context2.clone());

            if method == Method::GET {
                let (response, reason) = match (collection, rkey) {
                    (None, _) => {
                        // If we have no collection, send a com.atproto.repo.describeRepo request.
                        (client.describe_repo().await, "Failed to describe repo")
                    },
                    (Some(coll), None) => {
                        // If we have a collection but no rkey, send a com.atproto.repo.listRecords request.
                        (client.list_records(coll).await, "Failed to list records")
                    },
                    (Some(coll), Some(rkey)) => {
                        // Both collection and rkey are present, send a com.atproto.repo.getRecord request.
                        (client.get_record(coll, rkey).await, "Failed to get record")
                    },
                };
                maybe_bad_request(&url, reason, response)
            } else if method == Method::POST {
                let request = request.get_mut();

                // Check the mandatory content type.
                let Some(header_value) = request.headers.get(CONTENT_TYPE) else {
                    return http_response(url, StatusCode::BAD_REQUEST, "Content-Type missing");
                };
                let Ok(content_type) = header_value.to_str() else {
                    return http_response(url, StatusCode::BAD_REQUEST, "Invalid Content-Type");
                };
                info!("CONTENT_TYPE is: {:?}", content_type);

                let Some(ref body) = request.body else {
                    return http_response(url, StatusCode::BAD_REQUEST, "Missing body");
                };

                match (collection, rkey) {
                    (None, _) => {
                        // When no collection is specified, this is a blob update.
                        let response = client.upload_blob(content_type, body.clone()).await;
                        maybe_bad_request(&url, "Failed to create record", response)
                    },
                    (Some(_), Some(_)) => {
                        // TODO: allow rkey when it makes sense
                        http_response(
                            url,
                            StatusCode::BAD_REQUEST,
                            "Invalid parameters: expects repo and no rkey",
                        )
                    },
                    (Some(collection), None) => {
                        let response = client.create_record(collection, body.clone()).await;
                        maybe_bad_request(&url, "Failed to create record", response)
                    },
                }
            } else if method == Method::DELETE {
                // Only support record deletion requiring collection and rkey.
                if let (Some(collection), Some(rkey)) = (collection, rkey) {
                    let response = client.delete_record(collection, rkey).await;
                    maybe_bad_request(&url, "Failed to delete record", response)
                } else {
                    http_response(
                        url,
                        StatusCode::BAD_REQUEST,
                        "Missing parameters to delete record",
                    )
                }
            } else {
                http_response(
                    url,
                    StatusCode::BAD_REQUEST,
                    &format!("{method} not implemented yet"),
                )
            }
        })
    }

    fn is_fetchable(&self) -> bool {
        true
    }

    fn is_secure(&self) -> bool {
        true
    }
}
