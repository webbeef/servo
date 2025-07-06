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

use std::sync::Arc;

use constellation_traits::{AtProtoRequest, AtProtoResult};
use ipc_channel::ipc::{IpcSender, channel};
use log::error;
use net::async_runtime::spawn_task;
use net::atproto::session::SessionClient;
use net_traits::{AtProtoSessionState, CoreResourceMsg, CoreResourceThread};
use parking_lot::Mutex;
use servo_url::ServoUrl;
use sync_wrapper::SyncWrapper;

pub(crate) struct AtProtoManager {
    resource_thread: CoreResourceThread,
    session: Arc<Mutex<Option<AtProtoSessionState>>>,
}

impl AtProtoManager {
    pub(crate) fn new(resource_thread: CoreResourceThread) -> Self {
        let (tx, rx) = channel().expect("Failed to create IPC channel");
        let _ = resource_thread.send(CoreResourceMsg::GetAtProtoSession(tx));
        let session = rx.recv().unwrap_or(None);

        println!("ATProto session: {session:?}");

        Self {
            resource_thread,
            session: Arc::new(Mutex::new(session)),
        }
    }

    pub(crate) fn process_request(
        &self,
        request: AtProtoRequest,
        response: IpcSender<AtProtoResult>,
    ) {
        match request {
            AtProtoRequest::Login(login, password) => self.login(login, password, response),
            AtProtoRequest::Logout => self.logout(response),
            AtProtoRequest::Current => self.current(response),
        }
    }

    fn login(&self, handle: String, password: String, response: IpcSender<AtProtoResult>) {
        let resource_thread = SyncWrapper::new(self.resource_thread.clone());
        let resource_thread2 = SyncWrapper::new(self.resource_thread.clone());
        let local_session = Arc::clone(&self.session);

        spawn_task(async move {
            let result = SessionClient::create(&handle, &password, resource_thread).await;

            if let AtProtoResult::NewSession(ref session, ref endpoint_url) = result {
                Self::update_session_state(
                    endpoint_url,
                    &session.access_jwt,
                    &session.refresh_jwt,
                    &resource_thread2.into_inner(),
                    local_session,
                );
            }
            if let Err(err) = response.send(result) {
                error!("Failed to send new session: {err:?}");
            }
        });
    }

    fn logout(&self, response: IpcSender<AtProtoResult>) {
        let Some(ref session) = *self.session.lock() else {
            error!("No session available");
            let _ = response.send(AtProtoResult::Error);
            return;
        };

        let resource_thread = SyncWrapper::new(self.resource_thread.clone());
        let resource_thread2 = SyncWrapper::new(self.resource_thread.clone());
        let resource_thread3 = SyncWrapper::new(self.resource_thread.clone());
        let resource_thread4 = SyncWrapper::new(self.resource_thread.clone());
        let resource_thread5 = SyncWrapper::new(self.resource_thread.clone());

        let session = session.clone();
        let local_session = Arc::clone(&self.session);
        let local_session2 = Arc::clone(&self.session);

        spawn_task(async move {
            let mut result =
                SessionClient::delete(&session.endpoint, resource_thread, &session.refresh_jwt)
                    .await;
            if let AtProtoResult::RefreshRequired = result {
                error!("RefreshRequired");
                if let Some(refresh_session) = SessionClient::refresh(
                    &session.endpoint,
                    resource_thread2,
                    &session.refresh_jwt,
                )
                .await
                {
                    Self::update_session_state(
                        &session.endpoint,
                        &refresh_session.access_jwt,
                        &refresh_session.refresh_jwt,
                        &resource_thread3.into_inner(),
                        local_session,
                    );

                    result = SessionClient::delete(
                        &session.endpoint,
                        resource_thread4,
                        &session.refresh_jwt,
                    )
                    .await;
                } else {
                    result = AtProtoResult::Error;
                }
            }

            // Reset the ATProto session to an empty one.
            match result {
                AtProtoResult::Error => {},
                _ => {
                    let mut lock = local_session2.lock();
                    *lock = None;
                    let _ = resource_thread5
                        .into_inner()
                        .send(CoreResourceMsg::UpdateAtProtoSession(None));
                },
            }

            // Finally send the result.
            if let Err(err) = response.send(result) {
                error!("Failed to send current session: {err:?}");
            }
        });
    }

    fn current(&self, response: IpcSender<AtProtoResult>) {
        let Some(ref session) = *self.session.lock() else {
            error!("No session available");
            let _ = response.send(AtProtoResult::Error);
            return;
        };

        let resource_thread = SyncWrapper::new(self.resource_thread.clone());
        let resource_thread2 = SyncWrapper::new(self.resource_thread.clone());
        let resource_thread3 = SyncWrapper::new(self.resource_thread.clone());
        let resource_thread4 = SyncWrapper::new(self.resource_thread.clone());

        let session = session.clone();
        let local_session = Arc::clone(&self.session);

        spawn_task(async move {
            let mut result = SessionClient::current(&session.endpoint, resource_thread).await;
            if let AtProtoResult::RefreshRequired = result {
                error!("RefreshRequired");
                if let Some(refresh_session) = SessionClient::refresh(
                    &session.endpoint,
                    resource_thread2,
                    &session.refresh_jwt,
                )
                .await
                {
                    Self::update_session_state(
                        &session.endpoint,
                        &refresh_session.access_jwt,
                        &refresh_session.refresh_jwt,
                        &resource_thread3.into_inner(),
                        local_session,
                    );

                    result = SessionClient::current(&session.endpoint, resource_thread4).await;
                } else {
                    result = AtProtoResult::Error;
                }
            }

            if let Err(err) = response.send(result) {
                error!("Failed to send current session: {err:?}");
            }
        });
    }

    // Register the new session state.
    fn update_session_state(
        endpoint: &ServoUrl,
        access_jwt: &str,
        refresh_jwt: &str,
        resource_thread: &CoreResourceThread,
        local_session: Arc<Mutex<Option<AtProtoSessionState>>>,
    ) {
        // Update the HTTP state.
        let session_msg = AtProtoSessionState {
            endpoint: endpoint.clone(),
            access_jwt: access_jwt.to_owned(),
            refresh_jwt: refresh_jwt.to_owned(),
        };

        let mut lock = local_session.lock();
        *lock = Some(session_msg.clone());

        let _ = resource_thread.send(CoreResourceMsg::UpdateAtProtoSession(Some(session_msg)));
    }
}
