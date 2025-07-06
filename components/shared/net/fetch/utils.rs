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

use std::fmt::Debug;

use content_security_policy::{self as csp};
use http::{Method, StatusCode};
use parking_lot::Mutex;
use serde::de::DeserializeOwned;
use serde_json::json;
use servo_arc::Arc;
use servo_url::ServoUrl;
use sync_wrapper::SyncWrapper;

use crate::header::CONTENT_TYPE;
use crate::http_status::HttpStatus;
use crate::request::{CredentialsMode, Referrer, Request, create_request_body_with_content};
use crate::response::{Response, ResponseBody};
use crate::{
    CoreResourceThread, FetchMetadata, FetchResponseMsg, FetchTaskTarget, HeaderMap, HeaderValue,
    RequestBuilder, ResourceFetchTiming, ResourceTimingType, fetch_async,
};

/// A FetchTaskTarget that doesn't do anything.
#[derive(Default)]
pub struct SimpleFetchTarget {
    pub body: Vec<u8>,
}

impl FetchTaskTarget for SimpleFetchTarget {
    fn process_request_body(&mut self, _: &Request) {}
    fn process_request_eof(&mut self, _: &Request) {}
    fn process_response(&mut self, _: &Request, _: &Response) {}
    fn process_response_chunk(&mut self, _: &Request, payload: Vec<u8>) {
        self.body.extend(payload);
    }
    fn process_response_eof(&mut self, _: &Request, response: &Response) {
        // TODO: figure out a way to not clone.
        *response.body.lock() = ResponseBody::Done(self.body.clone());
    }
    fn process_csp_violations(&mut self, _: &Request, _: Vec<csp::Violation>) {}
}

/// Rewrites a response to replace the url.
pub fn rewrite_response_url(url: ServoUrl, inner_url: ServoUrl, fetched: Response) -> Response {
    let mut response = Response::new(
        url.clone(),
        ResourceFetchTiming::new(ResourceTimingType::Resource),
    );
    response.response_type = fetched.response_type;
    response.termination_reason = fetched.termination_reason;
    response.url_list = fetched
        .url_list
        .into_iter()
        .map(|list_url| {
            if list_url == inner_url {
                url.clone()
            } else {
                list_url.clone()
            }
        })
        .collect();
    response.status = fetched.status;
    response.headers = fetched.headers;
    response.body = fetched.body;
    response.cache_state = fetched.cache_state;
    response.https_state = fetched.https_state;
    response.referrer = fetched.referrer;
    response.referrer_policy = fetched.referrer_policy;
    response.cors_exposed_header_name_list = fetched.cors_exposed_header_name_list;
    response.location_url = fetched.location_url;
    response.internal_response = fetched.internal_response.map(|internal_response| {
        Box::new(rewrite_response_url(url, inner_url, *internal_response))
    });
    response.return_internal = fetched.return_internal;
    response.aborted = fetched.aborted;
    response.resource_timing = fetched.resource_timing;
    response.range_requested = fetched.range_requested;
    response
}

/// Creates a http response with the given status code and the message as the body.
pub fn http_response(url: ServoUrl, status: StatusCode, message: &str) -> Response {
    let mut response = Response::new(url, ResourceFetchTiming::new(ResourceTimingType::Resource));

    response.status = HttpStatus::new(status, vec![]);
    let body = ResponseBody::Done(message.as_bytes().to_vec());
    response.body = Arc::new(Mutex::new(body));

    response
}

// A simple fetch response.
#[derive(Default, Clone)]
pub struct FetchResponse {
    pub metadata: Option<FetchMetadata>,
    pub data: Arc<Vec<u8>>,
}

// pub type BoxedResponseCallback = Box<dyn FnMut(FetchResponse) + Send + 'static>;

// pub fn fetch_url(
//     url: ServoUrl,
//     params: &[(&str, &str)],
//     resource_thread: &CoreResourceThread,
//     mut callback: BoxedResponseCallback,
// ) {
//     let mut full_url = url.clone();
//     {
//         let mut url_params = full_url.as_mut_url().query_pairs_mut();
//         for param in params {
//             url_params.append_pair(param.0, param.1);
//         }
//     }

//     let request = RequestBuilder::new(None, full_url, Referrer::NoReferrer);

//     let mut state: FetchResponse = Default::default();

//     fetch_async(
//         resource_thread,
//         request,
//         None,
//         Box::new(move |response_message| match response_message {
//             FetchResponseMsg::ProcessResponseChunk(_id, chunk) => {
//                 Arc::get_mut(&mut state.data).unwrap().extend(chunk);
//             },
//             FetchResponseMsg::ProcessResponse(_id, Ok(metadata)) => state.metadata = Some(metadata),
//             FetchResponseMsg::ProcessResponseEOF(_id, _timing) => {
//                 callback(state.clone());
//             },
//             _ => {},
//         }),
//     );
// }

#[derive(Debug)]
pub enum FetchJsonError<T: DeserializeOwned + Debug> {
    Json(String),
    NoMetadata,
    NoContent,
    Other(T),
    Network,
}

pub async fn fetch_json<S: DeserializeOwned, E: DeserializeOwned + Debug>(
    url: ServoUrl,
    params: &[(&str, &str)],
    resource_thread: SyncWrapper<CoreResourceThread>,
    method: Method,
    headers: Option<HeaderMap>,
    requires_auth: bool,
) -> Result<S, FetchJsonError<E>> {
    let mut full_url = url.clone();

    if method == Method::GET && !params.is_empty() {
        let mut url_params = full_url.as_mut_url().query_pairs_mut();
        for param in params {
            url_params.append_pair(param.0, param.1);
        }
    }

    let mut headers = headers.unwrap_or_default();

    let body = if method == Method::POST && !params.is_empty() {
        headers.insert(CONTENT_TYPE, HeaderValue::from_static("application/json"));
        // Create a JSON encoded body.
        let mut object = serde_json::Map::new();
        for param in params {
            object.insert(param.0.into(), json!(param.1));
        }
        let value = serde_json::Value::Object(object);
        Some(create_request_body_with_content(
            &serde_json::to_string(&value).unwrap_or("{}".to_owned()),
        ))
    } else {
        None
    };

    let mut builder = RequestBuilder::new(None, full_url.clone(), Referrer::NoReferrer)
        .method(method)
        .headers(headers)
        .origin(full_url.origin())
        .body(body);

    if requires_auth {
        builder = builder.credentials_mode(CredentialsMode::Include);
    }

    let mut state: FetchResponse = Default::default();

    let (tx, rx) = tokio::sync::oneshot::channel();

    let mut tx = Some(tx);

    fetch_async(
        &resource_thread.into_inner(),
        builder,
        None,
        Box::new(move |response_message| match response_message {
            FetchResponseMsg::ProcessResponseChunk(_id, chunk) => {
                Arc::get_mut(&mut state.data).unwrap().extend(chunk.0)
            },
            FetchResponseMsg::ProcessResponse(_id, Ok(metadata)) => state.metadata = Some(metadata),
            FetchResponseMsg::ProcessResponseEOF(_id, _timing) => {
                let _ = tx.take().unwrap().send(state.clone());
            },
            _ => {},
        }),
    );

    let response = rx.await.unwrap();

    let Some(metadata) = response.metadata else {
        return Err(FetchJsonError::NoMetadata);
    };

    let status = &metadata.metadata().status;

    if status.is_success() {
        if response.data.is_empty() {
            Err(FetchJsonError::NoContent)
        } else {
            serde_json::from_slice::<S>(&response.data)
                .map_err(|err| FetchJsonError::Json(err.to_string()))
        }
    } else {
        log::error!(
            "Fetch Error {}: {}",
            status.raw_code(),
            String::from_utf8_lossy(&response.data)
        );
        if status.raw_code() == 400 {
            match serde_json::from_slice::<E>(&response.data) {
                Ok(err) => Err(FetchJsonError::Other(err)),
                Err(err) => Err(FetchJsonError::Json(err.to_string())),
            }
        } else {
            Err(FetchJsonError::Network)
        }
    }
}
