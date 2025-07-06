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

use async_recursion::async_recursion;
use constellation_traits::{AtProtoError, AtProtoErrorKind, AtProtoRefreshSession};
use headers::{Authorization, HeaderMapExt};
use http::header::CONTENT_TYPE;
use http::{HeaderMap, Method};
use log::error;
use net_traits::AtProtoSessionState;
use net_traits::fetch::utils::{SimpleFetchTarget, rewrite_response_url};
use net_traits::request::{
    CredentialsMode, Referrer, Request, RequestBody, RequestBuilder,
    create_request_body_with_content,
};
use net_traits::response::{Response, ResponseType};
use serde_json::json;
use servo_url::ServoUrl;

use crate::fetch::methods::{FetchContext, fetch};

pub struct XrpcClient {
    endpoint: ServoUrl,
    at_url: ServoUrl,
    repo_did: String,
    context: FetchContext,
}

impl XrpcClient {
    pub fn new(
        endpoint: ServoUrl,
        at_url: ServoUrl,
        repo_did: &str,
        context: FetchContext,
    ) -> Self {
        Self {
            endpoint,
            at_url,
            repo_did: repo_did.into(),
            context,
        }
    }

    fn build_request(
        &self,
        xrpc_call: &str,
        params: &[(&str, &str)],
        requires_auth: bool,
        body: Option<RequestBody>,
        method: Option<Method>,
        headers: Option<HeaderMap>,
    ) -> (Request, ServoUrl) {
        let mut xrpc_url = self
            .endpoint
            .join(xrpc_call)
            .map_err(|_| ())
            .expect("Failed to build xrpc url");
        if !params.is_empty() {
            let mut url_params = xrpc_url.as_mut_url().query_pairs_mut();
            for param in params {
                url_params.append_pair(param.0, param.1);
            }
        }
        let mut builder = RequestBuilder::new(None, xrpc_url.clone(), Referrer::NoReferrer)
            .method(method.unwrap_or(Method::GET))
            .headers(headers.unwrap_or_default())
            .origin(xrpc_url.origin())
            .body(body);
        if requires_auth {
            builder = builder.credentials_mode(CredentialsMode::Include);
        }
        (builder.build(), xrpc_url)
    }

    // TODO: proper errors
    #[async_recursion]
    async fn fetch(
        &self,
        xrpc_call: &str,
        params: &[(&str, &str)],
        requires_auth: bool,
        body: Option<RequestBody>,
        method: Option<Method>,
        headers: Option<HeaderMap>,
    ) -> Result<Response, ()> {
        let (request, xrpc_url) = self.build_request(
            xrpc_call,
            params,
            requires_auth,
            body.clone(),
            method.clone(),
            headers.clone(),
        );
        let mut target = SimpleFetchTarget::default();

        let mut xrpc_response = fetch(request, &mut target, &self.context).await;

        if xrpc_response.status.raw_code() == 400 {
            error!(
                "Error 400 for {} (auth: {}): {}",
                xrpc_call,
                requires_auth,
                String::from_utf8_lossy(&target.body)
            );
            // Check if the error is an ExpiredToken one.
            let Ok(error_response) = serde_json::from_slice::<AtProtoError>(&target.body) else {
                error!("Unexpected error response.");
                return Err(());
            };

            if error_response.error != AtProtoErrorKind::ExpiredToken {
                error!("Unexpected error kind");
                return Err(());
            }

            let atproto_session = {
                let atproto_session = self.context.state.atproto_session.read();
                let Some(ref atproto_session) = *atproto_session else {
                    return Err(());
                };
                atproto_session.clone()
            };

            // Try to refresh the auth token and retry the request.
            let mut refresh_headers = HeaderMap::new();
            refresh_headers
                .typed_insert(Authorization::bearer(&atproto_session.refresh_jwt).unwrap());
            let (refesh_request, _xrpc_url) = self.build_request(
                "/xrpc/com.atproto.server.refreshSession",
                &[],
                false,
                None,
                Some(Method::POST),
                Some(refresh_headers),
            );
            let mut target = SimpleFetchTarget::default();

            let refresh_response = fetch(refesh_request, &mut target, &self.context).await;
            if refresh_response.status.is_success() {
                let Ok(new_session) = serde_json::from_slice::<AtProtoRefreshSession>(&target.body)
                else {
                    error!("Unexpected refreshSession response.");
                    return Err(());
                };

                // Update the http state.
                let session_state = AtProtoSessionState {
                    endpoint: self.endpoint.clone(),
                    access_jwt: new_session.access_jwt,
                    refresh_jwt: new_session.refresh_jwt,
                };
                self.context
                    .state
                    .update_atproto_session(Some(session_state));

                // TODO: update jwts storage.

                // Replay the call
                self.fetch(xrpc_call, params, requires_auth, body, method, headers)
                    .await
            } else {
                Err(())
            }
        } else {
            xrpc_response.response_type = ResponseType::Basic;
            Ok(rewrite_response_url(
                self.at_url.clone(),
                xrpc_url,
                xrpc_response,
            ))
        }
    }

    // GET requests
    async fn fetch_get(&self, xprc_call: &str, params: &[(&str, &str)]) -> Result<Response, ()> {
        self.fetch(xprc_call, params, false, None, None, None).await
    }

    pub async fn describe_repo(&self) -> Result<Response, ()> {
        self.fetch_get(
            "/xrpc/com.atproto.repo.describeRepo",
            &[("repo", &self.repo_did)],
        )
        .await
    }

    pub async fn list_records(&self, collection: &str) -> Result<Response, ()> {
        self.fetch_get(
            "/xrpc/com.atproto.repo.listRecords",
            &[("repo", &self.repo_did), ("collection", collection)],
        )
        .await
    }

    pub async fn get_record(&self, collection: &str, rkey: &str) -> Result<Response, ()> {
        // If the collection name is "com.atproto.sync.blob", this is a link to a blob.
        if collection == "com.atproto.sync.blob" {
            return self
                .fetch_get(
                    "/xrpc/com.atproto.sync.getBlob",
                    &[("did", &self.repo_did), ("cid", rkey)],
                )
                .await;
        }

        self.fetch_get(
            "/xrpc/com.atproto.repo.getRecord",
            &[
                ("repo", &self.repo_did),
                ("collection", collection),
                ("rkey", rkey),
            ],
        )
        .await
    }

    pub async fn delete_record(&self, collection: &str, rkey: &str) -> Result<Response, ()> {
        let mut headers = HeaderMap::new();
        headers.insert(CONTENT_TYPE, "application/json".parse().unwrap());

        // Build the JSON body.
        let json = json!({
            "repo": self.repo_did,
            "collection": collection,
            "rkey": rkey
        });
        let body = create_request_body_with_content(&json.to_string());

        self.fetch(
            "/xrpc/com.atproto.repo.deleteRecord",
            &[],
            true,
            Some(body),
            Some(Method::POST),
            Some(headers),
        )
        .await
    }

    pub async fn create_record(&self, collection: &str, body: RequestBody) -> Result<Response, ()> {
        let mut headers = HeaderMap::new();
        headers.insert(CONTENT_TYPE, "application/json".parse().unwrap());

        self.fetch(
            "/xrpc/com.atproto.repo.createRecord",
            &[("repo", &self.repo_did), ("collection", collection)],
            true,
            Some(body),
            Some(Method::POST),
            Some(headers),
        )
        .await
    }

    pub async fn upload_blob(&self, content_type: &str, body: RequestBody) -> Result<Response, ()> {
        let mut headers = HeaderMap::new();
        headers.insert(CONTENT_TYPE, content_type.parse().unwrap());

        self.fetch(
            "/xrpc/com.atproto.repo.uploadBlob",
            &[],
            true,
            Some(body),
            Some(Method::POST),
            Some(headers),
        )
        .await
    }
}
