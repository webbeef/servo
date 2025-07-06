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

use constellation_traits::{
    AtProtoCurrentSession, AtProtoError, AtProtoErrorKind, AtProtoNewSession,
    AtProtoRefreshSession, AtProtoResult,
};
use headers::{Authorization, HeaderMap, HeaderMapExt};
use http::Method;
use log::error;
use net_traits::CoreResourceThread;
use net_traits::fetch::utils::{FetchJsonError, fetch_json};
use serde::Deserialize;
use servo_url::ServoUrl;
use sync_wrapper::SyncWrapper;

use crate::atproto::pds::get_endpoint_for_subject;

pub struct SessionClient {}

pub type AtProtoResultResponseCallback = Box<dyn FnOnce(AtProtoResult) + Send>;

impl SessionClient {
    pub async fn create(
        handle: &str,
        password: &str,
        resource_thread: SyncWrapper<CoreResourceThread>,
    ) -> AtProtoResult {
        let Ok((endpoint, _doc)) = get_endpoint_for_subject(handle, None, None).await else {
            return AtProtoResult::Error;
        };

        let Ok(url) = endpoint.join("/xrpc/com.atproto.server.createSession") else {
            return AtProtoResult::Error;
        };

        match fetch_json::<AtProtoNewSession, AtProtoError>(
            url,
            &[("identifier", handle), ("password", password)],
            resource_thread,
            Method::POST,
            None,
            false,
        )
        .await
        {
            Ok(session) => AtProtoResult::NewSession(session, endpoint),
            Err(err) => {
                error!("create error: {err:?}");
                AtProtoResult::Error
            },
        }
    }

    pub async fn current(
        endpoint: &ServoUrl,
        resource_thread: SyncWrapper<CoreResourceThread>,
    ) -> AtProtoResult {
        let Ok(url) = endpoint.join("/xrpc/com.atproto.server.getSession") else {
            return AtProtoResult::Error;
        };

        match fetch_json::<AtProtoCurrentSession, AtProtoError>(
            url,
            &[],
            resource_thread,
            Method::GET,
            None,
            true,
        )
        .await
        {
            Ok(session) => AtProtoResult::CurrentSession(session),
            Err(FetchJsonError::Other(err)) => {
                if err.error == AtProtoErrorKind::ExpiredToken {
                    AtProtoResult::RefreshRequired
                } else {
                    error!("current error: {err:?}");
                    AtProtoResult::Error
                }
            },
            Err(err) => {
                error!("current error: {err:?}");
                AtProtoResult::Error
            },
        }
    }

    pub async fn delete(
        endpoint: &ServoUrl,
        resource_thread: SyncWrapper<CoreResourceThread>,
        refresh_jwt: &str,
    ) -> AtProtoResult {
        let Ok(url) = endpoint.join("/xrpc/com.atproto.server.deleteSession") else {
            return AtProtoResult::Error;
        };

        #[derive(Deserialize)]
        struct DeleteSessionResult {}

        let mut headers = HeaderMap::new();
        headers.typed_insert(Authorization::bearer(refresh_jwt).unwrap());

        match fetch_json::<DeleteSessionResult, AtProtoError>(
            url,
            &[],
            resource_thread,
            Method::POST,
            Some(headers),
            false,
        )
        .await
        {
            Ok(_) | Err(FetchJsonError::NoContent) => AtProtoResult::Logout,
            Err(FetchJsonError::Other(err)) => {
                if err.error == AtProtoErrorKind::ExpiredToken {
                    AtProtoResult::RefreshRequired
                } else {
                    error!("delete error: {err:?}");
                    AtProtoResult::Error
                }
            },
            Err(err) => {
                error!("delete error: {err:?}");
                AtProtoResult::Error
            },
        }
    }

    pub async fn refresh(
        endpoint: &ServoUrl,
        resource_thread: SyncWrapper<CoreResourceThread>,
        refresh_jwt: &str,
    ) -> Option<AtProtoRefreshSession> {
        let Ok(url) = endpoint.join("/xrpc/com.atproto.server.refreshSession") else {
            return None;
        };

        let mut headers = HeaderMap::new();
        headers.typed_insert(Authorization::bearer(refresh_jwt).unwrap());

        fetch_json::<AtProtoRefreshSession, AtProtoError>(
            url,
            &[],
            resource_thread,
            Method::POST,
            Some(headers),
            false,
        )
        .await
        .ok()
    }
}
